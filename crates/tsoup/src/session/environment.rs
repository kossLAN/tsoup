use std::io;
use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::Sandbox;
use crate::node::{Folder, Node};
use crate::vm::VmStatus;

const PLACEMENT_FILE: &str = "node.json";

pub enum Environment {
    Sandbox(Arc<Sandbox>),
    Node(Folder),
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EnvironmentStatus {
    Sandbox { vm: VmStatus },
    Node(Placement),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NewEnvironment {
    Sandbox,
    Node(Placement),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Placement {
    pub name: String,
    pub folder: String,
}

impl Placement {
    pub async fn create(&self, dir: &Path) -> io::Result<()> {
        tokio::fs::create_dir(dir).await?;
        tokio::fs::write(dir.join(PLACEMENT_FILE), serde_json::to_vec(self)?).await
    }

    pub async fn open(dir: &Path) -> io::Result<Option<Self>> {
        match tokio::fs::read(dir.join(PLACEMENT_FILE)).await {
            Ok(placement) => Ok(Some(serde_json::from_slice(&placement)?)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }
}

impl Environment {
    pub fn status(&self) -> EnvironmentStatus {
        match self {
            Self::Sandbox(sandbox) => EnvironmentStatus::Sandbox {
                vm: sandbox.vm.status(),
            },
            Self::Node(folder) => EnvironmentStatus::Node(folder.placement.clone()),
        }
    }

    pub fn runs_on(&self, node: &Node) -> bool {
        matches!(self, Self::Node(folder) if folder.placement.name == node.name)
    }

    pub async fn destroy(&self) {
        if let Self::Sandbox(sandbox) = self {
            sandbox.vm.destroy().await;
        }
    }

    pub async fn reconcile(&self) {
        if let Self::Sandbox(sandbox) = self {
            sandbox.vm.reconcile().await;
        }
    }
}
