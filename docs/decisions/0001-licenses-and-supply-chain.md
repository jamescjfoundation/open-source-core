# 0001: Licenses and the supply chain

Date: 2026-11-24 · Status: accepted · Ported from Wordsmith Script decisions 0001 and 0040

## Decision

- **Allowlist.** Code we link must be under MIT, Apache-2.0 (also with the LLVM exception),
  BSD-2-Clause, BSD-3-Clause, 0BSD, ISC, OFL-1.1, CC0-1.0, CC-BY-4.0, CC-BY-SA-4.0 or Unlicense.
  Reviewed exceptions, needed by WordCraft and its dependencies: Zlib, Unicode-3.0, IJG, BSL-1.0,
  CDLA-Permissive-2.0. GPL and other copyleft code is never linked or copied; it may be used as a
  separate tool the user installs.
- **Two independent checks.** `cargo xtask licenses` reads every package's SPDX expression from
  `cargo tree` (an expression passes if the licenses one may choose are all allowed);
  `cargo deny check` checks licenses again, plus RustSec advisories, yanked versions, wildcards and
  sources. Both lists change together (`deny.toml`, `template/xtask/src/spdx.rs`).
- **Sources.** crates.io, plus the git repositories in `deny.toml` (`allow-git`): storytold's
  WordCraft, craft-libs, egui and winit, our forks, and the core itself (for apps). A git source is
  named by tag or exact commit, never by branch.
- **Advisories.** Every ignore in `deny.toml` carries its reason and is listed in
  `docs/UPSTREAM.md` under "Known issues". The core's own code never calls `zip`'s `extract` or
  `ZipStreamReader`, which is what lets us carry the zip 3 advisories while WordCraft does.
- **Exact toolchain** in `rust-toolchain.toml` (decision 0003).
- **Vendored sources.** Each release attaches `cargo xtask vendor`'s archive of every dependency's
  source, so a release can be rebuilt if a host or repository disappears.
- **Third-party license texts** for shipped binaries come from `cargo xtask third-party-licenses`
  (shared with every app through the template).
- **Assets** (fonts, images, data) are listed in `ATTRIBUTION.md` with their license.

## Revisit when

A dependency we need is under a license outside the list (a maintainer records the exception
here first), or WordCraft moves to zip 9 (drop the ignores).
