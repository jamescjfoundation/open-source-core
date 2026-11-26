// SPDX-License-Identifier: MIT OR Apache-2.0
//! The testbed for agents: a document's commands and the testbed's own commands as tools of one
//! server, every one named `testbed_…` (decision 0017).
//!
//! The document's tools are the session's (`jcj-engine` and `jcj-features`). The board's are the
//! testbed's own, on its own state, through the core's adapter ([`jcj_agents::AppCommands`]):
//! the server names them, lists them and keeps the file one of them writes inside the folder the
//! agent may write.

use std::sync::Arc;

use jcj_agents::{AgentConfig, AgentInfo, AgentServer, AppCommand, AppCommands};
use jcj_board::Board;
use jcj_doc::Document;
use jcj_engine::file_access::AutomationRoots;
use serde_json::{Value, json};

/// The testbed's short name: what its tools' names start with.
pub const PREFIX: &str = "testbed";

/// The document command namespaces that are tools of their own.
pub const EXPORTED: [&str; 2] = ["bookmark.", "note."];

/// What an agent is told when it connects.
pub const INSTRUCTIONS: &str = "The core's test app: a sample document and a board of cards. The document's commands are run with \
execute (list_commands lists them); bookmarks and notes are tools of their own. The board is testbed_board_list, testbed_board_addCard \
and testbed_board_export.";

/// The board's commands, as tools: the testbed's own state, not a document's.
#[derive(Default)]
pub struct BoardTools {
    /// The cards.
    pub board: Board,
}

impl AppCommands for BoardTools {
    fn commands(&self) -> Vec<AppCommand> {
        vec![
            AppCommand::new("board.list", "List the Cards").read_only(),
            AppCommand::new("board.addCard", "Add a Card").params(r#"{"title": text, "body": text}"#),
            AppCommand::new("board.export", "Write the Cards to a File").params(r#"{"path": the JSON file to write}"#).writes(&["path"]),
        ]
    }

    fn run(&mut self, id: &str, params: &Value) -> Result<Value, String> {
        let text = |key: &str| params.get(key).and_then(Value::as_str);
        let cards = |board: &Board| -> Vec<Value> { board.cards.iter().map(|c| json!({ "title": c.title, "body": c.body })).collect() };
        match id {
            "board.list" => Ok(json!(cards(&self.board))),
            "board.addCard" => {
                let title = text("title").ok_or("`title` is required")?;
                self.board.add_card(title, text("body").unwrap_or_default()).map_err(|e| format!("{e:?}"))?;
                Ok(json!({ "cards": self.board.cards.len() }))
            }
            "board.export" => {
                let path = text("path").ok_or("`path` is required")?;
                let bytes = serde_json::to_vec_pretty(&cards(&self.board)).map_err(|e| e.to_string())?;
                std::fs::write(path, bytes).map_err(|e| format!("{path}: {e}"))?;
                Ok(json!({ "written": path }))
            }
            other => Err(format!("unknown command {other}")),
        }
    }
}

/// Who the server says it is.
pub fn info() -> AgentInfo {
    AgentInfo::new("core-testbed", "Core testbed", env!("CARGO_PKG_VERSION"), INSTRUCTIONS)
}

/// A server over `doc` and an empty board, confined to `roots` (none: unconfined).
pub fn server(doc: Document, roots: Option<Arc<AutomationRoots>>) -> AgentServer {
    let mut config = AgentConfig::new(info()).confined(roots);
    for prefix in EXPORTED {
        config = config.export(prefix);
    }
    AgentServer::headless(crate::selftest::session(doc), config).with_app_commands(Box::<BoardTools>::default()).tool_prefix(PREFIX)
}

/// A server whose only tools are the board's: an app with no document behind its commands.
pub fn board_server(roots: Option<Arc<AutomationRoots>>) -> AgentServer {
    AgentServer::for_app(Box::<BoardTools>::default(), PREFIX, AgentConfig::new(info()).confined(roots))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(s: &mut AgentServer, name: &str, args: Value) -> Value {
        s.handle(json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": name, "arguments": args}})).unwrap()
    }

    #[test]
    fn every_tool_of_the_testbeds_own_starts_with_its_name() {
        let mut s = server(crate::selftest::sample_document().unwrap(), None);
        let names = s.tool_names();
        assert!(names.len() > 5 && names.iter().all(|n| n.starts_with("testbed_")), "{names:?}");
        assert!(names.contains(&"testbed_bookmark_add".to_owned()) && names.contains(&"testbed_board_addCard".to_owned()), "{names:?}");
        let r = call(&mut s, "testbed_board_addCard", json!({"title": "First", "body": "A card."}));
        assert_eq!(r["result"]["isError"], false, "{r}");
        let r = call(&mut s, "testbed_board_list", json!({}));
        assert!(r["result"]["content"][0]["text"].as_str().unwrap().contains("First"));
        let r = call(&mut s, "testbed_bookmark_add", json!({"name": "Start"}));
        assert_eq!(r["result"]["isError"], false, "{r}");
        // The general document tools are served beside them, under their own names.
        let r = call(&mut s, "execute", json!({"command": "bookmark.list"}));
        assert!(r["result"]["content"][0]["text"].as_str().unwrap().contains("Start"));
    }

    #[test]
    fn the_board_alone_serves_only_its_tools_and_writes_inside_the_write_folder() {
        let dir = std::env::temp_dir().join(format!("core-testbed-agents-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("out")).unwrap();
        let roots = AutomationRoots::new(None, Some(&dir.join("out"))).unwrap();
        let mut s = board_server(roots);
        assert_eq!(s.tool_names(), ["testbed_board_list", "testbed_board_addCard", "testbed_board_export"]);
        let list = s.handle(json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"})).unwrap();
        assert_eq!(list["result"]["tools"].as_array().unwrap().len(), 3);
        call(&mut s, "testbed_board_addCard", json!({"title": "Kept"}));
        let inside = dir.join("out/cards.json");
        let r = call(&mut s, "testbed_board_export", json!({"path": inside.to_string_lossy()}));
        assert_eq!(r["result"]["isError"], false, "{r}");
        assert!(std::fs::read_to_string(&inside).unwrap().contains("Kept"));
        let outside = dir.join("cards.json");
        let r = call(&mut s, "testbed_board_export", json!({"path": outside.to_string_lossy()}));
        assert_eq!(r["result"]["isError"], true);
        assert!(!outside.exists(), "a file outside the write folder is not written");
        assert_eq!(call(&mut s, "execute", json!({"command": "edit.undo"}))["result"]["isError"], true, "no document tools here");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
