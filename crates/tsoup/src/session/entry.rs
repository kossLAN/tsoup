use std::collections::HashMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::EnvironmentStatus;
use crate::llm::{Image, Progress, ToolCall};
use crate::settings::{ModelChoice, Selection};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Entry {
    User {
        text: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        images: Vec<Image>,
    },
    Assistant {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        thinking: Option<String>,
        text: Option<String>,
        tool_calls: Vec<ToolCall>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        context: Option<Context>,
    },
    Tool {
        call_id: String,
        output: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        images: Vec<Image>,
        error: bool,
        diff: Option<FileDiff>,
    },
    Compaction {
        summary: String,
    },
    Report {
        agent: Uuid,
        state: AgentState,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Context {
    pub used: u64,
    pub window: u64,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentState {
    Running,
    Finished,
    Failed,
    Stopped,
}

impl AgentState {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Finished => "finished",
            Self::Failed => "failed",
            Self::Stopped => "stopped",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSummary {
    pub id: Uuid,
    pub parent: Option<Uuid>,
    pub name: String,
    pub state: AgentState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelChoice>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Question {
    pub id: Uuid,
    pub agent: Uuid,
    pub text: String,
    pub options: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Record {
    Entry {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        agent: Option<Uuid>,
        entry: Entry,
    },
    Title {
        title: String,
    },
    Clear,
    Rewind {
        length: usize,
    },
    Agent {
        agent: AgentSummary,
    },
    Actions {
        actions: Vec<Action>,
    },
    Selection {
        selection: Selection,
    },
    Group {
        group: Option<Uuid>,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    Snapshot {
        title: String,
        start: usize,
        entries: Vec<Entry>,
        actions: Vec<Action>,
        selection: Selection,
        environment: EnvironmentStatus,
        busy: bool,
        agents: Vec<AgentSummary>,
        questions: Vec<Question>,
        queued: HashMap<Uuid, Vec<Entry>>,
    },
    Entry {
        agent: Uuid,
        index: usize,
        entry: Entry,
    },
    TurnStarted,
    TurnFinished,
    TurnFailed {
        message: String,
    },
    TurnCancelled,
    Environment {
        environment: EnvironmentStatus,
    },
    Actions {
        actions: Vec<Action>,
    },
    Selection {
        selection: Selection,
    },
    Progress {
        agent: Uuid,
        progress: Progress,
    },
    Agent {
        agent: AgentSummary,
    },
    Questions {
        questions: Vec<Question>,
    },
    Queued {
        agent: Uuid,
        entries: Vec<Entry>,
    },
    Title {
        title: String,
    },
    Group {
        group: Option<Uuid>,
    },
    Cleared,
    Rewind {
        length: usize,
    },
    Compacting {
        agent: Uuid,
    },
}
