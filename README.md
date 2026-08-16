# dewey

*Access open ebook libraries from your terminal.*

`dewey` is a command-line framework for open ebook libraries, named for the
Dewey Decimal System. A small Rust core talks to *modules* — one per library —
over a simple JSON-RPC protocol, so every library is accessed the same way:
search, browse, inspect, download.

It ships with modules for **Project Gutenberg** and **Standard Ebooks**, and
any library can be added by writing a module — see
[Developing library modules](docs/module-development.md).

## Features

- **Pluggable libraries.** Each library is a standalone program (Python,
  Lua, anything that speaks JSON over stdio) declaring what it can do. No
  core changes are needed to add a library.
- **Uniform access.** `search`, `categories`, `list`, `show`, and `download`
  behave the same across every library.
- **Host-owned downloads.** Progress bars, retries on transient failures,
  sanitized filenames, and an overwrite guard — one code path for all
  libraries.
- **Offline testing.** Modules can answer from recorded data via
  `DEWEY_FIXTURE`, so the test suite is hermetic and modules are easy to
  develop without a network.
- **Scriptable.** `--json` output and deterministic exit codes.

## Install

Requirements: Rust (stable) and `python3` (>= 3.8, for the bundled modules).

```bash
cargo build --release
install -m 755 target/release/dewey ~/.local/bin/
```

## Quick start

```bash
dewey install ./modules/gutenberg          # install a library module
dewey install ./modules/standard-ebooks

dewey libraries                            # what's available
dewey search "moby dick" --module gutenberg
dewey show 2701 --module gutenberg
dewey download 2701 --format epub --module gutenberg -o ~/Downloads
```

## Usage

### Commands

| Command | Description |
|---|---|
| `dewey libraries` | List available libraries and their capabilities |
| `dewey install <dir>` | Install a module directory into the discovery path (`--force` to overwrite) |
| `dewey search <query>` | Search a library (`--json` for machine-readable output) |
| `dewey categories` | List browsable categories/collections of a library |
| `dewey list --category <id>` | Browse a library's category |
| `dewey show <book-id>` | Show details and available formats |
| `dewey download <book-id> --format <fmt>` | Download a book in a format |

### Common flags

| Flag | Meaning |
|---|---|
| `--module <name>` | Which library to use (defaults to `default_module` in config) |
| `--limit <n>` | Max results (1–200, default 20) |
| `--json` | Emit normalized book records as JSON (search) |
| `-o, --dir <dir>` | Download directory (default: config `download_dir`, else `.`) |
| `--force` | Overwrite an existing file or module |

### Exit codes

| Code | Meaning |
|---|---|
| 0 | success |
| 1 | usage error (unknown module, empty query, bad flags) |
| 2 | module/protocol error |
| 3 | network/IO error (download failures) |
| 4 | config/discovery error |

## Configuration

`~/.config/dewey/config.toml`:

```toml
default_module = "gutenberg"
download_dir = "~/Downloads"
```

Environment variables:

| Variable | Meaning |
|---|---|
| `DEWEY_MODULES` | Directory to scan for modules (default `~/.config/dewey/modules`) |
| `DEWEY_FIXTURE` | Fixture directory — when set, modules answer from recorded data instead of the network |

## Modules

A module is a standalone program that translates a library's API into dewey's
protocol. Modules live in `~/.config/dewey/modules/<name>/` and are installed
with `dewey install`:

```bash
dewey install ./modules/gutenberg
```

`dewey libraries` shows what's installed and what each module can do.

**Modules are arbitrary programs that run with your privileges — only
install trusted modules. See [Security](#security).**

Want to add a library? The contract is small — read
[Developing library modules](docs/module-development.md), and use the bundled
`modules/gutenberg` and `modules/standard-ebooks` modules as working
references.

## Security

Modules are standalone programs that dewey executes with your user's
privileges — they are **not sandboxed**. Installing a module is equivalent
to running the software it contains: a malicious or compromised module can
read, modify, or delete your files, exfiltrate data, or otherwise do
anything your user account can do. Modules can also make their own network
requests — they are not limited to the library APIs they claim to wrap.

- **Only install modules from sources you trust.** Read a module's code
  (including the `command` in its `manifest.toml`) before installing.
- dewey does not verify a module's authorship or integrity. Treat every
  install the way you would treat installing any other software.
- The bundled `modules/gutenberg` and `modules/standard-ebooks` are written
  and reviewed in this repository; anything installed from elsewhere
  carries the same risk as running its code directly.

## Testing

```bash
cargo test                # unit + integration tests (hermetic, offline)
cargo test -- --ignored   # live tests against real library APIs (network)
```

## License

MIT — see [LICENSE.md](LICENSE.md).
