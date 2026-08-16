# dewey Release Cycle — Design

Date: 2026-08-15
Status: Approved
Name: dewey (github.com/tedwardd/dewey)

## Goal

An automated release cycle for dewey: continuous testing on every push and
PR, semver versioning + changelog driven by conventional commits, and
prebuilt binaries with checksums attached to GitHub Releases for
Linux x86_64, Linux aarch64, macOS x86_64, and macOS aarch64.

## Out of scope (explicit)

- crates.io publishing (deferred; binaries only for now).
- Installers / package managers (Homebrew tap, shell installer, npm).
- Windows binaries.
- Prerelease handling (alpha/beta/rc channels).
- Code-quality gates (clippy/fmt) — CI runs the test suite only.

## Architecture

Three GitHub Actions workflows plus one committed packaging script:

```
push/PR → ci.yml          → cargo test --locked (ubuntu-latest, hermetic)
push main → release-please.yml → release PR (version + CHANGELOG.md)
merge PR → tag vX.Y.Z + GitHub Release (created by release-please)
release created → release.yml → native-runner build matrix
                                → scripts/package.sh → tarballs
                                → combined SHA256SUMS.txt
                                → gh release upload
```

## ci.yml

Triggers: `push` to `main`, `pull_request`. One job `test` on
`ubuntu-latest`: checkout, `dtolnay/rust-toolchain@stable`,
`Swatinem/rust-cache@v2`, `cargo test --locked`. The suite is hermetic
(fixture mode, local TcpListener); the two live tests are `#[ignore]` and do
not run.

## release-please.yml

Triggers: `push` to `main`. Permissions: `contents: write`,
`pull-requests: write`. Job: `googleapis/release-please-action@v4` with
`release-type: rust` and `token: ${{ secrets.GITHUB_TOKEN }}`. Config-less
defaults: `feat:` → minor, `fix:` → patch, generated `CHANGELOG.md`. On
release-PR merge, release-please creates tag `v<version>` and the GitHub
Release.

## release.yml

Triggers: `release` `types: [created]`. Permissions: `contents: write`.

Build job — matrix on native runners (no cross-compilation):

| runner | artifact suffix |
|---|---|
| `ubuntu-latest` | `linux-x86_64` |
| `ubuntu-24.04-arm` | `linux-aarch64` |
| `macos-14` | `macos-aarch64` |
| `macos-14` (cross-compile `x86_64-apple-darwin`) | `macos-x86_64` |

Intel macOS is cross-compiled on the arm64 runner (GitHub no longer
provides reliable Intel macOS runners).

Steps per job: checkout, `dtolnay/rust-toolchain@stable`,
`Swatinem/rust-cache@v2`, `cargo build --release --locked`,
`./scripts/package.sh "<tag>" "<artifact>" target/release/dewey dist/`,
`actions/upload-artifact@v4` (name `dewey-<artifact>`, path `dist/`).

Publish job (`needs: build`, `ubuntu-latest`): download all artifacts with
`merge-multiple`, compute `sha256sum *.tar.gz > SHA256SUMS.txt`, upload
everything to the release:
`gh release upload "<tag>" dist/*` with `GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}`.

## Packaging contract (scripts/package.sh)

`./scripts/package.sh <tag> <artifact> <binary> <out-dir>`:

- `version = tag` with leading `v` stripped.
- Produces `<out-dir>/dewey-<artifact>-v<version>.tar.gz`.
- Archive contains one directory `dewey-<version>/` with the binary,
  `README.md`, and `LICENSE.md`.
- Script is committed and locally runnable — the workflow merely invokes it.
- `set -euo pipefail`; temp dir cleaned up on exit.

## Release flow (how a maintainer cuts a release)

1. Merge feature work to `main` (conventional commits).
2. release-please opens `chore(main): release <version>` PR (version bump +
   `CHANGELOG.md`).
3. Maintainer reviews and merges it.
4. Tag + GitHub Release are created automatically; release.yml builds and
   attaches `dewey-<platform>-v<version>.tar.gz` × 4 + `SHA256SUMS.txt`.

## Docs

- `README.md` Install section: "Prebuilt binaries (GitHub Releases)" table
  with download links per platform; build-from-source retained.
- `docs/releasing.md` (new): cycle walkthrough, how to cut a release, semver
  notes, and the explicit out-of-scope list.

## Verification

1. `scripts/package.sh` tested locally: release build, package, extract
   tarball, run the packaged binary.
2. Workflow YAML syntax-validated locally (`python3 -c yaml.safe_load`), plus
   `actionlint` when available.
3. End-to-end: cut the first release (v0.1.0) — merge the release-please PR,
   confirm the tag + release are created, and confirm the four tarballs +
   `SHA256SUMS.txt` are attached (verified via `gh release view`).
