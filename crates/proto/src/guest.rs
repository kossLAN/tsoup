use serde::{Deserialize, Serialize};

use crate::rpc::Protocol;
use crate::{mcp, workspace};

pub const PORT: &str = "tsoup.agent";
pub const WORKSPACE: &str = "/workspace";

pub struct Guest;

impl Protocol for Guest {
    type Request = Request;
    type Response = Response;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Request {
    Workspace(workspace::Request),
    Mcp(mcp::Request),
    Forward {
        from: u16,
        to: u16,
        require_listener: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Response {
    Workspace(workspace::Response),
    Mcp(mcp::Response),
    Done,
}
