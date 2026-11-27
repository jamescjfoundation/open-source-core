// SPDX-License-Identifier: MIT OR Apache-2.0
//! `cargo xtask <task>`: open-source-core's build, check and maintenance tasks.
//!
//! `cargo xtask ci` is the gate: the same command runs on a laptop, in the pre-push hook and in
//! GitHub Actions, so a change that passes locally passes in CI. `cargo xtask ci --changed` runs
//! clippy and tests only for the packages a change can affect (and their dependents). The
//! pre-push hook runs that, and so does CI on a push or a pull request; the scheduled run and
//! the run before a release are the full gate on three platforms. `cargo xtask release` makes
//! the release commit; the release workflow makes the tag, once that run has passed.
//!
//! The modules under `template/xtask/src/` are shared with every app repository's xtask (copied by
//! `cargo xtask sync <app>`); they are compiled into this xtask from there, so the core tests them.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

// The modules every app's xtask shares, compiled into this one from where apps copy them.
#[path = "../../template/xtask/src/shared.rs"]
mod shared;

mod layers;
mod pins;
mod release;
mod workflows;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use shared::changed::{self, Affected};
use shared::gate::scoped;
use shared::util::{self, TaskResult, cargo, cargo_output, report, workspace_members};
use shared::{corepin, docs, gate, licenses, link, lintroots, package, privacy, spdx, sync};

/// The gate, in order. Cheap checks first so a mistake fails fast.
const TASKS: &[(&str, &str)] = &[
    ("fmt", "rustfmt in check mode"),
    ("lint-roots", "every crate root forbids unsafe and denies the panicking lints"),
    ("layers", "only jcj-upstream names upstream; dependencies point down"),
    ("pins", "upstream pins, Cargo.lock and docs/UPSTREAM.md agree"),
    ("template", "the app template carries the core's patch block and shared files, and the core its own shared blocks"),
    ("workflows", "workflows use only GitHub's own actions at a full commit and agree on the tools they download; the list of apps reads"),
    ("docs", "links in Markdown lead somewhere, and decision references name a record"),
    ("privacy", "no personal details or secrets in files or commit identities"),
    ("licenses", "every dependency's license is on the allowlist; legal files exist"),
    ("clippy", "clippy on all targets and features, warnings are errors"),
    ("test", "all tests: by cargo-nextest when it is installed, else by `cargo test`"),
];

/// The programs the core ships (for the third-party license file of a release).
const SHIPPED: &[&str] = &["core-testbed"];

/// The marked blocks of `sync::BLOCKS` the core's own files do not carry: it has its own "Before
/// you commit" and house rules for code, and its own Cargo settings and ignore rules.
const NOT_IN_THE_CORE: &[&str] = &["app", "cargo", "ignore"];

/// (file under `template/`, the core's own file): whole managed files the core itself uses too.
const SAME_IN_THE_CORE: &[(&str, &str)] = &[("template/CODE_OF_CONDUCT.md", "CODE_OF_CONDUCT.md")];

