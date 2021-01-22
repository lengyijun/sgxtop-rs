/*!
 inspired by [sgxtop](https://github.com/fortanix/sgxtop)

 To run sgxtop-rs, you need to install customized [linux-sgx-driver](https://github.com/lengyijun/linux-sgx-driver/tree/top)

 This crate will read /proc/sgx_stats and /proc/sgx_enclaves
 and print it every second.

 It works similar to top and htop
*/

mod event;
use event::{Event, Events};

use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs;
use std::io::{stdout, Write};
use std::ops::Sub;
use std::path::PathBuf;

use termion::event::Key;
use termion::raw::{IntoRawMode, RawTerminal};
use termion::screen::*;
use termion::{color, style};

const SGX_ENCL_INITIALIZED: u64 = 1 << 0;
const SGX_ENCL_DEBUG: u64 = 1 << 1;
const SGX_ENCL_SECS_EVICTED: u64 = 1 << 2;
const SGX_ENCL_SUSPEND: u64 = 1 << 3;
const SGX_ENCL_DEAD: u64 = 1 << 4;

/// INIT:  SGX_ENCL_INITIALIZED
///
/// DEBUG: SGX_ENCL_DEBUG
///
/// EVICE: SGX_ENCL_SECS_EVICTED
///
/// SUS:   SGX_ENCL_SUSPEND
///
/// DEAD:  SGX_ENCL_DEAD
#[derive(Debug)]
struct EnclaveState(u64);
impl Display for EnclaveState {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        let mut v = vec![];
        if (self.0 & SGX_ENCL_INITIALIZED) > 0 {
            v.push("INIT");
        }
        if (self.0 & SGX_ENCL_DEBUG) > 0 {
            v.push("DEBUG");
        }
        if (self.0 & SGX_ENCL_SECS_EVICTED) > 0 {
            v.push("EVICT");
        }
        if (self.0 & SGX_ENCL_SUSPEND) > 0 {
            v.push("SUS");
        }
        if (self.0 & SGX_ENCL_DEAD) > 0 {
            v.push("DEAD");
        }
        let joined = v.join(",");
        write!(f, "{:>10}", joined)
    }
}

/// An abstract for memory for the sake of easy to print
///
/// The unit is KB
#[derive(Debug, Clone, Copy)]
struct Memory(u64);
impl Display for Memory {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
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

/// VIRT > EADDs > RSS
#[derive(Debug)]
struct Enclave {
    /// the coresponding process id
    pid: u64,

    /// global enclave unique id
    eid: u64,

    ///  the size of enclave in `vm_area_struct`, part of process virtual address area
    ///  
    ///  virt will never be reached
    virt: Memory,

    /// eadd counts
    eadds: Memory,

    /// actually occupied EPC pages
    rss: Memory,

    /// version array pages allocate for this enclave
    ///
    /// they may either in EPC or DRAM
    ///
    /// they are already counted in EADDS
    va: Memory,

    /// memory swaped to DRAM. It may be swaped back to EPC later
    swap: Memory,
    state: EnclaveState,
}

impl Display for Enclave {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        // "/proc/54142/cmdline"
        let mut path = PathBuf::from("/proc");
        path.push(self.pid.to_string());
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
            "{:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>10} {}\n\r",
            self.eid,
            self.pid,
            self.virt,
            self.eadds,
            self.rss,
            self.swap,
            self.va,
            self.state,
            command
        )
    }
}

struct GlobalStats {
    sgx_encl_created: u64,
    sgx_encl_released: u64,
    sgx_pages_alloced: Option<Memory>,
    sgx_pages_freed: Option<Memory>,

    /// Decide by your bios configuration, for example:32M, 64M and 128M
    ///
    /// always smaller than 32M, 64M and 128M.
    ///
    /// This variable is a constant.
    sgx_nr_total_epc_pages: Memory,
    sgx_va_pages_cnt: Memory,
    sgx_nr_free_pages: Memory,
    sgx_ewb_cnt: Option<Memory>,
    sgx_eldu_cnt: Option<Memory>,

    /// When enclave is released, all its pages will be free.
    ///
    /// Some pages are inside EPC, others in DRAM.
    ///
    /// This variable will track all pages in DRAM, freed when a enclave is released.
    sgx_freed_backing_pages: Memory,
    screen: termion::screen::AlternateScreen<RawTerminal<std::io::Stdout>>,
}

impl GlobalStats {
    fn new() -> Self {
        GlobalStats {
            sgx_encl_created: 0,
            sgx_encl_released: 0,
            sgx_pages_alloced: None,
            sgx_pages_freed: None,
            sgx_nr_total_epc_pages: Memory(0), //will not changed later
            sgx_va_pages_cnt: Memory(0),
            sgx_nr_free_pages: Memory(0),
            sgx_ewb_cnt: None,
            sgx_eldu_cnt: None,
            sgx_freed_backing_pages: Memory(0),
            screen: AlternateScreen::from(stdout().into_raw_mode().unwrap()),
        }
    }

    fn reset(&mut self) {
        write!(self.screen, "{}", termion::cursor::Show).unwrap();
        self.screen.flush().unwrap();
    }

