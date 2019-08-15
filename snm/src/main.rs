extern crate dbus;
extern crate libc;
extern crate regex;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate toml;
#[macro_use]
extern crate lazy_static;

mod config;
mod connection;
mod connection_types;
mod dbus_interface;
mod network_info;
mod parsers;
mod signalmsg;
mod snm;
mod support;

use signalmsg::SignalMsg;
use snm::{NetworkManager, NetworkManagerFactory};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};

lazy_static! {
    static ref STOP: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
}

fn sighandler(sig: i32) {
    println!("signal {} catched. Shutting down", sig);
    STOP.store(true, Ordering::SeqCst);
}

fn main() {
    support::signal(libc::SIGTERM, sighandler);
    support::signal(libc::SIGINT, sighandler);

    let path_name = "/";
    let connection = dbus::Connection::get_private(dbus::BusType::System).unwrap();
    connection
        .register_name(
            "com.github.okeri.snm",
            dbus::NameFlag::ReplaceExisting as u32,
        )
        .unwrap();

    let factory = dbus::tree::Factory::new_fn::<NetworkManagerFactory>();
    let iface =
        dbus_interface::com_github_okeri_snm_server(&factory, (), |minfo| minfo.path.get_data())
            // Although we have generated dbus interface, we have to add signals manually, lol)
            .add_s(Arc::new(factory.signal("network_list", ()).arg(
                dbus::tree::Argument::new(
                    Some("networks".to_string()),
                    dbus::Signature::new("a(usbu)").unwrap(),
                ),
            )))
            .add_s(Arc::new(factory.signal("state_changed", ()).arg(
                dbus::tree::Argument::new(
                    Some("state".to_string()),
                    dbus::Signature::new("(usbus)").unwrap(),
                ),
            )))
            .add_s(Arc::new(factory.signal("connect_status_changed", ()).arg(
                dbus::tree::Argument::new(
                    Some("networks".to_string()),
                    dbus::Signature::new("u").unwrap(),
                ),
            )));

    let (sender, receiver) = mpsc::channel::<SignalMsg>();
    let tree = factory.tree(()).add(
        factory
            .object_path(path_name, NetworkManager::new(sender))
            .introspectable()
            .add(iface),
    );
    tree.set_registered(&connection, true).unwrap();
    let path = dbus::Path::new(path_name).unwrap();

    connection.add_handler(tree);
    while !STOP.load(Ordering::SeqCst) {
        connection.incoming(1000).next();
        if let Ok(msg) = receiver.try_recv() {
            msg.log();
            msg.emit(&connection, &path);
        }
    }
}
