extern crate chan_signal;
extern crate fuse;
extern crate libc;
#[macro_use]
extern crate log;
extern crate time;

mod fs;
mod logger;

use std::env;
use std::io::{self, Write};
use std::process;
use chan_signal::Signal;

use fs::RuplicityFs;

fn main() {
    let mountpoint = match env::args().nth(1) {
        Some(path) => path,
        None => {
            let _ = write!(&mut io::stderr(),
                           "Usage: {} <MOUNTPOINT>",
                           env::args().nth(0).unwrap());
            process::exit(1);
        }
    };
    logger::init().unwrap();

    let signal = chan_signal::notify(&[Signal::INT, Signal::TERM]);
    let _mount = unsafe { fuse::spawn_mount(RuplicityFs, &mountpoint, &[]) };

    // Blocks until this process is sent an INT or TERM signal.
    // Since the channel is never closed, we can unwrap the received value.
    signal.recv().unwrap();
}
