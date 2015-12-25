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
        RuplicityFs { backup: backup }
    }

    fn getattr_root(&mut self, reply: ReplyAttr) {
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
    }

    fn readdir_root(&mut self, mut offset: u64, mut reply: ReplyDirectory) {
        if offset == 0 {
            reply.add(1, 0, FileType::Directory, &Path::new("."));
            reply.add(1, 1, FileType::Directory, &Path::new(".."));
            offset += 2;
            let snapshots = try_or_log!(self.backup.snapshots()).skip(offset as usize - 2);
            for snapshot in snapshots {
                let time = time::at(snapshot.time());
                let path = try_or_log!(time::strftime("%Y-%m-%d_%H:%M:%S", &time));
                if reply.add(1, offset, FileType::Directory, &Path::new(&path)) {
                    break;
                }
                offset += 1;
            }
        }
        reply.ok();
    }
}

impl Filesystem for RuplicityFs {
    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        if ino == 1 {
            self.getattr_root(reply);
        } else {
            reply.error(ENOSYS);
        }
    }

    fn readdir(&mut self, _req: &Request, ino: u64, _fh: u64, offset: u64, reply: ReplyDirectory) {
        if ino == 1 {
            self.readdir_root(offset, reply);
        } else {
            reply.error(ENOENT);
        }
    }
}
