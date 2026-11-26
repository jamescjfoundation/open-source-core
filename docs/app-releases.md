# App releases: the contract

What a public release of an app built on open-source-core is made of, so that three readers
find, verify and install it without knowing the app: the app's own update check (`jcj-shell`,
over craft-update), a launcher that installs and updates several apps, and a website that links
the downloads. Decision 0018 says why it is this way.

The contract is kept by code, not by hand. `cargo xtask package` (the shared xtask) names and
lays out the files, the shared workflow `shared-release.yml` builds, checks and publishes them,
and an app gives both nothing but data (`xtask/src/main.rs`). An app has no release workflow and
no packaging script of its own.

In the examples the app's release id is `sample-app` and the version is `1.2.3`.

## The release

| | |
| --- | --- |
| Where | A GitHub release of the app's own repository, published (never left a draft) and not marked a pre-release |
| Version | `X.Y.Z`: three numbers and nothing after them. It is the version under `[workspace.package]` of the app's `Cargo.toml` |
| Tag | `vX.Y.Z`, annotated, on the release commit of `main`. Made by the release workflow, never by hand; never moved or deleted |
| Title | `<the product's name> X.Y.Z` |
| Text | The section of the app's `CHANGELOG.md` for the version, as it stands there |
| Files | Exactly four: one package for each system, and `SHA256SUMS.txt` |

A version has no pre-release or build part (`1.2.3-rc.1`, `1.2.3+5`). The update check offers
only full releases, so a pre-release would be a release that no installed copy is ever offered;
what is tried before a release is tried with a dry run (below). A release that went out wrong is
mended by the next version.

The changelog has one `## ` section for each release, newest first. Until its release a section
is `## Unreleased`; the release commit turns it into `## X.Y.Z (YYYY-MM-DD)`.

## The files

| System | File | Kind |
| --- | --- | --- |
| Linux, x86_64 | `sample-app-1.2.3-linux-x86_64.tar.gz` | A gzip-compressed tar archive |
| macOS, Apple silicon | `sample-app-1.2.3-macos-aarch64.dmg` | A compressed disk image |
| Windows, x64 | `sample-app-1.2.3-windows-x64-portable.zip` | A zip archive; nothing is installed system-wide |
| | `SHA256SUMS.txt` | The checksums of the three |

The pattern is `<id>-<version>-<system>-<processor><kind>`, and every part is fixed:

- `<id>` is the app's release id: lower-case letters, digits and `-`. It is also the name of the
  app's desktop program and the id the app gives its update source (`ReleaseSource::id`).
- `<system>-<processor>` is `linux-x86_64`, `macos-aarch64` or `windows-x64`, spelled as here.
- `<kind>` is `.tar.gz`, `.dmg` or `-portable.zip`. The Windows name says `portable` because the
  update check reads the kind from it; the archive is an ordinary zip.
- A name holds only letters, digits, `.`, `-` and `_`.

These are the names craft-update's reader takes at the core's pin
(`crates/jcj-shell/src/updates/desktop.rs` tests it against that reader). A file named any other
way is not found by an installed copy.

### `SHA256SUMS.txt`

One line for each package, in the order of the names:

```text
3b7e0c1f5a9d2e486f0b1c7d9a3e5f2048c6b1d7e9f3a5c20b4d6e8f1a3c5e79  sample-app-1.2.3-linux-x86_64.tar.gz
9f2c4e6a8b0d1f3e5a7c9b1d3f5e7a9c0b2d4f6e8a1c3e5b7d9f0a2c4e6b8d1f  sample-app-1.2.3-macos-aarch64.dmg
c4a6e8f0b2d4c6a8e0f2b4d6c8a0e2f4b6d8c0a2e4f6b8d0c2a4e6f8b0d2c4a6  sample-app-1.2.3-windows-x64-portable.zip
```

A line is the SHA-256 checksum as 64 lower-case hexadecimal digits, two spaces, the bare file
name, and a line feed. No `./` before the name, no `*`, no folder, no carriage return, no blank
or comment line, and the file does not list itself. This is what `sha256sum` writes and
`sha256sum --check` reads. [`app-release-example/SHA256SUMS.txt`](app-release-example/SHA256SUMS.txt)
is the example the tests hold both the packaging and the update check to.

### Inside the Linux package

One folder, named like the file without its kind, laid out like `~/.local`:

```text
sample-app-1.2.3-linux-x86_64/
  bin/sample-app                    the desktop program, named by the release id
  bin/sample-app-cli                every other program, named <id>-<name>
  share/applications/<app id>.desktop
  share/icons/hicolor/<N>x<N>/apps/<app id>.png
  share/mime/packages/<app id>.xml  when the app has file types of its own
  share/sample-app/                 the license files (below)
  install.sh                        copies bin/ and share/ into ~/.local (PREFIX=<folder> for elsewhere)
  uninstall.sh                      removes those files again
```

The update check replaces a copy in place by this layout: it takes `bin/<id>`, the other
programs of `bin/` that are installed beside it, and, where the installed copy has a
`share/<id>` folder, every file under `share/`. So the menu entry names the program by its
name, not by a path, and `install.sh` leaves it so.

