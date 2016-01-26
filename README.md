# ruplicity-fuse
[![Build Status](https://travis-ci.org/mbrt/ruplicity-fuse.svg?branch=master)](https://travis-ci.org/mbrt/ruplicity-fuse)

Mount duplicity backups with userspace filesystem

## Installation

This application works only on Linux and OSX systems. You need to have the `fuse` kernel driver and `libfuse` installed. Under ubuntu or debian, this is as simple as installing `libfuse-dev` and `fuse` packages. In addition, your user needs to be in the `fuse` group:

```
sudo apt-get install libfuse-dev fuse
sudo addgroup <USERNAME> fuse
```

log out and log in again to apply permissions changes.

You can then install `ruplicity-fuse` with Cargo:

```
cargo install --git https://github.com/mbrt/ruplicity-fuse.git
```

## License

This crate is licensed through GPL-2.0. Why?
* The core functionality is already licensed under MIT, because it is exposed trough [ruplicity](https://github.com/mbrt/ruplicity) crate, so you can use it in whatever form you want (even closed source projects);
* This crate however provides a binary, and not a library, so I don't want anyone to fork it and close the sources (it could be possible with MIT license). Anyone is still free to use, contribute and modify it in whatever form they want. The only restriction is that they cannot change the license or integrate it in non-GPL projects.
