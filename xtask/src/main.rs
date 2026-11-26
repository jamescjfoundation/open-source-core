// SPDX-License-Identifier: MIT OR Apache-2.0
//! `cargo xtask <task>`: open-source-core's build, check and maintenance tasks.
//!
//! `cargo xtask ci` is the gate: the same command runs on a laptop, in the pre-push hook and in
//! GitHub Actions, so a change that passes locally passes in CI. `cargo xtask ci --changed` runs
//! clippy and tests only for the packages a change can affect (and their dependents), which is
//! what the pre-push hook runs; CI runs the full gate on every push to main.
//!
//! The modules under `template/xtask/src/` are shared with every app repository's xtask (copied by
//! `cargo xtask sync <app>`); they are compiled into this xtask from there, so the core tests them.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

// The shared modules carry items only an app xtask uses (dev-link, app-side sync).
#[path = "../../template/xtask/src/changed.rs"]
#[allow(dead_code)]
mod changed;
#[path = "../../template/xtask/src/corepin.rs"]
#[allow(dead_code)]
mod corepin;
#[path = "../../template/xtask/src/json.rs"]
#[allow(dead_code)]
mod json;
#[path = "../../template/xtask/src/licenses.rs"]
#[allow(dead_code)]
mod licenses;
#[path = "../../template/xtask/src/link.rs"]
#[allow(dead_code)]
mod link;
#[path = "../../template/xtask/src/lintroots.rs"]
#[allow(dead_code)]
mod lintroots;
#[path = "../../template/xtask/src/privacy.rs"]
#[allow(dead_code)]
mod privacy;
#[path = "../../template/xtask/src/spdx.rs"]
#[allow(dead_code)]
mod spdx;
#[path = "../../template/xtask/src/sync.rs"]
#[allow(dead_code)]
mod sync;
#[path = "../../template/xtask/src/util.rs"]
#[allow(dead_code)]
mod util;

mod layers;
mod pins;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use changed::Affected;
use util::{TaskResult, cargo, cargo_output, report, workspace_members};

/// The gate, in order. Cheap checks first so a mistake fails fast.
const TASKS: &[(&str, &str)] = &[
    ("fmt", "rustfmt in check mode"),
    ("lint-roots", "every crate root forbids unsafe and denies the panicking lints"),
    ("layers", "only jcj-upstream names upstream; dependencies point down"),
    ("pins", "upstream pins, Cargo.lock and docs/UPSTREAM.md agree"),
    ("template", "the app template carries the core's patch block and shared files"),
    ("privacy", "no personal details or secrets in files or commit identities"),
    ("licenses", "every dependency's license is on the allowlist; legal files exist"),
    ("clippy", "clippy on all targets and features, warnings are errors"),
    ("test", "all tests"),
];

/// The programs the core ships (for the third-party license file of a release).
const SHIPPED: &[&str] = &["core-testbed"];

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
        "affected" => affected_task(&root, rest),
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
        "vendor" => vendor(&root, rest.first().map(String::as_str)),
        "install-hooks" => install_hooks(&root),
        "template-check" => template_check(&root),
        "upstream-status" => pins::status(&root),
        "third-party-licenses" => licenses::task(&root, SHIPPED, "open-source-core's test app", rest.first().map(String::as_str)),
        "privacy" => privacy::task(&root, rest),
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
    println!("  ci [--changed] [--base <git ref>] [--skip <task>]");
    println!("                 the gate: every task below in order; --changed limits clippy and tests to");
    println!("                 the packages changed since --base (default: origin/main) and their dependents");
    for (name, about) in TASKS {
        println!("  {name:<14} {about}");
    }
    println!("\n  affected [--base <ref>]   print the packages a change affects");
    println!("  deny                      cargo-deny: licenses, advisories, sources");
    println!("  sync <app dir> [--check]  copy (or compare) shared settings and the patch block into an app");
    println!("  core-pin <app dir> [--behind] [--max-behind <n>]");
    println!("                            an app names one core release, and Cargo.lock holds one core; --behind");
    println!("                            says how many releases behind the pin is (asks the network)");
    println!("  vendor [out file]         archive the vendored sources of every dependency (release asset)");
    println!("  upstream-status           the pinned WordCraft release and the newest upstream (the train's trigger)");
    println!("  template-check            build the app template against this checkout, and run its own checks");
    println!("  install-hooks             use the repository's git hooks (pre-commit: fmt and privacy; pre-push: ci --changed)");
    println!("  third-party-licenses [file]  write the dependency license texts");
}

fn workspace_root() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().map_or_else(|| manifest.to_path_buf(), Path::to_path_buf)
}

/// `--base <ref>`, or `origin/main` when it exists, else `HEAD`.
fn base_ref(root: &Path, args: &[String]) -> String {
    if let Some(b) = args.iter().position(|a| a == "--base").and_then(|i| args.get(i + 1)) {
        return b.clone();
    }
    match util::git_output(root, &["rev-parse", "--verify", "--quiet", "origin/main"]) {
        Ok(_) => "origin/main".to_owned(),
        Err(_) => "HEAD".to_owned(),
    }
}

