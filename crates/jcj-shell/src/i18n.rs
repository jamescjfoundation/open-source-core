// SPDX-License-Identifier: MIT OR Apache-2.0
//! The shell's own texts in other languages (C4), through the core's catalogs (`jcj-i18n`).
//!
//! Nothing changes until an app turns a language on with [`set_language`], giving its own
//! catalogs: then the app's texts come from its catalog and the shell's from the shell's, and
//! any text neither has stays in English.

/// The languages the shell's texts come in: code and name in that language.
pub const LANGUAGES: [(&str, &str); 2] = [("en", "English"), ("es", "Español")];

/// The shell's catalog for a language (none for English).
pub fn catalog(code: &str) -> Option<&'static str> {
    match code {
        "es" => Some(include_str!("../i18n/es.toml")),
        _ => None,
    }
}

/// Show the interface in `code` ("" follows the computer, among the languages `apps` and the
/// shell have), with the app's catalogs (code and catalog text) before the shell's. A catalog
/// that cannot be read is left out. Gives the language now used.
pub fn set_language(code: &str, apps: &[(&str, &str)]) -> String {
    let available: Vec<&str> = LANGUAGES.iter().map(|l| l.0).chain(apps.iter().map(|a| a.0)).collect();
    let code = if code.trim().is_empty() { jcj_i18n::system_language(&available) } else { code.trim().to_owned() };
    let code = if available.contains(&code.as_str()) { code } else { "en".to_owned() };
    let catalogs =
        apps.iter().filter(|a| a.0 == code).map(|a| a.1).chain(catalog(&code)).filter_map(|text| jcj_i18n::Catalog::parse(text).ok()).collect();
    jcj_i18n::set(&code, catalogs);
    code
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::Path;

    fn sources(dir: &Path, out: &mut String) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                sources(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                let text = std::fs::read_to_string(&p).unwrap();
                // Tests' own texts are not the interface.
                out.push_str(text.split("#[cfg(test)]").next().unwrap_or(""));
            }
        }
    }

    #[test]
    fn every_shell_text_is_in_every_catalog_and_none_is_unused() {
        let mut src = String::new();
        sources(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut src);
        let used = jcj_i18n::texts_in(&src);
        assert!(used.len() > 100, "{}", used.len());
        let singulars = jcj_i18n::singulars_in(&src);
        for (code, _) in super::LANGUAGES.iter().filter(|l| l.0 != "en") {
            let cat = jcj_i18n::Catalog::parse(super::catalog(code).unwrap()).unwrap();
            let (missing, unused) = jcj_i18n::compare(&cat, &used, &singulars);
            assert!(missing.is_empty(), "{code} lacks:\n{}", missing.join("\n"));
            assert!(unused.is_empty(), "{code} has unused:\n{}", unused.join("\n"));
            let _: BTreeSet<_> = cat.keys();
        }
    }
}
