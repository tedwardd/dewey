#!/usr/bin/env python3
"""Standard Ebooks module for library-cli.

Speaks JSON-RPC 2.0 (one JSON object per line) over stdio.
When LIBRARY_CLI_FIXTURE is set, answers from recorded files
(<fixture>/<method>-<slug>.json) instead of the network.
"""
import html.parser
import json
import os
import re
import sys
import urllib.error
import urllib.parse
import urllib.request
import xml.etree.ElementTree as ET

ATOM = "{http://www.w3.org/2005/Atom}"
DC = "{http://purl.org/dc/elements/1.1/}"
DCTERMS = "{http://purl.org/dc/terms/}"
ROOT = "https://standardebooks.org/feeds/opds"
# Live feeds (2026): the OPDS search endpoint lives on the root URL with
# OpenSearch parameters; category feeds require Patrons Circle auth.
SEARCH_TEMPLATE = ROOT + "/all?query={searchTerms}&per-page={perPage}&page=1"

MIME_TO_FORMAT = {
    "application/epub+zip": "epub",
    "application/x-mobipocket-ebook": "azw3",
    "application/kepub+zip": "kepub",
    # Legacy types seen in older feeds.
    "application/vnd.amazon.ebook": "azw3",
    "application/x-kepub+zip": "kepub",
}

# Book pages advertise downloads via <a property="schema:contentUrl"
# class="epub|amazon|kobo"> anchors rather than <link> tags.
CLASS_TO_FORMAT = {
    "epub": "epub",
    "amazon": "azw3",
    "kobo": "kepub",
}


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


def http_bytes(url, accept=None):
    req = urllib.request.Request(url, headers={"Accept": accept} if accept else {})
    with urllib.request.urlopen(req, timeout=30) as resp:
        return resp.read()


def load_xml(method, key, url):
    path = fixture_path(method, key)
    if path:
        if not os.path.isfile(path):
            raise RuntimeError(f"fixture not found: {path}")
        with open(path, "rb") as f:
            return ET.fromstring(f.read())
    try:
        return ET.fromstring(http_bytes(url, "application/atom+xml"))
    except urllib.error.HTTPError as e:
        raise RuntimeError(f"standard-ebooks http {e.code}: {url}") from e
    except urllib.error.URLError as e:
        raise RuntimeError(f"standard-ebooks network error: {e.reason}") from e


def text_of(parent, tag):
    el = parent.find(tag)
    return el.text if el is not None and el.text else ""


def dc_text(parent, name):
    """Dublin Core text, tolerating both dc: (elements) and dcterms: prefixes."""
    for ns in (DC, DCTERMS):
        el = parent.find(ns + name)
        if el is not None and el.text:
            return el.text
    return ""


def to_book(entry):
    formats = []
    for link in entry.findall(ATOM + "link"):
        if "acquisition" not in link.get("rel", ""):
            continue
        tag = MIME_TO_FORMAT.get(link.get("type", ""))
        if tag:
            formats.append({"format": tag, "url": link.get("href", "")})
    issued = dc_text(entry, "issued")
    published = int(issued[:4]) if issued[:4].isdigit() else None
    lang = dc_text(entry, "language")
    return {
        "id": text_of(entry, ATOM + "id"),
        "title": text_of(entry, ATOM + "title"),
        "authors": [a.text for a in entry.findall(ATOM + "author/" + ATOM + "name") if a.text],
        "languages": [lang] if lang else [],
        "published": published,
        "description": dc_text(entry, "description") or text_of(entry, ATOM + "summary") or None,
        "categories": [c.get("term") for c in entry.findall(ATOM + "category") if c.get("term")],
        "formats": formats,
    }


def categories():
    feed = load_xml("categories", "", ROOT + "/all")
    cats = []
    for entry in feed.findall(ATOM + "entry"):
        title = text_of(entry, ATOM + "title")
        for link in entry.findall(ATOM + "link"):
            if link.get("rel") == "subsection":
                cats.append({"id": link.get("href", ""), "title": title})
    return {"categories": cats}


def list_books(params):
    category = params.get("category", "")
    if not category:
        raise RuntimeError("missing category param")
    limit = params.get("limit", 20)
    feed = load_xml("list", key_for(category), category)
    books = [to_book(e) for e in feed.findall(ATOM + "entry")][:limit]
    return {"books": books}


def search(params):
    query = params.get("query", "")
    if not query:
        raise RuntimeError("missing query param")
    limit = params.get("limit", 20)
    url = SEARCH_TEMPLATE.replace("{searchTerms}", urllib.parse.quote(query)).replace(
        "{perPage}", str(limit)
    )
    feed = load_xml("search", query, url)
    books = [to_book(e) for e in feed.findall(ATOM + "entry")][:limit]
    return {"books": books}


class _LinkParser(html.parser.HTMLParser):
    def __init__(self):
        super().__init__()
        self.links = []

    def handle_starttag(self, tag, attrs):
        d = dict(attrs)
        if tag == "link" and d.get("rel") == "alternate" and d.get("type") in MIME_TO_FORMAT:
            self.links.append((d["type"], d.get("href", "")))
        elif tag == "a" and d.get("property") == "schema:contentUrl":
            cls = d.get("class", "")
            if cls in CLASS_TO_FORMAT:
                self.links.append((cls, d.get("href", "")))


def book(params):
    bid = params.get("id", "")
    if not (bid.startswith("http://") or bid.startswith("https://")):
        raise RuntimeError(f"invalid book id: {bid}")
    path = fixture_path("book", key_for(bid))
    if path:
        with open(path, "rb") as f:
            raw = f.read()
    else:
        try:
            raw = http_bytes(bid, "application/atom+xml")
        except urllib.error.HTTPError as e:
            raise RuntimeError(f"standard-ebooks http {e.code}: {bid}") from e
        except urllib.error.URLError as e:
            raise RuntimeError(f"standard-ebooks network error: {e.reason}") from e
    if b"<entry" in raw:
        feed = ET.fromstring(raw)
        entries = feed.findall(ATOM + "entry")
        if entries:
            return {"book": to_book(entries[0])}
    parser = _LinkParser()
    parser.feed(raw.decode("utf-8", errors="replace"))
    formats = []
    for tag, href in parser.links:
        fmt = MIME_TO_FORMAT.get(tag) or CLASS_TO_FORMAT.get(tag)
        if fmt:
            formats.append({"format": fmt, "url": urllib.parse.urljoin(bid, href)})
    return {"book": {"id": bid, "title": key_for(bid), "authors": [], "formats": formats}}


def main():
    line = sys.stdin.readline()
    if not line:
        sys.stderr.write("standard-ebooks: no request on stdin\n")
        sys.exit(1)
    try:
        req = json.loads(line)
        method = req["method"]
        params = req.get("params") or {}
        if method == "search":
            result = search(params)
        elif method == "categories":
            result = categories()
        elif method == "list":
            result = list_books(params)
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
        sys.stderr.write(f"standard-ebooks: bad request: {e}\n")
        sys.exit(1)


if __name__ == "__main__":
    main()
