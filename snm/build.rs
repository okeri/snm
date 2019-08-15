use std::fs::File;
use std::path::Path;
use std::process::Command;

fn main() {
    let interface_path = Path::new("xml/snm.xml");
    let gen_source_path = Path::new("src/dbus_interface.rs");

    let output = File::create(gen_source_path).expect(&format!(
        "Can't create «{}»",
        gen_source_path.to_str().unwrap()
    ));

    let input = File::open(interface_path).expect(&format!(
        "Can't open «{}»",
        interface_path.to_str().unwrap()
    ));

    let status = Command::new("which")
        .arg("dbus-codegen-rust")
        .status()
        .expect("Failed to execute «which dbus-codegen-rust»");

    if status.success() == false {
        let status = Command::new("rustup")
            .arg("run")
            .arg("stable-x86_64-unknown-linux-gnu")
            .arg("cargo")
            .arg("install")
            .arg("dbus-codegen")
            .status()
            .expect("Failed to execute «which dbus-codegen-rust»");
        if status.success() == false {
            panic!("Failed to install «dbus-codegen-rust»");
        }
    }

    Command::new("dbus-codegen-rust")
        .stdin(input)
        .stdout(output)
        .output()
        .expect("Faled to generate sources");
}
