// SPDX-License-Identifier: MIT OR Apache-2.0
//! An app's commands as tools for agents (the Model Context Protocol, JSON-RPC 2.0 over stdio).
//!
//! ```no_run
//! use jcj_agents::{AgentConfig, AgentInfo, AgentServer};
//!
//! let registry = std::sync::Arc::new(jcj_engine::registry_with(vec![/* the app's commands */]));
//! let session = jcj_engine::session_with(jcj_doc::new_document(), registry);
//! let info = AgentInfo::new("notes", "Notes", "0.1.0", "Notes keeps short notes. Run commands with the tools.");
//! let mut server = AgentServer::headless(session, AgentConfig::new(info).export("notes."));
//! server.serve_stdio().unwrap();
//! ```
//!
//! The server is WordCraft's (`wordcraft-mcp`) over an in-process session (no window): its
//! general tools (list and run any command by id, type text, inspect the document, render a page,
//! open and save files) work on the app's registry. This crate adds:
//!
//! - the app's own name, version and instructions in the `initialize` answer;
//! - each app command whose id starts with an exported prefix as a tool of its own, named after
//!   the command (`notes.add` → `notes_add`), so an agent finds it in `tools/list` without
//!   searching the whole command list.
//!
//! An app command whose tool name is the name of a general tool is reachable through `execute`
//! only. craft-libs' `craft-mcp-server` is not at our pins; it would replace the server here.
//!
//! **Commands that do not run on a document.** An app whose commands work on its own state (a
//! project, a library) describes them with [`AppCommands`] and serves them with
//! [`AgentServer::for_app`]: each command is a tool named `<app>_<command>`, the files its
//! parameters name are checked against the folders the agent may use, and the general document
//! tools, which would work on a document the app does not have, are left out.
//!
//! ```
//! use jcj_agents::{AgentConfig, AgentInfo, AgentServer, AppCommand, AppCommands};
//! use serde_json::{Value, json};
//!
//! struct Shelf(Vec<String>);
//!
//! impl AppCommands for Shelf {
//!     fn commands(&self) -> Vec<AppCommand> {
//!         vec![
//!             AppCommand::new("book.add", "Add a Book").params(r#"{"title": text}"#),
//!             AppCommand::new("book.list", "List the Books").read_only(),
//!         ]
//!     }
//!
//!     fn run(&mut self, id: &str, params: &Value) -> Result<Value, String> {
//!         match id {
//!             "book.add" => {
//!                 self.0.push(params["title"].as_str().ok_or("a title is needed")?.to_owned());
//!                 Ok(json!(self.0.len()))
//!             }
//!             "book.list" => Ok(json!(self.0)),
//!             other => Err(format!("unknown command {other}")),
//!         }
//!     }
//! }
//!
//! let info = AgentInfo::new("shelf", "Shelf", "0.1.0", "Shelf keeps a list of books.");
//! let server = AgentServer::for_app(Box::new(Shelf(Vec::new())), "shelf", AgentConfig::new(info));
//! assert_eq!(server.tool_names(), ["shelf_book_add", "shelf_book_list"]);
//! ```
//!
//! [`cli`] holds what every app's command line does the same way: the exit codes, `--help` and
//! `--version`, and the arguments of `mcp` (decision 0017).

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod app;
pub mod cli;

pub use app::{AppCommand, AppCommands};

use std::io::{BufRead, Read, Write};
use std::sync::Arc;

use jcj_engine::Session;
use jcj_engine::file_access::AutomationRoots;
use jcj_upstream::mcp::tools::ToolResult;
use jcj_upstream::mcp::{Headless, Server};
use serde_json::{Value, json};

/// Longest request line read from the client, bytes.
pub const MAX_REQUEST_LINE: usize = 1 << 20;

/// What the server says about itself when a client connects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentInfo {
    /// Short machine name (`notes`).
    pub name: String,
    /// Name people read (`Notes`).
    pub title: String,
    pub version: String,
    /// What the app is and how to work with it, for the agent.
    pub instructions: String,
}

impl AgentInfo {
    /// Information about an app.
    pub fn new(name: &str, title: &str, version: &str, instructions: &str) -> Self {
        AgentInfo { name: name.into(), title: title.into(), version: version.into(), instructions: instructions.into() }
    }
}

/// How the server is set up.
#[derive(Clone)]
pub struct AgentConfig {
    pub info: AgentInfo,
    /// Command id prefixes whose commands become tools of their own (`"notes."`).
    pub exported: Vec<String>,
    /// Folders the commands may read and write files in; `None` leaves files unconfined.
    pub roots: Option<Arc<AutomationRoots>>,
}

