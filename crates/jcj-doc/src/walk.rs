// SPDX-License-Identifier: MIT OR Apache-2.0
//! Walking a document's paragraphs in reading order, with their positions.

use crate::{Block, Document, Paragraph, Path, StoryRef};

/// Every paragraph of the body in reading order, entering tables cell by cell, with its path.
pub fn body_paragraphs(doc: &Document) -> Vec<(Path, &Paragraph)> {
    let mut out = Vec::new();
    walk_blocks(&doc.body, &mut Vec::new(), &mut out, 0);
    out
}

/// The paragraphs of a story (the body or a part such as a header or note).
pub fn story_paragraphs(doc: &Document, story: StoryRef) -> Vec<(Path, &Paragraph)> {
    let mut out = Vec::new();
    if let Some(blocks) = doc.story(story) {
        walk_blocks(blocks, &mut Vec::new(), &mut out, 0);
    }
    out
}

/// Deeper nesting than this (tables in tables) is not walked: hostile files can nest without end.
const MAX_DEPTH: usize = 16;

fn walk_blocks<'a>(blocks: &'a [std::sync::Arc<Block>], prefix: &mut Vec<u32>, out: &mut Vec<(Path, &'a Paragraph)>, depth: usize) {
    if depth > MAX_DEPTH {
        return;
    }
    for (i, b) in blocks.iter().enumerate() {
        let Ok(i) = u32::try_from(i) else { return };
        prefix.push(i);
        match &**b {
            Block::Para(p) => out.push((Path(prefix.clone()), p)),
            Block::Table(t) => {
                for (r, row) in t.rows.iter().enumerate() {
                    for (c, cell) in row.cells.iter().enumerate() {
                        let (Ok(r), Ok(c)) = (u32::try_from(r), u32::try_from(c)) else { continue };
                        prefix.push(r);
                        prefix.push(c);
                        walk_blocks(&cell.blocks, prefix, out, depth + 1);
                        prefix.pop();
                        prefix.pop();
                    }
                }
            }
        }
        prefix.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_document_has_one_body_paragraph() {
        let d = Document::new();
        let ps = body_paragraphs(&d);
        assert_eq!(ps.len(), 1);
        assert_eq!(ps[0].0, Path::top(0));
    }
}
