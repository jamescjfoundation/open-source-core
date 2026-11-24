// SPDX-License-Identifier: MIT OR Apache-2.0
//! General editing commands every foundation app can register.
//!
//! The upstream engine already registers about 400 word-processor commands in every registry
//! (caret movement, selection, typing, clipboard, character and paragraph formatting, Word-style
//! find and replace, proofing). This module adds only what the engine lacks, or where an app
//! needs a different general behavior:
//!
//! - [`find`]: literal find and replace that wraps round the body, lists every match without
//!   moving the selection, and can be limited to paragraphs of chosen styles (`find.*`). Apps
//!   reuse its search and [`find::replace_all_with`] in their own commands;
//! - [`format`](mod@format): Plain Text, Reset to Style (keeps extension data, which the engine's resets
//!   drop) and Toggle Capitals (`format.*`);
//! - [`edit`]: a page break kept as a paragraph property, deleting a paragraph, and stepping the
//!   selection paragraph by paragraph (`edit.*`, `select.*`);
//! - [`spelling`]: spelling over the whole document with words the document itself keeps as
//!   correct (undoable, saved with it), and a guarded replace for agents (`spelling.*`). An app
//!   with its own rules (paragraphs to skip, words it knows) uses [`spelling::misspellings_with`]
//!   in its own `spelling.check`.
//!
//! An app registers them with its own: `registry_with(commands::merged(app_specs))`. An app
//! command with the id of one of these replaces it, as app commands replace engine commands.
//!
//! None of these ids is an engine id (a test checks it on every upstream update), so the engine's
//! command with the same purpose stays available beside them.
//!
//! # Earlier ids and where they are now
//!
//! Commands apps carried before the core, by their old ids. "Engine" means the upstream command
//! already does it; use its id. Where the old id took other parameters, they are noted. Decision
//! 0007 (`docs/decisions/0007-engine-agents-output.md`) has the same table.
//!
//! | Earlier id | Now | Notes |
//! | --- | --- | --- |
//! | `caret.left`, `caret.right`, `caret.up`, `caret.down` | Engine, same ids | |
//! | `caret.wordLeft`, `caret.wordRight` | Engine, same ids | Word Right stops after the word's trailing spaces, as before |
//! | `caret.lineStart`, `caret.lineEnd` | Engine `caret.home`, `caret.end` | |
//! | `caret.docStart`, `caret.docEnd` | Engine, same ids | |
//! | `select.left` ... `select.docEnd` | Engine `caret.*` with `{"extend": true}` | One command per movement, not two |
//! | `select.all` | Engine `select.all` | |
//! | `caret.to {pos, extend?}` | Engine `caret.set {pos, extend?}` | |
//! | `select.wordAt {pos}` | Engine `caret.set {pos}` then `select.word` | |
//! | `text.insert`, `text.newParagraph`, `text.tab`, `text.backspace`, `text.delete` | Engine, same ids | Rules for what typing does in a paragraph style stay in the app |
//! | `text.toggleCase` | `format.toggleCapitals` | Engine `format.changeCase {"mode": "toggle"}` swaps each letter instead |
//! | `text.pageBreak` | `edit.newPage` | Engine `text.pageBreak` inserts a page-break character instead |
//! | `paragraph.delete` | `edit.deleteParagraph` | |
//! | `paragraph.next`, `paragraph.previous` | `select.nextParagraph`, `select.previousParagraph` | |
//! | `goto.page {page}` | Engine `edit.goto {page}` | Page labels other than numbers stay in the app |
//! | An app's own Reset to Style | `format.resetToStyle` | Engine `para.reset` leaves character formatting; `format.clear` drops the style |
//! | `format.bold`, `format.italic`, `format.underline` | Engine, same ids | Engine takes `{"value"?: bool}` |
//! | `format.style {style, on?}` | Engine `format.<style> {value?}` | `bold`, `italic`, `underline`, `doubleUnderline`, `wordUnderline`, `strikethrough`, `superscript`, `subscript`, `allCaps`, `smallCaps`; dotted: `format.underline {"style": "dotted"}` |
//! | `format.plain` | `format.plain` | |
//! | `format.highlight {color}` | Engine `format.highlight {color}`, `format.shading {color}` | Highlighter names are the engine's; a `#RRGGBB` color is shading. An app's own color names stay in the app |
//! | `format.color {color}` | Engine `format.color {color}` | |
//! | `format.lowercase` | Engine `format.changeCase {"mode": "lower"}` | |
//! | `format.state` | Engine `format.state` | Returns each property, not a list of style names |
//! | `find.all`, `find.next`, `find.previous` | `find.all`, `find.next`, `find.previous` | `{"query", ...}`; a filter `{"styles": [id]}` replaces the earlier app-specific one |
//! | `replace.one`, `replace.all` | `find.replace`, `find.replaceAll` | Casing rules of a paragraph style stay in the app ([`find::replace_all_with`]) |
//! | `spelling.check` | `spelling.check` | Positions are `{start, end}` positions, not `{block, start, end}` |
//! | `spelling.suggest {word}` | `spelling.suggest {word}` | |
//! | `spelling.ignore {word}` | `spelling.ignore {word}` | |
//! | `spelling.addWord {word}` | Engine `review.addToDictionary {word}` | Also saves the user dictionary |
//! | `spelling.replace {block, start, end, word, with}` | `spelling.replace {start, end, word, with}` | `start` and `end` are positions |

pub mod edit;
pub mod find;
pub mod format;
pub mod spelling;

use crate::CommandSpec;

/// Every command of this module, in a stable order.
pub fn specs() -> Vec<CommandSpec> {
    let mut all = find::specs();
    all.extend(format::specs());
    all.extend(edit::specs());
    all.extend(spelling::specs());
    all
}

/// The core's commands without those `app` replaces (same id), followed by `app`. Pass the
/// result to [`registry_with`](crate::registry_with).
pub fn merged(app: Vec<CommandSpec>) -> Vec<CommandSpec> {
    let mut all: Vec<CommandSpec> = specs().into_iter().filter(|s| !app.iter().any(|a| a.id == s.id)).collect();
    all.extend(app);
    all
}

#[cfg(test)]
mod tests;
