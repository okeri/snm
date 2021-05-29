use std::sync::{Arc, Mutex};

use rustbus::connection::{get_session_bus_path, get_system_bus_path, Error, Timeout};
use rustbus::message_builder::MarshalledMessage;
use rustbus::{standard_messages, DuplexConn, RecvConn, SendConn};

use super::emitter;

#[allow(dead_code)]
pub enum Bus {
    Session,
    System,
}

pub struct Adapter {
    send: Arc<Mutex<SendConn>>,
    recv: RecvConn,
    iface: String,
}

impl Adapter {
    pub fn new(bus: Bus, iface: &str) -> Result<Self, Error> {
        let path = match bus {
            Bus::Session => get_session_bus_path()?,
            Bus::System => get_system_bus_path()?,
        };
        let mut conn = DuplexConn::connect_to_bus(path, false)?;
        conn.send_hello(Timeout::Infinite)?;
        conn.send
            .send_message(&mut standard_messages::request_name(
                iface.into(),
                standard_messages::DBUS_NAME_FLAG_REPLACE_EXISTING,
            ))?
            .write_all()
            .map_err(|(_, e)| e)?;

        Ok(Self {
            send: Arc::new(Mutex::new(conn.send)),
            recv: conn.recv,
            iface: iface.into(),
        })
    }

    pub fn new_emitter(&self, object: &str) -> emitter::Emitter {
        emitter::Emitter::new(self.send.clone(), &self.iface, object)
    }

    pub fn run<
        Service,
        Handler: FnMut(&mut Service, MarshalledMessage) -> Option<MarshalledMessage>,
    >(
        &mut self,
        service: &mut Service,
        mut handler: Handler,
    ) -> Result<(), Error> {
        loop {
            let msg = self.recv.get_next_message(Timeout::Infinite)?;
            if let Some(response) = handler(service, msg) {
                self.send
                    .lock()
                    .unwrap()
                    .send_message_write_all(&response)?;
            }
        }
    }

    pub fn add_match(&mut self, m: &str) -> Result<u32, Error> {
        self.send
            .lock()
            .unwrap()
            .send_message_write_all(&standard_messages::add_match(m.into()))
    }
}
