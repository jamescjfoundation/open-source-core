# 0004: Upstream pins, forks, and the follow / hold / own rule

Date: 2026-11-24 · Status: **proposed** (the plan's "Decisions for Dylan" item 3 awaits a
maintainer's approval; until then the rule is applied as written) · Replaces Wordsmith Script
decision 0039 (a fork of WordCraft) for every app built on the core

## Decision

- **Pins.** WordCraft at a release tag (v0.18.1); each craft-libs crate at the commit that WordCraft
  release pins for it; egui and winit at WordCraft's fork commits. Never "whatever main is".
- **Forks.** We build WordCraft from `jamescjfoundation/wordcraft`'s `jcj` branch: the release tag
  plus our patches, one commit each. The root `Cargo.toml` names storytold's address and tag and
  the patch block redirects it to the fork's commit, so dependencies read as upstream and the build
  is ours. Apps carry the same block (`cargo xtask sync`).
- **The train.** Every two weeks: move to WordCraft's newest release tag, rebuild `jcj`, run the
  contract tests, decide follow / hold / own per source, tag a core release, update the apps
  (`docs/UPSTREAM.md`, "The two-week train"). Urgent fixes may take an extra train.
- **Follow** is the default: the update needs at most about two days of adapter work.
- **Hold** when an update breaks the adapters badly or upstream is mid-refactor: stay on the pin,
  plan the migration with its own decision record. A hold longer than three months forces a
  decision.
- **Own** when upstream removes or rewrites something we depend on in a way that doesn't fit, a
  crate's project is quiet for 60 days, license or brand terms change unacceptably, or a hold has
  lasted three months: copy the crate from our fork at the last good version into the core as a
  `jcj-` crate, keep its notices, port fixes by hand.
- **Patches are offered upstream only when general**, following storytold's contribution rules;
  never on the critical path. A patch declined, or unanswered for two trains, is kept and marked
  "carried, not offered".

## Why

Principle 1: our apps never wait on anyone else. Principle 5: upstream is followed on our
schedule. Script's decision 0039 made a one-off fork that stopped following upstream; this keeps
following by default and owns a crate only deliberately, for every app at once.
