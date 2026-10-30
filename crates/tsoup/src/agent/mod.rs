mod tools;

use std::sync::Arc;

use self::tools::Toolbox;
use crate::llm::{LlmClient, LlmError, Message};
use crate::session::{Entry, Event, Session, SessionError};
use crate::settings::SettingsStore;

const SYSTEM_PROMPT: &str = "You are a coding agent working in a session workspace.
The workspace is at /workspace and persists for the whole session.
read_file, write_file and edit_file work on the workspace directly and need no VM.
To run anything, call create_vm with a NixOS module that installs what you need, then use shell.
The VM is a NixOS microVM, and create_vm's kind argument picks headless or graphical.
A graphical VM runs a KWin Wayland session that the user watches live,
so graphical programs you start with shell in the background appear on their screen.
A headless VM has no display, use it for command line programs and web apps.
The user always has a terminal into the VM in their preview pane.
To show them a website served from the VM, start its server in the background with shell
and call show_website with its port.
MCP servers in the VM appear as mcp__<server>__<tool> tools while the VM runs.
The VM is ephemeral: everything outside /workspace is lost when it is destroyed or replaced.
Calling create_vm again rebuilds the VM from a new module,
so add packages that way instead of installing them imperatively.
To start a VM that is not running without rebuilding it, call run_vm.";

const DESCRIPTION: &str = match option_env!("TSOUP_DESCRIPTION") {
    Some(description) => description,
    None => "",
};

const INTERRUPTED: &str = "the tool call was interrupted before it finished";

#[derive(Debug, thiserror::Error)]
enum TurnError {
    #[error(transparent)]
    Session(#[from] SessionError),
    #[error(transparent)]
    Llm(#[from] LlmError),
    #[error("no model is configured, choose one in settings")]
    NoModel,
}

pub struct Agent {
    llm: LlmClient,
    settings: Arc<SettingsStore>,
    tools: Toolbox,
}

impl Agent {
    pub fn new(settings: Arc<SettingsStore>) -> Self {
        Self {
            llm: LlmClient::default(),
            settings,
            tools: Toolbox::new(),
        }
    }

    pub async fn models(&self) -> Result<Vec<String>, LlmError> {
        let settings = self.settings.get().await;
        self.llm.models(settings.endpoint()).await
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
        let model = settings.model.as_deref().ok_or(TurnError::NoModel)?;

        session.push(Entry::User { text }).await?;

        loop {
            let messages = conversation(&session.entries().await);
            let tools = self.tools.definitions(&session.vm.mcp_tools().await);
            let reply = self
                .llm
                .complete(settings.endpoint(), model, &messages, &tools)
                .await?;
            let calls = reply.tool_calls.clone();
            session
                .push(Entry::Assistant {
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
}

fn conversation(entries: &[Entry]) -> Vec<Message> {
    let mut messages = vec![Message::System {
        content: system_prompt(),
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
            Entry::Assistant { text, tool_calls } => {
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

fn system_prompt() -> String {
    match DESCRIPTION.trim_end() {
        "" => SYSTEM_PROMPT.to_owned(),
        description => {
            format!("{SYSTEM_PROMPT}\nThese modules are part of every VM:\n{description}")
        },
    }
}

fn interrupt(messages: &mut Vec<Message>, unanswered: &mut Vec<String>) {
    messages.extend(unanswered.drain(..).map(|tool_call_id| Message::Tool {
        tool_call_id,
        content: INTERRUPTED.to_owned(),
    }));
}
