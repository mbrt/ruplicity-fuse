use fuse::{FileAttr, FileType, Filesystem, ReplyAttr, ReplyDirectory, ReplyEntry, Request};
use libc::{ENOENT, ENOSYS};
use time::{self, Timespec};
use ruplicity::{Backend, Backup, Snapshot};

use std::collections::HashMap;
use std::io;
use std::path::Path;


pub struct RuplicityFs<B> {
    backup: Backup<B>,
    snapshots_paths: HashMap<String, usize>,
}

impl<B: Backend> RuplicityFs<B> {
    pub fn new(backup: Backup<B>) -> io::Result<Self> {
        let mut spaths = HashMap::new();
        for (count, snapshot) in try!(backup.snapshots()).enumerate() {
            let path = time_to_path(snapshot.time());
            spaths.insert(path, count);
        }

        Ok(RuplicityFs {
            backup: backup,
            snapshots_paths: spaths,
        })
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
            perm: 0o555,
            nlink: 0,
            uid: 0,
            gid: 0,
            rdev: 0,
            flags: 0,
        };
        reply.attr(&ts, &attr);
    }

    fn getattr_snapshot(&mut self, ino: u64, reply: ReplyAttr) {
        match try_or_log!(self.backup.snapshots()).nth(ino as usize - 2) {
            Some(snapshot) => {
                let ts = snapshot.time();
                let attr = self.attr_snapshot(&snapshot, ino);
                reply.attr(&ts, &attr);
            }
            None => {
                error!("Cannot find snapshot for ino {}", ino);
                reply.error(ENOSYS);
            }
        }
    }

    fn readdir_root(&mut self, mut offset: u64, mut reply: ReplyDirectory) {
        // offset is the last returned offset
        if offset == 0 {
            // assume first two replies does fit in the buffer
            reply.add(1, 0, FileType::Directory, &Path::new("."));
            reply.add(1, 1, FileType::Directory, &Path::new(".."));
            offset += 1;
        }

        debug!("Skip first {} snapshots", offset - 1);
        let snapshots = try_or_log!(self.backup.snapshots()).skip(offset as usize - 1);
        for snapshot in snapshots {
            offset += 1;
            debug!("Add snapshot for offset {}", offset);
            let path = time_to_path(snapshot.time());
            if reply.add(offset, offset, FileType::Directory, &Path::new(&path)) {
                // the buffer is full, need to return
                break;
            }
        }
        reply.ok();
    }

    fn lookup_snapshot(&mut self, name: &Path, reply: ReplyEntry) {
        let sid = match self.snapshots_paths.get(name.to_str().unwrap()) {
            Some(id) => *id,
            None => {
                reply.error(ENOENT);
                return;
            }
        };
        match try_or_log!(self.backup.snapshots()).nth(sid) {
            Some(snapshot) => {
                let ts = snapshot.time();
                let attr = self.attr_snapshot(&snapshot, sid as u64 + 2);
                reply.entry(&ts, &attr, 0);
            }
            None => {
                reply.error(ENOENT);
                return;
            }
        };
    }

    fn is_snapshot(&self, ino: u64) -> bool {
        ino >= 2 && ino < self.snapshots_paths.len() as u64 + 2
    }

    fn attr_snapshot(&self, snapshot: &Snapshot, ino: u64) -> FileAttr {
        let ts = snapshot.time();
        FileAttr {
            ino: ino,
            size: 0,
            blocks: 0,
            atime: ts,
            mtime: ts,
            ctime: ts,
            crtime: ts,
            kind: FileType::Directory,
            perm: 0o555,
            nlink: 0,
            uid: 0,
            gid: 0,
            rdev: 0,
            flags: 0,
        }
    }
}

impl<B: Backend> Filesystem for RuplicityFs<B> {
    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        if ino == 1 {
            self.getattr_root(reply);
        } else if self.is_snapshot(ino) {
            self.getattr_snapshot(ino, reply);
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

    fn lookup(&mut self, _req: &Request, parent: u64, name: &Path, reply: ReplyEntry) {
        if parent == 1 {
            self.lookup_snapshot(name, reply);
        } else {
            reply.error(ENOENT);
        }
    }
}


fn time_to_path(time: Timespec) -> String {
    let time = time::at(time);
    time::strftime("%Y-%m-%d_%H-%M-%S", &time).unwrap()
}
