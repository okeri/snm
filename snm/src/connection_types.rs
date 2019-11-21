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
    Ethernet(String),
    Wifi(String, u32, bool, String),
    ConnectingEth,
    ConnectingWifi(String),
}

impl ConnectionInfo {
    pub fn wired(&self) -> bool {
        match self {
            ConnectionInfo::ConnectingEth | ConnectionInfo::Ethernet(_) => true,
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
            ConnectionInfo::ConnectingEth | ConnectionInfo::ConnectingWifi(_) => true,
            _ => false,
        }
    }
}

pub enum ConnectionSetting {
    Ethernet,
    Wifi {
        essid: String,
        password: String,
        threshold: Option<i32>,
    },
    OpenWifi {
        essid: String,
        threshold: Option<i32>,
    },
}

impl ConnectionSetting {
    pub fn need_auth(&self) -> bool {
        match self {
            ConnectionSetting::Wifi { .. } => true,
            _ => false,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct KnownNetwork {
    pub auto: bool,
    pub password: Option<String>,
    #[serde(default = "KnownNetwork::default_threshold")]
    pub threshold: Option<i32>,
}

impl KnownNetwork {
    fn default_threshold() -> Option<i32> {
        None
    }

    fn make_threshold(roaming: bool, value: i32) -> Option<i32> {
        if roaming {
            Some(value)
        } else {
            None
        }
    }

    fn make_password(enc: bool, value: String) -> Option<String> {
        if enc {
            Some(value)
        } else {
            None
        }
    }

    pub fn new(auto: bool, enc: bool, roaming: bool, password: &str, threshold: i32) -> Self {
        KnownNetwork {
            auto,
            password: KnownNetwork::make_password(enc, password.to_string()),
            threshold: KnownNetwork::make_threshold(roaming, threshold),
        }
    }

    pub fn to_dbus_tuple(&self) -> (String, i32, bool, bool, bool) {
        (
            self.password.clone().unwrap_or("".to_string()),
            self.threshold.unwrap_or(-65),
            self.auto,
            self.password.is_some(),
            self.threshold.is_some(),
        )
    }

    pub fn default_dbus_tuple() -> (String, i32, bool, bool, bool) {
        ("".to_string(), -65, false, false, false)
    }

    pub fn to_setting(&self, essid: &str) -> ConnectionSetting {
        if let Some(ref pass) = self.password {
            ConnectionSetting::Wifi {
                essid: essid.to_string(),
                password: pass.to_string(),
                threshold: self.threshold,
            }
        } else {
            ConnectionSetting::OpenWifi {
                essid: essid.to_string(),
                threshold: self.threshold,
            }
        }
    }
}

pub type KnownNetworks = HashMap<String, KnownNetwork>;
