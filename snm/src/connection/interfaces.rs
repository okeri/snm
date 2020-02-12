use super::support;
use super::types::ConnectionSetting;
use std::{fs, path::Path};

#[derive(Clone)]
pub struct Interfaces {
    eth: String,
    wlan: String,
}

impl Interfaces {
    pub fn new() -> Interfaces {
        let mut result = Interfaces {
            eth: "".to_string(),
            wlan: "".to_string(),
        };
        result.detect();
        result
    }

    pub fn from_setting(&self, setting: &ConnectionSetting) -> String {
        match *setting {
            ConnectionSetting::Ethernet => self.eth.clone(),
            _ => self.wlan.clone(),
        }
    }

    pub fn detect(&mut self) {
        for entry in fs::read_dir(&Path::new("/sys/class/net")).expect("no sysfs entry") {
            if let Ok(entry) = entry {
                let path = entry.file_name();
                if let Some(iface) = path.to_str() {
                    if let Some(sym) = iface.chars().next() {
                        match sym {
                            'e' => {
                                if self.eth.is_empty() {
                                    println!("Detected ethernet interface: {}", iface);
                                }
                                self.eth = iface.to_string();
                            }
                            'w' => {
                                if self.wlan.is_empty() {
                                    println!("Detected wifi interface: {}", iface);
                                }
                                self.wlan = iface.to_string();
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        self.wlan_up();
        self.eth_up();
    }

    pub fn get_wlan(&self) -> Option<&str> {
        if self.wlan.is_empty() {
            None
        } else {
            Some(&self.wlan)
        }
    }

    pub fn disconnect(&self) {
        let disconnect_iface = |ref iface| {
            support::run(&format!("dhcpcd -k {}", iface), false);
            support::run(&format!("ip addr flush dev {}", iface), false);
            support::run(
                &format!("wpa_cli -i {} -p /var/run/wpa terminate", iface),
                false,
            );
        };
        disconnect_iface(&self.eth);
        disconnect_iface(&self.wlan);
        support::run("dhcpcd -x", false);
    }

    pub fn wlan_scan(&self) -> String {
        support::run(&format!("iw dev {} scan", self.wlan), false)
    }

    pub fn up(iface: &str) {
        support::run(&format!("ip l set {} up", iface), false);
    }

    pub fn wlan_up(&self) {
        Self::up(&self.wlan);
    }

    pub fn eth_up(&self) {
        Self::up(&self.eth);
    }

    pub fn eth_plugged_in(&self) -> bool {
        if !self.eth.is_empty() {
            return Self::plugged_in(&self.eth);
        }
        false
    }

    pub fn wlan_plugged_in(&self) -> bool {
        if !self.wlan.is_empty() {
            return Self::plugged_in(&self.wlan);
        }
        false
    }

    pub fn plugged_in(iface: &str) -> bool {
        let filename = format!("/sys/class/net/{}/operstate", iface);
        if let Ok(value) = fs::read_to_string(&filename) {
            value == "up\n"
        } else {
            false
        }
    }
}
