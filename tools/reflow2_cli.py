#!/usr/bin/env python3
"""One-shot command-line access to a reflow2 design graph.

Calls a single reflow2-mcp tool and prints the JSON result. Each invocation
opens the graph, makes the call, and closes it — so it composes freely in shell
scripts and needs no long-running server. That also sidesteps RocksDB's
single-writer lock: nothing holds the graph between calls.

It is the same tool surface an MCP-connected agent sees; this is just a second
door onto it, for shells, scripts, and agents without an MCP connection.

    tools/reflow2_cli.py --list
    tools/reflow2_cli.py --describe add_interface
    tools/reflow2_cli.py detect_gaps
    tools/reflow2_cli.py add_requirement '{"id":"req:x","name":"X","statement":"..."}'

Options:
    --graph PATH   graph directory (default: ./.reflow2/graph, or $REFLOW2_GRAPH)
    --bin PATH     reflow2-mcp binary (default: ./target/debug/reflow2-mcp,
                   or $REFLOW2_BIN)
    --raw          print the raw JSON result with no indentation

Exit codes: 0 ok · 1 tool or usage error · 2 could not start the server.
Standard library only.
"""
from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys

# The binary lives with the reflow2 checkout, so resolve it relative to this
# script rather than the caller's cwd — the CLI is meant to be run from the
# project being designed, which is usually somewhere else entirely.
_REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def _default_bin() -> str:
    if env := os.environ.get("REFLOW2_BIN"):
        return env
    for build in ("release", "debug"):
        candidate = os.path.join(_REPO_ROOT, "target", build, "reflow2-mcp")
        if os.path.exists(candidate):
            return candidate
    return os.path.join(_REPO_ROOT, "target", "debug", "reflow2-mcp")


DEFAULT_BIN = _default_bin()
# The graph belongs to the project being designed, so this one *is* cwd-relative.
DEFAULT_GRAPH = os.environ.get("REFLOW2_GRAPH", ".reflow2/graph")


class Server:
    """A short-lived reflow2-mcp process spoken to over stdio JSON-RPC."""

    def __init__(self, binary: str, graph_path: str) -> None:
        try:
            self.proc = subprocess.Popen(
                [binary, "--graph-path", graph_path],
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                bufsize=1,
                env={**os.environ, "RUST_LOG": os.environ.get("RUST_LOG", "warn")},
            )
        except FileNotFoundError:
            die(2, f"binary not found: {binary}\nBuild it:  cargo build -p reflow2-mcp")
        self._id = 0
        self._rpc(
            "initialize",
            {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "reflow2_cli", "version": "0"},
            },
        )
        self._rpc("notifications/initialized", {}, notify=True)

    def _rpc(self, method: str, params=None, notify: bool = False):
        msg = {"jsonrpc": "2.0", "method": method}
        if params is not None:
            msg["params"] = params
        if not notify:
            self._id += 1
            msg["id"] = self._id
        self.proc.stdin.write(json.dumps(msg) + "\n")
        self.proc.stdin.flush()
        if notify:
            return None
        line = self.proc.stdout.readline()
        if not line:
            err = (self.proc.stderr.read() or "").strip()
            # The most common real cause, surfaced plainly rather than as a stack trace.
            if "LOCK" in err or "temporarily unavailable" in err:
                die(
                    2,
                    "the graph is already open in another process.\n"
                    "reflow2 is single-writer: close the other agent or server "
                    "(or use --graph on a different path) and retry.",
                )
            die(2, f"server exited without responding.\n{err}")
        return json.loads(line)

    def tools(self) -> list[dict]:
        return self._rpc("tools/list", {})["result"]["tools"]

    def call(self, tool: str, args: dict):
        resp = self._rpc("tools/call", {"name": tool, "arguments": args})
        if "error" in resp:
            die(1, f"{tool}: {resp['error'].get('message', resp['error'])}")
        result = resp["result"]
        if result.get("isError"):
            blocks = result.get("content") or []
            text = blocks[0].get("text") if blocks else str(result)
            die(1, f"{tool}: {text}")
        if "structuredContent" in result:
            return result["structuredContent"]
        blocks = result.get("content") or []
        return json.loads(blocks[0]["text"]) if blocks else None

    def close(self) -> None:
        try:
            self.proc.stdin.close()
            self.proc.wait(timeout=10)
        except Exception:
            self.proc.kill()


def die(code: int, message: str):
    print(f"error: {message}", file=sys.stderr)
    raise SystemExit(code)


def main() -> int:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("tool", nargs="?", help="tool name (omit with --list)")
    ap.add_argument("args", nargs="?", default="{}", help="arguments as a JSON object")
    ap.add_argument("--graph", default=DEFAULT_GRAPH)
    ap.add_argument("--bin", default=DEFAULT_BIN)
    ap.add_argument("--list", action="store_true", help="list available tools")
    ap.add_argument("--describe", metavar="TOOL", help="show a tool's description and inputs")
    ap.add_argument("--raw", action="store_true", help="unindented JSON output")
    opts = ap.parse_args()

    server = Server(os.path.abspath(opts.bin), opts.graph)
    try:
        if opts.list:
            for t in sorted(server.tools(), key=lambda t: t["name"]):
                summary = (t.get("description") or "").split(".")[0].strip()
                print(f"{t['name']:<28} {summary[:88]}")
            return 0

        if opts.describe:
            match = next((t for t in server.tools() if t["name"] == opts.describe), None)
            if not match:
                die(1, f"no such tool: {opts.describe}  (try --list)")
            print(match["name"])
            print()
            print(match.get("description", "(no description)"))
            props = (match.get("inputSchema") or {}).get("properties", {})
            required = set((match.get("inputSchema") or {}).get("required", []))
            if props:
                print("\narguments:")
                for name, spec in props.items():
                    flag = "required" if name in required else "optional"
                    desc = spec.get("description", "")
                    print(f"  {name:<24} ({flag}) {desc}")
            return 0

        if not opts.tool:
            die(1, "no tool given. Try --list to see what's available.")

        try:
            args = json.loads(opts.args)
        except json.JSONDecodeError as e:
            die(1, f"arguments are not valid JSON: {e}\nGot: {opts.args}")
        if not isinstance(args, dict):
            die(1, "arguments must be a JSON object, e.g. '{\"id\":\"req:x\"}'")

        result = server.call(opts.tool, args)
        print(json.dumps(result) if opts.raw else json.dumps(result, indent=2))
        return 0
    finally:
        server.close()


if __name__ == "__main__":
    sys.exit(main())
