use std::{str, fs, mem, ptr, io::{self, Read}, path::Path};
use std::process::Command;

pub fn run(cmd: &str, err: bool) -> String {
    let output = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .output()
        .expect("failed to execute command");
    if err {
        str::from_utf8(&output.stderr).expect("process returned bad output")
            .to_string()
    } else {
        str::from_utf8(&output.stdout).expect("process returned bad output")
            .to_string()
    }
}

pub fn mktemp() -> String {
    let output = run("mktemp -u", false);
    output[0..output.len() - 1].to_string()
}


pub fn read_file(filename: &str) -> Result<String, io::Error> {
    let file = fs::File::open(&filename);
    if file.is_err() {
        return Err(file.err().unwrap());
    }
    let mut data = String::new();
    match file.unwrap().read_to_string(&mut data) {
        Ok(_) => {
            data.pop();
            Ok(data)
        },
        Err(e) => Err(e),
    }
}

pub fn read_file_value(filename: &str) -> Result<u32, io::Error> {
    match read_file(filename) {
        Ok(strval) => {
            Ok(strval.parse::<u32>().expect("should be a value"))
        },
        Err(e) => Err(e),
    }
}

pub fn signal(signal: i32, action: fn(i32)) {
    use libc;
    unsafe {
        let mut sigset = mem::uninitialized();
        if libc::sigfillset(&mut sigset) != -1 {
            let mut sigaction: libc::sigaction = mem::zeroed();
            sigaction.sa_mask = sigset;
            sigaction.sa_sigaction = action as usize;
            libc::sigaction(signal, &sigaction, ptr::null_mut());
        }
    }
}

pub fn write_file(filename: &Path, data: &str) -> bool {
    let dir = filename.parent().unwrap();
    fs::create_dir_all(&dir).unwrap();
    fs::write(filename, data).is_ok()
}
