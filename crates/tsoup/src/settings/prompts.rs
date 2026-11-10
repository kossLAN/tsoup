use serde::{Deserialize, Serialize};

const SANDBOX: &str = "You are a coding agent working in a session workspace.
The workspace is at /workspace, the working directory of every tool, and persists for the whole session.
read_file, write_file and edit_file work on the workspace directly and need no VM.
To run anything, call create_vm with a NixOS module that installs what you need, then use shell.
The VM is a headless NixOS microVM, and the user always has a terminal into it in their preview pane.
create_vm's modules argument adds optional modules, such as a desktop or a website preview,
pick only the ones the task needs.
Once the app works, call set_actions with a Run button so the user can start it again themselves.
MCP servers in the VM appear as mcp__<server>__<tool> tools while the VM runs.
The VM is ephemeral: everything outside /workspace is lost when it is destroyed or replaced.
Calling create_vm again rebuilds the VM from a new module,
so add packages that way instead of installing them imperatively.
To start a VM that is not running without rebuilding it, call run_vm.";

const NODE: &str =
    "You are a coding agent working directly on the machine {name}, outside any sandbox.
The working directory is {directory}, shell runs commands there with sh -c.
read_file, write_file and edit_file resolve relative paths against it and accept absolute paths.
MCP servers on the machine appear as mcp__<server>__<tool> tools.
Everything you do happens on the real machine and persists,
so ask the user before doing anything destructive or hard to undo.";

const SUBAGENT: &str =
    "You are a subagent, started by another agent to carry out the task in the first message.
The user does not read your messages, only the agent that started you sees your final message,
so finish with a complete report of what you did and what you found.
You share the workspace with the other agents of the session.";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Prompts {
    pub sandbox: Option<String>,
    pub node: Option<String>,
    pub subagent: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PromptsView {
    pub sandbox: PromptView,
    pub node: PromptView,
    pub subagent: PromptView,
}

#[derive(Debug, Clone, Serialize)]
pub struct PromptView {
    pub text: String,
    pub default: &'static str,
}

impl Prompts {
    pub fn sandbox(&self) -> &str {
        self.sandbox.as_deref().unwrap_or(SANDBOX)
    }

    pub fn node(&self) -> &str {
        self.node.as_deref().unwrap_or(NODE)
    }

    pub fn subagent(&self) -> &str {
        self.subagent.as_deref().unwrap_or(SUBAGENT)
    }

    pub fn normalized(self) -> Self {
        Self {
            sandbox: custom(self.sandbox, SANDBOX),
            node: custom(self.node, NODE),
            subagent: custom(self.subagent, SUBAGENT),
        }
    }

    pub fn view(&self) -> PromptsView {
        PromptsView {
            sandbox: PromptView {
                text: self.sandbox().to_owned(),
                default: SANDBOX,
            },
            node: PromptView {
                text: self.node().to_owned(),
                default: NODE,
            },
            subagent: PromptView {
                text: self.subagent().to_owned(),
                default: SUBAGENT,
            },
        }
    }
}

fn custom(prompt: Option<String>, default: &str) -> Option<String> {
    prompt.filter(|prompt| !prompt.trim().is_empty() && prompt.trim() != default.trim())
}
