// SPDX-License-Identifier: MIT OR Apache-2.0
//! Interface languages (C4) in the testbed: with Spanish on, the shell's texts and the testbed's
//! come from their catalogs, texts neither has stay English, and English comes back unchanged.
//! (Its own test binary: the language is process-wide.)

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use jcj_shell::{Shell, egui, theme::Tokens};

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
    assert_eq!(jcj_i18n::tr("Save"), "Guardar");
    assert_eq!(jcj_i18n::trn("{n} page", "{n} pages", 1, &[]), "1 página");
    assert_eq!(jcj_i18n::tr("Bold"), "Bold", "a text no catalog has stays English");
    // A language nobody has falls back to English.
    assert_eq!(core_testbed::languages::set("xx"), "en");
    h.run();
    assert!(h.query_by_label("Sample").is_some());
    assert_eq!(jcj_i18n::tr("Save"), "Save");
}
