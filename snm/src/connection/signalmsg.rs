use super::types::{ConnectionInfo, ConnectionStatus, NetworkList};

pub enum SignalMsg {
    NetworkList(NetworkList),
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
}
