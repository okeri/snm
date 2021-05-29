use rustbus::connection::Error;
use rustbus::message_builder::MessageBuilder;
use rustbus::{Marshal, SendConn};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct Emitter {
    connection: Arc<Mutex<SendConn>>,
    iface: String,
    object: String,
}

impl Emitter {
    pub fn new(connection: Arc<Mutex<SendConn>>, iface: &str, object: &str) -> Self {
        Self {
            connection,
            iface: iface.to_owned(),
            object: object.to_owned(),
        }
    }

    pub fn emit<P: Marshal>(&mut self, member: &str, param: P) -> Result<u32, Error> {
        let mut sig = MessageBuilder::new()
            .signal(&self.iface, member, &self.object)
            .build();
        sig.body.push_param(param)?;
        self.connection
            .lock()
            .unwrap()
            .send_message_write_all(&mut sig)
    }
}
