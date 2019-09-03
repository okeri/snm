use config;
use connection::{Connection, CouldConnect};
use connection_types::*;
use dbus::tree::{DataType, MethodErr};
use dbus_interface;
use network_info::NetworkInfo;
use signalmsg::SignalMsg;
use std::{
    fmt,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        mpsc, Arc, Mutex,
    },
    thread, time,
};

const NETWORK_CHECK_INTERVAL: u64 = 2;
const NETWORK_SCAN_INTERVAL: u64 = 14;

pub struct NetworkManager {
    connection: Connection,
    auto_connect: Arc<AtomicBool>,
    client_count: Arc<AtomicU32>,
    running: Arc<AtomicBool>,
    monitor: Option<thread::JoinHandle<()>>,
    scanner: Option<thread::JoinHandle<()>>,
    known_networks: Arc<Mutex<KnownNetworks>>,
    sender: mpsc::Sender<ConnectionSetting>,
}

impl NetworkManager {
    fn create_monitor(
        &self,
        receiver: mpsc::Receiver<ConnectionSetting>,
        scanner: mpsc::Sender<()>,
    ) -> Option<thread::JoinHandle<()>> {
        let auto = self.auto_connect.clone();
        let client_count = self.client_count.clone();
        let running = self.running.clone();
        let connection = self.connection.clone();
        let known_networks = self.known_networks.clone();
        Some(thread::spawn(move || {
            let scan_iter = NETWORK_SCAN_INTERVAL / NETWORK_CHECK_INTERVAL;
            let mut iter = 0;
            let doscan = || {
                scanner.send(()).unwrap_or_default();
                0
            };
            connection.acquire();
            connection.scan();
            while running.load(Ordering::SeqCst) {
                let mut msg = Err(());
                loop {
                    let r = receiver.try_recv();
                    if r.is_err() {
                        break;
                    }
                    msg = Ok(r.unwrap());
                }

                if let Ok(setting) = msg {
                    if connection.connect(setting) {
                        auto.store(true, Ordering::SeqCst);
                    } else {
                        connection.disconnect();
                    }
                } else {
                    if auto.load(Ordering::SeqCst) {
                        let result =
                            connection.auto_connect_possible(&known_networks.lock().unwrap());
                        match result {
                            CouldConnect::Connect(setting) => {
                                if connection.connect(setting) {
                                    auto.store(true, Ordering::SeqCst);
                                } else {
                                    connection.disconnect();
                                }
                            }
                            CouldConnect::Disconnect => {
                                connection.disconnect();
                                iter = doscan();
                            }
                            CouldConnect::Rescan => {
                                iter = doscan();
                            }
                            _ => {}
                        }
                    }
                    if client_count.load(Ordering::SeqCst) > 0 && iter >= scan_iter {
                        iter = doscan();
                    } else {
                        iter += 1;
                    }
                }
                thread::sleep(time::Duration::from_secs(NETWORK_CHECK_INTERVAL));
            }
        }))
    }

    pub fn new(sender: mpsc::Sender<SignalMsg>) -> Self {
        let (connection_sender, receiver) = mpsc::channel::<ConnectionSetting>();
        let mut this = NetworkManager {
            connection: Connection::new(sender),
            auto_connect: Arc::new(AtomicBool::new(true)),
            client_count: Arc::new(AtomicU32::new(0)),
            running: Arc::new(AtomicBool::new(true)),
            monitor: None,
            scanner: None,
            known_networks: Arc::new(Mutex::new(config::read_networks())),
            sender: connection_sender,
        };
        let (scan_sender, scan_recv) = mpsc::channel::<()>();
        let connection = this.connection.clone();
        let running = this.running.clone();
        this.monitor = this.create_monitor(receiver, scan_sender);
        this.scanner = Some(thread::spawn(move || {
            while running.load(Ordering::SeqCst) {
                if let Ok(_) = scan_recv.recv() {
                    connection.scan();
                }
            }
        }));
        this
    }

    fn get_password(&self, essid: &str) -> Result<String, ()> {
        if let Some(ref known) = self.known_networks.lock().unwrap().get(essid) {
            if let Some(ref pass) = known.password {
                return Ok(pass.to_string());
            }
        }
        Err(())
    }
}

