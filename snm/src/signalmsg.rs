use connection_types::{ConnectionInfo, ConnectionStatus};
use dbus;
use dbus_interface;
use network_info::NetworkInfo;

pub enum SignalMsg {
    NetworkList(Vec<NetworkInfo>),
    ConnectStatusChanged(ConnectionStatus),
    StateChanged(ConnectionInfo),
}

impl SignalMsg {
    pub fn log(&self) {
        match self {
            SignalMsg::NetworkList(ref networks) => {
                println!("Scan complete. Found {} networks", networks.len());
            }

            SignalMsg::ConnectStatusChanged(ref status) => {
                println!(
                    "Connect status changed to {}",
                    match status {
                        ConnectionStatus::Initializing => "Bringing Interface Up",
                        ConnectionStatus::Connecting => "Connecting",
                        ConnectionStatus::Authenticating => "Authenticating",
                        ConnectionStatus::GettingIP => "Getting ip address",
                        ConnectionStatus::AuthFail => "Authorization failed",
                        ConnectionStatus::Aborted => "Connection canceled",
                        ConnectionStatus::ConnectFail => "Connection failed",
                    }
                );
            }
            SignalMsg::StateChanged(ref info) => match info {
                ConnectionInfo::ConnectingEth | ConnectionInfo::ConnectingWifi(_) => {
                    println!("Connecting")
                }

                ConnectionInfo::NotConnected => println!("Disconnected"),

                ConnectionInfo::Ethernet(ref ip) => {
                    println!("Connected to eth: {}", ip);
                }

                ConnectionInfo::Wifi(ref essid, _, _, ref ip) => {
                    println!("Connected to wifi: {}, ip: {}", essid, ip)
                }
            },
        }
    }

    pub fn emit(self, connection: &dbus::Connection, path: &dbus::Path) {
        use dbus::SignalArgs;
        let dbus_message = match self {
            SignalMsg::NetworkList(ref networks) => {
                let mut arg_data = Vec::new();
                for i in networks {
                    match i {
                        NetworkInfo::Ethernet => {
                            arg_data.push((
                                1,
                                "Ethernet connection".to_string(),
                                false,
                                100 as u32,
                            ));
                        }

                        NetworkInfo::Wifi(essid, quality, enc, _) => {
                            arg_data.push((2, essid.to_string(), *enc, *quality));
                        }
                    }
                }
                let args = dbus_interface::ComGithubOkeriSnmNetworkList { networks: arg_data };
                args.to_emit_message(path)
            }

            SignalMsg::ConnectStatusChanged(s) => {
                let args =
                    dbus_interface::ComGithubOkeriSnmConnectStatusChanged { status: s as u32 };
                args.to_emit_message(path)
            }

            SignalMsg::StateChanged(info) => {
                let arg_data = match info {
                    ConnectionInfo::NotConnected => (0, "".to_string(), false, 0, "".to_string()),
                    ConnectionInfo::Ethernet(ip) => {
                        (1, "Ethernet connection".to_string(), false, 100, ip)
                    }
                    ConnectionInfo::Wifi(essid, quality, enc, ip) => (2, essid, enc, quality, ip),
                    ConnectionInfo::ConnectingEth => (3, "".to_string(), false, 0, "".to_string()),
                    ConnectionInfo::ConnectingWifi(essid) => (4, essid, false, 0, "".to_string()),
                };
                let args = dbus_interface::ComGithubOkeriSnmStateChanged { state: arg_data };
                args.to_emit_message(path)
            }
        };
        connection.send(dbus_message).unwrap();
    }
}
