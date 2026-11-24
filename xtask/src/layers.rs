// SPDX-License-Identifier: MIT OR Apache-2.0
//! The layer rule (decision 0002).
//!
//! 1. `jcj-upstream` is the only crate that depends on WordCraft or craft-libs directly.
//! 2. Every workspace crate has a layer; a crate may depend on crates in its own layer or below.
//! 3. App repositories depend only on `jcj-` crates, never on `jcj-upstream` (checked by the
//!    app's own xtask, from the template).

/// Crates that are tooling, not product, and are not layered.
pub const UNLAYERED: &[&str] = &["xtask"];

/// (crate, layer). Higher numbers sit higher in the stack.
pub const LAYERS: &[(&str, u8)] = &[
    // The one door to upstream
    ("jcj-upstream", 1),
    // The model and proofing
    ("jcj-doc", 2),
    ("jcj-proof", 2),
    // Entries (typed records beside documents)
    ("jcj-entries", 2),
    // Layout and the editing engine
    ("jcj-layout", 3),
    ("jcj-engine", 3),
    // Output, files, formats and agents
    ("jcj-output", 4),
    ("jcj-project", 4),
    ("jcj-interchange", 4),
    ("jcj-agents", 4),
    // Document features (bookmarks, notes, revisions, page locks, ...)
    ("jcj-features", 4),
    // Interface building blocks
    ("jcj-theme", 5),
    ("jcj-board", 5),
    // The shell
    ("jcj-shell", 6),
    // Test app and contract tests: may use everything
    ("core-testbed", 7),
    ("contract-tests", 7),
];

/// Crate name prefixes that belong to upstream projects.
pub const UPSTREAM_PREFIXES: &[&str] = &["wordcraft-", "craft-"];

/// The only crate allowed to name upstream crates.
pub const UPSTREAM_DOOR: &str = "jcj-upstream";

pub fn layer_of(name: &str) -> Option<u8> {
    LAYERS.iter().find(|(n, _)| *n == name).map(|(_, l)| *l)
}

pub fn is_upstream(dep: &str) -> bool {
    UPSTREAM_PREFIXES.iter().any(|p| dep.starts_with(p))
}

/// Checks one direct dependency of a workspace crate. `workspace` says whether `to` is a member.
pub fn check_edge(from: &str, to: &str, workspace: bool) -> Result<(), String> {
    if UNLAYERED.contains(&from) {
        return Ok(());
    }
    if !workspace {
        if is_upstream(to) && from != UPSTREAM_DOOR {
            return Err(format!("{from} depends on upstream crate {to} directly; reach it through jcj-upstream"));
        }
        return Ok(());
    }
    if UNLAYERED.contains(&to) {
        return Err(format!("{from} depends on tooling crate {to}"));
    }
    let lf = layer_of(from).ok_or_else(|| format!("{from} has no layer in xtask/src/layers.rs"))?;
    let lt = layer_of(to).ok_or_else(|| format!("{to} has no layer in xtask/src/layers.rs"))?;
    if lt > lf {
        return Err(format!("{from} (layer {lf}) depends on {to} (layer {lt}); dependencies must point down"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edges() {
        assert!(check_edge("jcj-shell", "jcj-theme", true).is_ok());
        assert!(check_edge("jcj-doc", "jcj-upstream", true).is_ok());
        assert!(check_edge("jcj-doc", "jcj-shell", true).is_err());
        assert!(check_edge("jcj-new", "jcj-doc", true).is_err());
        assert!(check_edge("jcj-shell", "xtask", true).is_err());
        assert!(check_edge("xtask", "jcj-shell", true).is_ok());
    }

    #[test]
    fn only_the_door_names_upstream() {
        assert!(check_edge("jcj-upstream", "wordcraft-doc", false).is_ok());
        assert!(check_edge("jcj-doc", "wordcraft-doc", false).is_err());
        assert!(check_edge("jcj-shell", "craft-ui", false).is_err());
        assert!(check_edge("jcj-shell", "egui", false).is_ok());
    }

    #[test]
    fn upstream_is_the_bottom() {
        for (name, layer) in LAYERS {
            if *name != UPSTREAM_DOOR {
                assert!(*layer > 1, "{name} shares the upstream layer");
            }
        }
    }
}
