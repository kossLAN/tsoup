use std::io::{self, SeekFrom};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncBufReadExt as _, AsyncSeekExt as _, AsyncWriteExt as _, BufReader};
use tsoup_proto::node::StoredSession;
use uuid::Uuid;

const METADATA_FILE: &str = "session.json";
const LOG_FILE: &str = "log.jsonl";
const MAX_PAGE: u64 = 8 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
struct Metadata {
    folder: String,
}

pub struct Page {
    pub records: Vec<Value>,
    pub next: Option<u64>,
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

    pub async fn load(&self, session: Uuid, offset: u64) -> io::Result<Page> {
        let mut log = File::open(self.path(session).join(LOG_FILE)).await?;
        log.seek(SeekFrom::Start(offset)).await?;
        let mut log = BufReader::new(log);

        let mut records = Vec::new();
        let mut line = String::new();
        let mut end = offset;

        let next = loop {
            line.clear();
            let length = log.read_line(&mut line).await? as u64;

            if length == 0 {
                break None;
            }

            if !records.is_empty() && end + length > offset + MAX_PAGE {
                break Some(end);
            }

            records.push(serde_json::from_str(&line)?);
            end += length;
        };

        Ok(Page { records, next })
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
