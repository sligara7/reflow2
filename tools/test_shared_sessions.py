#!/usr/bin/env python3
"""Several sessions, one server, one design — the point of `--http`.

`req:sessions-share-a-graph` and `req:seat-per-client`. The store is
single-writer *per process*, which is why six sessions cannot each open the same
directory, and why the answer used to be a worktree each. One process holding
the graph with many client sessions attached has exactly one writer, so the
constraint is satisfied rather than worked around.

What this proves against a real server over real HTTP, because every interesting
failure here is in a seam:

  1. Two sessions get distinct MCP sessions from one process.
  2. A write by one is visible to the other immediately — no export, no merge.
  3. Their claims carry DIFFERENT seats. This is the one that would have failed
     silently: seat identity used to be minted per PROCESS, so a shared server
     would have reported every client as the same owner, and `claim_report`
     would have told six sessions they were each other.
  4. Concurrent readers all finish. The graph moved from a mutex to a read/write
     lock for exactly this.

stdlib only; skips cleanly when the binary is absent.
"""

from __future__ import annotations

import json
import os
import pathlib
import shutil
import socket
import subprocess
import tempfile
import threading
import time
import unittest
import urllib.request

REPO = pathlib.Path(__file__).resolve().parent.parent
BINARY = REPO / "target" / "debug" / "reflow2-mcp"


def free_port() -> int:
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


class Client:
    """One MCP session over streamable HTTP. Responses arrive as SSE frames."""

    def __init__(self, url: str, name: str):
        self.url = url
        self.session = None
        self.id = 1
        message, self.session = self._post(
            {
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-06-18",
                    "capabilities": {},
                    "clientInfo": {"name": name, "version": "1"},
                },
            }
        )
        assert message and "result" in message, f"handshake failed: {message}"
        self.instructions = message["result"].get("instructions", "")
        self._post({"jsonrpc": "2.0", "method": "notifications/initialized"})

    def _post(self, payload: dict):
        headers = {
            "Content-Type": "application/json",
            "Accept": "application/json, text/event-stream",
        }
        if self.session:
            headers["Mcp-Session-Id"] = self.session
        req = urllib.request.Request(
            self.url, data=json.dumps(payload).encode(), headers=headers
        )
        with urllib.request.urlopen(req, timeout=60) as response:
            sid = response.headers.get("Mcp-Session-Id")
            raw = response.read().decode()
        message = None
        for line in raw.splitlines():
            if line.startswith("data:") and line[5:].strip():
                try:
                    message = json.loads(line[5:].strip())
                except json.JSONDecodeError:
                    pass
        if message is None and raw.strip():
            try:
                message = json.loads(raw)
            except json.JSONDecodeError:
                pass
        return message, sid

    def call(self, tool: str, /, **args):
        # `tool` is positional-only: several reflow2 tools take a `name`
        # argument of their own, and a plain `name` parameter here collides
        # with them in a way that reads as a mystery TypeError.
        self.id += 1
        message, _ = self._post(
            {
                "jsonrpc": "2.0",
                "id": self.id,
                "method": "tools/call",
                "params": {"name": tool, "arguments": args},
            }
        )
        if message and "error" in message:
            raise AssertionError(f"{tool} failed: {message['error']}")
        return (message or {}).get("result", {}).get("structuredContent")


