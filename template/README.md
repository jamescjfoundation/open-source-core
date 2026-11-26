# A foundation app

Started from open-source-core's `template/`. To make it yours:

1. Copy this folder into a new repository.
2. Rename the package `app` (in `Cargo.toml` and `apps/app`), set `APP` and `NAME` in
   `apps/app/src/main.rs`, and `SHIPPED` and `PRODUCT` in `xtask/src/main.rs`.
3. Pin the core: every `jcj-` dependency in `Cargo.toml` at the same release tag.
4. Run `cargo xtask sync` (copies the core's shared settings, xtask modules, the CI and core-pin
   workflows, hooks, code of conduct, patch block and marked blocks from the pinned release),
   then `cargo xtask install-hooks` and `cargo xtask ci`.
5. Write the app's own parts of `AGENTS.md`, `CONTRIBUTING.md` and `SECURITY.md`: everything
   outside the blocks marked `open-source-core`. The blocks themselves are the core's;
   `cargo xtask sync` rewrites them and `cargo xtask sync --check` fails when one differs. If the
   app belongs to a project with several repositories, put the project's own block, with its
   map, above the core's blocks in `AGENTS.md`.
6. Add the product's `tokens.json` (see the core's `crates/jcj-theme/TOKENS.md`), license files,
   NOTICE (keep the credit "based on WordCraft by the ArtCraft team") and a decision record
   folder.

Working on a core change at the same time: `cargo xtask dev-link ../open-source-core`, and
`cargo xtask dev-unlink` before committing.
