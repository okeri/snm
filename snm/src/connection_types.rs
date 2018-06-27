use std::collections::HashMap;

pub enum ConnectionStatus {
    Initializing,
    Connecting,
    Authenticating,
    GettingIP,
    AuthFail,
    Aborted,
    ConnectFail,
}

#[derive(Clone)]
pub enum ConnectionInfo {
    NotConnected,
    Ethernet (String),
    Wifi (String, u32, bool, String),
    ConnectingEth,
    ConnectingWifi (String),
}

impl ConnectionInfo {
    pub fn wired(&self) -> bool {
        match self {
            ConnectionInfo::ConnectingEth |
            ConnectionInfo::Ethernet(_) => true,
            _ => false,
        }
    }

    pub fn active(&self) -> bool {
        match self {
            ConnectionInfo::NotConnected => false,
            _ => true,
        }
    }

    pub fn connecting(&self) -> bool {
        match self {
            ConnectionInfo::ConnectingEth |
            ConnectionInfo::ConnectingWifi(_) => true,
            _ => false,
        }
    }

}

pub enum ConnectionSetting {
    Ethernet,
    Wifi {essid: String, password: String},
    OpenWifi {essid: String},
}

#[derive (Clone, Serialize, Deserialize, PartialEq)]
pub struct KnownNetwork {
    pub auto: bool,
    pub password: Option<String>,
}

impl KnownNetwork {
    pub fn new(auto: bool, enc: bool, password: &str) -> Self {
        if enc {
            KnownNetwork{auto, password: Some(password.to_string())}
        } else {
            KnownNetwork{auto, password: None}
        }
    }

    pub fn to_dbus_tuple(&self) -> (String, bool, bool) {
        if let Some(ref p) = self.password {
            (p.to_string(), true, self.auto)
        } else {
            ("".to_string(), false, self.auto)
        }
    }

    pub fn default_dbus_tuple() -> (String, bool, bool) {
        ("".to_string(), false, false)
    }
}

pub type KnownNetworks = HashMap<String, KnownNetwork>;
