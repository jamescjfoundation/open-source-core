// SPDX-License-Identifier: MIT OR Apache-2.0
//! The `workflows` task: what the workflow files must hold that nothing on GitHub says until a
//! run fails (decision 0003).
//!
//! - Every action is one of GitHub's own, named by a full commit, and every reusable workflow
//!   is one of this repository's. The organisation refuses anything else, but only when the
//!   workflow runs.
//! - A tool a workflow downloads (its version, and the checksum of each file) is written in the
//!   `env` of that workflow. Two workflows that download the same tool agree on it.
//! - A script that several jobs must run word for word (the name of what CI keeps between runs,
//!   which two workflows must work out alike to share an archive) sits between the lines
//!   `# BEGIN same: NAME` and `# END same: NAME` in each, and every copy is the same text.
//! - `.github/apps.json`, the list of app repositories built against a core commit, can be
//!   read: the job that reads it runs once a day and before a release, and a slip there would
//!   show only then.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::shared::json;
use crate::shared::link::CORE_GIT;
use crate::shared::util::{TaskResult, report};

/// The folders whose workflow files are checked.
const FOLDERS: &[&str] = &[".github/workflows", "template/.github/workflows"];
/// The list of apps the nightly run builds against the checkout.
pub const APPS: &str = ".github/apps.json";

/// What `uses:` names on a line of a workflow, if the line has one.
fn used(line: &str) -> Option<&str> {
    let line = line.trim().trim_start_matches("- ");
    let value = line.strip_prefix("uses:")?.trim();
    Some(value.split_whitespace().next().unwrap_or_default().trim_matches(['"', '\'']))
}

/// The lines of a workflow that use something the rules do not allow.
pub fn uses_problems(file: &str, workflow: &str) -> Vec<String> {
    let core = CORE_GIT.trim_start_matches("https://github.com/");
    let own_workflow = format!("{core}/.github/workflows/");
    let mut out = Vec::new();
    for (i, line) in workflow.lines().enumerate() {
        let Some(what) = used(line).filter(|_| !line.trim_start().starts_with('#')) else { continue };
        let at = format!("{file}:{}", i + 1);
        if what.starts_with("./") {
            continue;
        }
        let Some((name, version)) = what.split_once('@') else {
            out.push(format!("{at}: `{what}` names no commit"));
            continue;
        };
        if name.starts_with(&own_workflow) {
            // The one reference by a tag that moves: the core's own file, called by an app.
            if !version.starts_with("workflows-v") {
                out.push(format!("{at}: `{what}`: a shared workflow is called at a `workflows-vN` tag"));
            }
        } else if !name.starts_with("actions/") {
            out.push(format!(
                "{at}: `{name}` is not one of GitHub's own actions (actions/...). A tool is downloaded and its checksum checked instead"
            ));
        } else if !(version.len() == 40 && version.bytes().all(|b| b.is_ascii_hexdigit())) {
            out.push(format!("{at}: `{what}` is not pinned to a full commit (40 characters), with the version in a comment after it"));
        }
    }
    out
}

/// The entries of a workflow's top-level `env` that pin a download: `NAME: "value"` where the
/// name ends in `_VERSION` or `_SHA256`.
pub fn tool_pins(workflow: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut in_env = false;
    for line in workflow.lines() {
        if !line.starts_with([' ', '#']) && !line.is_empty() {
            in_env = line.trim_end() == "env:";
            continue;
        }
        let Some((name, value)) = line.trim().split_once(':').filter(|_| in_env && line.starts_with("  ") && !line.starts_with("   ")) else {
            continue;
        };
        if name.ends_with("_VERSION") || name.ends_with("_SHA256") {
            out.insert(name.to_owned(), value.trim().trim_matches('"').to_owned());
        }
    }
    out
}

/// What is wrong with the pins of several workflows (file, pins): a checksum that is not one,
/// and a name with different values in two files.
pub fn pin_problems(files: &[(String, BTreeMap<String, String>)]) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen: BTreeMap<&str, (&str, &str)> = BTreeMap::new();
    for (file, pins) in files {
        for (name, value) in pins {
            if name.ends_with("_SHA256") && !(value.len() == 64 && value.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))) {
                out.push(format!("{file}: {name} is not a SHA-256 checksum (64 characters, lower case)"));
            }
            match seen.get(name.as_str()) {
                Some((first, other)) if other != value => {
                    out.push(format!("{name} is `{other}` in {first} and `{value}` in {file}: a tool is one version, with one checksum, everywhere"))
                }
                Some(_) => {}
                None => {
                    seen.insert(name, (file, value));
                }
            }
        }
    }
    out
}

