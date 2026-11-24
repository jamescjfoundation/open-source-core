// SPDX-License-Identifier: MIT OR Apache-2.0
//! The project file: a zip container with a manifest (`FORMAT.md` is the specification).
//!
//! Writing is deterministic (fixed entry times, sorted JSON), so saving an unchanged project
//! gives the same bytes. Reading is defensive: every size and count is bounded, a container from
//! a newer version is refused with a clear message, older containers are migrated step by step,
//! and nothing in the file can make the reader panic.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Read, Write};
use std::sync::Arc;

use jcj_doc::Document;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::project::{App, DocId, Entry, MAX_NAME_LEN, Project, Snapshot, SnapshotId, parse_rfc3339, rfc3339, valid_key};
use crate::{Error, Result};

/// The container version this build writes. Older versions are read and migrated.
pub const CONTAINER_VERSION: u32 = 2;
/// The first entry of every project file, stored uncompressed, as ODF and EPUB do.
pub const MIMETYPE: &str = "application/vnd.jcj.project+zip";

const MIMETYPE_ENTRY: &str = "mimetype";
const MANIFEST: &str = "manifest.json";
const APP_DATA: &str = "app.json";
/// The largest manifest or app data part, bytes.
const MAX_MANIFEST: u64 = 16 << 20;

/// Bounds on what a project file may hold, checked while reading (and when saving, so the core
/// never writes a file it would refuse).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// The largest project file, bytes.
    pub max_file: u64,
    /// The largest single entry once uncompressed, bytes.
    pub max_entry: u64,
    /// The most bytes uncompressed in all, over every entry read.
    pub max_total: u64,
    /// The most zip entries.
    pub max_entries: u64,
    /// The most documents.
    pub max_documents: usize,
    /// The most snapshots.
    pub max_snapshots: usize,
    /// The most assets.
    pub max_assets: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_file: 1 << 30,
            max_entry: 256 << 20,
            max_total: 2 << 30,
            max_entries: 40_000,
            max_documents: 10_000,
            max_snapshots: 20_000,
            max_assets: 10_000,
        }
    }
}

/// What a project file holds, read from its manifest alone ([`Project::peek`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Summary {
    /// The container version the file was written in.
    pub container_version: u32,
    /// The app the project belongs to.
    pub app_id: String,
    /// The version of the app's data in it.
    pub app_format: u32,
    /// Its documents in order: id and name.
    pub documents: Vec<(DocId, String)>,
    /// How many assets it holds.
    pub assets: usize,
    /// How many snapshots it holds.
    pub snapshots: usize,
}

// ---- The manifest, current version ----------------------------------------------------------

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    format: u32,
    app: AppRecord,
    next_id: u32,
    #[serde(default)]
    documents: Vec<DocRecord>,
    #[serde(default)]
    assets: Vec<String>,
    #[serde(default)]
    snapshots: Vec<SnapRecord>,
}

#[derive(Serialize, Deserialize)]
struct AppRecord {
    id: String,
    format: u32,
}

#[derive(Serialize, Deserialize)]
struct DocRecord {
    id: DocId,
    name: String,
}

#[derive(Serialize, Deserialize)]
struct SnapRecord {
    id: SnapshotId,
    document: DocId,
    name: String,
    taken: String,
}

/// Version 1 (the draft layout): documents by position, no ids, no snapshots, no app version.
#[derive(Deserialize)]
struct ManifestV1 {
    app: String,
    #[serde(default)]
    documents: Vec<String>,
    #[serde(default)]
    assets: Vec<String>,
}

fn doc_entry(id: DocId) -> String {
    format!("documents/{}.json", id.0)
}

fn snapshot_entry(id: SnapshotId) -> String {
    format!("snapshots/{}.json", id.0)
}

fn asset_entry(key: &str) -> String {
    format!("assets/{key}")
}

