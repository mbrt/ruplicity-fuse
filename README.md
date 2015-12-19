# ruplicity-fuse
Mount duplicity backups with userspace filesystem

## Ubuntu / Debian installation

Under ubuntu or debian, you need `libfuse-dev` and `fuse` packages. Your user needs to be in the `fuse` group:

```
apt-get install libfuse-dev fuse
sudo addgroup <USERNAME> fuse
```

log out and log in again to apply permissions changes.