/// The blocks of a workflow between `# BEGIN same: NAME` and `# END same: NAME` lines: each
/// name with its lines, without the indentation the block stands at.
pub fn same_blocks(workflow: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut open: Option<(String, Vec<&str>)> = None;
    for line in workflow.lines() {
        let mark = line.trim();
        if let Some(name) = mark.strip_prefix("# BEGIN same: ") {
            open = Some((name.to_owned(), Vec::new()));
        } else if let Some(name) = mark.strip_prefix("# END same: ") {
            if let Some((began, lines)) = open.take().filter(|(began, _)| began == name) {
                let indent = lines.iter().filter(|l| !l.trim().is_empty()).map(|l| l.len() - l.trim_start().len()).min().unwrap_or(0);
                let text: Vec<&str> = lines.iter().map(|l| l.get(indent..).unwrap_or("").trim_end()).collect();
                out.push((began, text.join("\n")));
            }
        } else if let Some((_, lines)) = open.as_mut() {
            lines.push(line);
        }
    }
    out
}

/// The names whose copies differ, in the files given (file, blocks): a copy that differs from
/// the first one found, and a block that was opened and not closed.
pub fn same_problems(files: &[(String, String)]) -> Vec<String> {
    let mut out = Vec::new();
    let mut first: BTreeMap<String, (String, String)> = BTreeMap::new();
    for (file, workflow) in files {
        let blocks = same_blocks(workflow);
        let begun = workflow.lines().filter(|l| l.trim().starts_with("# BEGIN same: ")).count();
        if begun != blocks.len() {
            out.push(format!("{file}: a `# BEGIN same:` line has no `# END same:` line with the same name after it"));
        }
        for (name, text) in blocks {
            match first.get(&name) {
                Some((_, known)) if *known == text => {}
                Some((other, _)) => {
                    out.push(format!("the `same: {name}` block of {file} differs from the one in {other}: every copy is the same text"))
                }
                None => {
                    first.insert(name, (file.clone(), text));
                }
            }
        }
    }
    out
}

/// The repositories `.github/apps.json` lists, or what is wrong with the file.
pub fn apps(text: &str) -> Result<Vec<String>, Vec<String>> {
    let value = json::parse(text).map_err(|e| vec![format!("{APPS}: {e}")])?;
    let Some(list) = value.get("apps") else { return Err(vec![format!("{APPS}: no `apps` list")]) };
    let mut out = Vec::new();
    let mut problems = Vec::new();
    let mut seen = BTreeSet::new();
    let part =
        |s: &str| !s.is_empty() && !s.starts_with(['.', '-']) && s.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'));
    for item in list.items() {
        match item.as_str().and_then(|name| name.split_once('/').map(|parts| (name, parts))) {
            Some((name, (owner, repository))) if part(owner) && part(repository) => {
                if seen.insert(name.to_ascii_lowercase()) {
                    out.push(name.to_owned());
                } else {
                    problems.push(format!("{APPS}: {name} is listed twice"));
                }
            }
            _ => problems.push(format!("{APPS}: every entry of `apps` is a repository written \"owner/name\"")),
        }
    }
    if problems.is_empty() { Ok(out) } else { Err(problems) }
}

