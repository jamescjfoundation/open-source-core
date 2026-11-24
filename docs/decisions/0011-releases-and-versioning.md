# 0011: Core releases and versioning

Date: 2026-11-24 · Status: accepted

## Decision

- **Versions follow craft-libs' policy.** While at 0.x, a patch release (0.1.1) only adds or
  fixes and never breaks an app; a minor release (0.2.0) may break, and its `CHANGELOG.md` entry
  has a **What apps must do** section saying exactly what each app changes.
- **Apps depend on a release tag** (`tag = "v0.1.0"`) of
  `https://github.com/jamescjfoundation/open-source-core`, carry the core's patch block, and stay at
  most one core release behind.
- **A release is cut only with a person's approval**, after `cargo xtask ci` (full), cargo-deny
  and the scheduled cross-platform run pass on the commit. Pushing a `v*` tag runs the release
  workflow, which re-runs the gate, attaches the vendored sources and the third-party license
  texts, and publishes the GitHub release with the changelog section.
- **Changes that alter what users see in other apps** sit behind an option (a shell setting or a
  Cargo feature) that defaults to the old behavior until every app has turned it on (plan, "Step
  3").
