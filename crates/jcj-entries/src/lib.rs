// SPDX-License-Identifier: MIT OR Apache-2.0
//! Entries: typed records with fields from a template, names and aliases, mentions in text and
//! backlinks. The general part of a story bible, a cast list, a glossary: whatever an app keeps
//! beside its documents about the people, places and things in them.
//!
//! Entry types and their templates are data, not code: an app ships its own defaults and the
//! writer adds more. Entries are generic over the app's id type (`Id`) and the body it keeps
//! (`B`, the app's own rich text, or a `String`), so each app stores and edits them its own way
//! while sharing the rules: which names an entry answers to, how a name is found in text (as a
//! whole word, matching case when it starts with a capital), and who refers to an entry.
//!
//! Promoted from Wordsmith Manuscript (core issue #14, C13), where it was built as the general
//! layer of the story bible, for Wordsmith Script's characters to share.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// One entry: a record of some type with fields, a body and names.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Entry<Id, B> {
    /// The type's key (any type a template defines).
    pub kind: String,
    /// The entry's name.
    pub name: String,
    /// Other names the entry goes by.
    pub aliases: Vec<String>,
    /// Field values by field name.
    pub fields: BTreeMap<String, String>,
    /// Typed links to other entries: (relation, entry).
    pub relations: Vec<(String, Id)>,
    /// The free body, in the app's own form.
    pub body: B,
}

impl<Id, B: Default> Entry<Id, B> {
    /// A new entry of a type, with a name.
    pub fn new(kind: &str, name: &str) -> Self {
        Entry { kind: kind.into(), name: name.into(), aliases: Vec::new(), fields: BTreeMap::new(), relations: Vec::new(), body: B::default() }
    }
}

impl<Id, B> Entry<Id, B> {
    /// The name and the aliases, without empties.
    pub fn names(&self) -> Vec<&str> {
        std::iter::once(self.name.as_str()).chain(self.aliases.iter().map(String::as_str)).filter(|n| !n.trim().is_empty()).collect()
    }
}

/// The kind of a template field.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FieldType {
    /// One line of text.
    #[default]
    Text,
    /// Several lines of text.
    LongText,
    /// A list of short values, one per line.
    List,
    /// A picture (its path in the app's files).
    Image,
    /// A link to another entry (its id).
    Link,
}

/// One field of a template.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateField {
    /// The field's name.
    pub name: String,
    /// Its type.
    #[serde(default)]
    pub kind: FieldType,
}

/// A template: the fields an entry type has, in order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Template {
    /// The type's key.
    pub kind: String,
    /// The type's name as shown (singular).
    pub title: String,
    /// The type's name for a group of entries (plural), as a folder is named.
    #[serde(default)]
    pub plural: String,
    /// The fields.
    pub fields: Vec<TemplateField>,
}

impl Template {
    /// A template of fields of the given types.
    pub fn new(kind: &str, title: &str, plural: &str, fields: &[(&str, FieldType)]) -> Template {
        Template {
            kind: kind.into(),
            title: title.into(),
            plural: plural.into(),
            fields: fields.iter().map(|(n, k)| TemplateField { name: (*n).into(), kind: *k }).collect(),
        }
    }

    /// An entry's fields in template order, then any values for fields the template no longer
    /// has (never dropped), each with its value ("" when unset).
    pub fn ordered<'a, Id, B>(&self, e: &'a Entry<Id, B>) -> Vec<(String, &'a str)> {
        let mut out: Vec<(String, &str)> = self.fields.iter().map(|f| (f.name.clone(), e.fields.get(&f.name).map_or("", String::as_str))).collect();
        for (k, v) in &e.fields {
            if !self.fields.iter().any(|f| &f.name == k) {
                out.push((k.clone(), v.as_str()));
            }
        }
        out
    }
}

/// Every name and alias with its entry, longest first (so a longer name wins over a shorter one
/// inside it), then in name order.
pub fn name_list<'a, Id: Copy + 'a, B: 'a>(entries: impl IntoIterator<Item = (&'a Id, &'a Entry<Id, B>)>) -> Vec<(String, Id)> {
    let mut out: Vec<(String, Id)> = entries.into_iter().flat_map(|(id, e)| e.names().into_iter().map(|n| (n.trim().to_owned(), *id))).collect();
    out.sort_by(|a, b| b.0.chars().count().cmp(&a.0.chars().count()).then_with(|| a.0.cmp(&b.0)));
    out
}

/// The entry a name or alias belongs to: an exact match first, else one in any case.
pub fn find_by_name<'a, Id: Copy + 'a, B: 'a>(entries: impl IntoIterator<Item = (&'a Id, &'a Entry<Id, B>)> + Clone, name: &str) -> Option<Id> {
    let name = name.trim();
    entries
        .clone()
        .into_iter()
        .find(|(_, e)| e.names().iter().any(|n| n.trim() == name))
        .or_else(|| entries.into_iter().find(|(_, e)| e.names().iter().any(|n| n.trim().to_lowercase() == name.to_lowercase())))
        .map(|(id, _)| *id)
}

