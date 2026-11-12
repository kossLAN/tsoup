use schemars::JsonSchema;
use serde::Deserialize;

use super::{Caller, Outcome, Scope, Tool, ToolError};
use crate::session::Entry;
use crate::workspace::Workspace;

pub struct Compact;

#[derive(Deserialize, JsonSchema)]
pub struct Arguments {
    #[schemars(
        description = "Everything needed to carry on, since you will see nothing else afterwards: what the user asked for, quoting their latest instructions verbatim, what has been done so far and what was learned, including the files and commands that matter, the decisions taken and the reasons for them, and what is in progress and remains to be done."
    )]
    summary: String,
}

impl<E: Workspace> Tool<E> for Compact {
    const NAME: &'static str = "compact";
    const DESCRIPTION: &'static str = "Replace the conversation so far with a summary you write, which frees up context and makes every later request cheaper.
Use it at a natural break when there is more work to do and the details so far are no longer needed,
such as after a long investigation, a finished piece of the task or a pile of large tool outputs.
Do not use it while the conversation is still short or when the task is nearly done.
Call it on its own, never alongside other tool calls, because their results would be replaced unseen.";

    type Arguments = Arguments;

    fn available(scope: Scope) -> bool {
        scope.compact
    }

    async fn run(
        &self,
        caller: &Caller<'_>,
        _environment: &E,
        Arguments { summary }: Arguments,
    ) -> Result<Outcome, ToolError> {
        if summary.trim().is_empty() {
            return Err(ToolError::NoSummary);
        }

        let entries = caller.session.entries(caller.id).await?;

        if matches!(
            entries.iter().nth_back(1),
            None | Some(Entry::Compaction { .. })
        ) {
            return Err(ToolError::NothingToCompact);
        }

        Ok(Outcome::compacted(summary))
    }
}
