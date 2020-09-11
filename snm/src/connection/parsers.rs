use regex::{Captures, Regex};

pub enum Parsers {
    WpaState,
    NetworkQuality,
    NetworkEnc,
    NetworkEssid,
}

lazy_static! {
    static ref PARSERS: Vec<Regex> = vec![
        Regex::new(r".*wpa_state=(.*?)\n").unwrap(),
        Regex::new(r".*signal: ([^\.]+)\.").unwrap(),
        Regex::new(r".*capability: ([^\n]*)\n").unwrap(),
        Regex::new(r".*SSID: ([^\n]*)\n").unwrap()
    ];
}

pub fn parse(parser: Parsers, text: &str) -> Option<Captures> {
    PARSERS[parser as usize].captures(text)
}
