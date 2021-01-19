use std::fmt::{Display, Error, Formatter};
use std::fs;
use std::path::{Path, PathBuf};

use ansi_term::Colour;

#[derive(Debug)]
struct Memory(u32);
impl Display for Memory {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        if self.0 >= 1024 {
            let x: u32 = self.0 / 1024;
            write!(f, "{:>7}M", x)
        } else {
            write!(f, "{:>7}K", self.0)
        }
    }
}

#[derive(Debug)]
struct Enclave {
    PID: u32,
    EID: u32,
    SIZE: Memory,
    EADDs: Memory,
    RSS: Memory,
    VA: Memory,
    //startTime
}

impl Display for Enclave {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        // "/proc/54142/cmdline"
        let mut path = PathBuf::from("/proc");
        path.push(self.PID.to_string());
        path.push("cmdline");

        let command: String = match fs::read(path.as_path()) {
            Err(_) => "".to_string(),
            Ok(v) => match String::from_utf8(v) {
                Ok(x) => x,
                Err(_) => "".to_string(),
            },
        };

        write!(
            f,
            "{:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {}\n",
            self.EID, self.PID, self.SIZE, self.EADDs, self.RSS, self.VA, command
        )
    }
}

fn read_sgx_enclave() -> Result<Vec<Enclave>, std::io::Error> {
    //CR: 10
    //LF: 13
    //Space: 32
    let x: Vec<Enclave> = fs::read("/proc/sgx_enclaves")?
        .split(|x| x == &10 || x == &13)
        .filter(|line| line.len() != 0)
        .map(|line| {
            let v: Vec<u32> = line
                .split(|x| x == &32)
                .map(|x| String::from_utf8(x.to_vec()).unwrap())
                .map(|x| x.parse::<u32>().unwrap())
                .collect();
            Enclave {
                PID: v[0],
                EID: v[1],
                SIZE: Memory(v[2]),
                EADDs: Memory(v[3]),
                RSS: Memory(v[4]),
                VA: Memory(v[5]),
                //startTime
            }
        })
        .collect();
    Ok(x)
}

fn main() {
    //text: black
    //backgroud: white
    let style = Colour::Black.on(Colour::White);
    print!(
        "{}",
        style.paint(&format!(
            "{:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {}\n",
            "EID", "PID", "SIZE", "EADDs", "RSS", "VA", "Command"
        ))
    );
    let ev:Vec<Enclave>=read_sgx_enclave().expect("/proc/enclaves not found");
    for e in ev{
        println!("{}",e);
    }
}