/// The `workflows` task of the gate.
pub fn task(root: &Path) -> TaskResult {
    let mut failures = Vec::new();
    let mut pins = Vec::new();
    let mut texts = Vec::new();
    let mut checked = 0usize;
    for folder in FOLDERS {
        let entries = std::fs::read_dir(root.join(folder)).map_err(|e| format!("cannot read {folder}: {e}"))?;
        let mut names: Vec<String> = entries.flatten().filter_map(|e| e.file_name().to_str().map(str::to_owned)).collect();
        names.sort();
        for name in names.iter().filter(|n| n.ends_with(".yml") || n.ends_with(".yaml")) {
            let file = format!("{folder}/{name}");
            let text = std::fs::read_to_string(root.join(&file)).map_err(|e| format!("cannot read {file}: {e}"))?;
            failures.extend(uses_problems(&file, &text));
            pins.push((file.clone(), tool_pins(&text)));
            texts.push((file, text));
            checked += 1;
        }
    }
    failures.extend(pin_problems(&pins));
    failures.extend(same_problems(&texts));
    let listed = match std::fs::read_to_string(root.join(APPS)) {
        Ok(text) => apps(&text).unwrap_or_else(|problems| {
            failures.extend(problems);
            Vec::new()
        }),
        Err(e) => {
            failures.push(format!("cannot read {APPS}: {e}"));
            Vec::new()
        }
    };
    report(
        failures,
        &format!(
            "{checked} workflow files use only pinned actions of GitHub's and agree on their tools and shared scripts; {APPS} lists {} app(s)",
            listed.len()
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "3d3c42e5aac5ba805825da76410c181273ba90b1";

    #[test]
    fn only_pinned_actions_of_githubs_own() {
        let good = format!(
            "jobs:\n  a:\n    uses: ./.github/workflows/nightly.yml\n  b:\n    steps:\n      - uses: actions/checkout@{SHA} # v7.0.1\n      \
             - name: Cache\n        uses: \"actions/cache/restore@{SHA}\"\n      # - uses: someone/else@v1\n  c:\n    \
             uses: jamescjfoundation/open-source-core/.github/workflows/shared-ci.yml@workflows-v1\n"
        );
        assert_eq!(uses_problems("x.yml", &good), Vec::<String>::new());
        let bad = format!(
            "steps:\n  - uses: actions/checkout@v7\n  - uses: someone/tool@{SHA}\n  - uses: actions/cache\n  \
             - uses: jamescjfoundation/open-source-core/.github/workflows/shared-ci.yml@main\n  - uses: actions/checkout@{}\n",
            &SHA[..39]
        );
        let found = uses_problems("x.yml", &bad);
        assert_eq!(found.len(), 5, "{found:?}");
        assert!(found[0].starts_with("x.yml:2:") && found[0].contains("not pinned to a full commit"));
        assert!(found[1].contains("`someone/tool` is not one of GitHub's own actions"));
        assert!(found[2].contains("names no commit"));
        assert!(found[3].contains("`workflows-vN` tag"));
        assert!(found[4].contains("not pinned to a full commit"));
    }

    #[test]
    fn two_workflows_agree_on_a_tool() {
        let sum = "d039b43c3e0958441b6ad0a39c8884d018b3d2e8fb9928f209074b1100b5e17a";
        let one = format!(
            "name: One\n\nenv:\n  CARGO_TERM_COLOR: always\n  TOOL_VERSION: \"1.2.3\"\n  TOOL_LINUX_SHA256: \"{sum}\"\n\njobs:\n  a:\n    env:\n      OTHER_VERSION: \"9\"\n"
        );
        let pins = tool_pins(&one);
        assert_eq!(pins.len(), 2, "{pins:?}");
        assert_eq!(pins.get("TOOL_VERSION").map(String::as_str), Some("1.2.3"));
        let same = ("two.yml".to_owned(), pins.clone());
        assert_eq!(pin_problems(&[("one.yml".to_owned(), pins.clone()), same]), Vec::<String>::new());
        let other = tool_pins(&one.replace("1.2.3", "1.2.4").replace(sum, "D039"));
        let found = pin_problems(&[("one.yml".to_owned(), pins), ("two.yml".to_owned(), other)]);
        assert_eq!(found.len(), 3, "{found:?}");
        assert!(found.iter().any(|p| p.contains("two.yml: TOOL_LINUX_SHA256 is not a SHA-256 checksum")));
        assert!(found.iter().any(|p| p.contains("TOOL_VERSION is `1.2.3` in one.yml and `1.2.4` in two.yml")));
        assert!(tool_pins("name: None\njobs:\n  a:\n    runs-on: x\n").is_empty());
    }

    #[test]
    fn a_shared_script_is_the_same_text_in_every_copy() {
        let one = "jobs:\n  a:\n    steps:\n      - run: |\n          # BEGIN same: name it\n          key=$(hash)\n          {\n            echo \"key=$key\"\n          } >> out\n          # END same: name it\n          echo done\n";
        // The same script at another depth, in another file.
        let two = "steps:\n  - run: |\n      # BEGIN same: name it\n      key=$(hash)\n      {\n        echo \"key=$key\"\n      } >> out\n      # END same: name it\n";
        let blocks = same_blocks(one);
        assert_eq!(blocks, [("name it".to_owned(), "key=$(hash)\n{\n  echo \"key=$key\"\n} >> out".to_owned())]);
        assert_eq!(same_blocks(two), blocks);
        let files = |texts: &[&str]| texts.iter().enumerate().map(|(i, t)| (format!("{i}.yml"), (*t).to_owned())).collect::<Vec<_>>();
        assert_eq!(same_problems(&files(&[one, two, one])), Vec::<String>::new());
        let other = two.replace("key=$(hash)", "key=$(hash --short)");
        let found = same_problems(&files(&[one, &other]));
        assert!(found.len() == 1 && found[0].contains("`same: name it` block of 1.yml differs from the one in 0.yml"), "{found:?}");
        // A block that is never closed, or closed under another name, is said so.
        let open = "run: |\n  # BEGIN same: name it\n  key=1\n  # END same: another\n";
        assert!(same_blocks(open).is_empty());
        assert!(same_problems(&files(&[open]))[0].contains("has no `# END same:` line"));
        assert!(same_blocks("jobs: {}\n").is_empty());
    }

    #[test]
    fn reads_the_list_of_apps() {
        assert_eq!(
            apps(r#"{"about": "which apps", "apps": ["example/app-one", "example/app.two"]}"#),
            Ok(vec!["example/app-one".to_owned(), "example/app.two".to_owned()])
        );
        assert_eq!(apps(r#"{"apps": []}"#), Ok(vec![]));
        for bad in [
            "",
            "[]",
            r#"{"list": []}"#,
            r#"{"apps": ["app"]}"#,
            r#"{"apps": ["owner/app/more"]}"#,
            r#"{"apps": ["owner/"]}"#,
            r#"{"apps": ["owner/app", "Owner/App"]}"#,
            r#"{"apps": [{"repository": "owner/app"}]}"#,
            r#"{"apps": ["owner/app; rm -rf ."]}"#,
            r#"{"apps": ["../owner/app"]}"#,
        ] {
            assert!(apps(bad).is_err(), "{bad}");
        }
    }
}
