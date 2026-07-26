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

Then `req:sessions-across-machines` — the same server, reached from ANOTHER
machine (`RemoteSessions` below). A remote session is not simulated by mocking
anything: what makes a request remote, to this transport, is the `Host` header
it carries, so these tests dial loopback while sending the host a session on
another machine would have dialled. That is the real path, byte for byte.

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
import urllib.error
import urllib.request

REPO = pathlib.Path(__file__).resolve().parent.parent
BINARY = REPO / "target" / "debug" / "reflow2-mcp"


def free_port() -> int:
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def start_server(graph_path, bind: str, *extra: str) -> subprocess.Popen:
    """Start a server and wait for the port, rather than sleeping a guessed
    amount. Raises with the server's own stderr if it died instead."""
    server = subprocess.Popen(
        [str(BINARY), "--graph-path", str(graph_path), "--http", bind, *extra],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env={**os.environ, "RUST_LOG": "error"},
    )
    host, _, port = bind.rpartition(":")
    # A wildcard bind is not an address you can connect to.
    dial = "127.0.0.1" if host in ("0.0.0.0", "", "::") else host
    deadline = time.time() + 60
    while time.time() < deadline:
        try:
            with socket.create_connection((dial, int(port)), timeout=1):
                return server
        except OSError:
            if server.poll() is not None:
                raise AssertionError(f"server exited: {server.stderr.read()}")
            time.sleep(0.2)
    raise AssertionError(f"server never came up on {bind}")


def stop_server(server: subprocess.Popen) -> str:
    """Stop it and hand back what it said on stderr, closing both pipes. The
    return value matters: the startup advisory is only ever on stderr, so a
    test that wants to read it must not race the process being reaped."""
    server.terminate()
    try:
        server.wait(timeout=10)
    except subprocess.TimeoutExpired:
        server.kill()
        server.wait(timeout=10)
    said = server.stderr.read()
    server.stderr.close()
    server.stdout.close()
    return said


class Client:
    """One MCP session over streamable HTTP. Responses arrive as SSE frames.

    `host` overrides the `Host` header — which is the entire difference between
    a local session and one on another machine, as far as this transport is
    concerned."""

    def __init__(self, url: str, name: str, host: str | None = None):
        self.url = url
        self.host = host
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
        if self.host:
            # An explicit Host suppresses the one urllib would derive from the
            # URL, so the server sees exactly what a remote client would send.
            headers["Host"] = self.host
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
        cls.server = start_server(cls.dir / ".reflow2" / "graph", f"127.0.0.1:{port}")

    @classmethod
    def tearDownClass(cls):
        stop_server(cls.server)
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


REMOTE = "reflow2-box.example-tailnet.ts.net"


class RemoteSessions(unittest.TestCase):
    """`req:sessions-across-machines`. The third case, and the one with a real
    obstacle in it: the transport answers only requests whose `Host` header is
    on an allowlist — loopback by default. That is DNS-rebinding protection, and
    since reflow2 has no authentication it is the only thing between a web page
    the user visits and their design. So binding a reachable address is not
    enough; the hosts a remote session will dial are named on purpose."""

    @classmethod
    def setUpClass(cls):
        if not BINARY.exists():
            raise unittest.SkipTest(f"{BINARY} not built (cargo build -p reflow2-mcp)")
        cls.dir = pathlib.Path(tempfile.mkdtemp(prefix="reflow2-remote-"))

        closed_port = free_port()
        cls.closed_url = f"http://127.0.0.1:{closed_port}/"
        cls.closed = start_server(cls.dir / "closed", f"127.0.0.1:{closed_port}")

        open_port = free_port()
        cls.open_url = f"http://127.0.0.1:{open_port}/"
        cls.open = start_server(
            cls.dir / "open", f"127.0.0.1:{open_port}", "--http-allow-host", REMOTE
        )

    @classmethod
    def tearDownClass(cls):
        stop_server(cls.closed)
        stop_server(cls.open)
        shutil.rmtree(cls.dir, ignore_errors=True)

    def test_a_host_the_server_was_not_told_about_is_refused(self):
        """The starting state, and the reason the flag exists at all: without
        it, a session on another machine gets a 403 and nothing says why."""
        with self.assertRaises(urllib.error.HTTPError) as caught:
            Client(self.closed_url, "remote-unnamed", host=REMOTE)
        self.assertEqual(caught.exception.code, 403, "an unnamed host is refused")
        # A bare status code would pass on a 403 raised for some unrelated
        # reason, which would make the differential below prove nothing.
        self.assertIn("Host", caught.exception.read().decode(), "refused for THIS reason")

    def test_a_named_host_gets_a_whole_session(self):
        """Not just a 200 on the handshake — a remote seat must be able to do
        the work: initialize, write, and read its own write back."""
        remote = Client(self.open_url, "remote-named", host=REMOTE)
        self.assertTrue(remote.session, "a named host completes the handshake")

        remote.call("add_project", id="proj:remote", name="Remote")
        remote.call(
            "add_requirement",
            id="req:from-afar",
            name="From afar",
            statement="Written by a session on another machine.",
        )
        seen = remote.call("get_node", node_type="Requirement", id="req:from-afar")
        self.assertTrue(seen and seen.get("node"), "the remote seat's write landed")

    def test_a_remote_seat_and_a_local_one_share_the_design(self):
        """The whole point of case three: the two seats are not on the same
        machine, and there is still one design, live, with no export or merge.
        This also pins that the flag EXTENDS the default allowlist rather than
        replacing it — naming a remote host must not lock out the loopback
        sessions already using this server."""
        remote = Client(self.open_url, "remote-shared", host=REMOTE)
        local = Client(self.open_url, "local-shared")
        self.assertNotEqual(remote.session, local.session)

        remote.call("add_project", id="proj:both", name="Both")
        remote.call("add_requirement", id="req:theirs", name="Theirs", statement="r")
        local.call("add_requirement", id="req:ours", name="Ours", statement="l")

        self.assertTrue(
            local.call("get_node", node_type="Requirement", id="req:theirs")["node"],
            "the local seat sees what the remote one wrote",
        )
        self.assertTrue(
            remote.call("get_node", node_type="Requirement", id="req:ours")["node"],
            "and the remote seat sees the local write",
        )

    def test_binding_off_the_box_without_naming_a_host_says_so(self):
        """Rule 4, on the exact failure this feature is here to prevent: a
        reachable bind with a loopback-only allowlist refuses every remote
        session with an opaque 403. The server must say that BEFORE anyone
        tries, and name what would have worked."""
        port = free_port()
        stderr = stop_server(start_server(self.dir / "wildcard", f"0.0.0.0:{port}"))

        self.assertIn("WARNING", stderr, stderr)
        self.assertIn("--http-allow-host", stderr, "it must name the remedy")
        self.assertIn("403", stderr, "and the symptom it prevents")

    def test_loopback_alone_is_still_warned_about_nothing(self):
        """The counterpart, so the warning stays worth reading: the ordinary
        one-machine case must not nag."""
        port = free_port()
        stderr = stop_server(start_server(self.dir / "loopback", f"127.0.0.1:{port}"))
        self.assertNotIn("WARNING", stderr, stderr)


if __name__ == "__main__":
    os.chdir(REPO)
    unittest.main(verbosity=2)
