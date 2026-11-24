// SPDX-License-Identifier: MIT OR Apache-2.0
//! The project in memory: documents, assets, snapshots and the app's data.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use jcj_doc::Document;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{Error, Result};

/// The longest document or snapshot name, bytes.
pub const MAX_NAME_LEN: usize = 1024;
/// The longest app id or asset key, bytes.
pub const MAX_KEY_LEN: usize = 128;

/// Whether `key` may name an app or an asset: 1 to [`MAX_KEY_LEN`] bytes of ASCII letters,
/// digits, `.`, `_` and `-`, not starting with `.` (so it is a safe file name in the container).
pub fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= MAX_KEY_LEN
        && !key.starts_with('.')
        && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-')
}

/// An app's hook that brings a project saved by an older version of the app up to date.
///
/// It is called once, after the container is read, with the app data version the file was
/// saved with (`from`, lower than [`App::format`]) and the whole project: documents (with their
/// extension data), snapshots, assets and [`Project::app_data`]. It changes what it must and
/// returns; the project is then marked with the app's current version. An `Err` refuses the file
/// with [`Error::Migration`]. Write one step per version (`from` 1 → 2 → 3 ...) so every older
/// file stays readable.
pub type MigrateFn = fn(from: u32, project: &mut Project) -> std::result::Result<(), String>;

/// The app a project belongs to: its id, the version of its own data, and its migration hook.
#[derive(Clone, Copy, Debug)]
pub struct App {
    id: &'static str,
    format: u32,
    migrate: Option<MigrateFn>,
}

impl App {
    /// An app with id `id` (reverse-domain style is suggested: `org.example.notes`; see
    /// [`valid_key`]) whose own data is at version `format`.
    pub const fn new(id: &'static str, format: u32) -> App {
        App { id, format, migrate: None }
    }

    /// This app with a migration hook for projects saved by its older versions. Without one,
    /// such projects open as they are.
    pub const fn with_migration(self, migrate: MigrateFn) -> App {
        App { migrate: Some(migrate), ..self }
    }

    /// The app's id.
    pub fn id(&self) -> &'static str {
        self.id
    }

    /// The version of the app's own data this build writes.
    pub fn format(&self) -> u32 {
        self.format
    }

    pub(crate) fn migration(&self) -> Option<MigrateFn> {
        self.migrate
    }
}

/// A document's id in its project: stable across renames, moves and saves.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DocId(pub(crate) u32);

impl DocId {
    /// The number, as the file stores it.
    pub fn get(self) -> u32 {
        self.0
    }
}

/// A snapshot's id in its project.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SnapshotId(pub(crate) u32);

impl SnapshotId {
    /// The number, as the file stores it.
    pub fn get(self) -> u32 {
        self.0
    }
}

/// A named, dated copy of one document.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub(crate) id: SnapshotId,
    pub(crate) document: DocId,
    pub(crate) name: String,
    /// Seconds since 1970-01-01 UTC.
    pub(crate) taken: i64,
    pub(crate) content: Document,
}

impl Snapshot {
    /// The snapshot's id.
    pub fn id(&self) -> SnapshotId {
        self.id
    }
    /// The document it is a copy of.
    pub fn document(&self) -> DocId {
        self.document
    }
    /// Its name.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// When it was taken.
    pub fn taken(&self) -> SystemTime {
        match u64::try_from(self.taken) {
            Ok(s) => UNIX_EPOCH + std::time::Duration::from_secs(s),
            Err(_) => UNIX_EPOCH,
        }
    }
    /// When it was taken, as RFC 3339 in UTC (`2026-11-24T15:03:22Z`).
    pub fn taken_utc(&self) -> String {
        rfc3339(self.taken)
    }
    /// The document as it was.
    pub fn content(&self) -> &Document {
        &self.content
    }
}

/// One document of a project.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Entry {
    pub(crate) id: DocId,
    pub(crate) name: String,
    pub(crate) doc: Document,
}

