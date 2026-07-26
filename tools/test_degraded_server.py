#!/usr/bin/env python3
"""Tests for the degraded MCP server and the locked-graph snapshot read.

Both exist because of one field report. A StoryFlow fleet stood up three peer
Bosses against a single graph on 2026-07-25; the first to start won the exclusive
lock and the other two died at startup, before any tool existed. What those
sessions saw was not an error but *nothing* — zero `reflow2__*` tools, and, in the
words of the boss who wrote it up, "nothing distinguished this from 'reflow2 was
never configured for this project'". reflow2's genuinely good diagnosis went to
stderr and died with the process.

So these pin the two behaviours that turn an invisible outage into a self-
explaining one, both measured against a REAL held lock rather than a mock:

  1. A server that cannot open the graph still completes the MCP handshake,
     carries the reason in its instructions, and serves one unmistakably-named
     tool.
  2. `--export-snapshot` reads a graph somebody else is holding, and says loudly
     that the read is best-effort.
  3. It does that ON THE TRANSPORT THAT WAS ASKED FOR (BL-105). Everything above
     drove stdio, and for the whole of v0.14.0 that was all the failure path
     could do: ask for `--http` against a held graph and the explanation went to
     stdio while every session pointed at the URL got connection refused -- the
     original outage, reintroduced on the newer transport, with a green suite
     the entire time because nothing here had ever said the word "http".

Hermetic and stdlib-only; skips cleanly when the binary is absent.
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
BINARY = REPO / "target" / "debug" / "reflow2-mcp"


def rpc(proc, method, params=None, mid=1):
    msg = {"jsonrpc": "2.0", "id": mid, "method": method}
    if params is not None:
        msg["params"] = params
    proc.stdin.write(json.dumps(msg) + "\n")
    proc.stdin.flush()
    line = proc.stdout.readline()
    if not line:
        raise AssertionError(f"server closed stdout; stderr:\n{proc.stderr.read()}")
    return json.loads(line)


def serve(graph_path):
    return subprocess.Popen(
        [str(BINARY), "--graph-path", str(graph_path)],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=1,
        env={**os.environ, "RUST_LOG": "error"},
    )


def handshake(proc, client="test-peer"):
    init = rpc(
        proc,
        "initialize",
        {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": client, "version": "1"},
        },
    )
    assert "result" in init, f"handshake failed: {init}"
    proc.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n")
    proc.stdin.flush()
    return init["result"]


class DegradedServerTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if not BINARY.exists():
            raise unittest.SkipTest(f"{BINARY} not built (cargo build -p reflow2-mcp)")

    def setUp(self):
        self.dir = pathlib.Path(tempfile.mkdtemp(prefix="reflow2-degraded-"))
        self.graph = self.dir / "graph"
        self.addCleanup(shutil.rmtree, self.dir, ignore_errors=True)
        # A real graph with real content, and a real holder of its lock.
        self.holder = serve(self.graph)
        self.addCleanup(self.holder.terminate)
        handshake(self.holder, "holder")
        rpc(
            self.holder,
            "tools/call",
            {"name": "add_project", "arguments": {"id": "proj:t", "name": "Held"}},
            2,
        )

    def test_a_peer_that_cannot_open_the_graph_still_handshakes(self):
        """The fix, in one assertion: the peer connects instead of vanishing."""
        peer = serve(self.graph)
        self.addCleanup(peer.terminate)
        result = handshake(peer, "peer")
        self.assertEqual(result["serverInfo"]["name"], "reflow2-mcp")

    def test_the_reason_arrives_in_the_handshake_instructions(self):
        """Where it matters: instructions land in the agent's context, so the
        reason is there BEFORE the agent wonders where the tools went."""
        peer = serve(self.graph)
        self.addCleanup(peer.terminate)
        instructions = handshake(peer, "peer").get("instructions", "")

        self.assertIn("UNAVAILABLE", instructions)
        self.assertIn("already has the design graph", instructions)
        self.assertIn(
            "DO NOT conclude",
            instructions,
            "the instruction not to report the design brain as missing is the point — that "
            "misreport is what the fleet actually did",
        )
        self.assertIn(str(self.graph), instructions, "which graph failed must be named")

    def test_one_unmistakably_named_tool_is_served(self):
        peer = serve(self.graph)
        self.addCleanup(peer.terminate)
        handshake(peer, "peer")
        tools = rpc(peer, "tools/list", {}, 2)["result"]["tools"]
        names = [t["name"] for t in tools]

        self.assertEqual(names, ["reflow2_unavailable"], f"got {names}")
        self.assertIn(
            "UNAVAILABLE",
            tools[0]["description"],
            "a session that only lists tools must still learn what is wrong",
        )

    def test_the_tool_returns_the_reason_and_what_to_do(self):
        peer = serve(self.graph)
        self.addCleanup(peer.terminate)
        handshake(peer, "peer")
        called = rpc(
            peer, "tools/call", {"name": "reflow2_unavailable", "arguments": {}}, 3
        )["result"]
        payload = called["structuredContent"]

        self.assertFalse(payload["available"])
        self.assertIn("already has the design graph", payload["reason"])
        self.assertTrue(payload["remedies"], "a diagnosis with no remedy is half an answer")
        self.assertIn("merge driver", " ".join(payload["remedies"]))

    def test_a_healthy_graph_still_serves_the_whole_surface(self):
        """The guard on the guard: degraded mode must not leak into the normal
        path, or the fix would have cost every session its tools."""
        other = self.dir / "own-graph"
        peer = serve(other)
        self.addCleanup(peer.terminate)
        handshake(peer, "solo")
        names = [t["name"] for t in rpc(peer, "tools/list", {}, 2)["result"]["tools"]]
        self.assertGreater(len(names), 50, "a free graph serves the real surface")
        self.assertNotIn("reflow2_unavailable", names)


class DegradedOverHttpTest(unittest.TestCase):
    """BL-105. The degraded surface must come out of the door the caller asked
    for. Serving it on stdio when `--http` was given is the original outage
    wearing new clothes: the explanation exists, and it is on a transport
    nobody is listening to, so every session pointed at that URL sees a refused
    connection -- indistinguishable from reflow2 never having been configured,
    which is the precise thing `req:never-silently-absent` forbids."""

    @classmethod
    def setUpClass(cls):
        if not BINARY.exists():
            raise unittest.SkipTest(f"{BINARY} not built (cargo build -p reflow2-mcp)")
        sys.path.insert(0, str(REPO / "tools"))

    def setUp(self):
        from test_shared_sessions import free_port

        self.dir = pathlib.Path(tempfile.mkdtemp(prefix="reflow2-degraded-http-"))
        self.graph = self.dir / "graph"
        self.addCleanup(shutil.rmtree, self.dir, ignore_errors=True)
        # A REAL holder, exactly as the other suite does it — the whole point is
        # that this is measured against a genuinely locked graph, not a mock.
        self.holder = serve(self.graph)
        self.addCleanup(self.holder.terminate)
        handshake(self.holder, "holder")
        self.port = free_port()
        self.url = f"http://127.0.0.1:{self.port}/"

    def start_degraded_http(self):
        """Start a server with --http against the held graph, and wait for the
        port. Returns the process; fails with the server's own output if it
        never listens, because 'no listener' IS the bug under test."""
        import socket
        import time

        proc = subprocess.Popen(
            [
                str(BINARY),
                "--graph-path",
                str(self.graph),
                "--http",
                f"127.0.0.1:{self.port}",
            ],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            env={**os.environ, "RUST_LOG": "error"},
        )
        self.addCleanup(proc.terminate)
        deadline = time.time() + 30
        while time.time() < deadline:
            try:
                with socket.create_connection(("127.0.0.1", self.port), timeout=1):
                    return proc
            except OSError:
                if proc.poll() is not None:
                    self.fail(
                        "the degraded server exited instead of serving over HTTP "
                        f"(this was the bug):\n{proc.stderr.read()}"
                    )
                time.sleep(0.2)
        self.fail("nothing ever listened on the port the caller asked for")

    def test_a_held_graph_still_serves_the_degraded_surface_over_http(self):
        """The regression, in one assertion: an HTTP client gets a handshake
        rather than a refused connection."""
        from test_shared_sessions import Client

        self.start_degraded_http()
        peer = Client(self.url, "http-peer")
        self.assertTrue(peer.session, "the HTTP client must complete a handshake")

    def test_the_reason_reaches_an_http_client_too(self):
        """Handshaking is not enough — the reason has to arrive where the agent
        reads it, which is the instructions, on this transport as on stdio."""
        from test_shared_sessions import Client

        self.start_degraded_http()
        instructions = Client(self.url, "http-peer").instructions

        self.assertIn("UNAVAILABLE", instructions)
        self.assertIn("already has the design graph", instructions)
        self.assertIn("DO NOT conclude", instructions)

    def test_the_one_tool_is_callable_over_http(self):
        """And it must actually answer, not merely be advertised."""
        from test_shared_sessions import Client

        self.start_degraded_http()
        peer = Client(self.url, "http-peer")
        answer = peer.call("reflow2_unavailable")

        self.assertIn("already has the design graph", json.dumps(answer))

    def test_the_startup_line_does_not_claim_to_be_serving_the_design(self):
        """A degraded server looks like a working one from the outside. An
        operator who reads 'serving over HTTP' and walks away has been misled,
        so the line has to say which surface this is."""
        proc = self.start_degraded_http()
        proc.terminate()
        proc.wait(timeout=10)
        stderr = proc.stderr.read()

        self.assertIn("DEGRADED", stderr, stderr)
        self.assertIn("could not be opened", stderr)


class SnapshotReadTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if not BINARY.exists():
            raise unittest.SkipTest(f"{BINARY} not built")

    def setUp(self):
        self.dir = pathlib.Path(tempfile.mkdtemp(prefix="reflow2-snaptest-"))
        self.graph = self.dir / "graph"
        self.addCleanup(shutil.rmtree, self.dir, ignore_errors=True)
        self.holder = serve(self.graph)
        self.addCleanup(self.holder.terminate)
        handshake(self.holder, "holder")
        rpc(
            self.holder,
            "tools/call",
            {"name": "add_project", "arguments": {"id": "proj:held", "name": "Held design"}},
            2,
        )
        rpc(
            self.holder,
            "tools/call",
            {
                "name": "add_requirement",
                "arguments": {
                    "id": "req:one",
                    "name": "Something",
                    "statement": "Written while the lock was held.",
                },
            },
            3,
        )

    def run_snapshot(self):
        return subprocess.run(
            [str(BINARY), "--graph-path", str(self.graph), "--export-snapshot"],
            capture_output=True,
            text=True,
            cwd=REPO,
            env={**os.environ, "RUST_LOG": "error"},
        )

    def test_a_held_graph_can_be_read(self):
        """The blocker this removes: a peer could not so much as export the
        design, and export is where the whole merge workflow starts."""
        r = self.run_snapshot()
        self.assertEqual(r.returncode, 0, f"stderr:\n{r.stderr}")
        doc = json.loads(r.stdout)
        ids = {n["node_id"] for n in doc["nodes"]}
        self.assertIn("proj:held", ids)
        self.assertIn("req:one", ids, "including writes made by the holder")

    def test_the_caveat_is_impossible_to_miss(self):
        """It is a best-effort read of a live database. Shipping that without the
        caveat is how folklore spreads."""
        r = self.run_snapshot()
        self.assertIn("BEST-EFFORT SNAPSHOT", r.stderr)
        self.assertIn("NOT crash-consistent", r.stderr)
        self.assertIn("not a backup", r.stderr.lower())

    def test_nothing_is_left_behind(self):
        """Including the provenance sidecar, which is written BESIDE the
        directory — the trap that made a fresh graph refuse to open in the
        field, and which bit this code on its first run."""
        self.run_snapshot()
        # Precise glob: the snapshot names itself `reflow2-snapshot-<pid>`, and a
        # loose pattern would match this test's own scratch directory — which it
        # did, on the first run.
        # ANY residue named after a snapshot, not just the suffixes we happen to
        # know: the first version of this stripped `.meta.json` alone and so
        # missed `.id.json` entirely when design identity added one (found
        # 2026-07-25 by looking, not by this test — which is the finding).
        residue = [
            p
            for p in pathlib.Path(tempfile.gettempdir()).glob("reflow2-snapshot-*")
            if p.name.removeprefix("reflow2-snapshot-").split(".")[0].isdigit()
        ]
        self.assertEqual(residue, [], f"left behind: {residue}")

    def test_an_unlocked_graph_gets_a_real_export_and_says_so(self):
        """A snapshot nobody needed would be a worse answer than the truth."""
        self.holder.terminate()
        self.holder.wait(timeout=10)
        r = self.run_snapshot()
        self.assertEqual(r.returncode, 0, f"stderr:\n{r.stderr}")
        self.assertIn("ORDINARY export", r.stderr)
        self.assertNotIn("BEST-EFFORT", r.stderr)


if __name__ == "__main__":
    os.chdir(REPO)
    unittest.main(verbosity=2)
