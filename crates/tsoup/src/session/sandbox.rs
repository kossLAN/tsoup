use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt as _;

use super::{Events, Record, SessionError};
use crate::vm::{Builder, Vm};

const LOG_FILE: &str = "session.jsonl";
pub const WORKSPACE_DIR: &str = "workspace";

pub struct Sandbox {
    dir: PathBuf,
    pub vm: Vm,
}

impl Sandbox {
    pub async fn create(
        dir: PathBuf,
        runtime: PathBuf,
        builder: Arc<Builder>,
        events: Events,
    ) -> io::Result<Self> {
        tokio::fs::create_dir_all(dir.join(WORKSPACE_DIR)).await?;
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(dir.join(LOG_FILE))
            .await?;
        Ok(Self::open(dir, runtime, builder, events).await)
    }

    pub async fn open(
        dir: PathBuf,
        runtime: PathBuf,
        builder: Arc<Builder>,
        events: Events,
    ) -> Self {
        Self {
            vm: Vm::new(dir.clone(), runtime, builder, events).await,
            dir,
        }
    }

    pub fn workspace(&self) -> PathBuf {
        self.dir.join(WORKSPACE_DIR)
    }

    pub async fn records(&self) -> Result<Vec<Record>, SessionError> {
        Ok(tokio::fs::read_to_string(self.dir.join(LOG_FILE))
            .await?
            .lines()
            .map(serde_json::from_str)
            .collect::<Result<_, _>>()?)
    }

    pub async fn append(&self, record: &Record) -> Result<(), SessionError> {
        let mut line = serde_json::to_vec(record)?;
        line.push(b'\n');

        let mut log = OpenOptions::new()
            .append(true)
            .open(self.dir.join(LOG_FILE))
            .await?;
        log.write_all(&line).await?;
        log.sync_data().await?;
        Ok(())
    }

    pub async fn destroy(&self) -> io::Result<()> {
        self.vm.destroy().await;
        tokio::fs::remove_dir_all(&self.dir).await
    }
}
