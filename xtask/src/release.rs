// SPDX-License-Identifier: MIT OR Apache-2.0
//! `cargo xtask release`: the release commit, and what the release workflow asks about it
//! (decision 0011).
//!
//! - `cargo xtask release <version>` makes the changes of a release commit: the version in
//!   `Cargo.toml` and `Cargo.lock`, the tag on every `jcj-` line of `template/Cargo.toml`, and
//!   the changelog's `## Unreleased` heading, which becomes `## <version> (<date>)`. It refuses
//!   when the section has no **What apps must do**: a release tells apps what to do, even when
//!   that is only to move the pin.
//! - `cargo xtask release --check` says whether the checkout is a release commit, and prints
//!   `version=<version>`. The release workflow runs it before the gate.
//! - `cargo xtask release --notes` prints the changelog section of the checkout's version: the
//!   text of the GitHub release.
//! - `cargo xtask release --override <owner>/<repository>` prints the reason the newest
//!   changelog section gives for going ahead although that app's gate fails against the core,
//!   and fails when it gives none. The job that builds the apps asks it for an app that failed.
//!
//! The tag and the GitHub release are made by `.github/workflows/release.yml`, only after the
//! gate has passed on the commit. Nothing here tags or pushes.

use std::path::Path;

use crate::shared::link::{self, CORE_GIT};
// What reads a manifest and a changelog is shared with the apps' own `release` task.
pub use crate::shared::release::{
    Section, UNRELEASED, is_date, released_heading, sections, today, version, with_workspace_version, workspace_version,
};
use crate::shared::util::{TaskResult, cargo_output};
/// The heading every section needs before it can be released.
pub const APPS_MUST_DO: &str = "### What apps must do";
/// How an override line starts, before the app's repository. The whole line is
/// `**App gate override, <owner>/<repository>:** <reason>`.
pub const OVERRIDE: &str = "**App gate override, ";
/// The fewest characters a reason may have: a sentence, not a word.
const SHORTEST_REASON: usize = 20;

const USAGE: &str = "usage: cargo xtask release <version> [--date YYYY-MM-DD]\n       \
                     cargo xtask release --check | --notes | --override <owner>/<repository>";

/// `cargo xtask release ...`.
pub fn task(root: &Path, args: &[String]) -> TaskResult {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["--check"] => check(root).map(|version| println!("version={version}")),
        ["--notes"] => notes(root).map(|text| println!("{text}")),
        ["--override", app] => override_task(root, app),
        [version] if !version.starts_with('-') => cut(root, version, &today()),
        [version, "--date", date] if !version.starts_with('-') => cut(root, version, date),
        _ => Err(USAGE.to_owned()),
    }
}

/// The tags the template's manifest names on its lines for the core's crates, in order.
pub fn template_tags(manifest: &str) -> Vec<&str> {
    let lines = manifest.lines().filter(|l| !l.trim_start().starts_with('#'));
    lines.filter(|l| link::value_of(l, "git").is_some_and(|git| link::same_address(git, CORE_GIT))).filter_map(|l| link::value_of(l, "tag")).collect()
}

/// The template's manifest with `tag` on every line for a crate of the core, and how many
/// lines that is.
pub fn with_template_tags(manifest: &str, tag: &str) -> (String, usize) {
    let mut out = String::with_capacity(manifest.len());
    let mut lines = 0;
    for line in manifest.split_inclusive('\n') {
        let is_core = !line.trim_start().starts_with('#') && link::value_of(line, "git").is_some_and(|git| link::same_address(git, CORE_GIT));
        match link::value_of(line, "tag").filter(|_| is_core) {
            Some(old) => {
                out.push_str(&line.replacen(&format!("tag = \"{old}\""), &format!("tag = \"{tag}\""), 1));
                lines += 1;
            }
            None => out.push_str(line),
        }
    }
    (out, lines)
}

/// Whether a section says what apps must do: the heading, and at least one line of text under
/// it before the next heading.
pub fn says_what_apps_must_do(section: &Section) -> bool {
    let Some(at) = section.body.iter().position(|l| l.trim() == APPS_MUST_DO) else { return false };
    let below = section.body.iter().skip(at + 1).take_while(|l| !l.starts_with("### "));
    below.into_iter().any(|l| !l.trim().is_empty())
}

