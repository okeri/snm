use super::parsers::{parse, Parsers};
use super::support;
use super::types::{ConnectionInfo, ConnectionSetting};
use nix::libc;
use smoltcp::dhcp::Dhcpv4Client;
use smoltcp::iface::{EthernetInterfaceBuilder, NeighborCache, Routes};
use smoltcp::phy::{wait, RawSocket};
use smoltcp::socket::{RawPacketMetadata, RawSocketBuffer, SocketSet};
use smoltcp::time::{Duration, Instant};
use smoltcp::wire::{EthernetAddress, IpCidr, Ipv4Address, Ipv4Cidr};
use std::collections::{BTreeMap, HashSet};
use std::os::unix::io::AsRawFd;
use std::{fmt, fs, path::Path};

#[derive(Hash, Eq, PartialEq, Clone)]
pub struct Interface(String);

impl Interface {
    pub fn new(name: &str) -> Self {
        Interface { 0: name.to_owned() }
    }

    pub fn disconnect(&self) {
        support::run(&format!("ip addr flush dev {}", self.0), false);
        support::run(
            &format!("wpa_cli -i {} -p /var/run/wpa disconnect", self.0),
            false,
        );
        support::run(
            &format!("wpa_cli -i {} -p /var/run/wpa terminate", self.0),
            false,
        );
    }

    pub fn scan(&self) -> String {
        if self.valid() {
            support::run(&format!("iw dev {} scan", self.0), false)
        } else {
            "".to_owned()
        }
    }

    pub fn up(&self) {
        if self.valid() {
            support::run(&format!("ip l set {} up", self.0), false);
        }
    }

    pub fn down(&self) {
        if self.valid() {
            support::run(&format!("ip l set {} down", self.0), false);
        }
    }

    pub fn is_plugged_in(&self) -> bool {
        if self.valid() {
            let filename = format!("/sys/class/net/{}/carrier", self.0);
            if let Ok(value) = fs::read_to_string(&filename) {
                return value == "1\n";
            }
        }
        false
    }

    pub fn is_up(&self) -> bool {
        if self.valid() {
            let filename = format!("/sys/class/net/{}/operstate", self.0);
            if let Ok(value) = fs::read_to_string(&filename) {
                return value == "up\n";
            }
        }
        false
    }

    fn valid(&self) -> bool {
        !self.0.is_empty()
    }

    fn set_addr(&mut self, addr: Ipv4Cidr) {
        support::run(&format!("ip addr add {} dev {}", addr, self.0), false);
    }

    fn set_route(&mut self, route: Ipv4Address) {
        support::run(
            &format!("ip route add default via {} dev {}", route, self.0),
            false,
        );
    }

    fn set_dns(&mut self, dns: [Option<Ipv4Address>; 3]) {
        use std::io::Write;
        let mut resolv =
            std::fs::File::create("/etc/resolv.conf").expect("cannot rewrite /etc/resolv.conf");
        for dns_server in dns.iter().filter_map(|s| *s) {
            writeln!(resolv, "nameserver {}\n", dns_server)
                .expect("cannot write /etc/resolve.conf");
        }
    }

    pub fn dhcp(&mut self) -> Result<String, ()> {
        let mut result: Result<String, ()> = Err(());
        if !self.valid() {
            return result;
        }
        let mac = self.detect_mac().expect("cannot detect HW addr");

        let device = RawSocket::new(&self.0).unwrap();
        let fd = device.as_raw_fd();
        let neighbor_cache = NeighborCache::new(BTreeMap::new());
        let ip_addrs = [IpCidr::new(Ipv4Address::UNSPECIFIED.into(), 0)];
        let mut routes_storage = [None; 1];
        let routes = Routes::new(&mut routes_storage[..]);
        let mut iface = EthernetInterfaceBuilder::new(device)
            .ethernet_addr(mac)
            .neighbor_cache(neighbor_cache)
            .ip_addrs(ip_addrs)
            .routes(routes)
            .finalize();

        let mut sockets = SocketSet::new(vec![]);
        let dhcp_rx_buffer = RawSocketBuffer::new([RawPacketMetadata::EMPTY; 1], vec![0; 900]);
        let dhcp_tx_buffer = RawSocketBuffer::new([RawPacketMetadata::EMPTY; 1], vec![0; 300]);

        let mut dhcp =
            Dhcpv4Client::new(&mut sockets, dhcp_rx_buffer, dhcp_tx_buffer, Instant::now());

        let mut tries = 0;

        let delay = Duration::from_millis(500);
        while result.is_err() && tries < 10 {
            let timestamp = Instant::now();

            if iface.poll(&mut sockets, timestamp).is_ok() {
                if let Ok(config) = dhcp.poll(&mut iface, &mut sockets, timestamp) {
                    if let Some(config) = config {
                        config.address.map(|cidr| {
                            iface.update_ip_addrs(|addrs| {
                                addrs.iter_mut().nth(0).map(|addr| {
                                    *addr = IpCidr::Ipv4(cidr);
                                });
                            });
                            self.set_addr(cidr);
                            result = Ok(cidr.address().to_string());
                        });

                        config.router.map(|router| {
                            self.set_route(router);
                        });

                        if config.dns_servers.iter().any(|s| s.is_some()) {
                            self.set_dns(config.dns_servers);
                        }
                    }

                    wait(fd, Some(delay)).unwrap_or(());
                    tries += 1;
                }
            }
        }
        return result;
    }

