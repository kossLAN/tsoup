mod entry;
mod environment;
mod sandbox;
mod thread;

use std::cmp::Reverse;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Arc;
use std::{io, mem};

use serde::Serialize;
use tokio::sync::{Mutex, RwLock, broadcast, oneshot};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tsoup_proto::node::{StoredSession, valid_folder};
use uuid::Uuid;

pub use self::entry::{
    Action, AgentState, AgentSummary, Context, Entry, Event, FileDiff, Question, Record,
};
pub use self::environment::{Environment, EnvironmentStatus, NewEnvironment};
pub use self::sandbox::{Sandbox, WORKSPACE_DIR};
pub use self::thread::EntryPage;
use self::thread::{Pending, Thread};
use crate::node::{Folder, Node, NodeError, NodeSummary};
use crate::settings::{ModelChoice, Selection};
use crate::vm::Builder;

const EVENT_BUFFER: usize = 256;
const TITLE_LENGTH: usize = 60;

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("no such session")]
    NotFound,
    #[error("a turn is already running")]
    Busy,
    #[error("no such agent")]
    NoSuchAgent,
    #[error("the agent is not running")]
    AgentIdle,
    #[error("no such question is pending")]
    NoSuchQuestion,
    #[error("no such node is connected")]
    NoSuchNode,
    #[error("a node named {0} is already connected")]
    NodeConnected(String),
    #[error("the session does not run in a sandbox")]
    NotSandbox,
    #[error("{0:?} is not a folder name, it must not be empty, start with '.' or contain '/'")]
    InvalidFolder(String),
    #[error(transparent)]
    Node(#[from] NodeError),
    #[error("session storage: {0}")]
    Io(#[from] io::Error),
    #[error("encoding the session: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionSummary {
    pub id: Uuid,
    pub title: String,
    pub busy: bool,
    pub environment: EnvironmentStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ListEvent {
    Snapshot {
        sessions: Vec<SessionSummary>,
        nodes: Vec<NodeSummary>,
    },
    Created {
        session: SessionSummary,
    },
    Removed {
        id: Uuid,
    },
    Busy {
        id: Uuid,
        busy: bool,
    },
    Environment {
        id: Uuid,
        environment: EnvironmentStatus,
    },
    Nodes {
        nodes: Vec<NodeSummary>,
    },
}

#[derive(Clone)]
pub struct Events {
    id: Uuid,
    session: broadcast::Sender<Event>,
    list: broadcast::Sender<ListEvent>,
}

impl Events {
    fn new(id: Uuid, list: broadcast::Sender<ListEvent>) -> Self {
        Self {
            id,
            session: broadcast::channel(EVENT_BUFFER).0,
            list,
        }
    }

    pub fn emit(&self, event: Event) {
        let change = match &event {
            Event::TurnStarted => Some(ListEvent::Busy {
                id: self.id,
                busy: true,
            }),
            Event::TurnFinished | Event::TurnFailed { .. } | Event::TurnCancelled => {
                Some(ListEvent::Busy {
                    id: self.id,
                    busy: false,
                })
            },
            Event::Environment { environment } => Some(ListEvent::Environment {
                id: self.id,
                environment: environment.clone(),
            }),
            _ => None,
        };

        let _ = self.session.send(event);
        if let Some(change) = change {
            let _ = self.list.send(change);
        }
    }

    fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.session.subscribe()
    }
}

struct SessionData {
    title: String,
    threads: Vec<Thread>,
    questions: Vec<Pending>,
    actions: Vec<Action>,
    selection: Selection,
}

impl SessionData {
    fn new(id: Uuid) -> Self {
        Self {
            title: String::new(),
            threads: vec![Thread::main(id)],
            questions: Vec::new(),
            actions: Vec::new(),
            selection: Selection::default(),
        }
    }

    fn replay(id: Uuid, records: Vec<Record>) -> Self {
        let mut data = Self::new(id);

        for record in records {
            data.apply(record);
        }

        for thread in &mut data.threads {
            if thread.running() {
                thread.summary.state = AgentState::Stopped;
            }
        }

        data
    }

    fn apply(&mut self, record: Record) -> Option<Event> {
        match record {
            Record::Entry { agent, entry } => {
                let main = self.threads[0].summary.id;
                let agent = agent.unwrap_or(main);

                if let Entry::User { text, .. } = &entry
                    && agent == main
                    && self.title.is_empty()
                {
                    self.title = title(text);
                }

                let thread = self.thread_mut(agent).ok()?;
                thread.entries.push(entry.clone());
                Some(Event::Entry {
                    agent,
                    index: thread.entries.len() - 1,
                    entry,
                })
            },
            Record::Rewind { length } => {
                self.threads[0].entries.truncate(length);
                Some(Event::Rewind { length })
            },
            Record::Agent { agent } => {
                match self.thread_mut(agent.id) {
                    Ok(thread) => thread.summary.clone_from(&agent),
                    Err(_) => self.threads.push(Thread::new(agent.clone())),
                }

                Some(Event::Agent { agent })
            },
            Record::Actions { actions } => {
                self.actions.clone_from(&actions);
                Some(Event::Actions { actions })
            },
            Record::Selection { selection } => {
                self.selection.clone_from(&selection);
                Some(Event::Selection { selection })
            },
        }
    }

    fn thread(&self, agent: Uuid) -> Result<&Thread, SessionError> {
        self.threads
            .iter()
            .find(|thread| thread.summary.id == agent)
            .ok_or(SessionError::NoSuchAgent)
    }

    fn thread_mut(&mut self, agent: Uuid) -> Result<&mut Thread, SessionError> {
        self.threads
            .iter_mut()
            .find(|thread| thread.summary.id == agent)
            .ok_or(SessionError::NoSuchAgent)
    }

    fn subtree(&self, root: Uuid) -> Vec<Uuid> {
        let mut agents = vec![root];
        let mut visited = 0;

        while let Some(&parent) = agents.get(visited) {
            agents.extend(
                self.threads
                    .iter()
                    .filter(|thread| thread.summary.parent == Some(parent))
                    .map(|thread| thread.summary.id),
            );
            visited += 1;
        }

        agents
    }

    fn agents(&self) -> Vec<AgentSummary> {
        self.threads[1..]
            .iter()
            .map(|thread| thread.summary.clone())
            .collect()
    }

    fn questions(&self) -> Vec<Question> {
        self.questions
            .iter()
            .map(|pending| pending.question.clone())
            .collect()
    }
}

pub struct Sessions {
    root: PathBuf,
    runtime: PathBuf,
    builder: Arc<Builder>,
    by_id: RwLock<HashMap<Uuid, Arc<Session>>>,
    nodes: RwLock<BTreeMap<String, Arc<Node>>>,
    changes: broadcast::Sender<ListEvent>,
}

impl Sessions {
    pub async fn load(
        root: PathBuf,
        runtime: PathBuf,
        builder: Arc<Builder>,
    ) -> Result<Self, SessionError> {
        let mut sessions = Self {
            root,
            runtime,
            builder,
            by_id: RwLock::default(),
            nodes: RwLock::default(),
            changes: broadcast::channel(EVENT_BUFFER).0,
        };
        let mut dirs = tokio::fs::read_dir(&sessions.root).await?;

        while let Some(dir) = dirs.next_entry().await? {
            let Ok(id) = dir.file_name().to_string_lossy().parse::<Uuid>() else {
                continue;
            };

            let events = sessions.events(id);
            let sandbox = Sandbox::open(
                dir.path(),
                sessions.sandbox_runtime(id),
                Arc::clone(&sessions.builder),
                events.clone(),
            )
            .await;

            match Session::open(id, Environment::Sandbox(Arc::new(sandbox)), events).await {
                Ok(session) => {
                    sessions.by_id.get_mut().insert(id, Arc::new(session));
                },
                Err(error) => tracing::warn!(%id, %error, "skipping an unreadable session"),
            }
        }

        Ok(sessions)
    }

    pub async fn list(&self) -> Vec<SessionSummary> {
        let sessions: Vec<_> = self.by_id.read().await.values().cloned().collect();
        let mut summaries = Vec::with_capacity(sessions.len());

        for session in sessions {
            summaries.push(session.summary().await);
        }

        summaries.sort_by_key(|summary| Reverse(summary.id));
        summaries
    }

    pub async fn subscribe(&self) -> (ListEvent, broadcast::Receiver<ListEvent>) {
        let receiver = self.changes.subscribe();
        let snapshot = ListEvent::Snapshot {
            sessions: self.list().await,
            nodes: summaries(&*self.nodes.read().await),
        };
        (snapshot, receiver)
    }

    pub async fn get(&self, id: Uuid) -> Result<Arc<Session>, SessionError> {
        self.by_id
            .read()
            .await
            .get(&id)
            .cloned()
            .ok_or(SessionError::NotFound)
    }

    pub async fn has_node(&self, name: &str) -> bool {
        self.nodes.read().await.contains_key(name)
    }

    pub async fn folders(&self, node: &str) -> Result<Vec<String>, SessionError> {
        let node = self
            .nodes
            .read()
            .await
            .get(node)
            .cloned()
            .ok_or(SessionError::NoSuchNode)?;
        Ok(node.folders().await?)
    }

    pub async fn create(
        &self,
        environment: NewEnvironment,
        selection: Selection,
    ) -> Result<Arc<Session>, SessionError> {
        let id = Uuid::now_v7();
        let events = self.events(id);
        let nodes = self.nodes.read().await;

        let environment = match environment {
            NewEnvironment::Sandbox => {
                let sandbox = Sandbox::create(
                    self.root.join(id.to_string()),
                    self.sandbox_runtime(id),
                    Arc::clone(&self.builder),
                    events.clone(),
                )
                .await?;
                Environment::Sandbox(Arc::new(sandbox))
            },
            NewEnvironment::Node { name, folder } => {
                if !valid_folder(&folder) {
                    return Err(SessionError::InvalidFolder(folder));
                }

                let node = nodes.get(&name).ok_or(SessionError::NoSuchNode)?;
                node.create(id, &folder).await?;
                Environment::Node(Folder {
                    node: Arc::clone(node),
                    name: folder,
                })
            },
        };

        let session = Arc::new(Session::new(id, environment, SessionData::new(id), events));
        session.select(selection).await?;
        self.insert(Arc::clone(&session)).await;
        Ok(session)
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), SessionError> {
        let session = self
            .by_id
            .write()
            .await
            .remove(&id)
            .ok_or(SessionError::NotFound)?;
        session.cancel_turn().await;
        let _ = self.changes.send(ListEvent::Removed { id });
        session.environment.delete(id).await
    }

    pub async fn attach(
        &self,
        node: Arc<Node>,
        stored: Vec<StoredSession>,
    ) -> Result<(), SessionError> {
        let mut opened = Vec::with_capacity(stored.len());

        for StoredSession { id, folder } in stored {
            let environment = Environment::Node(Folder {
                node: Arc::clone(&node),
                name: folder,
            });

            match Session::open(id, environment, self.events(id)).await {
                Ok(session) => opened.push(session),
                Err(error) => tracing::warn!(%id, %error, "skipping an unreadable session"),
            }
        }

        let mut nodes = self.nodes.write().await;

        if nodes.contains_key(&node.name) {
            return Err(SessionError::NodeConnected(node.name.clone()));
        }

        for session in opened {
            if self.by_id.read().await.contains_key(&session.id) {
                tracing::warn!(id = %session.id, "skipping a node session whose id is taken");
                continue;
            }

            self.insert(Arc::new(session)).await;
        }

        nodes.insert(node.name.clone(), node);
        let _ = self.changes.send(ListEvent::Nodes {
            nodes: summaries(&nodes),
        });
        Ok(())
    }

    pub async fn detach(&self, node: &Arc<Node>) {
        let mut nodes = self.nodes.write().await;
        nodes.remove(&node.name);

        let removed: Vec<_> = self
            .by_id
            .write()
            .await
            .extract_if(|_, session| session.environment.runs_on(node))
            .map(|(_, session)| session)
            .collect();

        for session in removed {
            session.cancel_turn().await;
            let _ = self.changes.send(ListEvent::Removed { id: session.id });
        }

        let _ = self.changes.send(ListEvent::Nodes {
            nodes: summaries(&nodes),
        });
    }

    async fn insert(&self, session: Arc<Session>) {
        let summary = session.summary().await;
        self.by_id.write().await.insert(session.id, session);
        let _ = self.changes.send(ListEvent::Created { session: summary });
    }

    fn events(&self, id: Uuid) -> Events {
        Events::new(id, self.changes.clone())
    }

    fn sandbox_runtime(&self, id: Uuid) -> PathBuf {
        self.runtime.join(id.to_string())
    }
}

fn summaries(nodes: &BTreeMap<String, Arc<Node>>) -> Vec<NodeSummary> {
    nodes.values().map(|node| node.summary()).collect()
}

pub struct Session {
    pub id: Uuid,
    pub environment: Environment,
    data: Mutex<SessionData>,
    events: Events,
    turn: Mutex<Option<JoinHandle<()>>>,
}

impl Session {
    fn new(id: Uuid, environment: Environment, data: SessionData, events: Events) -> Self {
        Self {
            id,
            environment,
            data: Mutex::new(data),
            events,
            turn: Mutex::new(None),
        }
    }

    async fn open(
        id: Uuid,
        environment: Environment,
        events: Events,
    ) -> Result<Self, SessionError> {
        let data = SessionData::replay(id, environment.records(id).await?);
        Ok(Self::new(id, environment, data, events))
    }

    pub fn sandbox(&self) -> Result<&Arc<Sandbox>, SessionError> {
        match &self.environment {
            Environment::Sandbox(sandbox) => Ok(sandbox),
            Environment::Node(_) => Err(SessionError::NotSandbox),
        }
    }

    pub async fn summary(&self) -> SessionSummary {
        SessionSummary {
            id: self.id,
            title: self.data.lock().await.title.clone(),
            busy: self.busy().await,
            environment: self.environment.status(),
        }
    }

    pub async fn entries(&self, agent: Uuid) -> Result<Vec<Entry>, SessionError> {
        Ok(self.data.lock().await.thread(agent)?.entries.clone())
    }

    pub async fn page(&self, agent: Uuid, before: usize) -> Result<EntryPage, SessionError> {
        Ok(self.data.lock().await.thread(agent)?.page(before))
    }

    pub async fn push(&self, agent: Uuid, entry: Entry) -> Result<(), SessionError> {
        let mut data = self.data.lock().await;
        data.thread(agent)?;
        let agent = (agent != self.id).then_some(agent);
        self.record(&mut data, Record::Entry { agent, entry }).await
    }

    pub async fn rewind(&self, length: usize) -> Result<(), SessionError> {
        let mut data = self.data.lock().await;
        self.record(&mut data, Record::Rewind { length }).await
    }

    pub async fn set_actions(&self, actions: Vec<Action>) -> Result<(), SessionError> {
        let mut data = self.data.lock().await;
        self.record(&mut data, Record::Actions { actions }).await
    }

    pub async fn select(&self, selection: Selection) -> Result<(), SessionError> {
        let mut data = self.data.lock().await;
        self.record(&mut data, Record::Selection { selection })
            .await
    }

    pub async fn selection(&self) -> Selection {
        self.data.lock().await.selection.clone()
    }

    pub async fn action(&self, label: &str) -> Option<Action> {
        self.data
            .lock()
            .await
            .actions
            .iter()
            .find(|action| action.label == label)
            .cloned()
    }

    pub async fn model(&self, agent: Uuid) -> Result<Option<ModelChoice>, SessionError> {
        Ok(self.data.lock().await.thread(agent)?.summary.model.clone())
    }

    pub async fn spawn_agent(
        &self,
        parent: Uuid,
        name: String,
        model: Option<ModelChoice>,
    ) -> Result<(Uuid, CancellationToken), SessionError> {
        let mut data = self.data.lock().await;
        data.thread(parent)?;

        let agent = AgentSummary {
            id: Uuid::now_v7(),
            parent: Some(parent),
            name,
            state: AgentState::Running,
            model,
        };
        let id = agent.id;
        self.record(&mut data, Record::Agent { agent }).await?;
        Ok((id, data.thread(id)?.stop.clone()))
    }

    pub async fn finish_agent(&self, agent: Uuid, state: AgentState) -> Result<(), SessionError> {
        let mut data = self.data.lock().await;
        let agent = AgentSummary {
            state,
            ..data.thread(agent)?.summary.clone()
        };
        self.record(&mut data, Record::Agent { agent }).await
    }

    pub async fn stop_agent(&self, agent: Uuid) -> Result<(), SessionError> {
        let data = self.data.lock().await;
        let thread = data.thread(agent)?;

        if !thread.running() {
            return Err(SessionError::AgentIdle);
        }

        thread.stop.cancel();
        Ok(())
    }

    pub async fn steer(&self, agent: Uuid, prompt: Entry) -> Result<(), SessionError> {
        let mut data = self.data.lock().await;
        let thread = data.thread_mut(agent)?;

        if !thread.active {
            return Err(SessionError::AgentIdle);
        }

        thread.steering.push(prompt);
        Ok(())
    }

    pub async fn take_steering(&self, agent: Uuid) -> Result<Vec<Entry>, SessionError> {
        let mut data = self.data.lock().await;
        let thread = data.thread_mut(agent)?;
        thread.active = true;
        Ok(mem::take(&mut thread.steering))
    }

    pub async fn rest(&self, agent: Uuid) -> Result<bool, SessionError> {
        let mut data = self.data.lock().await;
        let thread = data.thread_mut(agent)?;
        thread.active = !thread.steering.is_empty();
        Ok(!thread.active)
    }

    pub async fn settle(&self, root: Uuid) {
        let mut data = self.data.lock().await;
        let agents = data.subtree(root);

        for thread in &mut data.threads {
            if !agents.contains(&thread.summary.id) {
                continue;
            }

            thread.idle();

            if thread.running() {
                thread.summary.state = AgentState::Stopped;
                self.emit(Event::Agent {
                    agent: thread.summary.clone(),
                });
            }
        }

        let asked = data.questions.len();
        data.questions
            .retain(|pending| !agents.contains(&pending.question.agent));

        if data.questions.len() != asked {
            self.emit(Event::Questions {
                questions: data.questions(),
            });
        }
    }

    pub async fn ask(
        &self,
        agent: Uuid,
        text: String,
        options: Vec<String>,
    ) -> Result<String, SessionError> {
        let (reply, answer) = oneshot::channel();

        {
            let mut data = self.data.lock().await;
            data.thread(agent)?;
            data.questions.push(Pending {
                question: Question {
                    id: Uuid::now_v7(),
                    agent,
                    text,
                    options,
                },
                reply,
            });
            self.emit(Event::Questions {
                questions: data.questions(),
            });
        }

        answer.await.map_err(|_| SessionError::NoSuchQuestion)
    }

    pub async fn answer(&self, question: Uuid, answer: String) -> Result<(), SessionError> {
        let mut data = self.data.lock().await;
        let index = data
            .questions
            .iter()
            .position(|pending| pending.question.id == question)
            .ok_or(SessionError::NoSuchQuestion)?;
        let _ = data.questions.remove(index).reply.send(answer);
        self.emit(Event::Questions {
            questions: data.questions(),
        });
        Ok(())
    }

    pub fn emit(&self, event: Event) {
        self.events.emit(event);
    }

    pub async fn subscribe(&self) -> (Event, broadcast::Receiver<Event>) {
        let data = self.data.lock().await;
        let receiver = self.events.subscribe();
        let EntryPage { start, entries } = data.threads[0].tail();
        let snapshot = Event::Snapshot {
            title: data.title.clone(),
            start,
            entries,
            actions: data.actions.clone(),
            selection: data.selection.clone(),
            environment: self.environment.status(),
            busy: self.busy().await,
            agents: data.agents(),
            questions: data.questions(),
        };
        (snapshot, receiver)
    }

    pub async fn start_turn(
        &self,
        turn: impl Future<Output = ()> + Send + 'static,
    ) -> Result<(), SessionError> {
        let mut current = self.turn.lock().await;

        if current.as_ref().is_some_and(|handle| !handle.is_finished()) {
            return Err(SessionError::Busy);
        }

        *current = Some(tokio::spawn(turn));
        Ok(())
    }

    pub async fn cancel_turn(&self) {
        let handle = self.turn.lock().await.take();

        if let Some(handle) = handle.filter(|handle| !handle.is_finished()) {
            handle.abort();
            let _ = handle.await;
            self.environment.reconcile().await;
            self.settle(self.id).await;
            self.emit(Event::TurnCancelled);
        }
    }

    async fn record(&self, data: &mut SessionData, record: Record) -> Result<(), SessionError> {
        self.environment.append(self.id, &record).await?;

        if let Some(event) = data.apply(record) {
            self.emit(event);
        }

        Ok(())
    }

    async fn busy(&self) -> bool {
        self.turn
            .lock()
            .await
            .as_ref()
            .is_some_and(|handle| !handle.is_finished())
    }
}

fn title(text: &str) -> String {
    text.lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or_default()
        .trim()
        .chars()
        .take(TITLE_LENGTH)
        .collect()
}
