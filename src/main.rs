#![cfg_attr(feature = "nightly", allow(unstable_features))]
#![cfg_attr(feature = "lints", feature(plugin))]
#![cfg_attr(feature = "lints", plugin(clippy))]

extern crate chan_signal;
extern crate fuse;
extern crate libc;
#[macro_use]
extern crate log;
extern crate ruplicity;
extern crate time;

mod macros;
mod fs;
mod logger;

use std::env;
use std::io::{self, Write};
use std::path::Path;
use std::process;
use chan_signal::Signal;
use ruplicity::Backup;
use ruplicity::backend::local::LocalBackend;

use fs::RuplicityFs;

fn main() {
    let (mountp, backupp) = match (env::args().nth(1), env::args().nth(2)) {
        (Some(mount), Some(path)) => (mount, path),
        _ => {
            let _ = writeln!(&mut io::stderr(),
                             "Usage: {} <MOUNTPOINT> <BACKUP_PATH>",
                             env::args().nth(0).unwrap());
            process::exit(1);
        }
    };
    if let Err(e) = logger::init(log::LogLevelFilter::Debug) {
        println!("Logger initialization error {}", e);
        process::exit(1);
    };

    let backup = ordie(backup_from_path(backupp));
    let fs = ordie(RuplicityFs::new(backup));

    let signal = chan_signal::notify(&[Signal::INT, Signal::TERM]);
    let _mount = unsafe { fuse::spawn_mount(fs, &mountp, &[]) };

    // Blocks until this process is sent an INT or TERM signal.
    // Since the channel is never closed, we can unwrap the received value.
    signal.recv().unwrap();
}

fn backup_from_path<P: AsRef<Path>>(path: P) -> io::Result<Backup<LocalBackend>> {
    info!("Loading backup from path {:?}", path.as_ref());
    let backend = LocalBackend::new(path);
    Backup::new(backend)
}

fn ordie<T, E: ToString>(r: Result<T, E>) -> T {
    match r {
        Ok(r) => r,
        Err(e) => {
            fatal!("{}", e.to_string());
        }
    }
}
