# dewey Release Cycle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Automated release cycle for dewey: CI tests on push/PR, release-please versioning + changelog, and per-platform binaries with checksums on GitHub Releases.

**Architecture:** Three GitHub Actions workflows (`ci.yml`, `release-please.yml`, `release.yml`) plus a committed, locally-testable packaging script (`scripts/package.sh`). release-please drives versioning; a native-runner build matrix packages binaries; a publish job computes a combined `SHA256SUMS.txt` and uploads to the release.

**Tech Stack:** GitHub Actions (v4 actions), Rust stable toolchain, bash packaging script, `gh` CLI for release upload.

**Spec:** `docs/superpowers/specs/2026-08-15-dewey-release-cycle-design.md` (approved).

## Global Constraints

- No crates.io, no installers, no Windows, no prerelease handling (per spec).
- CI runs `cargo test --locked` only — no clippy/fmt gates.
- Runners are native (no cross-compilation): `ubuntu-latest` (x64), `ubuntu-24.04-arm`, `macos-13` (x64), `macos-14` (arm64).
- Artifact naming: `dewey-<artifact>-v<version>.tar.gz`; archive contains `dewey-<version>/` with binary + `README.md` + `LICENSE.md`.
- One combined `SHA256SUMS.txt` computed in the publish job.
- release-please v4, `release-type: rust`, config-less, `token: ${{ secrets.GITHUB_TOKEN }}`.
- Do NOT run formatters or linters; run only the commands listed.

---

### Task 1: Packaging script

**Files:**
- Create: `scripts/package.sh` (executable)

**Interfaces:**
- Produces: `./scripts/package.sh <tag> <artifact-suffix> <binary> <out-dir>` → `<out-dir>/dewey-<artifact>-v<version>.tar.gz`; exits non-zero when the tag lacks a `v` prefix. Consumed by release.yml's matrix jobs.

- [ ] **Step 1: Create `scripts/package.sh`**

```bash
#!/usr/bin/env bash
# Package a dewey release artifact.
#
# Usage: package.sh <tag> <artifact-suffix> <binary> <out-dir>
#   tag          release tag, e.g. v0.1.0
#   artifact     platform suffix, e.g. linux-x86_64
#   binary       path to the built binary (target/release/dewey)
#   out-dir      directory to write dewey-<artifact>-v<version>.tar.gz into
set -euo pipefail

tag=$1
artifact=$2
binary=$3
out=$4

version=${tag#v}
if [ "$version" = "$tag" ]; then
    echo "error: tag must start with 'v' (got: $tag)" >&2
    exit 1
fi

mkdir -p "$out"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

pkg="$tmp/dewey-$version"
mkdir -p "$pkg"
cp "$binary" "$pkg/dewey"
cp README.md LICENSE.md "$pkg/"

tar -czf "$out/dewey-$artifact-v$version.tar.gz" -C "$tmp" "dewey-$version"
echo "wrote $out/dewey-$artifact-v$version.tar.gz"
```

- [ ] **Step 2: Make it executable and verify locally**

Run:
```bash
chmod +x scripts/package.sh
cargo build --release --locked
mkdir -p /tmp/dewey-pkg-test
./scripts/package.sh v0.1.0 linux-x86_64 target/release/dewey /tmp/dewey-pkg-test
tar -tzf /tmp/dewey-pkg-test/dewey-linux-x86_64-v0.1.0.tar.gz
mkdir -p /tmp/dewey-extract && tar -xzf /tmp/dewey-pkg-test/dewey-linux-x86_64-v0.1.0.tar.gz -C /tmp/dewey-extract
/tmp/dewey-extract/dewey-0.1.0/dewey --version
```

Expected: script prints `wrote …dewey-linux-x86_64-v0.1.0.tar.gz`; `tar -tzf` lists `dewey-0.1.0/`, `dewey-0.1.0/dewey`, `dewey-0.1.0/README.md`, `dewey-0.1.0/LICENSE.md`; the extracted binary prints `dewey 0.1.0`.

Also verify the guard: `./scripts/package.sh 0.1.0 linux-x86_64 target/release/dewey /tmp/dewey-pkg-test` must exit 1 with the `must start with 'v'` message. Clean up the temp dirs afterwards.

- [ ] **Step 3: Commit**

```bash
git add scripts/package.sh
git commit -m "chore: add release packaging script"
```

---

### Task 2: CI workflow

**Files:**
- Create: `.github/workflows/ci.yml`

**Interfaces:**
- Produces: workflow running `cargo test --locked` on push to `main` and on every PR.

- [ ] **Step 1: Create `.github/workflows/ci.yml`**

```yaml
name: ci

on:
  push:
    branches: [main]
  pull_request:

permissions:
  contents: read

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo test --locked
```

