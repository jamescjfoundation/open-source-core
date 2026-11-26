# 0013: Building between an app and the core

Date: 2026-11-24 · Status: accepted · Amended 2026-11-26 (how changes land; what "no product
name" covers) · From the plan's "Building between an app and the core"

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
`core-testbed`; the app's use in the app; the core change lands first; release the core; move the
app's pin; then the app's change lands, naming the core issue it needed.

Prototype first, promote later: mark it `// core-candidate:` in the app with a core issue labelled
`core-candidate`. Code is never copied between app repositories.

How a change lands, while no app built on the core has a public release:

- A direct push to `main` is allowed, for people and agents. The gate (`cargo xtask ci`) is run
  before the push.
- A pull request is optional. Its author may merge it once its checks pass.
- A change that spans two repositories lands provider first: the core, its release, then the
  app's pin and the app's change. One app that another depends on is a provider in the same way
  (decision 0015).

What "the core carries no product name" covers: code, identifiers, interface text, brand (names,
colors, logos, fonts) and the instructions an app carries from the core (`template/`). Records of
history may say which app a thing came from: `CHANGELOG.md`, decision records and `docs/history/`.
The core's `README.md` and the opening of its `AGENTS.md` may link the map of a project built on
it, so that someone holding only the core can find who depends on it.

The full rules for agents are in `AGENTS.md`. The part every app carries is the marked blocks
of `template/AGENTS.md`, copied and checked by `cargo xtask sync` (decision 0003).

## Why the 2026-11-26 amendment

- The first text asked for two linked pull requests, and `AGENTS.md` for a person's approval to
  merge. Nothing enforced either and the history shows direct pushes. While nothing is released,
  a red `main` costs a fix and blocks no user, CI runs on every push, and the gate is the same
  command before a push as in a pull request. Writing down what is done lets an agent follow the
  text.
- The product-name rule exists so that any project can build on the core without carrying another
  product's identity. That is a rule about what ships and what an app inherits. A changelog or a
  decision that says where code was promoted from is a true record, and removing it would lose
  the reason the interface has its shape.

## Revisit when

The first app built on the core has a public release: from then a broken `main` can reach users
through an update, and required checks or pull requests on `main` should be decided again.
