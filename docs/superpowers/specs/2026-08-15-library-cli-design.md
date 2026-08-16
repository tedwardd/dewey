# library-cli — Design

Date: 2026-08-15
Status: Approved (pending written review)
Working name: `library-cli` (final name TBD)

## Overview

`library-cli` is a command-line framework for accessing open ebook libraries.
A Rust host binary provides the verbs (search, browse, download); each
library is accessed through a *module* — an independent program, written in
any language (reference modules use Python 3 stdlib-only), that the host
spawns per operation and talks to over a line-based JSON-RPC protocol.

Reference libraries targeted first: **Project Gutenberg** (gutendex JSON API)
and **Standard Ebooks** (OPDS Atom catalog). Both are plain HTTP APIs with no
authentication.

## Goals

- A single Rust binary that discovers modules, talks to them over a
  well-specified protocol, and normalizes every library onto one Book record.
- Modules as standalone programs: trivially testable by hand
  (`echo <request> | python3 module.py`), crash-isolated, and writable in any
  language that can read stdin and print JSON.
- No external runtime dependencies for reference modules (Python 3 stdlib only).
- Downloading is host-owned: modules return direct file URLs; the host fetches
  with progress and uniform error handling.

## Non-goals (explicitly out of scope)

- Interactive TUI (subcommands only; a picker could be layered on later).
- Authentication/OAuth.
- Result caching, indexing, or local library management.
- Plugin packaging/distribution (modules are copied into a directory).
- Resumable downloads. Signed/temporary download URLs (protocol extension later).
- Concurrency: one module process per CLI invocation, one request per process.

## Architecture

```
library-cli (Rust binary)
 ├─ cli layer      — clap subcommands, output formatting
 ├─ discovery      — scan module dirs, load + validate manifests
 ├─ module host    — spawn module, NDJSON/JSON-RPC transport, timeout
 └─ downloader     — fetch URL from a Book's format map, progress bar
```

