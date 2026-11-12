mod prompts;

use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt as _;
use tokio::sync::RwLock;
use tsoup_claude_code::Cli;
use uuid::Uuid;

use self::prompts::{Prompts, PromptsView};
use crate::llm::{Anthropic, ChatCompletions, ClaudeCode, Effort, Endpoint, Provider, Responses};

const DEFAULT_WINDOW: u64 = 200_000;
const MIN_WINDOW: u64 = 1_000;
const DEFAULT_DEPTH: usize = 3;
const MAX_DEPTH: usize = 5;
const DEFAULT_MEMORY: u32 = 4096;
const MIN_MEMORY: u32 = 512;
const DEFAULT_CPUS: u32 = 4;

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
    #[error("the SearXNG address has to be an http or https url")]
    SearchAddress,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub providers: Vec<ProviderSettings>,
    pub default_model: Option<ModelChoice>,
    pub last_effort: Option<Effort>,
    pub hide_thinking: bool,
    pub agent_titles: bool,
    pub prompts: Prompts,
    pub compaction: Compaction,
    pub subagents: Subagents,
    pub vm: VmSettings,
    pub search: Search,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Search {
    pub searxng: Option<String>,
}

impl Search {
    fn normalized(self) -> Result<Self, SettingsError> {
        let Some(address) = self
            .searxng
            .as_deref()
            .map(|address| address.trim().trim_end_matches('/'))
            .filter(|address| !address.is_empty())
        else {
            return Ok(Self::default());
        };

        if !Url::parse(address).is_ok_and(|url| matches!(url.scheme(), "http" | "https")) {
            return Err(SettingsError::SearchAddress);
        }

        Ok(Self {
            searxng: Some(address.to_owned()),
        })
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
pub struct Compaction {
    pub auto: bool,
    pub tool: bool,
    pub window: u64,
}

impl Default for Compaction {
    fn default() -> Self {
        Self {
            auto: true,
            tool: true,
            window: DEFAULT_WINDOW,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Subagents {
    pub enabled: bool,
    pub depth: usize,
    pub model: Option<ModelChoice>,
    pub effort: Option<Effort>,
    pub models: Vec<ModelChoice>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
pub struct VmSettings {
    pub memory: u32,
    pub cpus: u32,
    pub gpu: bool,
}

impl Default for VmSettings {
    fn default() -> Self {
        Self {
            memory: DEFAULT_MEMORY,
            cpus: DEFAULT_CPUS,
            gpu: false,
        }
    }
}

impl Default for Subagents {
    fn default() -> Self {
        Self {
            enabled: true,
            depth: DEFAULT_DEPTH,
            model: None,
            effort: None,
            models: Vec::new(),
        }
    }
}

impl Subagents {
    pub const fn limit(&self) -> usize {
        if self.enabled { self.depth } else { 0 }
    }
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

    pub fn label(&self, model: &ModelChoice) -> Option<String> {
        let provider = self.provider(model.provider)?;
        Some(format!("{}/{}", provider.name, model.name))
    }

    pub fn subagent_model(&self, label: &str) -> Option<&ModelChoice> {
        self.subagents
            .models
            .iter()
            .find(|model| self.label(model).as_deref() == Some(label))
    }

    fn prune(&mut self) {
        let providers: Vec<_> = self.providers.iter().map(|provider| provider.id).collect();
        let known = |model: &ModelChoice| providers.contains(&model.provider);

        self.default_model = self.default_model.take().filter(known);
        self.subagents.model = self.subagents.model.take().filter(known);
        let mut kept = Vec::new();
        self.subagents.models.retain(|model| {
            let fresh = known(model) && !kept.contains(model);
            kept.push(model.clone());
            fresh
        });
        self.subagents.depth = self.subagents.depth.clamp(1, MAX_DEPTH);
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
    pub agent_titles: bool,
    pub prompts: PromptsView,
    pub compaction: Compaction,
    pub subagents: Subagents,
    pub vm: VmSettings,
    pub search: Search,
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
            agent_titles: settings.agent_titles,
            prompts: settings.prompts.view(),
            compaction: settings.compaction,
            subagents: settings.subagents.clone(),
            vm: settings.vm,
            search: settings.search.clone(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SettingsPatch {
    pub providers: Option<Vec<ProviderUpdate>>,
    pub default_model: Option<ModelChoice>,
    pub hide_thinking: Option<bool>,
    pub agent_titles: Option<bool>,
    pub prompts: Option<Prompts>,
    pub compaction: Option<Compaction>,
    pub subagents: Option<Subagents>,
    pub vm: Option<VmSettings>,
    pub search: Option<Search>,
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
        }

        if let Some(model) = patch.default_model {
            updated.check(&model)?;
            updated.default_model = Some(model);
        }

        if let Some(hide) = patch.hide_thinking {
            updated.hide_thinking = hide;
        }

        if let Some(titles) = patch.agent_titles {
            updated.agent_titles = titles;
        }

        if let Some(prompts) = patch.prompts {
            updated.prompts = prompts.normalized();
        }

        if let Some(compaction) = patch.compaction {
            updated.compaction = Compaction {
                window: compaction.window.max(MIN_WINDOW),
                ..compaction
            };
        }

        if let Some(subagents) = patch.subagents {
            updated.subagents = subagents;
        }

        if let Some(vm) = patch.vm {
            updated.vm = VmSettings {
                memory: vm.memory.max(MIN_MEMORY),
                cpus: vm.cpus.max(1),
                ..vm
            };
        }

        if let Some(search) = patch.search {
            updated.search = search.normalized()?;
        }

        updated.prune();

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