    fn detect_mac(&self) -> Result<EthernetAddress, ()> {
        use nix::{ifaddrs::getifaddrs, sys::socket::SockAddr};
        let ifaces = getifaddrs().map_err(|_| ())?;

        for iface in ifaces {
            if iface.interface_name == self.0 {
                if let Some(addr) = iface.address {
                    if let SockAddr::Link(link) = addr {
                        return Ok(EthernetAddress(link.addr()));
                    }
                }
            }
        }
        Err(())
    }

    fn detect_ip(&self) -> Option<String> {
        let ok: bool;
        let ifreq = ifreq_ip::new(&self.0);
        unsafe {
            let fd = libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0);
            ok = libc::ioctl(fd, libc::SIOCGIFADDR, &ifreq) != -1;
            libc::close(fd);
        }
        if ok {
            let addr = ifreq.ifr_addr.sin_addr;
            Some(format!("{}.{}.{}.{}", addr[0], addr[1], addr[2], addr[3]))
        } else {
            None
        }
    }

    pub fn wlan_info(&self) -> ConnectionInfo {
        use std::str;
        let output = support::run(&format!("iw dev {} link", self.0), false);

        if let Some(ref ecaps) = parse(Parsers::NetworkEssid, &output) {
            if let Some(ip) = self.detect_ip() {
                let parsed = support::parse_essid(ecaps.get(1).unwrap().as_str());
                if let Ok(value) = str::from_utf8(&parsed) {
                    let mut quality = 100;

                    if let Some(ref caps) = parse(Parsers::NetworkQuality, &output) {
                        quality = support::dbm2perc(
                            caps.get(1).unwrap().as_str().parse::<i32>().unwrap_or(100),
                        );
                    }
                    return ConnectionInfo::Wifi(value.to_string(), quality, true, ip);
                }
            }
        }
        ConnectionInfo::NotConnected
    }

    pub fn eth_info(&self) -> ConnectionInfo {
        if let Some(ip) = self.detect_ip() {
            ConnectionInfo::Ethernet(ip)
        } else {
            ConnectionInfo::NotConnected
        }
    }
}

impl fmt::Display for Interface {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone)]
pub struct Interfaces {
    eth_ifaces: HashSet<Interface>,
    wlan_ifaces: HashSet<Interface>,
}

impl Interfaces {
    pub fn new() -> Interfaces {
        let mut result = Interfaces {
            eth_ifaces: HashSet::new(),
            wlan_ifaces: HashSet::new(),
        };
        result.detect();
        result
    }

    pub fn from_setting(&self, setting: &ConnectionSetting) -> Option<Interface> {
        match *setting {
            ConnectionSetting::Ethernet => self.eth(),
            _ => self.wlan(),
        }
    }

    pub fn detect(&mut self) {
        for entry in fs::read_dir(&Path::new("/sys/class/net")).expect("no sysfs entry") {
            if let Ok(entry) = entry {
                let path = entry.file_name();
                if let Some(iface_name) = path.to_str() {
                    if let Some(sym) = iface_name.chars().next() {
                        match sym {
                            'e' => {
                                let iface = Interface::new(iface_name);
                                if !self.eth_ifaces.contains(&iface) {
                                    println!("Detected ethernet interface: {}", iface_name);
                                    iface.up();
                                    self.eth_ifaces.insert(iface);
                                } else {
                                    iface.up();
                                }
                            }
                            'w' => {
                                let iface = Interface::new(iface_name);
                                if !self.wlan_ifaces.contains(&iface) {
                                    println!("Detected wifi interface: {}", iface_name);
                                    self.wlan_ifaces.insert(iface);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    pub fn disconnect(&self) {
        for iface in self.eth_ifaces.iter() {
            iface.disconnect();
        }
        for iface in self.wlan_ifaces.iter() {
            iface.disconnect();
        }
    }

    pub fn eth(&self) -> Option<Interface> {
        Self::most_used_iface(&self.eth_ifaces)
    }

    pub fn wlan(&self) -> Option<Interface> {
        Self::most_used_iface(&self.wlan_ifaces)
    }

    fn most_used_iface(ifaces: &HashSet<Interface>) -> Option<Interface> {
        match ifaces.len() {
            0 => None,
            1 => ifaces.iter().next().map(|e| e.clone()),
            _ => {
                if let Some(plugged) = ifaces.iter().find(|iface| iface.is_plugged_in()) {
                    Some(plugged.clone())
                } else if let Some(up) = ifaces.iter().find(|iface| iface.is_up()) {
                    Some(up.clone())
                } else {
                    ifaces.iter().next().map(|e| e.clone())
                }
            }
        }
    }
}

#[repr(C)]
struct sockaddr_in {
    pub sin_family: libc::sa_family_t,
    pub sin_port: libc::in_port_t,
    pub sin_addr: [u8; 4],
    pub sin_zero: [u8; 8],
}

#[repr(C)]
struct ifreq_ip {
    ifr_name: [libc::c_uchar; libc::IF_NAMESIZE],
    pub ifr_addr: sockaddr_in,
}

impl ifreq_ip {
    fn new(ifname: &str) -> Self {
        let mut ifr_name = [0; libc::IF_NAMESIZE];
        ifr_name[..ifname.len()].clone_from_slice(ifname.as_bytes());
        ifreq_ip {
            ifr_name,
            ifr_addr: sockaddr_in {
                sin_family: libc::AF_INET as libc::sa_family_t,
                sin_port: 0,
                sin_addr: [0xff; 4],
                sin_zero: [0; 8],
            },
        }
    }
}