impl AgentConfig {
    /// A server for the app `info` describes, exporting no commands of its own yet.
    pub fn new(info: AgentInfo) -> Self {
        AgentConfig { info, exported: Vec::new(), roots: None }
    }

    /// Export every command whose id starts with `prefix` as a tool.
    pub fn export(mut self, prefix: &str) -> Self {
        self.exported.push(prefix.to_string());
        self
    }

    /// Confine the files commands read and write to `roots`.
    pub fn confined(mut self, roots: Option<Arc<AutomationRoots>>) -> Self {
        self.roots = roots;
        self
    }
}

/// An app command exported as a tool.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppTool {
    /// The tool name (`notes_add`).
    pub name: String,
    /// The command it runs (`notes.add`).
    pub command: &'static str,
    title: &'static str,
    description: String,
    read_only: bool,
}

impl AppTool {
    fn definition(&self) -> Value {
        json!({
            "name": self.name,
            "title": self.title,
            "description": self.description,
            "inputSchema": {"type": "object", "description": "The command's parameters", "additionalProperties": true},
            "annotations": {"title": self.title, "readOnlyHint": self.read_only, "openWorldHint": false},
        })
    }
}

/// The tool name for command `id`: letters, digits, `_` and `-` only, at most 64 characters.
pub fn tool_name(id: &str) -> String {
    id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).take(64).collect()
}

/// The tool name for an app's command `id` under the app's short name `prefix`:
/// `prefix_command`, by [`tool_name`]'s rules (`notes` and `item.rename` give
/// `notes_item_rename`). An id that already starts with the prefix as its namespace
/// (`notes.rename`) is not prefixed twice. An empty prefix gives [`tool_name`].
pub fn app_tool_name(prefix: &str, id: &str) -> String {
    let prefix = tool_name(prefix.trim().trim_end_matches(['.', '_']));
    let name = tool_name(id);
    if prefix.is_empty() || name.starts_with(&format!("{prefix}_")) { name } else { tool_name(&format!("{prefix}_{name}")) }
}

/// An MCP server for an app.
pub struct AgentServer {
    inner: Server,
    info: AgentInfo,
    /// The session's commands the app exports, each with its command's own name.
    exported: Vec<AppTool>,
    /// Those that are tools, under the names they are served by.
    tools: Vec<AppTool>,
    /// The app's own commands, when it serves some ([`AgentServer::with_app_commands`]).
    app: Option<Box<dyn AppCommands>>,
    app_commands: Vec<AppCommand>,
    /// Those that are tools: (tool name, command).
    app_tools: Vec<(String, AppCommand)>,
    /// The app's short name, which its tools' names start with (empty: none).
    prefix: String,
    /// The folders the commands may read and write files in.
    roots: Option<Arc<AutomationRoots>>,
    /// Serve the general document tools too.
    general: bool,
}

fn general_tool_names() -> Vec<String> {
    match jcj_upstream::mcp::tools::tool_definitions() {
        Value::Array(v) => v.iter().filter_map(|t| t.get("name").and_then(Value::as_str).map(str::to_string)).collect(),
        _ => Vec::new(),
    }
}

impl AgentServer {
    /// A server over `session` (headless: no window), with `config`'s exported commands as tools.
    pub fn headless(session: Session, config: AgentConfig) -> Self {
        let mut exported: Vec<AppTool> = Vec::new();
        for spec in session.registry.all() {
            if !config.exported.iter().any(|p| spec.id.starts_with(p.as_str())) {
                continue;
            }
            let mut description = format!("{} ({}). Command `{}`.", spec.label, spec.location, spec.id);
            if spec.params != "{}" {
                description.push_str(&format!(" Parameters: {}", spec.params));
            }
            exported.push(AppTool { name: tool_name(spec.id), command: spec.id, title: spec.label, description, read_only: !spec.mutates });
        }
        let mut backend = Headless::default().with_automation_roots(config.roots.clone());
        backend.session = session;
        AgentServer {
            inner: Server::new(Box::new(backend)),
            info: config.info,
            exported,
            tools: Vec::new(),
            app: None,
            app_commands: Vec::new(),
            app_tools: Vec::new(),
            prefix: String::new(),
            roots: config.roots,
            general: true,
        }
        .named()
    }

