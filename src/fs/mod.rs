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
        let trees = (0..spaths.len()).map(|_| None).collect();

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
    fn readdir_files(&mut self, ino: u64, mut offset: u64, mut reply: ReplyDirectory) {
        // offset is the last returned offset
        if offset == 0 {
            // assume first two replies does fit in the buffer
            reply.add(ino, 0, FileType::Directory, &Path::new("."));
            reply.add(ino, 1, FileType::Directory, &Path::new(".."));
            offset += 1;
        }

        let sid = self.snapshots.sid_from_ino(ino);
        let (tree, snapshot) = try_or_log!(self.tree_for_snapshot(sid));
        let entries = try_or_log!(snapshot.entries());
        for (offset, entry) in tree.children(entries.as_signature())
                                   .enumerate()
                                   .skip(offset as usize - 1) {
            let offset = offset as u64 + 2;
            let ftype = from_entry_type(entry.as_signature().entry_type());
            let path = unwrap_opt_or_continue!(entry.path());
            trace!("Add ino {} for path {:?} with ftype {:?}",
                   entry.ino(),
                   path,
                   ftype);
            if reply.add(entry.ino(), offset, ftype, path) {
                // the buffer is full, need to return
                break;
            }
        }
        reply.ok();
    }

    /// lookup for snapshots.
    fn lookup_snapshot(&mut self, name: &Path, reply: ReplyEntry) {
        let sid = unwrap_opt_or_error!(self.snapshots.sid_from_path(name),
                                       reply,
                                       ENOENT,
                                       "Can't find snapshot for path {:?}",
                                       name);
        let snapshot = try_or_log!(self.snapshot_from_sid(sid));
        let ts = snapshot.time();
        let attr = self.attr_snapshot(&snapshot, self.snapshots.ino_from_sid(sid));
        reply.entry(&ts, &attr, 0);
    }

    /// lookup for snapshot entries.
    fn lookup_entry(&mut self, parent: u64, name: &Path, reply: ReplyEntry) {
        let (tree, sid) = unwrap_opt_or_error!(self.find_tree_with_ino(parent),
                                               reply,
                                               ENOENT,
                                               "Can't find tree for ino {}",
                                               parent);
        let parent_entry = unwrap_opt_or_error!(tree.find_node(parent),
                                                reply,
                                                ENOENT,
                                                "Can't find entry for ino {}",
                                                parent);
        let snapshot = try_or_log!(self.snapshot_from_sid(sid));
        let entries = try_or_log!(snapshot.entries());
        let entry = parent_entry.children(entries.as_signature()).find(|entry| {
            match entry.path() {
                Some(path) => path == name,
                None => false,
            }
        });
        let entry = unwrap_opt_or_error!(entry,
                                         reply,
                                         ENOENT,
                                         "Can't find path '{:?}' in parent {}",
                                         name,
                                         parent);
        let sig_entry = entry.as_signature();
        let ts = sig_entry.mtime();
        let attr = FileAttr {
            ino: entry.ino(),
            size: sig_entry.size_hint().map_or(0, |sh| sh.1 as u64),
            blocks: 0,
            atime: ts,
            mtime: ts,
            ctime: ts,
            crtime: ts,
            kind: from_entry_type(sig_entry.entry_type()),
            perm: sig_entry.mode().map_or(0o777, |p| p as u16),
            nlink: 0,
            uid: sig_entry.userid().unwrap_or(100),
            gid: sig_entry.groupid().unwrap_or(100),
            rdev: 0,
            flags: 0,
        };
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

    #[allow(dead_code)]
    fn snapshot_from_ino(&self, ino: u64) -> io::Result<Snapshot> {
        self.snapshot_from_sid(self.snapshots.sid_from_ino(ino))
    }

    fn tree_for_snapshot(&mut self, sid: usize) -> io::Result<(&SnapshotTree, Snapshot)> {
        // check if already present
        if self.trees[sid].is_some() {
            return Ok((self.trees[sid].as_ref().unwrap(),
                       try!(self.snapshot_from_sid(sid))));
        }

        // build the tree and recurse
        {
            let tree = {
                let snapshot = try!(self.snapshot_from_sid(sid));
                try!(SnapshotTree::new(&snapshot, self.last_ino + 1))
            };
            let opt_tree = &mut self.trees[sid];
            // update the last ino
            if let Some((_, last)) = tree.inodes() {
                self.last_ino = last;
            }
            *opt_tree = Some(tree);
        }
        self.tree_for_snapshot(sid)
    }

    /// Returns the tree having that inode and the corresponding snapshot id.
    fn find_tree_with_ino(&self, ino: u64) -> Option<(&SnapshotTree, usize)> {
        self.trees
            .iter()
            .enumerate()
            .find(|opt_tree| {
                opt_tree.1.as_ref().map_or(false, |tree| {
                    match tree.inodes() {
                        Some((first, last)) => first <= ino && ino <= last,
                        None => false,
                    }
                })
            })
            .map(|opt_tree| {
                match opt_tree {
                    (sid, tree) => (tree.as_ref().unwrap(), sid),
                }
            })
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
            self.lookup_entry(parent, name, reply);
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

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    pub fn sid_from_path(&self, name: &Path) -> Option<usize> {
        self.paths.get(name.to_str().unwrap()).map(Clone::clone)
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

fn from_entry_type(et: EntryType) -> FileType {
    // can't implement From nor Into traits, because neither EntryType nor FileType are from this
    // crate
    match et {
        EntryType::File | EntryType::HardLink | EntryType::Unknown(_) => FileType::RegularFile,
        EntryType::Dir => FileType::Directory,
        EntryType::SymLink => FileType::Symlink,
        EntryType::Fifo => FileType::NamedPipe,
    }
}