/// Where `name` appears in `text` as a whole word: the byte ranges. Names starting with a
/// capital letter match only with the same case; others match in any case.
pub fn find_mentions(text: &str, name: &str) -> Vec<std::ops::Range<usize>> {
    let name = name.trim();
    let mut out = Vec::new();
    if name.is_empty() {
        return out;
    }
    let case = name.chars().next().is_some_and(char::is_uppercase);
    let (hay, needle) = if case { (text.to_owned(), name.to_owned()) } else { (text.to_lowercase(), name.to_lowercase()) };
    if hay.len() != text.len() {
        // Lowercasing changed lengths (rare scripts): fall back to an exact-case search.
        return find_exact(text, name);
    }
    let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
    let mut from = 0;
    while let Some(at) = hay.get(from..).and_then(|h| h.find(&needle)) {
        let start = from + at;
        let end = start + needle.len();
        if !word(hay[..start].chars().next_back()) && !word(hay[end..].chars().next()) {
            out.push(start..end);
        }
        from = end;
    }
    out
}

fn find_exact(text: &str, name: &str) -> Vec<std::ops::Range<usize>> {
    let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(at) = text.get(from..).and_then(|h| h.find(name)) {
        let start = from + at;
        let end = start + name.len();
        if !word(text[..start].chars().next_back()) && !word(text[end..].chars().next()) {
            out.push(start..end);
        }
        from = end;
    }
    out
}

/// Whether `name` appears in `text` as a whole word (see [`find_mentions`]).
pub fn mentions(text: &str, name: &str) -> bool {
    !find_mentions(text, name).is_empty()
}

/// Who refers to an entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Backlinks<Id> {
    /// Documents that link to it, in the order given.
    pub linked: Vec<Id>,
    /// Documents that mention a name or alias of it without linking.
    pub mentioned: Vec<Id>,
}

impl<Id> Default for Backlinks<Id> {
    fn default() -> Self {
        Backlinks { linked: Vec::new(), mentioned: Vec::new() }
    }
}

/// One document as backlinks see it: its id, the ids it links to, and its paragraphs as plain
/// text.
pub struct Document<'a, Id> {
    /// The document.
    pub id: Id,
    /// The entries (or other documents) it links to.
    pub links: &'a [Id],
    /// Its paragraphs as plain text.
    pub paragraphs: &'a [String],
}

/// The backlinks of the entry `target` (with these names) among `documents`.
pub fn backlinks<'a, Id: PartialEq + Copy + 'a>(target: Id, names: &[&str], documents: impl IntoIterator<Item = Document<'a, Id>>) -> Backlinks<Id> {
    let mut out = Backlinks::default();
    for d in documents {
        if d.id == target {
            continue;
        }
        if d.links.contains(&target) {
            out.linked.push(d.id);
        } else if d.paragraphs.iter().any(|p| names.iter().any(|n| mentions(p, n))) {
            out.mentioned.push(d.id);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    type E = Entry<u32, String>;

    #[test]
    fn names_and_lookup() {
        let mut a = E::new("person", "Anna Field");
        a.aliases = vec!["Annie".into(), " ".into()];
        let b = E::new("place", "Field");
        let set = [(1u32, a), (2u32, b)];
        let refs: Vec<(&u32, &E)> = set.iter().map(|(i, e)| (i, e)).collect();
        let list = name_list(refs.clone());
        assert_eq!(list.first().map(|x| x.0.as_str()), Some("Anna Field"));
        assert_eq!(list.len(), 3);
        assert_eq!(find_by_name(refs.clone(), "annie"), Some(1));
        assert_eq!(find_by_name(refs.clone(), "Field"), Some(2));
        assert_eq!(find_by_name(refs, "Nobody"), None);
    }

    #[test]
    fn whole_word_mentions_respect_capitals() {
        assert!(mentions("Then Anna left.", "Anna"));
        assert!(!mentions("Then Annabel left.", "Anna"));
        assert!(!mentions("then anna left", "Anna"));
        assert!(mentions("The Old Mill stood.", "old mill"), "a name without a capital matches any case");
        assert_eq!(find_mentions("Anna met Anna.", "Anna"), vec![0..4, 9..13]);
    }

    #[test]
    fn fields_in_template_order_never_drop_values() {
        let t = Template::new("person", "Person", "People", &[("Age", FieldType::Text), ("Wants", FieldType::LongText)]);
        let mut e = E::new("person", "Anna");
        e.fields.insert("Wants".into(), "the sea".into());
        e.fields.insert("Eyes".into(), "grey".into());
        let got: Vec<(String, &str)> = t.ordered(&e);
        assert_eq!(got, vec![("Age".into(), ""), ("Wants".into(), "the sea"), ("Eyes".into(), "grey")]);
        let json = serde_json::to_string(&t).unwrap_or_default();
        assert!(json.contains("long-text"));
    }

    #[test]
    fn backlinks_split_links_and_mentions() {
        let paras_a = vec!["Anna came.".to_owned()];
        let paras_b = vec!["Nobody here.".to_owned()];
        let paras_c = vec!["Annie waved.".to_owned()];
        let docs = vec![
            Document { id: 10u32, links: &[1], paragraphs: &paras_b },
            Document { id: 11, links: &[], paragraphs: &paras_a },
            Document { id: 12, links: &[], paragraphs: &paras_b },
            Document { id: 13, links: &[], paragraphs: &paras_c },
            Document { id: 1, links: &[], paragraphs: &paras_a },
        ];
        let b = backlinks(1, &["Anna", "Annie"], docs);
        assert_eq!(b.linked, vec![10]);
        assert_eq!(b.mentioned, vec![11, 13]);
    }
}
