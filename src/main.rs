extern crate fuse;
mod fs;

use std::env;
use fs::RuplicityFs;

fn main() {
    let mountpoint = match env::args().nth(1) {
        Some(path) => path,
        None => {
            println!("Usage: {} <MOUNTPOINT>", env::args().nth(0).unwrap());
            return;
        }
    };
    fuse::mount(RuplicityFs, &mountpoint, &[]);
}