The package is built on GitHub's `ubuntu-24.04` image and needs that system's C library
(glibc 2.39) or a newer one.

### Inside the Windows package

One folder, `sample-app-1.2.3-windows-x64/`, with at its top the programs (`sample-app.exe`
first, every other one `sample-app-<name>.exe`), the license files, and, when the app opens file
types, `register-file-types.cmd` and `unregister-file-types.cmd`, which register them for the
current user and take them away. The folder runs from wherever it is unpacked.

### Inside the macOS disk image

The volume is named after the product and holds `<the product's name>.app`, a link to
`/Applications`, and a folder `Legal` with the license files. Inside the app:
`Contents/MacOS/` holds the programs (the desktop program is `CFBundleExecutable`),
`Contents/Info.plist` the name, the app id as `CFBundleIdentifier`, the version and the file
types, `Contents/Resources/<app id>.icns` the icon, and `Contents/Resources/` the license files
again, so that they stay with the app once it is copied off the image. It runs on macOS 11 and
later.

### The license files, in every package

Beside each other: `LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`, `ATTRIBUTION.md`, the legal files
the app adds, and `THIRD-PARTY-LICENSES.txt`, the license text of every package the programs are
built from. Under `licenses/`, by their path in the repository, the license texts of what the
programs embed (fonts, data). They sit in `share/<id>/` of the Linux package, at the top of the
Windows folder, and in `Contents/Resources/` of the macOS app. A package without its programs or
without `THIRD-PARTY-LICENSES.txt` is not made: the task stops.

## Signing

The steps are part of the workflow and run when their secrets exist, in the repository or the
organisation. Without them the packages are published unsigned, and the run's summary says so.

| Secret | What it holds |
| --- | --- |
| `MACOS_CERTIFICATE_P12` | A Developer ID Application certificate with its private key, exported as a `.p12` file, in base64 |
| `MACOS_CERTIFICATE_PASSWORD` | The password of that file |
| `MACOS_NOTARY_KEY` | An App Store Connect API key (the `.p8` file) allowed to notarise, in base64 |
| `MACOS_NOTARY_KEY_ID` | That key's id |
| `MACOS_NOTARY_ISSUER_ID` | The issuer id the key belongs to |
| `WINDOWS_CERTIFICATE_PFX` | A code signing certificate with its private key, as a `.pfx` file, in base64 |
| `WINDOWS_CERTIFICATE_PASSWORD` | The password of that file |

- **macOS.** With the certificate, the programs and the app are signed under the hardened
  runtime with a timestamp, and the disk image is signed; with the notary key as well, the
  image is notarised and the ticket is fixed to it. Without the certificate the app is signed
  ad hoc: it claims no identity, and it starts on Apple silicon, where a program with no
  signature at all does not. A downloaded ad hoc app is held back by Gatekeeper until its user
  allows it.
- **Windows.** With the certificate, every program is signed (SHA-256, with a timestamp) before
  the archive is made. Without it they are unsigned and SmartScreen warns on the first start.
  A certificate whose key cannot leave a hardware token or a signing service has no `.pfx`
  file; signing with one is a different step, to be added when such a certificate is chosen.
- **Linux.** Not signed: the package is verified by its checksum and its attestation.

Every release file also gets a build provenance attestation, signed through the run's own
identity and kept by GitHub, whatever the certificates:

```sh
gh attestation verify sample-app-1.2.3-linux-x86_64.tar.gz --repo <owner>/<app> \
  --signer-repo jamescjfoundation/open-source-core
```

`--signer-repo` names the core because the jobs that build the files are the core's shared
workflow.

## Updating in place

What the update check does with a release, at the core's pin of craft-update:

| System | In place | Otherwise |
| --- | --- | --- |
| Linux | A copy outside the system folders (under the home folder, `/usr/local`) replaces itself from the `.tar.gz` and starts the new version | A copy under `/usr`, `/opt`, a package manager's folder or a build folder is offered the release page |
| Windows | Only a copy signed with the same publisher's certificate as the download: the helper refuses anything else | An unsigned copy is offered the release page |
| macOS | No: craft-update replaces only an app that carries its own project's bundle id | The release page, where the new disk image is |

An app tells the update check where it cannot install (`ReleaseSource::installs_in_place`,
false on macOS), calls `ReleaseSource::finish_update` first in `main` (the Windows helper is a
copy of the program itself), closes its window when `Shell::close_requested` says so, and calls
`Shell::finish` last before it exits, which starts the installed version.

## What an app gives: its data

In `xtask/src/main.rs`, read by `cargo xtask package`, `cargo xtask release` and the gate's
`release-data` check:

```rust
const APP: App = App {
    product: "Sample App",
    // The packages that build the programs, the desktop program first.
    shipped: &["sample-app", "sample-app-cli"],
    package: Some(Package {
        id: "sample-app",
        app_id: "org.example.sample",
        summary: "Write samples",
        categories: "Office;WordProcessor;",
        icons: &[(256, "assets/brand/icon-256.png"), (512, "assets/brand/icon-512.png")],
        file_types: &[FileType { name: "Sample document", extensions: &["smp"], mime: "application/x-sample", ..FileType::DEFAULT }],
        license_files: &["assets/fonts"],
        ..Package::DEFAULT
    }),
    ..App::DEFAULT
};
```

