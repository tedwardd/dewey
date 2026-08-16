# library-cli

Access open ebook libraries from the command line. The core is a small Rust
binary; each library is a *module* — a standalone program (any language) that
speaks JSON-RPC over stdio and returns normalized book records.

## Build & run

Requires Rust and `python3` (>= 3.8).

```bash
cargo build --release
```

Modules live in `~/.config/library-cli/modules/<name>/` (override with the
`LIBRARY_CLI_MODULES` env var). Install from a directory:

```bash
library-cli install ./modules/gutenberg
library-cli install ./modules/standard-ebooks
```

## Usage

```bash
library-cli modules                       # list installed modules
library-cli search "moby dick" --module gutenberg
library-cli categories --module standard-ebooks
library-cli list --category <feed-url> --module standard-ebooks
library-cli show 2701 --module gutenberg
library-cli download 2701 --format epub --module gutenberg
library-cli download <book-id> --format epub --dir ~/Downloads
```

Default module and download dir go in `~/.config/library-cli/config.toml`:

```toml
default_module = "gutenberg"
download_dir = "~/Downloads"
```

## Module contract (short version)

- Manifest `manifest.toml` in a module dir: `name` (== dir name), `version`,
  `command` (argv; first element resolved relative to the dir if it names a
  file there), `capabilities` (subset of `search`, `categories`, `list`,
  `book`).
- One JSON-RPC 2.0 request object per line on stdin; one response object per
  line on stdout; exit 0. Nothing else on stdout (log to stderr).
- Methods: `search {query, page?, limit?}` → `{books, total?}`;
  `categories` → `{categories: [{id, title}]}`;
  `list {category, page?, limit?}` → `{books, total?}`;
  `book {id}` → `{book}`.
- Book: `{id, title, authors[], languages[], published?, description?,
  categories[], formats: [{format, url, size?}]}` with format tags
  `epub azw3 mobi kepub txt html`.
- Test a module by hand:
  `printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"search","params":{"query":"moby dick"}}' | python3 modules/gutenberg/module.py`
- Fixture mode: set `LIBRARY_CLI_FIXTURE=<dir>` to serve recorded responses
  (`<method>-<slug>.json`) instead of the network.

Exit codes: 0 ok, 1 usage, 2 module/protocol, 3 network/IO, 4 config/discovery.
