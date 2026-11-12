mod ask;
mod compact;
mod create_vm;
mod destroy_vm;
mod edit_file;
mod list_agents;
mod mcp;
mod read_file;
mod resume_agent;
mod run_vm;
mod set_actions;
mod shell;
mod show_preview;
mod spawn_agent;
mod wait_agents;
mod web;
mod web_fetch;
mod web_search;
mod write_file;

use std::sync::Arc;

use futures_util::future::BoxFuture;
use schemars::JsonSchema;
use schemars::generate::SchemaSettings;
use serde::de::DeserializeOwned;
use similar::TextDiff;
use tokio_util::sync::CancellationToken;
use tsoup_proto::{MAX_OUTPUT, McpFailure};
use uuid::Uuid;

use self::ask::Ask;
use self::compact::Compact;
use self::create_vm::CreateVm;
use self::destroy_vm::DestroyVm;
use self::edit_file::EditFile;
use self::list_agents::ListAgents;
use self::read_file::ReadFile;
use self::resume_agent::ResumeAgent;
use self::run_vm::RunVm;
use self::set_actions::SetActions;
use self::shell::Shell;
use self::show_preview::ShowPreview;
use self::spawn_agent::SpawnAgent;
use self::wait_agents::WaitAgents;
use self::web_fetch::WebFetch;
use self::web_search::WebSearch;
use self::write_file::WriteFile;
use super::Agent;
use super::subagents::Subagents;
use crate::llm::{FunctionCall, Image, ToolCall, ToolDefinition};
use crate::node::Folder;
use crate::session::{Environment, FileDiff, Sandbox, Session, SessionError};
use crate::vm::VmError;
use crate::workspace::{Workspace, WorkspaceError};

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("there is no tool named {0}")]
    Unknown(String),
    #[error("invalid arguments: {0}")]
    Arguments(serde_json::Error),
    #[error(transparent)]
    Vm(#[from] VmError),
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
    #[error("the command timed out after {0} seconds")]
    TimedOut(u64),
    #[error("{0} does not exist")]
    NoFile(String),
    #[error("old does not occur in the file")]
    NoMatch,
    #[error("old occurs {0} times in the file, include more context so it is unique")]
    AmbiguousMatch(usize),
    #[error("more than one action is labelled {0}")]
    DuplicateAction(String),
    #[error("{0} is not one of the models listed for spawn_agent")]
    UnknownModel(String),
    #[error("{0} is not one of your subagents")]
    NotSubagent(Uuid),
    #[error("requesting the page: {0}")]
    Http(#[from] reqwest::Error),
    #[error("{0} is not an http or https address")]
    Url(String),
    #[error("the address is not on the public internet")]
    Private,
    #[error("the page is {0}, not text")]
    NotText(String),
    #[error("the search returned no results")]
    NoResults,
    #[error("the summary is empty, write what is needed to carry on")]
    NoSummary,
    #[error("there is nothing to compact, the conversation was only just replaced by a summary")]
    NothingToCompact,
    #[error(transparent)]
    Session(#[from] SessionError),
}

pub struct Outcome {
    pub output: String,
    pub images: Vec<Image>,
    pub error: bool,
    pub diff: Option<FileDiff>,
    pub summary: Option<String>,
}

impl Outcome {
    pub fn text(output: impl Into<String>) -> Self {
        Self {
            output: output.into(),
            images: Vec::new(),
            error: false,
            diff: None,
            summary: None,
        }
    }

    fn changed(path: &str, before: &str, after: &str) -> Self {
        let unified = TextDiff::from_lines(before, after)
            .unified_diff()
            .context_radius(3)
            .header(path, path)
            .to_string();

        Self {
            diff: Some(FileDiff {
                path: path.to_owned(),
                patch: unified,
            }),
            ..Self::text(format!("wrote {path}"))
        }
    }

    fn compacted(summary: String) -> Self {
        Self {
            summary: Some(summary),
            ..Self::text("the conversation is replaced by the summary")
        }
    }

    fn failure(error: ToolError) -> Self {
        Self {
            error: true,
            ..Self::text(error.to_string())
        }
    }
}

fn head(mut content: String) -> String {
    if content.len() > MAX_OUTPUT {
        let end = (0..=MAX_OUTPUT)
            .rev()
            .find(|&index| content.is_char_boundary(index))
            .unwrap_or(0);
        content.truncate(end);
        content.push_str("\n[truncated]");
    }

    content
}

fn booted(failures: &[McpFailure]) -> Outcome {
    let mut output = String::from("the VM is running and the user sees it in their preview pane");

    for McpFailure { server, message } in failures {
        output.push_str(&format!("\nthe MCP server {server} failed: {message}"));
    }

    Outcome::text(output)
}

#[derive(Clone, Copy)]
pub struct Scope {
    pub depth: usize,
    pub limit: usize,
    pub compact: bool,
}

pub struct Caller<'a> {
    pub agent: &'a Arc<Agent>,
    pub session: &'a Arc<Session>,
    pub id: Uuid,
    pub scope: Scope,
    pub subagents: &'a Subagents,
}

impl Caller<'_> {
    async fn delegate(&self, agent: Uuid, stop: CancellationToken, prompt: String) -> Outcome {
        let depth = self.scope.depth + 1;
        self.subagents
            .start(self.agent, self.session, agent, stop, prompt, depth)
            .await;
        Outcome::text(format!(
            "agent {agent} is running, its report arrives as a message when it finishes"
        ))
    }
}

