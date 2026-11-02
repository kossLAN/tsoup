use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt as _;
use tsoup_proto::node::StoredSession;
use uuid::Uuid;

const METADATA_FILE: &str = "session.json";
const LOG_FILE: &str = "log.jsonl";

#[derive(Serialize, Deserialize)]
struct Metadata {
    folder: String,
}

pub struct Store {
    dir: PathBuf,
}

impl Store {
    pub async fn open(dir: PathBuf) -> io::Result<Self> {
        tokio::fs::create_dir_all(&dir).await?;
        Ok(Self { dir })
    }

    pub async fn list(&self) -> io::Result<Vec<StoredSession>> {
        let mut sessions = Vec::new();
        let mut dirs = tokio::fs::read_dir(&self.dir).await?;

        while let Some(dir) = dirs.next_entry().await? {
            let Ok(id) = dir.file_name().to_string_lossy().parse() else {
                continue;
            };

            match self.metadata(id).await {
                Ok(Metadata { folder }) => sessions.push(StoredSession { id, folder }),
                Err(error) => tracing::warn!(%id, %error, "skipping an unreadable session"),
            }
        }

        Ok(sessions)
    }

    pub async fn load(&self, session: Uuid) -> io::Result<Vec<Value>> {
        tokio::fs::read_to_string(self.path(session).join(LOG_FILE))
            .await?
            .lines()
            .map(|line| serde_json::from_str(line).map_err(io::Error::from))
            .collect()
    }

    pub async fn create(&self, session: Uuid, folder: String) -> io::Result<()> {
        let dir = self.path(session);
        tokio::fs::create_dir(&dir).await?;
        tokio::fs::write(
            dir.join(METADATA_FILE),
            serde_json::to_vec(&Metadata { folder })?,
        )
        .await?;
        tokio::fs::write(dir.join(LOG_FILE), "").await
    }

    pub async fn append(&self, session: Uuid, record: &Value) -> io::Result<()> {
        let mut line = serde_json::to_vec(record)?;
        line.push(b'\n');

        let mut log = OpenOptions::new()
            .append(true)
            .open(self.path(session).join(LOG_FILE))
            .await?;
        log.write_all(&line).await?;
        log.sync_data().await
    }

    pub async fn delete(&self, session: Uuid) -> io::Result<()> {
        tokio::fs::remove_dir_all(self.path(session)).await
    }

    async fn metadata(&self, session: Uuid) -> io::Result<Metadata> {
        let metadata = tokio::fs::read(self.path(session).join(METADATA_FILE)).await?;
        Ok(serde_json::from_slice(&metadata)?)
    }

    fn path(&self, session: Uuid) -> PathBuf {
        self.dir.join(session.to_string())
    }
}
