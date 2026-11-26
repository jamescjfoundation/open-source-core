// SPDX-License-Identifier: MIT OR Apache-2.0
//! An app's own commands as agent tools: for commands that work on the app's own state (a
//! project, a library, a board) and not on a document session.
//!
//! The engine's commands are functions on a document session, and
//! [`AgentServer::headless`](crate::AgentServer::headless) turns those into tools. An app whose
//! commands run on something else describes them here instead: [`AppCommands`] says what they
//! are ([`AppCommand`]) and runs one by id, and the server names a tool after each, lists it,
//! checks the files it names against the folders the agent may use, and calls it.

use serde_json::Value;

/// One of an app's own commands, as an agent sees it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AppCommand {
    /// The command's id in the app (`item.rename`). Its tool is named after it
    /// ([`crate::app_tool_name`]).
    pub id: String,
    /// Its name as people read it ("Rename an Item").
    pub label: String,
    /// Its parameters, described for the agent (`{"id": the item, "name": the new name}`);
    /// `{}` for none.
    pub params: String,
    /// Whether it only reads: it changes nothing, in the app or on disk.
    pub read_only: bool,
    /// The parameters that name a file or folder the command reads (a text or a list of texts).
    /// Each is checked against the folder the agent may read before the command runs.
    pub reads: Vec<String>,
    /// The parameters that name a file or folder the command writes, checked against the folder
    /// the agent may write.
    pub writes: Vec<String>,
}

impl AppCommand {
    /// A command that changes something, takes no parameters and names no file.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        AppCommand { id: id.into(), label: label.into(), params: "{}".into(), ..AppCommand::default() }
    }

    /// With its parameters described for the agent.
    pub fn params(mut self, params: impl Into<String>) -> Self {
        self.params = params.into();
        self
    }

    /// It only reads.
    pub fn read_only(mut self) -> Self {
        self.read_only = true;
        self
    }

    /// These parameters name files or folders it reads.
    pub fn reads(mut self, params: &[&str]) -> Self {
        self.reads.extend(params.iter().map(|p| (*p).to_owned()));
        self
    }

    /// These parameters name files or folders it writes.
    pub fn writes(mut self, params: &[&str]) -> Self {
        self.writes.extend(params.iter().map(|p| (*p).to_owned()));
        self
    }

    /// What the tool says about itself.
    pub(crate) fn description(&self) -> String {
        let mut text = format!("{}. Command `{}`.", self.label.trim_end_matches('.'), self.id);
        if self.params.trim() != "{}" && !self.params.trim().is_empty() {
            text.push_str(&format!(" Parameters: {}", self.params));
        }
        text
    }
}

/// An app's own commands: what they are, and how one runs.
///
/// The server holds the value for as long as it serves, on one thread: it needs no `Send`. A
/// command that changes the app's project saves it, or leaves that to a command of its own;
/// either way that is the app's rule, written in [`AppCommands::run`].
pub trait AppCommands {
    /// Every command an agent may run. Asked once, when the server is made.
    fn commands(&self) -> Vec<AppCommand>;

    /// Run command `id` with `params` (always an object) and give its result. An error's text is
    /// shown to the agent.
    fn run(&mut self, id: &str, params: &Value) -> Result<Value, String>;
}

/// The texts under `key` in `params`: one text, or each text of a list.
pub(crate) fn paths_at<'a>(params: &'a Value, key: &str) -> Vec<&'a str> {
    match params.get(key) {
        Some(Value::String(s)) => vec![s.as_str()],
        Some(Value::Array(list)) => list.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    }
}
