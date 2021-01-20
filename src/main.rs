use std::fmt::{Display, Error, Formatter};
use std::fs;
use std::io::{stdin, stdout, Write};
use std::ops::Sub;
use std::path::{Path, PathBuf};

use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::screen::*;
use termion::{color, style};

#[derive(Debug, Clone, Copy)]
struct Memory(u64);
impl Display for Memory {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        if self.0 >= 1024 {
            let x: u64 = self.0 / 1024;
            write!(f, "{:>7}M", x)
        } else {
            write!(f, "{:>7}K", self.0)
        }
    }
}

impl Sub for Memory {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self(self.0 - other.0)
    }
}

#[derive(Debug)]
struct Enclave {
    PID: u64,
    EID: u64,
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
            "{:>8} {:>8} {:>8} {:>8} {:>8} {:>8}  {}\n\r",
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
            let v: Vec<u64> = line
                .split(|x| x == &32 || x == &10 || x == &13)
                .take(6)
                .map(|x| x.iter().fold(0 as u64, |acc, x| acc * 10 + (x - 48) as u64))
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
    let mut sgx_encl_created: u64;
    let mut sgx_encl_released: u64;
    let mut sgx_pages_alloced: Option<Memory> = None;
    let mut sgx_pages_freed: Option<Memory> = None;
    let mut sgx_nr_total_epc_pages: Memory; //will not changed later
    let mut sgx_va_pages_cnt: Memory;
    let mut sgx_nr_free_pages: Memory;

    let mut screen = AlternateScreen::from(stdout().into_raw_mode().unwrap());
    write!(screen, "{}", termion::cursor::Hide).unwrap();

    write!(
        screen,
        "{}{}",
        termion::clear::All,
        termion::cursor::Goto(1, 1),
    )
    .unwrap();

    {
        let f = fs::read("/proc/sgx_stats").expect("/proc/sgx_stats not found");
        let mut iter = f
            .split(|x| x == &32 || x == &10 || x == &13) //split with space
            .take(7)
            .map(|x| {
                x.iter()
                    .fold(0 as u64, |acc, x| acc * 10 + ((x - 48) as u64))
            });

        sgx_encl_created = iter.next().unwrap();
        sgx_encl_released = iter.next().unwrap();
        let sgx_pages_alloced_new = Memory(iter.next().unwrap() << 2);
        let sgx_pages_freed_new = Memory(iter.next().unwrap() << 2);
        sgx_nr_total_epc_pages = Memory(iter.next().unwrap() << 2);
        sgx_va_pages_cnt = Memory(iter.next().unwrap() << 2);
        sgx_nr_free_pages = Memory(iter.next().unwrap() << 2);

        let eadd_speed = {
            match sgx_pages_alloced {
                None => Memory(0),
                Some(old) => sgx_pages_alloced_new - old,
            }
        };

        let eremove_speed = {
            match sgx_pages_freed {
                None => Memory(0),
                Some(old) => sgx_pages_freed_new - old,
            }
        };

        sgx_pages_alloced = Some(sgx_pages_alloced_new);
        sgx_pages_freed = Some(sgx_pages_freed_new);

        write!(
            screen,
            "{} enclaves running, Total {} enclaves created\n\r",
            sgx_encl_created - sgx_encl_released,
            sgx_encl_released
        )
        .unwrap();
        write!(
            screen,
            "eadd {:>8}/s, eremove {:>8}/s \n\r",
            eadd_speed, eremove_speed
        )
        .unwrap();
        write!(screen, "ewb {:>8}/s, eldu {:>8}/s \n\r", 0, 0).unwrap();
        write!(
            screen,
            "EPC mem: {:>8} total, {:>8} free, {:>8} used, {:>8} VA\n\r",
            sgx_nr_total_epc_pages,
            sgx_nr_free_pages,
            sgx_nr_total_epc_pages - sgx_nr_free_pages,
            sgx_va_pages_cnt,
        )
        .unwrap();
    }

    write!(
        screen,
        "\n\r{}{}{:>8} {:>8} {:>8} {:>8} {:>8} {:>8}  {}{}\n\r",
        color::Fg(color::Black),
        color::Bg(color::White),
        "EID",
        "PID",
        "SIZE",
        "EADDs",
        "RSS",
        "VA",
        "Command",
        style::Reset
    )
    .unwrap();

    let ev: Vec<Enclave> = read_sgx_enclave().expect("/proc/sgx_enclaves not found");
    for e in ev {
        write!(screen, "{}", e).unwrap()
    }
    screen.flush().unwrap();

    let stdin = stdin();
    for c in stdin.keys() {
        match c.unwrap() {
            Key::Char('q') => break,
            _ => {}
        }
        screen.flush().unwrap();
    }
    write!(screen, "{}", termion::cursor::Show).unwrap();
}
