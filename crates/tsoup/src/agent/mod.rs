mod tools;

use std::sync::Arc;

use reqwest::Client;
use tokio::sync::OnceCell;
use tsoup_claude_code::Cli;

use self::tools::Tools;
use crate::llm::{LlmError, Message};
use crate::node::Folder;
use crate::session::{Entry, Environment, Event, Session, SessionError};
use crate::settings::{ProviderSettings, SettingsStore};
use crate::vm::describe;

const SANDBOX_PROMPT: &str = "You are a coding agent working in a session workspace.
The workspace is at /workspace, the working directory of every tool, and persists for the whole session.
read_file, write_file and edit_file work on the workspace directly and need no VM.
To run anything, call create_vm with a NixOS module that installs what you need, then use shell.
The VM is a headless NixOS microVM, and the user always has a terminal into it in their preview pane.
create_vm's modules argument adds optional modules, such as a desktop or a website preview,
pick only the ones the task needs.
Once the app works, call set_actions with a Run button so the user can start it again themselves.
MCP servers in the VM appear as mcp__<server>__<tool> tools while the VM runs.
The VM is ephemeral: everything outside /workspace is lost when it is destroyed or replaced.
Calling create_vm again rebuilds the VM from a new module,
so add packages that way instead of installing them imperatively.
To start a VM that is not running without rebuilding it, call run_vm.";

const INTERRUPTED: &str = "the tool call was interrupted before it finished";

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
}

pub struct Agent {
    http: Client,
    cli: Arc<Cli>,
    settings: Arc<SettingsStore>,
    tools: Tools,
    modules: OnceCell<String>,
}

impl Agent {
    pub fn new(settings: Arc<SettingsStore>, cli: Arc<Cli>) -> Self {
        Self {
            http: Client::default(),
            cli,
            settings,
            tools: Tools::new(),
            modules: OnceCell::new(),
        }
    }

    pub async fn models(&self, provider: &ProviderSettings) -> Result<Vec<String>, LlmError> {
        provider.connect(&self.http, &self.cli).models().await
    }

    pub async fn run_turn(self: Arc<Self>, session: Arc<Session>, text: String) {
        session.emit(Event::TurnStarted);

        match self.turn(&session, text).await {
            Ok(()) => session.emit(Event::TurnFinished),
            Err(error) => {
                tracing::warn!(session = %session.id, %error, "a turn failed");
                session.emit(Event::TurnFailed {
                    message: error.to_string(),
                });
            },
        }
    }

    async fn turn(&self, session: &Session, text: String) -> Result<(), TurnError> {
        let settings = self.settings.get().await;
        let selection = session.selection().await;
        let model = selection
            .model
            .or_else(|| settings.default_model.clone())
            .ok_or(TurnError::NoModel)?;
        let llm = settings
            .provider(model.provider)
            .ok_or(TurnError::MissingProvider)?
            .connect(&self.http, &self.cli);

        session.push(Entry::User { text }).await?;
        let system = self.system_prompt(&session.environment).await;

        loop {
            let messages = conversation(&system, &session.entries().await);
            let tools = self.tools.definitions(session).await;
            let report = |progress| session.emit(Event::Progress { progress });
            let reply = llm
                .complete(&model.name, selection.effort, &messages, &tools, &report)
                .await?;
            let calls = reply.tool_calls.clone();
            session
                .push(Entry::Assistant {
                    thinking: reply.thinking,
                    text: reply.content,
                    tool_calls: reply.tool_calls,
                })
                .await?;

            if calls.is_empty() {
                return Ok(());
            }

            for call in calls {
                let outcome = self.tools.execute(session, &call).await;
                session
                    .push(Entry::Tool {
                        call_id: call.id,
                        output: outcome.output,
                        error: outcome.error,
                        diff: outcome.diff,
                    })
                    .await?;
            }
        }
    }

    async fn system_prompt(&self, environment: &Environment) -> String {
        match environment {
            Environment::Sandbox(_) => self.sandbox_prompt().await,
            Environment::Node(folder) => node_prompt(folder),
        }
    }

    async fn sandbox_prompt(&self) -> String {
        match self
            .modules
            .get_or_try_init(describe)
            .await
            .map(|modules| modules.trim_end())
        {
            Ok("") => SANDBOX_PROMPT.to_owned(),
            Ok(modules) => {
                format!(
                    "{SANDBOX_PROMPT}\nThe modules for create_vm's modules argument:\n{modules}"
                )
            },
            Err(error) => {
                tracing::warn!(%error, "listing the VM modules");
                SANDBOX_PROMPT.to_owned()
            },
        }
    }
}

fn node_prompt(folder: &Folder) -> String {
    format!(
        "You are a coding agent working directly on the machine {name}, outside any sandbox.
The working directory is {directory}, shell runs commands there with sh -c.
read_file, write_file and edit_file resolve relative paths against it and accept absolute paths.
MCP servers on the machine appear as mcp__<server>__<tool> tools.
Everything you do happens on the real machine and persists,
so ask the user before doing anything destructive or hard to undo.",
        name = folder.node.name,
        directory = folder.path().display(),
    )
}

fn conversation(system: &str, entries: &[Entry]) -> Vec<Message> {
    let mut messages = vec![Message::System {
        content: system.to_owned(),
    }];
    let mut unanswered = Vec::new();

    for entry in entries {
        match entry {
            Entry::User { text } => {
                interrupt(&mut messages, &mut unanswered);
                messages.push(Message::User {
                    content: text.clone(),
                });
            },
            Entry::Assistant {
                text, tool_calls, ..
            } => {
                interrupt(&mut messages, &mut unanswered);
                unanswered.extend(tool_calls.iter().map(|call| call.id.clone()));
                messages.push(Message::Assistant {
                    content: text.clone(),
                    tool_calls: tool_calls.clone(),
                });
            },
            Entry::Tool {
                call_id, output, ..
            } => {
                unanswered.retain(|id| id != call_id);
                messages.push(Message::Tool {
                    tool_call_id: call_id.clone(),
                    content: output.clone(),
                });
            },
        }
    }

    interrupt(&mut messages, &mut unanswered);
    messages
}

fn interrupt(messages: &mut Vec<Message>, unanswered: &mut Vec<String>) {
    messages.extend(unanswered.drain(..).map(|tool_call_id| Message::Tool {
        tool_call_id,
        content: INTERRUPTED.to_owned(),
    }));
}
