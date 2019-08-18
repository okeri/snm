use connection_types::*;
use network_info::NetworkInfo;
use parsers::{parse, Parsers};
use signalmsg::SignalMsg;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc, Arc, Mutex, RwLock,
};
use std::{thread, time};
use support;

const AUTH_MAX_TRIES: usize = 30;
const ASSOC_MAX_TRIES: usize = 12;

pub type SignalMsgHandler = mpsc::Sender<SignalMsg>;

#[derive(Clone)]
struct Interfaces {
    eth: String,
    wlan: String,
}

pub enum CouldConnect {
    Connect(ConnectionSetting),
    Disconnect,
    Rescan,
    DoNothing,
}

impl Interfaces {
    fn detect(&mut self) {
        use std::fs;
        use std::path::Path;
        for entry in fs::read_dir(&Path::new("/sys/class/net")).expect("no sysfs entry") {
            let entry = entry.unwrap().file_name();
            let path = entry;
            let iface = path.to_str().unwrap();
            match iface.chars().next().unwrap() {
                'e' => {
                    if self.eth.is_empty() {
                        println!("Detected ethernet interface: {}", iface);
                    }
                    self.eth = iface.to_string();
                    support::run(&format!("ip l set {} up", self.eth), false);
                }
                'w' => {
                    if self.wlan.is_empty() {
                        println!("Detected wifi interface: {}", iface);
                    }
                    self.wlan = iface.to_string();
                    support::run(&format!("ip l set {} up", self.wlan), false);
                }
                _ => {}
            }
        }
    }

    fn new() -> Interfaces {
        let mut result = Interfaces {
            eth: "".to_string(),
            wlan: "".to_string(),
        };
        result.detect();
        result
    }

    fn from_setting(&self, setting: &ConnectionSetting) -> String {
        match *setting {
            ConnectionSetting::Ethernet => self.eth.clone(),
            _ => self.wlan.clone(),
        }
    }
}

#[derive(Clone)]
pub struct Connection {
    ifaces: Arc<Mutex<Interfaces>>,
    tries: Arc<AtomicUsize>,
    current: Arc<RwLock<ConnectionInfo>>,
    wpa_config: Arc<Mutex<Option<String>>>,
    networks: Arc<Mutex<Vec<NetworkInfo>>>,
    signal: SignalMsgHandler,
}

impl Connection {
    fn plugged_in(iface: &str) -> bool {
        let filename = format!("/sys/class/net/{}/operstate", iface);
        if let Ok(value) = support::read_file(&filename) {
            value == "up"
        } else {
            false
        }
    }

    fn wait_for_auth(&self, iface: &str) -> bool {
        let mut try = 0;
        while try < self.tries.load(Ordering::SeqCst) {
            let output = support::run(
                &format!("wpa_cli -i {} -p /var/run/wpa status", iface),
                false,
            );
            if let Some(ref caps) = parse(Parsers::WpaState, &output) {
                let result = caps[1].as_ref();
                if let "COMPLETED" = result {
                    return true;
                }
            }
            thread::sleep(time::Duration::from_secs(1));
            try += 1;
        }
        false
    }

    fn wait_for_assoc(&self, iface: &str) -> bool {
        let mut try = 0;

        while try < self.tries.load(Ordering::SeqCst) {
            let output = support::run(&format!("iwconfig {}", iface), false);
            if let Some(ref caps) = parse(Parsers::Associated, &output) {
                if "Not-Associated" != caps[1].trim() {
                    return true;
                }
            }
            thread::sleep(time::Duration::from_secs(1));
            try += 1;
        }
        false
    }

    fn aborted(&self) -> bool {
        let result = self.tries.load(Ordering::SeqCst) == 0;
        if result {
            self.signal
                .send(SignalMsg::ConnectStatusChanged(ConnectionStatus::Aborted))
                .unwrap();
        }
        result
    }

