use super::connection::{
    ConnectionInfo, ConnectionSetting, ConnectionStatus, KnownNetwork, NetworkInfo, NetworkList,
};
use core::iter::FromIterator;
use rustbus::params::{Base, Container, Param};
use std::convert::{From, TryFrom};

impl<'a, 'e> From<ConnectionStatus> for Vec<Param<'a, 'e>> {
    fn from(s: ConnectionStatus) -> Self {
        vec![(s as u32).into()]
    }
}

impl<'a, 'e> From<ConnectionInfo> for Vec<Param<'a, 'e>> {
    fn from(s: ConnectionInfo) -> Self {
        vec![Container::Struct(match s {
            ConnectionInfo::NotConnected => vec![
                (0 as u32).into(),
                "".into(),
                false.into(),
                (0 as u32).into(),
                "".into(),
            ],
            ConnectionInfo::Ethernet(ip) => vec![
                (1 as u32).into(),
                "Ethernet connection".into(),
                false.into(),
                (100 as u32).into(),
                ip.into(),
            ],
            ConnectionInfo::Wifi(essid, quality, enc, ip) => vec![
                (2 as u32).into(),
                essid.into(),
                enc.into(),
                quality.into(),
                ip.into(),
            ],
            ConnectionInfo::ConnectingEth => vec![
                (3 as u32).into(),
                "".into(),
                false.into(),
                (0 as u32).into(),
                "".into(),
            ],
            ConnectionInfo::ConnectingWifi(essid) => vec![
                (4 as u32).into(),
                essid.into(),
                false.into(),
                (0 as u32).into(),
                "".into(),
            ],
        })
        .into()]
    }
}

impl<'a, 'e> From<&NetworkInfo> for Param<'a, 'e> {
    fn from(network: &NetworkInfo) -> Self {
        match network {
            NetworkInfo::Ethernet => Container::Struct(vec![
                (1 as u32).into(),
                "Ethernet connection".to_owned().into(),
                false.into(),
                (100 as u32).into(),
            ])
            .into(),
            NetworkInfo::Wifi(essid, quality, enc) => Container::Struct(vec![
                (2 as u32).into(),
                essid.to_owned().into(),
                (*enc).into(),
                (*quality).into(),
            ])
            .into(),
        }
    }
}

impl<'a, 'e> FromIterator<NetworkInfo> for Vec<Param<'a, 'e>> {
    fn from_iter<I: IntoIterator<Item = NetworkInfo>>(networks: I) -> Self {
        let mut params: Vec<Param> = Vec::new();
        for n in networks {
            params.push((&n).into());
        }
        params
    }
}

impl<'a, 'e> From<&KnownNetwork> for Vec<Param<'a, 'e>> {
    fn from(network: &KnownNetwork) -> Self {
        vec![
            network.password.clone().unwrap_or("".to_owned()).into(),
            network.threshold.unwrap_or(-65).into(),
            network.auto.into(),
            network.password.is_some().into(),
            network.threshold.is_some().into(),
        ]
    }
}

impl<'a, 'e> From<NetworkList> for Vec<Param<'a, 'e>> {
    fn from(nl: NetworkList) -> Self {
        let networks: Self = nl
            .iter()
            .map::<Param, _>(|network| network.into())
            .collect();
        vec![Container::make_array("(usbu)", &mut networks.into_iter())
            .unwrap()
            .into()]
    }
}

fn dbus_convert<'a, 'e, T: TryFrom<&'a Base<'a>>>(p: &'a Param<'a, 'e>) -> Result<T, ()> {
    if let Param::Base(ref base) = p {
        return T::try_from(base).map_err(|_| ());
    }
    Err(())
}

pub trait Convert: Sized {
    fn from_params(params: &Vec<Param>) -> Result<Self, ()>;
}

impl Convert for String {
    fn from_params(params: &Vec<Param>) -> Result<String, ()> {
        if params.len() == 1 {
            return dbus_convert(&params[0]);
        }
        Err(())
    }
}

impl Convert for ConnectionSetting {
    fn from_params(params: &Vec<Param>) -> Result<ConnectionSetting, ()> {
        if params.len() == 1 {
            if let Param::Container(c) = &params[0] {
                if let Container::Struct(p) = c {
                    if p.len() == 3 {
                        let tp = dbus_convert::<u32>(&p[0])?;
                        match tp {
                            1 => {
                                return Ok(ConnectionSetting::Ethernet);
                            }
                            2 => {
                                let essid = dbus_convert::<String>(&p[1])?;
                                let enc = dbus_convert::<bool>(&p[2])?;
                                return if enc {
                                    Ok(ConnectionSetting::Wifi {
                                        essid,
                                        password: "".to_owned(),
                                        threshold: None,
                                    })
                                } else {
                                    Ok(ConnectionSetting::OpenWifi {
                                        essid,
                                        threshold: None,
                                    })
                                };
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        Err(())
    }
}

impl Convert for (String, KnownNetwork) {
    fn from_params(params: &Vec<Param>) -> Result<(String, KnownNetwork), ()> {
        if params.len() == 6 {
            let essid = dbus_convert::<String>(&params[0])?;
            let password = dbus_convert::<String>(&params[1])?;
            let threshold = dbus_convert::<i32>(&params[2])?;
            let auto = dbus_convert::<bool>(&params[3])?;
            let enc = dbus_convert::<bool>(&params[4])?;
            let roaming = dbus_convert::<bool>(&params[5])?;
            return Ok((
                essid,
                KnownNetwork::new(auto, enc, roaming, &password, threshold),
            ));
        }
        Err(())
    }
}

pub fn convert<T: Convert>(p: &Vec<Param>) -> Result<T, ()> {
    T::from_params(p)
}