/// Legal and record files the house rules require.
const REQUIRED_FILES: &[&str] =
    &["LICENSE-MIT", "LICENSE-APACHE", "NOTICE", "ATTRIBUTION.md", "docs/UPSTREAM.md", "CHANGELOG.md", "AGENTS.md", "CLAUDE.md", "SECURITY.md"];

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let task = args.first().map(String::as_str).unwrap_or("help");
    let rest = args.get(1..).unwrap_or_default();
    let root = workspace_root();
    let result = match task {
        "ci" => ci(&root, rest),
        "affected" => changed::task(&root, rest),
        "deny" => cargo(&root, &["deny", "--locked", "check"]).map_err(|e| format!("{e} (install: cargo install --locked cargo-deny)")),
        "sync" => match rest.iter().find(|a| !a.starts_with("--")) {
            Some(app) => sync::core_task(&root, app, rest.iter().any(|a| a == "--check")),
            None => Err("usage: cargo xtask sync <app dir> [--check]".to_owned()),
        },
        // The core has no core pin of its own; like `sync`, the task here looks at an app.
        "core-pin" => match rest.first() {
            Some(app) if !app.starts_with("--") => corepin::task(Path::new(app), rest.get(1..).unwrap_or_default()),
            _ => Err("usage: cargo xtask core-pin <app dir> [--behind] [--max-behind <n>]".to_owned()),
        },
        // On a folder: an app, or a repository without Cargo, which has no other way to be checked.
        "docs" if rest.first().is_some_and(|a| !a.starts_with("--")) => docs::core_task(&root, rest.first().map(String::as_str)),
        // The hooks are shared with the apps, whose pre-commit rejects a dev-link in a tracked file; the core has none.
        "unlinked" => link::check_unlinked(&root),
        "vendor" => vendor(&root, rest.first().map(String::as_str)),
        "install-hooks" => install_hooks(&root),
        "template" if rest.iter().any(|a| a == "--write") => write_own_copies(&root),
        "template-check" => template_check(&root, rest),
        "upstream-status" => pins::status(&root),
        "third-party-licenses" => licenses::task(&root, SHIPPED, "open-source-core's test app", rest.first().map(String::as_str)),
        "privacy" => privacy::task(&root, rest),
        "prune-target" => changed::prune_task(&root, rest),
        "release" => release::task(&root, rest),
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        name => match TASKS.iter().find(|(n, _)| *n == name) {
            Some((n, _)) => run_task(n, &root, &Affected::All),
            None => {
                print_help();
                Err(format!("unknown task '{name}'"))
            }
        },
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("\nxtask: FAILED: {msg}");
            ExitCode::FAILURE
        }
    }
}

fn print_help() {
    println!("usage: cargo xtask <task>\n");
    println!("  ci [--changed] [--base <git ref>] [--skip <tasks>] [--only <tasks>]");
    println!("                 the gate: every task below in order; --changed limits clippy and tests to");
    println!("                 the packages changed since --base (default: origin/main) and their dependents;");
    println!("                 --skip a,b leaves tasks out, --only a,b runs those alone");
    for (name, about) in TASKS {
        println!("  {name:<14} {about}");
    }
    println!("\n  affected [--base <ref>]   print the packages a change affects (`all`, names, or an empty line); why, on standard error");
    println!("  deny                      cargo-deny: licenses, advisories, sources");
    println!("  template --write          copy the shared blocks and files from template/ into the core's own AGENTS.md,");
    println!("                            SECURITY.md, CONTRIBUTING.md and CODE_OF_CONDUCT.md (after changing one in template/)");
    println!("  sync <app dir> [--check]  copy (or compare) shared files, marked blocks and the patch block into an app");
    println!("  docs <dir>                the docs task on another folder: an app, or a repository without Cargo");
    println!("  core-pin <app dir> [--behind] [--max-behind <n>]");
    println!("                            an app names one core release, and Cargo.lock holds one core; --behind");
    println!("                            says how many releases behind the pin is (asks the network)");
    println!("  vendor [out file]         archive the vendored sources of every dependency (release asset)");
    println!("  upstream-status           the pinned WordCraft release and the newest upstream (the train's trigger)");
    println!("  template-check [--package]");
    println!("                            build the app template against this checkout, and run its own checks; --package makes");
    println!("                            its release package on this system instead, as an app's release workflow does");
    println!(
        "  install-hooks             use the repository's git hooks (pre-commit: fmt, privacy, no dev-link in a tracked file; pre-push: ci --changed)"
    );
    println!("  third-party-licenses [file]  write the dependency license texts");
    println!("  prune-target <older lock file> [--target <folder>]");
    println!("                            remove the build output that a move from that lock file made stale (CI's cache)");
    println!("  release <version> [--date YYYY-MM-DD]");
    println!("                            make the release commit's changes: the version in Cargo.toml and Cargo.lock, the tags");
    println!("                            in template/Cargo.toml, and `## Unreleased` in CHANGELOG.md as that version, dated today");
    println!("  release --check | --notes | --override <owner>/<repository>");
    println!("                            what the release workflow asks: is this a release commit; the release's text; the");
    println!("                            reason the changelog gives for going ahead although that app fails against the core");
}

