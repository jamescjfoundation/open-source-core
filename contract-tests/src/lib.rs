// SPDX-License-Identifier: MIT OR Apache-2.0
//! Contract tests: the behavior foundation apps rely on, checked against every upstream
//! candidate before it is accepted (OPEN_SOURCE_CORE_PLAN.md, "Contract tests catch behavior
//! drift"). The tests are in `tests/`:
//!
//! - `model.rs`: open, edit, undo, save as native JSON, reopen; extension data on documents,
//!   paragraphs and runs survives all of it;
//! - `layout.rs`: keep with next, widow and orphan control, page break before, footnotes, drop
//!   caps, each on a document built in code;
//! - `pdf.rs`: PDF output is the same bytes every time, and its SHA-256 matches the golden file;
//! - `adapters.rs`: each adapter function with its documented inputs and outputs.
//!
//! **Golden files** live in `golden/`. When a change to the output is intended (a new upstream
//! version that draws text differently), rewrite them with `UPDATE_GOLDEN=1 cargo test -p
//! contract-tests` and say why in the commit.
//!
//! This library holds the fixtures the tests share: documents on a bundled font, so layout and
//! output are the same on every machine. All text in them is written for the purpose.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::path::PathBuf;
use std::sync::Arc;

use jcj_doc::props::{CharProps, LineSpacing};
use jcj_doc::{Document, Paragraph, para_block};
use jcj_layout::{LayoutCache, print_layout};
use sha2::{Digest, Sha256};

/// The font every fixture is set in: bundled, so installed fonts cannot change the output.
pub const FONT: &str = "Source Serif 4";

/// A sentence of sample text (written for these tests).
pub const SENTENCE: &str = "The quick brown fox jumps over the lazy dog while the patient owl counts the stars above the quiet valley. ";

/// A document holding `paras`, set in [`FONT`] at 12 pt with single line spacing and no space
/// between paragraphs, on a Letter page with one-inch margins; system font fallback off.
pub fn document(paras: Vec<Paragraph>) -> Document {
    jcj_layout::fonts::set_system_fallback(false);
    let mut d = Document::new();
    let st = Arc::make_mut(&mut d.styles);
    st.default_chr.font = Some(FONT.into());
    st.default_chr.size = Some(12.0);
    st.default_para.space_after = Some(0.0);
    st.default_para.space_before = Some(0.0);
    st.default_para.line_spacing = Some(LineSpacing::Multiple(1.0));
    d.body = paras.into_iter().map(para_block).collect();
    if d.body.is_empty() {
        d.body.push(para_block(Paragraph::new()));
    }
    d
}

/// A paragraph of `text` with no formatting of its own.
pub fn para(text: &str) -> Paragraph {
    Paragraph::with_text(text, CharProps::default())
}

/// `n` one-line paragraphs, "Line 0", "Line 1", ...
pub fn lines(n: usize) -> Vec<Paragraph> {
    (0..n).map(|i| para(&format!("Line {i}"))).collect()
}

/// A paragraph of `n` [`SENTENCE`]s (about one line and a half each).
pub fn long_para(n: usize) -> Paragraph {
    para(SENTENCE.repeat(n).trim_end())
}

/// How many one-line paragraphs fill the first page of a [`document`].
pub fn lines_per_page() -> usize {
    let d = document(lines(200));
    let l = print_layout(&d, &mut LayoutCache::new());
    jcj_layout::pages::body_pieces(&l).first().map_or(0, Vec::len)
}

/// Lowercase hexadecimal SHA-256 of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// The path of golden file `name`.
pub fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("golden").join(name)
}

/// Compare `actual` with golden file `name` (whitespace at the ends ignored). With
/// `UPDATE_GOLDEN=1` in the environment the file is written instead.
pub fn check_golden(name: &str, actual: &str) -> Result<(), String> {
    let path = golden_path(name);
    if std::env::var("UPDATE_GOLDEN").is_ok_and(|v| v == "1") {
        std::fs::write(&path, format!("{}\n", actual.trim())).map_err(|e| format!("writing {}: {e}", path.display()))?;
        return Ok(());
    }
    let expected = std::fs::read_to_string(&path).map_err(|e| format!("reading {}: {e} (run with UPDATE_GOLDEN=1 to create it)", path.display()))?;
    if expected.trim() == actual.trim() {
        Ok(())
    } else {
        Err(format!(
            "golden file {name} differs:\n  expected {}\n  actual   {}\nIf the change is intended, run with UPDATE_GOLDEN=1 and say why in the commit.",
            expected.trim(),
            actual.trim()
        ))
    }
}
