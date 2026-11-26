# A foundation app

Started from open-source-core's `template/`. To make it yours:

1. Copy this folder into a new repository. Take it from the core release the app will pin, so the
   files agree with that release from the start.
2. Rename the package `app` (in `Cargo.toml` and `apps/app`), set `APP` (the app's identifier)
   and `NAME` in `apps/app/src/main.rs`, and `product` and `shipped` in `xtask/src/main.rs`.
   That file is the app's own and holds only its data and its own tasks; the gate is the core's
   (`xtask/src/gate.rs`).
3. Pin the core: every `jcj-` dependency in `Cargo.toml` at the same release tag. Add a line for
   each other `jcj-` crate the app uses.
4. Run `cargo xtask sync`. This folder already holds the shared xtask modules, the CI and
   core-pin workflows, the Dependabot settings, the pull request template, the hooks, the code
   of conduct and the files with the core's marked blocks (`AGENTS.md`, `SECURITY.md`,
   `CONTRIBUTING.md`, `Cargo.toml`, `.cargo/config.toml`, `.gitignore`, `.gitattributes`). The
   first run adds the four settings files kept at the core's root (`rust-toolchain.toml`,
   `rustfmt.toml`, `clippy.toml`, `deny.toml`) and rewrites the rest and the patch block in
   `Cargo.toml` from the pinned release. Cargo writes `Cargo.lock` on that run;
   commit it. Then `cargo xtask install-hooks` and `cargo xtask ci`. The gate says so each time
   it runs in a clone whose hooks are off.
5. Write the app's own parts of `AGENTS.md`, `CONTRIBUTING.md` and `SECURITY.md`: the opening
   lines and every section outside the blocks marked `open-source-core`, where the template has
   placeholder text. One of them is checked: the layer table in `AGENTS.md`, which
   `cargo xtask app-layers` reads. Give every new package a row in it. The blocks themselves are the core's: `cargo xtask sync` rewrites them and
   `cargo xtask sync --check`, a task of the gate, fails when one differs. To change one, change
   it in the core.
6. If the app belongs to a project with several repositories, the project's map of them is the
   project's own business. Put its block above the core's blocks in `AGENTS.md`; the core
   neither writes nor checks it.
7. Add what the template does not hold: this file rewritten as the app's `README.md`,
   `LICENSE-MIT` and `LICENSE-APACHE` (`cargo xtask third-party-licenses` reads them), `NOTICE`
   (keep the credit "based on WordCraft by the ArtCraft team"), `ATTRIBUTION.md` (every file
   under `assets/` is named in it, in backticks; `cargo xtask assets`, a task of the gate, fails
   until these four files exist), `CHANGELOG.md`, a `docs/decisions/` folder, and the product's
   `tokens.json` (the core's `crates/jcj-theme/TOKENS.md` describes it).
8. Code that should one day move to the core is marked `// core-candidate: open-source-core#N`
   with the core issue that tracks it; `cargo xtask docs`, a task of the gate, fails on a marker
   without an issue, on a relative Markdown link that leads nowhere and on a decision reference
   that names no record.
9. In the repository's settings on GitHub, turn on private vulnerability reporting (the
   `security` block and the code of conduct send reports there) and keep issues on (the
   core-pin workflow opens one when the pin falls behind). `ci.yml` and `core-pin.yml` call
   workflows kept in `jamescjfoundation/open-source-core`: an organisation that limits which
   actions and workflows may run must allow that repository's (its own are always allowed).

Moving to a newer core release later: change the tag on every `jcj-` line, run
`cargo metadata --format-version 1 >/dev/null` to bring `Cargo.lock` in line, run
`cargo xtask sync`, and follow "What apps must do" for each release passed, in the core's
`CHANGELOG.md`.

Working on a core change at the same time: `cargo xtask dev-link ../open-source-core`. The link
is a folder Git ignores and changes no tracked file; `cargo xtask ci --linked` runs the gate
against it, and `cargo xtask dev-unlink` removes it. `AGENTS.md`, "Core change an app needs",
has the steps, and how a pull request is checked against a core change before its release.
