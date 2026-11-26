#!/usr/bin/env python3
"""A stand-in for Cua Driver's `cua-driver mcp` (wisp.computer's tests): newline-delimited JSON-RPC on stdio,
four tools (two that look, one that acts, one wisp never offers), every call appended to $FAKE_CUA_LOG."""
import json
import os
import sys

TOOLS = [
    {"name": "get_screen_size", "description": "screen", "inputSchema": {"type": "object"}, "annotations": {"readOnlyHint": True}},
    {"name": "get_desktop_state", "description": "shot", "inputSchema": {"type": "object"}, "annotations": {"readOnlyHint": True}},
    {"name": "click", "description": "click", "inputSchema": {"type": "object"}, "annotations": {"readOnlyHint": False}},
    {"name": "set_config", "description": "cfg", "inputSchema": {"type": "object"}, "annotations": {"readOnlyHint": False}},
]
# a 1x1 PNG
PNG = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg=="


def send(o):
    sys.stdout.write(json.dumps(o) + "\n")
    sys.stdout.flush()


for line in sys.stdin:
    try:
        m = json.loads(line)
    except ValueError:
        continue
    if sys.argv[1:] != ["mcp"]:
        sys.exit(2)
    method, mid = m.get("method"), m.get("id")
    if mid is None:
        continue
    if method == "initialize":
        # the driver asks wisp something first, as a real one may: wisp must answer and go on
        send({"jsonrpc": "2.0", "id": "srv-1", "method": "ping"})
        send({"jsonrpc": "2.0", "id": mid, "result": {"protocolVersion": "2025-06-18", "capabilities": {"tools": {}}, "serverInfo": {"name": "cua-driver", "version": "0.62.1"}, "instructions": "FAKE-DRIVER-INSTRUCTIONS"}})
    elif method == "tools/list":
        send({"jsonrpc": "2.0", "id": mid, "result": {"tools": TOOLS}})
    elif method == "tools/call":
        p = m.get("params") or {}
        with open(os.environ.get("FAKE_CUA_LOG", os.devnull), "a") as f:
            f.write(json.dumps({"name": p.get("name"), "args": p.get("arguments"), "telemetry": os.environ.get("CUA_DRIVER_RS_TELEMETRY_ENABLED"), "update": os.environ.get("CUA_DRIVER_RS_UPDATE_CHECK")}) + "\n")
        if p.get("name") == "get_desktop_state":
            content = [{"type": "text", "text": "Desktop screenshot 1x1"}, {"type": "image", "data": PNG, "mimeType": "image/png"}]
        else:
            content = [{"type": "text", "text": "did " + str(p.get("name"))}]
        send({"jsonrpc": "2.0", "id": mid, "result": {"content": content}})
    elif mid is not None and "result" not in m:
        send({"jsonrpc": "2.0", "id": mid, "error": {"code": -32601, "message": "no"}})
