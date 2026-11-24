// SPDX-License-Identifier: MIT OR Apache-2.0
//! The document the format tests share, built in code: headings, emphasis, a list, a table.

use std::sync::Arc;

use jcj_doc::props::CharProps;
use jcj_doc::table::{Cell, Table};
use jcj_doc::{Block, Document, ListKind, Paragraph, para_block};
use jcj_upstream::doc::props::NumRef;

/// Two chapters (level 1 headings), a paragraph with bold and italic, a bulleted list, a level 2
/// heading and a two-by-two table.
pub fn sample() -> Document {
    let mut d = Document::new();
    d.core.title = "Sample Book".into();
    d.core.creator = "A. Writer".into();
    let num = Arc::make_mut(&mut d.numbering).add_list(ListKind::Bullet);
    let bold = CharProps { bold: Some(true), ..CharProps::default() };
    let italic = CharProps { italic: Some(true), ..CharProps::default() };
    let mut body = vec![para_block(Paragraph::with_text("Chapter One", CharProps::default()).styled("Heading1"))];
    let mut p = Paragraph::with_text("Plain, ", CharProps::default());
    p.insert_text(p.len(), "bold", &bold).unwrap();
    p.insert_text(p.len(), " and ", &CharProps::default()).unwrap();
    p.insert_text(p.len(), "italic", &italic).unwrap();
    p.insert_text(p.len(), " words & <signs>.", &CharProps::default()).unwrap();
    body.push(para_block(p));
    for item in ["First item", "Second item"] {
        let mut li = Paragraph::with_text(item, CharProps::default());
        li.props.numbering = Some(NumRef { num, level: 0 });
        body.push(para_block(li));
    }
    body.push(para_block(Paragraph::with_text("A Section", CharProps::default()).styled("Heading2")));
    let mut t = Table::new(2, 2, 400.0);
    for (r, row) in [["Name", "Value"], ["alpha", "1"]].iter().enumerate() {
        for (c, text) in row.iter().enumerate() {
            t.rows[r].cells[c] = Cell::with_text(text);
        }
    }
    body.push(Arc::new(Block::Table(t)));
    body.push(para_block(Paragraph::with_text("Chapter Two", CharProps::default()).styled("Heading1")));
    body.push(para_block(Paragraph::with_text("The end.", CharProps::default())));
    d.body = body;
    d
}