    /// Give every tool of the app's own its name, in order: the exported commands, then the
    /// app's own commands. A name a general tool has (while those are served), or an earlier
    /// tool, is taken: that command gets no tool.
    fn named(mut self) -> Self {
        let mut taken = if self.general { general_tool_names() } else { Vec::new() };
        let mut free = |name: &str| {
            let ok = !name.is_empty() && !taken.iter().any(|t| t == name);
            if ok {
                taken.push(name.to_owned());
            }
            ok
        };
        self.tools = Vec::new();
        for tool in &self.exported {
            let name = app_tool_name(&self.prefix, tool.command);
            if free(&name) {
                self.tools.push(AppTool { name, ..tool.clone() });
            }
        }
        self.app_tools = Vec::new();
        for command in &self.app_commands {
            let name = app_tool_name(&self.prefix, &command.id);
            if free(&name) {
                self.app_tools.push((name, command.clone()));
            }
        }
        self
    }

    /// A server for an app whose commands work on its own state and not on a document: its
    /// tools are `app`'s commands, each named `prefix_command` ([`app_tool_name`]), and nothing
    /// else. The files a command's parameters name are checked against `config`'s folders
    /// before it runs ([`AppCommand::reads`], [`AppCommand::writes`]).
    ///
    /// It is [`AgentServer::headless`] on an empty document, with
    /// [`AgentServer::with_app_commands`], [`AgentServer::tool_prefix`] and
    /// [`AgentServer::without_general_tools`].
    pub fn for_app(app: Box<dyn AppCommands>, prefix: &str, config: AgentConfig) -> Self {
        let session = Session::new(jcj_upstream::doc::Document::new());
        AgentServer::headless(session, config).with_app_commands(app).tool_prefix(prefix).without_general_tools()
    }

    /// Serve `app`'s own commands as tools too, beside the session's. A command whose tool name
    /// is taken (by a general tool, an exported command or an earlier one of these) gets no
    /// tool. Calling this again replaces the app's commands.
    pub fn with_app_commands(mut self, app: Box<dyn AppCommands>) -> Self {
        self.app_commands = app.commands();
        self.app = Some(app);
        self.named()
    }

    /// Name every tool of the app's own after the app: `prefix_command` ([`app_tool_name`]),
    /// for the exported commands and for the app's own commands. The general document tools
    /// keep their names. Without this a tool is named after its command alone.
    pub fn tool_prefix(mut self, prefix: &str) -> Self {
        self.prefix = prefix.to_owned();
        self.named()
    }

    /// Leave out the general document tools (`execute`, `list_commands`, typing, inspecting,
    /// rendering, opening and saving files): the server lists and runs only the app's own
    /// tools. For an app that has no document behind them.
    pub fn without_general_tools(mut self) -> Self {
        self.general = false;
        self.named()
    }

    /// The app commands exported as tools (the session's commands, by
    /// [`AgentConfig::export`]). [`AgentServer::tool_names`] lists these and the tools of
    /// [`AgentServer::with_app_commands`].
    pub fn app_tools(&self) -> &[AppTool] {
        &self.tools
    }

    /// The name of every tool that is the app's own, in the order `tools/list` gives them: the
    /// exported commands, then the app's own commands.
    pub fn tool_names(&self) -> Vec<String> {
        self.tools.iter().map(|t| t.name.clone()).chain(self.app_tools.iter().map(|t| t.0.clone())).collect()
    }

    /// Run one of the app's own commands: the files its parameters name are checked first.
    fn call_app(&mut self, command: &AppCommand, args: &Value) -> ToolResult {
        let Some(app) = self.app.as_mut() else { return ToolResult::error("the app's commands are not available") };
        let roots = self.roots.clone();
        let result = jcj_engine::file_access::confine(roots.as_ref(), || {
            for path in command.reads.iter().flat_map(|key| app::paths_at(args, key)) {
                jcj_engine::file_access::check_read(path)?;
            }
            for path in command.writes.iter().flat_map(|key| app::paths_at(args, key)) {
                jcj_engine::file_access::check_write(path)?;
            }
            app.run(&command.id, args)
        });
        match result {
            Ok(v) => ToolResult::json(&v),
            Err(e) => ToolResult::error(e),
        }
    }

    fn app_definitions(&self) -> Vec<Value> {
        self.app_tools
            .iter()
            .map(|(name, c)| {
                json!({
                    "name": name,
                    "title": c.label,
                    "description": c.description(),
                    "inputSchema": {"type": "object", "description": "The command's parameters", "additionalProperties": true},
                    "annotations": {"title": c.label, "readOnlyHint": c.read_only, "openWorldHint": false},
                })
            })
            .collect()
    }