pub trait Tool<E>: Send + Sync + 'static {
    const NAME: &'static str;
    const DESCRIPTION: &'static str;
    const PARALLEL: bool = false;

    type Arguments: DeserializeOwned + JsonSchema + Send;

    fn available(_scope: Scope) -> bool {
        true
    }

    fn run(
        &self,
        caller: &Caller<'_>,
        environment: &E,
        arguments: Self::Arguments,
    ) -> impl Future<Output = Result<Outcome, ToolError>> + Send;
}

trait ErasedTool<E>: Send + Sync {
    fn name(&self) -> &'static str;

    fn parallel(&self) -> bool;

    fn available(&self, scope: Scope) -> bool;

    fn definition(&self) -> ToolDefinition;

    fn call<'a>(
        &'a self,
        caller: &'a Caller<'a>,
        environment: &'a E,
        raw: &'a str,
    ) -> BoxFuture<'a, Result<Outcome, ToolError>>;
}

impl<E, T> ErasedTool<E> for T
where
    E: Sync,
    T: Tool<E>,
{
    fn name(&self) -> &'static str {
        T::NAME
    }

    fn parallel(&self) -> bool {
        T::PARALLEL
    }

    fn available(&self, scope: Scope) -> bool {
        T::available(scope)
    }

    fn definition(&self) -> ToolDefinition {
        let parameters = SchemaSettings::draft07()
            .with(|settings| settings.meta_schema = None)
            .into_generator()
            .into_root_schema_for::<T::Arguments>();
        ToolDefinition {
            name: T::NAME.into(),
            description: T::DESCRIPTION.into(),
            parameters,
        }
    }

    fn call<'a>(
        &'a self,
        caller: &'a Caller<'a>,
        environment: &'a E,
        raw: &'a str,
    ) -> BoxFuture<'a, Result<Outcome, ToolError>> {
        Box::pin(async move { self.run(caller, environment, arguments(raw)?).await })
    }
}

struct Toolbox<E> {
    tools: Vec<Box<dyn ErasedTool<E>>>,
    definitions: Vec<ToolDefinition>,
}

impl<E: Workspace> Toolbox<E> {
    fn new(tools: Vec<Box<dyn ErasedTool<E>>>) -> Self {
        let definitions = tools.iter().map(|tool| tool.definition()).collect();
        Self { tools, definitions }
    }

