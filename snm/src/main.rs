#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate serde_derive;

mod config;
mod connection;
mod convert;
mod dbus;

use connection::{Connection, ConnectionSetting, CouldConnect, KnownNetwork, SignalMsg};
use rustbus::{message::Message, standard_messages, MessageType};

use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc, Mutex,
};
use std::{thread, time};

const NETWORK_CHECK_INTERVAL: u64 = 2;
const NETWORK_SCAN_INTERVAL: u64 = 14;

fn main() -> Result<(), rustbus::client_conn::Error> {
    let (scan_sender, scan_recv) = mpsc::channel::<()>();
    let (connect_sender, connect_recv) = mpsc::channel::<ConnectionSetting>();
    let mut adapter = dbus::DBusLoop::connect_to_bus(dbus::Bus::System, "com.github.okeri.snm")?;
    let mut emitter = adapter.new_emitter("/");
    let mut tracker = dbus::ProxyTracker::new();
    let auto_connect = Arc::new(AtomicBool::new(true));
    let known_networks = Arc::new(Mutex::new(config::read_networks()));

    let mut connection = Connection::new(move |signal| {
        signal.log();
        match signal {
            SignalMsg::StateChanged(state) => {
                emitter.emit("state_changed", state.into()).unwrap();
            }
            SignalMsg::ConnectStatusChanged(status) => {
                emitter
                    .emit("connect_status_changed", status.into())
                    .unwrap();
            }
            SignalMsg::NetworkList(networks) => {
                emitter.emit("network_list", networks.into()).unwrap();
            }
        }
    });

    let start_scanner = || {
        let mut c = connection.clone();
        thread::spawn(move || loop {
            if let Ok(_) = scan_recv.recv() {
                c.scan();
            }
        });
    };

    let start_monitor = || {
        let mut c = connection.clone();
        let auto = auto_connect.clone();
        let known = known_networks.clone();
        let proxy_count = tracker.active_proxies_counter();
        thread::spawn(move || {
            let scan_iter = NETWORK_SCAN_INTERVAL / NETWORK_CHECK_INTERVAL;
            let mut iter = 0;
            let doscan = || {
                scan_sender.send(()).unwrap();
                0
            };

            let last_message = || {
                let mut msg = Err(());
                while let Ok(r) = connect_recv.try_recv() {
                    msg = Ok(r);
                }
                return msg;
            };
            c.acquire();
            c.scan();

            loop {
                if let Ok(setting) = last_message() {
                    if c.connect(setting) {
                        auto.store(true, Ordering::SeqCst);
                        iter = 0;
                    }
                } else {
                    if auto.load(Ordering::SeqCst) {
                        let result = c.auto_connect_possible(&known.lock().unwrap());
                        match result {
                            CouldConnect::Connect(setting) => {
                                if c.connect(setting) {
                                    auto.store(true, Ordering::SeqCst);
                                } else {
                                    c.disconnect();
                                }
                            }
                            CouldConnect::Disconnect => {
                                c.disconnect();
                                iter = doscan();
                            }
                            CouldConnect::Rescan => {
                                if iter >= scan_iter {
                                    iter = doscan();
                                }
                            }
                            _ => {}
                        }
                    }
                    if proxy_count.load(Ordering::SeqCst) > 0 && iter >= scan_iter {
                        iter = doscan();
                    } else {
                        iter += 1;
                    }
                    thread::sleep(time::Duration::from_secs(NETWORK_CHECK_INTERVAL));
                }
            }
        });
    };

    start_scanner();
    start_monitor();
    adapter.add_match("type='signal', path='/org/freedesktop/DBus', interface='org.freedesktop.DBus', member='NameOwnerChanged'")?;
    adapter.run(move |msg| {
        match msg.typ {
            MessageType::Call => {
                use convert::convert;
                let mut reply = msg.make_response();
                if let Some(ref member) = msg.member {
                    match member.as_ref() {
                        "hello" => {
                            tracker.start_track(&msg);
                        }
                        "connect" => {
                            if !connection.allow_reconnect() {
                                return make_failed(msg, "Reconnect is not alowed");
                            }
                            if connection.current_state().connecting() {
                                connection.disconnect();
                            }
                            if let Ok(got_sets) = convert::<ConnectionSetting>(&msg.params) {
                                let settings = if let ConnectionSetting::Wifi {
                                    ref essid, ..
                                } = got_sets
                                {
                                    if let Some(known) = known_networks.lock().unwrap().get(essid) {
                                        known.to_setting(essid)
                                    } else {
                                        return make_failed(
                                            msg,
                                            "Connection is secured but no password specified",
                                        );
                                    }
                                } else {
                                    got_sets
                                };
                                connect_sender.send(settings).unwrap();
                            } else {
                                return Some(standard_messages::invalid_args(&msg, Some("(usb)")));
                            }
                        }
                        "disconnect" => {
                            connection.disconnect();
                            auto_connect.store(false, Ordering::SeqCst);
                        }
                        "get_state" => {
                            reply.push_params(connection.current_state().into());
                        }
                        "get_networks" => {
                            reply.push_params(connection.get_networks().into());
                        }
                        "get_props" => {
                            if let Ok(ref essid) = convert::<String>(&msg.params) {
                                if let Some(network) = known_networks.lock().unwrap().get(essid) {
                                    reply.push_params(network.into());
                                } else {
                                    let network = KnownNetwork::default();
                                    reply.push_params((&network).into());
                                }
                            } else {
                                return Some(standard_messages::invalid_args(&msg, Some("s")));
                            }
                        }
                        "set_props" => {
                            if let Ok((essid, props)) = convert(&msg.params) {
                                if let Ok(mut known) = known_networks.lock() {
                                    let upd_props = props.clone();
                                    if props.password.is_some() || props.auto {
                                        *known.entry(essid.to_string()).or_insert(props) =
                                            upd_props;
                                    } else {
                                        known.remove(&essid);
                                    }
                                    if config::write_networks(&known).is_err() {
                                        return make_failed(msg, "Cannot write config");
                                    }
                                }
                            } else {
                                return Some(standard_messages::invalid_args(&msg, Some("ssibbb")));
                            }
                        }
                        "Introspect" => {
                            let xml = include_str!("../xml/snm.xml").to_owned();
                            reply.push_params(vec![xml]);
                        }
                        _ => {
                            return Some(standard_messages::unknown_method(&msg));
                        }
                    }
                }
                return Some(reply);
            }
            MessageType::Signal => {
                if msg.interface.eq(&Some("org.freedesktop.DBus".to_owned())) {
                    tracker.event(&msg);
                }
            }
            _ => {}
        }
        None
    })
}

fn make_failed<'a, 'e>(msg: Message<'a, 'e>, text: &str) -> Option<Message<'a, 'e>> {
    let reply = msg.make_error_response(
        "org.freedesktop.DBus.Error.Failed".to_owned(),
        Some(text.to_owned()),
    );
    Some(reply)
}
