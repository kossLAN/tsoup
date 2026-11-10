use schemars::JsonSchema;
use serde::Deserialize;

use super::{Caller, Outcome, Tool, ToolError};
use crate::workspace::Workspace;

pub struct Ask;

#[derive(Deserialize, JsonSchema)]
pub struct Arguments {
    #[schemars(description = "The question, with the context the user needs to answer it")]
    question: String,
    #[schemars(
        description = "Suggested answers the user can pick with one click, they can always type their own"
    )]
    #[serde(default)]
    options: Vec<String>,
}

impl<E: Workspace> Tool<E> for Ask {
    const NAME: &'static str = "ask";
    const DESCRIPTION: &'static str = "Ask the user a question and wait for their answer.
Use it when you cannot continue without a decision or a piece of information only the user has.";

    type Arguments = Arguments;

    async fn run(
        &self,
        caller: &Caller<'_>,
        _environment: &E,
        Arguments { question, options }: Arguments,
    ) -> Result<Outcome, ToolError> {
        let answer = caller.session.ask(caller.id, question, options).await?;
        Ok(Outcome::text(answer))
    }
}
