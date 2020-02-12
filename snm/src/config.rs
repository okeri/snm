use super::connection::KnownNetworks;
use std::{fs, path::Path};
use toml;

const CONFIG_FILE: &str = "/etc/snm/networks";

pub fn read_networks() -> KnownNetworks {
    if let Ok(data) = fs::read_to_string(CONFIG_FILE) {
        toml::decode_str(&data).expect("cannot parse config")
    } else {
        KnownNetworks::new()
    }
}

pub fn write_networks(networks: &KnownNetworks) -> std::io::Result<()> {
    let file = Path::new(CONFIG_FILE);
    let dir = file.parent().ok_or(std::io::ErrorKind::NotFound)?;
    fs::create_dir_all(&dir)?;
    fs::write(file, &toml::encode_str(networks))?;
    Ok(())
}