- **`product`** is the name people see: the release's title, the macOS app, the menu entry.
- **`shipped`** names workspace packages. Each builds one program. The first builds the desktop
  program, which is named by the release id; every other program is named `<id>-<name>`, so
  that two apps installed side by side share no name and the update check knows the app's own.
- **`id`**, the release id, is also the `id` of the app's `ReleaseSource`; **`app_id`** is also
  the app id the window is given (`ViewportBuilder::with_app_id`), which is how a Linux desktop
  joins a window to its menu entry and icon. An app keeps a test that the two places agree.
- **`icons`** are PNG files, as (pixels a side, path). Every size goes into the Linux package;
  the sizes a macOS icon holds (16, 32, 64, 128, 256, 512, 1024) go into the `.icns`, unless
  `macos_icons` names files drawn for it. `windows_icon` names the `.ico` file the app's build
  script embeds in the Windows program; the package task checks that it is in there.
- **`file_types`**: a name, extensions, a media type; `parent` for the type it is a kind of
  (`application/zip`, `text/plain`); `folder: true` for a project kept as a folder that a
  desktop shows as one document; `own: false` for another program's format, which the app is
  offered for and never made the owner of.
- **`license_files`**: files, or folders searched for files named like a license (`LICENSE`,
  `OFL`, `NOTICE`, `COPYING`).

The gate's `release-data` check (every push) fails when a file the data names is missing, an
icon is not the size it is said to be, a program is not named as above, or the changelog's
newest section is neither `## Unreleased` nor a dated release.

## Releasing

A person or an agent cuts a release, on a commit whose gate has passed. The gate runs first and
the workflow makes the tag, as for the core (decision 0011).

1. Move the app to the newest core (`cargo xtask core-pin --behind` says how far it is).
2. Write what the release changes under `## Unreleased` of `CHANGELOG.md`.
3. `cargo xtask release X.Y.Z` makes the changes of the release commit: the version in
   `Cargo.toml` and `Cargo.lock`, and the changelog heading with today's date in UTC.
4. `cargo xtask ci`, commit as "<the product's name> X.Y.Z", push to `main`.
5. Start the release:

   ```sh
   gh workflow run release.yml --ref main
   gh run watch "$(gh run list --workflow release.yml --limit 1 --json databaseId --jq '.[0].databaseId')"
   ```

   It checks that the commit is a release commit that is not released yet and that the app is
   on the newest core; runs the whole gate on Linux, macOS and Windows, and cargo-deny; builds
   the package of each system; checks the three files and writes `SHA256SUMS.txt`; attests
   them. Only when all of that has passed does it make the tag and publish the release.
6. A red job stops the release and no tag exists. Fix the cause on `main` and start the workflow
   again: the version was never tagged, so the commit with the fix is the release commit now.
   When only a runner or the network was at fault: `gh run rerun <run> --failed`.
7. Do not make or push a `v*` tag by hand, and do not create or edit a release by hand: nothing
   runs for a pushed tag, and it cannot be taken back.

Locally, `cargo xtask package` builds this system's package into `dist/`, and
`cargo xtask package --sums dist` writes the checksum file for what is there.

## A dry run

`gh workflow run release.yml --ref <branch> -f dry-run=true` builds the package of every system
on any commit, release commit or not, checks the three files, writes the checksum file, and
stops: no tag, no release, no attestation, nothing signed with a real identity. The files are
kept with the run for a week (`gh run download <run> --name release`). `-f gate=true` adds the
full gate on the three systems.

The same dry run starts by itself once a week in every app, so a release that would fail is
known before it is needed; when the scheduled run fails, an issue is opened in the app. The
core tries the packaging itself, on the three systems, every night and before each of its own
releases, with the template app (`cargo xtask template-check --package`).

## For a launcher or a website

- The newest release of an app: `GET https://api.github.com/repos/<owner>/<app>/releases`, the
  newest by version among those that are neither a draft nor a pre-release. With no API
  allowance left, `https://github.com/<owner>/<app>/releases/latest` redirects to
  `.../releases/tag/vX.Y.Z`, and every file is then at
  `https://github.com/<owner>/<app>/releases/download/vX.Y.Z/<file>`.
- The file for a system is found by its name alone, from the id, the version and the table
  above. Nothing else in the release needs reading.
- Before a file is opened, its SHA-256 checksum is compared with its line in `SHA256SUMS.txt`
  of the same release (GitHub also gives each file's checksum in the API's `digest` field).
  A file with no line, or with two different ones, is not installed.
- A website has nothing to do at a release: it reads the public release. A system with no file
  is shown as having none, never as a wrong link.

## When the contract changes

A file name, the checksum file's form and the place of a program inside a package are read by
copies already installed, which cannot be changed after the fact. A change there keeps what
those copies read for as long as they can still be in use, is decided in a new record that
amends decision 0018, and is said in the core's changelog under **What apps must do**.
