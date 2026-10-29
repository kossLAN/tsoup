use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt as _;
use tokio::sync::RwLock;

use crate::llm::Endpoint;

const DEFAULT_LLM_URL: &str = "https://api.openai.com/v1";

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("settings storage: {0}")]
    Io(#[from] io::Error),
    #[error("malformed settings: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub llm_url: String,
    pub llm_key: Option<String>,
    pub model: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            llm_url: DEFAULT_LLM_URL.to_owned(),
            llm_key: None,
            model: None,
        }
    }
}

impl Settings {
    pub fn endpoint(&self) -> Endpoint<'_> {
        Endpoint {
            url: &self.llm_url,
            key: self.llm_key.as_deref(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SettingsView {
    pub llm_url: String,
    pub has_llm_key: bool,
    pub model: Option<String>,
}

impl From<&Settings> for SettingsView {
    fn from(settings: &Settings) -> Self {
        Self {
            llm_url: settings.llm_url.clone(),
            has_llm_key: settings.llm_key.is_some(),
            model: settings.model.clone(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SettingsUpdate {
    pub llm_url: String,
    pub llm_key: Option<String>,
    pub model: Option<String>,
}

pub struct SettingsStore {
    path: PathBuf,
    settings: RwLock<Settings>,
}

impl SettingsStore {
    pub async fn load(path: PathBuf) -> Result<Self, SettingsError> {
        let settings = match tokio::fs::read(&path).await {
            Ok(contents) => serde_json::from_slice(&contents)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => Settings::default(),
            Err(error) => return Err(error.into()),
        };

        Ok(Self {
            path,
            settings: RwLock::new(settings),
        })
    }

    pub async fn get(&self) -> Settings {
        self.settings.read().await.clone()
    }

    pub async fn view(&self) -> SettingsView {
        SettingsView::from(&*self.settings.read().await)
    }

    pub async fn update(&self, update: SettingsUpdate) -> Result<SettingsView, SettingsError> {
        let mut settings = self.settings.write().await;
        let mut updated = Settings {
            llm_url: update.llm_url.trim().trim_end_matches('/').to_owned(),
            llm_key: settings.llm_key.clone(),
            model: update.model.filter(|model| !model.trim().is_empty()),
        };

        if let Some(key) = update.llm_key {
            updated.llm_key = Some(key).filter(|key| !key.is_empty());
        }

        let staging = self.path.with_extension("json.tmp");
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&staging)
            .await?;
        file.write_all(&serde_json::to_vec_pretty(&updated)?)
            .await?;
        file.sync_all().await?;
        tokio::fs::rename(&staging, &self.path).await?;

        *settings = updated;
        Ok(SettingsView::from(&*settings))
    }
}