/// The changelog with its `## Unreleased` section turned into the section of `new`, dated
/// `date`. Refuses a changelog whose newest section is not `Unreleased`, one that already has a
/// section for `new`, and a section that does not say what apps must do.
pub fn with_release(changelog: &str, new: &str, date: &str) -> Result<String, String> {
    let all = sections(changelog);
    let Some(top) = all.first().filter(|s| s.heading == UNRELEASED) else {
        return Err(format!(
            "CHANGELOG.md: the newest section is not `## {UNRELEASED}`. Write what this release changes for apps under that heading first"
        ));
    };
    if all.iter().any(|s| released_heading(s.heading).is_some_and(|(number, _)| number == new)) {
        return Err(format!("CHANGELOG.md already has a section for {new}"));
    }
    if !says_what_apps_must_do(top) {
        return Err(format!(
            "CHANGELOG.md: `## {UNRELEASED}` has no `{APPS_MUST_DO}` with text under it. Every release says what an app does to take it, \
             starting with the pin move, even when that is all"
        ));
    }
    let mut out = String::with_capacity(changelog.len() + 16);
    for (i, line) in changelog.split_inclusive('\n').enumerate() {
        if i == top.line {
            out.push_str(&format!("## {new} ({date})\n"));
        } else {
            out.push_str(line);
        }
    }
    Ok(out)
}

/// The reason a section gives for going ahead although `app`'s gate fails against the core:
/// the text of the line `**App gate override, <app>:** <reason>` (as it stands, or as an item
/// of a list), with the lines that continue it. `None` without that line, or with a reason
/// shorter than a sentence.
pub fn override_reason(section: &Section, app: &str) -> Option<String> {
    let start = format!("{OVERRIDE}{app}:**");
    let at = section.body.iter().position(|l| l.trim_start().trim_start_matches("- ").starts_with(&start))?;
    let first = section.body.get(at)?.trim_start().trim_start_matches("- ").strip_prefix(&start)?.trim();
    let mut reason = first.to_owned();
    // A paragraph or a list item goes on until a blank line, a heading or the next item.
    for line in section.body.iter().skip(at + 1).map(|l| l.trim()) {
        if line.is_empty() || line.starts_with('#') || line.starts_with("- ") || line.starts_with("**") {
            break;
        }
        reason.push(' ');
        reason.push_str(line);
    }
    let reason = reason.trim().to_owned();
    (reason.chars().count() >= SHORTEST_REASON).then_some(reason)
}

fn read(root: &Path, file: &str) -> Result<String, String> {
    std::fs::read_to_string(root.join(file)).map_err(|e| format!("cannot read {file}: {e}"))
}

fn write(root: &Path, file: &str, text: &str) -> TaskResult {
    std::fs::write(root.join(file), text).map_err(|e| format!("cannot write {file}: {e}"))
}

/// `cargo xtask release <version>`: the four files of a release commit.
fn cut(root: &Path, new: &str, date: &str) -> TaskResult {
    let wanted = version(new).ok_or_else(|| format!("'{new}' is not a version (X.Y.Z). {USAGE}"))?;
    if !is_date(date) {
        return Err(format!("'{date}' is not a date (YYYY-MM-DD)"));
    }
    let manifest = read(root, "Cargo.toml")?;
    let now = workspace_version(&manifest).ok_or("Cargo.toml has no version under [workspace.package]")?;
    if version(now).is_none_or(|now| wanted <= now) {
        return Err(format!("{new} is not newer than {now}, the version in Cargo.toml"));
    }
    // Everything is worked out before anything is written, so a refusal changes no file.
    let new_manifest = with_workspace_version(&manifest, new).ok_or("Cargo.toml: could not write the version under [workspace.package]")?;
    let (new_template, lines) = with_template_tags(&read(root, "template/Cargo.toml")?, &format!("v{new}"));
    if lines == 0 {
        return Err("template/Cargo.toml names no crate of the core by tag".to_owned());
    }
    let new_changelog = with_release(&read(root, "CHANGELOG.md")?, new, date)?;
    write(root, "Cargo.toml", &new_manifest)?;
    write(root, "template/Cargo.toml", &new_template)?;
    write(root, "CHANGELOG.md", &new_changelog)?;
    // Cargo brings the workspace's own packages to the new version in the lock file and moves
    // nothing else.
    cargo_output(root, &["metadata", "--format-version", "1"])?;
    println!("open-source-core {new} ({date}): Cargo.toml, Cargo.lock, template/Cargo.toml ({lines} lines at v{new}) and CHANGELOG.md are changed.");
    println!("Next:");
    println!("  1. cargo xtask ci");
    println!("  2. commit the four files as \"open-source-core {new}\" and push to main");
    println!("  3. gh workflow run release.yml --ref main");
    println!("     It runs the gate on Linux, macOS and Windows and builds the apps; when all of it has passed it");
    println!("     makes the tag v{new} and publishes the release. Do not make the tag by hand.");
    Ok(())
}

