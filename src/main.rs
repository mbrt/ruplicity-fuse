extern crate fuse;
extern crate chan_signal;

mod fs;

use std::env;
use chan_signal::Signal;

use fs::RuplicityFs;

fn main() {
    let mountpoint = match env::args().nth(1) {
        Some(path) => path,
        None => {
            println!("Usage: {} <MOUNTPOINT>", env::args().nth(0).unwrap());
            return;
        }
    };

    let signal = chan_signal::notify(&[Signal::INT, Signal::TERM]);
    let _mount = unsafe { fuse::spawn_mount(RuplicityFs, &mountpoint, &[]) };

    // Blocks until this process is sent an INT or TERM signal.
    // Since the channel is never closed, we can unwrap the received value.
    signal.recv().unwrap();
}
