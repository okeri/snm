use rustbus::{
    message_builder::MarshalledMessage,
    params::{message::Message, Base, Param},
};

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct ProxyTracker {
    proxies: Arc<Mutex<HashSet<String>>>,
}

fn to_string(param: &Param) -> Option<String> {
    if let Param::Base(base) = param {
        if let Base::String(value) = base {
            return Some(value.clone());
        }
    }
    None
}

impl ProxyTracker {
    pub fn new() -> Self {
        ProxyTracker {
            proxies: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub fn start_track(&mut self, msg: &MarshalledMessage) {
        if let Some(ref sender) = msg.dynheader.sender {
            let mut proxies = self.proxies.lock().unwrap();
            proxies.insert(sender.to_owned());
        }
    }

    pub fn event<'a, 'e>(&mut self, msg: Message<'a, 'e>) {
        if msg
            .dynheader
            .member
            .eq(&Some("NameOwnerChanged".to_owned()))
        {
            if msg.params.len() == 3 {
                if let Some(value) = to_string(&msg.params[2]) {
                    if value == "" {
                        if let Some(sender) = to_string(&msg.params[0]) {
                            let mut proxies = self.proxies.lock().unwrap();
                            proxies.remove(&sender);
                        }
                    }
                }
            }
        }
    }

    pub fn active(&self) -> usize {
        self.proxies.lock().unwrap().len()
    }
}
