use std::collections::HashMap;
use std::ffi::OsStr;

use fuser::{Errno, INodeNo};

struct Node {
    path: String,
    lookups: u64,
}

pub struct Inodes {
    nodes: HashMap<INodeNo, Node>,
    numbers: HashMap<String, INodeNo>,
    last: u64,
}

impl Inodes {
    pub fn new() -> Self {
        let root = Node {
            path: String::new(),
            lookups: 1,
        };

        Self {
            nodes: HashMap::from([(INodeNo::ROOT, root)]),
            numbers: HashMap::from([(String::new(), INodeNo::ROOT)]),
            last: INodeNo::ROOT.0,
        }
    }

    pub fn path(&self, ino: INodeNo) -> Result<String, Errno> {
        self.nodes
            .get(&ino)
            .map(|node| node.path.clone())
            .ok_or(Errno::ENOENT)
    }

    pub fn child(&self, parent: INodeNo, name: &OsStr) -> Result<String, Errno> {
        let name = name.to_str().ok_or(Errno::EINVAL)?;
        Ok(join(&self.path(parent)?, name))
    }

    pub fn remember(&mut self, path: &str) -> INodeNo {
        if let Some(&ino) = self.numbers.get(path) {
            self.nodes
                .get_mut(&ino)
                .expect("a numbered path has a node")
                .lookups += 1;
            return ino;
        }

        self.last += 1;
        let ino = INodeNo(self.last);
        let node = Node {
            path: path.to_owned(),
            lookups: 1,
        };
        self.nodes.insert(ino, node);
        self.numbers.insert(path.to_owned(), ino);
        ino
    }

    pub fn forget(&mut self, ino: INodeNo, lookups: u64) {
        let Some(node) = self.nodes.get_mut(&ino) else {
            return;
        };
        node.lookups = node.lookups.saturating_sub(lookups);

        if node.lookups == 0 && ino != INodeNo::ROOT {
            if self.numbers.get(&node.path) == Some(&ino) {
                self.numbers.remove(&node.path);
            }

            self.nodes.remove(&ino);
        }
    }

    pub fn unlink(&mut self, path: &str) {
        self.numbers.remove(path);
    }

    pub fn rename(&mut self, from: &str, to: &str) {
        self.unlink(to);

        for (&ino, node) in &mut self.nodes {
            let Some(rest) = node.path.strip_prefix(from) else {
                continue;
            };

            if !rest.is_empty() && !rest.starts_with('/') {
                continue;
            }

            let path = format!("{to}{rest}");

            if self.numbers.get(&node.path) == Some(&ino) {
                self.numbers.remove(&node.path);
                self.numbers.insert(path.clone(), ino);
            }

            node.path = path;
        }
    }
}

pub fn join(directory: &str, name: &str) -> String {
    if directory.is_empty() {
        name.to_owned()
    } else {
        format!("{directory}/{name}")
    }
}
