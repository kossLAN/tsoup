use super::prompt;
use super::subagents::describe;
use crate::llm::Message;
use crate::session::{Context, Entry};

const INTERRUPTED: &str = "the tool call was interrupted before it finished";

pub fn conversation(system: &str, entries: &[Entry]) -> Vec<Message> {
    let mut messages = vec![Message::System {
        content: system.to_owned(),
    }];
    let mut unanswered = Vec::new();

    for entry in &entries[compacted(entries)..] {
        match entry {
            Entry::User { text, images } => {
                interrupt(&mut messages, &mut unanswered);
                messages.push(Message::User {
                    content: text.clone(),
                    images: images.clone(),
                });
            },
            Entry::Assistant {
                text, tool_calls, ..
            } => {
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
            Entry::Compaction { summary } => {
                messages.push(Message::user(format!("{}\n\n{summary}", prompt::SUMMARY)));
            },
            Entry::Report { agent, state, text } => {
                interrupt(&mut messages, &mut unanswered);
                messages.push(Message::user(describe(*agent, *state, text.as_deref())));
            },
        }
    }

    interrupt(&mut messages, &mut unanswered);
    messages
}

pub fn context(entries: &[Entry]) -> Option<Context> {
    entries[compacted(entries)..]
        .iter()
        .rev()
        .find_map(|entry| match entry {
            Entry::Assistant { context, .. } => *context,
            _ => None,
        })
}

pub fn trimmed(mut messages: Vec<Message>) -> Option<Vec<Message>> {
    let kept = (messages.len().div_ceil(2).max(3)..messages.len()).find(|&index| {
        matches!(
            messages[index],
            Message::User { .. } | Message::Assistant { .. }
        )
    })?;

    messages.splice(1..kept, [Message::user(prompt::TRIMMED)]);
    Some(messages)
}

fn compacted(entries: &[Entry]) -> usize {
    entries
        .iter()
        .rposition(|entry| matches!(entry, Entry::Compaction { .. }))
        .unwrap_or(0)
}

fn interrupt(messages: &mut Vec<Message>, unanswered: &mut Vec<String>) {
    messages.extend(unanswered.drain(..).map(|tool_call_id| Message::Tool {
        tool_call_id,
        content: INTERRUPTED.to_owned(),
    }));
}