    fn draw(&mut self) {
        write!(self.screen, "{}", termion::cursor::Hide).unwrap();
        write!(
            self.screen,
            "{}{}",
            termion::clear::All,
            termion::cursor::Goto(1, 1),
        )
        .unwrap();

        let f = fs::read("/proc/sgx_stats").expect("/proc/sgx_stats not found");
        let mut iter = f
            .split(|x| x == &32 || x == &10 || x == &13) //split with space
            .map(|x| {
                x.iter()
                    .fold(0 as u64, |acc, x| acc * 10 + ((x - 48) as u64))
            });

        self.sgx_encl_created = iter.next().unwrap();
        self.sgx_encl_released = iter.next().unwrap();
        let sgx_pages_alloced_new = Memory(iter.next().unwrap() << 2);
        let sgx_pages_freed_new = Memory(iter.next().unwrap() << 2);
        self.sgx_nr_total_epc_pages = Memory(iter.next().unwrap() << 2);
        self.sgx_va_pages_cnt = Memory(iter.next().unwrap() << 2);
        self.sgx_nr_free_pages = Memory(iter.next().unwrap() << 2);
        let sgx_ewb_cnt_new = Memory(iter.next().unwrap() << 2);
        let sgx_eldu_cnt_new = Memory(iter.next().unwrap() << 2);
        self.sgx_freed_backing_pages = Memory(iter.next().unwrap() << 2);

        let eadd_speed = {
            match self.sgx_pages_alloced {
                None => Memory(0),
                Some(old) => sgx_pages_alloced_new - old,
            }
        };

        let eremove_speed = {
            match self.sgx_pages_freed {
                None => Memory(0),
                Some(old) => sgx_pages_freed_new - old,
            }
        };

        let ewb_speed = {
            match self.sgx_ewb_cnt {
                None => Memory(0),
                Some(old) => sgx_ewb_cnt_new - old,
            }
        };

        let eldu_speed = {
            match self.sgx_eldu_cnt {
                None => Memory(0),
                Some(old) => sgx_eldu_cnt_new - old,
            }
        };

        self.sgx_pages_alloced = Some(sgx_pages_alloced_new);
        self.sgx_pages_freed = Some(sgx_pages_freed_new);
        self.sgx_ewb_cnt = Some(sgx_ewb_cnt_new);
        self.sgx_eldu_cnt = Some(sgx_eldu_cnt_new);

        write!(
            self.screen,
            "{} enclaves running, total {} enclaves created\n\r",
            self.sgx_encl_created - self.sgx_encl_released,
            self.sgx_encl_created
        )
        .unwrap();
        write!(
            self.screen,
            "eadd {:>8}/s, eremove {:>8}/s \n\r",
            eadd_speed, eremove_speed
        )
        .unwrap();
        write!(
            self.screen,
            "ewb {:>8}/s, eldu {:>8}/s \n\r",
            ewb_speed, eldu_speed
        )
        .unwrap();
        write!(
            self.screen,
            "EPC mem: {:>8} total, {:>8} free, {:>8} used, {:>8} VA\n\r",
            self.sgx_nr_total_epc_pages,
            self.sgx_nr_free_pages,
            self.sgx_nr_total_epc_pages - self.sgx_nr_free_pages,
            self.sgx_va_pages_cnt,
        )
        .unwrap();

        let swap_size = match self.sgx_ewb_cnt {
            None => Memory(0),
            Some(_) => {
                self.sgx_ewb_cnt.unwrap()
                    - self.sgx_eldu_cnt.unwrap()
                    - self.sgx_freed_backing_pages
            }
        };
        write!(self.screen, "Swap: {:>8}\n\r", swap_size).unwrap();

        write!(
            self.screen,
            "\n\r{}{}{:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>10} {}{}\n\r",
            color::Fg(color::Black),
            color::Bg(color::White),
            "EID",
            "PID",
            "VIRT",
            "EADDs",
            "RSS",
            "SWAP",
            "VA",
            "state",
            "Command",
            style::Reset
        )
        .unwrap();

        let ev: Vec<Enclave> = read_sgx_enclave().expect("/proc/sgx_enclaves not found");
        for e in ev {
            write!(self.screen, "{}", e).unwrap()
        }
        self.screen.flush().unwrap();
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
            let mut iter = line
                .split(|x| x == &32 || x == &10 || x == &13)
                .map(|x| x.iter().fold(0 as u64, |acc, x| acc * 10 + (x - 48) as u64));
            Enclave {
                pid: iter.next().unwrap(),
                eid: iter.next().unwrap(),
                virt: Memory(iter.next().unwrap() >> 10),
                eadds: Memory(iter.next().unwrap() << 2),
                rss: Memory(iter.next().unwrap() << 2),
                va: Memory(iter.next().unwrap() << 2),
                state: EnclaveState(iter.next().unwrap()),
                swap: Memory(iter.next().unwrap() << 2),
                //startTime
            }
        })
        .collect();
    Ok(x)
}

fn main() -> Result<(), Box<dyn Error>> {
    let events = Events::new();
    let mut g = GlobalStats::new();
    g.draw();

    loop {
        match events.next()? {
            Event::Input(input) => match input {
                Key::Char('q') => {
                    g.reset();
                    break;
                }
                Key::Ctrl('c') => {
                    g.reset();
                    break;
                }
                _ => {}
            },
            Event::Tick => {
                g.draw();
            }
        }
    }

    Ok(())
}
