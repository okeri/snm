use rustbus::{message::Base, message::Param, Message};

use std::collections::HashSet;
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};

pub struct ProxyTracker {
    proxies: HashSet<String>,
    count: Arc<AtomicU32>,
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
            proxies: HashSet::new(),
            count: Arc::new(AtomicU32::new(0)),
        }
    }

    pub fn start_track(&mut self, msg: &Message) {
        if let Some(ref sender) = msg.sender {
            self.proxies.insert(sender.to_owned());
            self.count.fetch_add(1, Ordering::SeqCst);
        }
    }

    pub fn event(&mut self, msg: &Message) {
        if msg.member.eq(&Some("NameOwnerChanged".to_owned())) {
            if msg.params.len() == 3 {
                if let Some(value) = to_string(&msg.params[2]) {
                    if value == "" {
                        if let Some(sender) = to_string(&msg.params[0]) {
                            if self.proxies.remove(&sender) {
                                self.count.fetch_sub(1, Ordering::SeqCst);
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn active_proxies_counter(&self) -> Arc<AtomicU32> {
        self.count.clone()
    }
}
