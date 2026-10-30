mod create_vm;
mod destroy_vm;
mod edit_file;
mod mcp;
mod read_file;
mod run_vm;
mod shell;
mod show_website;
mod workspace;
mod write_file;

use std::io;

use futures_util::future::BoxFuture;
use schemars::JsonSchema;
use schemars::generate::SchemaSettings;
use serde::de::DeserializeOwned;
use tsoup_proto::{McpFailure, McpTool};

use self::create_vm::CreateVm;
use self::destroy_vm::DestroyVm;
use self::edit_file::EditFile;
use self::read_file::ReadFile;
use self::run_vm::RunVm;
use self::shell::Shell;
use self::show_website::ShowWebsite;
use self::write_file::WriteFile;
use crate::llm::{FunctionCall, FunctionDefinition, ToolCall, ToolDefinition, ToolKind};
use crate::session::{FileDiff, Session};
use crate::vm::VmError;

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("there is no tool named {0}")]
    Unknown(String),
    #[error("invalid arguments: {0}")]
    Arguments(serde_json::Error),
    #[error(transparent)]
    Vm(#[from] VmError),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("the command timed out after {0} seconds")]
    TimedOut(u64),
    #[error("the guest agent failed: {0}")]
    Guest(String),
    #[error("the guest agent sent an unexpected response")]
    UnexpectedResponse,
    #[error("old does not occur in the file")]
    NoMatch,
    #[error("old occurs {0} times in the file, include more context so it is unique")]
    AmbiguousMatch(usize),
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

pub trait Tool: Send + Sync + 'static {
    const NAME: &'static str;
    const DESCRIPTION: &'static str;

    type Arguments: DeserializeOwned + JsonSchema + Send;

    fn run(
        &self,
        session: &Session,
        arguments: Self::Arguments,
    ) -> impl Future<Output = Result<Outcome, ToolError>> + Send;
}

trait ErasedTool: Send + Sync {
    fn name(&self) -> &'static str;

    fn definition(&self) -> ToolDefinition;

    fn call<'a>(
        &'a self,
        session: &'a Session,
        raw: &'a str,
    ) -> BoxFuture<'a, Result<Outcome, ToolError>>;
}

impl<T> ErasedTool for T
where
    T: Tool,
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
            kind: ToolKind::Function,
            function: FunctionDefinition {
                name: T::NAME.into(),
                description: T::DESCRIPTION.into(),
                parameters,
            },
        }
    }

    fn call<'a>(
        &'a self,
        session: &'a Session,
        raw: &'a str,
    ) -> BoxFuture<'a, Result<Outcome, ToolError>> {
        Box::pin(async move { self.run(session, arguments(raw)?).await })
    }
}

pub struct Toolbox {
    tools: Vec<Box<dyn ErasedTool>>,
    definitions: Vec<ToolDefinition>,
}

impl Toolbox {
    pub fn new() -> Self {
        let tools: Vec<Box<dyn ErasedTool>> = vec![
            Box::new(CreateVm),
            Box::new(RunVm),
            Box::new(DestroyVm),
            Box::new(Shell),
            Box::new(ShowWebsite),
            Box::new(ReadFile),
            Box::new(WriteFile),
            Box::new(EditFile),
        ];
        let definitions = tools.iter().map(|tool| tool.definition()).collect();
        Self { tools, definitions }
    }

    pub fn definitions(&self, mcp: &[McpTool]) -> Vec<ToolDefinition> {
        self.definitions
            .iter()
            .cloned()
            .chain(mcp.iter().filter_map(mcp::definition))
            .collect()
    }

    pub async fn execute(&self, session: &Session, call: &ToolCall) -> Outcome {
        let FunctionCall { name, arguments } = &call.function;

        let result = if let Some(tool) = self.tools.iter().find(|tool| tool.name() == name) {
            tool.call(session, arguments).await
        } else if let Some((server, tool)) = mcp::parse(name) {
            mcp::call(session, server, tool, arguments).await
        } else {
            Err(ToolError::Unknown(name.clone()))
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