fn ci(root: &Path, args: &[String]) -> TaskResult {
    let names: Vec<&str> = TASKS.iter().map(|(n, _)| *n).collect();
    // `--changed` and `--base <ref>` are ours; the rest are `--skip` arguments.
    let mut skip_args = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--changed" => {}
            "--base" => {
                it.next();
            }
            _ => skip_args.push(a.clone()),
        }
    }
    let skip = util::skipped(&skip_args, &names)?;
    let scope = if args.iter().any(|a| a == "--changed") {
        let base = base_ref(root, args);
        let a = changed::affected(root, &base)?;
        match &a {
            Affected::All => println!("changes since {base} affect every package"),
            Affected::Some(p) if p.is_empty() => println!("changes since {base} affect no package (documentation only)"),
            Affected::Some(p) => println!("changes since {base} affect: {}", p.join(", ")),
        }
        a
    } else {
        Affected::All
    };
    let started = std::time::Instant::now();
    for (name, _) in TASKS {
        if skip.iter().any(|s| s == name) {
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
        "privacy" => privacy::task(root, &[]),
        "licenses" => check_licenses(root),
        "clippy" => scoped(root, scope, &["clippy", "--locked", "--all-targets", "--all-features"], &["--", "-D", "warnings"]),
        "test" => test(root, scope),
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

/// Run a cargo command over the affected packages, or not at all when none are.
fn scoped(root: &Path, scope: &Affected, head: &[&str], tail: &[&str]) -> TaskResult {
    if scope.is_empty() {
        println!("nothing to do: no package affected");
        return Ok(());
    }
    let pkgs = scope.cargo_args();
    let mut args: Vec<&str> = head.to_vec();
    args.extend(pkgs.iter().map(String::as_str));
    args.extend(tail);
    cargo(root, &args)
}

fn has_nextest(root: &Path) -> bool {
    cargo_output(root, &["nextest", "--version"]).is_ok()
}

/// Tests: cargo-nextest when installed (faster: one process per test, all cores), plus doc tests.
fn test(root: &Path, scope: &Affected) -> TaskResult {
    if has_nextest(root) {
        scoped(root, scope, &["nextest", "run", "--locked", "--all-features", "--no-tests=pass"], &[])?;
        // Doc tests exist only in libraries; `--doc -p <bin-only package>` is an error.
        let docs = match scope {
            Affected::All => Affected::All,
            Affected::Some(names) => {
                let graph = changed::workspace_graph(root)?;
                Affected::Some(
                    names
                        .iter()
                        .filter(|n| graph.get(n.as_str()).is_some_and(|(dir, _)| root.join(dir).join("src/lib.rs").is_file()))
                        .cloned()
                        .collect(),
                )
            }
        };
        scoped(root, &docs, &["test", "--locked", "--all-features", "--doc"], &[])
    } else {
        scoped(root, scope, &["test", "--locked", "--all-features"], &[])
    }
}

fn affected_task(root: &Path, args: &[String]) -> TaskResult {
    let base = base_ref(root, args);
    match changed::affected(root, &base)? {
        Affected::All => println!("all"),
        Affected::Some(p) => println!("{}", p.join(" ")),
    }
    Ok(())
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
    report(failures, "the app template is in step with the core")
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

/// `cargo xtask template-check`: a copy of `template/` in `target/template-check`, synced from this
/// checkout and depending on it by path instead of by release tag, must build and pass its own
/// lint-roots, core-pin, layers and sync checks. The scheduled CI run does this, so the template
/// never rots between releases.
fn template_check(root: &Path) -> TaskResult {
    let dir = root.join("target").join("template-check");
    let _ = std::fs::remove_dir_all(&dir);
    copy_dir(&root.join("template"), &dir, 0)?;
    sync::write(root, &dir)?;
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
    let target = root.join("target").to_string_lossy().into_owned();
    let run = |args: &[&str]| -> TaskResult {
        let status = Command::new(util::cargo_bin())
            .args(args)
            .current_dir(&dir)
            .env("CARGO_TARGET_DIR", &target)
            .status()
            .map_err(|e| format!("could not run cargo: {e}"))?;
        if status.success() { Ok(()) } else { Err(format!("template: `cargo {}` exited with {status}", args.join(" "))) }
    };
    run(&["fmt", "--all", "--", "--check"])?;
    run(&["clippy", "--workspace", "--all-targets", "--", "-D", "warnings"])?;
    run(&["run", "--quiet", "-p", "xtask", "--", "lint-roots"])?;
    run(&["run", "--quiet", "-p", "xtask", "--", "core-pin"])?;
    run(&["run", "--quiet", "-p", "xtask", "--", "layers"])?;
    run(&["run", "--quiet", "-p", "xtask", "--", "sync", "--check"])?;
    run(&["test", "--workspace"])?;
    println!("ok: the app template builds against this checkout and passes its own checks");
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
    println!("git hooks on: pre-commit runs fmt and the privacy check on staged lines; pre-push runs `cargo xtask ci --changed`");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_ci_task_has_a_runner() {
        let names: Vec<&str> = TASKS.iter().map(|(n, _)| *n).collect();
        assert!(!names.contains(&"ci"));
        let unique: BTreeSet<&str> = names.iter().copied().collect();
        assert_eq!(unique.len(), names.len());
    }
}
