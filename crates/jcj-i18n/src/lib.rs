// SPDX-License-Identifier: MIT OR Apache-2.0
//! Interface translation catalogs: an app's and the shell's texts shown in the user's language.
//!
//! A catalog is one TOML file per language that maps each interface text, keyed by its English
//! exactly as the code writes it, to the text shown instead:
//!
//! ```toml
//! "Save" = "Guardar"
//! "Save the changes to “{name}”?" = "¿Guardar los cambios de «{name}»?"
//! "{n} words" = { one = "{n} palabra", other = "{n} palabras" }
//! ```
//!
//! Placeholders are `{name}`, filled by [`tr_with`]; a plural takes the language's plural forms
//! (`zero`, `one`, `two`, `few`, `many`, `other`; [`plural_form`]) and is looked up by
//! [`trn`] under its English plural. A text no catalog has is shown in English, so a partial
//! translation never hides anything.
//!
//! The language in use is process-wide: [`set`] installs catalogs, in order (an app's own first,
//! then the shell's), and nothing is translated until something calls it. English needs no
//! catalog.
//!
//! [`texts_in`] finds the texts a source file asks for, so a test can check that every text is
//! in every catalog and that no catalog keeps texts nothing uses ([`compare`]).

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, OnceLock, RwLock};

/// The longest catalog read, in bytes.
pub const MAX_CATALOG: usize = 4 << 20;

/// A plural's forms, as a catalog names them.
pub const PLURAL_FORMS: [&str; 6] = ["zero", "one", "two", "few", "many", "other"];

/// One catalog entry: a text, or a plural's forms.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    /// The text shown.
    Text(String),
    /// The text for each plural form the language has (`other` always).
    Plural(BTreeMap<String, String>),
}

/// One language's texts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Catalog {
    entries: HashMap<String, Entry>,
}

impl Catalog {
    /// Read a catalog. Every value must be a string or a table of plural forms with `other`.
    pub fn parse(text: &str) -> Result<Catalog, String> {
        if text.len() > MAX_CATALOG {
            return Err(format!("the catalog is larger than {} MB", MAX_CATALOG >> 20));
        }
        let table: toml::Table = text.parse().map_err(|e: toml::de::Error| e.to_string())?;
        let mut entries = HashMap::new();
        for (key, value) in table {
            let entry = match value {
                toml::Value::String(s) => Entry::Text(s),
                toml::Value::Table(t) => {
                    let mut forms = BTreeMap::new();
                    for (form, v) in t {
                        if !PLURAL_FORMS.contains(&form.as_str()) {
                            return Err(format!("“{key}”: “{form}” is not a plural form ({})", PLURAL_FORMS.join(", ")));
                        }
                        let toml::Value::String(s) = v else { return Err(format!("“{key}”: the {form} form is not text")) };
                        forms.insert(form, s);
                    }
                    if !forms.contains_key("other") {
                        return Err(format!("“{key}”: a plural needs its other form"));
                    }
                    Entry::Plural(forms)
                }
                _ => return Err(format!("“{key}” is not text or a plural")),
            };
            entries.insert(key, entry);
        }
        Ok(Catalog { entries })
    }

    /// The entry for an English text.
    pub fn get(&self, english: &str) -> Option<&Entry> {
        self.entries.get(english)
    }

    /// Every English text the catalog has.
    pub fn keys(&self) -> BTreeSet<&str> {
        self.entries.keys().map(String::as_str).collect()
    }