Module discovery path: `~/.config/library-cli/modules/<name>/`
(overridable via `LIBRARY_CLI_MODULES` env var, which supports one directory;
the repo's `modules/` dir is exercised this way in development).
Each module dir contains a `manifest.toml` and the module's code.

## Module manifest

`modules/<name>/manifest.toml`:

```toml
name = "gutenberg"
version = "0.1.0"
description = "Project Gutenberg via gutendex"
command = ["python3", "module.py"]
capabilities = ["search", "book"]
```

Validation rules (host-side, enforced at load):

- `name`: required, `^[a-z0-9-]+$`, must equal the containing directory name.
- `version`: required, non-empty string.
- `description`: optional.
- `command`: required non-empty array of strings. The first element is
  resolved relative to the module dir if it does not contain a path separator
  resolving to an existing executable; remaining elements resolve relative to
  the module dir.
- `capabilities`: required, non-empty subset of
  `["search", "categories", "list", "book"]`. The host only surfaces verbs
  backed by a declared capability.
- A module with an invalid manifest is reported as broken; addressing it
  yields exit code 4 and a message naming the validation failure.

## Protocol

JSON-RPC 2.0 over newline-delimited JSON (NDJSON):

- One complete JSON object per line, LF-terminated, compact (non-pretty),
  UTF-8. No other bytes — including logs — may be written to module stdout;
  diagnostics go to stderr.
- Host sends exactly one **request**; module responds with exactly one
  **response**; module then exits 0. One-shot per CLI verb.
- `id` is an integer, echoed by the module. Unknown/extra fields are ignored
  (forward compatibility).
- Host-enforced timeout: 30 s per exchange. On timeout or non-zero exit
  without a response line, the host reports a module failure (exit code 2).

Request:

```json
{"jsonrpc":"2.0","id":1,"method":"search","params":{"query":"dune","page":1,"limit":20}}
```

Response (exactly one of):

```json
{"jsonrpc":"2.0","id":1,"result":{...}}
{"jsonrpc":"2.0","id":1,"error":{"code":-32000,"message":"network: timeout"}}
```

### Methods

| Method | Params | Result | Requires capability |
|---|---|---|---|
| `search` | `query` (string, required), `page` (int, 1-indexed, default 1), `limit` (int, default 20, max 200) | `{books: [Book], total?: int}` | `search` |
| `categories` | — | `{categories: [{id, title}]}` | `categories` |
| `list` | `category` (string, required), `page`, `limit` (as search) | `{books: [Book], total?: int}` | `list` |
| `book` | `id` (string, required) | `{book: Book}` | `book` |

- `total`, when provided, enables "showing x of y" output; when absent the
  host prints "n shown".
- Empty `query` or missing `category` is a host-side usage error (exit 1)
  before spawning.
- Module-side failures use JSON-RPC implementation-defined codes
  `-32000..-32099` with a human-readable `message` (e.g. network failure,
  upstream API error, parse failure of upstream data). `-32601` (method not
  found) is returned by modules for methods not in their capabilities.

## Data model

### Book (normalized contract)

All fields except `id`, `title`, `formats` are optional; absent = unknown.

```json
{
  "id": "205",
  "title": "Moby Dick",
  "authors": ["Herman Melville"],
  "languages": ["en"],
  "published": 1851,
  "description": "…",
  "categories": ["fiction"],
  "formats": [
    {"format": "epub", "url": "https://…", "size": 123456},
    {"format": "txt",  "url": "https://…"}
  ]
}
```

- `id`: opaque, module-scoped, stable string.
- `authors`: array of strings.
- `languages`: array of ISO 639-1 codes.
- `published`: integer year.
- `formats`: array of `{format, url, size?}`; `format` is a normalized
  lowercase tag, `url` is `http(s)://` only, `size` is bytes.

### Format tags

Normalized tags the host understands for display and filename extension:
`epub`, `azw3`, `mobi`, `kepub`, `txt`, `html`. Each module maps its
library's MIME types / content types onto these (unknown MIME types map to a
lowercased tag derived from the file extension). The host never interprets
MIME types itself.

### Category

`{id: string, title: string}` — `id` is opaque (e.g. an OPDS feed href), used
as the `category` param to `list`.

## CLI surface

```
library-cli modules                        list installed modules + capabilities
library-cli install <module-dir>           copy module into discovery path
library-cli search <query> [--module m] [--limit N] [--json]
library-cli categories [--module m]
library-cli list --category C [--module m] [--limit N]
library-cli show <book-id> [--module m]
library-cli download <book-id> --format F [--module m] [-o DIR] [--force]
```

- Module resolution order: `--module` flag → `default_module` in config →
  error listing installed modules (exit 1). An unknown `--module` name is a
  usage error (exit 1).
- Output: aligned table (title, author, id, formats) by default; `--json`
  emits the normalized Book records for scripting.
- `download`: resolves the format's URL from the displayed/specified Book,
  fetches with a progress bar, 60 s overall timeout, single retry on
  transient network failure (HTTP 5xx, connection reset), writes to
  `-o`/`download_dir` (default `.`).
  Filename: `Title - Author.ext` (or `Title.ext` when the book has no
  authors), where `Author` is `authors` joined with `, ` and `.ext` derives
  from the format tag; sanitized (characters illegal on common filesystems
  and path separators replaced with `-`, runs of whitespace collapsed,
  trimmed). Refuses to overwrite an existing file unless `--force`.
