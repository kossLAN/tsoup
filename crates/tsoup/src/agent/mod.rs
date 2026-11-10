mod compact;
mod conversation;
mod prompt;
mod tools;

use std::collections::HashMap;
use std::collections::hash_map::Entry as Listing;
use std::sync::Arc;

use futures_util::future::join_all;
use reqwest::Client;
use tokio::sync::{Mutex, OnceCell};
use tsoup_claude_code::Cli;
use uuid::Uuid;

use self::conversation::{context, conversation};
use self::tools::{Caller, Scope, Tools};
use crate::llm::{Effort, LlmError, Message, Model, Provider};
use crate::session::{Context, Entry, Environment, Event, Session, SessionError};
use crate::settings::{ProviderSettings, Settings, SettingsStore};
use crate::vm::describe;

const AUTO_COMPACT_PERCENT: u64 = 80;

#[derive(Debug, thiserror::Error)]
enum TurnError {
    #[error(transparent)]
    Session(#[from] SessionError),
    #[error(transparent)]
    Llm(#[from] LlmError),
    #[error("no model is selected, choose one from the model menu")]
    NoModel,
    #[error("the selected model's provider was removed, choose another model")]
    MissingProvider,
    #[error("the model returned no summary to compact the conversation with")]
    NoSummary,
    #[error("there is no reply to regenerate")]
    NoReply,
}

pub struct Agent {
    http: Client,
    cli: Arc<Cli>,
    settings: Arc<SettingsStore>,
    tools: Tools,
    modules: OnceCell<String>,
    catalog: Mutex<HashMap<Uuid, Vec<Model>>>,
}

struct Turn {
    llm: Box<dyn Provider>,
    model: String,
    effort: Option<Effort>,
    system: String,
    window: u64,
    auto: bool,
    scope: Scope,
    titles: bool,
}

impl Turn {
    const fn full(&self, context: Context) -> bool {
        self.auto && context.used * 100 >= self.window * AUTO_COMPACT_PERCENT
    }
}

impl Agent {
    pub fn new(settings: Arc<SettingsStore>, cli: Arc<Cli>) -> Self {
        Self {
            http: Client::default(),
            cli,
            settings,
            tools: Tools::new(),
            modules: OnceCell::new(),
            catalog: Mutex::default(),
        }
    }

    pub async fn models(&self, provider: &ProviderSettings) -> Result<Vec<String>, LlmError> {
        let models = provider.connect(&self.http, &self.cli).models().await?;
        let names = models.iter().map(|model| model.name.clone()).collect();
        self.catalog.lock().await.insert(provider.id, models);
        Ok(names)
    }

    pub async fn run_turn(self: Arc<Self>, session: Arc<Session>, prompt: Entry) {
        let turn = async {
            let turn = self.prepare(&session, session.id, 0).await?;

            if turn.titles
                && !session.titled().await
                && let Entry::User { text, .. } = &prompt
                && !text.is_empty()
            {
                tokio::spawn(Arc::clone(&self).title(Arc::clone(&session), text.clone()));
            }

            session.push(session.id, prompt).await?;
            self.drive(&session, session.id, &turn).await?;
            Ok(())
        };
        supervise(&session, turn).await;
    }

    async fn title(self: Arc<Self>, session: Arc<Session>, text: String) {
        let titled = async {
            let turn = self.prepare(&session, session.id, 0).await?;
            let messages = [
                Message::System {
                    content: prompt::TITLE.to_owned(),
                },
                Message::user(text),
            ];
            let reply = turn
                .llm
                .complete(&turn.model, None, &messages, &[], &|_| {})
                .await?;

            if let Some(title) = reply.content {
                session.rename(title.trim().trim_matches('"')).await?;
            }

            Ok::<_, TurnError>(())
        };

        if let Err(error) = titled.await {
            tracing::warn!(session = %session.id, %error, "titling a session");
        }
    }

    pub async fn run_regeneration(self: Arc<Self>, session: Arc<Session>) {
        let regeneration = async {
            let turn = self.prepare(&session, session.id, 0).await?;
            let entries = session.entries(session.id).await?;

            let Some(Entry::Assistant { tool_calls, .. }) = entries.last() else {
                return Err(TurnError::NoReply);
            };

            if !tool_calls.is_empty() {
                return Err(TurnError::NoReply);
            }

            session.rewind(entries.len() - 1).await?;
            self.drive(&session, session.id, &turn).await?;
            Ok(())
        };
        supervise(&session, regeneration).await;
    }

    pub async fn run_compaction(self: Arc<Self>, session: Arc<Session>) {
        let compaction = async {
            let turn = self.prepare(&session, session.id, 0).await?;
            self.compact(&session, session.id, &turn).await
        };
        supervise(&session, compaction).await;
    }

    async fn run(
        &self,
        session: &Session,
        id: Uuid,
        depth: usize,
    ) -> Result<Option<String>, TurnError> {
        let turn = self.prepare(session, id, depth).await?;
        self.drive(session, id, &turn).await
    }

    async fn prepare(&self, session: &Session, id: Uuid, depth: usize) -> Result<Turn, TurnError> {
        let settings = self.settings.get().await;
        let selection = session.selection().await;
        let scope = Scope {
            depth,
            limit: settings.subagents.limit(),
        };
        let effort = match settings.subagents.effort {
            Some(effort) if depth > 0 => Some(effort),
            _ => selection.effort,
        };
        let model = session
            .model(id)
            .await?
            .or(selection.model)
            .or_else(|| settings.default_model.clone())
            .ok_or(TurnError::NoModel)?;
        let provider = settings
            .provider(model.provider)
            .ok_or(TurnError::MissingProvider)?;
        let llm = provider.connect(&self.http, &self.cli);
        let window = self
            .window(provider.id, &*llm, &model.name)
            .await
            .unwrap_or(settings.compaction.window);
        let system = self.system(&settings, &session.environment, scope).await;

        Ok(Turn {
            llm,
            model: model.name,
            effort,
            system,
            window,
            auto: settings.compaction.auto,
            scope,
            titles: settings.agent_titles,
        })
    }

    async fn drive(
        &self,
        session: &Session,
        id: Uuid,
        turn: &Turn,
    ) -> Result<Option<String>, TurnError> {
        let caller = Caller {
            agent: self,
            session,
            id,
            scope: turn.scope,
        };
        let mut recovered = false;

        loop {
            for prompt in session.take_steering(id).await? {
                session.push(id, prompt).await?;
            }

            let entries = session.entries(id).await?;

            if context(&entries).is_some_and(|context| turn.full(context)) {
                self.compact(session, id, turn).await?;
                continue;
            }

            let messages = conversation(&turn.system, &entries);
            let tools = self.tools.definitions(session, turn.scope).await;
            let report = |progress| {
                session.emit(Event::Progress {
                    agent: id,
                    progress,
                });
            };
            let reply = match turn
                .llm
                .complete(&turn.model, turn.effort, &messages, &tools, &report)
                .await
            {
                Err(LlmError::Overflow) if !recovered => {
                    recovered = true;
                    self.compact(session, id, turn).await?;
                    continue;
                },
                reply => reply?,
            };
            recovered = false;

            let calls = reply.tool_calls.clone();
            let text = reply.content.clone();
            session
                .push(
                    id,
                    Entry::Assistant {
                        thinking: reply.thinking,
                        text: reply.content,
                        tool_calls: reply.tool_calls,
                        context: reply.usage.map(|usage| Context {
                            used: usage.input + usage.output,
                            window: turn.window,
                        }),
                    },
                )
                .await?;

            if calls.is_empty() {
                if session.rest(id).await? {
                    return Ok(text);
                }

                continue;
            }

            let batches = calls.chunk_by(|left, right| {
                self.tools.parallel(session, left) && self.tools.parallel(session, right)
            });

            for batch in batches {
                let outcomes =
                    join_all(batch.iter().map(|call| self.tools.execute(&caller, call))).await;

                for (call, outcome) in batch.iter().zip(outcomes) {
                    session
                        .push(
                            id,
                            Entry::Tool {
                                call_id: call.id.clone(),
                                output: outcome.output,
                                error: outcome.error,
                                diff: outcome.diff,
                            },
                        )
                        .await?;
                }
            }
        }
    }

    async fn window(&self, provider: Uuid, llm: &dyn Provider, model: &str) -> Option<u64> {
        let mut catalog = self.catalog.lock().await;

        let models = match catalog.entry(provider) {
            Listing::Occupied(listing) => listing.into_mut(),
            Listing::Vacant(listing) => {
                listing.insert(llm.models().await.unwrap_or_else(|error| {
                    tracing::warn!(%error, "listing models for their context windows");
                    Vec::new()
                }))
            },
        };

        models.iter().find(|listed| listed.name == model)?.context
    }

    async fn system(&self, settings: &Settings, environment: &Environment, scope: Scope) -> String {
        let prompts = &settings.prompts;
        let mut system = match environment {
            Environment::Sandbox(_) => prompt::sandbox(prompts.sandbox(), self.modules().await),
            Environment::Node(folder) => prompt::node(prompts.node(), folder),
        };

        if scope.depth > 0 {
            system.push_str("\n\n");
            system.push_str(prompts.subagent());
        }

        let models: Vec<_> = settings
            .subagents
            .models
            .iter()
            .filter_map(|model| settings.label(model))
            .collect();

        if scope.depth < scope.limit && !models.is_empty() {
            system.push_str("\n\n");
            system.push_str(&prompt::models(&models));
        }

        system
    }

    async fn modules(&self) -> &str {
        match self.modules.get_or_try_init(describe).await {
            Ok(modules) => modules.trim_end(),
            Err(error) => {
                tracing::warn!(%error, "listing the VM modules");
                ""
            },
        }
    }
}

async fn supervise(session: &Session, turn: impl Future<Output = Result<(), TurnError>>) {
    session.emit(Event::TurnStarted);

    match turn.await {
        Ok(()) => session.emit(Event::TurnFinished),
        Err(error) => {
            tracing::warn!(session = %session.id, %error, "a turn failed");
            session.settle(session.id).await;
            session.emit(Event::TurnFailed {
                message: error.to_string(),
            });
        },
    }
}
