use serde::{Deserialize, Serialize};

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
    Create {
        folder: String,
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
    },
    Folders {
        folders: Vec<String>,
    },
    Workspace(workspace::Response),
    Mcp(mcp::Response),
    Done,
}

pub fn valid_folder(name: &str) -> bool {
    !name.is_empty() && !name.starts_with('.') && !name.contains(['/', '\0'])
}
