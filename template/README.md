# A foundation app

Started from open-source-core's `template/`. To make it yours:

1. Copy this folder into a new repository. Take it from the core release the app will pin, so the
   files agree with that release from the start.
2. Rename the package `app` (in `Cargo.toml` and `apps/app`), set `APP` (the app's identifier)
   and `NAME` in `apps/app/src/main.rs`, and `SHIPPED` and `PRODUCT` in `xtask/src/main.rs`.
3. Pin the core: every `jcj-` dependency in `Cargo.toml` at the same release tag. Add a line for
   each other `jcj-` crate the app uses.
4. Run `cargo xtask sync`. This folder already holds the shared xtask modules, the CI and
   core-pin workflows, the hooks, the code of conduct and the three files with the core's marked
   blocks. The first run adds the four settings files kept at the core's root
   (`rust-toolchain.toml`, `rustfmt.toml`, `clippy.toml`, `deny.toml`) and rewrites the rest and
   the patch block in `Cargo.toml` from the pinned release. Cargo writes `Cargo.lock` on that run;
   commit it. Then `cargo xtask install-hooks` and `cargo xtask ci`.
5. Write the app's own parts of `AGENTS.md`, `CONTRIBUTING.md` and `SECURITY.md`: the opening
   lines and every section outside the blocks marked `open-source-core`, where the template has
   placeholder text. The blocks themselves are the core's: `cargo xtask sync` rewrites them and
   `cargo xtask sync --check`, a task of the gate, fails when one differs. To change one, change
   it in the core.
6. If the app belongs to a project with several repositories, the project's map of them is the
   project's own business. Put its block above the core's blocks in `AGENTS.md`; the core
   neither writes nor checks it.
7. Add what the template does not hold: this file rewritten as the app's `README.md`,
   `LICENSE-MIT` and `LICENSE-APACHE` (`cargo xtask third-party-licenses` reads them), `NOTICE`
   (keep the credit "based on WordCraft by the ArtCraft team"), `CHANGELOG.md`, a
   `docs/decisions/` folder, and the product's `tokens.json` (the core's
   `crates/jcj-theme/TOKENS.md` describes it).
8. In the repository's settings on GitHub, turn on private vulnerability reporting (the
   `security` block and the code of conduct send reports there) and keep issues on (the
   core-pin workflow opens one when the pin falls behind).

Moving to a newer core release later: change the tag on every `jcj-` line, run
`cargo metadata --format-version 1 >/dev/null` to bring `Cargo.lock` in line, run
`cargo xtask sync` until `cargo xtask sync --check` passes, and follow "What apps must do" for
each release passed, in the core's `CHANGELOG.md`.

Working on a core change at the same time: `cargo xtask dev-link ../open-source-core`, and
`cargo xtask dev-unlink` before committing.
