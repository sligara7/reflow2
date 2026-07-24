#!/usr/bin/env python3
"""Tests for tools/reflow2_check.py — the consumer CI coherence gate (BL-66, BL-88).

Hermetic and stdlib-only. Each case builds a small design with the *real*
reflow2-mcp binary (over stdio, via smoke_mcp.Server), exports it to a temp
file, then runs the gate as a subprocess and asserts on its exit code and
output. The gate's whole contract is that exit code — **0 coherent · 1 gate
failed · 2 could not run** — and the erosion it exists to catch is a registered
artifact drifting from the committed design with no two-sided accept. So this
pins the doctored-fails / clean-passes / missing-refuses trio the gate was
hand-verified against when BL-66 landed, plus the two drift shapes and the
integrity check — the gate itself finally has a regression net.

Skips cleanly when the binary is absent (the gate genuinely cannot run without
it); CI's `full` job builds it first.
"""

from __future__ import annotations

import hashlib
import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from smoke_mcp import Server  # noqa: E402

CHECK = pathlib.Path(__file__).resolve().parent / "reflow2_check.py"
REPO = pathlib.Path(__file__).resolve().parent.parent


def find_bin() -> str | None:
    env = os.environ.get("REFLOW2_BIN")
    if env and os.path.exists(env):
        return env
    for c in (REPO / "target/debug/reflow2-mcp", REPO / "target/release/reflow2-mcp"):
        if c.exists():
            return str(c)
    return shutil.which("reflow2-mcp")


BIN = find_bin()


def short_sha(path: pathlib.Path, n: int = 16) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()[:n]


def coherent(s: Server) -> None:
    """A minimal but coherent thread: nothing an anchored ≥0.8 gap can catch,
    and no artifacts, so a clean export gates green."""
    s.call("create_node", {"node_type": "Project", "id": "proj:x",
                            "props": {"name": "Widget"}})
    s.call("create_node", {"node_type": "Requirement", "id": "req:a",
                            "props": {"name": "A need", "statement": "it must work"}})
    s.call("create_node", {"node_type": "Capability", "id": "cap:a",
                            "props": {"name": "Do it", "description": "does the thing"}})
    s.call("create_node", {"node_type": "Component", "id": "cmp:a",
                            "props": {"name": "The part", "purpose": "holds the doing"}})
    s.call("create_edge", {"edge_type": "SATISFIES", "from_type": "Capability",
                           "from_id": "cap:a", "to_type": "Requirement", "to_id": "req:a"})
    s.call("create_edge", {"edge_type": "ALLOCATED_TO", "from_type": "Capability",
                           "from_id": "cap:a", "to_type": "Component", "to_id": "cmp:a"})


@unittest.skipUnless(BIN, "reflow2-mcp binary not found (build it: cargo build -p reflow2-mcp)")
class Reflow2Check(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory(prefix="reflow2-check-test-")
        self.tmp = pathlib.Path(self._tmp.name)

    def tearDown(self):
        self._tmp.cleanup()

    def export(self, build) -> pathlib.Path:
        """Build a graph with the real binary and export it to a temp file."""
        s = Server(BIN, str(self.tmp / "graph"))
        try:
            build(s)
            path = self.tmp / "design.json"
            s.call("export_graph", {"path": str(path), "overwrite": True})
            return path
        finally:
            s.close()

    def gate(self, export, root=None):
        cmd = [sys.executable, str(CHECK), "--export", str(export),
               "--root", str(root or self.tmp), "--bin", BIN]
        return subprocess.run(cmd, capture_output=True, text=True, timeout=120)

    # ---- the trio ---------------------------------------------------------

    def test_a_coherent_design_passes(self):
        r = self.gate(self.export(coherent))
        self.assertEqual(r.returncode, 0, f"expected clean pass\n{r.stdout}\n{r.stderr}")
        self.assertIn("design and build agree", r.stdout)

    def test_a_missing_export_cannot_run(self):
        r = self.gate(self.tmp / "does-not-exist.json")
        self.assertEqual(r.returncode, 2, "a missing export is 'could not run', never a pass")
        self.assertIn("no design export", r.stderr)

    def test_a_tampered_export_fails_integrity(self):
        export = self.export(coherent)
        doc = json.loads(export.read_text())
        # Edit content without re-hashing: the record no longer matches its own
        # content_hash — hand-edited or corrupted, which the chain must catch.
        self.assertTrue(doc.get("content_hash"), "the export must carry a content_hash to tamper")
        for n in doc["nodes"]:
            if n["node_id"] == "req:a":
                n["properties"]["name"] = "Tampered in the committed file"
        export.write_text(json.dumps(doc))

        r = self.gate(export)
        self.assertEqual(r.returncode, 1, f"a tampered record must fail the gate\n{r.stdout}")
        self.assertIn("INTEGRITY", r.stdout)

    # ---- the erosion the gate exists for: registered artifacts drift ------

    def test_a_changed_artifact_file_is_drift(self):
        art_file = self.tmp / "a.txt"
        art_file.write_text("the built thing, v1")
        registered = short_sha(art_file)

        def build(s):
            coherent(s)
            s.call("create_node", {"node_type": "Artifact", "id": "art:a", "props": {
                "name": "a.txt", "location": "a.txt", "checksum": registered}})
            s.call("create_edge", {"edge_type": "REALIZES", "from_type": "Artifact",
                                   "from_id": "art:a", "to_type": "Capability", "to_id": "cap:a"})

        export = self.export(build)
        # As registered, the file matches — but now it changes with no accept.
        art_file.write_text("the built thing, v2 — edited, design not reconciled")
        r = self.gate(export)
        self.assertEqual(r.returncode, 1, f"unaccepted drift must fail\n{r.stdout}")
        self.assertIn("DRIFT", r.stdout)
        self.assertIn("art:a", r.stdout)

    def test_a_vanished_artifact_file_is_drift(self):
        def build(s):
            coherent(s)
            s.call("create_node", {"node_type": "Artifact", "id": "art:gone", "props": {
                "name": "ghost.rs", "location": "ghost.rs", "checksum": "sha256:deadbeefdeadbeef"}})
            s.call("create_edge", {"edge_type": "REALIZES", "from_type": "Artifact",
                                   "from_id": "art:gone", "to_type": "Capability", "to_id": "cap:a"})

        # ghost.rs was never created under root, so it reads as vanished.
        r = self.gate(self.export(build))
        self.assertEqual(r.returncode, 1, f"a missing registered artifact must fail\n{r.stdout}")
        self.assertIn("DRIFT", r.stdout)

    def test_an_unregistered_artifact_is_a_note_not_a_failure(self):
        # An artifact with no checksum (no_baseline) is reported, but does not
        # gate — registering a hash is the fix, not a red build.
        present = self.tmp / "present.txt"
        present.write_text("here")

        def build(s):
            coherent(s)
            s.call("create_node", {"node_type": "Artifact", "id": "art:new", "props": {
                "name": "present.txt", "location": "present.txt"}})
            s.call("create_edge", {"edge_type": "REALIZES", "from_type": "Artifact",
                                   "from_id": "art:new", "to_type": "Capability", "to_id": "cap:a"})

        r = self.gate(self.export(build))
        self.assertEqual(r.returncode, 0, f"no_baseline must not gate\n{r.stdout}")
        self.assertIn("no_baseline", r.stdout)


if __name__ == "__main__":
    unittest.main(verbosity=2)
