# 0007: The full engine, general commands, agents, output and proofing

Date: 2026-11-24 · Status: accepted · Replaces Wordsmith Script decisions 0006 (engine trim) and
0007 (MCP trim); takes over the general parts of Script decisions 0013 (PDF export and printing)
and 0014 (spelling data)

## Context

Script cut WordCraft's engine down to the session, undo, the command registry and recovery
(Script 0006), and its MCP server down to a few tools with one document per session (Script
0007). Both cuts were made on copies of WordCraft's code. The core uses WordCraft itself, through
adapters (decisions 0004 and 0005), so a trim would be a patch we carry on every upstream update.
Script also wrote its own caret, formatting, find and spelling commands; most of them do what the
engine's own commands already do.

## Decision

### The full engine, and apps register their own commands

- **Every registry holds the whole upstream engine** (about 420 word-processor commands),
  through `jcj-engine`. An app adds its commands with `jcj_engine::registry_with` and shows only
  its own in its interface. An app command with an engine command's id replaces it. A slimmer
  engine is a patch we have proposed upstream, not one we carry (`docs/UPSTREAM.md`).
- **The core adds general commands the engine lacks**, in `jcj_engine::commands`. Apps register
  them with `registry_with(commands::merged(app_specs))`; an app command with the same id
  replaces the core's (as `spelling.check` does when an app has its own spelling rules). None of
  these ids is an engine id, and a test fails if an upstream update adds one:

  | Commands | What they add to the engine |
  | --- | --- |
  | `find.all`, `find.next`, `find.previous`, `find.replace`, `find.replaceAll` | Literal find (no wildcards or `^` codes) over the whole body, wrapping; lists every match without moving the selection; limited to chosen paragraph styles; `find::replace_all_with` lets an app choose each replacement (a style's casing) |
  | `format.plain` | Removes the text styles set by hand but keeps font, size and marks (`format.resetChar` drops font and size) |
  | `format.resetToStyle` | Back to the paragraph style, keeping paragraph extension data and run marks (`para.reset` leaves character formatting; `format.clear` drops the style) |
  | `format.toggleCapitals` | All capitals, or small letters when there are no small letters (`format.changeCase {"mode": "toggle"}` swaps each letter) |
  | `edit.newPage` | A page break as a paragraph property, splitting at the caret (`text.pageBreak` inserts a break character) |
  | `edit.deleteParagraph`, `select.nextParagraph`, `select.previousParagraph` | Not in the engine |
  | `spelling.check`, `spelling.suggest`, `spelling.ignore`, `spelling.unignore`, `spelling.words`, `spelling.replace` | Spelling with words the document keeps as correct (`core.spelling`, undoable and saved), and a replace that checks the word is still there |

- **Where the engine already does the job, apps use the engine's id.** Script's ids map as
  follows (the same table, without the product's name, is in `jcj_engine::commands`' docs):

  | Script id | Core or engine id |
  | --- | --- |
  | `caret.left/right/up/down/wordLeft/wordRight/docStart/docEnd` | Engine, same ids |
  | `caret.lineStart`, `caret.lineEnd` | Engine `caret.home`, `caret.end` |
  | `select.left` ... `select.docEnd` | Engine `caret.*` with `{"extend": true}` |
  | `select.all` | Engine `select.all` |
  | `caret.to {pos, extend?}` | Engine `caret.set {pos, extend?}` |
  | `select.wordAt {pos}` | Engine `caret.set {pos}`, then `select.word` |
  | `text.insert`, `text.newParagraph`, `text.tab`, `text.backspace`, `text.delete` | Engine, same ids; Script's typing rules stay in Script |
  | `text.toggleCase` | `format.toggleCapitals` |
  | `text.pageBreak` | `edit.newPage` |
  | `paragraph.delete`, `paragraph.next`, `paragraph.previous` | `edit.deleteParagraph`, `select.nextParagraph`, `select.previousParagraph` |
  | `goto.page {page}` | Engine `edit.goto {page}`; page labels stay in Script |
  | `element.reset` | `format.resetToStyle` |
  | `format.bold`, `format.italic`, `format.underline` | Engine, same ids (`{"value"?: bool}`) |
  | `format.style {style, on?}` | Engine `format.<style> {value?}` (`format.strikethrough`, `format.allCaps`, ...; dotted: `format.underline {"style": "dotted"}`) |
  | `format.plain` | `format.plain` |
  | `format.highlight {color}` | Engine `format.highlight` (engine names), `format.shading` for `#RRGGBB`; Script's own color names stay in Script |
  | `format.color`, `format.state` | Engine, same ids (`format.state` answers per property) |
  | `format.lowercase` | Engine `format.changeCase {"mode": "lower"}` |
  | `find.all`, `find.next`, `find.previous` | Core, same ids; `styles` replaces `elements` |
  | `replace.one`, `replace.all` | `find.replace`, `find.replaceAll`; element casing stays in Script, through `replace_all_with` |
  | `spelling.check`, `spelling.suggest`, `spelling.ignore`, `spelling.replace` | Core, same ids; positions are `{start, end}` positions, not `{block, start, end}` |
  | `spelling.addWord` | Engine `review.addToDictionary` (it also saves the user dictionary) |

  Screenplay commands (`element.*`, `scene.*`, `dialogue.*`, `select.scene`, `goto.scene` and
  the rest) stay in Script.

### Agents: `jcj-agents`

- The MCP server is WordCraft's (`wordcraft-mcp`), headless over an in-process session, on the
  app's registry. Its general tools (list and run any command, type text, inspect the document,
  render a page, open and save files) work for every app unchanged; Script's trimmed tool set is
  not carried.
