use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::VmError;

const MANIFEST: &str = "share/tsoup/surfaces.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Protocol {
    Http,
    Vnc,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Surface {
    pub name: String,
    pub label: String,
    pub protocol: Protocol,
    #[serde(default)]
    pub target: Option<u16>,
    #[serde(skip_serializing)]
    pub socket: PathBuf,
    #[serde(skip_serializing)]
    pub port: Option<u16>,
    #[serde(skip_serializing)]
    pub forwarded: bool,
}

impl Surface {
    pub async fn load(runner: &Path) -> Result<Vec<Self>, VmError> {
        let manifest = tokio::fs::read(runner.join(MANIFEST)).await?;
        Ok(serde_json::from_slice(&manifest)?)
    }

    pub fn visible(&self) -> bool {
        !self.forwarded || self.target.is_some()
    }
}
