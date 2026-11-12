use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rustix::io::Errno;
use tokio::runtime::Handle;

use crate::session::{Sessions, Workspace};

const UNTITLED: &str = "New session";
const ID_SUFFIX: usize = 8;

#[derive(Clone)]
pub struct Root {
    pub name: String,
    pub path: PathBuf,
}

pub struct Roots {
    sessions: Arc<Sessions>,
    runtime: Handle,
    known: Mutex<Vec<Root>>,
}

impl Roots {
    pub fn new(sessions: Arc<Sessions>, runtime: Handle) -> Self {
        Self {
            sessions,
            runtime,
            known: Mutex::default(),
        }
    }

    pub fn base(&self) -> &Path {
        self.sessions.root()
    }

    pub fn list(&self) -> Vec<Root> {
        let mut known = self.known.lock().unwrap();
        *known = self.current();
        known.clone()
    }

    pub fn find(&self, name: &str) -> io::Result<PathBuf> {
        let mut known = self.known.lock().unwrap();

        if !known.iter().any(|root| root.name == name) {
            *known = self.current();
        }

        known
            .iter()
            .find(|root| root.name == name)
            .map(|root| root.path.clone())
            .ok_or_else(|| Errno::NOENT.into())
    }

    fn current(&self) -> Vec<Root> {
        name(self.runtime.block_on(self.sessions.workspaces()))
    }
}

fn name(workspaces: Vec<Workspace>) -> Vec<Root> {
    let mut titled: HashMap<String, usize> = HashMap::new();

    for workspace in &workspaces {
        *titled.entry(folder(&workspace.title)).or_default() += 1;
    }

    workspaces
        .into_iter()
        .map(|workspace| {
            let folder = folder(&workspace.title);
            let name = if titled[&folder] > 1 {
                let id = workspace.id.simple().to_string();
                format!("{folder} {}", &id[id.len() - ID_SUFFIX..])
            } else {
                folder
            };

            Root {
                name,
                path: workspace.path,
            }
        })
        .collect()
}

fn folder(title: &str) -> String {
    let words: Vec<_> = title
        .split(|char: char| char == '/' || char.is_whitespace() || char.is_control())
        .filter(|word| !word.is_empty())
        .collect();
    let folder = words.join(" ");

    if folder.is_empty() || folder.chars().all(|char| char == '.') {
        UNTITLED.to_owned()
    } else {
        folder
    }
}
