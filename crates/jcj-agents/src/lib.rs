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

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

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

/// An MCP server for an app.
pub struct AgentServer {
    inner: Server,
    info: AgentInfo,
    tools: Vec<AppTool>,
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
        let general = general_tool_names();
        let mut tools: Vec<AppTool> = Vec::new();
        for spec in session.registry.all() {
            if !config.exported.iter().any(|p| spec.id.starts_with(p.as_str())) {
                continue;
            }
            let name = tool_name(spec.id);
            if general.contains(&name) || tools.iter().any(|t| t.name == name) {
                continue;
            }
            let mut description = format!("{} ({}). Command `{}`.", spec.label, spec.location, spec.id);
            if spec.params != "{}" {
                description.push_str(&format!(" Parameters: {}", spec.params));
            }
            tools.push(AppTool { name, command: spec.id, title: spec.label, description, read_only: !spec.mutates });
        }
        let mut backend = Headless::default().with_automation_roots(config.roots);
        backend.session = session;
        AgentServer { inner: Server::new(Box::new(backend)), info: config.info, tools }
    }

    /// The app commands exported as tools.
    pub fn app_tools(&self) -> &[AppTool] {
        &self.tools
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
                let name = msg.get("params").and_then(|p| p.get("name")).and_then(Value::as_str).unwrap_or_default();
                let Some(tool) = self.tools.iter().find(|t| t.name == name).cloned() else { return self.inner.handle(msg) };
                let args = msg.get("params").and_then(|p| p.get("arguments")).cloned().filter(|a| !a.is_null()).unwrap_or(json!({}));
                let result = match self.inner.backend().call("engine.execute", json!({"command": tool.command, "params": args})) {
                    Ok(v) => ToolResult::json(&v),
                    Err(e) => ToolResult::error(e),
                };
                id.map(|id| json!({"jsonrpc": "2.0", "id": id, "result": result.to_value()}))
            }
            "tools/list" => {
                let mut reply = self.inner.handle(msg)?;
                if let Some(list) = reply.pointer_mut("/result/tools").and_then(Value::as_array_mut) {
                    list.extend(self.tools.iter().map(AppTool::definition));
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
