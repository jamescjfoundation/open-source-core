// SPDX-License-Identifier: MIT OR Apache-2.0
//! Interface languages (C4): the testbed's few texts in Spanish, layered over the shell's
//! catalog. `CORE_TESTBED_LANG=es core-testbed` shows the window in Spanish.
//!
//! The testbed translates through `jcj_shell::i18n` alone, as an app's interface crate does: it
//! has no translation dependency of its own. `tests/languages.rs` holds this catalog to the
//! texts the window marks.

/// The testbed's own catalogs: language code and catalog text.
pub const CATALOGS: [(&str, &str); 1] = [(
    "es",
    r#"
"Sample" = "Muestra"
"Insert" = "Insertar"
"Add Card" = "Añadir tarjeta"
"Every Format" = "Todos los formatos"
"Self-Test" = "Autoprueba"
"{n} cards" = { one = "{n} tarjeta", other = "{n} tarjetas" }
"#,
)];

/// Show the testbed in `code` ("" follows the computer). Gives the language now used.
pub fn set(code: &str) -> String {
    jcj_shell::i18n::set_language(code, &CATALOGS)
}
