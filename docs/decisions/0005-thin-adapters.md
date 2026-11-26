# 0005: Thin adapters: the jcj- crates are the interface apps use

Date: 2026-11-24 · Status: accepted

## Context

Script names about 145 distinct upstream items (`docs/history/script-29ed4bf.md`, "Adapter
interface"). Apps must never name upstream types (plan principle 2), but wrapping WordCraft's whole
document model in new types would be a rewrite of the model's API, not an adapter.

## Decision

- **Re-export where the type *is* the interface.** `jcj-doc` re-exports the model types item by
  item (`Document`, `Paragraph`, `CharProps`, ...), in modules shaped for the apps. Each re-exported
  name is part of the core's contract. When upstream renames or moves one, the adapter keeps the
  old name working (an alias or a small wrapper) and the apps don't change.
- **Wrap where the core adds meaning or upstream's shape is awkward.** Typed extension keys, page
  layout facts, hit testing, the pagination pass interface, output options, the project container,
  format detection: these are the core's own types and functions.
- **Never re-export a whole upstream crate to apps.** Glob re-exports would make every upstream
  change an app-visible change.
- **Behavior is pinned by tests, not by types.** A wrapper cannot hide a different behavior; the
  contract tests (`contract-tests/`) check what apps rely on, against every upstream candidate.
- **Breaking changes.** Removing or changing a public `jcj-` item is a breaking change: a minor
  release while at 0.x, listed under "What apps must do" in `CHANGELOG.md` (decision 0011).

## Revisit when

WordCraft's model moves to `craft-wordmodel`: `jcj-doc` is then the only crate that changes, and
this record says whether the re-exports become wrappers.
