# 0011: Core releases and versioning

Date: 2026-11-24 · Status: accepted · Amended 2026-11-26 (who cuts a release; how far behind an
app may be)

## Decision

- **Versions follow craft-libs' policy.** While at 0.x, a patch release (0.1.1) only adds or
  fixes and never breaks an app; a minor release (0.2.0) may break, and its `CHANGELOG.md` entry
  has a **What apps must do** section saying exactly what each app changes.
- **Apps depend on a release tag** (`tag = "v0.1.0"`) of
  `https://github.com/jamescjfoundation/open-source-core` and carry the core's patch block. Every
  `jcj-` line in an app names the same tag, and an app's dependency graph holds one core release.
- **An app moves to the newest core before new work starts on it and before its own release.**
  Between those it may fall behind. A scheduled check in each app (`cargo xtask core-pin`, run by
  the core-owned workflow `core-pin.yml`) opens an issue in the app when its pin is more than three
  releases behind the newest.
- **A release is cut by a person or an agent once the full gate has passed on the commit**:
  `cargo xtask ci` (full), cargo-deny and the cross-platform run. This holds for patch and minor
  releases alike. No release is cut on a commit whose gate has not passed. Pushing a `v*` tag runs
  the release workflow, which re-runs the gate, attaches the vendored sources and the third-party
  license texts, and publishes the GitHub release with the changelog section.
- **Changes that alter what users see in other apps** sit behind an option (a shell setting or a
  Cargo feature) that defaults to the old behavior until every app has turned it on (plan, "Step
  3").

## Why the 2026-11-26 amendment

- The first rule was "a release is cut only with a person's approval". A core change an app needs
  reaches the app only through a release, so every such change waited on one person, and in
  practice patch releases were cut as soon as the gate passed. What makes a release safe is the
  gate and the rule that a patch only adds or fixes; both are checked the same way whoever cuts it.
- The first rule was "apps stay at most one core release behind". With several patch releases a
  day it was broken within hours and nothing checked it. A rule tied to the moments that matter
  (starting work, releasing) can be kept, and a check that opens an issue makes drift visible
  without turning an app's CI red for a release it does not need yet.

## Revisit when

An app has a public release that users update from (a core release then reaches users, and may
need a slower path), or the core reaches 1.0.