    /// How many texts it has.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether it has none.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// The plural form a count takes in a language (by its code's first part: `pt-BR` is `pt`). The
/// common European rules; any other language uses `other` for every count.
pub fn plural_form(language: &str, n: u64) -> &'static str {
    let lang = language.split(['-', '_']).next().unwrap_or("").to_ascii_lowercase();
    let (n10, n100) = (n % 10, n % 100);
    match lang.as_str() {
        "en" | "de" | "nl" | "sv" | "da" | "nb" | "no" | "nn" | "fi" | "et" | "el" | "hu" | "it" | "es" | "ca" | "gl" | "eu" | "bg" | "tr" => {
            if n == 1 { "one" } else { "other" }
        }
        "fr" | "pt" => {
            if n <= 1 {
                "one"
            } else {
                "other"
            }
        }
        "ru" | "uk" | "be" | "sr" | "hr" | "bs" => {
            if n10 == 1 && n100 != 11 {
                "one"
            } else if (2..=4).contains(&n10) && !(12..=14).contains(&n100) {
                "few"
            } else {
                "many"
            }
        }
        "pl" => {
            if n == 1 {
                "one"
            } else if (2..=4).contains(&n10) && !(12..=14).contains(&n100) {
                "few"
            } else {
                "many"
            }
        }
        "cs" | "sk" => match n {
            1 => "one",
            2..=4 => "few",
            _ => "other",
        },
        _ => "other",
    }
}

struct Current {
    language: String,
    catalogs: Vec<Arc<Catalog>>,
}

fn current() -> &'static RwLock<Current> {
    static CURRENT: OnceLock<RwLock<Current>> = OnceLock::new();
    CURRENT.get_or_init(|| RwLock::new(Current { language: "en".into(), catalogs: Vec::new() }))
}

/// Show texts in `language`, looked up in `catalogs` in order (the first that has a text gives
/// it). English, or no catalogs, shows every text as the code writes it.
pub fn set(language: &str, catalogs: Vec<Catalog>) {
    if let Ok(mut c) = current().write() {
        c.language = if language.trim().is_empty() { "en".into() } else { language.trim().to_owned() };
        c.catalogs = catalogs.into_iter().map(Arc::new).collect();
    }
}

/// The language texts are shown in.
pub fn language() -> String {
    current().read().map(|c| c.language.clone()).unwrap_or_else(|_| "en".into())
}

fn lookup(english: &str) -> Option<(String, Entry)> {
    let c = current().read().ok()?;
    c.catalogs.iter().find_map(|cat| cat.get(english).cloned()).map(|e| (c.language.clone(), e))
}

/// The text for `english` in the language in use.
pub fn tr(english: &str) -> String {
    match lookup(english) {
        Some((_, Entry::Text(s))) => s,
        Some((_, Entry::Plural(forms))) => forms.get("other").cloned().unwrap_or_else(|| english.to_owned()),
        None => english.to_owned(),
    }
}

/// Fill `{name}` placeholders.
pub fn fill(text: &str, args: &[(&str, &str)]) -> String {
    let mut out = text.to_owned();
    for (name, value) in args {
        out = out.replace(&format!("{{{name}}}"), value);
    }
    out
}

/// The text for `english` in the language in use, its `{name}` placeholders filled.
pub fn tr_with(english: &str, args: &[(&str, &str)]) -> String {
    fill(&tr(english), args)
}

/// A count's text: `one` and `other` are the English forms (looked up under `other`), `{n}` is
/// the count, and `args` fill other placeholders.
pub fn trn(one: &str, other: &str, n: u64, args: &[(&str, &str)]) -> String {
    let count = n.to_string();
    let mut all: Vec<(&str, &str)> = vec![("n", &count)];
    all.extend_from_slice(args);
    let text = match lookup(other) {
        Some((lang, Entry::Plural(forms))) => {
            let form = plural_form(&lang, n);
            forms.get(form).or_else(|| forms.get("other")).cloned().unwrap_or_else(|| other.to_owned())
        }
        Some((_, Entry::Text(s))) => s,
        None => (if n == 1 { one } else { other }).to_owned(),
    };
    fill(&text, &all)
}

/// The computer's language as a code (`es_ES.UTF-8` is `es`), from `LC_ALL`, `LC_MESSAGES` or
/// `LANG`, when it is one of `available`; else English.
pub fn system_language(available: &[&str]) -> String {
    for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(v) = std::env::var(var) {
            let code: String = v.chars().take_while(char::is_ascii_alphabetic).collect::<String>().to_ascii_lowercase();
            if !code.is_empty() {
                return if available.contains(&code.as_str()) { code } else { "en".into() };
            }
        }
    }
    "en".into()
}

