#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate serde_derive;

mod config;
mod connection;
mod convert;
mod dbus;
mod marshal;

use connection::{
    Connection, ConnectionInfo, ConnectionSetting, CouldConnect, KnownNetwork, KnownNetworks,
    SignalMsg,
};

use rustbus::{
    connection::Error,
    message_builder::{DynamicHeader, MarshalledMessage},
    standard_messages, MessageType,
};

use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc, Mutex,
};
use std::{thread, time};

const NETWORK_CHECK_INTERVAL: u64 = 2;
const NETWORK_SCAN_INTERVAL: u64 = 14;

#[derive(Clone)]
struct ServiceData<SignalHandler: FnMut(SignalMsg)> {
    connection: Connection<SignalHandler>,
    known_networks: Arc<Mutex<KnownNetworks>>,
    auto: Arc<AtomicBool>,
    proxy_tracker: dbus::ProxyTracker,
    connect_sender: mpsc::Sender<ConnectionSetting>,
}

impl<SignalHandler: FnMut(SignalMsg)> ServiceData<SignalHandler> {
    fn new(signal_handler: SignalHandler, connect_sender: mpsc::Sender<ConnectionSetting>) -> Self {
        Self {
            connection: Connection::new(signal_handler),
            known_networks: Arc::new(Mutex::new(config::read_networks())),
            auto: Arc::new(AtomicBool::new(true)),
            proxy_tracker: dbus::ProxyTracker::new(),
            connect_sender,
        }
    }
}

