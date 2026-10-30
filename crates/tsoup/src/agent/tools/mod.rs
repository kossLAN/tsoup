mod create_vm;
mod destroy_vm;
mod edit_file;
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

use self::create_vm::CreateVm;
use self::destroy_vm::DestroyVm;
use self::edit_file::EditFile;
use self::read_file::ReadFile;
use self::run_vm::RunVm;
use self::shell::Shell;
use self::show_website::ShowWebsite;
use self::write_file::WriteFile;
use crate::llm::{FunctionDefinition, ToolCall, ToolDefinition, ToolKind};
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
        arguments: &'a str,
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
                name: T::NAME,
                description: T::DESCRIPTION,
                parameters,
            },
        }
    }

    fn call<'a>(
        &'a self,
        session: &'a Session,
        arguments: &'a str,
    ) -> BoxFuture<'a, Result<Outcome, ToolError>> {
        Box::pin(async move {
            let arguments = if arguments.trim().is_empty() {
                "{}"
            } else {
                arguments
            };

            let arguments = serde_json::from_str(arguments).map_err(ToolError::Arguments)?;
            self.run(session, arguments).await
        })
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

    pub fn definitions(&self) -> &[ToolDefinition] {
        &self.definitions
    }

    pub async fn execute(&self, session: &Session, call: &ToolCall) -> Outcome {
        let result = match self
            .tools
            .iter()
            .find(|tool| tool.name() == call.function.name)
        {
            Some(tool) => tool.call(session, &call.function.arguments).await,
            None => Err(ToolError::Unknown(call.function.name.clone())),
        };

        result.unwrap_or_else(Outcome::failure)
    }
}
