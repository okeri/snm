mod interfaces;
mod parsers;
mod signalmsg;
mod support;
mod types;

use interfaces::Interfaces;
use parsers::{parse, Parsers};
pub use signalmsg::SignalMsg;
pub use types::*;

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex, RwLock,
};
use std::{fs, path::Path, thread, time};

const AUTH_MAX_TRIES: usize = 30;
const ASSOC_MAX_TRIES: usize = 12;
const SHORT_INTERVAL: u32 = 30;
const LONG_INTERVAL: u32 = 1800;
const ROAMING_DB_PATH: &str = "/etc/snm/roaming.db";

#[derive(Clone)]
pub struct Connection<SignalHandler>
where
    SignalHandler: FnMut(SignalMsg),
{
    ifaces: Arc<Mutex<Interfaces>>,
    tries: Arc<AtomicUsize>,
    current: Arc<RwLock<ConnectionInfo>>,
    networks: Arc<Mutex<NetworkList>>,
    signal_handler: SignalHandler,
}

impl<SignalHandler> Connection<SignalHandler>
where
    SignalHandler: FnMut(SignalMsg),
{
    fn wait_for_auth(&self, iface: &str) -> bool {
        let mut tries = 0;
        while tries < self.tries.load(Ordering::SeqCst) {
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
            tries += 1;
        }
        false
    }

    fn signal(&mut self, s: SignalMsg) {
        (self.signal_handler)(s);
    }

    fn aborted(&mut self) -> bool {
        let result = self.tries.load(Ordering::SeqCst) == 0;
        if result {
            self.signal(SignalMsg::ConnectStatusChanged(ConnectionStatus::Aborted));
        }
        result
    }

    fn generate_wpa_config(setting: &ConnectionSetting) -> Option<String> {
        match *setting {
            ConnectionSetting::Wifi {
                ref essid,
                ref password,
                threshold,
            } => support::gen_wpa_config(
                essid,
                Some(password),
                threshold,
                ROAMING_DB_PATH,
                SHORT_INTERVAL,
                LONG_INTERVAL,
            )
            .ok(),
            ConnectionSetting::OpenWifi {
                ref essid,
                threshold,
            } => support::gen_wpa_config(
                essid,
                None,
                threshold,
                ROAMING_DB_PATH,
                SHORT_INTERVAL,
                LONG_INTERVAL,
            )
            .ok(),
            _ => None,
        }
    }

    fn change_state(&mut self, info: ConnectionInfo) {
        *self.current.write().unwrap() = info.clone();
        self.signal(SignalMsg::StateChanged(info));
    }

    fn get_network(&self, essid: &str) -> Result<NetworkInfo, ()> {
        if let Ok(networks) = self.networks.lock() {
            let result = networks.iter().find(|network| {
                if let NetworkInfo::Wifi(net_essid, ..) = network {
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
        if let NetworkInfo::Wifi(ref new_essid, ref new_q, ref new_enc) = new_network {
            for network in networks.iter_mut() {
                if let NetworkInfo::Wifi(ref mut essid, ref mut q, ref mut enc) = network {
                    if essid == new_essid {
                        if new_q > q {
                            *q = *new_q;
                            *enc = *new_enc;
                        }
                        return;
                    }
                }
            }
        }
        networks.push(new_network);
    }

    pub fn new(signal_handler: SignalHandler) -> Self {
        Connection {
            ifaces: Arc::new(Mutex::new(Interfaces::new())),
            tries: Arc::new(AtomicUsize::new(0)),
            current: Arc::new(RwLock::new(ConnectionInfo::NotConnected)),
            networks: Arc::new(Mutex::new(NetworkList::new())),
            signal_handler,
        }
    }

    pub fn connect(&mut self, setting: ConnectionSetting) -> bool {
        self.tries.store(AUTH_MAX_TRIES, Ordering::SeqCst);
        let mut network = NetworkInfo::Ethernet;
        let iface = self.ifaces.lock().unwrap().from_setting(&setting);

        let connection = self.current.read().unwrap().clone();

        if connection.active() {
            self.disconnect();
        }

        match setting {
            ConnectionSetting::Wifi { ref essid, .. }
            | ConnectionSetting::OpenWifi { ref essid, .. } => {
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
                if !Interfaces::plugged_in(&iface) {
                    return false;
                }
            }
        }
        self.signal(SignalMsg::ConnectStatusChanged(
            ConnectionStatus::Initializing,
        ));

        let wpa_config = Self::generate_wpa_config(&setting);
        let erase_wpa_config = || {
            if let Some(ref path) = wpa_config {
                fs::remove_file(Path::new(path)).unwrap_or_default();
            }
        };

        support::run(&format!("ip l set {} up", iface), false);

        if self.aborted() {
            erase_wpa_config();
            return false;
        }
        let need_auth = setting.need_auth();
        if let Some(ref c) = wpa_config {
            if need_auth {
                self.signal(SignalMsg::ConnectStatusChanged(
                    ConnectionStatus::Authenticating,
                ));
            } else {
                self.tries.store(ASSOC_MAX_TRIES, Ordering::SeqCst);
                self.signal(SignalMsg::ConnectStatusChanged(
                    ConnectionStatus::Connecting,
                ));
            }
            support::run(
                &format!(
                    "wpa_supplicant -B -i{} -c{} -Dnl80211 -C/var/run/wpa",
                    iface, c
                ),
                false,
            );
            if !self.wait_for_auth(&iface) {
                if !self.aborted() {
                    self.signal(SignalMsg::ConnectStatusChanged(if need_auth {
                        ConnectionStatus::AuthFail
                    } else {
                        ConnectionStatus::ConnectFail
                    }));
                }
                erase_wpa_config();
                return false;
            }
        }
        self.signal(SignalMsg::ConnectStatusChanged(ConnectionStatus::GettingIP));
        let output = support::run(&format!("dhcpcd -4 -i {}", iface), true);
        if let Some(ref caps) = parse(Parsers::Ip, &output) {
            let info = match setting {
                ConnectionSetting::Ethernet => ConnectionInfo::Ethernet(caps[1].to_string()),

                ConnectionSetting::Wifi { essid, .. }
                | ConnectionSetting::OpenWifi { essid, .. } => {
                    if let NetworkInfo::Wifi(_, ref quality, ref enc) = network {
                        ConnectionInfo::Wifi(essid.to_string(), *quality, *enc, caps[1].to_string())
                    } else {
                        ConnectionInfo::NotConnected
                    }
                }
            };
            erase_wpa_config();
            self.change_state(info);
            return true;
        }
        erase_wpa_config();
        false
    }

    pub fn acquire(&self) {
        // TODO: find already established connections
    }

    pub fn disconnect(&mut self) {
        self.tries.store(0, Ordering::SeqCst);
        if let Ok(ifaces) = self.ifaces.lock() {
            ifaces.disconnect();
        }
        self.change_state(ConnectionInfo::NotConnected);
    }

    pub fn auto_connect_possible(&mut self, known_networks: &KnownNetworks) -> CouldConnect {
        let mut eth_plugged_in = false;
        let mut wifi_plugged_in = false;
        if let Ok(mut ifaces) = self.ifaces.lock() {
            ifaces.detect();
            eth_plugged_in = ifaces.eth_plugged_in();
            wifi_plugged_in = ifaces.wlan_plugged_in();
        }

        let add_phantom_eth = || {
            let mut networks = self.networks.lock().unwrap();
            if networks.len() > 0 {
                if let NetworkInfo::Wifi(..) = networks[0] {
                    networks.insert(0, NetworkInfo::Ethernet);
                    return Some(networks.clone());
                }
            }
            None
        };

        let remove_phantom_eth = || {
            let mut networks = self.networks.lock().unwrap();
            if networks.len() > 0 {
                if let NetworkInfo::Ethernet = networks[0] {
                    networks.remove(0);
                    return Some(networks.clone());
                }
            }
            None
        };

        let remove_wlan_if_nonexists = |essid: &str| {
            let mut networks = self.networks.lock().unwrap();
            let result = networks.iter().position(|x| {
                return if let NetworkInfo::Wifi(name, ..) = x {
                    name == essid
                } else {
                    false
                };
            });
            if let Some(index) = result {
                networks.remove(index);
                return Some(networks.clone());
            }
            None
        };

        let connection = self.current.read().unwrap().clone();
        match connection {
            ConnectionInfo::NotConnected => {
                if eth_plugged_in {
                    if let Some(networks) = add_phantom_eth() {
                        self.signal(SignalMsg::NetworkList(networks));
                    }
                    return CouldConnect::Connect(ConnectionSetting::Ethernet);
                } else {
                    let networks = self.networks.lock().unwrap();
                    if networks.len() == 0 {
                        return CouldConnect::Rescan;
                    }
                    for n in networks.iter() {
                        if let NetworkInfo::Wifi(ref essid, ..) = n {
                            if let Some(ref known) = known_networks.get(essid) {
                                if known.auto {
                                    return CouldConnect::Connect(known.to_setting(essid));
                                }
                            }
                        }
                    }
                }
            }

            ConnectionInfo::Wifi(ref essid, ..) => {
                if eth_plugged_in {
                    return CouldConnect::Connect(ConnectionSetting::Ethernet);
                } else if !wifi_plugged_in {
                    if let Some(networks) = remove_wlan_if_nonexists(essid) {
                        self.signal(SignalMsg::NetworkList(networks));
                    }
                    return CouldConnect::Disconnect;
                }
            }

            ConnectionInfo::Ethernet(_) => {
                if !eth_plugged_in {
                    if let Some(networks) = remove_phantom_eth() {
                        self.signal(SignalMsg::NetworkList(networks));
                    }
                    return CouldConnect::Disconnect;
                }
            }
            _ => {}
        }
        CouldConnect::DoNothing
    }

    pub fn scan(&mut self) {
        use std::str;

        let dbm2perc = |dbm: i32| -> u32 {
            if dbm < -92 {
                1
            } else if dbm > -21 {
                100
            } else {
                let x = dbm as f32;
                ((-0.0154 * x * x) - (0.3794 * x) + 98.182).round() as u32
            }
        };

        let mut networks = NetworkList::new();

        let ifaces = self.ifaces.lock().unwrap().clone();
        if ifaces.eth_plugged_in() {
            networks.push(NetworkInfo::Ethernet);
        }

        if let Some(wlan) = ifaces.get_wlan() {
            let down = ifaces.wlan_plugged_in();
            if down {
                ifaces.wlan_up();
            }

            let output = ifaces.wlan_scan();

            let mut quality: u32;
            let mut essid: String;
            let mut enc: bool;
            for chunk in output.split(&format!("(on {})", wlan)) {
                quality = 0;
                enc = true;
                essid = "".to_string();
                if let Some(ref caps) = parse(Parsers::NetworkQuality, chunk) {
                    quality = dbm2perc(
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
                    Self::add_wifi_network(&mut networks, NetworkInfo::Wifi(essid, quality, enc));
                }
            }
            networks.as_mut_slice().sort();
        }
        *self.networks.lock().unwrap() = networks.clone();
        self.signal(SignalMsg::NetworkList(networks));
    }

    pub fn allow_reconnect(&self) -> bool {
        !self.current.read().unwrap().wired()
    }

    pub fn current_state(&self) -> ConnectionInfo {
        self.current.read().unwrap().clone()
    }

    pub fn get_networks(&self) -> NetworkList {
        self.networks.lock().unwrap().clone()
    }
}