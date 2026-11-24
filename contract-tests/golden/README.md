# Golden files

Outputs the contract tests compare against, byte for byte.

| File | What it holds | Test |
| --- | --- | --- |
| `sample-pdf.sha256` | SHA-256 of the sample document's PDF (two pages: title, drop cap, footnote, page break) written with `PdfSettings::deterministic()` | `tests/pdf.rs` |

When an output change is intended (a new upstream version that draws text differently, a
change to the sample), rewrite them and say why in the commit:

```sh
UPDATE_GOLDEN=1 cargo test -p contract-tests
```

A golden file that changes without a reason is behavior drift: find out why before accepting it.
