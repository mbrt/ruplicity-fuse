use fuse::{FileAttr, FileType, Filesystem, ReplyAttr, ReplyDirectory, Request};
use libc::{ENOENT, ENOSYS};
use time;
use ruplicity::Backup;
use ruplicity::backend::local::LocalBackend;

use std::path::Path;


pub struct RuplicityFs {
    backup: Backup<LocalBackend>,
}

impl RuplicityFs {
    pub fn new(backup: Backup<LocalBackend>) -> Self {
        RuplicityFs {
            backup: backup,
        }
    }
}

impl Filesystem for RuplicityFs {
    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        if ino == 1 {
            let ts = time::get_time();
            let attr = FileAttr {
                ino: 1,
                size: 0,
                blocks: 0,
                atime: ts,
                mtime: ts,
                ctime: ts,
                crtime: ts,
                kind: FileType::Directory,
                perm: 0o755,
                nlink: 0,
                uid: 0,
                gid: 0,
                rdev: 0,
                flags: 0,
            };
            reply.attr(&ts, &attr);
        } else {
            reply.error(ENOSYS);
        }
    }

    fn readdir(&mut self,
               _req: &Request,
               ino: u64,
               _fh: u64,
               offset: u64,
               mut reply: ReplyDirectory) {
        if ino == 1 {
            if offset == 0 {
                reply.add(1, 0, FileType::Directory, &Path::new("."));
                reply.add(1, 1, FileType::Directory, &Path::new(".."));
            }
            reply.ok();
        } else {
            reply.error(ENOENT);
        }
    }
}