class SharedSessions(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if not BINARY.exists():
            raise unittest.SkipTest(f"{BINARY} not built (cargo build -p reflow2-mcp)")
        cls.dir = pathlib.Path(tempfile.mkdtemp(prefix="reflow2-shared-"))
        port = free_port()
        cls.url = f"http://127.0.0.1:{port}/"
        cls.server = subprocess.Popen(
            [
                str(BINARY),
                "--graph-path",
                str(cls.dir / ".reflow2" / "graph"),
                "--http",
                f"127.0.0.1:{port}",
            ],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            env={**os.environ, "RUST_LOG": "error"},
        )
        # Wait for the port, rather than sleeping a guessed amount.
        deadline = time.time() + 60
        while time.time() < deadline:
            try:
                with socket.create_connection(("127.0.0.1", port), timeout=1):
                    return
            except OSError:
                if cls.server.poll() is not None:
                    raise AssertionError(f"server exited: {cls.server.stderr.read()}")
                time.sleep(0.2)
        raise AssertionError("server never came up")

    @classmethod
    def tearDownClass(cls):
        cls.server.terminate()
        try:
            cls.server.wait(timeout=10)
        except subprocess.TimeoutExpired:
            cls.server.kill()
        shutil.rmtree(cls.dir, ignore_errors=True)

    def test_two_sessions_share_one_design(self):
        """THE test: no worktree, no export, no merge — one graph, two sessions."""
        a = Client(self.url, "seat-A")
        b = Client(self.url, "seat-B")
        self.assertNotEqual(a.session, b.session, "each client gets its own session")

        a.call("add_project", id="proj:shared", name="Shared")
        a.call(
            "add_requirement",
            id="req:from-a",
            name="From A",
            statement="Session A wrote this.",
        )

        seen = b.call("get_node", node_type="Requirement", id="req:from-a")
        self.assertTrue(seen and seen.get("node"), "B must see A's write immediately")

        b.call(
            "add_requirement",
            id="req:from-b",
            name="From B",
            statement="Session B wrote this.",
        )
        back = a.call("get_node", node_type="Requirement", id="req:from-b")
        self.assertTrue(back and back.get("node"), "and A must see B's")

    def test_each_session_claims_as_itself(self):
        """`req:seat-per-client`. Seats were minted per PROCESS, so a shared
        server would have reported every client as the same owner — the
        mechanism looking like it worked while telling six sessions they are
        each other."""
        a = Client(self.url, "claim-A")
        b = Client(self.url, "claim-B")
        a.call("add_project", id="proj:claims", name="Claims")
        a.call("add_requirement", id="req:ca", name="CA", statement="a")
        a.call("add_requirement", id="req:cb", name="CB", statement="b")
        a.call("add_contributor", id="who:ann", name="Ann", kind="person")
        a.call("add_contributor", id="who:bob", name="Bob", kind="person")

        a.call("claim_region", contributor_id="who:ann", seed_id="req:ca", note="A working")
        b.call("claim_region", contributor_id="who:bob", seed_id="req:cb", note="B working")

        report = a.call("claim_report")
        seats = {c["contributor_id"]: c.get("seat") for c in report["claims"]}
        self.assertEqual(len(seats), 2, seats)
        self.assertEqual(
            len(set(seats.values())),
            2,
            f"two sessions must not report the same seat: {seats}",
        )
        self.assertTrue(
            all(c["liveness"] == "live" for c in report["claims"]),
            "both sessions are connected, so both claims are live",
        )

    def test_concurrent_readers_all_finish(self):
        """The graph moved from a mutex to a read/write lock for this. Not a
        benchmark — a deadlock canary: four sessions orienting at once must all
        come back."""
        warm = Client(self.url, "warm")
        warm.call("add_project", id="proj:reads", name="Reads")
        for i in range(10):
            warm.call("add_requirement", id=f"req:r{i}", name=f"R{i}", statement="x")

        errors: list[str] = []

        def orient(name: str) -> None:
            try:
                client = Client(self.url, name)
                for _ in range(3):
                    client.call("graph_report")
            except Exception as e:  # noqa: BLE001 — reported, never swallowed
                errors.append(f"{name}: {e}")

        threads = [threading.Thread(target=orient, args=(f"reader-{i}",)) for i in range(4)]
        for t in threads:
            t.start()
        for t in threads:
            t.join(timeout=120)

        self.assertEqual(errors, [], "concurrent readers must all succeed")
        self.assertFalse(any(t.is_alive() for t in threads), "no reader may hang")

    def test_the_handshake_carries_the_served_kit(self):
        """Served content must reach an HTTP client exactly as it reaches a
        stdio one — the kit belongs to the server, not to the transport."""
        client = Client(self.url, "instructions")
        self.assertIn("persistent, coherent design brain", client.instructions)
        self.assertIn("SKILLS ARE SERVED", client.instructions)


if __name__ == "__main__":
    os.chdir(REPO)
    unittest.main(verbosity=2)