fn main() -> Result<(), Error> {
    let (connect_sender, connect_recv) = mpsc::channel::<ConnectionSetting>();
    let mut adapter = dbus::Adapter::new(dbus::Bus::System, "com.github.okeri.snm")?;
    let mut emitter = adapter.new_emitter("/");
    let signal_handler = move |signal: SignalMsg| {
        signal.log();
        match signal {
            SignalMsg::StateChanged(state) => {
                emitter.emit("state_changed", &state).unwrap_or_default();
            }
            SignalMsg::ConnectStatusChanged(status) => {
                emitter
                    .emit("connect_status_changed", status as u32)
                    .unwrap_or_default();
            }
            SignalMsg::NetworkList(networks) => {
                emitter.emit("network_list", &networks).unwrap_or_default();
            }
        }
    };
    let mut service_data = ServiceData::new(signal_handler, connect_sender);

    let start_monitor = || {
        let mut service = service_data.clone();
        let mut scan_c = service.connection.clone();
        thread::spawn(move || {
            let scan_iter = NETWORK_SCAN_INTERVAL / NETWORK_CHECK_INTERVAL;
            let mut iter = 0;
            let mut doscan = || {
                scan_c.scan();
                0
            };

            let last_message = || {
                let mut msg = Err(());
                while let Ok(r) = connect_recv.try_recv() {
                    msg = Ok(r);
                }
                return msg;
            };
            service.connection.acquire();
            match service.connection.current_state() {
                ConnectionInfo::NotConnected | ConnectionInfo::Wifi(_, _, _, _) => {
                    service.connection.scan();
                }
                _ => {}
            }

            loop {
                if let Ok(setting) = last_message() {
                    if service.connection.connect(setting) {
                        service.auto.store(true, Ordering::SeqCst);
                        iter = 0;
                    }
                } else {
                    if service.auto.load(Ordering::SeqCst) {
                        let result = service
                            .connection
                            .auto_connect_possible(&service.known_networks.lock().unwrap());
                        match result {
                            CouldConnect::Connect(setting) => {
                                if service.connection.connect(setting) {
                                    service.auto.store(true, Ordering::SeqCst);
                                } else {
                                    service.connection.disconnect();
                                }
                            }
                            CouldConnect::Disconnect => {
                                service.connection.disconnect();
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
                    if service.proxy_tracker.active() > 0 && iter >= scan_iter {
                        iter = doscan();
                    } else {
                        iter += 1;
                    }
                    thread::sleep(time::Duration::from_secs(NETWORK_CHECK_INTERVAL));
                }
            }
        });
    };

    start_monitor();
    adapter.add_match("type='signal', path='/org/freedesktop/DBus', interface='org.freedesktop.DBus', member='NameOwnerChanged'")?;
    adapter.run(&mut service_data, dbus_handler)
}

fn make_failed(call: &DynamicHeader, text: &str) -> Option<MarshalledMessage> {
    let reply = call.make_error_response(
        "org.freedesktop.DBus.Error.Failed".to_owned(),
        Some(text.to_owned()),
    );
    Some(reply)
}

fn dbus_handler<SignalHandler: FnMut(SignalMsg)>(
    service: &mut ServiceData<SignalHandler>,
    msg: MarshalledMessage,
) -> Option<MarshalledMessage> {
    use convert::convert;
    match msg.typ {
        MessageType::Call => {
            let mut reply = msg.dynheader.make_response();
            if let Some(ref member) = msg.dynheader.member {
                match member.as_str() {
                    "hello" => {
                        service.proxy_tracker.start_track(&msg);
                    }
                    "connect" => {
                        if !service.connection.allow_reconnect() {
                            return make_failed(&msg.dynheader, "Reconnect is not alowed");
                        }
                        if service.connection.current_state().connecting() {
                            service.connection.disconnect();
                        }
                        let fallback = msg.dynheader.clone();
                        if let Ok(got_sets) = convert::<ConnectionSetting>(msg) {
                            let settings =
                                if let ConnectionSetting::Wifi { ref essid, .. } = got_sets {
                                    if let Some(known) =
                                        service.known_networks.lock().unwrap().get(essid)
                                    {
                                        known.to_setting(essid)
                                    } else {
                                        return make_failed(
                                            &fallback,
                                            "Connection is secured but no password specified",
                                        );
                                    }
                                } else {
                                    got_sets
                                };
                            service.connect_sender.send(settings).unwrap();
                        } else {
                            return Some(standard_messages::invalid_args(&fallback, Some("(usb)")));
                        }
                    }
                    "disconnect" => {
                        service.connection.disconnect();
                        service.auto.store(false, Ordering::SeqCst);
                    }
                    "get_state" => {
                        reply
                            .body
                            .push_param(&service.connection.current_state())
                            .unwrap();
                    }
                    "get_networks" => {
                        reply
                            .body
                            .push_param(&service.connection.get_networks())
                            .unwrap();
                    }
                    "get_props" => {
                        let fallback = msg.dynheader.clone();
                        if let Ok(ref essid) = convert::<String>(msg) {
                            if let Some(network) = service.known_networks.lock().unwrap().get(essid)
                            {
                                reply.body.push_param(network).unwrap();
                            } else {
                                reply.body.push_param(&KnownNetwork::default()).unwrap();
                            }
                        } else {
                            return Some(standard_messages::invalid_args(&fallback, Some("s")));
                        }
                    }
                    "set_props" => {
                        let fallback = msg.dynheader.clone();
                        if let Ok((essid, props)) = convert(msg) {
                            if let Ok(mut known) = service.known_networks.lock() {
                                let upd_props = props.clone();
                                if props.password.is_some() || props.auto {
                                    *known.entry(essid.to_string()).or_insert(props) = upd_props;
                                } else {
                                    known.remove(&essid);
                                }
                                if config::write_networks(&known).is_err() {
                                    return make_failed(&fallback, "Cannot write config");
                                }
                            }
                        } else {
                            return Some(standard_messages::invalid_args(
                                &fallback,
                                Some("ssibbb"),
                            ));
                        }
                    }
                    "Introspect" => {
                        let xml = include_str!("../xml/snm.xml").to_owned();
                        reply.body.push_param(xml).unwrap();
                    }
                    _ => {
                        return Some(standard_messages::unknown_method(&msg.dynheader));
                    }
                }
            }
            return Some(reply);
        }
        MessageType::Signal => {
            if msg
                .dynheader
                .interface
                .eq(&Some("org.freedesktop.DBus".to_owned()))
            {
                let fallback = msg.dynheader.clone();
                if let Ok(umsg) = msg.unmarshall_all() {
                    service.proxy_tracker.event(umsg);
                } else {
                    return Some(standard_messages::invalid_args(&fallback, None));
                }
            }
        }
        _ => {}
    }
    None
}