impl dbus_interface::ComGithubOkeriSnm for NetworkManager {
    type Err = MethodErr;
    fn connect(&self, setting: (u32, &str, bool)) -> Result<(), Self::Err> {
        if !self.connection.allow_reconnect() {
            return Err(MethodErr::failed(
                &"Reconnect of this connection is not alowed",
            ));
        }

        if self.connection.current_state().connecting() {
            self.connection.disconnect();
        }

        let (tp, essid, enc) = setting;
        let connection_setting = match tp {
            1 => ConnectionSetting::Ethernet,
            2 => {
                if enc {
                    if let Ok(pass) = self.get_password(essid) {
                        ConnectionSetting::Wifi {
                            essid: essid.to_string(),
                            password: pass.to_string(),
                        }
                    } else {
                        return Err(MethodErr::failed(
                            &"Connection is secured but no password specified",
                        ));
                    }
                } else {
                    ConnectionSetting::OpenWifi {
                        essid: essid.to_string(),
                    }
                }
            }
            _ => {
                return Err(MethodErr::invalid_arg(&setting));
            }
        };

        self.sender.send(connection_setting).unwrap();
        Ok(())
    }

    fn disconnect(&self) -> Result<(), Self::Err> {
        self.connection.disconnect();
        self.auto_connect.store(false, Ordering::SeqCst);
        Ok(())
    }

    fn get_state(&self) -> Result<(u32, String, bool, u32, String), Self::Err> {
        let state = self.connection.current_state();
        Ok(match state {
            ConnectionInfo::NotConnected => (0, "".to_string(), false, 0, "".to_string()),
            ConnectionInfo::Ethernet(ip) => (1, "Ethernet connection".to_string(), false, 100, ip),
            ConnectionInfo::Wifi(essid, quality, enc, ip) => (2, essid, enc, quality, ip),
            ConnectionInfo::ConnectingEth => (
                3,
                "Ethernet connection".to_string(),
                false,
                0,
                "".to_string(),
            ),
            ConnectionInfo::ConnectingWifi(essid) => (4, essid, false, 0, "".to_string()),
        })
    }

    fn get_networks(&self) -> Result<Vec<(u32, String, bool, u32)>, Self::Err> {
        let result: Vec<(u32, String, bool, u32)> = self
            .connection
            .get_networks()
            .iter()
            .map(|network| match network {
                NetworkInfo::Ethernet => (1, "Ethernet connection".to_string(), false, 100),
                NetworkInfo::Wifi(essid, quality, enc) => (2, essid.to_string(), *enc, *quality),
            })
            .collect();
        Ok(result)
    }

    fn monitor(&self, active: bool) -> Result<(), Self::Err> {
        if active {
            self.client_count.fetch_add(1, Ordering::SeqCst);
        } else {
            self.client_count.fetch_sub(1, Ordering::SeqCst);
        }
        Ok(())
    }

    fn get_props(&self, essid: &str) -> Result<(String, bool, bool), Self::Err> {
        if let Some(ref known) = self.known_networks.lock().unwrap().get(essid) {
            Ok(known.to_dbus_tuple())
        } else {
            Ok(KnownNetwork::default_dbus_tuple())
        }
    }

    fn set_props(
        &self,
        essid: &str,
        password: &str,
        auto: bool,
        encryption: bool,
    ) -> Result<(), Self::Err> {
        let props = KnownNetwork::new(auto, encryption, password);
        let upd_props = props.clone();
        if let Ok(mut known_networks) = self.known_networks.lock() {
            if encryption || auto {
                *known_networks.entry(essid.to_string()).or_insert(props) = upd_props;
            } else {
                known_networks.remove(essid);
            }

            if config::write_networks(&known_networks) {
                return Ok(());
            }
        }
        Err(MethodErr::failed(&"Cannot save props for network"))
    }
}

impl fmt::Debug for NetworkManager {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "NetworkManager()")
    }
}

impl Drop for NetworkManager {
    fn drop(&mut self) {
        if self.running.load(Ordering::SeqCst) {
            self.running.store(false, Ordering::SeqCst);
            self.monitor.take().unwrap().join().unwrap_or(());
            self.scanner.take().unwrap().join().unwrap_or(());
        }
    }
}

#[derive(Default)]
pub struct NetworkManagerFactory {}

impl DataType for NetworkManagerFactory {
    type ObjectPath = NetworkManager;
    type Property = ();
    type Interface = ();
    type Method = ();
    type Signal = ();
    type Tree = ();
}
