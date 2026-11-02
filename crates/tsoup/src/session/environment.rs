use std::sync::Arc;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Record, Sandbox, SessionError};
use crate::node::{Folder, Node};
use crate::vm::VmStatus;

pub enum Environment {
    Sandbox(Arc<Sandbox>),
    Node(Folder),
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EnvironmentStatus {
    Sandbox { vm: VmStatus },
    Node { name: String, folder: String },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NewEnvironment {
    Sandbox,
    Node { name: String, folder: String },
}

impl Environment {
    pub fn status(&self) -> EnvironmentStatus {
        match self {
            Self::Sandbox(sandbox) => EnvironmentStatus::Sandbox {
                vm: sandbox.vm.status(),
            },
            Self::Node(folder) => EnvironmentStatus::Node {
                name: folder.node.name.clone(),
                folder: folder.name.clone(),
            },
        }
    }

    pub fn runs_on(&self, node: &Arc<Node>) -> bool {
        matches!(self, Self::Node(folder) if Arc::ptr_eq(&folder.node, node))
    }

    pub async fn records(&self, id: Uuid) -> Result<Vec<Record>, SessionError> {
        match self {
            Self::Sandbox(sandbox) => sandbox.records().await,
            Self::Node(folder) => Ok(folder.node.records(id).await?),
        }
    }

    pub async fn append(&self, id: Uuid, record: &Record) -> Result<(), SessionError> {
        match self {
            Self::Sandbox(sandbox) => sandbox.append(record).await,
            Self::Node(folder) => Ok(folder.node.append(id, record).await?),
        }
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), SessionError> {
        match self {
            Self::Sandbox(sandbox) => Ok(sandbox.destroy().await?),
            Self::Node(folder) => Ok(folder.node.delete(id).await?),
        }
    }

    pub async fn reconcile(&self) {
        if let Self::Sandbox(sandbox) = self {
            sandbox.vm.reconcile().await;
        }
    }
}
