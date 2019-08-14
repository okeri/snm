use std::{str, fs, mem, ptr, io::{self, Read}, path::Path, collections::VecDeque};
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

pub fn signal(signal: i32, action: fn(i32)) {
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

pub fn parse_essid(input: &str) -> Vec<u8> {
    let mut queue : VecDeque<_> = String::from(input).chars().collect();
    let mut result = vec![];
    let mut bytes2 = String::with_capacity(2);
    while let Some(c) = queue.pop_front() {
        if c != '\\' {
            result.push(c as u8);
            continue;
        }

        match queue.pop_front() {
            Some('t') => result.push(0x9),
            Some('\'') => result.push(0x27),
            Some('\"') => result.push(0x22),
            Some('\\') => result.push(0x5c),
            Some('x') => {
                bytes2.push(queue.pop_front().expect("Ill-formed string"));
                bytes2.push(queue.pop_front().expect("Ill-formed string"));
                result.push(u8::from_str_radix(&bytes2, 16).ok().unwrap());
                bytes2.clear();
            }
            _ => return result
        };
    }
    result
}
