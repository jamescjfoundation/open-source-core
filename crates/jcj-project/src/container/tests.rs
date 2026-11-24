// SPDX-License-Identifier: MIT OR Apache-2.0

use std::io::{Cursor, Write};
use std::time::{Duration, UNIX_EPOCH};

use jcj_doc::props::CharProps;
use jcj_doc::{ExtKey, Paragraph, para_block};
use serde_json::json;
use zip::write::SimpleFileOptions;

use super::*;

const APP: App = App::new("org.example.test", 3);

const DOC_TAG: ExtKey<String> = ExtKey::new("test.tag");
const PARA_STATUS: ExtKey<u32> = ExtKey::new("test.status");
const RUN_MARK: ExtKey<Vec<String>> = ExtKey::new("test.mark");

/// A document with extension data on itself, a paragraph and a run.
fn rich_doc(title: &str) -> Document {
    let mut d = Document::new();
    d.core.title = title.into();
    DOC_TAG.set(&mut d, &"draft".to_string()).unwrap();
    let mut marked = CharProps { bold: Some(true), ..CharProps::default() };
    RUN_MARK.set(&mut marked, &vec!["a".to_string(), "b".to_string()]).unwrap();
    let mut p = Paragraph::with_text("plain ", CharProps::default());
    p.insert_text(p.len(), "marked", &marked).unwrap();
    PARA_STATUS.set(&mut p, &7).unwrap();
    d.body = vec![para_block(p), para_block(Paragraph::with_text("Second paragraph.", CharProps::default()))];
    d
}

fn rich_project() -> (Project, DocId, DocId) {
    let mut p = Project::new(&APP);
    let a = p.add_document("Chapter One", rich_doc("One")).unwrap();
    let b = p.add_document("Notes \u{e9}\u{4e2d}", rich_doc("Two")).unwrap();
    p.add_asset(vec![0x89, b'P', b'N', b'G', 1, 2, 3], "png").unwrap();
    p.insert_asset("cover.jpeg", vec![0xFF, 0xD8, 0xFF]).unwrap();
    p.take_snapshot(a, "First draft", UNIX_EPOCH + Duration::from_secs(1_795_100_602)).unwrap();
    p.app_data_mut().insert("test.settings".into(), json!({"zoom": 1.5, "tags": ["x"]}));
    (p, a, b)
}

#[test]
fn everything_round_trips_with_extension_data() {
    let (p, a, b) = rich_project();
    let bytes = p.to_bytes().unwrap();
    let back = Project::from_bytes(&bytes, &APP).unwrap();
    assert_eq!(back, p);
    let doc = back.document(a).unwrap();
    assert_eq!(DOC_TAG.get(doc).unwrap().as_deref(), Some("draft"));
    let para = doc.body[0].as_para().unwrap();
    assert_eq!(PARA_STATUS.get(para).unwrap(), Some(7));
    let (_, run) = para.run_ranges().find(|(r, _)| &para.text[r.clone()] == "marked").unwrap();
    assert_eq!(RUN_MARK.get(run).unwrap(), Some(vec!["a".to_string(), "b".to_string()]));
    assert_eq!(back.name(b), Some("Notes \u{e9}\u{4e2d}"));
    assert_eq!(back.snapshots()[0].taken_utc(), "2026-11-19T15:03:22Z");
    assert_eq!(DOC_TAG.get(back.snapshots()[0].content()).unwrap().as_deref(), Some("draft"));
    // Saving what was read gives the same bytes.
    assert_eq!(back.to_bytes().unwrap(), bytes);
}

#[test]
fn ids_are_not_reused_after_a_round_trip() {
    let (mut p, a, _) = rich_project();
    p.remove_document(a);
    let mut back = Project::from_bytes(&p.to_bytes().unwrap(), &APP).unwrap();
    let c = back.add_document("New", Document::new()).unwrap();
    assert!(c.get() > 3, "{c:?}");
}

