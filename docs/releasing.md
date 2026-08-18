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
  Release automatically. The release PR is auto-merged once CI goes green, so
  the whole cycle from a `main` merge to a published release is hands-off.
- `release.yml` reacts to the created release: four jobs build and package
  binaries for Linux x86_64, Linux aarch64, macOS aarch64, and macOS x86_64
  (the Intel build is cross-compiled on the arm64 runner), then a publish
  job computes `SHA256SUMS.txt` and uploads everything to the release.
  Release builds can also be re-run manually with
  `gh workflow run release.yml -f tag=v<version>`.
- `aur.yml` is dispatched explicitly by `release.yml` once the binaries are
  attached, and updates both `dewey` and `dewey-bin` on the AUR. (It no
  longer listens for the `release: published` event, whose timing could race
  the binary build.)

## Cutting a release

1. Merge your feature work to `main` using conventional commit messages
   (`feat:`, `fix:`, `docs:`, `chore:`, …). Only `feat:` and `fix:` affect
   the version.
2. release-please opens `chore(main): release <version>` and auto-merges it
   once `ci.yml` is green. No action needed.
3. Verify: `gh release view v<version> --json tagName,assets` — expect the
   four tarballs and `SHA256SUMS.txt`.
4. Confirm the AUR sync: `gh run list --workflow aur.yml` should show a
   success for `v<version>`.

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