- [ ] **Step 2: Validate the YAML**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))" && echo OK`
Expected: `OK`. If `yaml` is not importable, install nothing — instead validate with `ruby -ryaml -e 'YAML.load_file(".github/workflows/ci.yml"); puts "OK"'` or note the skip.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: add test workflow"
```

---

### Task 3: release-please workflow

**Files:**
- Create: `.github/workflows/release-please.yml`

**Interfaces:**
- Produces: workflow opening release PRs (version + `CHANGELOG.md`) on push to `main`; on merge, creates tag + GitHub Release.

- [ ] **Step 1: Create `.github/workflows/release-please.yml`**

```yaml
name: release-please

on:
  push:
    branches: [main]

permissions:
  contents: write
  pull-requests: write

jobs:
  release-please:
    runs-on: ubuntu-latest
    steps:
      - uses: googleapis/release-please-action@v4
        with:
          release-type: rust
          token: ${{ secrets.GITHUB_TOKEN }}
```

- [ ] **Step 2: Validate the YAML**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release-please.yml'))" && echo OK`
Expected: `OK` (or the ruby equivalent from Task 2).

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/release-please.yml
git commit -m "ci: add release-please workflow"
```

---

### Task 4: Release build workflow

**Files:**
- Create: `.github/workflows/release.yml`

**Interfaces:**
- Consumes: `scripts/package.sh` (Task 1); the GitHub Release created by release-please (Task 3).
- Produces: on `release: created`, four `dewey-<artifact>-v<ver>.tar.gz` + combined `SHA256SUMS.txt` attached to the release.

- [ ] **Step 1: Create `.github/workflows/release.yml`**

```yaml
name: release

on:
  release:
    types: [created]

permissions:
  contents: write

