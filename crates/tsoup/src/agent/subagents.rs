use std::mem;
use std::panic;
use std::pin::pin;
use std::sync::Arc;

use tokio::sync::Mutex;
use tokio::task::{JoinError, JoinSet};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::Agent;
use crate::session::{AgentState, Entry, Session, SessionError};

pub struct Report {
    pub agent: Uuid,
    pub state: AgentState,
    pub text: Option<String>,
}

impl From<Report> for Entry {
    fn from(Report { agent, state, text }: Report) -> Self {
        Self::Report { agent, state, text }
    }
}

pub fn describe(agent: Uuid, state: AgentState, text: Option<&str>) -> String {
    match (state, text) {
        (AgentState::Failed, error) => {
            format!("agent {agent} failed: {}", error.unwrap_or_default())
        },
        (AgentState::Stopped, _) => format!("the user stopped agent {agent}"),
        (_, Some(report)) => format!("agent {agent} reported:\n{report}"),
        (_, None) => format!("agent {agent} finished without a report"),
    }
}

// Dropping this aborts the tasks, so a subagent never outlives the turn of its parent.
#[derive(Default)]
pub struct Subagents(Mutex<Running>);

#[derive(Default)]
struct Running {
    tasks: JoinSet<Report>,
    agents: Vec<Uuid>,
    finished: Vec<Report>,
}

impl Running {
    fn join(&mut self, joined: Result<Report, JoinError>) {
        match joined {
            Ok(report) => {
                self.agents.retain(|agent| *agent != report.agent);
                self.finished.push(report);
            },
            Err(error) if error.is_panic() => panic::resume_unwind(error.into_panic()),
            Err(_) => {},
        }
    }

    fn reap(&mut self) {
        while let Some(joined) = self.tasks.try_join_next() {
            self.join(joined);
        }
    }
}

impl Subagents {
    pub async fn start(
        &self,
        agent: &Arc<Agent>,
        session: &Arc<Session>,
        id: Uuid,
        stop: CancellationToken,
        prompt: String,
        depth: usize,
    ) {
        let task = delegate(
            Arc::clone(agent),
            Arc::clone(session),
            id,
            stop,
            prompt,
            depth,
        );
        let mut running = self.0.lock().await;
        running.agents.push(id);
        running.tasks.spawn(task);
    }

    pub async fn busy(&self) -> bool {
        !self.0.lock().await.agents.is_empty()
    }

    pub async fn finished(&self) -> Vec<Report> {
        self.take(&[]).await.0
    }

    pub async fn take(&self, targets: &[Uuid]) -> (Vec<Report>, Vec<Uuid>) {
        let mut running = self.0.lock().await;
        running.reap();

        let (taken, kept) = mem::take(&mut running.finished)
            .into_iter()
            .partition(|report| targeted(targets, report.agent));
        running.finished = kept;

        let waiting = running
            .agents
            .iter()
            .copied()
            .filter(|&agent| targeted(targets, agent))
            .collect();
        (taken, waiting)
    }

    pub async fn wait(
        &self,
        session: &Session,
        parent: Uuid,
        targets: &[Uuid],
    ) -> Result<(), SessionError> {
        let steered = session.steered(parent).await?;
        let mut notified = pin!(steered.notified());
        notified.as_mut().enable();

        if session.has_steering(parent).await? {
            return Ok(());
        }

        let mut running = self.0.lock().await;

        while running.agents.iter().any(|&agent| targeted(targets, agent)) {
            tokio::select! {
                joined = running.tasks.join_next() => match joined {
                    Some(joined) => running.join(joined),
                    None => break,
                },
                () = notified.as_mut() => break,
            }
        }

        Ok(())
    }
}

fn targeted(targets: &[Uuid], agent: Uuid) -> bool {
    targets.is_empty() || targets.contains(&agent)
}

async fn delegate(
    agent: Arc<Agent>,
    session: Arc<Session>,
    id: Uuid,
    stop: CancellationToken,
    prompt: String,
    depth: usize,
) -> Report {
    let run = async {
        let prompt = Entry::User {
            text: prompt,
            images: Vec::new(),
        };
        session.push(id, prompt).await?;
        agent.run(&session, id, depth).await
    };
    let result = tokio::select! {
        result = run => Some(result),
        () = stop.cancelled() => None,
    };

    let (state, text) = match result {
        Some(Ok(report)) => (AgentState::Finished, report),
        Some(Err(error)) => (AgentState::Failed, Some(error.to_string())),
        None => (AgentState::Stopped, None),
    };
    let finished = session.finish_agent(id, state).await;
    session.settle(id).await;

    match finished {
        Ok(()) => Report {
            agent: id,
            state,
            text,
        },
        Err(error) => Report {
            agent: id,
            state: AgentState::Failed,
            text: Some(error.to_string()),
        },
    }
}
