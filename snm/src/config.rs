use connection_types::KnownNetworks;
use std::path::Path;
use support;
use toml;

const CONFIG_FILE: &str = "/etc/snm/networks";

pub fn read_networks() -> KnownNetworks {
    if let Ok(data) = support::read_file(CONFIG_FILE) {
        toml::decode_str(&data).expect("cannot parse config")
    } else {
        KnownNetworks::new()
    }
}

pub fn write_networks(networks: &KnownNetworks) -> bool {
    let file = Path::new(CONFIG_FILE);
    support::write_file(&file, &toml::encode_str(networks))
}
