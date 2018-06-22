use regex::{Regex, Captures};

pub enum Parsers {
    WpaState,
    Ip,
    Associated,
    NetworkQuality,
    NetworkEnc,
    NetworkEssid,
    NetworkChannel,
}

lazy_static! {
    static ref PARSERS: Vec<Regex> = vec![Regex::new(r".*wpa_state=(.*?)\n").unwrap(),
                                          Regex::new(r".*leased[^\d]?(.*)?[^ ]* for").unwrap(),
                                          Regex::new(r".*Access Point: ([\P{Cc}]*).*\n").unwrap(),
                                          Regex::new(r".*Quality=(\d+).(\d+)").unwrap(),
                                          Regex::new(r".*Encryption key:(on|off)\n").unwrap(),
                                          Regex::new(r####".*ESSID:"([^"]*)""####).unwrap(),
                                          Regex::new(r".*Channel:([\d]*)").unwrap()];
}

pub fn parse(parser: Parsers, text: &str) -> Option<Captures> {
    PARSERS[parser as usize].captures(text)
}
