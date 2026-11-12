use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::rpc::Protocol;
use crate::{mcp, workspace};

pub const PROTOCOL: &str = "tsoup-node";

pub struct Node;

impl Protocol for Node {
    type Request = Request;
    type Response = Response;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Request {
    Hello,
    Folders,
    Load {
        session: Uuid,
        offset: u64,
    },
    Create {
        session: Uuid,
        folder: String,
    },
    Append {
        session: Uuid,
        record: Value,
    },
    Delete {
        session: Uuid,
    },
    Workspace {
        folder: String,
        request: workspace::Request,
    },
    Mcp(mcp::Request),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Response {
    Hello {
        directory: String,
        #[serde(default)]
        sandboxed: bool,
        sessions: Vec<StoredSession>,
    },
    Folders {
        folders: Vec<String>,
    },
    Records {
        records: Vec<Value>,
        next: Option<u64>,
    },
    Workspace(workspace::Response),
    Mcp(mcp::Response),
    Done,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredSession {
    pub id: Uuid,
    pub folder: String,
}

pub fn valid_folder(name: &str) -> bool {
    !name.is_empty() && !name.starts_with('.') && !name.contains(['/', '\0'])
}