    fn generate_wpa_config(setting: &ConnectionSetting) -> Option<String> {
        match *setting {
            ConnectionSetting::Wifi {
                ref essid,
                ref password,
            } => {
                use std::fs::File;
                use std::io::Write;
                let filename = support::mktemp();
                let config = support::run(
                    &format!("wpa_passphrase \"{}\" \"{}\"", essid, password),
                    false,
                );
                let mut file =
                    File::create(&filename).expect("Cannot open wpa config file for writing");
                file.write_all(config.as_bytes())
                    .expect("Cannot write wpa config file");

                Some(filename)
            }
            _ => None,
        }
    }

    fn change_state(&self, info: ConnectionInfo) {
        *self.current.write().unwrap() = info.clone();
        self.signal.send(SignalMsg::StateChanged(info)).ok();
    }

    fn get_network(&self, essid: &str) -> Result<NetworkInfo, ()> {
        if let Ok(networks) = self.networks.lock() {
            let result = networks.iter().find(|network| {
                if let NetworkInfo::Wifi(net_essid, _, _, _) = network {
                    essid == net_essid
                } else {
                    false
                }
            });
            if let Some(network) = result {
                return Ok(network.clone());
            }
        }
        Err(())
    }

    fn add_wifi_network(networks: &mut Vec<NetworkInfo>, new_network: NetworkInfo) {
        if let NetworkInfo::Wifi(ref new_essid, ref new_q, ref new_enc, ref new_channel) =
            new_network
        {
            for network in networks.iter_mut() {
                if let NetworkInfo::Wifi(ref mut essid, ref mut q, ref mut enc, ref mut channel) =
                    network
                {
                    if essid == new_essid {
                        if new_q > q {
                            *q = *new_q;
                            *enc = *new_enc;
                            *channel = *new_channel;
                        }
                        return;
                    }
                }
            }
        }
        networks.push(new_network);
    }

    pub fn new(handler: SignalMsgHandler) -> Self {
        Connection {
            ifaces: Arc::new(Mutex::new(Interfaces::new())),
            tries: Arc::new(AtomicUsize::new(0)),
            current: Arc::new(RwLock::new(ConnectionInfo::NotConnected)),
            wpa_config: Arc::new(Mutex::new(None)),
            networks: Arc::new(Mutex::new(Vec::new())),
            signal: handler,
        }
    }

    pub fn connect(&self, setting: ConnectionSetting) -> bool {
        self.tries.store(AUTH_MAX_TRIES, Ordering::SeqCst);
        let mut network = NetworkInfo::Ethernet;
        let iface = self.ifaces.lock().unwrap().from_setting(&setting);

        let connection = self.current.read().unwrap().clone();

        if connection.active() {
            self.disconnect();
        }

        match setting {
            ConnectionSetting::Wifi { ref essid, .. }
            | ConnectionSetting::OpenWifi { ref essid } => {
                self.change_state(ConnectionInfo::ConnectingWifi(essid.to_string()));
                let network_found = self.get_network(essid);
                if let Ok(found) = network_found {
                    network = found;
                } else {
                    return false;
                }
            }
            ConnectionSetting::Ethernet => {
                self.change_state(ConnectionInfo::ConnectingEth);
                if !Connection::plugged_in(&iface) {
                    return false;
                }
            }
        }
        self.signal
            .send(SignalMsg::ConnectStatusChanged(
                ConnectionStatus::Initializing,
            ))
            .unwrap();
        let wpa_config = Connection::generate_wpa_config(&setting);

        support::run(&format!("ip l set {} up", iface), false);

        if self.aborted() {
            return false;
        }
        if let Some(ref c) = wpa_config {
            *self.wpa_config.lock().unwrap() = wpa_config.clone();
            self.signal
                .send(SignalMsg::ConnectStatusChanged(
                    ConnectionStatus::Authenticating,
                ))
                .unwrap();
            support::run(
                &format!(
                    "wpa_supplicant -B -i{} -c{} -Dnl80211 -C/var/run/wpa",
                    iface, c
                ),
                false,
            );
            if !self.wait_for_auth(&iface) {
                if !self.aborted() {
                    self.signal
                        .send(SignalMsg::ConnectStatusChanged(ConnectionStatus::AuthFail))
                        .unwrap();
                }
                return false;
            }
        } else if let ConnectionSetting::OpenWifi { ref essid } = setting {
            self.signal
                .send(SignalMsg::ConnectStatusChanged(
                    ConnectionStatus::Connecting,
                ))
                .unwrap();
            if let NetworkInfo::Wifi(_, _, _, ref channel) = network {
                support::run(
                    &format!("iwconfig {} essid -- {} channel {}", iface, essid, *channel),
                    false,
                );
                self.tries.store(ASSOC_MAX_TRIES, Ordering::SeqCst);
                if !self.wait_for_assoc(&iface) {
                    self.signal
                        .send(SignalMsg::ConnectStatusChanged(
                            ConnectionStatus::ConnectFail,
                        ))
                        .unwrap();
                    return false;
                }
            }
        }
        self.signal
            .send(SignalMsg::ConnectStatusChanged(ConnectionStatus::GettingIP))
            .unwrap();
        let output = support::run(&format!("dhcpcd -4 -i {}", iface), true);
        if let Some(ref caps) = parse(Parsers::Ip, &output) {
            let info = match setting {
                ConnectionSetting::Ethernet => ConnectionInfo::Ethernet(caps[1].to_string()),

                ConnectionSetting::Wifi { essid, .. } | ConnectionSetting::OpenWifi { essid } => {
                    if let NetworkInfo::Wifi(_, ref quality, ref enc, _) = network {
                        ConnectionInfo::Wifi(essid.to_string(), *quality, *enc, caps[1].to_string())
                    } else {
                        ConnectionInfo::NotConnected
                    }
                }
            };
            self.change_state(info);
            return true;
        }

        false
    }