jobs:
  build:
    name: build ${{ matrix.artifact }}
    strategy:
      fail-fast: false
      matrix:
        include:
          - os: ubuntu-latest
            artifact: linux-x86_64
          - os: ubuntu-24.04-arm
            artifact: linux-aarch64
          - os: macos-13
            artifact: macos-x86_64
          - os: macos-14
            artifact: macos-aarch64
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo build --release --locked
      - run: ./scripts/package.sh "${{ github.event.release.tag_name }}" "${{ matrix.artifact }}" target/release/dewey dist/
      - uses: actions/upload-artifact@v4
        with:
          name: dewey-${{ matrix.artifact }}
          path: dist/

  publish:
    needs: build
    runs-on: ubuntu-latest
    steps:
      - uses: actions/download-artifact@v4
        with:
          pattern: dewey-*
          path: dist/
          merge-multiple: true
      - run: |
          cd dist
          sha256sum *.tar.gz > SHA256SUMS.txt
          cat SHA256SUMS.txt
      - run: gh release upload "${{ github.event.release.tag_name }}" dist/*
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

- [ ] **Step 2: Validate**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml'))" && echo OK`
Expected: `OK`. Then attempt `actionlint` (optional, if the download works):

Run: `curl -sL https://github.com/rhysd/actionlint/releases/latest/download/actionlint_linux_amd64.tar.gz | tar xz -C /tmp && /tmp/actionlint .github/workflows/*.yml`
Expected: no errors. If the download fails, note the skip — the YAML parse above is the gate.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci: add release build workflow"
```

---

### Task 5: Docs — install binaries, releasing guide

**Files:**
- Modify: `README.md` (Install section)
- Create: `docs/releasing.md`

**Interfaces:**
- Produces: README prebuilt-binaries table; a maintainer's guide to cutting a release.

- [ ] **Step 1: Add the prebuilt-binaries section to `README.md`**

Replace the current `## Install` section body with:

```markdown
## Install

### Prebuilt binaries (GitHub Releases)

Grab the archive for your platform from the
[latest release](https://github.com/tedwardd/dewey/releases) and verify it
against `SHA256SUMS.txt`:

| Platform | Artifact |
|---|---|
| Linux x86_64 | `dewey-linux-x86_64-v<version>.tar.gz` |
| Linux aarch64 | `dewey-linux-aarch64-v<version>.tar.gz` |
| macOS x86_64 | `dewey-macos-x86_64-v<version>.tar.gz` |
| macOS aarch64 | `dewey-macos-aarch64-v<version>.tar.gz` |

```bash
tar -xzf dewey-linux-x86_64-v0.1.0.tar.gz
install -m 755 dewey-0.1.0/dewey ~/.local/bin/
```

### Build from source

Requirements: Rust (stable) and `python3` (>= 3.8, for the bundled modules).

```bash
cargo build --release
install -m 755 target/release/dewey ~/.local/bin/
```
```

(Nested code fences: the README file must contain real fenced blocks, not
the escaped ones shown in this plan.)

- [ ] **Step 2: Create `docs/releasing.md`**

```markdown
# Releasing dewey

The release cycle is automated with GitHub Actions. This page is the
maintainer's guide.

## How the cycle works

- `ci.yml` runs `cargo test` on every push and pull request.
- `release-please.yml` watches `main`. Conventional commits drive the next
  version: `feat:` → minor bump, `fix:` → patch bump. It opens (and updates)
  a single release PR that bumps `Cargo.toml` and regenerates
  `CHANGELOG.md`.
- Merging that release PR creates the `v<version>` tag and the GitHub
  Release automatically.
- `release.yml` reacts to the created release: four native-runner jobs build
  and package binaries for Linux x86_64, Linux aarch64, macOS x86_64, and
  macOS aarch64, then a publish job computes `SHA256SUMS.txt` and uploads
  everything to the release.

## Cutting a release

1. Merge your feature work to `main` using conventional commit messages
   (`feat:`, `fix:`, `docs:`, `chore:`, …). Only `feat:` and `fix:` affect
   the version.
2. Wait for release-please to open `chore(main): release <version>`.
3. Review the PR — check the version bump and the changelog entries.
4. Merge it. The tag and Release are created; binaries attach within a few
   minutes.
5. Verify: `gh release view v<version> --json tagName,assets` — expect the
   four tarballs and `SHA256SUMS.txt`.

## Semver rules (release-please defaults)

- `feat:` → minor (`0.1.0` → `0.2.0`)
- `fix:` → patch (`0.1.0` → `0.1.1`)
- `docs:`/`chore:`/`ci:`/`test:`/`refactor:` → no release
- Breaking changes: add `!` (e.g. `feat!:`) → major bump when ≥ 1.0

## Not yet automated

- crates.io publishing (`cargo install dewey`)
- Installers / Homebrew tap
- Windows binaries
- Prerelease channels (alpha/beta/rc)
```

- [ ] **Step 3: Verify README renders correctly**

Run: `grep -n "Prebuilt binaries" README.md && grep -n "releasing.md" README.md`
Expected: the section header exists and the docs link points at
`docs/releasing.md` (add a `## Releasing` link in README's docs area if
missing — e.g. under the Modules section: `See [Releasing](docs/releasing.md)`).

- [ ] **Step 4: Commit**

```bash
git add README.md docs/releasing.md
git commit -m "docs: prebuilt binaries install section and releasing guide"
```

---

### Task 6: First release end-to-end (consent-gated)

**Files:**
- None (GitHub-side flow).

**Interfaces:**
- Consumes: all four workflows; proves the entire cycle.

- [ ] **Step 1: Push and watch release-please**

Run: `git push` (the workflows are now on main; the push triggers
release-please).
Then poll: `gh pr list --repo tedwardd/dewey` — expect a PR titled
`chore(main): release 0.1.0` (may take a minute).

- [ ] **Step 2: Inspect the release PR**

Run: `gh pr diff <pr-number> --repo tedwardd/dewey | head -40`
Expected: `Cargo.toml` unchanged (version already 0.1.0) and a new
`CHANGELOG.md` listing the merged commits.

- [ ] **Step 3: Merge the release PR (consent required — creates public v0.1.0)**

Run: `gh pr merge <pr-number> --repo tedwardd/dewey --merge`
Expected: merge succeeds; release-please creates tag `v0.1.0` + the GitHub
Release; `release.yml` starts.

- [ ] **Step 4: Wait for binaries and verify**

Poll: `gh run list --repo tedwardd/dewey --workflow release.yml`
Then: `gh release view v0.1.0 --repo tedwardd/dewey --json tagName,assets`
Expected: `tagName: v0.1.0`; assets include `dewey-linux-x86_64-v0.1.0.tar.gz`,
`dewey-linux-aarch64-v0.1.0.tar.gz`, `dewey-macos-x86_64-v0.1.0.tar.gz`,
`dewey-macos-aarch64-v0.1.0.tar.gz`, and `SHA256SUMS.txt`.

- [ ] **Step 5: Spot-check an artifact**

Run: download the linux x86_64 tarball via `gh release download v0.1.0 --repo tedwardd/dewey -p "dewey-linux-x86_64*"`; extract and run
`./dewey-0.1.0/dewey --version`; verify the checksum matches:
`sha256sum -c SHA256SUMS.txt`.
Expected: `dewey 0.1.0` and `dewey-linux-x86_64-v0.1.0.tar.gz: OK`.

- [ ] **Step 6: Final state**

Run: `git log --oneline -3 && gh release view v0.1.0 --repo tedwardd/dewey --json assets -q '.assets[].name'`
Expected: release cycle complete and documented in the ledger.
