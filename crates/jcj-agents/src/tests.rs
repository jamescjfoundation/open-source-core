// SPDX-License-Identifier: MIT OR Apache-2.0
use std::sync::Arc;

use jcj_doc::ExtKey;
use jcj_engine::{CmdError, CommandSpec, Session, registry_with, session_with};
use serde_json::{Value, json};

use crate::{AgentConfig, AgentInfo, AgentServer, AppCommand, AppCommands, app_tool_name, tool_name};

const COUNT: ExtKey<u32> = ExtKey::new("test.count");

fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("test.bump", "Bump", "Test", |s: &mut Session, v: &Value| {
            let by = v.get("by").and_then(Value::as_u64).ok_or_else(|| CmdError::Params("`by` is required".into()))? as u32;
            let n = COUNT.get_or_none(&s.doc).unwrap_or(0) + by;
            COUNT.set(&mut s.doc, &n).map_err(|e| CmdError::Failed(e.to_string()))?;
            Ok(json!(n))
        })
        .params(r#"{"by": n}"#),
        CommandSpec::new("test.count", "Count", "Test", |s, _| Ok(json!(COUNT.get_or_none(&s.doc).unwrap_or(0)))).pure(),
        // Its tool name is a general tool's: reachable through `execute` only.
        CommandSpec::new("list.commands", "Clash", "Test", |_, _| Ok(Value::Null)),
    ]
}

fn server() -> AgentServer {
    let session = session_with(jcj_doc::new_document(), Arc::new(registry_with(specs())));
    let info = AgentInfo::new("tester", "Tester", "9.9.9", "A test app.");
    AgentServer::headless(session, AgentConfig::new(info).export("test.").export("list.commands"))
}

fn call(s: &mut AgentServer, id: u64, method: &str, params: Value) -> Value {
    s.handle(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})).unwrap()
}

#[test]
fn tool_names_are_safe() {
    assert_eq!(tool_name("notes.add"), "notes_add");
    assert_eq!(tool_name("a b/c-d"), "a_b_c-d");
    assert_eq!(tool_name(&"x".repeat(100)).len(), 64);
}

#[test]
fn initialize_names_the_app() {
    let mut s = server();
    let r = call(&mut s, 1, "initialize", json!({"protocolVersion": "2025-06-18"}));
    assert_eq!(r["result"]["serverInfo"]["name"], "tester");
    assert_eq!(r["result"]["serverInfo"]["version"], "9.9.9");
    assert_eq!(r["result"]["instructions"], "A test app.");
    assert!(r["result"]["capabilities"]["tools"].is_object());
}

#[test]
fn app_commands_are_listed_and_called_as_tools() {
    let mut s = server();
    let names: Vec<&str> = s.app_tools().iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["test_bump", "test_count"]);
    let list = call(&mut s, 1, "tools/list", json!({}));
    let tools = list["result"]["tools"].as_array().unwrap();
    let find = |n: &str| tools.iter().find(|t| t["name"] == n).cloned();
    assert!(find("execute").is_some(), "the general tools stay");
    let bump = find("test_bump").unwrap();
    assert_eq!(bump["annotations"]["readOnlyHint"], false);
    assert!(bump["description"].as_str().unwrap().contains("\"by\""));
    assert_eq!(find("test_count").unwrap()["annotations"]["readOnlyHint"], true);

    let r = call(&mut s, 2, "tools/call", json!({"name": "test_bump", "arguments": {"by": 2}}));
    assert_eq!(r["result"]["isError"], false);
    assert_eq!(r["result"]["content"][0]["text"], "2");
    let r = call(&mut s, 3, "tools/call", json!({"name": "test_bump", "arguments": {}}));
    assert_eq!(r["result"]["isError"], true);
    // The general `execute` tool runs app commands too, on the same session.
    let r = call(&mut s, 4, "tools/call", json!({"name": "execute", "arguments": {"command": "test.count"}}));
    assert_eq!(r["result"]["content"][0]["text"], "2");
    // An app edit is undoable through the engine's own commands.
    call(&mut s, 5, "tools/call", json!({"name": "execute", "arguments": {"command": "edit.undo"}}));
    let r = call(&mut s, 6, "tools/call", json!({"name": "test_count", "arguments": {}}));
    assert_eq!(r["result"]["content"][0]["text"], "0");
}

