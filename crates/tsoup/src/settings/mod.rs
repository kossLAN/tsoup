use std::io;
use std::path::PathBuf;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt as _;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::llm::{Effort, Endpoint, Provider, ProviderKind};

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("settings storage: {0}")]
    Io(#[from] io::Error),
    #[error("malformed settings: {0}")]
    Json(#[from] serde_json::Error),
    #[error("the model's provider does not exist")]
    UnknownProvider,
    #[error("every provider needs an endpoint url")]
    MissingUrl,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub providers: Vec<ProviderSettings>,
    pub default_model: Option<ModelChoice>,
    pub last_effort: Option<Effort>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderSettings {
    pub id: Uuid,
    pub name: String,
    pub kind: ProviderKind,
    pub url: String,
    pub key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelChoice {
    pub provider: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Selection {
    pub model: Option<ModelChoice>,
    pub effort: Option<Effort>,
}

impl Settings {
    pub fn provider(&self, id: Uuid) -> Option<&ProviderSettings> {
        self.providers.iter().find(|provider| provider.id == id)
    }

    pub fn check(&self, model: &ModelChoice) -> Result<(), SettingsError> {
        if self.provider(model.provider).is_none() {
            return Err(SettingsError::UnknownProvider);
        }

        Ok(())
    }
}

impl ProviderSettings {
    pub fn connect(&self, http: &Client) -> Box<dyn Provider> {
        self.kind.connect(Endpoint {
            http: http.clone(),
            url: self.url.clone(),
            key: self.key.clone(),
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SettingsView {
    pub providers: Vec<ProviderView>,
    pub default_model: Option<ModelChoice>,
    pub last_effort: Option<Effort>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderView {
    pub id: Uuid,
    pub name: String,
    pub kind: ProviderKind,
    pub url: String,
    pub has_key: bool,
}

impl From<&Settings> for SettingsView {
    fn from(settings: &Settings) -> Self {
        Self {
            providers: settings
                .providers
                .iter()
                .map(|provider| ProviderView {
                    id: provider.id,
                    name: provider.name.clone(),
                    kind: provider.kind,
                    url: provider.url.clone(),
                    has_key: provider.key.is_some(),
                })
                .collect(),
            default_model: settings.default_model.clone(),
            last_effort: settings.last_effort,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SettingsPatch {
    pub providers: Option<Vec<ProviderUpdate>>,
    pub default_model: Option<ModelChoice>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProviderUpdate {
    pub id: Option<Uuid>,
    pub name: String,
    pub kind: ProviderKind,
    pub url: String,
    pub key: Option<String>,
}

impl ProviderUpdate {
    fn resolve(self, saved: &Settings) -> Result<ProviderSettings, SettingsError> {
        let url = self.url.trim().trim_end_matches('/');

        if url.is_empty() {
            return Err(SettingsError::MissingUrl);
        }

        let name = self.name.trim();
        let key = match self.key {
            Some(key) => Some(key).filter(|key| !key.is_empty()),
            None => self
                .id
                .and_then(|id| saved.provider(id))
                .and_then(|provider| provider.key.clone()),
        };

        Ok(ProviderSettings {
            id: self.id.unwrap_or_else(Uuid::now_v7),
            name: if name.is_empty() {
                self.kind.label()
            } else {
                name
            }
            .to_owned(),
            kind: self.kind,
            url: url.to_owned(),
            key,
        })
    }
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

    pub async fn update(&self, patch: SettingsPatch) -> Result<SettingsView, SettingsError> {
        let mut settings = self.settings.write().await;
        let mut updated = settings.clone();

        if let Some(providers) = patch.providers {
            updated.providers = providers
                .into_iter()
                .map(|provider| provider.resolve(&settings))
                .collect::<Result<_, _>>()?;
            updated.default_model = updated.default_model.take().filter(|model| {
                updated
                    .providers
                    .iter()
                    .any(|provider| provider.id == model.provider)
            });
        }

        if let Some(model) = patch.default_model {
            updated.check(&model)?;
            updated.default_model = Some(model);
        }

        self.save(&updated).await?;

        *settings = updated;
        Ok(SettingsView::from(&*settings))
    }

    pub async fn remember_effort(&self, effort: Option<Effort>) -> Result<(), SettingsError> {
        let mut settings = self.settings.write().await;

        if settings.last_effort == effort {
            return Ok(());
        }

        let updated = Settings {
            last_effort: effort,
            ..settings.clone()
        };

        self.save(&updated).await?;

        *settings = updated;
        Ok(())
    }

    async fn save(&self, settings: &Settings) -> Result<(), SettingsError> {
        let staging = self.path.with_extension("json.tmp");
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&staging)
            .await?;
        file.write_all(&serde_json::to_vec_pretty(settings)?)
            .await?;
        file.sync_all().await?;
        tokio::fs::rename(&staging, &self.path).await?;
        Ok(())
    }
}