/// A project: documents in order, assets, snapshots and the app's own data.
#[derive(Clone, Debug, PartialEq)]
pub struct Project {
    pub(crate) app_id: String,
    pub(crate) app_format: u32,
    pub(crate) next_id: u32,
    pub(crate) documents: Vec<Entry>,
    pub(crate) assets: BTreeMap<String, Arc<Vec<u8>>>,
    pub(crate) snapshots: Vec<Snapshot>,
    pub(crate) app_data: BTreeMap<String, Value>,
}

/// Checks a document or snapshot name.
fn check_name(name: &str) -> Result<()> {
    if name.len() > MAX_NAME_LEN {
        return Err(Error::Invalid(format!("a name may be at most {MAX_NAME_LEN} bytes")));
    }
    Ok(())
}

impl Project {
    /// An empty project of `app` (no documents yet).
    pub fn new(app: &App) -> Project {
        Project {
            app_id: app.id.to_string(),
            app_format: app.format,
            next_id: 1,
            documents: Vec::new(),
            assets: BTreeMap::new(),
            snapshots: Vec::new(),
            app_data: BTreeMap::new(),
        }
    }

    /// A project of `app` holding one document.
    pub fn with_document(app: &App, name: &str, doc: Document) -> Result<Project> {
        let mut p = Project::new(app);
        p.add_document(name, doc)?;
        Ok(p)
    }

    /// The id of the app the project belongs to.
    pub fn app_id(&self) -> &str {
        &self.app_id
    }

    /// The version of the app's data the project holds.
    pub fn app_format(&self) -> u32 {
        self.app_format
    }

    fn take_id(&mut self) -> Result<u32> {
        let id = self.next_id;
        self.next_id = id.checked_add(1).ok_or_else(|| Error::Invalid("the project has run out of ids".into()))?;
        Ok(id)
    }

    // ---- Documents -----------------------------------------------------------------------

    /// The documents' ids, in project order.
    pub fn document_ids(&self) -> Vec<DocId> {
        self.documents.iter().map(|e| e.id).collect()
    }

    /// How many documents the project holds.
    pub fn document_count(&self) -> usize {
        self.documents.len()
    }

    /// The documents in project order: id, name and document.
    pub fn documents(&self) -> impl Iterator<Item = (DocId, &str, &Document)> {
        self.documents.iter().map(|e| (e.id, e.name.as_str(), &e.doc))
    }

    fn entry(&self, id: DocId) -> Option<&Entry> {
        self.documents.iter().find(|e| e.id == id)
    }

    fn entry_mut(&mut self, id: DocId) -> Option<&mut Entry> {
        self.documents.iter_mut().find(|e| e.id == id)
    }

    /// A document.
    pub fn document(&self, id: DocId) -> Option<&Document> {
        self.entry(id).map(|e| &e.doc)
    }

    /// A document, to change.
    pub fn document_mut(&mut self, id: DocId) -> Option<&mut Document> {
        self.entry_mut(id).map(|e| &mut e.doc)
    }

    /// Replace a document's content, keeping its id, name and place.
    pub fn set_document(&mut self, id: DocId, doc: Document) -> Result<()> {
        let e = self.entry_mut(id).ok_or_else(|| unknown_doc(id))?;
        e.doc = doc;
        Ok(())
    }

    /// A document's name.
    pub fn name(&self, id: DocId) -> Option<&str> {
        self.entry(id).map(|e| e.name.as_str())
    }

    /// The first document named `name`.
    pub fn find_document(&self, name: &str) -> Option<DocId> {
        self.documents.iter().find(|e| e.name == name).map(|e| e.id)
    }

    /// Add a document at the end; names need not be unique.
    pub fn add_document(&mut self, name: &str, doc: Document) -> Result<DocId> {
        self.insert_document(self.documents.len(), name, doc)
    }

    /// Add a document at `index` (clamped to the end).
    pub fn insert_document(&mut self, index: usize, name: &str, doc: Document) -> Result<DocId> {
        check_name(name)?;
        let id = DocId(self.take_id()?);
        let at = index.min(self.documents.len());
        self.documents.insert(at, Entry { id, name: name.to_string(), doc });
        Ok(id)
    }

    /// Rename a document.
    pub fn rename_document(&mut self, id: DocId, name: &str) -> Result<()> {
        check_name(name)?;
        let e = self.entry_mut(id).ok_or_else(|| unknown_doc(id))?;
        e.name = name.to_string();
        Ok(())
    }

