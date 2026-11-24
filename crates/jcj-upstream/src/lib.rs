// SPDX-License-Identifier: MIT OR Apache-2.0
//! The one place upstream crates are named.
//!
//! open-source-core builds on WordCraft and craft-libs (decision 0004). This crate is the only one
//! in the core that depends on them directly, and it does nothing but re-export them, so that:
//!
//! - every `jcj-` adapter crate names one dependency and can never pick a different upstream
//!   commit by mistake;
//! - `cargo xtask layers` can check the rule mechanically: no other crate in this workspace, and
//!   no crate in an app repository, may depend on a `wordcraft-*` or `craft-*` crate, and app
//!   repositories may not depend on this crate either.
//!
//! **Apps must not use this crate.** They use the `jcj-` interfaces (`jcj-doc`, `jcj-layout`, ...)
//! which absorb upstream changes. The pins themselves live in the workspace `Cargo.toml` and are
//! recorded, with every carried patch, in `docs/UPSTREAM.md`.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub use craft_atomic as atomic;
pub use craft_proof as proof;
pub use wordcraft_control_key as control_key;
pub use wordcraft_doc as doc;
pub use wordcraft_docx as docx;
pub use wordcraft_engine as engine;
pub use wordcraft_fonts as fonts;
pub use wordcraft_formats as formats;
pub use wordcraft_geom as geom;
pub use wordcraft_layout as layout;
pub use wordcraft_mcp as mcp;
pub use wordcraft_pdf as pdf;
pub use wordcraft_render as render;

#[cfg(feature = "shell")]
pub use craft_dialogs as dialogs;
#[cfg(feature = "shell")]
pub use craft_update as update;

#[cfg(feature = "print")]
pub use craft_print as print;

#[cfg(feature = "pdf-read")]
pub use craft_pdf as pdf_read;

/// The WordCraft release the core is built on (our fork's `jcj` branch is this tag plus our
/// patches; docs/UPSTREAM.md).
pub const WORDCRAFT_RELEASE: &str = "v0.18.1";

/// The commit of our fork's `jcj` branch the core builds from.
pub const WORDCRAFT_FORK_COMMIT: &str = "73717786b6a01a93fba79892f04a2f2d3380a761";

#[cfg(test)]
mod tests {
    #[test]
    fn the_fork_commit_matches_the_workspace_patch_block() {
        let manifest = include_str!("../../../Cargo.toml");
        assert!(manifest.contains(super::WORDCRAFT_FORK_COMMIT), "update WORDCRAFT_FORK_COMMIT with the patch block");
        assert!(manifest.contains(&format!("tag = \"{}\"", super::WORDCRAFT_RELEASE)));
    }
}
