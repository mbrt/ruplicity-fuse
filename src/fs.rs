use fuse::{FileAttr, FileType, Filesystem, ReplyAttr, ReplyDirectory, ReplyEntry, Request};
use libc::{ENOENT, ENOSYS};
use time::{self, Timespec};
use ruplicity::{Backend, Backup, Snapshot};
use ruplicity::signatures::EntryType;

use std::collections::HashMap;
use std::io;
use std::path::Path;


pub struct RuplicityFs<B> {
    backup: Backup<B>,
    snapshots: SnapshotsInos,
    last_ino: u64,
}

struct SnapshotsInos {
    paths: HashMap<String, usize>,
}


impl<B: Backend> RuplicityFs<B> {
    /// Creates a new Filesystem instance for a duplicity backup.
    pub fn new(backup: Backup<B>) -> io::Result<Self> {
        let spaths = try!(SnapshotsInos::new(&backup));
        let last_ino = spaths.last_ino();

        Ok(RuplicityFs {
            backup: backup,
            snapshots: spaths,
            last_ino: last_ino,
        })
    }

    /// getattr for the root directory.
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

    /// getattr for a snapshot directory.
    fn getattr_snapshot(&mut self, ino: u64, reply: ReplyAttr) {
        match try_or_log!(self.snapshot_from_sid(self.snapshots.sid_from_ino(ino))) {
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

    /// readdir for the root directory.
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
            trace!("Add snapshot for offset {}", offset);
            let path = time_to_path(snapshot.time());
            if reply.add(offset, offset, FileType::Directory, &Path::new(&path)) {
                // the buffer is full, need to return
                break;
            }
        }
        reply.ok();
    }

    /// readdir for snapshot contents.
    fn readdir_files(&mut self, ino: u64, offset: u64, mut reply: ReplyDirectory) {
        let snapshot = match try_or_log!(self.snapshot_from_sid(self.snapshots
                                                                    .sid_from_ino(ino))) {
            Some(snapshot) => snapshot,
            None => {
                error!("No snapshot found");
                reply.error(ENOENT);
                return;
            }
        };
        if offset == 0 {
            let entries = try_or_log!(snapshot.entries());
            for (offset, entry) in entries.as_signature().enumerate() {
                let offset = offset as u64;
                let ftype = match entry.entry_type() {
                    EntryType::File | EntryType::HardLink | EntryType::Unknown(_) => {
                        FileType::RegularFile
                    }
                    EntryType::Dir => FileType::Directory,
                    EntryType::SymLink => FileType::Symlink,
                    EntryType::Fifo => FileType::NamedPipe,
                };
                let path = match entry.path().components().next() {
                    Some(p) => p,
                    None => {
                        continue;
                    }
                };
                trace!("Add ino {} for path {:?} with ftype {:?}",
                       self.last_ino + offset,
                       path,
                       ftype);
                reply.add(self.last_ino + offset, offset, ftype, path);
            }
        }
        reply.ok();
    }

    /// lookup for snapshots.
    fn lookup_snapshot(&mut self, name: &Path, reply: ReplyEntry) {
        let sid = match self.snapshots.sid_from_path(name) {
            Some(id) => *id,
            None => {
                reply.error(ENOENT);
                return;
            }
        };
        match try_or_log!(self.snapshot_from_sid(sid)) {
            Some(snapshot) => {
                let ts = snapshot.time();
                let attr = self.attr_snapshot(&snapshot, self.snapshots.ino_from_sid(sid));
                reply.entry(&ts, &attr, 0);
            }
            None => {
                reply.error(ENOENT);
            }
        };
    }

    /// Returns attributes for a snapshot.
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

    fn snapshot_from_sid(&self, sid: usize) -> io::Result<Option<Snapshot>> {
        self.backup.snapshots().map(|mut s| s.nth(sid))
    }
}

impl<B: Backend> Filesystem for RuplicityFs<B> {
    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        if ino == 1 {
            self.getattr_root(reply);
        } else if self.snapshots.is_snapshot(ino) {
            self.getattr_snapshot(ino, reply);
        } else {
            reply.error(ENOSYS);
        }
    }

    fn readdir(&mut self, _req: &Request, ino: u64, _fh: u64, offset: u64, reply: ReplyDirectory) {
        if ino == 1 {
            self.readdir_root(offset, reply);
        } else if self.snapshots.is_snapshot(ino) {
            self.readdir_files(ino, offset, reply);
        } else {
            error!("Unknown ino {} for readdir", ino);
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


impl SnapshotsInos {
    /// Creates a new Filesystem instance for a duplicity backup.
    pub fn new<B: Backend>(backup: &Backup<B>) -> io::Result<Self> {
        let mut spaths = HashMap::new();
        for (count, snapshot) in try!(backup.snapshots()).enumerate() {
            let path = time_to_path(snapshot.time());
            spaths.insert(path, count);
        }
        Ok(SnapshotsInos { paths: spaths })
    }

    fn sid_from_path(&self, name: &Path) -> Option<&usize> {
        self.paths.get(name.to_str().unwrap())
    }

    fn sid_from_ino(&self, ino: u64) -> usize {
        assert!(ino >= 2);
        ino as usize - 2
    }

    fn ino_from_sid(&self, sid: usize) -> u64 {
        sid as u64 + 2
    }

    fn last_ino(&self) -> u64 {
        self.ino_from_sid(self.paths.len())
    }

    /// Returns whether an inode is a snapshot.
    fn is_snapshot(&self, ino: u64) -> bool {
        ino >= 2 && ino < self.ino_from_sid(self.paths.len())
    }
}


fn time_to_path(time: Timespec) -> String {
    let time = time::at(time);
    time::strftime("%Y-%m-%d_%H-%M-%S", &time).unwrap()
}