- `install`: copies the module dir into the discovery path as
  `<name>/` (the manifest's `name`); refuses to overwrite an existing module
  unless `--force`.

## Config

`~/.config/library-cli/config.toml` (optional; TOML):

```toml
default_module = "gutenberg"
download_dir = "~/Downloads"
```

- `download_dir` supports `~` expansion. Missing/invalid config → exit 4 with
  a message; never a silent fallback for an explicitly provided file, but an
  absent file is fine (defaults apply).

## Errors & exit codes

| Code | Meaning |
|---|---|
| 0 | success |
| 1 | usage error (bad args, unknown module, empty query) |
| 2 | module/protocol error (spawn failure, timeout, crash, JSON-RPC error) |
| 3 | network/IO (host-side download/network failure, filesystem write errors) |
| 4 | config/discovery error (invalid manifest, invalid config) |

- Module errors are reported as `module <name>: <message>`.
- Internal errors surface as `error: <context>: <cause>`; no panics in
  normal operation.

## Testing strategy

- **Rust unit tests**: manifest parsing + validation, NDJSON framing
  encode/decode, JSON-RPC request/response handling, Book serde, filename
  sanitization, format-tag → extension mapping, exit-code mapping, config
  parsing.
- **Fixture-based integration tests (default CI)**: each module supports a
  fixture mode via `LIBRARY_CLI_FIXTURE=<fixture-dir>` env var — when set, the
  module answers from recorded upstream responses (scenario files keyed by
  method + params, e.g. `search-dune.json`) instead of the network. Tests
  spawn the real module binaries and drive them through the real host code,
  achieving full-contract coverage with no network. When a fixture dir is
  set, modules MUST NOT touch the network.
- **Download integration**: tiny in-process `TcpListener` serving a small
  file; exercises downloader end-to-end without external network.
- **Live-network tests**: exist but `#[ignore]`-gated (manual opt-in), marked
  `live`; verify real gutendex/OPDS endpoints and mappings.
- **Module self-tests**: each reference module documents the one-liner
  manual check (`echo '…request…' | python3 module.py`).

## Project layout

```
Cargo.toml
src/
  main.rs             entry, exit-code mapping
  cli.rs              clap definitions
  config.rs           config.toml load/validate
  discovery.rs        module scan, manifest load/validate
  module/
    mod.rs            module host: spawn, one-shot exchange, timeout
    jsonrpc.rs        JSON-RPC request/response types + framing
  book.rs             Book/Category types, format tags
  download.rs         URL fetch, progress, filename sanitization, overwrite
  output.rs           table / --json rendering
  errors.rs           error types → exit codes
modules/
  gutenberg/          manifest.toml, module.py, fixtures/
  standard-ebooks/    manifest.toml, module.py, fixtures/
tests/
  integration.rs      spawns real modules (fixture mode) + download TcpListener
docs/superpowers/specs/2026-08-15-library-cli-design.md
```

## Reference modules

Both stdlib-only Python 3 (`urllib.request`, `json`, `xml.etree.ElementTree`,
`html.parser` as needed).

### Gutenberg (`search`, `book`)

- `search`: `GET https://gutendex.com/books/?search=<q>&page=<n>` → gutendex
  `results` → Books. `id` = gutendex numeric id as string. `title`/`authors`
  direct. Format mapping from gutendex `formats` MIME keys:
  `text/plain; charset=us-ascii` → `txt`, `application/epub+zip` → `epub`,
  `application/x-mobipocket-ebook` → `mobi`, `text/html` → `html`.
  `total` from `count`.
- `book`: `GET https://gutendex.com/books/<id>` → single result → Book.
- No `categories`/`list` capability.

### Standard Ebooks (`search`, `categories`, `list`, `book`)

- OPDS root feed: `https://standardebooks.org/feeds/opds/all`.
- `categories`: entry navigation links (title + href).
- `list {category}`: fetch the acquisition feed href, parse `entry` elements
  → Books. Format mapping from entry `link` rel=acquisition content types:
  `application/epub+zip` → `epub`, `application/vnd.amazon.ebook` → `azw3`,
  `application/x-kepub+zip` → `kepub`.
- `search`: OPDS search template link with `{searchTerms}` substituted.
- `book {id}`: refetch the URL the id encodes (id = entry permalink).

> API details (exact feed shapes, MIME strings) are verified against the live
> endpoints during implementation; the protocol contract above is the source
> of truth regardless of upstream details.