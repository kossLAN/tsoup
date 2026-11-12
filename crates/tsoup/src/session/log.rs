use std::io;
use std::path::{Path, PathBuf};

use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt as _;

use super::{Record, SessionError};

const FILE: &str = "session.jsonl";

pub struct Log {
    path: PathBuf,
}

impl Log {
    pub async fn create(dir: &Path) -> io::Result<Self> {
        let log = Self::open(dir);
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&log.path)
            .await?;
        Ok(log)
    }

    pub fn open(dir: &Path) -> Self {
        Self {
            path: dir.join(FILE),
        }
    }

    pub async fn records(&self) -> Result<Vec<Record>, SessionError> {
        Ok(tokio::fs::read_to_string(&self.path)
            .await?
            .lines()
            .map(serde_json::from_str)
            .collect::<Result<_, _>>()?)
    }

    pub async fn append(&self, record: &Record) -> Result<(), SessionError> {
        let mut line = serde_json::to_vec(record)?;
        line.push(b'\n');

        let mut log = OpenOptions::new().append(true).open(&self.path).await?;
        log.write_all(&line).await?;
        log.sync_data().await?;
        Ok(())
    }
}
