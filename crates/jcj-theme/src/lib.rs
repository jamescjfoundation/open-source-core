// SPDX-License-Identifier: MIT OR Apache-2.0
//! Design tokens to theme.
//!
//! A product describes its look in a `tokens.json` file: its colors in a light, a dark and an
//! extra-dark theme, the names of its interface fonts, its type sizes, spacing and corner radii
//! (the format is in `TOKENS.md` beside this crate). This crate reads that file into [`Tokens`],
//! picks one [`Theme`] from it for the mode the user chose, and turns it into egui's visuals and
//! style. The fonts' bytes come from the app ([`FontFace`]); the theme installs them.
//!
//! The core carries no product's colors: [`Tokens::default`] is a neutral set, for the core's
//! test app and for apps that have no tokens yet.
//!
//! Interface code reads the theme in use with [`Theme::get`], so every widget draws with the
//! product's colors without being handed them.
//!
//! An app's brand files are loaded the same way in every app ([`brand`]): [`Brand`] gives the
//! tokens, the app's own section of the tokens file and the window's icon; [`Mask`] paints a
//! mark in any color.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod brand;
mod color;
mod fonts;
mod tokens;
mod visuals;

pub use brand::{Brand, Mask};
pub use color::{ColorError, hex, parse_hex};
pub use fonts::{
    FAMILY_MEDIUM, FAMILY_SEMIBOLD, FontFace, default_faces, font_definitions, install_fonts, medium, mono, regular, semibold, with_defaults,
};
pub use tokens::{Colors, FontNames, Mode, Radius, Spacing, Theme, Tokens, TokensError, TypeSizes};
pub use visuals::{apply, style, visuals};