/// What stops the checkout from being a release commit, given its three files.
pub fn release_problems(manifest: &str, template: &str, changelog: &str) -> Result<String, Vec<String>> {
    let Some(now) = workspace_version(manifest).filter(|v| version(v).is_some()) else {
        return Err(vec!["Cargo.toml has no version X.Y.Z under [workspace.package]".to_owned()]);
    };
    let mut problems = Vec::new();
    let tags = template_tags(template);
    if tags.is_empty() || tags.iter().any(|tag| tag.strip_prefix('v') != Some(now)) {
        problems.push(format!("template/Cargo.toml does not name v{now} on every line for a crate of the core"));
    }
    let all = sections(changelog);
    match all.first() {
        Some(top) if released_heading(top.heading).is_some_and(|(number, _)| number == now) => {
            if !says_what_apps_must_do(top) {
                problems.push(format!("CHANGELOG.md: the section of {now} has no `{APPS_MUST_DO}` with text under it"));
            }
        }
        Some(top) => problems.push(format!(
            "CHANGELOG.md: the newest section is `## {}`, not `## {now} (date)`. A release is cut on its release commit (`cargo xtask release <version>`)",
            top.heading
        )),
        None => problems.push("CHANGELOG.md has no section".to_owned()),
    }
    if problems.is_empty() { Ok(now.to_owned()) } else { Err(problems) }
}

/// `cargo xtask release --check`: the checkout is a release commit; its version.
fn check(root: &Path) -> Result<String, String> {
    let found = release_problems(&read(root, "Cargo.toml")?, &read(root, "template/Cargo.toml")?, &read(root, "CHANGELOG.md")?);
    let now = found.map_err(|problems| format!("not a release commit:\n  - {}", problems.join("\n  - ")))?;
    cargo_output(root, &["metadata", "--format-version", "1", "--locked"])
        .map_err(|e| format!("Cargo.lock is not in line with the manifests (`cargo metadata --format-version 1 >/dev/null` brings it there): {e}"))?;
    Ok(now)
}

/// `cargo xtask release --notes`: the changelog section of the checkout's version.
fn notes(root: &Path) -> Result<String, String> {
    let manifest = read(root, "Cargo.toml")?;
    let now = workspace_version(&manifest).ok_or("Cargo.toml has no version under [workspace.package]")?;
    let changelog = read(root, "CHANGELOG.md")?;
    let all = sections(&changelog);
    let section = all
        .iter()
        .find(|s| released_heading(s.heading).is_some_and(|(number, _)| number == now))
        .ok_or_else(|| format!("CHANGELOG.md has no section for {now}"))?;
    let text = section.body.join("\n").trim().to_owned();
    if text.is_empty() { Err(format!("CHANGELOG.md: the section of {now} is empty")) } else { Ok(text) }
}

