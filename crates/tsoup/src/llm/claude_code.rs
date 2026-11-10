use std::iter;
use std::sync::Arc;

use futures_util::future::BoxFuture;
use serde::Serialize;
use tsoup_claude_code::{
    ClaudeCodeError, Cli, ControlReply, ControlRequest, ControlResponse, McpTool, Output,
    SdkServer, UserInput,
};

use super::anthropic::{ImageSource, StreamEvent};
use super::stream::Collector;
use super::{
    Effort, Image, LlmError, Message, Model, OnProgress, Provider, Reply, ToolDefinition,
    overflows, system,
};

const SERVER: &str = "tsoup";
const INTRO: &str =
    "The conversation so far follows as tagged turns, continue it as the assistant.";

pub struct ClaudeCode(pub Arc<Cli>);

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Block<'a> {
    Text { text: String },
    Image { source: ImageSource<'a> },
}

fn tagged(message: &Message) -> Option<String> {
    match message {
        Message::System { .. } => None,
        Message::User { content, .. } => Some(format!("<user>\n{content}\n</user>")),
        Message::Assistant {
            content,
            tool_calls,
        } => {
            let turn: Vec<_> = content
                .iter()
                .map(|content| format!("<assistant>\n{content}\n</assistant>"))
                .chain(tool_calls.iter().map(|call| {
                    format!(
                        "<tool_call id=\"{}\" name=\"{}\">\n{}\n</tool_call>",
                        call.id, call.function.name, call.function.arguments
                    )
                }))
                .collect();
            (!turn.is_empty()).then(|| turn.join("\n"))
        },
        Message::Tool {
            tool_call_id,
            content,
        } => Some(format!(
            "<tool_result id=\"{tool_call_id}\">\n{content}\n</tool_result>"
        )),
    }
}

fn failed(message: String) -> LlmError {
    if overflows(&message) {
        LlmError::Overflow
    } else {
        ClaudeCodeError::Failed(message).into()
    }
}

fn images(message: &Message) -> &[Image] {
    match message {
        Message::User { images, .. } => images,
        _ => &[],
    }
}

fn content(messages: &[Message]) -> Vec<Block<'_>> {
    iter::once(Block::Text {
        text: INTRO.to_owned(),
    })
    .chain(messages.iter().flat_map(|message| {
        tagged(message)
            .map(|text| Block::Text { text })
            .into_iter()
            .chain(images(message).iter().map(|image| Block::Image {
                source: image.into(),
            }))
    }))
    .collect()
}

impl Provider for ClaudeCode {
    fn complete<'a>(
        &'a self,
        model: &'a str,
        effort: Option<Effort>,
        messages: &'a [Message],
        tools: &'a [ToolDefinition],
        progress: &'a OnProgress<'a>,
    ) -> BoxFuture<'a, Result<Reply, LlmError>> {
        Box::pin(async move {
            let server = SdkServer::new(
                SERVER,
                tools
                    .iter()
                    .map(|tool| McpTool {
                        name: &tool.name,
                        description: &tool.description,
                        input_schema: &tool.parameters,
                    })
                    .collect(),
            );
            let mut args = vec![
                "--include-partial-messages".to_owned(),
                "--thinking-display".to_owned(),
                "summarized".to_owned(),
                "--mcp-config".to_owned(),
                server.config()?,
                "--system-prompt".to_owned(),
                system(messages),
                "--model".to_owned(),
                model.to_owned(),
            ];

            if let Some(effort) = effort {
                args.extend(["--effort".to_owned(), effort.as_str().to_owned()]);
            }

            let mut process = self.0.spawn(args)?;
            process.initialize(&[server.name()]).await?;
            process.send(&UserInput::new(content(messages))).await?;

            let mut collector = Collector::new(progress);

            loop {
                match process.next::<StreamEvent>().await? {
                    Output::StreamEvent { event } => {
                        if event
                            .apply(&mut collector, |name| server.tool_name(name))?
                            .is_break()
                        {
                            break;
                        }
                    },
                    Output::ControlRequest {
                        request_id,
                        request: ControlRequest::McpMessage { message },
                    } => {
                        process
                            .send(&ControlReply::new(&request_id, server.answer(message)))
                            .await?;
                    },
                    Output::ControlResponse {
                        response: ControlResponse::Error { error, .. },
                    } => return Err(failed(error)),
                    Output::Finished(finished) => match finished.failure() {
                        Some(failure) => return Err(failed(failure)),
                        None => break,
                    },
                    _ => {},
                }
            }

            Ok(collector.finish())
        })
    }

    fn models(&self) -> BoxFuture<'_, Result<Vec<Model>, LlmError>> {
        Box::pin(async move {
            let models = self.0.models().await?;
            Ok(models
                .into_iter()
                .map(|name| Model {
                    name,
                    context: None,
                })
                .collect())
        })
    }
}