- `jcj-agents` adds the app's own name, version and instructions to `initialize`, and exports
  each app command under a chosen id prefix as a tool of its own (`notes.add` → `notes_add`).
  Files the commands read and write can be confined to chosen folders
  (`AgentConfig::confined`, the engine's `file_access`).
- craft-libs' `craft-mcp-server` is not at our pins; when it is, it replaces the server inside
  `jcj-agents` and apps do not change. Script's branding (server name, environment variables,
  profile folder) is each app's `AgentInfo`.

### PDF and printing: `jcj-output` (from Script 0013)

- PDF is written from the same layout the page view draws (`jcj_output::export_pdf`, or
  `export_pdf_with_layout`), so pages match the screen. With `PdfSettings::deterministic()` the
  same document gives the same bytes, for golden files. `render_page` and `render_area` draw page
  images for previews and tests.
- Printing (Cargo feature `print`) sends a print-ready PDF to the system spooler through
  craft-print: CUPS on macOS and Linux, the Windows spooler on Windows. The craft-print
  dependency Script avoided on Windows (sigstore) is not needed at our craft-libs pin
  (`docs/review/script-29ed4bf.md`). Script's fallback (`lp` by hand, a PDF viewer on Windows)
  is not carried.
- Screenplay PDF rules (title page, scene numbers in margins, revision colors) stay in Script.
  PDF/X waits for a crate at our pins.

### Spelling: `jcj-proof` (from Script 0014)

- `jcj-proof` wraps craft-proof: spelling, suggestions, the user dictionary and the session's
  ignored words, and hyphenation. English always has a word list: the SCOWL dictionaries when
  the language data is built in (`CRAFT_I18N_DIR` at build time, pinned in `docs/UPSTREAM.md`),
  else a system Hunspell English dictionary, else craft-proof's bundled public-domain list. Other
  languages use a system Hunspell dictionary when there is one.
- The engine's proofing commands and `jcj_engine::commands::spelling` use the same dictionaries,
  so a word added in one is right in the other.
- Script's rules about what not to check (paragraphs of some styles, names it lists, words in
  capitals in some paragraphs) stay in Script: its own `spelling.check` calls
  `misspellings_with` with them.

## Consequences

- Every app carries the engine's whole catalog in its binary and registry. Agents see it through
  `list_commands`; apps that want agents to see less export only their own prefix as tools.
- The engine's `para.reset` and `format.clear` used to drop paragraph extension data; our fork's
  extension-data patch now keeps it there too, and a test in `jcj-engine` pins that.
- Formatting at a caret inside a word formats the whole word, as the engine's commands do (Script
  formatted only what was typed next).

## Revisit when

The slimmer engine is accepted upstream; `craft-mcp-server` reaches our pins; a second app needs
a spelling rule `misspellings_with` cannot express.
