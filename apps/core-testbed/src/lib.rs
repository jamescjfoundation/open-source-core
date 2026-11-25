// SPDX-License-Identifier: MIT OR Apache-2.0
//! open-source-core's test app. It uses every `jcj-` crate the way an app does: a headless
//! self-test ([`selftest`]), a desktop window on the shell ([`window`]), and an MCP server for
//! agents. A new core interface gets a use here before any product uses it (AGENTS.md).

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod languages;
pub mod selftest;
pub mod window;