#[test]
fn the_mimetype_comes_first_uncompressed() {
    let bytes = Project::new(&APP).to_bytes().unwrap();
    assert_eq!(&bytes[..4], b"PK\x03\x04");
    assert_eq!(u16::from_le_bytes([bytes[8], bytes[9]]), 0, "stored");
    assert_eq!(u16::from_le_bytes([bytes[26], bytes[27]]), 8);
    assert_eq!(&bytes[30..38], b"mimetype");
    let extra = usize::from(u16::from_le_bytes([bytes[28], bytes[29]]));
    assert_eq!(&bytes[38 + extra..38 + extra + MIMETYPE.len()], MIMETYPE.as_bytes());
    let s = Project::peek(&bytes).unwrap();
    assert_eq!((s.container_version, s.app_id.as_str(), s.app_format, s.documents.len()), (CONTAINER_VERSION, "org.example.test", 3, 0));
}

#[test]
fn peek_reads_the_manifest_only() {
    let (p, a, b) = rich_project();
    let s = Project::peek(&p.to_bytes().unwrap()).unwrap();
    assert_eq!(s.documents, vec![(a, "Chapter One".to_string()), (b, "Notes \u{e9}\u{4e2d}".to_string())]);
    assert_eq!((s.assets, s.snapshots), (2, 1));
}

/// A version 1 container, as the draft layout wrote it.
fn v1_container() -> Vec<u8> {
    let manifest = json!({"format": 1, "app": "org.example.test", "documents": ["Intro", "Body"], "assets": ["logo.png"]});
    let d0 = serde_json::to_vec(&rich_doc("Intro")).unwrap();
    let d1 = serde_json::to_vec(&rich_doc("Body")).unwrap();
    let m = serde_json::to_vec(&manifest).unwrap();
    raw_container(&[("manifest.json", &m), ("documents/0.json", &d0), ("documents/1.json", &d1), ("assets/logo.png", b"img")])
}

#[test]
fn a_version_1_container_still_reads() {
    let bytes = v1_container();
    assert_eq!(Project::peek(&bytes).unwrap().container_version, 1);
    let p = Project::from_bytes(&bytes, &APP).unwrap();
    let names: Vec<(u32, &str)> = p.documents().map(|(id, n, _)| (id.get(), n)).collect();
    assert_eq!(names, [(1, "Intro"), (2, "Body")]);
    assert_eq!(p.document(DocId(2)).unwrap().core.title, "Body");
    assert_eq!(PARA_STATUS.get(p.document(DocId(1)).unwrap().body[0].as_para().unwrap()).unwrap(), Some(7));
    assert_eq!(p.asset("logo.png"), Some(&b"img"[..]));
    assert_eq!(p.app_format(), 3, "marked with the app's version once opened");
    // Saved again, it is a current container that reads the same.
    let again = p.to_bytes().unwrap();
    assert_eq!(Project::peek(&again).unwrap().container_version, CONTAINER_VERSION);
    assert_eq!(Project::from_bytes(&again, &APP).unwrap(), p);
}

