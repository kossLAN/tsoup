mod binary;
pub mod guest;
pub mod mcp;
pub mod node;
pub mod rpc;
pub mod share;
pub mod workspace;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MAX_OUTPUT: usize = 64 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Exit {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub server: String,
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpFailure {
    pub server: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpListing {
    pub tools: Vec<McpTool>,
    pub failures: Vec<McpFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpOutput {
    pub output: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<McpImage>,
    pub error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpImage {
    pub media_type: String,
    pub data: String,
}
