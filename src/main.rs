use std::fmt::{Display, Error, Formatter};
use std::fs;
use std::io::{stdin, stdout, Write};
use std::path::{Path, PathBuf};

use ansi_term::Colour;

use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::screen::*;

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
            "{:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {}\n\r",
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
                SIZE: Memory(v[2] >> 10),
                EADDs: Memory(v[3] << 2),
                RSS: Memory(v[4] << 2),
                VA: Memory(v[5] << 2),
                //startTime
            }
        })
        .collect();
    Ok(x)
}

fn main() {
    let mut sgx_encl_created: u32;
    let mut sgx_encl_released: u32;
    let mut sgx_pages_alloced: Option<u32> = None;
    let mut sgx_pages_freed: Option<u32> = None;
    let mut sgx_nr_total_epc_pages: u32; //will not changed later
    let mut sgx_va_pages_cnt: u32;
    let mut sgx_nr_free_pages: u32;

    let stdin = stdin();
    let mut screen = AlternateScreen::from(stdout().into_raw_mode().unwrap());
    write!(screen, "{}", termion::cursor::Hide).unwrap();

    //text: black
    //backgroud: white
    let style = Colour::Black.on(Colour::White);
    write!(
        screen,
        "{}{}{:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {}\n\r",
        termion::clear::All,
        termion::cursor::Goto(1, 1),
        "EID",
        "PID",
        "SIZE",
        "EADDs",
        "RSS",
        "VA",
        "Command"
    )
    .unwrap();

    let ev: Vec<Enclave> = read_sgx_enclave().expect("/proc/enclaves not found");
    for e in ev {
        println!("{}", e);
    }
    screen.flush().unwrap();

    for c in stdin.keys() {
        match c.unwrap() {
            Key::Char('q') => break,
            _ => {}
        }
        screen.flush().unwrap();
    }
    write!(screen, "{}", termion::cursor::Show).unwrap();
}