/// A manifest brought up to the current version, with the entry each document is read from.
struct Loaded {
    manifest: Manifest,
    doc_entries: Vec<String>,
}

/// Bring a manifest from container version `from` up to [`CONTAINER_VERSION`]. Each version is
/// one step, so a future change adds a step and every older file stays readable.
fn migrate(from: u32, manifest: Value) -> Result<Loaded> {
    match from {
        // 1 → 2: documents get ids (1, 2, ... in order) and the app a data version (0); their
        // entries stay where version 1 put them (`documents/<position>.json`).
        1 => {
            let v1: ManifestV1 = serde_json::from_value(manifest).map_err(|e| Error::Damaged(format!("manifest: {e}")))?;
            let n = u32::try_from(v1.documents.len()).map_err(|_| Error::TooLarge("too many documents".into()))?;
            let documents: Vec<DocRecord> = v1.documents.into_iter().zip(1..).map(|(name, id)| DocRecord { id: DocId(id), name }).collect();
            let doc_entries = (0..documents.len()).map(|i| format!("documents/{i}.json")).collect();
            let manifest = Manifest {
                format: CONTAINER_VERSION,
                app: AppRecord { id: v1.app, format: 0 },
                next_id: n.saturating_add(1),
                documents,
                assets: v1.assets,
                snapshots: Vec::new(),
            };
            Ok(Loaded { manifest, doc_entries })
        }
        CONTAINER_VERSION => {
            let manifest: Manifest = serde_json::from_value(manifest).map_err(|e| Error::Damaged(format!("manifest: {e}")))?;
            let doc_entries = manifest.documents.iter().map(|d| doc_entry(d.id)).collect();
            Ok(Loaded { manifest, doc_entries })
        }
        v if v > CONTAINER_VERSION => Err(Error::NewerContainer(v)),
        v => Err(Error::Damaged(format!("unknown container version {v}"))),
    }
}

// ---- Reading -----------------------------------------------------------------------------------

/// An open container and what has been read from it so far.
struct Reader<'a> {
    zip: ZipArchive<Cursor<&'a [u8]>>,
    limits: Limits,
    read: u64,
}

