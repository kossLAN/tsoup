mod create_vm;
mod destroy_vm;
mod edit_file;
mod mcp;
mod read_file;
mod run_vm;
mod set_actions;
mod shell;
mod show_preview;
mod write_file;

use futures_util::future::BoxFuture;
use schemars::JsonSchema;
use schemars::generate::SchemaSettings;
use serde::de::DeserializeOwned;
use similar::TextDiff;
use tsoup_proto::McpFailure;

use self::create_vm::CreateVm;
use self::destroy_vm::DestroyVm;
use self::edit_file::EditFile;
use self::read_file::ReadFile;
use self::run_vm::RunVm;
use self::set_actions::SetActions;
use self::shell::Shell;
use self::show_preview::ShowPreview;
use self::write_file::WriteFile;
use crate::llm::{FunctionCall, ToolCall, ToolDefinition};
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
    #[error(transparent)]
    Session(#[from] SessionError),
}

pub struct Outcome {
    pub output: String,
    pub error: bool,
    pub diff: Option<FileDiff>,
}

impl Outcome {
    pub fn text(output: impl Into<String>) -> Self {
        Self {
            output: output.into(),
            error: false,
            diff: None,
        }
    }

    fn changed(path: &str, before: &str, after: &str) -> Self {
        let unified = TextDiff::from_lines(before, after)
            .unified_diff()
            .context_radius(3)
            .header(path, path)
            .to_string();

        Self {
            output: format!("wrote {path}"),
            error: false,
            diff: Some(FileDiff {
                path: path.to_owned(),
                patch: unified,
            }),
        }
    }

    fn failure(error: ToolError) -> Self {
        Self {
            output: error.to_string(),
            error: true,
            diff: None,
        }
    }
}

fn booted(failures: &[McpFailure]) -> Outcome {
    let mut output = String::from("the VM is running and the user sees it in their preview pane");

    for McpFailure { server, message } in failures {
        output.push_str(&format!("\nthe MCP server {server} failed: {message}"));
    }

    Outcome::text(output)
}

pub trait Tool<E>: Send + Sync + 'static {
    const NAME: &'static str;
    const DESCRIPTION: &'static str;

    type Arguments: DeserializeOwned + JsonSchema + Send;

    fn run(
        &self,
        session: &Session,
        environment: &E,
        arguments: Self::Arguments,
    ) -> impl Future<Output = Result<Outcome, ToolError>> + Send;
}

trait ErasedTool<E>: Send + Sync {
    fn name(&self) -> &'static str;

    fn definition(&self) -> ToolDefinition;

    fn call<'a>(
        &'a self,
        session: &'a Session,
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
        session: &'a Session,
        environment: &'a E,
        raw: &'a str,
    ) -> BoxFuture<'a, Result<Outcome, ToolError>> {
        Box::pin(async move { self.run(session, environment, arguments(raw)?).await })
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

    async fn definitions(&self, environment: &E) -> Vec<ToolDefinition> {
        self.definitions
            .iter()
            .cloned()
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
        session: &Session,
        environment: &E,
        FunctionCall { name, arguments }: &FunctionCall,
    ) -> Result<Outcome, ToolError> {
        if let Some(tool) = self.tools.iter().find(|tool| tool.name() == name) {
            tool.call(session, environment, arguments).await
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
            ]),
            node: Toolbox::new(vec![
                Box::new(Shell),
                Box::new(ReadFile),
                Box::new(WriteFile),
                Box::new(EditFile),
            ]),
        }
    }

    pub async fn definitions(&self, session: &Session) -> Vec<ToolDefinition> {
        match &session.environment {
            Environment::Sandbox(sandbox) => self.sandbox.definitions(sandbox).await,
            Environment::Node(folder) => self.node.definitions(folder).await,
        }
    }

    pub async fn execute(&self, session: &Session, call: &ToolCall) -> Outcome {
        let result = match &session.environment {
            Environment::Sandbox(sandbox) => {
                self.sandbox.execute(session, sandbox, &call.function).await
            },
            Environment::Node(folder) => self.node.execute(session, folder, &call.function).await,
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
