// SPDX-License-Identifier: MIT OR Apache-2.0
//! Interface languages (C4) in the testbed: with Spanish on, the shell's texts and the testbed's
//! come from their catalogs, texts neither has stay English, and English comes back unchanged.
//! (Its own test binary: the language is process-wide.)

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use jcj_shell::{Shell, egui, i18n, theme::Tokens};

type State = (Shell, core_testbed::window::Testbed);

fn harness() -> Harness<'static, State> {
    let dir = std::env::temp_dir().join(format!("core-testbed-languages-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let app = core_testbed::window::Testbed::new(dir.join("untitled.jcjtest"));
    Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_ui_state(
        |ui, s: &mut State| {
            let (shell, app) = s;
            shell.ui(ui, app);
        },
        (Shell::new("Core testbed", "0", Tokens::default()), app),
    )
}

#[test]
fn the_window_follows_the_language_and_english_is_unchanged() {
    let mut h = harness();
    h.run();
    assert!(h.query_by_label("Sample").is_some());
    assert_eq!(core_testbed::languages::set("es"), "es");
    h.run();
    assert!(h.query_by_label("Muestra").is_some(), "the testbed's own text");
    assert!(h.query_by_label("Sample").is_none());
    assert!(h.query_by_label("Cerrar pestaña").is_some() || h.query_by_label("Archivo").is_some(), "the shell's text");
    assert_eq!(i18n::language(), "es");
    assert_eq!(i18n::tr("Save"), "Guardar");
    assert_eq!(i18n::trn("{n} card", "{n} cards", 1, &[]), "1 tarjeta");
    assert!(h.query_by_label("0 tarjetas").is_some(), "a count in the status bar");
    assert_eq!(i18n::tr_with("{a} of {b}", &[("a", "1"), ("b", "2")]), "1 of 2");
    assert_eq!(i18n::tr("Bold"), "Bold", "a text no catalog has stays English");
    // A language nobody has falls back to English.
    assert_eq!(core_testbed::languages::set("xx"), "en");
    h.run();
    assert!(h.query_by_label("Sample").is_some());
    assert_eq!(i18n::tr("Save"), "Save");
    assert_eq!(i18n::language(), "en");
}

/// The catalog against the code, with the shell as the only translation dependency: every text
/// the window marks is in every catalog, and no catalog keeps a text nothing uses.
#[test]
fn the_catalog_has_every_text_the_window_marks_and_no_other() {
    let source = include_str!("../src/window.rs");
    let (used, singulars) = (i18n::texts_in(source), i18n::singulars_in(source));
    assert!(used.contains("Sample") && singulars.contains("{n} card"), "{used:?}");
    for (code, text) in core_testbed::languages::CATALOGS {
        let catalog = i18n::Catalog::parse(text).unwrap_or_else(|e| panic!("{code}: {e}"));
        let (missing, unused) = i18n::compare(&catalog, &used, &singulars);
        assert!(missing.is_empty(), "{code} lacks: {missing:?}");
        assert!(unused.is_empty(), "{code} has unused: {unused:?}");
        assert!(matches!(catalog.get("{n} cards"), Some(i18n::Entry::Plural(_))));
    }
}