impl<'a> Reader<'a> {
    fn open(bytes: &'a [u8], limits: &Limits) -> Result<Reader<'a>> {
        if bytes.len() as u64 > limits.max_file {
            return Err(Error::TooLarge(format!("the file is larger than {} MB", limits.max_file >> 20)));
        }
        if !bytes.starts_with(b"PK\x03\x04") {
            return Err(Error::NotAProject);
        }
        // Count the entries the zip declares before `zip` parses them all.
        jcj_upstream::doc::zip_guard::check(bytes, limits.max_entries).map_err(|e| Error::TooLarge(e.to_string()))?;
        let zip = ZipArchive::new(Cursor::new(bytes)).map_err(|e| Error::Damaged(format!("zip: {e}")))?;
        if zip.len() as u64 > limits.max_entries {
            return Err(Error::TooLarge(format!("more than {} entries", limits.max_entries)));
        }
        let mut r = Reader { zip, limits: *limits, read: 0 };
        match r.entry(MIMETYPE_ENTRY, 256)? {
            Some(m) if m == MIMETYPE.as_bytes() => Ok(r),
            _ => Err(Error::NotAProject),
        }
    }

    /// An entry's bytes, `None` when there is none. Never reads more than `limit` bytes of it, nor
    /// more than the total budget, whatever the entry's header says.
    fn entry(&mut self, name: &str, limit: u64) -> Result<Option<Vec<u8>>> {
        let limit = limit.min(self.limits.max_entry);
        let mut file = match self.zip.by_name(name) {
            Ok(f) => f,
            Err(zip::result::ZipError::FileNotFound) => return Ok(None),
            Err(e) => return Err(Error::Damaged(format!("{name}: {e}"))),
        };
        if file.size() > limit {
            return Err(Error::TooLarge(format!("{name} is larger than {} MB", limit >> 20)));
        }
        let mut out = Vec::with_capacity(usize::try_from(file.size()).unwrap_or(0).min(1 << 20));
        (&mut file).take(limit + 1).read_to_end(&mut out).map_err(|e| Error::Damaged(format!("{name}: {e}")))?;
        if out.len() as u64 > limit {
            return Err(Error::TooLarge(format!("{name} is larger than {} MB", limit >> 20)));
        }
        self.read += out.len() as u64;
        if self.read > self.limits.max_total {
            return Err(Error::TooLarge(format!("more than {} MB in all", self.limits.max_total >> 20)));
        }
        Ok(Some(out))
    }

    fn required(&mut self, name: &str, limit: u64) -> Result<Vec<u8>> {
        self.entry(name, limit)?.ok_or_else(|| Error::Damaged(format!("{name} is missing")))
    }

    /// The manifest at the current version.
    fn manifest(&mut self) -> Result<(u32, Loaded)> {
        let bytes = self.required(MANIFEST, MAX_MANIFEST)?;
        let value: Value = serde_json::from_slice(&bytes).map_err(|e| Error::Damaged(format!("manifest: {e}")))?;
        let version = value
            .get("format")
            .and_then(Value::as_u64)
            .and_then(|v| u32::try_from(v).ok())
            .ok_or_else(|| Error::Damaged("the manifest has no container version".into()))?;
        let loaded = migrate(version, value)?;
        check_manifest(&loaded.manifest, &self.limits)?;
        Ok((version, loaded))
    }

    fn document(&mut self, entry: &str, what: &str) -> Result<Document> {
        let bytes = self.required(entry, u64::MAX)?;
        let mut doc: Document = serde_json::from_slice(&bytes).map_err(|e| Error::Damaged(format!("{what}: {e}")))?;
        // As WordCraft reads its native JSON: runs that don't fit the text are repaired.
        doc.normalize_runs();
        doc.ensure_nonempty();
        Ok(doc)
    }
}

/// Counts, names, keys and ids of a manifest are within bounds and consistent.
fn check_manifest(m: &Manifest, limits: &Limits) -> Result<()> {
    if m.documents.len() > limits.max_documents {
        return Err(Error::TooLarge(format!("more than {} documents", limits.max_documents)));
    }
    if m.snapshots.len() > limits.max_snapshots {
        return Err(Error::TooLarge(format!("more than {} snapshots", limits.max_snapshots)));
    }
    if m.assets.len() > limits.max_assets {
        return Err(Error::TooLarge(format!("more than {} assets", limits.max_assets)));
    }
    if !valid_key(&m.app.id) {
        return Err(Error::Damaged("the app id is not valid".into()));
    }
    let mut ids = BTreeSet::new();
    for d in &m.documents {
        if d.name.len() > MAX_NAME_LEN || !ids.insert(d.id.0) {
            return Err(Error::Damaged(format!("document {} is listed badly", d.id.0)));
        }
    }
    for s in &m.snapshots {
        if s.name.len() > MAX_NAME_LEN || !ids.insert(s.id.0) || !m.documents.iter().any(|d| d.id == s.document) {
            return Err(Error::Damaged(format!("snapshot {} is listed badly", s.id.0)));
        }
    }
    let mut keys = BTreeSet::new();
    for k in &m.assets {
        if !valid_key(k) || !keys.insert(k.as_str()) {
            return Err(Error::Damaged(format!("asset `{}` is listed badly", k.chars().take(64).collect::<String>())));
        }
    }
    Ok(())
}

impl Project {
    /// Read a project of `app` from a file's bytes, with the default [`Limits`].
    pub fn from_bytes(bytes: &[u8], app: &App) -> Result<Project> {
        Project::from_bytes_with(bytes, app, &Limits::default())
    }

