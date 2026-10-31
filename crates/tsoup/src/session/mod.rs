mod entry;

use std::cmp::Reverse;
use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, RwLock, broadcast};
use tokio::task::JoinHandle;
use uuid::Uuid;

pub use self::entry::{Action, Entry, Event, FileDiff};
use crate::vm::{Builder, Vm, VmStatus};

const SESSION_FILE: &str = "session.json";
pub const WORKSPACE_DIR: &str = "workspace";
const EVENT_BUFFER: usize = 256;
const TITLE_LENGTH: usize = 60;
const PAGE_SIZE: usize = 50;

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("no such session")]
    NotFound,
    #[error("a turn is already running")]
    Busy,
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
    pub vm: VmStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ListEvent {
    Snapshot { sessions: Vec<SessionSummary> },
    Busy { id: Uuid, busy: bool },
    Vm { id: Uuid, status: VmStatus },
}

#[derive(Clone)]
pub struct Events {
    id: Uuid,
    session: broadcast::Sender<Event>,
    list: broadcast::Sender<ListEvent>,
}

impl Events {
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
            Event::Vm { status } => Some(ListEvent::Vm {
                id: self.id,
                status: status.clone(),
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

#[derive(Debug, Clone, Serialize)]
pub struct EntryPage {
    pub start: usize,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct SessionData {
    title: String,
    entries: Vec<Entry>,
    #[serde(default)]
    actions: Vec<Action>,
}

pub struct Sessions {
    root: PathBuf,
    runtime: PathBuf,
    builder: Arc<Builder>,
    by_id: RwLock<HashMap<Uuid, Arc<Session>>>,
    changes: broadcast::Sender<ListEvent>,
}

impl Sessions {
    pub async fn load(
        root: PathBuf,
        runtime: PathBuf,
        builder: Arc<Builder>,
    ) -> Result<Self, SessionError> {
        let (changes, _) = broadcast::channel(EVENT_BUFFER);
        let mut sessions = HashMap::new();
        let mut dirs = tokio::fs::read_dir(&root).await?;

        while let Some(dir) = dirs.next_entry().await? {
            let Ok(id) = dir.file_name().to_string_lossy().parse::<Uuid>() else {
                continue;
            };

            match Session::load(
                id,
                dir.path(),
                runtime.join(id.to_string()),
                Arc::clone(&builder),
                changes.clone(),
            )
            .await
            {
                Ok(session) => {
                    sessions.insert(id, Arc::new(session));
                },
                Err(error) => tracing::warn!(%id, %error, "skipping an unreadable session"),
            }
        }

        Ok(Self {
            root,
            runtime,
            builder,
            by_id: RwLock::new(sessions),
            changes,
        })
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

    pub async fn create(&self) -> Result<Arc<Session>, SessionError> {
        let id = Uuid::now_v7();
        let dir = self.root.join(id.to_string());
        tokio::fs::create_dir_all(dir.join(WORKSPACE_DIR)).await?;

        let session = Arc::new(
            Session::new(
                id,
                dir,
                self.runtime.join(id.to_string()),
                SessionData::default(),
                Arc::clone(&self.builder),
                self.changes.clone(),
            )
            .await,
        );
        session.persist(&*session.data.lock().await).await?;
        self.by_id.write().await.insert(id, Arc::clone(&session));
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
        session.vm.destroy().await;
        tokio::fs::remove_dir_all(&session.dir).await?;
        Ok(())
    }
}

pub struct Session {
    pub id: Uuid,
    pub vm: Vm,
    dir: PathBuf,
    data: Mutex<SessionData>,
    events: Events,
    turn: Mutex<Option<JoinHandle<()>>>,
}

impl Session {
    async fn new(
        id: Uuid,
        dir: PathBuf,
        runtime: PathBuf,
        data: SessionData,
        builder: Arc<Builder>,
        changes: broadcast::Sender<ListEvent>,
    ) -> Self {
        let events = Events {
            id,
            session: broadcast::channel(EVENT_BUFFER).0,
            list: changes,
        };

        Self {
            id,
            vm: Vm::new(dir.clone(), runtime, builder, events.clone()).await,
            dir,
            data: Mutex::new(data),
            events,
            turn: Mutex::new(None),
        }
    }

    async fn load(
        id: Uuid,
        dir: PathBuf,
        runtime: PathBuf,
        builder: Arc<Builder>,
        changes: broadcast::Sender<ListEvent>,
    ) -> Result<Self, SessionError> {
        let data = serde_json::from_slice(&tokio::fs::read(dir.join(SESSION_FILE)).await?)?;
        Ok(Self::new(id, dir, runtime, data, builder, changes).await)
    }

    pub fn workspace(&self) -> PathBuf {
        self.dir.join(WORKSPACE_DIR)
    }

    pub async fn summary(&self) -> SessionSummary {
        SessionSummary {
            id: self.id,
            title: self.data.lock().await.title.clone(),
            busy: self.busy().await,
            vm: self.vm.status(),
        }
    }

    pub async fn entries(&self) -> Vec<Entry> {
        self.data.lock().await.entries.clone()
    }

    pub async fn page(&self, before: usize) -> EntryPage {
        let data = self.data.lock().await;
        let end = before.min(data.entries.len());
        let start = end.saturating_sub(PAGE_SIZE);
        EntryPage {
            start,
            entries: data.entries[start..end].to_vec(),
        }
    }

    pub async fn push(&self, entry: Entry) -> Result<(), SessionError> {
        let mut data = self.data.lock().await;

        if let Entry::User { text } = &entry
            && data.title.is_empty()
        {
            data.title = title(text);
        }

        data.entries.push(entry.clone());
        self.persist(&data).await?;
        self.emit(Event::Entry { entry });
        Ok(())
    }

    pub async fn set_actions(&self, actions: Vec<Action>) -> Result<(), SessionError> {
        let mut data = self.data.lock().await;
        data.actions.clone_from(&actions);
        self.persist(&data).await?;
        self.emit(Event::Actions { actions });
        Ok(())
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

    pub fn emit(&self, event: Event) {
        self.events.emit(event);
    }

    pub async fn subscribe(&self) -> (Event, broadcast::Receiver<Event>) {
        let data = self.data.lock().await;
        let receiver = self.events.subscribe();
        let start = data.entries.len().saturating_sub(PAGE_SIZE);
        let snapshot = Event::Snapshot {
            title: data.title.clone(),
            start,
            entries: data.entries[start..].to_vec(),
            actions: data.actions.clone(),
            vm: self.vm.status(),
            busy: self.busy().await,
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
            self.vm.reconcile().await;
            self.emit(Event::TurnCancelled);
        }
    }

    async fn busy(&self) -> bool {
        self.turn
            .lock()
            .await
            .as_ref()
            .is_some_and(|handle| !handle.is_finished())
    }

    async fn persist(&self, data: &SessionData) -> Result<(), SessionError> {
        let path = self.dir.join(SESSION_FILE);
        let staging = path.with_extension("json.tmp");
        tokio::fs::write(&staging, serde_json::to_vec(data)?).await?;
        tokio::fs::rename(&staging, &path).await?;
        Ok(())
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
