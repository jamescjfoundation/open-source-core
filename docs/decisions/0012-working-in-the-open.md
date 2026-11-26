# 0012: Working in the open

Date: 2026-11-24 · Status: accepted · Amended 2026-11-26 (reviews and working notes)

Every repository of the foundation is public, history included. The rules are in `CONTRIBUTING.md`
and `AGENTS.md`; this record says how they are enforced.

## Decision

- **Commit identity**: a GitHub no-reply address or a foundation bot identity. `cargo xtask
  privacy` fails on any commit whose author or committer email is anything else.
- **Content**: `cargo xtask privacy` scans every tracked text file for email addresses (other than
  no-reply and example addresses), home-folder paths, and the shapes of common keys and tokens. A
  line that must keep a match carries the marker `privacy:allow`. The pre-commit hook runs the same
  scan on the staged lines.
- **Fixtures** are public-domain works or text written for the purpose; never anyone's private
  writing.
- **Reviews, audits and working notes** stay in `.private/` (ignored by Git). One is published
  only when a maintainer says so; it then lives under `docs/history/` and its first lines say it
  is history and not maintained. Finished plans kept for the record live there too. What a review
  concluded is written publicly as a decision record or a changelog entry, with the reason itself.
- **Contact** is GitHub's private vulnerability reporting (`SECURITY.md`); no personal inbox.
- **Secrets** live only in GitHub's encrypted secrets; secret scanning and push protection are on.

## Why the 2026-11-26 amendment

The rule for reviews was written three ways across the repositories ("always private", "private
if not ready", and a standing `docs/review/` folder). A review is written to find faults, often
quotes things that should not be public, and goes stale the day after; a decision record is the
durable, public form of what it found. Private by default with one labelled place for the few
published ones gives a reader one answer: anything under `docs/history/` is the past, anything
else in `docs/` is current.
