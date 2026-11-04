use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt as _;
use tokio::sync::RwLock;
use tsoup_claude_code::Cli;
use uuid::Uuid;

use crate::llm::{Anthropic, ChatCompletions, ClaudeCode, Effort, Endpoint, Provider, Responses};

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
    pub hide_thinking: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderSettings {
    pub id: Uuid,
    pub name: String,
    #[serde(flatten)]
    pub access: Access,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Access {
    ChatCompletions(Api),
    Responses(Api),
    Anthropic(Api),
    ClaudeCode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Api {
    pub url: String,
    pub key: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    ChatCompletions,
    Responses,
    Anthropic,
    ClaudeCode,
}

impl ProviderKind {
    const fn label(self) -> &'static str {
        match self {
            Self::ChatCompletions => "OpenAI compatible",
            Self::Responses => "OpenAI Responses",
            Self::Anthropic => "Anthropic",
            Self::ClaudeCode => "Claude Code",
        }
    }
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
    pub fn connect(&self, http: &Client, cli: &Arc<Cli>) -> Box<dyn Provider> {
        match &self.access {
            Access::ChatCompletions(api) => Box::new(ChatCompletions(api.endpoint(http))),
            Access::Responses(api) => Box::new(Responses(api.endpoint(http))),
            Access::Anthropic(api) => Box::new(Anthropic(api.endpoint(http))),
            Access::ClaudeCode => Box::new(ClaudeCode(Arc::clone(cli))),
        }
    }
}

impl Access {
    const fn kind(&self) -> ProviderKind {
        match self {
            Self::ChatCompletions(_) => ProviderKind::ChatCompletions,
            Self::Responses(_) => ProviderKind::Responses,
            Self::Anthropic(_) => ProviderKind::Anthropic,
            Self::ClaudeCode => ProviderKind::ClaudeCode,
        }
    }

    const fn api(&self) -> Option<&Api> {
        match self {
            Self::ChatCompletions(api) | Self::Responses(api) | Self::Anthropic(api) => Some(api),
            Self::ClaudeCode => None,
        }
    }
}

impl Api {
    fn endpoint(&self, http: &Client) -> Endpoint {
        Endpoint {
            http: http.clone(),
            url: self.url.clone(),
            key: self.key.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SettingsView {
    pub providers: Vec<ProviderView>,
    pub default_model: Option<ModelChoice>,
    pub last_effort: Option<Effort>,
    pub hide_thinking: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderView {
    pub id: Uuid,
    pub name: String,
    pub kind: ProviderKind,
    pub url: Option<String>,
    pub has_key: bool,
}

impl From<&Settings> for SettingsView {
    fn from(settings: &Settings) -> Self {
        Self {
            providers: settings
                .providers
                .iter()
                .map(|provider| {
                    let api = provider.access.api();

                    ProviderView {
                        id: provider.id,
                        name: provider.name.clone(),
                        kind: provider.access.kind(),
                        url: api.map(|api| api.url.clone()),
                        has_key: api.is_some_and(|api| api.key.is_some()),
                    }
                })
                .collect(),
            default_model: settings.default_model.clone(),
            last_effort: settings.last_effort,
            hide_thinking: settings.hide_thinking,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SettingsPatch {
    pub providers: Option<Vec<ProviderUpdate>>,
    pub default_model: Option<ModelChoice>,
    pub hide_thinking: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProviderUpdate {
    pub id: Option<Uuid>,
    pub name: String,
    pub kind: ProviderKind,
    pub url: Option<String>,
    pub key: Option<String>,
}

impl ProviderUpdate {
    fn resolve(self, saved: &Settings) -> Result<ProviderSettings, SettingsError> {
        let name = self.name.trim();
        let access = match self.kind {
            ProviderKind::ChatCompletions => Access::ChatCompletions(self.api(saved)?),
            ProviderKind::Responses => Access::Responses(self.api(saved)?),
            ProviderKind::Anthropic => Access::Anthropic(self.api(saved)?),
            ProviderKind::ClaudeCode => Access::ClaudeCode,
        };

        Ok(ProviderSettings {
            id: self.id.unwrap_or_else(Uuid::now_v7),
            name: if name.is_empty() {
                self.kind.label()
            } else {
                name
            }
            .to_owned(),
            access,
        })
    }

    fn api(&self, saved: &Settings) -> Result<Api, SettingsError> {
        let url = self
            .url
            .as_deref()
            .map(|url| url.trim().trim_end_matches('/'))
            .filter(|url| !url.is_empty())
            .ok_or(SettingsError::MissingUrl)?;
        let key = self.key.as_ref().map_or_else(
            || self.saved_key(saved),
            |key| Some(key.clone()).filter(|key| !key.is_empty()),
        );

        Ok(Api {
            url: url.to_owned(),
            key,
        })
    }

    fn saved_key(&self, saved: &Settings) -> Option<String> {
        saved.provider(self.id?)?.access.api()?.key.clone()
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

        if let Some(hide) = patch.hide_thinking {
            updated.hide_thinking = hide;
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
