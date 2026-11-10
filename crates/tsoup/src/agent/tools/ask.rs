use futures_util::future::try_join_all;
use schemars::JsonSchema;
use serde::Deserialize;

use super::{Caller, Outcome, Tool, ToolError};
use crate::workspace::Workspace;

pub struct Ask;

#[derive(Deserialize, JsonSchema)]
pub struct Arguments {
    #[schemars(
        description = "The questions, one entry for each thing you need to know. Never combine several questions into one entry."
    )]
    questions: Vec<Question>,
}

#[derive(Deserialize, JsonSchema)]
pub struct Question {
    #[schemars(description = "A single question, with the context the user needs to answer it")]
    question: String,
    #[schemars(
        description = "Suggested answers the user can pick with one click, they can always type their own"
    )]
    #[serde(default)]
    options: Vec<String>,
}

impl<E: Workspace> Tool<E> for Ask {
    const NAME: &'static str = "ask";
    const DESCRIPTION: &'static str = "Ask the user questions and wait for their answers.
Use it when you cannot continue without a decision or a piece of information only the user has.
Each question goes into a queue the user answers one at a time.";
    const PARALLEL: bool = true;

    type Arguments = Arguments;

    async fn run(
        &self,
        caller: &Caller<'_>,
        _environment: &E,
        Arguments { questions }: Arguments,
    ) -> Result<Outcome, ToolError> {
        let answers = try_join_all(questions.into_iter().map(async |asked| {
            let Question { question, options } = asked;
            let answer = caller
                .session
                .ask(caller.id, question.clone(), options)
                .await?;
            Ok::<_, ToolError>(format!("{question}\n{answer}"))
        }))
        .await?;

        Ok(Outcome::text(answers.join("\n\n")))
    }
}
