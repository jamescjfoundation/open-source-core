# Upstream: pins, carried patches, follow / hold / own

Every outside project open-source-core builds on, the exact version it uses, every change we carry
on top, and its state under the follow / hold / own rule (decision 0004). `cargo xtask pins` fails
if a git pin in `Cargo.toml` is missing from this file, or if the lock file builds WordCraft from
anywhere but our fork.

Last train: 2026-11-24 (first pin).

## Pins

| Source | Pinned at | How we reach it | State |
| --- | --- | --- | --- |
| WordCraft (`storytold/wordcraft`) | release tag `v0.18.1` (commit `079f478b`) | Named at storytold's address and tag; the patch block points it at our fork's `jcj` branch, commit `0e24d51b8e05bbd0652b2b9904d941b12f6a9849` (tagged `jcj-2026-11-24` in the fork) | Follow |
| craft-libs: `craft-proof` | `01315d9c92b5b0e69082d35ca56099461a1efd45` | storytold's address, the commit WordCraft v0.18.1 pins for it | Follow |
| craft-libs: `craft-pdf`, `krilla` (patched), `craft-update`, `craft-atomic`, `craft-dialogs` | `6e03f65bf45181db77ba6229716e6624db9eb25b` | storytold's address, the commit WordCraft v0.18.1 pins for craft-pdf and craft-update; craft-atomic and craft-dialogs, which WordCraft does not use, take the same commit | Follow |
| craft-libs: `craft-font-store`, `craft-font-catalog` | `762a06a4c6b48b522b7d4de84a87917ef90265e6` | Through WordCraft (not named by us) | Follow |
| craft-libs: `craft-print` | `a3e0b5a9440d18f19422a9dca13516747b7d3181` | storytold's address, the commit WordCraft's app pins | Follow |
| egui 0.37 (`storytold/egui`) | `6d518fc799681571963842f95b55a6b68431b15a` | `[patch.crates-io]`, as WordCraft | Follow |
| winit (`storytold/winit`) | `a517b3846ee8457d84ee4975110bfda6797c44dc` | `[patch.crates-io]`, as WordCraft | Follow |
| winprint (`storytold/winprint.rs`) | `5f80b9baf6b58937c401656634e42d33f00ddf49` | Through craft-print (Windows) | Follow |
| craft-i18n language data | `1e515422afdda2f317af430c1f9d68a5f83935b4` (WordCraft's `craft-i18n.rev`) | Optional at build time (`CRAFT_I18N_DIR`); without it craft-proof falls back to its bundled English list | Follow |

**One craft-libs commit per crate.** WordCraft v0.18.1 pins several craft-libs commits, one per
group of crates. Cargo treats each crate as its own package, so this builds; but two different
commits of the *same* crate would be two types. Never name a craft-libs crate at a commit other
than the one WordCraft uses for it.

**The forks.** `jamescjfoundation/wordcraft` and `jamescjfoundation/craft-libs` hold everything we
build from, so nothing storytold does to its branches can take a pinned commit away from us. Each
core release also stores a vendored copy of every dependency's source (`cargo xtask vendor`,
attached to the GitHub release).

## Carried patches

The fork's `jcj` branch is always "the pinned upstream release, plus our patches on top", one
commit per patch. Older `jcj` branches are kept as tags (`jcj-YYYY-MM-DD`).

| Patch | Fork, topic branch | Files | Why | Upstream status |
| --- | --- | --- | --- | --- |
| Extension data on documents, paragraphs and runs (`Ext`) | `jamescjfoundation/wordcraft`, `topic/extension-data` (commit `0e24d51b`) | `crates/doc/src/ext.rs` (new), `lib.rs`, `document.rs`, `props/char_props.rs`, `props/para_props.rs`, `crates/doc/tests/ext.rs` (new), `crates/docx/tests/it/roundtrip.rs` (field lists) | Every app attaches its own data to the model (decision 0006). Replaces Script's nine screenplay fields and `Document::app_data`. | Not yet offered; general enough to offer (claim an issue first, per their AGENTS.md) |

## Expected patches, not carried yet

| Change | Why | Plan |
| --- | --- | --- |
| A pagination hook (a pass after page breaking that can add continuation lines) | Continuation labels for split tables or quotes; Script's (MORE)/(CONT'D) could move onto it | Not needed yet: `jcj-layout` exposes the layout primitives such a pass uses from above (decision 0005) |
| An engine without the Word command catalog | Smaller builds | Not carried: apps use the full upstream engine and register their own commands (decision 0007) |
| Cargo features to leave out heavy parts (math, charts, SmartArt, metafiles) | Build time and binary size | Optional; offer upstream first |

## Held or owned crates

None.

## Known issues at our pins

| Issue | Effect on us | Tracking |
| --- | --- | --- |
| `zip` 3 has advisories RUSTSEC-2026-0367, -0368, -0369, -0429, -0545, -0548 | All are in `ZipArchive::extract`, `ZipStreamReader` or AES entries. WordCraft reads entries by name into memory; the `aes` feature is off; the core's own zip code never extracts or streams | Ignored in `deny.toml` with reasons; leaves when WordCraft moves to zip 9 |
| `chrono` unmaintained (RUSTSEC-2026-0543), `proc-macro-error` unmaintained (RUSTSEC-2024-0370) | Notices only | Ignored in `deny.toml`; upstream's to replace |
| No EPUB, RTF, DOCX or MCP-server crates in craft-libs at these commits | The plan named `craft-epub`, `craft-rtf`, `craft-docx`, `craft-mcp-server`; at v0.18.1 WordCraft carries its own `wordcraft-docx`, `wordcraft-formats` and `wordcraft-mcp`, and there is no EPUB code | `jcj-interchange` wraps WordCraft's crates and has its own EPUB 3 writer (decision 0008) |

## The two-week train

At each train (decision 0004):

1. Move WordCraft to its newest release tag; take the craft-libs commits that release pins.
2. Rebuild the fork's `jcj` branch: start from the new tag, drop patches upstream merged, re-apply
   the rest in order, tag the old branch `jcj-<date>`.
3. Update the pins in `Cargo.toml` (and `template/Cargo.toml`'s patch block), this file, and
   `WORDCRAFT_RELEASE` / `WORDCRAFT_FORK_COMMIT` in `jcj-upstream`.
4. `cargo xtask ci` and `cargo deny check`; the contract tests decide.
5. Decide follow, hold or own for each source and record it above.
6. Tag a core release; open a pull request in each app moving to it.