    /// Move a document to `index` (clamped to the end). False when there is no such document.
    pub fn move_document(&mut self, id: DocId, index: usize) -> bool {
        let Some(from) = self.documents.iter().position(|e| e.id == id) else { return false };
        let e = self.documents.remove(from);
        let at = index.min(self.documents.len());
        self.documents.insert(at, e);
        true
    }

    /// Remove a document and its snapshots; returns the document.
    pub fn remove_document(&mut self, id: DocId) -> Option<Document> {
        let at = self.documents.iter().position(|e| e.id == id)?;
        self.snapshots.retain(|s| s.document != id);
        Some(self.documents.remove(at).doc)
    }

    // ---- Assets --------------------------------------------------------------------------

    /// Add media by content: the key is made from its SHA-256 and `ext` (`png`, `jpeg`...), so
    /// adding the same bytes twice keeps one copy. Returns the key.
    pub fn add_asset(&mut self, bytes: Vec<u8>, ext: &str) -> Result<String> {
        let digest = Sha256::digest(&bytes);
        let hex: String = digest.iter().take(16).map(|b| format!("{b:02x}")).collect();
        let ext: String = ext.trim_start_matches('.').chars().filter(char::is_ascii_alphanumeric).take(16).collect::<String>().to_ascii_lowercase();
        let key = if ext.is_empty() { hex } else { format!("{hex}.{ext}") };
        self.insert_asset(&key, bytes)?;
        Ok(key)
    }

    /// Store media under a key the app chooses ([`valid_key`]), replacing any already there.
    pub fn insert_asset(&mut self, key: &str, bytes: Vec<u8>) -> Result<()> {
        if !valid_key(key) {
            return Err(Error::Invalid(format!("`{key}` is not a valid asset key")));
        }
        self.assets.insert(key.to_string(), Arc::new(bytes));
        Ok(())
    }

    /// An asset's bytes.
    pub fn asset(&self, key: &str) -> Option<&[u8]> {
        self.assets.get(key).map(|b| b.as_slice())
    }

    /// The assets' keys, sorted.
    pub fn asset_keys(&self) -> impl Iterator<Item = &str> {
        self.assets.keys().map(String::as_str)
    }

    /// Remove an asset. False when there was none.
    pub fn remove_asset(&mut self, key: &str) -> bool {
        self.assets.remove(key).is_some()
    }

    // ---- App data ------------------------------------------------------------------------

    /// The app's own data for the whole project, by key (namespace them: `notes.tags`). Saved as
    /// JSON with the project.
    pub fn app_data(&self) -> &BTreeMap<String, Value> {
        &self.app_data
    }

    /// The app's own data, to change.
    pub fn app_data_mut(&mut self) -> &mut BTreeMap<String, Value> {
        &mut self.app_data
    }

    // ---- Snapshots -----------------------------------------------------------------------

    /// Take a snapshot of document `doc`: a copy named `name`, dated `at`.
    pub fn take_snapshot(&mut self, doc: DocId, name: &str, at: SystemTime) -> Result<SnapshotId> {
        check_name(name)?;
        let content = self.document(doc).ok_or_else(|| unknown_doc(doc))?.clone();
        let id = SnapshotId(self.take_id()?);
        let taken = at.duration_since(UNIX_EPOCH).map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
        self.snapshots.push(Snapshot { id, document: doc, name: name.to_string(), taken, content });
        Ok(id)
    }

    /// Every snapshot, oldest taken first.
    pub fn snapshots(&self) -> &[Snapshot] {
        &self.snapshots
    }

    /// The snapshots of one document, oldest taken first.
    pub fn snapshots_of(&self, doc: DocId) -> impl Iterator<Item = &Snapshot> {
        self.snapshots.iter().filter(move |s| s.document == doc)
    }

    /// A snapshot.
    pub fn snapshot(&self, id: SnapshotId) -> Option<&Snapshot> {
        self.snapshots.iter().find(|s| s.id == id)
    }