    fn find(&self, name: &str) -> Option<&dyn ErasedTool<E>> {
        self.tools
            .iter()
            .find(|tool| tool.name() == name)
            .map(|tool| &**tool)
    }

    async fn definitions(&self, environment: &E, scope: Scope) -> Vec<ToolDefinition> {
        self.tools
            .iter()
            .zip(&self.definitions)
            .filter(|(tool, _)| tool.available(scope))
            .map(|(_, definition)| definition.clone())
            .chain(
                environment
                    .mcp_tools()
                    .await
                    .iter()
                    .filter_map(mcp::definition),
            )
            .collect()
    }

    async fn execute(
        &self,
        caller: &Caller<'_>,
        environment: &E,
        FunctionCall { name, arguments }: &FunctionCall,
    ) -> Result<Outcome, ToolError> {
        if let Some(tool) = self.find(name).filter(|tool| tool.available(caller.scope)) {
            tool.call(caller, environment, arguments).await
        } else if let Some((server, tool)) = mcp::parse(name) {
            mcp::call(environment, server, tool, arguments).await
        } else {
            Err(ToolError::Unknown(name.clone()))
        }
    }
}

pub struct Tools {
    sandbox: Toolbox<Sandbox>,
    node: Toolbox<Folder>,
}

impl Tools {
    pub fn new() -> Self {
        let http = web::client();

        Self {
            sandbox: Toolbox::new(vec![
                Box::new(CreateVm),
                Box::new(RunVm),
                Box::new(DestroyVm),
                Box::new(Shell),
                Box::new(ShowPreview),
                Box::new(SetActions),
                Box::new(ReadFile),
                Box::new(WriteFile),
                Box::new(EditFile),
                Box::new(SpawnAgent),
                Box::new(ResumeAgent),
                Box::new(ListAgents),
                Box::new(WaitAgents),
                Box::new(Ask),
                Box::new(Compact),
                Box::new(WebSearch(http.clone())),
                Box::new(WebFetch(http.clone())),
            ]),
            node: Toolbox::new(vec![
                Box::new(Shell),
                Box::new(ReadFile),
                Box::new(WriteFile),
                Box::new(EditFile),
                Box::new(SpawnAgent),
                Box::new(ResumeAgent),
                Box::new(ListAgents),
                Box::new(WaitAgents),
                Box::new(Ask),
                Box::new(Compact),
                Box::new(WebSearch(http.clone())),
                Box::new(WebFetch(http)),
            ]),
        }
    }

    pub async fn definitions(&self, session: &Session, scope: Scope) -> Vec<ToolDefinition> {
        match &session.environment {
            Environment::Sandbox(sandbox) => self.sandbox.definitions(sandbox, scope).await,
            Environment::Node(folder) => self.node.definitions(folder, scope).await,
        }
    }

    pub fn parallel(&self, session: &Session, call: &ToolCall) -> bool {
        let name = &call.function.name;

        match &session.environment {
            Environment::Sandbox(_) => self.sandbox.find(name).is_some_and(ErasedTool::parallel),
            Environment::Node(_) => self.node.find(name).is_some_and(ErasedTool::parallel),
        }
    }

    pub async fn execute(&self, caller: &Caller<'_>, call: &ToolCall) -> Outcome {
        let result = match &caller.session.environment {
            Environment::Sandbox(sandbox) => {
                self.sandbox.execute(caller, sandbox, &call.function).await
            },
            Environment::Node(folder) => self.node.execute(caller, folder, &call.function).await,
        };

        result.unwrap_or_else(Outcome::failure)
    }
}

fn arguments<T>(raw: &str) -> Result<T, ToolError>
where
    T: DeserializeOwned,
{
    let raw = if raw.trim().is_empty() { "{}" } else { raw };
    serde_json::from_str(raw).map_err(ToolError::Arguments)
}
