# Review of Wordsmith Script at 29ed4bf: what moves to the core

Milestone M1 of the core plan. `jamescjfoundation/wordsmith-script`, version 0.1.2, commit
`29ed4bf` (2026-11-22), reviewed module by module on 2026-11-24 to decide what is general (moves
to open-source-core) and what is about screenplays (stays in Script). This is the review the plan
asks for before any reset of the Script repository; the rebuild (M4) follows
`docs/porting-script.md`.

**How it was measured.** For every module of Script's own crates (`wordsmith-script`,
`wordsmith-format`, `wordsmith-ui`, the apps) we counted lines and lines naming a screenplay
concept (scene, dialogue, character, transition, Fountain, element, (MORE)/(CONT'D) and similar),
then read each module's header and public interface. A module with almost no screenplay terms that
keeps its state in the document's own data was taken as general; a module whose rules are
screenplay rules stays, even when it is small. The copied WordCraft crates (about 83,000 lines) are
not moved: they are replaced by upstream WordCraft through the adapters.

## 1. Module by module

"Term %" is the share of lines naming a screenplay concept.

### Moves to the core

| Script module | Lines | Term % | Home in the core | Notes |
| --- | ---: | ---: | --- | --- |
| ui `autosave` | 95 | 0 | `jcj-shell` (scheduling) | The app supplies the save; `jcj-project` writes |
| ui `snapshot` | 189 | 0 | `jcj-shell` (UI) + `jcj-project` (snapshots) | |
| ui `updates`, app `updates` | 154 + 253 | 0 | `jcj-shell` | craft-update; the app supplies its release identity |
| ui `prefs` | 284 | 0.4 | `jcj-shell` | Generic over each app's own preferences |
| ui `keys` | 185 | 0 | `jcj-shell` | |
| ui `theme`, `brand`, `fonts` | 147 + 132 + 33 | 0 | `jcj-theme` | Brand values become the app's tokens (`tokens.json`) |
| ui `icons` | 1,768 | 0 | `jcj-shell` | General icons only; screenplay icons stay |
| ui `ribbon_fit`, `ribbon/widgets` | 315 + 405 | 0 | `jcj-shell` | |
| ui `ribbon/mod` | 585 | 2.2 | `jcj-shell` | Tabs and groups supplied by the app |
| ui `workspace` | 692 | 0.7 | `jcj-shell` | Panels, docking, saved layout |
| ui `board` | 499 | 0 | `jcj-board` | |
| ui `backstage` (File page) | 451 | 2.4 | `jcj-shell` | App adds its own entries |
| ui `dialogs` | 684 | 5.1 | `jcj-shell` (general dialogs) | Screenplay dialogs stay |
| ui `sidebar`, `chrome` | 942 + 479 | 9.2, 10.4 | `jcj-shell` (rails, panel frame, status bar) | Panel contents stay |
| script `bookmarks` | 253 | 0 | `jcj-features` | Paragraph ext data + document ext list |
| script `notes` | 453 | 2.6 | `jcj-features` | Run ext mark + document ext list |
| script `revisions` | 431 | 0.5 | `jcj-features` (sets, marks) + `jcj-output` (page colors) | The industry color cycle is a default the app may replace |
| script `tracking` | 285 | 0 | `jcj-features` | Over the model's own tracked changes |
| script `locks` | 468 | 0.9 | `jcj-features` (marks, labels) + `jcj-layout` (pagination respects them) | |
| script `watermark` | 204 | 0.5 | `jcj-features` + `jcj-output` | |
| script `header_footer` | 408 | 3.4 | `jcj-features` (bands, fields) | `{scene}` becomes an app-supplied field |
| script `dates`, `data` | 61 + 30 | 0 | `jcj-features` | Settings groups in document ext |
| script `storyboard` (general part) | 673 | 0.6 | `jcj-board` | Beats ↔ outline paragraphs stay in Script |
| script `outline` (lanes) | 322 | 2.5 | `jcj-board` | |
| script `compare` (paragraph matching) | 313 | 14.0 | `jcj-features` | Scene matching stays in Script, on top |
| script `spelling` (engine use) | 200 | 8.5 | `jcj-proof` | Screenplay skipping rules stay |
| script commands: `caret`, `format`, `find`, `spelling` | 198 + 260 + 252 + 105 | ≤ 7 | `jcj-engine` (general commands) | |
| format `native` container, `files` | 367 + 366 | 8.2, 4.4 | `jcj-project` | The `.wss` text contents stay |
| format `emphasis` (markers) | 431 | 2.5 | `jcj-interchange` (text markers) | Script's note/revision markers stay |
| format `export`, `import` plumbing | part of 361 + 438 | — | `jcj-interchange` | Classification rules stay |
| format `import/pdf` | 263 | 3.8 | Stays for now | Screenplay-specific reading of PDF text; promote when a second app imports PDF |
| xtask, CI, cargo-deny, toolchain, clippy, rustfmt, license allowlist | — | — | The core's tooling, synced into every app (`template/`) | |

### Stays in Wordsmith Script

| Script module | Lines | Term % |
| --- | ---: | ---: |
| `element`, `typing`, `autocomplete`, `template`, `templates`, `macros` (writing rules) | 2,346 | 12–40 |
| `paginate` ((MORE)/(CONT'D), dual dialogue, margins) | 1,833 | 1–18 |
| `scene_data`, `scene_heading`, `scene_numbers`, `scenes`, `strip`, `cast`, `checker` | 1,813 | 12–35 |
| `reports`, `reports_more`, `planning_pdf`, `work` | 1,380 | 5–29 |
| `title_page` (screenplay title page layout) | 320 | 0.9 (layout is screenplay convention, decision 0021) |
| Screenplay commands: `element`, `scene`, `character`, `editing`, `text`, `settings`, `autocomplete` | ~2,500 | 13–38 |
| `fountain`, `import` classification, `.wss` contents | ~1,800 | 14–17 |
| ui `editor` (element logic), `production`, `reports`, `structure`, `tables`, `tools`, `outline`, `strip`, `script_icons`, ribbon `tabs` | ~5,500 | 2–14 |
| Brand: Wordsmith tokens, logo, icon | — | — |

About 16,000 of Script's 37,000 lines move, in line with the plan's first estimate (15,000 to
18,000).

**Where this differs from the plan's first pass.** The general document features (bookmarks,
notes, revisions, tracking, locks, watermark, headers and footers, comparison) were to go into
"`jcj-doc` extensions and `jcj-output`". They need commands and pagination, which sit above the
model, so they live in one crate of their own, `jcj-features` (layer 4), using the model's
extension data. The title page stays in Script: its layout is screenplay convention.

## 2. The adapter interface: every upstream item Script uses

Extracted from every `wordsmith_{geom,doc,fonts,layout,render,pdf,engine,control_key,mcp}::` path
in Script's own crates (95 distinct items through `use` paths; the plan's count of about 145
includes items reached through glob imports and method calls). Each is reachable through a `jcj-`
crate:

| Upstream | Items | Core interface |
| --- | --- | --- |
| doc | `Block`, `Document`, `InlineObject`, `Paragraph`, `Path`, `Pos`, `Revision`, `RevisionKind`, `SectionProps`, `StoryRef`, `Style`, `StyleKind`, `StyleSheet`, `CoreProps`, `PartKind`, `para_block`, `props::{Align, CharProps, ParaProps, Rgb, TextColor, Highlight, Underline, VertAlign}`, `para::{Anchor, Float, OBJ, ShapeKind, Wrap}`, `fill::Fill`, `freeform`, `wrap::arrow_outline`, `bidi::is_word_char` | `jcj-doc` (re-exported item by item, decision 0005) |
| layout | `DocLayout`, `FieldCtx`, `LayoutCache`, `LayoutOptions`, `Page`, `ParaLayout`, `Placed`, `ViewMode`, `layout`, `para::{ParaEnv, layout_para}` | `jcj-layout` |
| render | `RenderOptions`, `render_area` | `jcj-output` |
| pdf | `PdfOptions`, `export_layout` | `jcj-output` |
| engine | `CmdError`, `CmdResult`, `CommandSpec`, `Registry`, `Selection`, `Session` (`run`, `with_registry`), `cmd::{registry_with, specs, sel_result, pos_json, now_unix, civil_from_days}`, `duplicate_ids`, `file_access::{AutomationRoots, confine_process}` | `jcj-engine` |
| control_key | `settings_dir` | `jcj-shell` |
| mcp | `Headless`, `Server` | `jcj-agents` |
| geom | `Rect` | `jcj-layout` |

## 3. Patches needed in our WordCraft fork

| Patch | Why | Status |
| --- | --- | --- |
| Typed extension data on paragraphs, runs and documents | Replaces Script's nine fields and `Document::app_data` (decision 0006) | **Carried** on `jcj` (`docs/UPSTREAM.md`) |
| Pagination hook | (MORE)/(CONT'D) could move onto it | Not needed: Script's pass works above layout through `jcj-layout`'s primitives |
| Slimmer engine | Script trimmed WordCraft's engine to session, undo, registry, recovery | Not carried: apps use the full engine and register their own commands (decision 0007) |
| craft-print's sigstore dependency on Windows (Script's `UPSTREAM.md`) | Build size on Windows | Not needed at our craft-libs pin |

## 4. The `.wss` format change

Script's native format stores paragraph and run fields as attributes and text markers. On the core:

- the nine fields move into extension data: on paragraphs `script.scene_number`, `script.omitted`,
  `script.beat`, `script.bookmark` (or the core's `core.bookmark`), `script.scene_id`,
  `script.dual`; on runs `script.note` (or `core.note`), `script.revised` (or `core.revised`),
  `script.page_lock` (or `core.page_lock`);
- `Document::app_data` entries move into the document's `ext` under the same keys;
- the format version is bumped, and reading an older version maps the old attributes onto the
  new keys. A migration test opens every golden and fixture file of 0.1.x and checks it saves,
  reopens and paginates identically (`docs/porting-script.md`, step 5).

## 5. Script's 43 decisions

| Script decision | Becomes |
| --- | --- |
| 0001 License allowlist | Core 0001 |
| 0002 Workspace layers | Core 0002 (core layers); Script keeps its own app layers |
| 0003 Toolchain and CI | Core 0003 |
| 0004 craft-libs pins | Core 0004 |
| 0005 Copying WordCraft | Replaced by core 0004 (no copies) |
| 0006 Engine trim | Replaced by core 0007 (full engine, own commands) |
| 0007 MCP trim | Replaced by core 0007 |
| 0008 App data | Replaced by core 0006 (extension data) |
| 0009 Element model, 0010 Typing rules | Stay in Script |
| 0011 The first editor | General parts (recovery, focus, page view) to core 0014; element editing stays |
| 0012 Pagination | Stays in Script |
| 0013 PDF export and printing | Core 0007 (output); screenplay PDF rules stay |
| 0014 Spelling data | Core 0007 (proofing); screenplay skipping rules stay |
| 0015 Autocomplete | Stays |
| 0016 Views, theme, writing modes | Theme to core 0014; screenplay views stay |
| 0017 Native format | Container to core 0009; `.wss` contents stay |
| 0018 Fountain, 0019 Import rules | Stay |
| 0020 Saving, auto-save, backups, preferences | Core 0009 (files, auto-save) and 0014 (preferences) |
| 0021 Title page and headers | Headers and footers to `jcj-features`; title page stays |
| 0022 Scenes and tables | Index cards to core 0014 (`jcj-board`); scenes stay |
| 0023 Notes, 0024 Revisions, 0026 Page locking | `jcj-features` (record in core 0010); screenplay defaults stay |
| 0025 Scene numbers, 0027 Reports and revised pages | Stay |
| 0028 Outlining | Lanes and board to `jcj-board`; outline elements stay |
| 0029 CLI, MCP and release | MCP registration to core 0007; CLI stays |
| 0030 SPEC-2, 0031 Editing additions | Stay (app scope) |
| 0032 Macros and styles | Stays |
| 0033 Characters and reports, 0034 Planning | Stay (boards: `jcj-board`) |
| 0035 Production additions | Watermark, locks, tracking, comparison to `jcj-features`; dual dialogue stays |
| 0036 Workspace | Core 0014 (keys, preferences); tabs, split, statistics and sprints stay until a second app wants them |
| 0037 Ribbon and panels | Core 0014 |
| 0038 Releases and updates | Core 0014 (update window) and 0011 (core releases); each app keeps its release identity |
| 0039 A fork of WordCraft | **Replaced** by core 0004 |
| 0040 Supply chain | Core 0001 and 0003 |
| 0041 Our names for panel and view | Stays (names are the app's) |
| 0042 Scope and asking first | Stays; core 0013 covers the core |
| 0043 Key tips and commands | Core 0014 (every control runs a command; key tips) |