    /// Rename a snapshot.
    pub fn rename_snapshot(&mut self, id: SnapshotId, name: &str) -> Result<()> {
        check_name(name)?;
        let s = self.snapshots.iter_mut().find(|s| s.id == id).ok_or_else(|| Error::Invalid(format!("no snapshot {}", id.0)))?;
        s.name = name.to_string();
        Ok(())
    }

    /// Put a snapshot's content back into its document. The current content is not kept: take a
    /// snapshot first to keep it.
    pub fn restore_snapshot(&mut self, id: SnapshotId) -> Result<()> {
        let s = self.snapshot(id).ok_or_else(|| Error::Invalid(format!("no snapshot {}", id.0)))?;
        let (doc, content) = (s.document, s.content.clone());
        self.set_document(doc, content)
    }

    /// Remove a snapshot. False when there was none.
    pub fn remove_snapshot(&mut self, id: SnapshotId) -> bool {
        let before = self.snapshots.len();
        self.snapshots.retain(|s| s.id != id);
        self.snapshots.len() != before
    }
}

fn unknown_doc(id: DocId) -> Error {
    Error::Invalid(format!("no document {}", id.0))
}

/// Seconds since 1970 as RFC 3339 in UTC.
pub(crate) fn rfc3339(secs: i64) -> String {
    chrono::DateTime::from_timestamp(secs, 0).unwrap_or_default().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// RFC 3339 back to seconds since 1970.
pub(crate) fn parse_rfc3339(s: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(s).ok().map(|t| t.timestamp())
}

#[cfg(test)]
mod tests {
    use super::*;

    const APP: App = App::new("test.app", 1);

    #[test]
    fn documents_keep_their_ids_through_moves_and_renames() {
        let mut p = Project::new(&APP);
        let a = p.add_document("A", Document::new()).unwrap();
        let b = p.add_document("B", Document::new()).unwrap();
        let c = p.insert_document(0, "C", Document::new()).unwrap();
        assert_eq!(p.document_ids(), [c, a, b]);
        assert!(p.move_document(c, 99));
        assert_eq!(p.document_ids(), [a, b, c]);
        p.rename_document(a, "First").unwrap();
        assert_eq!(p.find_document("First"), Some(a));
        assert!(p.rename_document(DocId(99), "x").is_err());
        assert!(p.rename_document(a, &"x".repeat(MAX_NAME_LEN + 1)).is_err());
    }

    #[test]
    fn assets_are_keyed_by_content_and_keys_are_checked() {
        let mut p = Project::new(&APP);
        let k1 = p.add_asset(b"picture".to_vec(), ".PNG").unwrap();
        let k2 = p.add_asset(b"picture".to_vec(), "png").unwrap();
        assert_eq!(k1, k2);
        assert!(k1.ends_with(".png") && k1.len() == 36);
        assert_eq!(p.asset_keys().count(), 1);
        for bad in ["", "../x", ".hidden", "a/b", "a b", &"k".repeat(MAX_KEY_LEN + 1)] {
            assert!(p.insert_asset(bad, vec![]).is_err(), "{bad:?}");
        }
        assert!(p.remove_asset(&k1) && !p.remove_asset(&k1));
    }

    #[test]
    fn snapshots_copy_restore_and_go_with_their_document() {
        let mut p = Project::new(&APP);
        let mut d = Document::new();
        d.core.title = "One".into();
        let id = p.add_document("Doc", d).unwrap();
        let at = UNIX_EPOCH + std::time::Duration::from_secs(1_795_100_602);
        let s = p.take_snapshot(id, "Before", at).unwrap();
        p.document_mut(id).unwrap().core.title = "Two".into();
        assert_eq!(p.snapshot(s).unwrap().content().core.title, "One");
        assert_eq!(p.snapshot(s).unwrap().taken_utc(), "2026-11-19T15:03:22Z");
        assert_eq!(p.snapshot(s).unwrap().taken(), at);
        p.restore_snapshot(s).unwrap();
        assert_eq!(p.document(id).unwrap().core.title, "One");
        assert_eq!(p.snapshots_of(id).count(), 1);
        p.remove_document(id);
        assert!(p.snapshots().is_empty());
    }
}
