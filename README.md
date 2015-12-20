# ruplicity-fuse
Mount duplicity backups with userspace filesystem

## Installation

This application works only on Linux and OSX systems. You need to have the `fuse` kernel driver and `libfuse` installed in your system. Under ubuntu or debian, this is as simple as installing `libfuse-dev` and `fuse` packages. In addition, your user needs to be in the `fuse` group:

```
sudo apt-get install libfuse-dev fuse
sudo addgroup <USERNAME> fuse
```

log out and log in again to apply permissions changes.

You can then install `ruplicity-fuse` with Cargo:

```
cargo install --git https://github.com/mbrt/ruplicity-fuse.git
```