fn workspace_root() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().map_or_else(|| manifest.to_path_buf(), Path::to_path_buf)
}

fn ci(root: &Path, args: &[String]) -> TaskResult {
    let names: Vec<&str> = TASKS.iter().map(|(n, _)| *n).collect();
    let asked = gate::asked(args, &names, &|| changed::base_ref(root, &[]))?;
    util::warn_if_hooks_off(root, "template/hooks");
    let scope = gate::scope(root, &asked)?;
    let started = std::time::Instant::now();
    for (name, _) in TASKS {
        if asked.skip.iter().any(|s| s == name) {
            println!("\n==> {name} (skipped)");
            continue;
        }
        run_task(name, root, &scope)?;
    }
    println!("\nxtask: all checks passed in {:.0?}", started.elapsed());
    Ok(())
}

fn run_task(name: &str, root: &Path, scope: &Affected) -> TaskResult {
    println!("\n==> {name}");
    let t = std::time::Instant::now();
    let result = match name {
        "fmt" => cargo(root, &["fmt", "--all", "--", "--check"]).and_then(|()| fmt_template(root)),
        "lint-roots" => lintroots::check_lint_roots(root, &[], &[]),
        "layers" => check_layers(root),
        "pins" => pins::task(root),
        "template" => check_template(root),
        "workflows" => workflows::task(root),
        "docs" => docs::core_task(root, None),
        "privacy" => privacy::task(root, &[]),
        "licenses" => check_licenses(root),
        "clippy" => scoped(root, scope, &["clippy", "--locked", "--all-targets", "--all-features"], &["--", "-D", "warnings"]),
        "test" => gate::test(root, scope, &["--all-features"]),
        other => Err(format!("no runner for task '{other}'")),
    };
    println!("    ({name}: {:.1?})", t.elapsed());
    result.map_err(|e| format!("{name}: {e}"))
}

/// rustfmt on the app template's own Rust files (not in this workspace, so `cargo fmt` skips them).
fn fmt_template(root: &Path) -> TaskResult {
    let files = ["template/xtask/src/main.rs", "template/apps/app/src/main.rs"];
    let status = Command::new("rustfmt")
        .current_dir(root)
        .args(["--check", "--edition", "2024", "--config-path", "rustfmt.toml"])
        .args(files)
        .status()
        .map_err(|e| format!("could not run rustfmt: {e}"))?;
    if status.success() { Ok(()) } else { Err("the template's Rust files need `rustfmt --edition 2024` (see the diff above)".to_owned()) }
}

fn check_layers(root: &Path) -> TaskResult {
    let members = workspace_members(root)?;
    let names: BTreeSet<&str> = members.iter().map(|m| m.name.as_str()).collect();
    let mut failures = Vec::new();
    for name in &names {
        if !layers::UNLAYERED.contains(name) && layers::layer_of(name).is_none() {
            failures.push(format!("{name} has no layer in xtask/src/layers.rs"));
        }
    }
    let mut edges = 0usize;
    for member in &members {
        for dep in util::direct_deps(root, &member.name)? {
            edges += 1;
            if let Err(e) = layers::check_edge(&member.name, &dep, names.contains(dep.as_str())) {
                failures.push(e);
            }
        }
    }
    println!("checked {edges} dependency edges of {} crates", members.len());
    report(failures, "layering holds")
}

