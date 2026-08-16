#!/usr/bin/env python3
import json
import sys

line = sys.stdin.readline()
if not line:
    sys.exit(1)
req = json.loads(line)
method = req["method"]
if method == "search":
    result = {"books": [{"id": "1", "title": "Fake Book", "authors": ["Fake Author"],
                         "formats": [{"format": "epub", "url": "http://127.0.0.1:1/x.epub"}]}]}
elif method == "categories":
    result = {"categories": [{"id": "cat1", "title": "Category One"}]}
elif method == "list":
    result = {"books": [{"id": "2", "title": "Listed Book", "formats": []}]}
elif method == "book":
    result = {"book": {"id": "1", "title": "Fake Book", "authors": ["Fake Author"],
                       "formats": [{"format": "epub", "url": "http://127.0.0.1:1/x.epub"}]}}
else:
    sys.stderr.write("fake: unknown method\n")
    sys.exit(1)
sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": req["id"], "result": result}) + "\n")
