# Rebuilding Wordsmith Script on the core (milestone M4)

How `jamescjfoundation/wordsmith-script` moves from its own copy of WordCraft onto open-source-core.
The module-by-module decisions are in [review/script-29ed4bf.md](review/script-29ed4bf.md); this is
the order of work and the exact core interfaces each part lands on. It is written for the people
and agents doing the rebuild.

## Before anything else

1. **The review is done against `29ed4bf`** (this repository's `docs/review/`), so the repository
   may now be reset or rebuilt on top of its history (the plan's decision for Dylan).
2. **Keep today's code readable**: tag `v0.1.2-final`; optionally an archived read-only copy.
3. **Look after 0.1.x users**: they check this repository's GitHub releases for updates (Script
   decision 0038). Keep the repository's address and releases, or publish a last 0.1.x release
   pointing to the new home. Without a reset, 0.1.x fixes go on `release/0.1`.
4. **Golden files are the judge.** Before touching code, run Script's golden suite
   (`crates/wordsmith-format/tests/golden.rs`, the PDF hashes in `docs/golden.md`) and keep its
   outputs: every step below must reproduce them, or record a reviewed, deliberate difference.

## Order of work

Each step builds and passes Script's tests before the next one starts.

### 1. Adopt the template's tooling

Copy from the core's `template/`: `xtask/` (keep Script's own tasks such as `screenshots` and
`bundle` in its `main.rs`, next to the template's), `hooks/`, `.github/workflows/ci.yml` (Script's
release workflow stays its own), and run `cargo xtask sync` to bring the toolchain, lint, deny
settings and patch block. Script's own license allowlist and supply-chain decisions (0001, 0040)
now point to the core's decision 0001.

### 2. Depend on the core

```toml
[workspace.dependencies]
jcj-doc = { git = "https://github.com/jamescjfoundation/open-source-core", tag = "v0.1.0" }
# jcj-layout, jcj-engine, jcj-output, jcj-interchange, jcj-proof, jcj-project, jcj-features,
# jcj-theme, jcj-board, jcj-shell, jcj-agents: all at the same tag
```

Delete the copied crates (`wordsmith-{geom,doc,fonts,layout,render,pdf,engine,control-key,mcp}`)
and replace every `wordsmith_doc::` (and the others) with the `jcj-` interface (table below).
`cargo xtask layers` in the app then fails on any remaining upstream name.

| Script used | Now |
| --- | --- |
| `wordsmith_doc::{Document, Paragraph, Block, Pos, Path, StoryRef, Style, ...}`, `props::*`, `para::*` | `jcj_doc::...` (same names), `jcj_doc::props`, `jcj_doc::para` |
| `wordsmith_layout::{DocLayout, LayoutCache, LayoutOptions, ViewMode, Page, ParaLayout, Placed, FieldCtx, layout, para::*}` | `jcj_layout::...` (same names); `jcj_layout::print_layout`, `page_boxes`, `hit::*` |
| `wordsmith_render::{RenderOptions, render_area}` | `jcj_output::{RenderSettings, render_area, render_page}` |
| `wordsmith_pdf::{PdfOptions, export_layout}` | `jcj_output::{PdfSettings, export_pdf}` |
| `wordsmith_engine::{Session, Registry, CommandSpec, Selection, CmdError, CmdResult}` | `jcj_engine::...` (same names) |
| `wordsmith_engine::cmd::{registry_with, specs, sel_result, pos_json, now_unix, civil_from_days}`, `duplicate_ids` | `jcj_engine::{registry_with, duplicate_ids}`, `jcj_engine::cmd::*` |
| `wordsmith_engine::file_access::*` | `jcj_engine::file_access::*` |
| `wordsmith_control_key::settings_dir` | `jcj_shell::prefs::settings_dir(id, name)` |
| `wordsmith_mcp::{Server, Headless}` | `jcj_agents::{AgentServer, AgentConfig, AgentInfo}` |
| `wordsmith_geom::Rect` | `jcj_layout::Rect` |

The registry: `jcj_engine::registry_with(jcj_engine::commands::merged(app))` where `app` is
`jcj_features::commands()` plus Script's own commands. Script's ids that the engine or the core
now provides under another id are mapped in decision 0007 (for example `caret.lineStart` →
`caret.home`, `spelling.addWord` → `review.addToDictionary`); the command line and MCP keep
working if Script registers aliases for its old ids.

### 3. Screenplay fields into extension data (and `.wss` format version bump)

Declare Script's keys once, in its own namespace, and use the core's keys for what moved to
`jcj-features`:

| Script field | Becomes |
| --- | --- |
| `ParaProps::scene_number` | `const SCENE_NUMBER: ExtKey<String> = ExtKey::new("script.scene_number")` |
| `ParaProps::omitted`, `beat`, `scene_id`, `dual` | `script.omitted`, `script.beat`, `script.scene_id`, `script.dual` |
| `ParaProps::bookmark` | `jcj_features::keys::BOOKMARK` (`core.bookmark`) |
| `CharProps::note` | `jcj_features::keys::NOTE` (`core.note`) |
| `CharProps::revised` | `jcj_features::keys::REVISED` (`core.revised`) |
| `CharProps::page_lock` | `jcj_features::keys::PAGE_LOCK` (`core.page_lock`) |
| `Document::app_data["…"]` | `Document::ext` under the same key (bookmarks, notes, revisions, locks, watermark, header/footer: the core's `core.*` keys) |
| `ParaProps::extra_attrs` | Keep in Script's reader/writer (unknown attributes of a newer `.wss`) |

The `.wss` reader maps the old attributes and text markers (`{n12}`, `{r2}`, `{p7}`) onto these
keys; the writer writes the new version. Add a migration test that opens every golden and fixture
file of 0.1.x, saves it, reopens it and paginates it identically. Whether `.wss` stays Script's text
format or becomes a `jcj-project` container holding a native document is Script's decision; the
text format keeps Fountain-like diffs, the container gives snapshots and assets for free.

### 4. Pagination on `jcj-layout`

Script's `paginate/` (measure, plan, output) was generalized into `jcj_layout::{measure, pages,
paginate}`. Script's (MORE)/(CONT'D), dual dialogue, sentence splits and scene rules become one
`PaginationPass` implementation:

- `may_split` / `split_points`: dialogue splits at sentence ends; action splits; never headings;
- `continuation`: `(MORE)` at the foot and `CHARACTER (CONT'D)` at the head of the next page;
- `prepare` / `finish`: dual dialogue, revision marks in the margin (Script's own).

Wrap it in `jcj_features::locks::LockPass` for locked pages, and call
`jcj_features::pages::set_pages` once at start so the features' commands plan pages the way
Script does. Revised-page colors and headers/footers then come from
`jcj_features::revisions::decorate_output` and `jcj_features::header_footer::apply` (with a
`FieldSource` that supplies `{scene}`). Golden PDFs decide.

### 5. Format code

- Fountain, the import classification rules and screenplay exports stay in Script
  (`wordsmith-format`), on `jcj-doc`.
- RTF, text, HTML, DOCX, ODT, Markdown, EPUB: `jcj_interchange::{import, export, Format}`.
- Emphasis markers: `jcj_interchange::emphasis` for `**`, `*`, `_`; Script keeps its note,
  revision and lock markers on top.
- Saving, backups, recovery: `jcj_project::files` (atomic writes, `backup`, `save_with_backup`,
  `write_recovery`, `recovery`), and `jcj_project::AutoSave` for the timing.

### 6. Spelling

`jcj_engine::commands::spelling` and `jcj_proof`. Script's skip rules (Character paragraphs,
words in capitals, speakers' names) become its own `spelling.check` using
`misspellings_with(doc, opts, skip, correct)`.

### 7. The interface on `jcj-shell` (egui 0.37)

- `wordsmith-ui`'s general half is now the core: implement `jcj_shell::App` for Script's editor
  state; ribbon tabs (`TabSpec`, `widgets::*`), panels (`PanelSpec`), File pages, About (with the
  credit "Based on WordCraft by the ArtCraft team.").
- Brand: Script's `brand.rs` values become `wordsmith/brand/script/tokens.json` in the shape of
  `jcj-theme`'s `TOKENS.md`, loaded with `Tokens::from_json`; Script's fonts (Courier Prime for the
  page; its interface faces) go in through `Shell::with_fonts`.
- Index cards and the story board: `jcj_board::{Board, Corkboard, Canvas}`; Script keeps beats ↔
  outline paragraphs.
- Updates: `jcj_shell::updates::desktop::Updater` with Script's `ReleaseSource`.
- egui 0.36 → 0.37 changes in Script's remaining UI code (editor, panels, screenplay tabs) are
  done here; WordCraft's `crates/ui-egui` shows the 0.37 idioms.

### 8. Gate and release

All golden files identical (or deliberately changed and recorded), all Script tests passing, every
feature in Script's README working, then Script 0.2.0 on the core. Script's decisions 0005–0008
and 0039 are marked replaced, pointing to the core's 0004, 0006 and 0007.

## When the core is missing something

Follow the core's AGENTS.md: a core issue, `cargo xtask dev-link ../open-source-core`, the general
part in the core with a use in `core-testbed`, then Script's use; core pull request first. Known
gaps today, any of which Script may hit:

- `jcj-layout`'s planner does not plan footnotes, floats or columns and keeps tables whole;
  locks inside a paragraph allow but cannot force a split (needs a byte-level break hook).
- No PDF/X; no EPUB import.
- Title pages stay in Script (screenplay layout); headers and footers are in `jcj-features`.
- Document tabs, split view, statistics and sprints stay in Script until a second app wants them.