/// Mark a text that is translated later, where it is shown (a label kept as data, passed to a
/// widget that calls [`tr`]): the text itself, found by [`texts_in`].
pub const fn tl(english: &'static str) -> &'static str {
    english
}

/// The functions whose first text argument (and second, for [`trn`]) is an interface text.
pub const CALLS: [&str; 4] = ["tr(", "tr_with(", "trn(", "tl("];

/// Read one Rust string literal starting at `s` (after any spaces): normal (`"…"`, with the
/// common escapes) or raw (`r"…"`, `r#"…"#`). Gives the text and the rest.
fn literal(s: &str) -> Option<(String, &str)> {
    let s = s.trim_start();
    if let Some(rest) = s.strip_prefix('r') {
        let hashes = rest.chars().take_while(|c| *c == '#').count();
        let rest = rest[hashes..].strip_prefix('"')?;
        let end = format!("\"{}", "#".repeat(hashes));
        let i = rest.find(&end)?;
        return Some((rest[..i].to_owned(), &rest[i + end.len()..]));
    }
    let rest = s.strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = rest.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '"' => return Some((out, &rest[i + 1..])),
            '\\' => match chars.next()?.1 {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                '\\' => out.push('\\'),
                '"' => out.push('"'),
                '\'' => out.push('\''),
                '0' => out.push('\0'),
                'u' => {
                    if chars.next()?.1 != '{' {
                        return None;
                    }
                    let mut hex = String::new();
                    loop {
                        let (_, h) = chars.next()?;
                        if h == '}' {
                            break;
                        }
                        hex.push(h);
                    }
                    out.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
                }
                '\n' => {
                    // A line continuation: the next line's leading spaces are skipped.
                    let mut peek = chars.clone();
                    while let Some((_, w)) = peek.next() {
                        if !w.is_whitespace() {
                            break;
                        }
                        chars = peek.clone();
                    }
                }
                _ => return None,
            },
            _ => out.push(c),
        }
    }
    None
}

/// The interface texts a Rust source file asks for: the literal first argument of every call to
/// [`tr`], [`tr_with`], [`trn`] and [`tl`] (its second too, for `trn`), by any path
/// (`i18n::tr("…")`).
/// A call whose argument is not a literal is not counted.
pub fn texts_in(source: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for call in CALLS {
        let mut rest = source;
        while let Some(i) = rest.find(call) {
            let before = rest[..i].chars().next_back();
            let after = &rest[i + call.len()..];
            rest = after;
            // A whole name: `tr(` but not `attr(`.
            if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                continue;
            }
            let Some((text, more)) = literal(after) else { continue };
            out.insert(text);
            if call == "trn("
                && let Some(more) = more.trim_start().strip_prefix(',')
                && let Some((other, _)) = literal(more)
            {
                out.insert(other);
            }
        }
    }
    out
}

/// A catalog against the texts the code uses: those it lacks, and those it keeps that nothing
/// uses. A `trn` call's English singular needs no entry of its own (its plural holds both).
pub fn compare(catalog: &Catalog, used: &BTreeSet<String>, singulars: &BTreeSet<String>) -> (Vec<String>, Vec<String>) {
    let keys = catalog.keys();
    let missing = used.iter().filter(|t| !singulars.contains(*t) && !keys.contains(t.as_str())).cloned().collect();
    let unused = keys.iter().filter(|k| !used.contains(**k)).map(|k| (*k).to_owned()).collect();
    (missing, unused)
}

