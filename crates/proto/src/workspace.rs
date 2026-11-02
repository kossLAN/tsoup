use serde::{Deserialize, Serialize};

use crate::Exit;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Request {
    Exec { command: String, timeout_secs: u64 },
    Spawn { command: String },
    Read { path: String },
    Write { path: String, content: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Response {
    Exited(Exit),
    TimedOut,
    File { content: Option<String> },
    Done,
}
