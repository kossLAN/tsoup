use crate::node::Folder;

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

pub fn node(prompt: &str, folder: &Folder) -> String {
    prompt
        .replace("{name}", &folder.node.name)
        .replace("{directory}", &folder.path().display().to_string())
}

pub fn models(labels: &[String]) -> String {
    format!(
        "The models for spawn_agent's model argument:\n{}",
        labels.join("\n")
    )
}
