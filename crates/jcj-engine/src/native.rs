// SPDX-License-Identifier: MIT OR Apache-2.0
//! The engine's native format: the whole document model as JSON, extension data included.
//!
//! It is lossless (everything the model holds comes back) but not a container: an app's project
//! file (`jcj-project`) wraps it with its own version and metadata.

use jcj_doc::Document;

/// The file name extension the engine reads and writes native JSON under.
pub const EXTENSION: &str = "json";

/// `doc` as native JSON bytes.
pub fn save(doc: &Document) -> Result<Vec<u8>, String> {
    jcj_upstream::engine::io::save_bytes("document.json", doc)
}

/// A document from native JSON bytes, made ready for editing as when a file is opened.
pub fn open(bytes: &[u8]) -> Result<Document, String> {
    jcj_upstream::engine::io::open_bytes("document.json", bytes)
}
