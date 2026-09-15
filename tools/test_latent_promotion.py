#!/usr/bin/env python3
"""The latent server promotes ITSELF to the full design surface — on any client.

FIELD REPORT, Alex on Grok Build, 2026-09-14: `reflow2_start_design` created
an empty `.reflow2/` and its `next_step` said "run /mcp" — a Claude Code
command. Grok Build has `/mcps` and no reconnect. The latent two-tool stdio
process was never replaced, `.reflow2/graph` was never created, genesis could
not finish, and report-friction could not run either. His client DID re-query
the tool list after "continue"; the server still offered two tools.

The design's own rule (rule:mcp-and-the-graph-are-the-only-common-ground):
reflow2 targets ANY MCP client, and the only assumption is that it can call an
MCP server. Completing opt-in must therefore need nothing outside MCP. MCP has
the mechanism: `notifications/tools/list_changed`. So the process that opted
the directory in opens the store itself, serves the full surface from then on,
and tells the client the list changed. No client command is named as the way
to finish; a restart is only the fallback if a client ignores the notification.

Runs the REAL binary over stdio, exactly as a client does. Skipped when no
binary is built (CI builds one first).
"""
from __future__ import annotations

import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import unittest

REPO = pathlib.Path(__file__).resolve().parent.parent


def find_binary() -> str | None:
    env = os.environ.get("REFLOW2_MCP_BIN")
    if env and pathlib.Path(env).exists():
        return env
    for c in (REPO / "target/debug/reflow2-mcp", REPO / "target/release/reflow2-mcp"):
        if c.exists():
            return str(c)
    return shutil.which("reflow2-mcp")


class Client:
    """A minimal stdio JSON-RPC client that keeps the notifications it sees."""

    def __init__(self, binary: str, cwd: pathlib.Path, graph_path: str):
        self.proc = subprocess.Popen(
            [binary, "--graph-path", graph_path, "--only-if-present"],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            text=True, bufsize=1, cwd=cwd, env={**os.environ, "RUST_LOG": "warn"},
        )
        self.notifications: list[dict] = []
        self._id = 0
        self.rpc("initialize", {
            "protocolVersion": "2025-11-25", "capabilities": {},
            "clientInfo": {"name": "test-any-client", "version": "0"},
        })
        self.notify("notifications/initialized", {})

    def notify(self, method: str, params: dict) -> None:
        self.proc.stdin.write(json.dumps({"jsonrpc": "2.0", "method": method, "params": params}) + "\n")
        self.proc.stdin.flush()

    def rpc(self, method: str, params: dict | None = None) -> dict:
        self._id += 1
        msg = {"jsonrpc": "2.0", "id": self._id, "method": method}
        if params is not None:
            msg["params"] = params
        self.proc.stdin.write(json.dumps(msg) + "\n")
        self.proc.stdin.flush()
        while True:
            line = self.proc.stdout.readline()
            if not line:
                raise SystemExit(f"server closed stdout.\nstderr:\n{self.proc.stderr.read()}")
            reply = json.loads(line)
            if "id" not in reply:            # a server notification, not our reply
                self.notifications.append(reply)
                continue
            return reply

    def tools(self) -> list[str]:
        r = self.rpc("tools/list", {})
        return sorted(t["name"] for t in r["result"]["tools"])

    def call(self, name: str, args: dict | None = None) -> dict:
        r = self.rpc("tools/call", {"name": name, "arguments": args or {}})
        if "error" in r:
            raise AssertionError(f"{name}: {r['error']}")
        res = r["result"]
        if res.get("isError"):
            raise AssertionError(f"{name}: tool error: {res.get('content')}")
        return res.get("structuredContent") or json.loads(res["content"][0]["text"])

    def close(self) -> None:
        try:
            self.proc.stdin.close()
            self.proc.wait(timeout=10)
        except Exception:
            self.proc.kill()


@unittest.skipUnless(find_binary(), "no reflow2-mcp binary built")
class LatentPromotion(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory(prefix="latent-promo-")
        self.dir = pathlib.Path(self._tmp.name)
        self.client = Client(find_binary(), self.dir, ".reflow2/graph")

    def tearDown(self):
        self.client.close()
        self._tmp.cleanup()

    def test_before_opt_in_only_the_latent_tools_are_served(self):
        names = self.client.tools()
        self.assertIn("reflow2_start_design", names)
        self.assertNotIn("add_project", names, "the full surface must not be served before opt-in")

    def test_start_design_promotes_this_same_process_and_says_so(self):
        out = self.client.call("reflow2_start_design")
        self.assertTrue(out["started"], out)
        step = out["next_step"]
        # The instruction must be followable on ANY MCP client: it must not
        # send the user to a command only one client has.
        self.assertNotIn("run /mcp", step, step)
        self.assertIn("served", step.lower(), step)
        # The store exists now — this process opened it.
        self.assertTrue((self.dir / ".reflow2" / "graph").exists(), "the store was not created")
        # The client was told the tool list changed, the MCP way.
        methods = [n.get("method") for n in self.client.notifications]
        self.assertIn("notifications/tools/list_changed", methods, methods)
        # And the same connection now serves the full surface.
        names = self.client.tools()
        self.assertIn("add_project", names)
        self.assertIn("get_skill", names)
        skill = self.client.call("get_skill", {"name": "genesis"})
        self.assertIn("body", skill)

    def test_calling_start_design_again_says_it_is_served_not_run_mcp(self):
        self.client.call("reflow2_start_design")
        again = self.client.call("reflow2_start_design")
        self.assertFalse(again["started"], again)
        self.assertNotIn("run /mcp", again["next_step"], again)
        self.assertIn("served", again["next_step"].lower(), again)

    def test_a_design_that_appears_underneath_is_served_on_the_next_call(self):
        # The 2026-08-15 case: a restore (`--import`) built a store under a
        # running latent server, which went on serving one tool. Now it
        # re-probes on every call and promotes in place.
        (self.dir / ".reflow2" / "graph").mkdir(parents=True)
        names = self.client.tools()
        self.assertIn("add_project", names, "a design that appeared underneath must be served")


if __name__ == "__main__":
    unittest.main(verbosity=1)
