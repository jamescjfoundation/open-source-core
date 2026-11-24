// SPDX-License-Identifier: MIT OR Apache-2.0
use std::sync::Arc;

use jcj_doc::{Document, ExtKey, HasExt};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{CmdError, CmdResult, CommandSpec, Session, commands_in, duplicate_ids, native, registry_with, session_with, try_registry_with};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Status {
    label: String,
}

const STATUS: ExtKey<Status> = ExtKey::new("test.status");
const TITLE: ExtKey<String> = ExtKey::new("test.title");

fn set_status(s: &mut Session, v: &Value) -> CmdResult {
    let label = crate::params::str(v, "label").ok_or_else(|| CmdError::Params("`label` is required".into()))?.to_string();
    let Some(jcj_doc::Block::Para(p)) = s.doc.body.get_mut(0).map(Arc::make_mut) else { return Err(CmdError::Failed("no paragraph".into())) };
    STATUS.set(p, &Status { label }).map_err(|e| CmdError::Failed(e.to_string()))?;
    Ok(Value::Null)
}

fn set_title(s: &mut Session, v: &Value) -> CmdResult {
    let title = crate::params::str(v, "title").unwrap_or_default().to_string();
    TITLE.set(&mut s.doc, &title).map_err(|e| CmdError::Failed(e.to_string()))?;
    Ok(Value::Null)
}

fn app_specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("test.setStatus", "Set Status", "Test", set_status).params(r#"{"label": string}"#),
        CommandSpec::new("test.setTitle", "Set Title", "Test", set_title),
        CommandSpec::new("test.status", "Status", "Test", |s, _| {
            Ok(json!(STATUS.get_or_none(s.doc.body[0].as_para().unwrap_or(&Default::default())).map(|st| st.label)))
        })
        .pure(),
    ]
}

fn session() -> Session {
    session_with(Document::new(), Arc::new(registry_with(app_specs())))
}

#[test]
fn an_edit_to_extension_data_through_a_command_is_undoable() {
    let mut s = session();
    s.run("test.setStatus", &json!({"label": "Draft"})).unwrap();
    assert_eq!(s.run("test.status", &json!({})).unwrap(), json!("Draft"));
    s.run("test.setStatus", &json!({"label": "Final"})).unwrap();
    assert!(s.dirty);
    assert!(s.can_undo());
    assert_eq!(s.undo_label(), Some("Set Status"));
    s.run("edit.undo", &json!({})).unwrap();
    assert_eq!(s.run("test.status", &json!({})).unwrap(), json!("Draft"));
    s.run("edit.undo", &json!({})).unwrap();
    assert_eq!(s.run("test.status", &json!({})).unwrap(), Value::Null);
    s.run("edit.redo", &json!({})).unwrap();
    assert_eq!(s.run("test.status", &json!({})).unwrap(), json!("Draft"));
    // On the document too.
    s.run("test.setTitle", &json!({"title": "A"})).unwrap();
    assert_eq!(TITLE.get_or_none(&s.doc).as_deref(), Some("A"));
    assert!(s.undo());
    assert!(!TITLE.is_set(&s.doc));
}

#[test]
fn a_failed_command_changes_nothing_and_adds_no_undo_step() {
    let mut s = session();
    let depth = s.undo_depth();
    let err = s.run("test.setStatus", &json!({})).unwrap_err();
    assert!(matches!(err, CmdError::Params(_)));
    assert_eq!(s.undo_depth(), depth);
    assert!(!STATUS.is_set(s.doc.body[0].as_para().unwrap()));
    assert!(matches!(s.run("no.suchCommand", &json!({})), Err(CmdError::Unknown(_))));
}

#[test]
fn an_app_registry_holds_the_engines_commands_and_its_own() {
    let r = registry_with(app_specs());
    assert!(r.get("edit.undo").is_some());
    assert!(r.get("test.setStatus").is_some());
    assert!(duplicate_ids(r.all()).is_empty());
    assert_eq!(commands_in(&r, "test.").len(), 3);
    // An app command replaces the engine's command with its id.
    let undo = CommandSpec::new("edit.undo", "Undo Mine", "Test", |_, _| Ok(json!("mine"))).pure();
    let r = registry_with(vec![undo]);
    assert_eq!(r.get("edit.undo").map(|s| s.label), Some("Undo Mine"));
    assert!(duplicate_ids(r.all()).is_empty());
}

#[test]
fn an_app_command_registered_twice_is_reported() {
    let mut specs = app_specs();
    specs.push(CommandSpec::new("test.setTitle", "Again", "Test", set_title));
    assert_eq!(try_registry_with(specs).err(), Some(vec!["test.setTitle"]));
    assert!(try_registry_with(app_specs()).is_ok());
}

#[test]
fn native_json_keeps_extension_data() {
    let mut s = session();
    s.run("test.setStatus", &json!({"label": "Kept"})).unwrap();
    s.run("test.setTitle", &json!({"title": "Kept too"})).unwrap();
    let bytes = native::save(&s.doc).unwrap();
    let back = native::open(&bytes).unwrap();
    assert_eq!(STATUS.get_or_none(back.body[0].as_para().unwrap()), Some(Status { label: "Kept".into() }));
    assert_eq!(TITLE.get_or_none(&back).as_deref(), Some("Kept too"));
    assert!(back.ext().contains("test.title"));
    assert!(native::open(b"{not json").is_err());
}

#[test]
fn the_command_helpers_answer() {
    let s = session();
    let sel = crate::cmd::sel_result(&s).unwrap();
    assert_eq!(sel["anchor"]["off"], json!(0));
    assert_eq!(crate::cmd::civil_from_days(0), (1970, 1, 1));
    assert!(crate::cmd::now_unix() > 0);
    assert_eq!(crate::cmd::parse_pos(&json!({"block": 0, "off": 2})), Some(jcj_doc::Pos::body(0, 2)));
}

#[test]
fn recovery_starts_with_nothing_to_recover() {
    let dir = std::env::temp_dir().join(format!("jcj-engine-recovery-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = session();
    crate::recovery::enable(&mut s, dir.clone(), Arc::new(|| {})).unwrap();
    crate::recovery::tick(&mut s);
    assert!(crate::recovery::documents(&s).is_empty());
    assert!(crate::recovery::error(&s).is_none());
    crate::recovery::finish(&mut s);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn file_access_reports_the_folders_in_force() {
    use crate::file_access::{AutomationRoots, confine, current, unconfined};
    assert!(current().is_none(), "nothing is confined by default");
    let dir = std::env::temp_dir();
    let roots = AutomationRoots::new(Some(&dir), None).ok().flatten();
    assert!(roots.is_some());
    confine(roots.as_ref(), || {
        assert!(current().is_some(), "inside confine the roots are in force");
        unconfined(|| assert!(current().is_none(), "unconfined lifts them"));
    });
    assert!(current().is_none());
}