fn check_template(root: &Path) -> TaskResult {
    let mut failures = Vec::new();
    for (from, _) in sync::MANAGED {
        if !root.join(from).is_file() {
            failures.push(format!("{from} (a file synced into apps) is missing"));
        }
    }
    let read = |p: &str| std::fs::read_to_string(root.join(p)).map_err(|e| format!("cannot read {p}: {e}"));
    let core = link::extract_block(&read("Cargo.toml")?, link::BLOCK_BEGIN, link::BLOCK_END).ok_or("Cargo.toml has no patch block")?;
    match link::extract_block(&read("template/Cargo.toml")?, link::BLOCK_BEGIN, link::BLOCK_END) {
        Some(t) if t == core => {}
        _ => failures.push("template/Cargo.toml: patch block differs from Cargo.toml's (copy it over)".to_owned()),
    }
    // The template's build.rs names the program from `[package.metadata.winresource]`, which
    // winresource reads only with its `toml` feature.
    // The core's own workspace and the testbed are held to the same, should either name it.
    for file in ["template/Cargo.toml", "Cargo.toml", "apps/core-testbed/Cargo.toml"] {
        if !winresource_reads_its_table(&read(file)?) {
            failures.push(format!("{file}: winresource needs `features = [\"toml\"]`, or a build.rs ignores [package.metadata.winresource]"));
        }
    }
    // The rules every app carries are marked blocks in the template's files. The core carries
    // the ones that are about more than an app, word for word.
    for (from, own, name) in sync::BLOCKS {
        let text = read(from)?;
        let marks = sync::marks(from, name);
        let Some(block) = sync::marked_block(&text, &marks) else {
            failures.push(format!("{from}: no `{name}` block (the lines `{}` and `{}`)", marks.begin, marks.end));
            continue;
        };
        if NOT_IN_THE_CORE.contains(name) {
            continue;
        }
        match sync::marked_block(&read(own)?, &sync::marks(own, name)) {
            Some(b) if b == block => {}
            Some(_) => {
                failures.push(format!("{own}: the `{name}` block differs from {from}'s (change it in {from}, then `cargo xtask template --write`)"))
            }
            None => failures.push(format!("{own}: no `{name}` block ({from} shows where it goes)")),
        }
    }
    // The managed file with a part that is the app's: the core's own copy leaves that part empty.
    let deny = read(sync::DENY)?;
    if sync::with_sources(&deny, &[]) != deny || !deny.contains(sync::SOURCES_BEGIN) {
        failures.push(format!(
            "{}: the lines `{}` and `{}` hold an app's own sources, and nothing in the core (it names no product)",
            sync::DENY,
            sync::SOURCES_BEGIN,
            sync::SOURCES_END
        ));
    }
    for (from, own) in SAME_IN_THE_CORE {
        if std::fs::read(root.join(from)).ok() != std::fs::read(root.join(own)).ok() {
            failures.push(format!("{own} differs from {from} (change {from}, then `cargo xtask template --write`)"));
        }
    }
    // An app's workflow files call the shared workflows kept here. Nothing runs that call
    // before an app does, so a file that was renamed, or a tag written two ways, is caught here.
    let mut tags = BTreeSet::new();
    for (from, _) in sync::MANAGED.iter().filter(|(from, _)| from.starts_with("template/.github/workflows/")) {
        let calls = shared_workflow_calls(&read(from)?);
        if calls.is_empty() {
            failures.push(format!("{from} calls no shared workflow of the core"));
        }
        for (file, tag) in calls {
            match read(&format!(".github/workflows/{file}")) {
                Ok(text) if text.lines().any(|l| l.trim() == "workflow_call:") => {}
                Ok(_) => failures.push(format!("{from} calls .github/workflows/{file}, which is not a reusable workflow (no `workflow_call:`)")),
                Err(_) => failures.push(format!("{from} calls .github/workflows/{file}, which does not exist")),
            }
            if !is_workflows_tag(&tag) {
                failures.push(format!("{from} calls {file} at `{tag}`: apps call the shared workflows at a `workflows-vN` tag"));
            }
            tags.insert(tag);
        }
    }
    if tags.len() > 1 {
        failures.push(format!("the template's workflow files name different tags: {}", tags.into_iter().collect::<Vec<_>>().join(", ")));
    }
    report(failures, "the app template is in step with the core")
}

