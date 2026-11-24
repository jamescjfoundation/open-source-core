// SPDX-License-Identifier: MIT OR Apache-2.0
use std::sync::Arc;

use jcj_doc::ExtKey;
use jcj_engine::{CmdError, CommandSpec, Session, registry_with, session_with};
use serde_json::{Value, json};

use crate::{AgentConfig, AgentInfo, AgentServer, tool_name};

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
