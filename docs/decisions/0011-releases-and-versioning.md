# 0011: Core releases and versioning

Date: 2026-11-24 · Status: accepted · Amended 2026-11-26 (who cuts a release; how far behind an
app may be; the gate before the tag, and the apps in the gate; the same order for an app's own
release, decision 0019) · Amended 2026-11-27 (looked at again once apps had public releases;
the release commit lands through a pull request)

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
  releases alike. No release is cut on a commit whose gate has not passed.
- **The gate runs before the tag exists, and the workflow makes the tag** (amended 2026-11-26).
  `cargo xtask release <version>` makes the release commit. The release workflow is started
  for that commit of `main`; it runs the gate, and only when the gate has passed does it make
  the annotated tag, attach the vendored sources and the third-party license texts, and
  publish the GitHub release with the changelog section. Nobody pushes a `v*` tag by hand.
- **The gate of a release** (amended 2026-11-26) is the full gate on Linux, macOS and Windows,
  the minimum Rust version, cargo-deny, the app template, and each app in `.github/apps.json`
  built against the commit with the app's own gate. All three systems pass before every
  release, a patch release included. The gate runs once for a commit: a run of it that has
  already passed on the very commit is used and not repeated.
- **A red app stops a release** (amended 2026-11-26). An app that does not build, or does not
  pass its own gate, against the commit fails the gate; a golden file of the app that differs
  is such a failure. Whoever cuts the release may go ahead by writing the reason in the
  release's changelog section, one line for each such app:
  `**App gate override, <owner>/<repository>:** <the reason>`. The workflow reads that line
  and nothing else lets a red app through.
- **An app's own release follows the same order** (amended 2026-11-26; decision 0019). A
  workflow started by hand for the app's release commit runs the app's gate on the three
  systems and builds every release file; only then does it make the tag and publish. The
  workflow is the core's (`shared-release.yml`), and it refuses an app that is not on the
  newest core unless it is told to go ahead.
- **A release commit lands like any change** (amended 2026-11-27): through a pull request whose
  checks pass (decision 0013), and the release workflow is then started for it on `main`.
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

## Why the gate comes before the tag, and holds the apps (2026-11-26)

- The release workflow used to start when a tag was pushed and ran the gate then. The order
  "gate first" was kept by hand: a run started by hand, a wait, then a tag made by hand. A tag
  is never moved or deleted, because apps pin it, so a tag pushed too early could not be taken
  back, and an app could pin it even when the gate then failed. The only place where "no tag
  on a commit that failed" can be held is the step that makes the tag, so the workflow makes
  it. The same commit was also checked twice, once before the tag and once after.
- "A patch release never breaks an app" rested on review and on an empty template app. The
  break showed when an app moved its pin, after the tag, and the fix was another release.
  Building each app against the commit, with the app's own gate, shows it while the fix is
  still an ordinary commit. An app's golden files count: when the core changes what an app
  writes, either the core is wrong or the app must be told, and the changelog is where apps
  are told. That is why the way past a red app is a line in the changelog and not a switch on
  the workflow: the reason is then in the release's own text.
- Considered and not chosen: macOS and Windows only for minor releases (a patch that breaks one
  system would reach every app before the nightly run saw it); a person's approval before the
  tag (the first rule of this record, dropped for the reasons above); building the apps on
  macOS and Windows too (three times the cost for what an app's own release build checks).

## Why an app's release takes the same order (2026-11-26)

The apps had three release workflows of their own; two started on a pushed tag and one on a
branch, and none had run. What holds for the core's tag holds more for an app's: an installed
copy reads the app's releases and installs what it finds, so a tag or a release that exists
before the gate has passed reaches users, not only other repositories. Decision 0019 has the
contract and the reasons.

## Why no slower path once apps have public releases (2026-11-27)

Apps built on the core published releases that installed copies update from. A core release
does not reach a user by itself: it reaches an app when the app moves its pin, and a user when
that app makes its own release, which runs the app's whole gate on Linux, macOS and Windows
before its tag exists (decision 0019). Before the core's tag exists, the core's gate already
builds each app against the commit with the app's own gate. From this day each app's full gate
also runs on macOS and Windows every night (decision 0003), and every change, a release commit
included, lands through a pull request whose checks pass (decision 0013). A slower path for the
core (a waiting time, a person's approval) would add a wait to every fix an app needs and no
check that the gates above do not already make. The rules of this record stand as written.

## Revisit when

The core reaches 1.0, or a fault reaches users through an app's release that these gates
passed (issue #52 was the last review).
