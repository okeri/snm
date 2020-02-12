use std::cmp::{Ord, Ordering};
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

impl Default for KnownNetwork {
    fn default() -> Self {
        KnownNetwork {
            auto: false,
            password: None,
            threshold: None,
        }
    }
}

pub type KnownNetworks = HashMap<String, KnownNetwork>;

#[derive(Eq, Clone)]
pub enum NetworkInfo {
    Ethernet,
    Wifi(String, u32, bool),
}

impl Ord for NetworkInfo {
    fn cmp(&self, other: &NetworkInfo) -> Ordering {
        if let NetworkInfo::Wifi(ref essid1, ref quality1, _) = *self {
            if let NetworkInfo::Wifi(ref essid2, ref quality2, _) = other {
                let t = quality2.cmp(quality1);
                if t == Ordering::Equal {
                    essid1.cmp(essid2)
                } else {
                    t
                }
            } else {
                Ordering::Greater
            }
        } else {
            Ordering::Less
        }
    }
}

impl PartialEq for NetworkInfo {
    fn eq(&self, other: &NetworkInfo) -> bool {
        if let NetworkInfo::Wifi(ref essid1, _, _) = *self {
            if let NetworkInfo::Wifi(ref essid2, _, _) = other {
                return essid1 == essid2;
            }
        }
        false
    }
}

impl PartialOrd for NetworkInfo {
    fn partial_cmp(&self, other: &NetworkInfo) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone)]
pub struct NetworkList(Vec<NetworkInfo>);

impl NetworkList {
    pub fn new() -> Self {
        NetworkList { 0: vec![] }
    }
}

impl std::ops::Deref for NetworkList {
    type Target = Vec<NetworkInfo>;
    fn deref(&self) -> &Vec<NetworkInfo> {
        &self.0
    }
}

impl std::ops::DerefMut for NetworkList {
    fn deref_mut(&mut self) -> &mut Vec<NetworkInfo> {
        &mut self.0
    }
}

// impl std::convert::From<Vec<NetworkInfo>> for NetworkList {
//     fn from(v: Vec<NetworkInfo>) -> Self {
// 	NetworkList{0: v}
//     }
// }

pub enum CouldConnect {
    Connect(ConnectionSetting),
    Disconnect,
    Rescan,
    DoNothing,
}
