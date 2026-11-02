use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{McpListing, McpOutput};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Request {
    Tools,
    Call {
        server: String,
        tool: String,
        arguments: Map<String, Value>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Response {
    Tools(McpListing),
    Output(McpOutput),
}
