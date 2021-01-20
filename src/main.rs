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
        "{}{}",
        termion::clear::All,
        termion::cursor::Goto(1, 1),
    );
    {
        let f = fs::read("/proc/sgx_stats").expect("/proc/sgx_stats not found");
        let mut iter = f
            .split(|x| x == &32 || x == &10 || x == &13) //split with space
            .take(7)
            .map(|x| String::from_utf8(x.to_vec()).unwrap())
            .map(|x| x.parse::<u32>().unwrap());

        sgx_encl_created = iter.next().unwrap();
        sgx_encl_released = iter.next().unwrap();
        let sgx_pages_alloced_new = iter.next().unwrap();
        let sgx_pages_freed_new = iter.next().unwrap();
        sgx_nr_total_epc_pages = iter.next().unwrap();
        sgx_va_pages_cnt = iter.next().unwrap();
        sgx_nr_free_pages = iter.next().unwrap();

        let add_page_speed = {
            match sgx_pages_alloced {
                None => 0,
                Some(old) => sgx_pages_alloced_new - old,
            }
        };

        let free_page_speed = {
            match sgx_pages_freed {
                None => 0,
                Some(old) => sgx_pages_freed_new - old,
            }
        };

        sgx_pages_alloced = Some(sgx_pages_alloced_new);
        sgx_pages_freed = Some(sgx_pages_freed_new);

        write!(
            screen,
            "Enclaves running:    {:>8}, Total enclaves created: {:>8} \n\r",
            sgx_encl_created - sgx_encl_released,
            sgx_encl_released
        )
        .unwrap();
    }

    write!(
        screen,
        "{:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {}\n\r",
        "EID", "PID", "SIZE", "EADDs", "RSS", "VA", "Command"
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
