// SPDX-License-Identifier: MIT OR Apache-2.0
//! The testbed's command line, held to the core's convention (decision 0017,
//! `docs/command-lines-and-agent-tools.md`): the program itself is run and its exit code, its
//! standard output and its standard error are read. An app's own test of its command line can
//! be written the same way.

use std::io::Write;
use std::process::{Command, Output, Stdio};

use jcj_agents::cli::Exit;

fn run(args: &[&str], input: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_core-testbed"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
    child.wait_with_output().unwrap()
}

fn code(o: &Output) -> Option<i32> {
    o.status.code()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn exit(e: Exit) -> Option<i32> {
    Some(i32::from(e.code()))
}

#[test]
fn help_and_version_are_answered_on_standard_output() {
    for help in ["--help", "-h", "help"] {
        let o = run(&[help], "");
        assert_eq!(code(&o), exit(Exit::Done), "{help}");
        assert!(text(&o.stdout).starts_with("usage:") && o.stderr.is_empty(), "{help}");
        assert!(text(&o.stdout).contains("mcp [<file>] [--read-root <folder>] [--write-root <folder>]"));
    }
    for version in ["--version", "-V"] {
        let o = run(&[version], "");
        assert_eq!(code(&o), exit(Exit::Done), "{version}");
        assert_eq!(text(&o.stdout).lines().next(), Some(concat!("core-testbed ", env!("CARGO_PKG_VERSION"))));
    }
}

#[test]
fn a_wrong_command_line_is_a_usage_error_on_standard_error() {
    for bad in [&["--bogus"][..], &["verify", "one"], &["mcp", "--bogus"], &["mcp", "--read-root"], &["mcp", "a", "b"]] {
        let o = run(bad, "");
        assert_eq!(code(&o), exit(Exit::Usage), "{bad:?}");
        assert!(o.stdout.is_empty(), "{bad:?}: nothing on standard output");
        assert!(text(&o.stderr).starts_with("core-testbed: ") && text(&o.stderr).contains("usage:"), "{bad:?}: {}", text(&o.stderr));
    }
    // A root that is no folder is a wrong command line too.
    let o = run(&["mcp", "--read-root", "no-such-folder-for-the-testbed"], "");
    assert_eq!(code(&o), exit(Exit::Usage));
}

#[test]
fn a_difference_is_not_a_failure() {
    let dir = std::env::temp_dir().join(format!("core-testbed-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = |name: &str, text: &str| {
        let path = dir.join(name);
        std::fs::write(&path, text).unwrap();
        path.to_string_lossy().into_owned()
    };
    let (a, same, other) = (file("a.txt", "One two three."), file("same.md", "One two three."), file("other.txt", "One two tree."));
    let o = run(&["verify", &a, &same], "");
    assert_eq!(code(&o), exit(Exit::Done));
    assert!(text(&o.stdout).contains("every word matches (3 words)"));
    // The check ran and said no: 3, with the answer on standard output.
    let o = run(&["verify", &a, &other], "");
    assert_eq!(code(&o), exit(Exit::Differs));
    assert!(text(&o.stdout).contains("word 3 differs") && o.stderr.is_empty(), "{}", text(&o.stdout));
    // The check could not run: 1, with the reason on standard error.
    let o = run(&["verify", &a, &dir.join("missing.txt").to_string_lossy()], "");
    assert_eq!(code(&o), exit(Exit::Failed));
    assert!(o.stdout.is_empty() && text(&o.stderr).starts_with("core-testbed: "), "{}", text(&o.stderr));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn mcp_serves_until_its_input_ends_and_keeps_to_the_folders_given() {
    let request = |id: u64, method: &str, params: serde_json::Value| {
        serde_json::json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string() + "\n"
    };
    // Unconfined: said on standard error; standard output carries only the protocol.
    let o = run(&["mcp"], &(request(1, "initialize", serde_json::json!({})) + &request(2, "tools/list", serde_json::json!({}))));
    assert_eq!(code(&o), exit(Exit::Done));
    assert!(text(&o.stderr).contains("no --read-root or --write-root given"), "{}", text(&o.stderr));
    let replies: Vec<serde_json::Value> = text(&o.stdout).lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(replies.len(), 2, "one line of JSON for each request, nothing else");
    assert_eq!(replies[0]["result"]["serverInfo"]["name"], "core-testbed");
    let names: Vec<&str> = replies[1]["result"]["tools"].as_array().unwrap().iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(names.contains(&"testbed_board_addCard") && names.contains(&"testbed_note_add") && names.contains(&"execute"), "{names:?}");

    // Confined: the board's export writes inside the write folder and nowhere else.
    let dir = std::env::temp_dir().join(format!("core-testbed-mcp-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("out")).unwrap();
    let (inside, outside) = (dir.join("out/cards.json"), dir.join("cards.json"));
    let export = |id: u64, path: &std::path::Path| {
        request(id, "tools/call", serde_json::json!({"name": "testbed_board_export", "arguments": {"path": path.to_string_lossy()}}))
    };
    let o = run(&["mcp", "--write-root", &dir.join("out").to_string_lossy()], &(export(1, &inside) + &export(2, &outside)));
    assert_eq!(code(&o), exit(Exit::Done));
    assert!(!text(&o.stderr).contains("any file"), "{}", text(&o.stderr));
    let replies: Vec<serde_json::Value> = text(&o.stdout).lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!((replies[0]["result"]["isError"].as_bool(), replies[1]["result"]["isError"].as_bool()), (Some(false), Some(true)));
    assert!(inside.is_file() && !outside.exists());
    let _ = std::fs::remove_dir_all(&dir);
}