    /// [`Project::from_bytes`] within `limits`.
    ///
    /// A project of another app is refused ([`Error::OtherApp`]), as is one saved by a newer
    /// version of the app ([`Error::NewerApp`]). One saved by an older version is passed to the
    /// app's migration hook ([`App::with_migration`]).
    pub fn from_bytes_with(bytes: &[u8], app: &App, limits: &Limits) -> Result<Project> {
        let mut project = read(bytes, limits, Some(app))?;
        if project.app_format < app.format() {
            if let Some(migrate) = app.migration() {
                migrate(project.app_format, &mut project).map_err(Error::Migration)?;
            }
            project.app_format = app.format();
        }
        Ok(project)
    }

    /// Read a project whatever app it belongs to, without migrating the app's data: for tools
    /// that inspect or convert projects.
    pub fn from_bytes_any(bytes: &[u8], limits: &Limits) -> Result<Project> {
        read(bytes, limits, None)
    }

    /// What a project file holds, from its manifest alone (no documents are read).
    pub fn peek(bytes: &[u8]) -> Result<Summary> {
        let mut r = Reader::open(bytes, &Limits::default())?;
        let (version, Loaded { manifest: m, .. }) = r.manifest()?;
        Ok(Summary {
            container_version: version,
            app_id: m.app.id,
            app_format: m.app.format,
            documents: m.documents.into_iter().map(|d| (d.id, d.name)).collect(),
            assets: m.assets.len(),
            snapshots: m.snapshots.len(),
        })
    }

    /// The project file's bytes, in the current container version.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        write(self)
    }
}

fn read(bytes: &[u8], limits: &Limits, app: Option<&App>) -> Result<Project> {
    let mut r = Reader::open(bytes, limits)?;
    let (_, Loaded { manifest: m, doc_entries }) = r.manifest()?;
    if let Some(app) = app {
        if m.app.id != app.id() {
            return Err(Error::OtherApp { found: m.app.id });
        }
        if m.app.format > app.format() {
            return Err(Error::NewerApp { found: m.app.format, supported: app.format() });
        }
    }
    let mut documents = Vec::with_capacity(m.documents.len());
    for (d, entry) in m.documents.into_iter().zip(doc_entries) {
        let doc = r.document(&entry, &format!("document {:?}", d.name.chars().take(64).collect::<String>()))?;
        documents.push(Entry { id: d.id, name: d.name, doc });
    }
    let mut snapshots = Vec::with_capacity(m.snapshots.len());
    for s in m.snapshots {
        let content = r.document(&snapshot_entry(s.id), &format!("snapshot {}", s.id.0))?;
        let taken = parse_rfc3339(&s.taken).ok_or_else(|| Error::Damaged(format!("snapshot {} has no valid date", s.id.0)))?;
        snapshots.push(Snapshot { id: s.id, document: s.document, name: s.name, taken, content });
    }
    let mut assets = BTreeMap::new();
    for key in m.assets {
        let bytes = r.required(&asset_entry(&key), u64::MAX)?;
        assets.insert(key, Arc::new(bytes));
    }
    let app_data = match r.entry(APP_DATA, MAX_MANIFEST)? {
        Some(bytes) => serde_json::from_slice(&bytes).map_err(|e| Error::Damaged(format!("app data: {e}")))?,
        None => BTreeMap::new(),
    };
    // Ids are never reused, even when the file's counter is behind.
    let max_id = documents.iter().map(|d| d.id.0).chain(snapshots.iter().map(|s| s.id.0)).max().unwrap_or(0);
    Ok(Project {
        app_id: m.app.id,
        app_format: m.app.format,
        next_id: m.next_id.max(max_id.saturating_add(1)),
        documents,
        assets,
        snapshots,
        app_data,
    })
}

// ---- Writing ------------------------------------------------------------------------------------

/// JSON with every object's keys in order, so the file is stable.
fn sorted(v: Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(m.into_iter().map(|(k, v)| (k, sorted(v))).collect::<BTreeMap<_, _>>().into_iter().collect()),
        Value::Array(a) => Value::Array(a.into_iter().map(sorted).collect()),
        other => other,
    }
}

