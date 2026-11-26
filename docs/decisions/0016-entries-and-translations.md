# 0016: Entries and interface translations

Date: 2026-11-26 · Status: accepted · Records two crates added after decision 0014 (releases 0.1.4
and 0.1.13)

## Decision

- **`jcj-entries`, layer 2: entries kept beside documents.** An entry is a typed record with a
  name, aliases, fields, relations to other entries and a free body. Entry types and their field
  templates are data an app ships and a user extends, not code. The crate is generic over the
  app's id type and body type, so each app stores and edits entries its own way and shares the
  rules: which names an entry answers to, how a name is found in text (whole words, matching case
  when the name starts with a capital), and which entries refer to which. It depends on `serde`
  only and knows nothing of documents, files or the interface.
- **`jcj-i18n`, layer 2: interface translation catalogs.** One TOML file per language maps each
  interface text, keyed by its English exactly as the code writes it, to the text shown. Plurals
  take the language's forms; `{name}` placeholders are filled; catalogs are layered, an app's
  first and then the shell's; a text no catalog has is shown in English. Reads are bounded. The
  crate also finds the texts a source file asks for, so a test can require that every text is in
  every catalog and no catalog keeps a text nothing uses.
- **The shell shows its own texts through the catalogs** (`jcj_shell::i18n`), and the labels,
  titles and tooltips an app gives it. Nothing is translated until an app sets a language, so an
  app that does not use translations is unchanged.
- **`jcj-features`** (layer 4) has its own record, decision 0010. With these two, every crate in
  the layer table of decision 0002 has a decision.

## Why

Both began as one app's own code and came to the core when a second app needed the same thing
(decision 0013). They
sit at layer 2 because they need nothing above the model: an app's model and format crates can
use entries, and its command line can use catalogs, without the engine or a window.

Keying a translation by the English text keeps the code readable and makes English need no
catalog; the cost, that changing an English text orphans its translations, is what the
compare-in-a-test helper is for.

## Revisit when

An app needs entries stored by the core (they would then join `jcj-project`'s container), or a
language needs something the plural forms and placeholders cannot say (gender, ordering by
context).
