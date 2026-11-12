use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use super::Events;
use crate::vm::{Builder, Vm};

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
}
