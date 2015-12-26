use fuse::{FileAttr, FileType, Filesystem, ReplyAttr, ReplyEntry, ReplyDirectory, Request};
use libc::{ENOENT, ENOSYS};
use time;
use ruplicity::{Backend, Backup};

use std::collections::HashMap;
use std::path::Path;


pub struct RuplicityFs<B> {
    backup: Backup<B>,
    snapshots_ino: (u64, u64),
    snapshots_paths: HashMap<String, usize>,
}

impl<B: Backend> RuplicityFs<B> {
    pub fn new(backup: Backup<B>) -> Self {
        let mut spaths = HashMap::new();
        for (count, snapshot) in backup.snapshots().unwrap().enumerate() {
            let time = time::at(snapshot.time());
            let path = time::strftime("%Y-%m-%d_%H:%M:%S", &time).unwrap();
            spaths.insert(path, count);
        }
        let num_snapshots = spaths.len();

        RuplicityFs {
            backup: backup,
            snapshots_ino: (2, num_snapshots as u64 + 2),
            snapshots_paths: spaths,
        }
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

    fn getattr_snapshot(&mut self, ino: u64, reply: ReplyAttr) {
        match try_or_log!(self.backup.snapshots()).nth(ino as usize - 2) {
            Some(snapshot) => {
                let ts = snapshot.time();
                let attr = FileAttr {
                    ino: ino,
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
            None => {
                error!("Cannot find snapshot for ino {}", ino);
                reply.error(ENOSYS);
            }
        }
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
                if reply.add(offset, offset, FileType::Directory, &Path::new(&path)) {
                    break;
                }
                offset += 1;
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
                let attr = FileAttr {
                    ino: sid as u64 + 2,
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
                reply.entry(&ts, &attr, 0);
            }
            None => {
                reply.error(ENOENT);
                return;
            }
        };
    }

    fn is_snapshot(&self, ino: u64) -> bool {
        self.snapshots_ino.0 <= ino && ino < self.snapshots_ino.1
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