/// `cargo xtask release --override <owner>/<repository>`.
fn override_task(root: &Path, app: &str) -> TaskResult {
    let changelog = read(root, "CHANGELOG.md")?;
    let all = sections(&changelog);
    let top = all.first().ok_or("CHANGELOG.md has no section")?;
    match override_reason(top, app) {
        Some(reason) => {
            println!("{reason}");
            Ok(())
        }
        None => Err(format!(
            "CHANGELOG.md: `## {}` has no override for {app}. To go ahead although that app's gate fails against this commit, the section \
             carries the line `{OVERRIDE}{app}:** <the reason, a sentence or more>`",
            top.heading
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG: &str = "# Changelog\n\nIntro.\n\n## Unreleased\n\n### Added\n\n- A thing.\n\n```toml\n## not a heading\n```\n\n### What apps must do\n\n\
                       1. Move every `jcj-` dependency to the new tag.\n\n## 0.1.20 (2026-11-26)\n\n### What apps must do\n\nNothing.\n";

    #[test]
    fn reads_versions_and_dates() {
        assert_eq!(version("0.1.21"), Some((0, 1, 21)));
        assert_eq!(version("12.0.3"), Some((12, 0, 3)));
        for other in ["", "0.1", "0.1.21.4", "v0.1.21", "0.1.x", "0.1.21-rc1", "0..1", " 0.1.2"] {
            assert_eq!(version(other), None, "{other}");
        }
        assert!(is_date("2026-11-26"));
        for other in ["2026-11-6", "26-11-2026", "2026/11/26", "today", ""] {
            assert!(!is_date(other), "{other}");
        }
    }

    #[test]
    fn writes_the_version_and_the_template_tags() {
        let manifest = "[workspace]\nmembers = []\n\n[workspace.package]\nedition = \"2024\"\nversion = \"0.1.20\"\nrust-version = \"1.95\"\n\n\
                        [workspace.dependencies]\nserde = { version = \"0.1.20\" }\n";
        assert_eq!(workspace_version(manifest), Some("0.1.20"));
        let new = with_workspace_version(manifest, "0.1.21").unwrap();
        assert!(new.contains("[workspace.package]\nedition = \"2024\"\nversion = \"0.1.21\"\n"));
        // A dependency that happens to carry the same number is left alone.
        assert!(new.contains("serde = { version = \"0.1.20\" }"));
        assert_eq!(workspace_version("[package]\nversion = \"1.0.0\"\n"), None);
        assert_eq!(with_workspace_version("[package]\nversion = \"1.0.0\"\n", "1.0.1"), None);

        let template = "[workspace.dependencies]\n# The core, at a release tag.\n\
                        jcj-doc = { git = \"https://github.com/jamescjfoundation/open-source-core\", tag = \"v0.1.20\" }\n\
                        jcj-shell = { git = \"https://github.com/jamescjfoundation/open-source-core\", tag = \"v0.1.20\" }\n\
                        other = { git = \"https://example.invalid/other\", tag = \"v0.1.20\" }\n";
        assert_eq!(template_tags(template), ["v0.1.20", "v0.1.20"]);
        let (new, lines) = with_template_tags(template, "v0.1.21");
        assert_eq!(lines, 2);
        assert_eq!(template_tags(&new), ["v0.1.21", "v0.1.21"]);
        assert!(new.contains("other = { git = \"https://example.invalid/other\", tag = \"v0.1.20\" }"));
        assert_eq!(with_template_tags("[workspace]\n", "v1.0.0").1, 0);
    }

    #[test]
    fn turns_unreleased_into_the_release() {
        let found = sections(LOG);
        assert_eq!(found.iter().map(|s| s.heading).collect::<Vec<_>>(), ["Unreleased", "0.1.20 (2026-11-26)"]);
        assert!(says_what_apps_must_do(&found[0]) && says_what_apps_must_do(&found[1]));
        let new = with_release(LOG, "0.1.21", "2026-11-27").unwrap();
        assert!(new.contains("\n## 0.1.21 (2026-11-27)\n\n### Added\n"));
        assert!(!new.contains("## Unreleased"));
        assert_eq!(new.lines().count(), LOG.lines().count());
        // The code block's line is not taken for a heading, before or after.
        assert_eq!(sections(&new).len(), 2);
        assert_eq!(released_heading("0.1.21 (2026-11-27)"), Some(("0.1.21", "2026-11-27")));
        assert_eq!(released_heading("Unreleased"), None);
        assert_eq!(released_heading("0.1.21"), None);
    }

    #[test]
    fn refuses_a_release_that_does_not_say_what_apps_must_do() {
        // No heading at all.
        let none = LOG.replacen("### What apps must do\n\n1. Move every `jcj-` dependency to the new tag.\n\n", "", 1);
        assert!(with_release(&none, "0.1.21", "2026-11-27").unwrap_err().contains("What apps must do"));
        // The heading with nothing under it.
        let empty = LOG.replacen("1. Move every `jcj-` dependency to the new tag.\n", "", 1);
        assert!(with_release(&empty, "0.1.21", "2026-11-27").unwrap_err().contains("What apps must do"));
        // The heading with only another heading under it.
        let other = LOG.replacen("1. Move every `jcj-` dependency to the new tag.\n", "### Fixed\n\n- A thing.\n", 1);
        assert!(with_release(&other, "0.1.21", "2026-11-27").unwrap_err().contains("What apps must do"));
        // No Unreleased section, and a version that is already there.
        let released = LOG.replacen("## Unreleased", "## 0.1.21 (2026-11-27)", 1);
        assert!(with_release(&released, "0.1.22", "2026-11-28").unwrap_err().contains("is not `## Unreleased`"));
        assert!(with_release(LOG, "0.1.20", "2026-11-27").unwrap_err().contains("already has a section"));
    }

    #[test]
    fn knows_a_release_commit() {
        let manifest = "[workspace.package]\nversion = \"0.1.21\"\n";
        let template = "jcj-doc = { git = \"https://github.com/jamescjfoundation/open-source-core\", tag = \"v0.1.21\" }\n";
        let log = with_release(LOG, "0.1.21", "2026-11-27").unwrap();
        assert_eq!(release_problems(manifest, template, &log), Ok("0.1.21".to_owned()));
        // The changelog still says Unreleased, or belongs to another version.
        let found = release_problems(manifest, template, LOG).unwrap_err();
        assert!(found.len() == 1 && found[0].contains("the newest section is `## Unreleased`"), "{found:?}");
        let older = "[workspace.package]\nversion = \"0.1.20\"\n";
        assert!(release_problems(older, template, &log).is_err());
        // The template still names the release before.
        let behind = template.replace("v0.1.21", "v0.1.20");
        let found = release_problems(manifest, &behind, &log).unwrap_err();
        assert!(found.len() == 1 && found[0].contains("template/Cargo.toml"), "{found:?}");
        assert!(release_problems(manifest, "", &log).is_err());
        assert!(release_problems("[package]\nname = \"x\"\n", template, &log).is_err());
    }

    #[test]
    fn an_override_names_the_app_and_gives_a_reason() {
        let log = "## Unreleased\n\n### What apps must do\n\n1. Move the pin.\n\n\
                   **App gate override, example/app-one:** its export test compares a file this release changes on purpose;\n\
                   the app updates its golden file when it moves.\n\n\
                   - **App gate override, example/app-two:** too short\n\
                   - **App gate override, example/app-three:** the app is on a core from before the gate could be linked.\n\n\
                   ## 0.1.20 (2026-11-26)\n\n**App gate override, example/app-old:** a reason that belongs to the release before this one.\n";
        let all = sections(log);
        let top = &all[0];
        assert_eq!(
            override_reason(top, "example/app-one").as_deref(),
            Some("its export test compares a file this release changes on purpose; the app updates its golden file when it moves.")
        );
        // As an item of a list; and one app's line does not cover another.
        assert_eq!(override_reason(top, "example/app-three").as_deref(), Some("the app is on a core from before the gate could be linked."));
        assert_eq!(override_reason(top, "example/app"), None);
        assert_eq!(override_reason(top, "example/app-four"), None);
        // A reason must be a sentence, and an older release's override is not this one's.
        assert_eq!(override_reason(top, "example/app-two"), None);
        assert_eq!(override_reason(top, "example/app-old"), None);
        assert!(override_reason(&all[1], "example/app-old").is_some());
    }
}