/// Whether the `winresource` dependency of a `Cargo.toml` turns on the `toml` feature, by which
/// it reads `[package.metadata.winresource]`; `true` when there is no such dependency, or when
/// it is the workspace's (which is checked where it is declared).
fn winresource_reads_its_table(cargo_toml: &str) -> bool {
    cargo_toml
        .lines()
        .filter(|l| l.trim_start().starts_with("winresource ") || l.trim_start().starts_with("winresource="))
        .filter(|l| !l.replace(' ', "").contains("workspace=true"))
        .all(|l| l.split_once("features").is_some_and(|(_, f)| f.contains("\"toml\"")))
}

/// The calls to the core's shared workflows in a workflow file: (file under
/// `.github/workflows/`, the ref it is called at).
fn shared_workflow_calls(workflow: &str) -> Vec<(String, String)> {
    let repository = link::CORE_GIT.trim_start_matches("https://github.com/");
    let prefix = format!("uses: {repository}/.github/workflows/");
    let calls = workflow.lines().filter_map(|line| line.trim().strip_prefix(&prefix)?.split_once('@'));
    calls.map(|(file, tag)| (file.to_owned(), tag.split_whitespace().next().unwrap_or_default().to_owned())).collect()
}

/// Whether `tag` is `workflows-v` and a number: the tags the shared workflows are called at.
fn is_workflows_tag(tag: &str) -> bool {
    tag.strip_prefix("workflows-v").is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// `cargo xtask template --write`: after a change under `template/`, bring the core's own copies
/// of the shared blocks and files in line. A block is written only where its marker lines are.
fn write_own_copies(root: &Path) -> TaskResult {
    let read = |p: &str| std::fs::read_to_string(root.join(p)).map_err(|e| format!("cannot read {p}: {e}"));
    let mut written = 0usize;
    for (from, own, name) in sync::BLOCKS.iter().filter(|(_, _, name)| !NOT_IN_THE_CORE.contains(name)) {
        let template = read(from)?;
        let block = sync::marked_block(&template, &sync::marks(from, name)).ok_or_else(|| format!("{from} has no `{name}` block"))?;
        let have = read(own)?;
        let new = sync::replace_marked_block(&have, &sync::marks(own, name), block)
            .ok_or_else(|| format!("{own} has no `{name}` block ({from} shows where it goes)"))?;
        if new != have {
            std::fs::write(root.join(own), new).map_err(|e| format!("{own}: {e}"))?;
            written += 1;
        }
    }
    for (from, own) in SAME_IN_THE_CORE {
        let want = std::fs::read(root.join(from)).map_err(|e| format!("cannot read {from}: {e}"))?;
        if std::fs::read(root.join(own)).ok().as_ref() != Some(&want) {
            std::fs::write(root.join(own), want).map_err(|e| format!("{own}: {e}"))?;
            written += 1;
        }
    }
    println!("ok: {written} of the core's own copies changed");
    Ok(())
}

fn check_licenses(root: &Path) -> TaskResult {
    let mut failures = Vec::new();
    let out = cargo_output(
        root,
        &[
            "tree",
            "--workspace",
            "--locked",
            "--all-features",
            "--target",
            "all",
            "-e",
            "normal,build,dev",
            "--prefix",
            "none",
            "--format",
            "{p}\t{l}",
        ],
    )?;
    let mut checked = BTreeSet::new();
    for line in out.lines().filter(|l| !l.trim().is_empty()) {
        let line = line.trim().trim_end_matches("(*)").trim_end();
        let Some((pkg, license)) = line.split_once('\t') else {
            failures.push(format!("unexpected cargo tree line: {line}"));
            continue;
        };
        if !checked.insert(pkg.to_owned()) {
            continue;
        }
        let license = license.trim();
        if license.is_empty() {
            failures.push(format!("{pkg}: no SPDX license in its manifest"));
            continue;
        }
        match spdx::is_allowed(license) {
            Ok(true) => {}
            Ok(false) => failures.push(format!("{pkg}: license '{license}' is not on the allowlist")),
            Err(e) => failures.push(format!("{pkg}: cannot read license '{license}': {e}")),
        }
    }
    println!("checked {} packages against the license allowlist", checked.len());
    for file in REQUIRED_FILES {
        if !root.join(file).is_file() {
            failures.push(format!("{file} is missing"));
        }
    }
    report(failures, "licenses are on the allowlist and the legal files exist")
}

/// `cargo xtask vendor [out]`: every dependency's source, for rebuilding a release even if a git
/// host or repository disappears (plan, "Nothing can be taken away from us").
///
/// `cargo vendor` cannot hold two commits of the same crate version, and WordCraft pins some
/// craft-libs crates at two commits, so the archive is a fresh Cargo home filled by
/// `cargo fetch --locked` (crates.io archives, the index entries used, and every git source).
/// To rebuild: unpack it, then `CARGO_HOME=<unpacked>/cargo-home cargo build --offline --locked`.
fn vendor(root: &Path, out: Option<&str>) -> TaskResult {
    let dist = root.join("dist");
    let home = dist.join("cargo-home");
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).map_err(|e| format!("{}: {e}", home.display()))?;
    let status = Command::new(util::cargo_bin())
        .current_dir(root)
        .env("CARGO_HOME", &home)
        .args(["fetch", "--locked"])
        .status()
        .map_err(|e| format!("could not run cargo: {e}"))?;
    if !status.success() {
        return Err(format!("cargo fetch exited with {status}"));
    }
    // Extracted sources are rebuilt from the archives; leave them out.
    let _ = std::fs::remove_dir_all(home.join("registry").join("src"));
    std::fs::write(
        home.join("README.txt"),
        "Every dependency of this open-source-core release, as a Cargo home.\n\
         Rebuild without network: CARGO_HOME=<this folder> cargo build --offline --locked\n",
    )
    .map_err(|e| format!("README.txt: {e}"))?;
    // tar runs inside dist/, so a relative path is taken from the workspace root here.
    let out = out.map_or_else(|| dist.join("vendored-sources.tar.gz"), |o| root.join(o));
    let status =
        Command::new("tar").current_dir(&dist).arg("-czf").arg(&out).arg("cargo-home").status().map_err(|e| format!("could not run tar: {e}"))?;
    if !status.success() {
        return Err(format!("tar exited with {status}"));
    }
    println!("wrote {}", out.display());
    Ok(())
}