fn migrate_v0(from: u32, p: &mut Project) -> std::result::Result<(), String> {
    if from == 0 {
        p.app_data_mut().insert("test.migrated".into(), json!(true));
        let ids = p.document_ids();
        for id in ids {
            if let Some(d) = p.document_mut(id) {
                DOC_TAG.set(d, &"migrated".to_string()).map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

fn refuse(_: u32, _: &mut Project) -> std::result::Result<(), String> {
    Err("too old".into())
}

#[test]
fn the_app_migrates_its_own_data() {
    let app = APP.with_migration(migrate_v0);
    let p = Project::from_bytes(&v1_container(), &app).unwrap();
    assert_eq!(p.app_data().get("test.migrated"), Some(&json!(true)));
    assert_eq!(DOC_TAG.get(p.document(DocId(1)).unwrap()).unwrap().as_deref(), Some("migrated"));
    assert_eq!(p.app_format(), 3);
    // The hook runs only for older data.
    let current = Project::from_bytes(&p.to_bytes().unwrap(), &APP.with_migration(refuse)).unwrap();
    assert_eq!(current, p);
    assert!(matches!(Project::from_bytes(&v1_container(), &APP.with_migration(refuse)), Err(Error::Migration(m)) if m == "too old"));
}

#[test]
fn other_apps_and_newer_versions_are_refused() {
    let bytes = rich_project().0.to_bytes().unwrap();
    assert!(matches!(Project::from_bytes(&bytes, &App::new("org.example.other", 3)), Err(Error::OtherApp { found }) if found == "org.example.test"));
    assert!(matches!(Project::from_bytes(&bytes, &App::new("org.example.test", 2)), Err(Error::NewerApp { found: 3, supported: 2 })));
    assert!(Project::from_bytes_any(&bytes, &Limits::default()).is_ok());
    let newer = raw_container(&[("manifest.json", br#"{"format": 99}"#)]);
    assert!(matches!(Project::from_bytes(&newer, &APP), Err(Error::NewerContainer(99))));
    assert!(Project::from_bytes(&newer, &APP).unwrap_err().to_string().contains("newer file format"));
}

/// A zip of exactly these entries, without the project's own mimetype unless given.
fn plain_zip(parts: &[(&str, &[u8])]) -> Vec<u8> {
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in parts {
        w.start_file(*name, SimpleFileOptions::default()).unwrap();
        w.write_all(bytes).unwrap();
    }
    w.finish().unwrap().into_inner()
}

/// A current container with one document whose JSON is `doc`.
fn with_doc_json(doc: &[u8]) -> Vec<u8> {
    let m = json!({"format": 2, "app": {"id": "org.example.test", "format": 3}, "nextId": 2, "documents": [{"id": 1, "name": "A"}]});
    raw_container(&[("manifest.json", &serde_json::to_vec(&m).unwrap()), ("documents/1.json", doc)])
}

#[test]
fn files_that_are_not_projects_are_refused() {
    assert!(matches!(Project::from_bytes(b"hello", &APP), Err(Error::NotAProject)));
    assert!(matches!(Project::from_bytes(&[], &APP), Err(Error::NotAProject)));
    assert!(matches!(Project::from_bytes(&plain_zip(&[("a.txt", b"x")]), &APP), Err(Error::NotAProject)));
    assert!(matches!(Project::from_bytes(&plain_zip(&[("mimetype", b"application/epub+zip")]), &APP), Err(Error::NotAProject)));
    // A docx-like zip with a project-looking manifest but no mimetype.
    assert!(matches!(Project::from_bytes(&plain_zip(&[("manifest.json", b"{}")]), &APP), Err(Error::NotAProject)));
}

#[test]
fn damaged_projects_are_refused_cleanly() {
    let damaged = |bytes: &[u8]| matches!(Project::from_bytes(bytes, &APP), Err(Error::Damaged(_)));
    assert!(damaged(&raw_container(&[])), "no manifest");
    assert!(damaged(&raw_container(&[("manifest.json", b"{oops")])));
    assert!(damaged(&raw_container(&[("manifest.json", b"{}")])), "no version");
    assert!(damaged(&raw_container(&[("manifest.json", br#"{"format": 0}"#)])));
    assert!(damaged(&raw_container(&[("manifest.json", br#"{"format": 2, "app": 5}"#)])));
    assert!(damaged(&raw_container(&[("manifest.json", br#"{"format": 2, "app": {"id": "../x", "format": 1}, "nextId": 1}"#)])));
    // A listed document that is missing or not a document.
    let m = json!({"format": 2, "app": {"id": "org.example.test", "format": 3}, "nextId": 2, "documents": [{"id": 1, "name": "A"}]});
    assert!(damaged(&raw_container(&[("manifest.json", &serde_json::to_vec(&m).unwrap())])));
    assert!(damaged(&with_doc_json(b"[1, 2, 3]")));
    // A nested-JSON bomb is refused, not overflowed.
    assert!(damaged(&with_doc_json("[".repeat(100_000).as_bytes())));
    // Duplicate ids, a snapshot of no document, a bad asset key.
    for m in [
        json!({"format": 2, "app": {"id": "a.b", "format": 1}, "nextId": 3, "documents": [{"id": 1, "name": "A"}, {"id": 1, "name": "B"}]}),
        json!({"format": 2, "app": {"id": "a.b", "format": 1}, "nextId": 3, "snapshots": [{"id": 2, "document": 1, "name": "S", "taken": "2026-01-01T00:00:00Z"}]}),
        json!({"format": 2, "app": {"id": "a.b", "format": 1}, "nextId": 1, "assets": ["../../etc/passwd"]}),
    ] {
        let bytes = raw_container(&[("manifest.json", &serde_json::to_vec(&m).unwrap())]);
        assert!(matches!(Project::from_bytes_any(&bytes, &Limits::default()), Err(Error::Damaged(_))), "{m}");
    }
}

#[test]
fn runs_that_do_not_fit_their_text_are_repaired() {
    let doc =
        r#"{"body": [{"kind": "para", "text": "abc", "runs": [{"len": 99}]}, {"kind": "para", "text": "été", "runs": [{"len": 1}]}]}"#.as_bytes();
    let p = Project::from_bytes(&with_doc_json(doc), &APP).unwrap();
    let d = p.document(DocId(1)).unwrap();
    assert_eq!(d.body.len(), 2);
    for b in &d.body {
        let para = b.as_para().unwrap();
        assert_eq!(para.runs.iter().map(|r| r.len).sum::<usize>(), para.text.len(), "{para:?}");
        assert!(para.run_ranges().all(|(r, _)| para.text.get(r).is_some()), "runs end on character boundaries");
    }
}

#[test]
fn limits_bound_what_is_read() {
    let (p, ..) = rich_project();
    let bytes = p.to_bytes().unwrap();
    let small = |f: fn(&mut Limits)| {
        let mut l = Limits::default();
        f(&mut l);
        Project::from_bytes_with(&bytes, &APP, &l)
    };
    assert!(matches!(small(|l| l.max_file = 100), Err(Error::TooLarge(_))));
    assert!(matches!(small(|l| l.max_entry = 64), Err(Error::TooLarge(_))));
    assert!(matches!(small(|l| l.max_total = 1000), Err(Error::TooLarge(_))));
    assert!(matches!(small(|l| l.max_entries = 3), Err(Error::TooLarge(_))));
    assert!(matches!(small(|l| l.max_documents = 1), Err(Error::TooLarge(_))));
    assert!(matches!(small(|l| l.max_snapshots = 0), Err(Error::TooLarge(_))));
    assert!(matches!(small(|l| l.max_assets = 1), Err(Error::TooLarge(_))));
    assert!(small(|_| {}).is_ok());
}

#[test]
fn an_entry_that_lies_about_its_size_is_cut_off() {
    // A highly compressible entry: small on disk, large once inflated.
    let big = vec![b' '; 4 << 20];
    let bytes = with_doc_json(&big);
    assert!(bytes.len() < 64 << 10);
    let l = Limits { max_entry: 1 << 20, ..Limits::default() };
    assert!(matches!(Project::from_bytes_with(&bytes, &APP, &l), Err(Error::TooLarge(_))));
}

#[test]
fn hostile_bytes_never_panic() {
    let (p, ..) = rich_project();
    let bytes = p.to_bytes().unwrap();
    // Truncated anywhere.
    for cut in (0..bytes.len()).step_by(37) {
        let _ = Project::from_bytes(&bytes[..cut], &APP);
    }
    // Any byte changed.
    for at in (0..bytes.len()).step_by(11) {
        let mut b = bytes.clone();
        b[at] ^= 0x5A;
        let _ = Project::from_bytes(&b, &APP);
        let _ = Project::peek(&b);
    }
}

#[test]
fn saving_refuses_what_it_could_not_read_back() {
    let mut p = Project::new(&App::new("not valid!", 1));
    assert!(matches!(p.to_bytes(), Err(Error::Invalid(_))));
    p.app_id = "ok.app".into();
    assert!(p.to_bytes().is_ok());
}
