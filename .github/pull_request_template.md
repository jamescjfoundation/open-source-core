<!-- A pull request is optional while no app has a public release (decision 0013); a push to main follows the same list. -->

## What changed, and why

<!-- The reason as its own argument: no names, no conversation, nothing private. The general need, not one app's feature. -->

## Checked

- [ ] `cargo xtask ci` passes on this branch; `cargo xtask template-check` too if anything under `template/` changed.
- [ ] **The apps.** What every app must do after this change is written under "What apps must do" in `CHANGELOG.md`, or "nothing":
- [ ] **A public `jcj-` interface that breaks** says so in the title, and each app that uses it has an issue. Linked here, or "none":
- [ ] **A marked block or managed file** was changed under `template/`, and `cargo xtask template --write` was run.
- [ ] **Upstream.** A pin or a carried patch that changed is in `docs/UPSTREAM.md`, or "none":
- [ ] **No product** name, color, logo or concept in code, interface text or `template/`.
- [ ] **Nothing private**: no person's name, email, account, machine detail, secret or private writing, in the files or the commits.
