use std::cmp::Ordering;
use std::io;
use std::iter::Peekable;
use std::path::{Component, Path};
use std::slice;

use ruplicity::Snapshot;
use ruplicity::signatures::{Entry as SigEntry, SnapshotEntries};


#[derive(Debug)]
pub struct SnapshotTree {
    /// Paths in the root backup.
    ///
    /// The root node is not used; only its children are.
    root: TreeNode,
}

pub struct ChildrenIter<'a, 'b> {
    tree_it: slice::Iter<'a, TreeNode>,
    entry_it: SnapshotEntries<'b>,
    curr_index: usize,
    path_depth: usize,
}

pub struct PathEntry<'a, 'b> {
    node: &'a TreeNode,
    entry: SigEntry<'b>,
    depth: usize,
}

pub struct NodeEntry<'a> {
    node: &'a TreeNode,
    depth: usize,
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
        let root = match TreeNode::new(0, 0, first_ino, &mut entries) {
            Some(node) => node,
            None => {
                // create a dummy root with empty children
                TreeNode {
                    index: 0,
                    ino: first_ino,
                    children: Vec::new(),
                }
            }
        };
        Ok(SnapshotTree { root: root })
    }

    pub fn inodes(&self) -> Option<(u64, u64)> {
        match (self.root.children.first(), self.root.children.last()) {
            (Some(first), Some(last)) => Some((first.inodes().0, last.inodes().1)),
            _ => None,
        }
    }

    pub fn children<'a, 'b>(&'a self, mut entries: SnapshotEntries<'b>) -> ChildrenIter<'a, 'b> {
        // skip the root
        entries.next().unwrap();
        ChildrenIter {
            tree_it: self.root.children.iter(),
            entry_it: entries,
            curr_index: 0,
            path_depth: 0,
        }
    }

    pub fn find_node(&self, ino: u64) -> Option<NodeEntry> {
        fn find_node_rec(node: &TreeNode, ino: u64, depth: usize) -> Option<NodeEntry> {
            // check if found
            if node.ino == ino {
                return Some(NodeEntry{ node: node, depth: depth });
            }
            // check if impossible to find
            let (first, last) = node.inodes();
            if ino < first || ino > last {
                return None;
            }
            // find the children having that inode
            let child_index = node.children.binary_search_by(|c| {
                let (first, last) = c.inodes();
                if ino < first {
                    Ordering::Less
                } else if ino > last {
                    Ordering::Greater
                } else {
                    Ordering::Equal
                }
            });
            match child_index {
                Ok(index) => find_node_rec(&node.children[index], ino, depth + 1),
                Err(_) => None,
            }
        }
        find_node_rec(&self.root, ino, 0)
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


impl<'a, 'b> Iterator for ChildrenIter<'a, 'b> {
    type Item = PathEntry<'a, 'b>;

    fn next(&mut self) -> Option<Self::Item> {
        self.tree_it.next().map(|child| {
            let skip = child.index - self.curr_index - 1;
            self.curr_index += skip + 1;
            PathEntry {
                node: &child,
                entry: self.entry_it.nth(skip).unwrap(),
                depth: self.path_depth,
            }
        })
    }
}


impl<'a, 'b> PathEntry<'a, 'b> {
    pub fn as_signature(&self) -> &SigEntry<'b> {
        &self.entry
    }

    pub fn path(&self) -> Option<&Path> {
        match self.entry.path().components().nth(self.depth).unwrap() {
            Component::Normal(p) => Some(p.as_ref()),
            _ => None,
        }
    }

    pub fn ino(&self) -> u64 {
        self.node.ino
    }
}


impl<'a> NodeEntry<'a> {
    pub fn ino(&self) -> u64 {
        self.node.ino
    }

    pub fn children<'b>(&self, mut entries: SnapshotEntries<'b>) -> ChildrenIter<'a, 'b> {
        // skip the root
        entries.next().unwrap();
        ChildrenIter {
            tree_it: self.node.children.iter(),
            entry_it: entries,
            curr_index: 0,
            path_depth: self.depth,
        }
    }
}