fn encode<T: Serialize>(what: &str, v: &T) -> Result<Vec<u8>> {
    let value = serde_json::to_value(v).map_err(|e| Error::Invalid(format!("{what} could not be saved: {e}")))?;
    serde_json::to_vec(&sorted(value)).map_err(|e| Error::Invalid(format!("{what} could not be saved: {e}")))
}

fn write(p: &Project) -> Result<Vec<u8>> {
    let limits = Limits::default();
    if !valid_key(&p.app_id) {
        return Err(Error::Invalid(format!("`{}` is not a valid app id", p.app_id)));
    }
    if p.documents.len() > limits.max_documents || p.snapshots.len() > limits.max_snapshots || p.assets.len() > limits.max_assets {
        return Err(Error::TooLarge("more documents, snapshots or assets than a project file holds".into()));
    }
    let manifest = Manifest {
        format: CONTAINER_VERSION,
        app: AppRecord { id: p.app_id.clone(), format: p.app_format },
        next_id: p.next_id,
        documents: p.documents.iter().map(|e| DocRecord { id: e.id, name: e.name.clone() }).collect(),
        assets: p.assets.keys().cloned().collect(),
        snapshots: p.snapshots.iter().map(|s| SnapRecord { id: s.id, document: s.document, name: s.name.clone(), taken: rfc3339(s.taken) }).collect(),
    };
    let mut parts: Vec<(String, Vec<u8>, CompressionMethod)> = Vec::new();
    let deflated = CompressionMethod::Deflated;
    parts.push((MANIFEST.into(), encode("the manifest", &manifest)?, deflated));
    if !p.app_data.is_empty() {
        parts.push((APP_DATA.into(), encode("the app data", &p.app_data)?, deflated));
    }
    for e in &p.documents {
        parts.push((doc_entry(e.id), encode(&format!("document {:?}", e.name), &e.doc)?, deflated));
    }
    for s in &p.snapshots {
        parts.push((snapshot_entry(s.id), encode(&format!("snapshot {:?}", s.name), &s.content)?, deflated));
    }
    for (k, v) in &p.assets {
        // Media are compressed already.
        parts.push((asset_entry(k), v.to_vec(), CompressionMethod::Stored));
    }
    for (name, bytes, _) in &parts {
        if bytes.len() as u64 > limits.max_entry {
            return Err(Error::TooLarge(format!("{name} is larger than {} MB", limits.max_entry >> 20)));
        }
    }
    zip_parts(&parts).map_err(|e| Error::Invalid(format!("the project could not be packed: {e}")))
}

fn zip_parts(parts: &[(String, Vec<u8>, CompressionMethod)]) -> zip::result::ZipResult<Vec<u8>> {
    // A fixed time, so the same project always gives the same bytes.
    let base = SimpleFileOptions::default().last_modified_time(zip::DateTime::default());
    let mut w = ZipWriter::new(Cursor::new(Vec::new()));
    w.start_file(MIMETYPE_ENTRY, base.compression_method(CompressionMethod::Stored))?;
    w.write_all(MIMETYPE.as_bytes())?;
    for (name, bytes, method) in parts {
        w.start_file(name.as_str(), base.compression_method(*method))?;
        w.write_all(bytes)?;
    }
    Ok(w.finish()?.into_inner())
}

/// A container from parts as given, for tests of old versions and damaged files.
#[cfg(test)]
pub(crate) fn raw_container(parts: &[(&str, &[u8])]) -> Vec<u8> {
    let parts: Vec<(String, Vec<u8>, CompressionMethod)> =
        parts.iter().map(|(n, b)| (n.to_string(), b.to_vec(), CompressionMethod::Deflated)).collect();
    zip_parts(&parts).unwrap_or_default()
}

#[cfg(test)]
mod tests;
