#!/usr/bin/env python3
import json
import sys

req = json.loads(sys.stdin.readline())
sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": req["id"], "result": {"books": []}}) + "\n")