    pub fn acquire(&self) {
        // TODO: find already established connections
    }

    pub fn disconnect(&self) {
        self.tries.store(0, Ordering::SeqCst);
        if let Ok(ifaces) = self.ifaces.lock() {
            let disconnect_iface = |ref iface| {
                support::run(&format!("dhcpcd -k {}", iface), false);
                support::run(&format!("ip addr flush dev {}", iface), false);
                support::run(
                    &format!("wpa_cli -i {} -p /var/run/wpa terminate", iface),
                    false,
                );
            };

            disconnect_iface(&ifaces.eth);
            disconnect_iface(&ifaces.wlan);
        }

        support::run("dhcpcd -x", false);
        if let Some(ref cfg_path) = *self.wpa_config.lock().expect("locking wpa_config failed") {
            use std::fs;
            use std::path::Path;
            fs::remove_file(Path::new(cfg_path)).unwrap();
        }
        *self.wpa_config.lock().unwrap() = None;
        self.change_state(ConnectionInfo::NotConnected);
    }

    pub fn auto_connect_possible(&self, known_networks: &KnownNetworks) -> CouldConnect {
        let mut eth_plugged_in = false;
        let mut wifi_plugged_in = false;
        if let Ok(mut ifaces) = self.ifaces.lock() {
            ifaces.detect();
            eth_plugged_in = Connection::plugged_in(&ifaces.eth);
            wifi_plugged_in = Connection::plugged_in(&ifaces.wlan);
        }

        let connection = self.current.read().unwrap().clone();
        match connection {
            ConnectionInfo::NotConnected => {
                if eth_plugged_in {
                    let mut networks = self.networks.lock().unwrap();
                    if networks.len() > 0 {
                        if let NetworkInfo::Wifi(_, _, _, _) = networks[0] {
                            networks.insert(0, NetworkInfo::Ethernet);
                        }
                    }
                    self.signal
                        .send(SignalMsg::NetworkList(networks.clone()))
                        .unwrap();
                    return CouldConnect::Connect(ConnectionSetting::Ethernet);
                } else {
                    let networks = self.networks.lock().unwrap();
                    if networks.len() == 0 {
                        return CouldConnect::Rescan;
                    }
                    for n in networks.iter() {
                        if let NetworkInfo::Wifi(ref essid, _, enc, _) = n {
                            if let Some(ref known) = known_networks.get(essid) {
                                if known.auto {
                                    if let Some(ref pass) = known.password {
                                        if *enc {
                                            return CouldConnect::Connect(
                                                ConnectionSetting::Wifi {
                                                    essid: essid.to_string(),
                                                    password: pass.to_string(),
                                                },
                                            );
                                        }
                                    } else if !enc {
                                        return CouldConnect::Connect(
                                            ConnectionSetting::OpenWifi {
                                                essid: essid.to_string(),
                                            },
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }

            ConnectionInfo::Wifi(ref essid, _, _, _) => {
                if eth_plugged_in {
                    return CouldConnect::Connect(ConnectionSetting::Ethernet);
                } else if !wifi_plugged_in {
                    if let Ok(mut networks) = self.networks.lock() {
                        let result = networks.iter().position(|x| {
                            return if let NetworkInfo::Wifi(name, _, _, _) = x {
                                name == essid
                            } else {
                                false
                            };
                        });
                        if let Some(index) = result {
                            networks.remove(index);
                            self.signal
                                .send(SignalMsg::NetworkList(networks.clone()))
                                .unwrap();
                        }
                    }
                    return CouldConnect::Disconnect;
                }
            }

            ConnectionInfo::Ethernet(_) => {
                if !eth_plugged_in {
                    let mut networks = self.networks.lock().unwrap();
                    if networks.len() > 0 {
                        if let NetworkInfo::Ethernet = networks[0] {
                            networks.remove(0);
                            self.signal
                                .send(SignalMsg::NetworkList(networks.clone()))
                                .unwrap();
                        }
                    }
                    return CouldConnect::Disconnect;
                }
            }
            _ => {}
        }
        CouldConnect::DoNothing
    }

    fn dbm2perc(dbm: i32) -> u32 {
        return if dbm < -92 {
            1
        } else if dbm > -21 {
            100
        } else {
            let x = dbm as f32;
            ((-0.0154 * x * x) - (0.3794 * x) + 98.182).round() as u32
        };
    }

    pub fn scan(&self) {
        use std::str;
        let mut networks = Vec::new();
        let ifaces = self.ifaces.lock().unwrap().clone();

        support::run(&format!("ip l set {} up", ifaces.wlan), false);
        let output = support::run(&format!("iw dev {} scan", ifaces.wlan), false);
        if Connection::plugged_in(&ifaces.eth) {
            networks.push(NetworkInfo::Ethernet);
        }

        let mut quality: u32;
        let mut essid: String;
        let mut enc: bool;
        let mut channel: u32;
        for chunk in output.split(&format!("(on {})", ifaces.wlan)) {
            quality = 0;
            channel = 0;
            enc = true;
            essid = "".to_string();
            if let Some(ref caps) = parse(Parsers::NetworkChannel, chunk) {
                channel = caps
                    .get(1)
                    .unwrap()
                    .as_str()
                    .parse::<u32>()
                    .expect("should be a value");
            }

            if let Some(ref caps) = parse(Parsers::NetworkQuality, chunk) {
                quality = Connection::dbm2perc(
                    caps.get(1)
                        .unwrap()
                        .as_str()
                        .parse::<i32>()
                        .expect("should be a value"),
                );
            }

            if let Some(ref caps) = parse(Parsers::NetworkEssid, chunk) {
                let parsed = support::parse_essid(caps.get(1).unwrap().as_str());
                let decoded = str::from_utf8(&parsed);
                if let Ok(value) = decoded {
                    essid = value.to_string();
                }
            }

            if let Some(ref caps) = parse(Parsers::NetworkEnc, chunk) {
                if caps.get(1).unwrap().as_str().matches("Privacy").count() == 0 {
                    enc = false;
                }
            }

            if !essid.is_empty() {
                Connection::add_wifi_network(
                    &mut networks,
                    NetworkInfo::Wifi(essid, quality, enc, channel),
                );
            }

            networks.as_mut_slice().sort();
        }
        *self.networks.lock().unwrap() = networks.clone();
        self.signal.send(SignalMsg::NetworkList(networks)).unwrap();
    }

    pub fn allow_reconnect(&self) -> bool {
        !self.current.read().unwrap().wired()
    }

    pub fn current_state(&self) -> ConnectionInfo {
        self.current.read().unwrap().clone()
    }

    pub fn get_networks(&self) -> Vec<NetworkInfo> {
        self.networks.lock().unwrap().clone()
    }
}
