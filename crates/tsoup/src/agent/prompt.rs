use crate::node::Node;

pub const COMPACT: &str = "The conversation is about to run out of context, so it will be replaced by a summary you write now.
Do not call any tools, reply only with the summary.
Whoever continues will see nothing but this summary, so include everything needed to carry on:
what the user asked for, quoting their latest instructions verbatim,
what has been done so far and what was learned, including the files and commands that matter,
the decisions taken and the reasons for them,
and what was in progress and remains to be done.";

pub const SUMMARY: &str =
    "The earlier conversation was replaced by this summary to free up context.
Continue from where it left off without asking the user to repeat anything.";

pub const TRIMMED: &str =
    "The beginning of the conversation was dropped because it did not fit in the context.";

pub fn sandbox(prompt: &str, modules: &str) -> String {
    if modules.is_empty() {
        prompt.to_owned()
    } else {
        format!("{prompt}\nThe modules for create_vm's modules argument:\n{modules}")
    }
}

const UNCONFINED: &str = "Nothing confines you on the machine:
read_file, write_file and edit_file also accept absolute paths,
and everything you do happens on the real machine and persists,
so ask the user before doing anything destructive or hard to undo.";

const CONFINED: &str = "A sandbox confines you to {directory}:
shell commands see that directory, the system's programs and a home directory and /tmp of their own,
and nothing else on the machine exists for them.
read_file, write_file and edit_file work inside {directory} only.
What you do there happens on the real machine and persists,
so ask the user before doing anything destructive or hard to undo.";

pub fn node(prompt: &str, node: &Node, folder: &str) -> String {
    let access = if node.sandboxed { CONFINED } else { UNCONFINED };

    format!("{prompt}\n\n{access}")
        .replace("{name}", &node.name)
        .replace("{directory}", &node.path(folder).display().to_string())
}

pub fn models(labels: &[String]) -> String {
    format!(
        "The models for spawn_agent's model argument:\n{}",
        labels.join("\n")
    )
}

pub const TITLE: &str =
    "Write a title of at most six words for a session that starts with the message below.
Reply with the title only, without quotes.";