    /// Answer one JSON-RPC line; `None` for notifications and blank lines.
    pub fn handle_line(&mut self, line: &str) -> Option<String> {
        let line = line.trim();
        if line.is_empty() {
            return None;
        }
        match serde_json::from_str::<Value>(line) {
            Ok(msg @ Value::Object(_)) => self.handle(msg).map(|r| r.to_string()),
            // Batches and malformed lines: the general server answers them.
            _ => self.inner.handle_line(line),
        }
    }

    /// Answer one JSON-RPC message; `None` for notifications.
    pub fn handle(&mut self, msg: Value) -> Option<Value> {
        let method = msg.get("method").and_then(Value::as_str).unwrap_or_default().to_string();
        let id = msg.get("id").cloned();
        match method.as_str() {
            "tools/call" => {
                let name = msg.get("params").and_then(|p| p.get("name")).and_then(Value::as_str).unwrap_or_default().to_string();
                let args = msg.get("params").and_then(|p| p.get("arguments")).cloned().filter(|a| !a.is_null()).unwrap_or(json!({}));
                let result = if let Some(tool) = self.tools.iter().find(|t| t.name == name).cloned() {
                    match self.inner.backend().call("engine.execute", json!({"command": tool.command, "params": args})) {
                        Ok(v) => ToolResult::json(&v),
                        Err(e) => ToolResult::error(e),
                    }
                } else if let Some((_, command)) = self.app_tools.iter().find(|t| t.0 == name).cloned() {
                    // An app command always gets an object, whatever the client sent.
                    let args = if args.is_object() { args } else { json!({}) };
                    self.call_app(&command, &args)
                } else if self.general {
                    return self.inner.handle(msg);
                } else {
                    ToolResult::error(format!("unknown tool `{name}`"))
                };
                id.map(|id| json!({"jsonrpc": "2.0", "id": id, "result": result.to_value()}))
            }
            "tools/list" => {
                let mut reply = self.inner.handle(msg)?;
                let own: Vec<Value> = self.tools.iter().map(AppTool::definition).chain(self.app_definitions()).collect();
                if let Some(list) = reply.pointer_mut("/result/tools").and_then(Value::as_array_mut) {
                    if !self.general {
                        list.clear();
                    }
                    list.extend(own);
                }
                Some(reply)
            }
            "initialize" => {
                let mut reply = self.inner.handle(msg)?;
                if let Some(r) = reply.get_mut("result").and_then(Value::as_object_mut) {
                    let i = &self.info;
                    r.insert("serverInfo".into(), json!({"name": i.name, "title": i.title, "version": i.version}));
                    r.insert("instructions".into(), json!(i.instructions));
                }
                Some(reply)
            }
            _ => self.inner.handle(msg),
        }
    }

    /// Serve newline-delimited JSON-RPC from `input` to `output` until `input` closes. An
    /// over-long line is answered with an error and skipped.
    pub fn serve(&mut self, mut input: impl BufRead, mut output: impl Write) -> std::io::Result<()> {
        loop {
            let mut buf = Vec::new();
            (&mut input).take(MAX_REQUEST_LINE as u64 + 1).read_until(b'\n', &mut buf)?;
            if buf.is_empty() {
                return Ok(());
            }
            let reply = if buf.last() != Some(&b'\n') && buf.len() > MAX_REQUEST_LINE {
                skip_line(&mut input)?;
                Some(json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32600, "message": "request line too long"}}).to_string())
            } else {
                match String::from_utf8(buf) {
                    Ok(line) => self.handle_line(&line),
                    Err(_) => {
                        Some(json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": "request line is not UTF-8"}}).to_string())
                    }
                }
            };
            if let Some(reply) = reply {
                output.write_all(reply.as_bytes())?;
                output.write_all(b"\n")?;
                output.flush()?;
            }
        }
    }

    /// [`AgentServer::serve`] on the process's standard input and output (logs go to stderr).
    pub fn serve_stdio(&mut self) -> std::io::Result<()> {
        self.serve(std::io::stdin().lock(), std::io::stdout().lock())
    }
}

/// Discard the rest of the current line.
fn skip_line(input: &mut impl BufRead) -> std::io::Result<()> {
    loop {
        let chunk = input.fill_buf()?;
        if chunk.is_empty() {
            return Ok(());
        }
        let found = chunk.iter().position(|&b| b == b'\n');
        let used = found.map_or(chunk.len(), |i| i + 1);
        input.consume(used);
        if found.is_some() {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests;
