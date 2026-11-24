# 0008: Format interchange over WordCraft's own format crates

Date: 2026-11-24 · Status: accepted

## Context

The plan named craft-libs' `craft-docx`, `craft-odf`, `craft-rtf`, `craft-markdown`, `craft-html`
and `craft-epub` as the base for `jcj-interchange`. At the craft-libs commits WordCraft v0.18.1
pins, none of those crates exist: WordCraft carries its own `wordcraft-docx` (DOCX) and
`wordcraft-formats` (ODT, RTF, Markdown, HTML, plain text, LaTeX), and nothing writes EPUB.

## Decision

- `jcj-interchange` has one small API: a `Format` enum (DOCX, ODT, RTF, Markdown, HTML, text,
  EPUB) with detection by extension and by content, `import(format, bytes)` and
  `export(format, &doc)`. Errors are values; input is capped (`MAX_IMPORT_BYTES`); upstream calls
  sit behind `catch_unwind` as a last guard.
- DOCX goes through `wordcraft-docx`; the others through `wordcraft-formats`.
- **EPUB 3 is written by the core** (`epub` module): `mimetype` first and stored, the container,
  an OPF package from the document's properties, a navigation document, one XHTML chapter per
  level-1 heading, pictures packaged once. Output is byte-for-byte deterministic. EPUB import is a
  gap.
- Text emphasis markers (`**bold**`, `*italic*`, `_underline_`, `\` escapes) are general
  (`emphasis` module); an app's own markers (Script's notes, revisions, page locks) stay in the app.
- Extension data is not carried by DOCX, ODT, RTF, Markdown, HTML or EPUB; the project container
  keeps it (decision 0009).

## Revisit when

craft-libs gains the format crates WordCraft moves to (follow them), or Book needs EPUB import or
EPUBCheck in CI (Book's own plan).
