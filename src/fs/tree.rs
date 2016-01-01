use std::io;
use std::iter::Peekable;

use ruplicity::Snapshot;
use ruplicity::signatures::SnapshotEntries;


#[derive(Debug)]
pub struct SnapshotTree {
    /// paths in the root backup.
    children: Vec<TreeNode>,
}

#[derive(Debug)]
struct TreeNode {
    /// The index of the entry in the `SnapshotEntries` iterator.
    index: usize,
    /// The inode for this node.
    ino: u64,
    /// Children nodes.
    ///
    /// Each children is a sub-path of the current path.
    children: Vec<TreeNode>,
}


impl SnapshotTree {
    pub fn new(snapshot: &Snapshot, first_ino: u64) -> io::Result<Self> {
        let entries = try!(snapshot.entries());
        let mut entries = entries.as_signature().peekable();
        let children = match TreeNode::new(0, 0, first_ino, &mut entries) {
            Some(node) => node.children,
            None => Vec::new(),
        };
        Ok(SnapshotTree { children: children })
    }

    pub fn inodes(&self) -> Option<(u64, u64)> {
        match (self.children.first(), self.children.last()) {
            (Some(first), Some(last)) => Some((first.inodes().0, last.inodes().1)),
            _ => None,
        }
    }
}


impl TreeNode {
    pub fn new(path_depth: usize,
               index: usize,
               ino: u64,
               entries: &mut Peekable<SnapshotEntries>)
               -> Option<Self> {
        // need to check if there are more entries
        entries.next().map(|_| {
            // in this case the 'depth' component of the current path is the path handled by this
            // node
            TreeNode {
                index: index,
                ino: ino,
                children: Self::new_children(path_depth, index + 1, ino + 1, entries),
            }
        })
    }

    pub fn new_children(path_depth: usize,
                        index: usize,
                        first_ino: u64,
                        entries: &mut Peekable<SnapshotEntries>)
                        -> Vec<Self> {
        let mut result = Vec::new();
        let mut index = index;
        let mut ino = first_ino;

        loop {
            {
                // peek the next entry
                let entry = match entries.peek() {
                    Some(entry) => entry,
                    None => {
                        // end of iteration, there are no more entries
                        break;
                    }
                };
                if !entry.path().components().nth(path_depth).is_some() {
                    // the entry does not belong to the current children
                    // this is because it does not have the 'path-depth' path component, so it must be
                    // a parent directory (different than the current one)
                    break;
                }
            }
            let child = match TreeNode::new(path_depth + 1, index, ino, entries) {
                Some(child) => child,
                None => {
                    // failed to create the child, break!
                    break;
                }
            };

            // compute the number of entries by looking at added inodes
            let child_entries = child.inodes().1 - ino + 1;
            ino += child_entries;
            index += child_entries as usize;

            // push the just created child
            result.push(child);
        }
        result
    }

    pub fn inodes(&self) -> (u64, u64) {
        let last = match self.children.last() {
            Some(node) => node.inodes().1,
            None => self.ino,
        };
        (self.ino, last)
    }
}
