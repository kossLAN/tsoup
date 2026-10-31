use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::VmError;

const MANIFEST: &str = "share/tsoup/previews.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preview {
    pub name: String,
    pub label: String,
    pub path: String,
    #[serde(default)]
    pub target: Option<u16>,
    #[serde(skip_serializing)]
    pub socket: PathBuf,
    #[serde(skip_serializing)]
    pub port: u16,
    #[serde(skip_serializing)]
    pub forwarded: bool,
}

impl Preview {
    pub async fn load(runner: &Path) -> Result<Vec<Self>, VmError> {
        let manifest = tokio::fs::read(runner.join(MANIFEST)).await?;
        Ok(serde_json::from_slice(&manifest)?)
    }

    pub fn visible(&self) -> bool {
        !self.forwarded || self.target.is_some()
    }
}