/// The English singulars of the `trn` calls in a source file (see [`compare`]).
pub fn singulars_in(source: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut rest = source;
    while let Some(i) = rest.find("trn(") {
        let before = rest[..i].chars().next_back();
        rest = &rest[i + 4..];
        if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
            continue;
        }
        if let Some((one, _)) = literal(rest) {
            out.insert(one);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const ES: &str = r#"
"Save" = "Guardar"
"Save the changes to “{name}”?" = "¿Guardar los cambios de «{name}»?"
"{n} words" = { one = "{n} palabra", other = "{n} palabras" }
"#;

    // The language is process-wide: one test changes it, in order.
    #[test]
    fn texts_are_shown_in_the_language_set_and_in_english_otherwise() {
        assert_eq!(tr("Save"), "Save", "nothing is translated until a catalog is set");
        assert_eq!(trn("{n} word", "{n} words", 1, &[]), "1 word");
        assert_eq!(trn("{n} word", "{n} words", 3, &[]), "3 words");
        set("es", vec![Catalog::parse(ES).unwrap()]);
        assert_eq!(language(), "es");
        assert_eq!(tr("Save"), "Guardar");
        assert_eq!(tr("Open"), "Open", "a text the catalog lacks is shown in English");
        assert_eq!(tr_with("Save the changes to “{name}”?", &[("name", "Uno")]), "¿Guardar los cambios de «Uno»?");
        assert_eq!(trn("{n} word", "{n} words", 1, &[]), "1 palabra");
        assert_eq!(trn("{n} word", "{n} words", 0, &[]), "0 palabras");
        // Layers: the first catalog that has a text gives it.
        set("es", vec![Catalog::parse(r#""Save" = "Guardar ahora""#).unwrap(), Catalog::parse(ES).unwrap()]);
        assert_eq!(tr("Save"), "Guardar ahora");
        assert_eq!(trn("{n} word", "{n} words", 2, &[]), "2 palabras");
        set("en", Vec::new());
        assert_eq!(tr("Save"), "Save");
    }

    #[test]
    fn catalogs_are_checked_when_read() {
        assert_eq!(Catalog::parse(ES).unwrap().len(), 3);
        for bad in [r#""a" = 1"#, r#""a" = { one = "x" }"#, r#""a" = { une = "x", other = "y" }"#, r#""a" = { one = 1, other = "y" }"#, "not toml ="]
        {
            assert!(Catalog::parse(bad).is_err(), "{bad}");
        }
        assert!(Catalog::parse(&" ".repeat(MAX_CATALOG + 1)).is_err());
    }

    #[test]
    fn plural_forms_follow_the_language() {
        assert_eq!([0, 1, 2].map(|n| plural_form("en", n)), ["other", "one", "other"]);
        assert_eq!([0, 1, 2].map(|n| plural_form("fr", n)), ["one", "one", "other"]);
        assert_eq!([1, 3, 5, 21, 22, 25, 11, 12].map(|n| plural_form("ru", n)), ["one", "few", "many", "one", "few", "many", "many", "many"]);
        assert_eq!([1, 2, 5, 22, 12].map(|n| plural_form("pl", n)), ["one", "few", "many", "few", "many"]);
        assert_eq!(plural_form("pt-BR", 0), "one");
        assert_eq!(plural_form("ja", 1), "other");
    }

    #[test]
    fn the_texts_a_source_asks_for_are_found() {
        let src = r##"
            ui.label(tr("Save"));
            let s = i18n::tr_with("Save the changes to “{name}”?", &[("name", n)]);
            let w = trn("{n} word", "{n} words", n, &[]);
            let x = tr(r#"A "quoted" text"#);
            let y = tr("Line one\nand \u{201C}two\u{201D}");
            let z = attr("not this");
            let l = Control::new(id).label(tl("Later"));
            let v = tr(variable);
        "##;
        let t = texts_in(src);
        let want: BTreeSet<String> =
            ["Save", "Save the changes to “{name}”?", "{n} word", "{n} words", "A \"quoted\" text", "Line one\nand “two”", "Later"]
                .map(String::from)
                .into();
        assert_eq!(t, want);
        let one = singulars_in(src);
        assert_eq!(one, ["{n} word".to_owned()].into());
        let cat = Catalog::parse(&format!("{ES}\n\"Unused\" = \"Sin uso\"")).unwrap();
        let (missing, unused) = compare(&cat, &t, &one);
        assert_eq!(missing, vec!["A \"quoted\" text".to_owned(), "Later".to_owned(), "Line one\nand “two”".to_owned()]);
        assert_eq!(unused, vec!["Unused".to_owned()]);
    }

    #[test]
    fn the_system_language_is_one_available_or_english() {
        // Only the parsing: the environment is the process's.
        assert!(["en", "es"].contains(&system_language(&["en", "es"]).as_str()));
    }
}