/// A copy of `template/` in `target/<name>`, synced from this checkout and depending on it by
/// path instead of by release tag: the app a new repository starts as, built against the core as
/// it is here. Returns the folder and the target folder its builds share with the core's.
fn template_copy(root: &Path, name: &str) -> Result<(PathBuf, String), String> {
    let dir = root.join("target").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    copy_dir(&root.join("template"), &dir, 0)?;
    sync::write(root, &dir, Some(&[]))?;
    let manifest_path = dir.join("Cargo.toml");
    let manifest = std::fs::read_to_string(&manifest_path).map_err(|e| format!("template Cargo.toml: {e}"))?;
    let mut out = String::new();
    for line in manifest.lines() {
        match (line.split_once(" = {"), line.contains(link::CORE_GIT)) {
            (Some((name, _)), true) => {
                let path = root.join("crates").join(name.trim()).to_string_lossy().replace('\\', "/");
                out.push_str(&format!("{} = {{ path = \"{path}\" }}\n", name.trim()));
            }
            _ => {
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    std::fs::write(&manifest_path, out).map_err(|e| format!("template Cargo.toml: {e}"))?;
    std::fs::copy(root.join("Cargo.lock"), dir.join("Cargo.lock")).map_err(|e| format!("Cargo.lock: {e}"))?;
    Ok((dir, root.join("target").to_string_lossy().into_owned()))
}

/// Run cargo in a copy of the template, building into the core's own target folder.
fn template_cargo(dir: &Path, target: &str, args: &[&str]) -> TaskResult {
    let status = Command::new(util::cargo_bin())
        .args(args)
        .current_dir(dir)
        .env("CARGO_TARGET_DIR", target)
        .status()
        .map_err(|e| format!("could not run cargo: {e}"))?;
    if status.success() { Ok(()) } else { Err(format!("template: `cargo {}` exited with {status}", args.join(" "))) }
}

/// `cargo xtask template-check`: the copy of the template must build and pass its own
/// lint-roots, core-pin, unlinked, layers, app-layers, sync and docs checks. The scheduled CI
/// run does this, so the template never rots between releases. `--package`: see
/// [`template_package`].
fn template_check(root: &Path, args: &[String]) -> TaskResult {
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        [] => {}
        ["--package"] => return template_package(root),
        _ => return Err("usage: cargo xtask template-check [--package]".to_owned()),
    }
    let (dir, target) = template_copy(root, "template-check")?;
    let run = |args: &[&str]| template_cargo(&dir, &target, args);
    run(&["fmt", "--all", "--", "--check"])?;
    run(&["clippy", "--workspace", "--all-targets", "--", "-D", "warnings"])?;
    run(&["run", "--quiet", "-p", "xtask", "--", "lint-roots"])?;
    run(&["run", "--quiet", "-p", "xtask", "--", "core-pin"])?;
    run(&["run", "--quiet", "-p", "xtask", "--", "unlinked"])?;
    run(&["run", "--quiet", "-p", "xtask", "--", "layers"])?;
    run(&["run", "--quiet", "-p", "xtask", "--", "app-layers"])?;
    run(&["run", "--quiet", "-p", "xtask", "--", "sync", "--check"])?;
    run(&["run", "--quiet", "-p", "xtask", "--", "docs"])?;
    run(&["test", "--workspace"])?;
    println!("ok: the app template builds against this checkout and passes its own checks");
    Ok(())
}

/// `cargo xtask template-check --package`: the release package of the template app, made on
/// this system by the app's own `cargo xtask package`, as an app's release workflow makes it
/// (`docs/app-releases.md`). The copy is given what a new repository adds before its first
/// release: the legal files and a changelog. The scheduled run does this on Linux, macOS and
/// Windows, so the packaging of every system is tried before an app's release needs it.
fn template_package(root: &Path) -> TaskResult {
    let (dir, target) = template_copy(root, "template-package")?;
    for file in package::LEGAL {
        std::fs::copy(root.join(file), dir.join(file)).map_err(|e| format!("{file}: {e}"))?;
    }
    std::fs::write(dir.join("CHANGELOG.md"), "# Changelog\n\n## Unreleased\n\n- The first release.\n").map_err(|e| format!("CHANGELOG.md: {e}"))?;
    let run = |args: &[&str]| template_cargo(&dir, &target, args);
    run(&["run", "--quiet", "-p", "xtask", "--", "release-data"])?;
    run(&["run", "--quiet", "-p", "xtask", "--", "release", "0.9.0", "--date", "2026-01-01"])?;
    run(&["run", "--quiet", "-p", "xtask", "--", "release", "--check"])?;
    run(&["run", "--quiet", "-p", "xtask", "--", "package"])?;
    run(&["run", "--quiet", "-p", "xtask", "--", "package", "--sums", "dist"])?;
    let platform = package::Platform::host().ok_or("a release has no file for this system")?;
    let file = dir.join("dist").join(package::file_name("app", "0.9.0", platform));
    let sums = std::fs::read_to_string(dir.join("dist").join(package::SUMS)).map_err(|e| format!("{}: {e}", package::SUMS))?;
    let expected = format!("{}  {}\n", package::sha256_of(&file)?, package::file_name("app", "0.9.0", platform));
    if sums != expected {
        return Err(format!("{} is not the one line it should be:\n{sums}expected:\n{expected}", package::SUMS));
    }
    println!("ok: the app template packages on this system: {} ({} bytes)", file.display(), std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0));
    Ok(())
}

fn copy_dir(from: &Path, to: &Path, depth: usize) -> TaskResult {
    if depth > 16 {
        return Err(format!("{} is nested too deeply", from.display()));
    }
    std::fs::create_dir_all(to).map_err(|e| format!("{}: {e}", to.display()))?;
    for entry in std::fs::read_dir(from).map_err(|e| format!("{}: {e}", from.display()))?.flatten() {
        let (src, dst) = (entry.path(), to.join(entry.file_name()));
        if src.is_dir() {
            copy_dir(&src, &dst, depth + 1)?;
        } else {
            std::fs::copy(&src, &dst).map_err(|e| format!("{}: {e}", src.display()))?;
        }
    }
    Ok(())
}

fn install_hooks(root: &Path) -> TaskResult {
    util::git_output(root, &["config", "core.hooksPath", "template/hooks"])?;
    println!("git hooks on: pre-commit runs fmt, the privacy check on staged lines and the dev-link check; pre-push runs `cargo xtask ci --changed`");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_release_example_is_what_the_shared_task_writes() {
        // docs/app-release-example/SHA256SUMS.txt is the contract's example (docs/app-releases.md).
        // jcj-shell tests that the updater reads it; this holds the packaging code to it.
        let example = std::fs::read_to_string(workspace_root().join("docs/app-release-example").join(package::SUMS)).unwrap();
        let sums: Vec<(String, String)> =
            example.lines().map(|l| l.split_once("  ").unwrap()).map(|(sum, name)| (name.to_owned(), sum.to_owned())).collect();
        let names: Vec<String> = package::Platform::ALL.iter().map(|p| package::file_name("sample-app", "1.2.3", *p)).collect();
        assert_eq!(sums.iter().map(|(name, _)| name.clone()).collect::<Vec<_>>(), names);
        assert_eq!(package::sums_text(&sums), example);
        assert!(package::set_problems(&names, "sample-app", "1.2.3", true).is_empty());
    }

    #[test]
    fn reads_the_calls_to_shared_workflows() {
        let workflow = "jobs:\n  gate:\n    uses: jamescjfoundation/open-source-core/.github/workflows/shared-ci.yml@workflows-v1\n    \
                        with:\n      full: true\n  own:\n    uses: ./.github/workflows/own.yml\n  step:\n    - uses: actions/checkout@abc # v1\n  \
                        pin:\n    uses: jamescjfoundation/open-source-core/.github/workflows/shared-core-pin.yml@main # moved\n";
        assert_eq!(
            shared_workflow_calls(workflow),
            [("shared-ci.yml".to_owned(), "workflows-v1".to_owned()), ("shared-core-pin.yml".to_owned(), "main".to_owned())]
        );
        assert!(shared_workflow_calls("jobs:\n  build:\n    runs-on: ubuntu-latest\n").is_empty());
        assert!(is_workflows_tag("workflows-v1") && is_workflows_tag("workflows-v12"));
        for other in ["main", "v0.1.19", "workflows-v", "workflows-v1.2", "workflows-vx", ""] {
            assert!(!is_workflows_tag(other), "{other}");
        }
    }

    #[test]
    fn the_template_s_winresource_reads_its_table() {
        assert!(winresource_reads_its_table(&std::fs::read_to_string(workspace_root().join("template/Cargo.toml")).unwrap()));
        assert!(!winresource_reads_its_table("[workspace.dependencies]\nwinresource = { version = \"0.1.34\", default-features = false }\n"));
        assert!(winresource_reads_its_table("[workspace.dependencies]\nwinresource = { version = \"0.1\", features = [\"toml\"] }\n"));
        assert!(winresource_reads_its_table("[workspace.dependencies]\nserde = \"1\"\n"), "no winresource, nothing to check");
        assert!(winresource_reads_its_table("[build-dependencies]\nwinresource = { workspace = true }\n"), "checked in the workspace");
    }

    #[test]
    fn every_ci_task_has_a_runner() {
        let names: Vec<&str> = TASKS.iter().map(|(n, _)| *n).collect();
        assert!(!names.contains(&"ci"));
        let unique: BTreeSet<&str> = names.iter().copied().collect();
        assert_eq!(unique.len(), names.len());
    }
}
