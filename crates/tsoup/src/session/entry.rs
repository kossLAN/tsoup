use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::llm::ToolCall;
use crate::vm::VmStatus;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Entry {
    User {
        text: String,
    },
    Assistant {
        text: Option<String>,
        tool_calls: Vec<ToolCall>,
    },
    Tool {
        call_id: String,
        output: String,
        error: bool,
        diff: Option<FileDiff>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDiff {
    pub path: String,
    pub patch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Action {
    #[schemars(description = "The button text, e.g. Run")]
    pub label: String,
    #[schemars(description = "The command, run detached with sh -c in /workspace")]
    pub command: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    Snapshot {
        title: String,
        start: usize,
        entries: Vec<Entry>,
        actions: Vec<Action>,
        vm: VmStatus,
        busy: bool,
    },
    Entry {
        entry: Entry,
    },
    TurnStarted,
    TurnFinished,
    TurnFailed {
        message: String,
    },
    TurnCancelled,
    Vm {
        status: VmStatus,
    },
    Actions {
        actions: Vec<Action>,
    },
}
