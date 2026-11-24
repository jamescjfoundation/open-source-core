# 0013: Building between an app and the core

Date: 2026-11-24 · Status: accepted · From the plan's "Building between an app and the core"

## Decision

Where a change belongs, asked in order (the first "yes" decides):

1. Does it change how we use WordCraft, craft-libs or another upstream project? The core's
   adapters (and our fork first if upstream code must change).
2. Would it look or work the same in any foundation app? The core.
3. Does a second app already need it, or have a written plan to use it? The core.
4. Otherwise the app. When unsure, the app: moving code into the core later is cheap; taking a
   wrong general interface back out is not.

A core change an app needs: an issue in the core describing the general need; both repositories
side by side (`cargo xtask dev-link ../open-source-core` in the app, never committed:
`dev-link --check` fails in CI); the general part in the core with its own tests and a use in
`core-testbed`; the app's use in the app; two linked pull requests, core first; release the core;
move the app's pin.

Prototype first, promote later: mark it `// core-candidate:` in the app with a core issue labelled
`core-candidate`. Code is never copied between app repositories.

The full rules for agents are in `AGENTS.md`.
