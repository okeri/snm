use super::support;
use super::types::ConnectionSetting;
use std::collections::HashSet;
use std::{fmt, fs, path::Path};

#[derive(Hash, Eq, PartialEq, Clone)]
pub struct Interface(String);

impl Interface {
    pub fn new(name: &str) -> Self {
        Interface { 0: name.to_owned() }
    }

    pub fn disconnect(&self) {
        support::run(&format!("dhcpcd -k {}", self.0), false);
        support::run(&format!("ip addr flush dev {}", self.0), false);
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
                return value == "on\n";
            }
        }
        false
    }

    fn valid(&self) -> bool {
        !self.0.is_empty()
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
