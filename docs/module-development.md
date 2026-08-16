# Developing library modules for dewey

This guide explains how to add a new ebook library to dewey. A *module* is a
standalone program that translates a library's API (or website) into dewey's
protocol. Modules can be written in any language that can read a line from
stdin and write JSON to stdout — the bundled modules are Python 3
stdlib-only, which keeps them dependency-free, but Lua, Node, shell, or
anything else works the same way.

Working references live in this repository: `modules/gutenberg` (JSON API,
`search` + `book`) and `modules/standard-ebooks` (OPDS Atom with an HTML
fallback, all four methods).

## Table of contents

1. [How dewey works](#how-dewey-works)
2. [Module layout](#module-layout)
3. [The manifest](#the-manifest)
4. [The protocol](#the-protocol)
5. [Methods](#methods)
6. [The Book record](#the-book-record)
7. [Format tags](#format-tags)
8. [Errors](#errors)
9. [Fixture mode (offline development)](#fixture-mode)
10. [Testing your module](#testing-your-module)
11. [Walkthrough: a module from scratch](#walkthrough)
12. [Gotchas](#gotchas)

## How dewey works

When you run a dewey command, the host:

1. resolves which module to use (`--module`, then config, then an error),
2. spawns the module's `command` as a child process,
3. writes exactly **one** JSON-RPC request line to the module's stdin,
4. reads exactly **one** response line from the module's stdout,
5. waits for the module to exit.

That is the whole lifecycle: one spawn, one request, one response, exit. No
session state, no keep-alive — which is what makes modules trivial to write
and trivial to test by hand. Each verb is a separate process invocation.

## Module layout

A module is a directory (anywhere on disk; install it with `dewey install`)
containing a manifest and the module's code:

```
open-shelf/
  manifest.toml
  module.py          # or module.lua, main.js, ...
  fixtures/          # optional: recorded responses for offline testing
```

## The manifest

`manifest.toml` declares the module to dewey. All fields are TOML.

| Field | Required | Rules |
|---|---|---|
| `name` | yes | `^[a-z0-9-]+$` and must equal the directory name |
| `version` | yes | non-empty string |
| `description` | optional | one-line human-readable description |
| `command` | yes | non-empty array of strings (argv). The first element is a PATH lookup, *or* a file relative to the module dir if one exists there. Remaining arguments resolve relative to the module dir. |
| `capabilities` | yes | non-empty subset of `["search", "categories", "list", "book"]` |

`capabilities` is important: dewey only surfaces verbs backed by a declared
capability, and it refuses (exit 1) to call a method you didn't declare. Keep
it honest.

Example (`modules/gutenberg/manifest.toml`):

```toml
name = "gutenberg"
version = "0.1.0"
description = "Project Gutenberg via gutendex"
command = ["python3", "module.py"]
capabilities = ["search", "book"]
```

## The protocol

dewey speaks **JSON-RPC 2.0** over **newline-delimited JSON** (NDJSON):

- Exactly one compact JSON object per line, LF-terminated, UTF-8. No
  pretty-printing.
- The host sends exactly one **request**; the module responds with exactly
  one **response**; then the module exits `0`.
- **Nothing** other than protocol lines may be written to stdout. Logs and
  diagnostics go to stderr.
- The host enforces a **30-second timeout** per exchange. Respond or die.
- Unknown/extra JSON fields are ignored (forward compatibility).

Request (host → module, on stdin):

```json
{"jsonrpc":"2.0","id":1,"method":"search","params":{"query":"moby dick","page":1,"limit":20}}
```

Response (module → host, on stdout) — exactly one of:

```json
{"jsonrpc":"2.0","id":1,"result":{"books":[]}}
{"jsonrpc":"2.0","id":1,"error":{"code":-32000,"message":"network: timeout"}}
```

`id` is an integer set by the host; echo it back unchanged.

## Methods

| Method | Params | Result | Capability |
|---|---|---|---|
| `search` | `query` (required), `page` (1-indexed, default 1), `limit` (default 20, max 200) | `{"books": [Book], "total"?: int}` | `search` |
| `categories` | — | `{"categories": [{"id", "title"}]}` | `categories` |
| `list` | `category` (required), `page`, `limit` | `{"books": [Book], "total"?: int}` | `list` |
| `book` | `id` (required) | `{"book": Book}` | `book` |

Notes:

- `total`, when provided, enables "showing x of y" output; when absent dewey
  prints "n shown". Both are fine.
- `search` and `list` return the same shape; `book` returns a single Book.
  Search results may carry the full Book record (formats included) — dewey
  downloads straight from the URLs in a result, so prefer returning complete
  records.
- `categories` feeds `list`: the `id` you return here is passed back to your
  `list` method as `category`. Use the URL or feed href of the collection.
- Return `-32601` ("method not found") for anything outside your
  capabilities — though the host should refuse to call it before you ever
  see the request.

## The Book record

Every method returns books in this normalized shape — the contract all
libraries are translated onto:

| Field | Required | Type / notes |
|---|---|---|
| `id` | yes | opaque, stable, module-scoped string (e.g. the numeric id or permalink) |
| `title` | yes | string |
| `formats` | yes | array of `{format, url, size?}` — may be empty |
| `authors` | no | array of strings |
| `languages` | no | array of ISO 639-1 codes |
| `published` | no | integer year |
| `description` | no | string |
| `categories` | no | array of strings (tags/subjects) |

Example:

```json
{
  "id": "2701",
  "title": "Moby Dick; Or, The Whale",
  "authors": ["Melville, Herman"],
  "languages": ["en"],
  "published": 1851,
  "description": "…",
  "categories": ["Whaling -- Fiction"],
  "formats": [
    {"format": "epub", "url": "https://www.gutenberg.org/ebooks/2701.epub.noimages", "size": 1500000},
    {"format": "txt",  "url": "https://www.gutenberg.org/ebooks/2701.txt.utf-8"}
  ]
}
```

Download URLs must be `http(s)`. The host fetches them with progress bars,
retries, and its own naming rules — your module never downloads anything.

## Format tags

`format` must be one of the canonical tags dewey understands (it drives
display and the filename extension):

```
epub  azw3  mobi  kepub  txt  html
```

Map your library's MIME types / content types onto these. Anything else is
treated as a literal extension — avoid it.

## Errors

Module-side failures must look like this — **always**:

```json
{"jsonrpc":"2.0","id":1,"error":{"code":-32000,"message":"gutendex http 503: https://…"}}
```

Rules:

- Use codes `-32000` … `-32099` (implementation-defined) with a human-readable
  `message`. Reserve `-32601` for unknown methods, `-32602` for invalid
  params.
- **Never** print a Python traceback (or equivalent) and exit non-zero.
  Catch everything: network errors *while reading the body*, XML/JSON parse
  failures, missing fixtures. A module that crashes violates the protocol and
  shows Python internals to the user.
- After emitting an error line, exit `0`. Yes — exit 0: the error is
  *delivered in the response*, not in the exit status.

The host maps module errors to exit code 2 (`error: module <name>: <message>`).

## Fixture mode

dewey's test suite (and your own development) can run modules fully offline.
When the environment variable `DEWEY_FIXTURE=<dir>` is set, your module
**must** answer from recorded data and **must not** touch the network.

Fixture filename scheme:

- No-params methods: `<method>.json` — e.g. `categories.json`
- Param-keyed methods: `<method>-<slug>.json` where the slug is the key
  lowercased with runs of non-alphanumerics replaced by `-`; URLs are keyed
  by their last path segment. E.g. `search` for "moby dick" →
  `search-moby-dick.json`; `book` for
  `https://example.org/books/frankenstein` → `book-frankenstein.json`.

If the requested fixture file is missing, respond with a `-32000` error
("fixture not found: …") — never crash.

## Testing your module

Test it standalone — no dewey needed:

```bash
printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"search","params":{"query":"moby dick","limit":5}}' \
  | DEWEY_FIXTURE=fixtures python3 module.py
```

Expected: exactly one line of JSON with a `result`.

Then through dewey:

```bash
dewey install ./open-shelf
dewey search "test" --module open-shelf
dewey show <id> --module open-shelf
```

The dewey test suite uses the same trick: committed fixtures plus the real
host binary. If you write your module in the same repo, follow the pattern in
`tests/integration.rs` (set `DEWEY_MODULES` and `DEWEY_FIXTURE` on the spawned
binary) and your module's contract is covered end to end, offline.

## Walkthrough: a module from scratch

Let's write a module for a fictional library, "Open Shelf", which exposes a
JSON search API (`https://api.open-shelf.example/search?q=…`) and a book
endpoint (`https://api.open-shelf.example/books/<id>`), serving `epub` and
`txt` files.

`open-shelf/manifest.toml`:

```toml
name = "open-shelf"
version = "0.1.0"
description = "Open Shelf example library"
command = ["python3", "module.py"]
capabilities = ["search", "book"]
```

`open-shelf/module.py` — a complete, copyable module:

```python
#!/usr/bin/env python3
"""Open Shelf module for dewey.

Speaks JSON-RPC 2.0 (one JSON object per line) over stdio.
When DEWEY_FIXTURE is set, answers from recorded files
(<fixture>/<method>-<slug>.json) instead of the network.
"""
import json
import os
import re
import sys
import urllib.error
import urllib.parse
import urllib.request

API = "https://api.open-shelf.example"

# Map the library's MIME types onto dewey's canonical format tags.
MIME_TO_FORMAT = {
    "application/epub+zip": "epub",
    "text/plain": "txt",
}

USER_AGENT = "dewey/0.1 (open-shelf module)"


def slug(s):
    """Fixture slug: lowercase, non-alphanumerics become '-', strip ends."""
    return re.sub(r"[^a-z0-9]+", "-", s.lower()).strip("-")


def key_for(s):
    """Fixture key for URLs: use the last path segment."""
    return s.rsplit("/", 1)[-1]


def fixture_path(method, key):
    root = os.environ.get("DEWEY_FIXTURE")
    if not root:
        return None
    k = slug(key) if key else ""
    return os.path.join(root, f"{method}{'-' + k if k else ''}.json")


def http_json(url):
    req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            return json.loads(resp.read().decode("utf-8"))
    except (urllib.error.HTTPError, urllib.error.URLError, OSError) as e:
        # Every failure becomes a -32000 module error, never a traceback.
        raise RuntimeError(f"open-shelf network error: {e}") from e


def load(method, key, url):
    """Fixture mode first; the network only when DEWEY_FIXTURE is unset."""
    path = fixture_path(method, key)
    if path:
        if not os.path.isfile(path):
            raise RuntimeError(f"fixture not found: {path}")
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    return http_json(url)


def to_book(item):
    formats = []
    for mime, url in (item.get("formats") or {}).items():
        tag = MIME_TO_FORMAT.get(mime)
        if tag:
            formats.append({"format": tag, "url": url})
    return {
        "id": str(item["id"]),
        "title": item.get("title", ""),
        "authors": item.get("authors", []),
        "languages": item.get("languages", []),
        "published": item.get("published"),
        "description": item.get("description"),
        "categories": item.get("subjects", []),
        "formats": formats,
    }


def search(params):
    query = params.get("query", "")
    if not query:
        raise RuntimeError("missing query param")
    limit = params.get("limit", 20)
    page = params.get("page", 1)
    url = f"{API}/search?q={urllib.parse.quote(query)}&page={page}"
    data = load("search", query, url)
    books = [to_book(i) for i in data.get("results", [])][:limit]
    return {"books": books, "total": data.get("count")}


def book(params):
    bid = params.get("id", "")
    if not bid:
        raise RuntimeError("missing id param")
    data = load("book", key_for(bid), f"{API}/books/{urllib.parse.quote(bid)}")
    return {"book": to_book(data)}


def main():
    line = sys.stdin.readline()
    if not line:
        sys.stderr.write("open-shelf: no request on stdin\n")
        sys.exit(1)
    try:
        req = json.loads(line)
        method = req["method"]
        params = req.get("params") or {}
        if method == "search":
            result = search(params)
        elif method == "book":
            result = book(params)
        else:
            sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": req["id"],
                "error": {"code": -32601, "message": f"method not found: {method}"}}) + "\n")
            return
        sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": req["id"], "result": result}) + "\n")
    except RuntimeError as e:
        sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": req.get("id", 0),
            "error": {"code": -32000, "message": str(e)}}) + "\n")
        sys.exit(0)
    except (json.JSONDecodeError, KeyError, TypeError, ValueError) as e:
        sys.stderr.write(f"open-shelf: bad request: {e}\n")
        sys.exit(1)


if __name__ == "__main__":
    main()
```

That's the whole contract: one request in, one response out, exit 0, no
tracebacks, fixture support. Record a sample of your library's real API
response into `open-shelf/fixtures/search-<query>.json` and
`open-shelf/fixtures/book-<id>.json`, and the module is testable offline
forever.

## Gotchas

- **Respect `limit`.** Responses are bounded; use `page` for more results.
- **Stable ids.** A `show`/`download` may happen long after a search — the
  id you return must still resolve via `book`.
- **http(s) URLs only.** The host refuses everything else.
- **Log to stderr.** Any stray stdout byte corrupts the protocol.
- **Catch body-read errors.** `urllib` only wraps connection/header errors;
  `ConnectionResetError`, `socket.timeout`, and `http.client.IncompleteRead`
  surface separately — catch `(OSError, http.client.HTTPException)` too.
- **Fixtures never hit the network.** If `DEWEY_FIXTURE` is set, your module
  must be able to answer entirely from disk.
- **Test by hand first.** The one-liner in [Testing your module](#testing-your-module)
  catches 90% of bugs before dewey is involved.
