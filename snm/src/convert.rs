use super::connection::{ConnectionSetting, KnownNetwork};
use rustbus::message_builder::MarshalledMessage;
use rustbus::params::{Base, Container, Param};
use std::convert::TryFrom;

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

pub fn convert<T: Convert>(msg: MarshalledMessage) -> Result<T, ()> {
    msg.unmarshall_all()
        .map_err(|_| ())
        .and_then(|m| T::from_params(&m.params))
}
