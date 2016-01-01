mod tree;

use fuse::{FileAttr, FileType, Filesystem, ReplyAttr, ReplyDirectory, ReplyEntry, Request};
use libc::{ENOENT, ENOSYS};
use time::{self, Timespec};
use ruplicity::{Backend, Backup, Snapshot};
use ruplicity::signatures::EntryType;

use std::collections::HashMap;
use std::io;
use std::path::Path;

use self::tree::SnapshotTree;


pub struct RuplicityFs<B> {
    backup: Backup<B>,
    snapshots: SnapshotsInos,
    trees: Vec<Option<SnapshotTree>>,
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
        let trees = {
            // vec![] macro does not work because SnapshotTree is not Clone
            let mut v = Vec::new();
            for _ in 0..spaths.len() {
                v.push(None);
            }
            v
        };

        Ok(RuplicityFs {
            backup: backup,
            snapshots: spaths,
            last_ino: last_ino,
            trees: trees,
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
        let snapshot = try_or_log!(self.snapshot_from_sid(self.snapshots.sid_from_ino(ino)));
        let ts = snapshot.time();
        let attr = self.attr_snapshot(&snapshot, ino);
        reply.attr(&ts, &attr);
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
        let snapshot = try_or_log!(self.snapshot_from_ino(ino));
        if offset == 0 {
            let tree = try_or_log!(SnapshotTree::new(&snapshot, self.last_ino));
            let entries = try_or_log!(snapshot.entries());
            for (offset, entry) in tree.children(entries.as_signature()).enumerate() {
                let offset = offset as u64;
                let ftype = match entry.as_signature().entry_type() {
                    EntryType::File | EntryType::HardLink | EntryType::Unknown(_) => {
                        FileType::RegularFile
                    }
                    EntryType::Dir => FileType::Directory,
                    EntryType::SymLink => FileType::Symlink,
                    EntryType::Fifo => FileType::NamedPipe,
                };
                let path = unwrap_opt_or_continue!(entry.path());
                trace!("Add ino {} for path {:?} with ftype {:?}",
                       entry.ino(),
                       path,
                       ftype);
                reply.add(entry.ino(), offset, ftype, path);
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
        let snapshot = try_or_log!(self.snapshot_from_sid(sid));
        let ts = snapshot.time();
        let attr = self.attr_snapshot(&snapshot, self.snapshots.ino_from_sid(sid));
        reply.entry(&ts, &attr, 0);
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

    fn snapshot_from_sid(&self, sid: usize) -> io::Result<Snapshot> {
        match try!(self.backup.snapshots()).nth(sid) {
            Some(s) => Ok(s),
            None => Err(io::Error::new(io::ErrorKind::NotFound, "Snapshot not found")),
        }
    }

    fn snapshot_from_ino(&self, ino: u64) -> io::Result<Snapshot> {
        self.snapshot_from_sid(self.snapshots.sid_from_ino(ino))
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

    pub fn len(&self) -> usize {
        self.paths.len()
    }

    pub fn sid_from_path(&self, name: &Path) -> Option<&usize> {
        self.paths.get(name.to_str().unwrap())
    }

    pub fn sid_from_ino(&self, ino: u64) -> usize {
        assert!(ino >= 2);
        ino as usize - 2
    }

    pub fn ino_from_sid(&self, sid: usize) -> u64 {
        sid as u64 + 2
    }

    pub fn last_ino(&self) -> u64 {
        self.ino_from_sid(self.paths.len())
    }

    /// Returns whether an inode is a snapshot.
    pub fn is_snapshot(&self, ino: u64) -> bool {
        ino >= 2 && ino < self.ino_from_sid(self.paths.len())
    }
}


fn time_to_path(time: Timespec) -> String {
    let time = time::at(time);
    time::strftime("%Y-%m-%d_%H-%M-%S", &time).unwrap()
}
