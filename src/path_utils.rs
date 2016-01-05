use std::io;
use std::path::Path;
#[cfg(unix)]
use std::os::unix::prelude::*;
#[cfg(windows)]
use std::os::windows::prelude::*;

#[cfg(windows)]
pub fn path2bytes(p: &Path) -> io::Result<&[u8]> {
    p.as_os_str()
     .to_str()
     .map(|s| s.as_bytes())
     .ok_or_else(|| other("path was not valid unicode"))
}

#[cfg(unix)]
pub fn path2bytes(p: &Path) -> io::Result<&[u8]> {
    Ok(p.as_os_str().as_bytes())
}
