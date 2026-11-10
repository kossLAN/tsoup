use uuid::Uuid;

use super::conversation::{conversation, trimmed};
use super::{Agent, Turn, TurnError, prompt};
use crate::llm::{LlmError, Message};
use crate::session::{Entry, Event, Session};

impl Agent {
    pub(super) async fn compact(
        &self,
        session: &Session,
        id: Uuid,
        turn: &Turn,
    ) -> Result<(), TurnError> {
        let entries = session.entries(id).await?;

        if matches!(entries.last(), None | Some(Entry::Compaction { .. })) {
            return Ok(());
        }

        session.emit(Event::Compacting { agent: id });

        let tools = self.tools.definitions(session, turn.scope).await;
        let mut messages = conversation(&turn.system, &entries);

        let summary = loop {
            messages.push(Message::user(prompt::COMPACT));

            match turn
                .llm
                .complete(&turn.model, turn.effort, &messages, &tools, &|_| {})
                .await
            {
                Ok(reply) => break reply.content.ok_or(TurnError::NoSummary)?,
                Err(LlmError::Overflow) => {
                    messages.pop();
                    messages = trimmed(messages).ok_or(LlmError::Overflow)?;
                },
                Err(error) => return Err(error.into()),
            }
        };

        session.push(id, Entry::Compaction { summary }).await?;
        Ok(())
    }
}