#[test]
fn serve_answers_each_line_and_skips_notifications() {
    let mut s = server();
    let input = [
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}).to_string(),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}).to_string(),
        String::new(),
        "not json".to_string(),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "test_bump", "arguments": {"by": 5}}}).to_string(),
    ]
    .join("\n");
    let mut out = Vec::new();
    s.serve(input.as_bytes(), &mut out).unwrap();
    let replies: Vec<Value> = String::from_utf8(out).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(replies.len(), 3);
    assert_eq!(replies[0]["id"], 1);
    assert!(replies[1]["error"].is_object(), "a parse error is answered");
    assert_eq!(replies[2]["result"]["content"][0]["text"], "5");
}

/// An app whose commands work on its own state: a list of lines, with files it reads and writes.
#[derive(Default)]
struct Lines {
    lines: Vec<String>,
    /// Every command run, for the tests.
    ran: std::rc::Rc<std::cell::RefCell<Vec<String>>>,
}

impl AppCommands for Lines {
    fn commands(&self) -> Vec<AppCommand> {
        vec![
            AppCommand::new("line.add", "Add a Line").params(r#"{"text": the line}"#),
            AppCommand::new("line.list", "List the Lines.").read_only(),
            AppCommand::new("lines.import", "Import Lines").params(r#"{"path": a text file, "paths": more text files}"#).reads(&["path", "paths"]),
            AppCommand::new("lines.export", "Export the Lines").params(r#"{"path": where to write}"#).writes(&["path"]),
            // Named as an earlier one is: it gets no tool.
            AppCommand::new("line_add", "A Second Add"),
        ]
    }

    fn run(&mut self, id: &str, params: &Value) -> Result<Value, String> {
        self.ran.borrow_mut().push(id.to_owned());
        assert!(params.is_object(), "parameters are always an object");
        match id {
            "line.add" => {
                self.lines.push(params.get("text").and_then(Value::as_str).ok_or("`text` is required")?.to_owned());
                Ok(json!(self.lines.len()))
            }
            "line.list" => Ok(json!(self.lines)),
            "lines.import" => {
                let path = params.get("path").and_then(Value::as_str).ok_or("`path` is required")?;
                let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
                self.lines.extend(text.lines().map(str::to_owned));
                Ok(json!(self.lines.len()))
            }
            "lines.export" => {
                let path = params.get("path").and_then(Value::as_str).ok_or("`path` is required")?;
                std::fs::write(path, self.lines.join("\n")).map_err(|e| e.to_string())?;
                Ok(Value::Null)
            }
            other => Err(format!("unknown command {other}")),
        }
    }
}

fn info() -> AgentInfo {
    AgentInfo::new("liner", "Liner", "1.0.0", "Liner keeps lines.")
}

fn text(reply: &Value) -> &str {
    reply["result"]["content"][0]["text"].as_str().unwrap()
}

#[test]
fn an_apps_tools_carry_its_name() {
    assert_eq!(app_tool_name("notes", "item.rename"), "notes_item_rename");
    assert_eq!(app_tool_name("notes", "notes.rename"), "notes_rename", "its own namespace is not said twice");
    assert_eq!(app_tool_name("notes.", "notes_rename"), "notes_rename");
    assert_eq!(app_tool_name("note", "notes.rename"), "note_notes_rename", "a prefix is a whole name");
    assert_eq!(app_tool_name("", "item.rename"), tool_name("item.rename"));
    assert_eq!(app_tool_name("my app", "a b"), "my_app_a_b");
    assert_eq!(app_tool_name("notes", &"x".repeat(100)).len(), 64);

    // The session's exported commands take the prefix too, and the general tools keep theirs. A
    // command whose bare name was a general tool's has a name of its own now.
    let session = session_with(jcj_doc::new_document(), Arc::new(registry_with(specs())));
    let mut s = AgentServer::headless(session, AgentConfig::new(info()).export("test.").export("list.commands")).tool_prefix("liner");
    assert_eq!(s.tool_names(), ["liner_test_bump", "liner_test_count", "liner_list_commands"]);
    assert_eq!(s.app_tools().iter().map(|t| t.command).collect::<Vec<_>>(), ["test.bump", "test.count", "list.commands"]);
    let list = call(&mut s, 1, "tools/list", json!({}));
    let names: Vec<&str> = list["result"]["tools"].as_array().unwrap().iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(names.contains(&"execute") && names.contains(&"liner_test_bump") && !names.contains(&"test_bump"), "{names:?}");
    let r = call(&mut s, 2, "tools/call", json!({"name": "liner_test_bump", "arguments": {"by": 3}}));
    assert_eq!(text(&r), "3");
    let r = call(&mut s, 3, "tools/call", json!({"name": "test_bump", "arguments": {"by": 3}}));
    assert_eq!(r["result"]["isError"], true, "the bare name is no tool any more");
}

#[test]
fn an_apps_own_commands_are_its_only_tools() {
    let ran = std::rc::Rc::default();
    let app = Lines { ran: std::rc::Rc::clone(&ran), ..Lines::default() };
    let mut s = AgentServer::for_app(Box::new(app), "liner", AgentConfig::new(info()));
    assert_eq!(s.tool_names(), ["liner_line_add", "liner_line_list", "liner_lines_import", "liner_lines_export"]);
    assert!(s.app_tools().is_empty(), "no session command is exported");
    let r = call(&mut s, 1, "initialize", json!({}));
    assert_eq!(r["result"]["serverInfo"]["name"], "liner");
    assert_eq!(r["result"]["instructions"], "Liner keeps lines.");

    let list = call(&mut s, 2, "tools/list", json!({}));
    let tools = list["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 4, "the general document tools are left out: there is no document");
    assert_eq!(tools[0]["name"], "liner_line_add");
    assert_eq!(tools[0]["description"], "Add a Line. Command `line.add`. Parameters: {\"text\": the line}");
    assert_eq!(tools[0]["annotations"]["readOnlyHint"], false);
    assert_eq!(tools[1]["description"], "List the Lines. Command `line.list`.");
    assert_eq!(tools[1]["annotations"]["readOnlyHint"], true);
    assert_eq!(tools[1]["inputSchema"]["type"], "object");

    let r = call(&mut s, 3, "tools/call", json!({"name": "liner_line_add", "arguments": {"text": "One."}}));
    assert_eq!((r["result"]["isError"].as_bool(), text(&r)), (Some(false), "1"));
    // No arguments, or arguments that are no object: the command still gets an object.
    let r = call(&mut s, 4, "tools/call", json!({"name": "liner_line_list"}));
    assert_eq!(serde_json::from_str::<Value>(text(&r)).unwrap(), json!(["One."]));
    let r = call(&mut s, 5, "tools/call", json!({"name": "liner_line_list", "arguments": [1, 2]}));
    assert_eq!(r["result"]["isError"], false);
    // A command's own error is the tool's error.
    let r = call(&mut s, 6, "tools/call", json!({"name": "liner_line_add", "arguments": {}}));
    assert_eq!((r["result"]["isError"].as_bool(), text(&r)), (Some(true), "`text` is required"));
    // The general tools are not there to call either.
    for name in ["execute", "list_commands", "line_add", ""] {
        let r = call(&mut s, 7, "tools/call", json!({"name": name, "arguments": {"command": "edit.undo"}}));
        assert_eq!(r["result"]["isError"], true, "{name}");
        assert!(text(&r).contains("unknown tool"), "{name}: {}", text(&r));
    }
    assert_eq!(*ran.borrow(), ["line.add", "line.list", "line.list", "line.add"]);
    // The rest of the protocol is the general server's.
    assert!(call(&mut s, 8, "ping", json!({}))["result"].is_object());
}

#[test]
fn the_files_an_app_command_names_stay_inside_the_agents_folders() {
    let base = std::env::temp_dir().join(format!("jcj-agents-app-{}", std::process::id()));
    let (inbox, outbox) = (base.join("in"), base.join("out"));
    std::fs::create_dir_all(&inbox).unwrap();
    std::fs::create_dir_all(&outbox).unwrap();
    std::fs::write(inbox.join("a.txt"), "One.\nTwo.").unwrap();
    std::fs::write(base.join("outside.txt"), "Secret.").unwrap();
    let path = |p: &std::path::Path| p.to_string_lossy().into_owned();

    let ran = std::rc::Rc::default();
    let app = Lines { ran: std::rc::Rc::clone(&ran), ..Lines::default() };
    let roots = jcj_engine::file_access::AutomationRoots::new(Some(&inbox), Some(&outbox)).unwrap();
    let mut s = AgentServer::for_app(Box::new(app), "liner", AgentConfig::new(info()).confined(roots));

    let r = call(&mut s, 1, "tools/call", json!({"name": "liner_lines_import", "arguments": {"path": path(&inbox.join("a.txt"))}}));
    assert_eq!((r["result"]["isError"].as_bool(), text(&r)), (Some(false), "2"));
    let r = call(&mut s, 2, "tools/call", json!({"name": "liner_lines_export", "arguments": {"path": path(&outbox.join("b.txt"))}}));
    assert_eq!(r["result"]["isError"], false, "{}", text(&r));
    assert_eq!(std::fs::read_to_string(outbox.join("b.txt")).unwrap(), "One.\nTwo.");

    // Outside the folders: refused before the command runs.
    let before = ran.borrow().len();
    for (tool, args) in [
        ("liner_lines_import", json!({"path": path(&base.join("outside.txt"))})),
        ("liner_lines_import", json!({"path": path(&inbox.join("a.txt")), "paths": [path(&inbox.join("a.txt")), path(&base.join("outside.txt"))]})),
        ("liner_lines_import", json!({"path": path(&inbox.join("../outside.txt"))})),
        ("liner_lines_export", json!({"path": path(&inbox.join("c.txt"))})),
        ("liner_lines_export", json!({"path": path(&base.join("c.txt"))})),
    ] {
        let r = call(&mut s, 3, "tools/call", json!({"name": tool, "arguments": args}));
        assert_eq!(r["result"]["isError"], true, "{tool} {args}");
        assert!(text(&r).contains("outside the"), "{}", text(&r));
    }
    assert_eq!(ran.borrow().len(), before, "a refused command never ran");
    assert!(!inbox.join("c.txt").exists() && !base.join("c.txt").exists());

    // With no folders given, nothing is checked.
    let mut free = AgentServer::for_app(Box::<Lines>::default(), "liner", AgentConfig::new(info()));
    let r = call(&mut free, 4, "tools/call", json!({"name": "liner_lines_import", "arguments": {"path": path(&base.join("outside.txt"))}}));
    assert_eq!(text(&r), "1");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn an_app_can_serve_a_document_and_its_own_commands_together() {
    let session = session_with(jcj_doc::new_document(), Arc::new(registry_with(specs())));
    let config = AgentConfig::new(info()).export("test.");
    // The order of the calls does not matter.
    let mut s = AgentServer::headless(session, config).tool_prefix("liner").with_app_commands(Box::<Lines>::default());
    assert_eq!(
        s.tool_names(),
        ["liner_test_bump", "liner_test_count", "liner_line_add", "liner_line_list", "liner_lines_import", "liner_lines_export"]
    );
    let list = call(&mut s, 1, "tools/list", json!({}));
    let names: Vec<&str> = list["result"]["tools"].as_array().unwrap().iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(names.contains(&"execute") && names.contains(&"liner_test_count") && names.contains(&"liner_line_list"));
    call(&mut s, 2, "tools/call", json!({"name": "liner_test_bump", "arguments": {"by": 4}}));
    call(&mut s, 3, "tools/call", json!({"name": "liner_line_add", "arguments": {"text": "A line."}}));
    assert_eq!(text(&call(&mut s, 4, "tools/call", json!({"name": "liner_test_count"}))), "4");
    assert_eq!(text(&call(&mut s, 5, "tools/call", json!({"name": "execute", "arguments": {"command": "test.count"}}))), "4");
    // Without a prefix, an app command named as a general tool or an exported command gets no
    // tool of its own.
    struct Clash;
    impl AppCommands for Clash {
        fn commands(&self) -> Vec<AppCommand> {
            vec![AppCommand::new("execute", "Clash"), AppCommand::new("test.count", "Clash"), AppCommand::new("own", "Own")]
        }
        fn run(&mut self, _: &str, _: &Value) -> Result<Value, String> {
            Ok(Value::Null)
        }
    }
    let session = session_with(jcj_doc::new_document(), Arc::new(registry_with(specs())));
    let s = AgentServer::headless(session, AgentConfig::new(info()).export("test.")).with_app_commands(Box::new(Clash));
    assert_eq!(s.tool_names(), ["test_bump", "test_count", "own"]);
}
