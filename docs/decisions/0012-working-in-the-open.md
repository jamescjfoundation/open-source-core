# 0012: Working in the open

Date: 2026-11-24 · Status: accepted

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
- **Contact** is GitHub's private vulnerability reporting (`SECURITY.md`); no personal inbox.
- **Secrets** live only in GitHub's encrypted secrets; secret scanning and push protection are on.
