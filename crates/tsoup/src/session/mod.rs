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

pub use self::entry::{Entry, Event, FileDiff};
use crate::vm::{Builder, Vm};

const SESSION_FILE: &str = "session.json";
pub const WORKSPACE_DIR: &str = "workspace";
const EVENT_BUFFER: usize = 256;
const TITLE_LENGTH: usize = 60;

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
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct SessionData {
    title: String,
    entries: Vec<Entry>,
}

pub struct Sessions {
    root: PathBuf,
    runtime: PathBuf,
    builder: Arc<Builder>,
    by_id: RwLock<HashMap<Uuid, Arc<Session>>>,
}

impl Sessions {
    pub async fn load(
        root: PathBuf,
        runtime: PathBuf,
        builder: Arc<Builder>,
    ) -> Result<Self, SessionError> {
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

        let session = Arc::new(Session::new(
            id,
            dir,
            self.runtime.join(id.to_string()),
            SessionData::default(),
            Arc::clone(&self.builder),
        ));
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
    events: broadcast::Sender<Event>,
    turn: Mutex<Option<JoinHandle<()>>>,
}

impl Session {
    fn new(
        id: Uuid,
        dir: PathBuf,
        runtime: PathBuf,
        data: SessionData,
        builder: Arc<Builder>,
    ) -> Self {
        let (events, _) = broadcast::channel(EVENT_BUFFER);

        Self {
            id,
            vm: Vm::new(dir.clone(), runtime, builder, events.clone()),
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
    ) -> Result<Self, SessionError> {
        let data = serde_json::from_slice(&tokio::fs::read(dir.join(SESSION_FILE)).await?)?;
        Ok(Self::new(id, dir, runtime, data, builder))
    }

    pub fn workspace(&self) -> PathBuf {
        self.dir.join(WORKSPACE_DIR)
    }

    pub async fn summary(&self) -> SessionSummary {
        SessionSummary {
            id: self.id,
            title: self.data.lock().await.title.clone(),
        }
    }

    pub async fn entries(&self) -> Vec<Entry> {
        self.data.lock().await.entries.clone()
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

    pub fn emit(&self, event: Event) {
        let _ = self.events.send(event);
    }

    pub async fn subscribe(&self) -> (Event, broadcast::Receiver<Event>) {
        let data = self.data.lock().await;
        let receiver = self.events.subscribe();
        let snapshot = Event::Snapshot {
            title: data.title.clone(),
            entries: data.entries.clone(),
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
