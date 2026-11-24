// SPDX-License-Identifier: MIT OR Apache-2.0
//! `cargo xtask pins`: the upstream pins in `Cargo.toml`, the lock file and `docs/UPSTREAM.md`
//! agree (decision 0004).
//!
//! - Every git `rev` and `tag` named in `Cargo.toml` is recorded in `docs/UPSTREAM.md`.
//! - The lock file builds WordCraft only from our fork, at the commit the patch block names: no
//!   WordCraft crate resolves to storytold's address (that would bypass our patches).

use std::collections::BTreeSet;
use std::path::Path;

use crate::util::{TaskResult, report};

/// Every `(git url, rev or tag)` written in a manifest.
pub fn git_pins(manifest: &str) -> BTreeSet<(String, String)> {
    let mut out = BTreeSet::new();
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        let Some(git) = quoted_after(line, "git = ") else { continue };
        for key in ["rev = ", "tag = "] {
            if let Some(v) = quoted_after(line, key) {
                out.insert((git.trim_end_matches(".git").to_owned(), v));
            }
        }
    }
    out
}

fn quoted_after(line: &str, key: &str) -> Option<String> {
    let rest = &line[line.find(key)? + key.len()..];
    let rest = rest.strip_prefix('"')?;
    Some(rest[..rest.find('"')?].to_owned())
}

pub fn problems(manifest: &str, lock: &str, upstream_md: &str) -> Vec<String> {
    let mut failures = Vec::new();
    let pins = git_pins(manifest);
    for (git, pin) in &pins {
        if !upstream_md.contains(pin.as_str()) {
            failures.push(format!("{git} at {pin} is not recorded in docs/UPSTREAM.md"));
        }
    }
    for line in lock.lines().filter(|l| l.starts_with("source = ")) {
        if line.contains("github.com/storytold/wordcraft") {
            failures.push("Cargo.lock resolves a WordCraft crate from storytold's address; the patch block must cover it".to_owned());
            break;
        }
    }
    let fork: BTreeSet<&str> = pins.iter().filter(|(g, _)| g.ends_with("jamescjfoundation/wordcraft")).map(|(_, p)| p.as_str()).collect();
    if fork.len() != 1 {
        failures.push(format!("the patch block must name exactly one fork commit, found {}", fork.len()));
    }
    for rev in fork {
        if !lock.contains(&format!("jamescjfoundation/wordcraft?rev={rev}#{rev}")) {
            failures.push(format!("Cargo.lock does not build WordCraft from the fork commit {rev}"));
        }
    }
    failures
}

/// The newest `vX.Y.Z` among `ls-remote --tags` lines.
pub fn newest_release(ls_remote: &str) -> Option<(u32, u32, u32)> {
    ls_remote
        .lines()
        .filter_map(|l| l.split("refs/tags/").nth(1))
        .filter(|t| !t.ends_with("^{}"))
        .filter_map(|t| {
            let mut parts = t.strip_prefix('v')?.split('.').map(|p| p.parse::<u32>().ok());
            Some((parts.next()??, parts.next()??, parts.next()??))
        })
        .max()
}

/// The WordCraft release tag the workspace pins.
pub fn pinned_release(manifest: &str) -> Option<String> {
    git_pins(manifest).into_iter().find(|(g, _)| g.ends_with("storytold/wordcraft")).map(|(_, tag)| tag)
}

/// `cargo xtask upstream-status`: the pinned WordCraft release and the newest one upstream. Prints
/// `newer=<tag>` when a train has something to move to (the scheduled workflow reads it).
pub fn status(root: &Path) -> TaskResult {
    let manifest = std::fs::read_to_string(root.join("Cargo.toml")).map_err(|e| format!("Cargo.toml: {e}"))?;
    let pinned = pinned_release(&manifest).ok_or("no WordCraft release tag in Cargo.toml")?;
    let out = crate::util::git_output(root, &["ls-remote", "--tags", "https://github.com/storytold/wordcraft"])?;
    let (a, b, c) = newest_release(&out).ok_or("storytold/wordcraft has no release tags")?;
    let newest = format!("v{a}.{b}.{c}");
    println!("pinned={pinned}");
    println!("newest={newest}");
    if newest != pinned && newest_release(&format!("x refs/tags/{pinned}")) < Some((a, b, c)) {
        println!("newer={newest}");
    }
    Ok(())
}

pub fn task(root: &Path) -> TaskResult {
    let read = |p: &str| std::fs::read_to_string(root.join(p)).map_err(|e| format!("cannot read {p}: {e}"));
    let failures = problems(&read("Cargo.toml")?, &read("Cargo.lock")?, &read("docs/UPSTREAM.md")?);
    report(failures, "upstream pins, lock file and docs/UPSTREAM.md agree")
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = r#"
a = { git = "https://github.com/storytold/wordcraft", tag = "v1.0.0", version = "=1.0.0" }
# b = { git = "https://example.invalid/x", rev = "ignored" }
c = { git = "https://github.com/jamescjfoundation/wordcraft", rev = "abc123" }
"#;

    #[test]
    fn finds_the_newest_release() {
        let out = "a refs/tags/v0.9.0\nb refs/tags/v0.18.1\nc refs/tags/v0.18.1^{}\nd refs/tags/v0.2.10\ne refs/tags/nightly\n";
        assert_eq!(newest_release(out), Some((0, 18, 1)));
        assert_eq!(pinned_release(MANIFEST).as_deref(), Some("v1.0.0"));
    }

    #[test]
    fn reads_pins() {
        let pins = git_pins(MANIFEST);
        assert_eq!(pins.len(), 2);
        assert!(pins.contains(&("https://github.com/storytold/wordcraft".to_owned(), "v1.0.0".to_owned())));
    }

    #[test]
    fn checks_docs_and_lock() {
        let good_lock = "source = \"git+https://github.com/jamescjfoundation/wordcraft?rev=abc123#abc123\"\n";
        assert!(problems(MANIFEST, good_lock, "v1.0.0 abc123").is_empty());
        assert_eq!(problems(MANIFEST, good_lock, "abc123").len(), 1);
        let bad_lock = "source = \"git+https://github.com/storytold/wordcraft?tag=v1.0.0#x\"\n";
        assert_eq!(problems(MANIFEST, bad_lock, "v1.0.0 abc123").len(), 2);
    }
}
