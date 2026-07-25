#!/usr/bin/env python3
"""The two-seat scenario, end to end against the real binary.

`req:stale-seat-knows`. `crates/reflow2-core/tests/stale_seat.rs` pins the
judgement; this pins that the judgement is actually *wired into the write*, with
two separate graphs and one shared export file — which is the shape of the bug
and the only way to prove the marker survives a real export.

The scenario is the one docs/collaborating.md had to teach a workaround for:

  1. Seat A exports the design to the shared file.
  2. Seat B (a different graph) adds work and exports to the same file — as if
     it arrived by `git pull`.
  3. Seat A, which never caught up, exports again. Its document is complete and
     older, so nothing downstream would call it a conflict.

Step 3 must be REFUSED, and the refusal must name what would have gone.

Hermetic and stdlib-only; skips cleanly when the binary is absent.
"""

from __future__ import annotations

import json
import os
import pathlib
import shutil
import subprocess
import tempfile
import unittest

REPO = pathlib.Path(__file__).resolve().parent.parent
BINARY = REPO / "target" / "debug" / "reflow2-mcp"


class Seat:
    """One session with its own graph, talking real MCP over stdio."""

    def __init__(self, graph_path: pathlib.Path):
        self.proc = subprocess.Popen(
            [str(BINARY), "--graph-path", str(graph_path)],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
            env={**os.environ, "RUST_LOG": "error"},
        )
        self.id = 0
        self._rpc(
            "initialize",
            {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "seat", "version": "1"},
            },
        )
        self.proc.stdin.write(
            json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n"
        )
        self.proc.stdin.flush()

    def _rpc(self, method, params=None):
        self.id += 1
        msg = {"jsonrpc": "2.0", "id": self.id, "method": method}
        if params is not None:
            msg["params"] = params
        self.proc.stdin.write(json.dumps(msg) + "\n")
        self.proc.stdin.flush()
        line = self.proc.stdout.readline()
        if not line:
            raise AssertionError(f"server closed; stderr:\n{self.proc.stderr.read()}")
        return json.loads(line)

    def call(self, name, args):
        """Returns (structuredContent, error_message)."""
        r = self._rpc("tools/call", {"name": name, "arguments": args})
        if "error" in r:
            return None, r["error"].get("message", "")
        result = r["result"]
        if result.get("isError"):
            text = " ".join(c.get("text", "") for c in result.get("content", []))
            return None, text
        return result.get("structuredContent"), None

    def close(self):
        self.proc.terminate()


class StaleSeatTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if not BINARY.exists():
            raise unittest.SkipTest(f"{BINARY} not built (cargo build -p reflow2-mcp)")

    def setUp(self):
        self.dir = pathlib.Path(tempfile.mkdtemp(prefix="reflow2-staleseat-"))
        self.addCleanup(shutil.rmtree, self.dir, ignore_errors=True)
        self.shared = self.dir / "design.json"

        self.a = Seat(self.dir / "graph-a")
        self.addCleanup(self.a.close)
        self.b = Seat(self.dir / "graph-b")
        self.addCleanup(self.b.close)

        # A common starting point, exported by A and imported by B — the state
        # after both have cloned and are in step.
        self.a.call("add_project", {"id": "proj:s", "name": "Shared"})
        self.a.call(
            "add_requirement",
            {"id": "req:common", "name": "Common", "statement": "Agreed by both."},
        )
        out, err = self.a.call("export_graph", {"path": str(self.shared)})
        self.assertIsNone(err, err)
        self.assertIsNotNone(out)
        b_import, err = self.b.call("import_graph", {"path": str(self.shared)})
        self.assertIsNone(err, err)
        self.assertGreater(b_import["nodes_written"], 0)

    def export_a(self, **extra):
        return self.a.call("export_graph", {"path": str(self.shared), "overwrite": True, **extra})

    def test_the_ordinary_export_goes_through_quietly(self):
        """It must not chatter: A owns the file and nobody else has touched it."""
        self.a.call(
            "add_requirement",
            {"id": "req:mine", "name": "Mine", "statement": "A's own work."},
        )
        out, err = self.export_a()

        self.assertIsNone(err, err)
        self.assertNotIn("sync_note", out, f"a quiet case must stay quiet: {out}")

    def test_writing_over_the_other_seats_work_is_refused(self):
        """THE test. B's work reached the file; A never caught up."""
        self.b.call(
            "add_requirement",
            {"id": "req:theirs", "name": "Theirs", "statement": "B's work."},
        )
        out, err = self.b.call(
            "export_graph", {"path": str(self.shared), "overwrite": True}
        )
        self.assertIsNone(err, err)

        # A adds its own work and exports from a graph that never saw B's.
        self.a.call(
            "add_requirement",
            {"id": "req:mine", "name": "Mine", "statement": "A's own work."},
        )
        out, err = self.export_a()

        self.assertIsNone(out, "the write must not happen")
        self.assertIn("REFUSED", err, err)
        self.assertIn("req:theirs", err, "name whose work would go: {}".format(err))
        self.assertIn("import_graph", err, "and what to do instead")
        # And the file on disk is untouched — the point of the whole exercise.
        on_disk = json.loads(self.shared.read_text())
        ids = {n["node_id"] for n in on_disk["nodes"]}
        self.assertIn("req:theirs", ids, "B's work must still be there")
        self.assertNotIn("req:mine", ids, "A's export must not have landed")

    def test_importing_the_file_first_clears_the_refusal(self):
        """The remedy the message names must actually work, or it is a wall."""
        self.b.call(
            "add_requirement",
            {"id": "req:theirs", "name": "Theirs", "statement": "B's work."},
        )
        self.b.call("export_graph", {"path": str(self.shared), "overwrite": True})
        self.a.call(
            "add_requirement",
            {"id": "req:mine", "name": "Mine", "statement": "A's own work."},
        )
        _, refused = self.export_a()
        self.assertIn("REFUSED", refused or "")

        # Do exactly what the message says.
        report, err = self.a.call("import_graph", {"path": str(self.shared)})
        self.assertIsNone(err, err)
        out, err = self.export_a()

        self.assertIsNone(err, f"after importing, the export must go through: {err}")
        both = {n["node_id"] for n in json.loads(self.shared.read_text())["nodes"]}
        self.assertIn("req:theirs", both, "B's work survived")
        self.assertIn("req:mine", both, "and A's landed")

    def test_the_override_exists_and_is_explicit(self):
        """Discarding their work must be possible — and impossible by accident."""
        self.b.call(
            "add_requirement",
            {"id": "req:theirs", "name": "Theirs", "statement": "B's work."},
        )
        self.b.call("export_graph", {"path": str(self.shared), "overwrite": True})
        self.a.call(
            "add_requirement",
            {"id": "req:mine", "name": "Mine", "statement": "A's own work."},
        )

        out, err = self.export_a(accept_divergence=True)

        self.assertIsNone(err, err)
        ids = {n["node_id"] for n in json.loads(self.shared.read_text())["nodes"]}
        self.assertNotIn("req:theirs", ids, "the override really does discard")

    def test_a_fresh_graph_cannot_wipe_a_real_design(self):
        """The worst case: a new clone exports its empty graph over the design."""
        fresh = Seat(self.dir / "graph-fresh")
        self.addCleanup(fresh.close)

        out, err = fresh.call(
            "export_graph", {"path": str(self.shared), "overwrite": True}
        )

        self.assertIsNone(out, "an empty graph must not replace a real design")
        self.assertIn("REFUSED", err, err)
        self.assertIn("req:common", err, err)


if __name__ == "__main__":
    os.chdir(REPO)
    unittest.main(verbosity=2)
