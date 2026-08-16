#!/usr/bin/env python3
"""Project Gutenberg module for library-cli.

Speaks JSON-RPC 2.0 (one JSON object per line) over stdio.
When LIBRARY_CLI_FIXTURE is set, answers from recorded gutendex JSON files
(<fixture>/search-<slug>.json, <fixture>/book-<id>.json) instead of the network.
"""
import json
import os
import re
import sys
import urllib.error
import urllib.parse
import urllib.request

API = "https://gutendex.com/books"

MIME_PREFIXES = [
    ("text/plain", "txt"),
    ("application/epub+zip", "epub"),
    ("application/x-mobipocket-ebook", "mobi"),
    ("text/html", "html"),
]


def slug(s):
    return re.sub(r"[^a-z0-9]+", "-", s.lower()).strip("-")


def key_for(s):
    return s.rsplit("/", 1)[-1]


def fixture_path(method, key):
    root = os.environ.get("LIBRARY_CLI_FIXTURE")
    if not root:
        return None
    k = slug(key) if key else ""
    return os.path.join(root, f"{method}{'-' + k if k else ''}.json")


def load(method, key, url):
    path = fixture_path(method, key)
    if path:
        if not os.path.isfile(path):
            raise RuntimeError(f"fixture not found: {path}")
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    try:
        with urllib.request.urlopen(url, timeout=30) as resp:
            return json.loads(resp.read().decode("utf-8"))
    except urllib.error.HTTPError as e:
        raise RuntimeError(f"gutendex http {e.code}: {url}") from e
    except urllib.error.URLError as e:
        raise RuntimeError(f"gutendex network error: {e.reason}") from e


def to_book(g):
    formats = []
    for mime, url in (g.get("formats") or {}).items():
        for prefix, tag in MIME_PREFIXES:
            if mime.startswith(prefix):
                formats.append({"format": tag, "url": url})
                break
    return {
        "id": str(g["id"]),
        "title": g.get("title", ""),
        "authors": [a.get("name", "") for a in g.get("authors", []) if a.get("name")],
        "languages": g.get("languages", []),
        "categories": g.get("subjects", []),
        "formats": formats,
    }


def search(params):
    query = params.get("query", "")
    if not query:
        raise RuntimeError("missing query param")
    page = params.get("page", 1)
    limit = params.get("limit", 20)
    url = f"{API}/?search={urllib.parse.quote(query)}&page={page}"
    data = load("search", query, url)
    books = [to_book(g) for g in data.get("results", [])][:limit]
    return {"books": books, "total": data.get("count")}


def book(params):
    bid = params.get("id", "")
    data = load("book", key_for(bid), f"{API}/{urllib.parse.quote(bid)}")
    return {"book": to_book(data)}


def main():
    line = sys.stdin.readline()
    if not line:
        sys.stderr.write("gutenberg: no request on stdin\n")
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
        sys.stderr.write(f"gutenberg: bad request: {e}\n")
        sys.exit(1)


if __name__ == "__main__":
    main()
