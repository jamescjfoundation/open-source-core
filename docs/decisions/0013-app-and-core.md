# 0013: Building between an app and the core

Date: 2026-11-24 · Status: accepted · Amended 2026-11-26 (how changes land; what "no product
name" covers; when a promotion is finished; the dev-link and the temporary pin) · From the plan's
"Building between an app and the core"

## Decision

Where a change belongs, asked in order (the first "yes" decides):

1. Does it change how we use WordCraft, craft-libs or another upstream project? The core's
   adapters (and our fork first if upstream code must change).
2. Would it look or work the same in any foundation app? The core.
3. Does a second app already need it, or have a written plan to use it? The core.
4. Otherwise the app. When unsure, the app: moving code into the core later is cheap; taking a
   wrong general interface back out is not.

A core change an app needs: an issue in the core describing the general need; both repositories
side by side (`cargo xtask dev-link ../open-source-core` in the app); the general part in the
core with its own tests and a use in `core-testbed`; the app's use in the app; the core change
lands first; release the core; move the app's pin; then the app's change lands, naming the core
issue it needed.

The dev-link (amended 2026-11-26) changes no file Git tracks. It is a folder Git ignores,
`.cargo/dev-link/`, with a Cargo configuration file that the app's tracked `.cargo/config.toml`
includes when it exists, and the lock file of the linked build. The app's own `Cargo.lock` is
not rewritten while linked, so link, build, unlink leaves `git status` clean. The gate refuses
to run linked, because it says what CI will say and CI builds the pinned release;
`cargo xtask ci --linked` runs it against the link on purpose. No tracked file may hold a
`[patch]` to a local path, and the pre-commit hook and the gate check that. The link relies on
two things the Cargo of the pinned toolchain does (decision 0003): reading an included
configuration file, and keeping a lock file at another path. `dev-link` checks both took
effect and keeps no link when they did not.

A temporary pin (amended 2026-11-26) lets CI check the app's half before the core is released.
On a branch of the app, every `jcj-` line names the same whole commit of the core, `rev` in
place of `tag`. `cargo xtask core-pin` passes there, and in the branch's pull request, with a
warning that GitHub shows on the checks page. It fails on `main`, on a tag and wherever the run
cannot be told to be a branch, so `main` and every release build on a core release. Nothing
stops the merge itself: the pull request's author moves the pin to the release's tag first, and
a `main` that received the pin is red at once.

Prototype first, promote later: mark it `// core-candidate: open-source-core#N` in the app, with
that core issue labelled `core-candidate`. Code is never copied between app repositories.

When a promotion is finished (amended 2026-11-26): when the app the code came from uses the
core's copy and its own copy is deleted. The core issue stays open until then, and the marker on
the app's copy names it. A core release that carries the code does not close the issue.

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

## Why the promotion rule, the dev-link in an ignored folder and the temporary pin

- The first promotions were released and their issues closed while the app each came from still
  held its own copy. The code then existed twice and the two copies changed separately, with
  nothing open that said a move was half done: the marker named a closed issue, which the `docs`
  task accepts. One open issue per unfinished move is the smallest thing that keeps the list
  true (issue #30).
- The link used to be written into `.cargo/config.toml`, which every app tracks, with the full
  path of a folder on one machine, and a build while linked rewrote `Cargo.lock`. One
  `git commit -a` published the path; after unlinking, the gate failed until the lock file was
  restored by hand. It also replaced nothing when the checkout's version differed from the
  pinned one, because Cargo keeps what a lock file holds. Cargo reads an included configuration
  file and a lock file at another path, so the link needs no tracked file at all, and the lock
  file of the linked build can start as the app's own, less the linked packages.
- An app change could be checked in CI only after the core change it needed was released, which
  pushed toward one core release per feature and found problems after the release. A commit is
  as exact as a tag, and cargo-deny already accepts one. What a commit is not is a release, so
  it is refused where releases are built from.

## Revisit when

The first app built on the core has a public release: from then a broken `main` can reach users
through an update, and required checks or pull requests on `main` should be decided again.
