use serde::Serialize;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::{AgentState, AgentSummary, Entry, Question};

const PAGE_SIZE: usize = 50;

#[derive(Debug, Clone, Serialize)]
pub struct EntryPage {
    pub start: usize,
    pub entries: Vec<Entry>,
}

pub struct Thread {
    pub summary: AgentSummary,
    pub entries: Vec<Entry>,
    pub steering: Vec<Entry>,
    pub active: bool,
    pub stop: CancellationToken,
}

pub struct Pending {
    pub question: Question,
    pub reply: oneshot::Sender<String>,
}

impl Thread {
    pub fn new(summary: AgentSummary) -> Self {
        Self {
            summary,
            entries: Vec::new(),
            steering: Vec::new(),
            active: false,
            stop: CancellationToken::new(),
        }
    }

    pub fn main(id: Uuid) -> Self {
        Self::new(AgentSummary {
            id,
            parent: None,
            name: String::new(),
            state: AgentState::Finished,
            model: None,
        })
    }

    pub fn page(&self, before: usize) -> EntryPage {
        let end = before.min(self.entries.len());
        let start = end.saturating_sub(PAGE_SIZE);
        EntryPage {
            start,
            entries: self.entries[start..end].to_vec(),
        }
    }

    pub fn tail(&self) -> EntryPage {
        self.page(self.entries.len())
    }

    pub fn running(&self) -> bool {
        self.summary.state == AgentState::Running
    }

    pub fn idle(&mut self) {
        self.active = false;
        self.steering.clear();
    }
}
