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

    def gate(self, export, root=None, cwd=None):
        cmd = [sys.executable, str(CHECK), "--export", str(export),
               "--root", str(root or self.tmp), "--bin", BIN]
        return subprocess.run(cmd, capture_output=True, text=True, timeout=120,
                              cwd=str(cwd) if cwd else None)

    def git_repo_with_export(self) -> tuple[pathlib.Path, pathlib.Path]:
        """A real git repo holding a committed export. The lineage check reads
        git, so a fixture that mocked it would prove nothing about the thing
        that actually broke."""
        repo = self.tmp / "repo"
        repo.mkdir()
        for args in (["init", "-q"], ["config", "user.email", "t@t"],
                     ["config", "user.name", "t"]):
            subprocess.run(["git", *args], cwd=repo, check=True,
                           capture_output=True, timeout=60)
        export = self.export(coherent)
        committed = repo / "design.json"
        shutil.copy(export, committed)
        subprocess.run(["git", "add", "design.json"], cwd=repo, check=True,
                       capture_output=True, timeout=60)
        subprocess.run(["git", "commit", "-qm", "first export"], cwd=repo,
                       check=True, capture_output=True, timeout=60)
        return repo, committed

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

    def test_a_truncated_baseline_on_an_untouched_file_is_not_drift(self):
        """BL-160, end to end and in the layer that was wrong.

        `build_design_graph.py` registers `hexdigest()[:16]`, so most of
        reflow2's own baselines are truncated, and `hash_file` used to truncate
        the OBSERVATION to match — a Python workaround only this file knew. On
        2026-08-01 a direct MCP sweep of the same clean tree the gate had just
        called OK reported 51 artifacts drifted, which is what that divergence
        looks like from outside. The gate now hashes the whole file and the core
        decides whether the two digests are the same digest, so this case proves
        the rule moved rather than vanished: nothing touched the file, and green
        must mean the core agreed, not that the caller pre-truncated.
        """
        art_file = self.tmp / "short.txt"
        art_file.write_text("registered short, never edited")
        registered = short_sha(art_file)  # 16 of the 64 hex chars
        self.assertEqual(len(registered), len("sha256:") + 16)

        def build(s):
            coherent(s)
            s.call("create_node", {"node_type": "Artifact", "id": "art:short", "props": {
                "name": "short.txt", "location": "short.txt", "checksum": registered}})
            s.call("create_edge", {"edge_type": "REALIZES", "from_type": "Artifact",
                                   "from_id": "art:short", "to_type": "Capability", "to_id": "cap:a"})

        r = self.gate(self.export(build))
        self.assertEqual(
            r.returncode, 0,
            f"a truncated baseline on an untouched file must not be drift\n{r.stdout}")
        self.assertNotIn("DRIFT", r.stdout)

    def test_a_changed_file_registered_short_is_still_drift(self):
        """The counterweight to the case above, and the reason it is a separate
        test: a length rule loose enough to stop the false red must not stop the
        true one. Same 16-char baseline, file genuinely edited."""
        art_file = self.tmp / "short2.txt"
        art_file.write_text("registered short, v1")
        registered = short_sha(art_file)

        def build(s):
            coherent(s)
            s.call("create_node", {"node_type": "Artifact", "id": "art:short2", "props": {
                "name": "short2.txt", "location": "short2.txt", "checksum": registered}})
            s.call("create_edge", {"edge_type": "REALIZES", "from_type": "Artifact",
                                   "from_id": "art:short2", "to_type": "Capability", "to_id": "cap:a"})

        export = self.export(build)
        art_file.write_text("registered short, v2 — edited, design not reconciled")
        r = self.gate(export)
        self.assertEqual(r.returncode, 1, f"real drift must still fail\n{r.stdout}")
        self.assertIn("DRIFT", r.stdout)
        self.assertIn("art:short2", r.stdout)

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




class ExportChain(unittest.TestCase):
    """BL-107. `dec:export-hash-chain` gives the design a history independent of
    git — each export records the `content_hash` of the one it replaced. Six
    consecutive commits lost that link in July 2026 while the gate reported 0
    notes, the loop stayed clean and `detect_gaps` stayed zero, because nothing
    read the chain. These are the checks that would have said so."""

    setUp = Reflow2Check.setUp
    tearDown = Reflow2Check.tearDown
    export = Reflow2Check.export
    gate = Reflow2Check.gate
    git_repo_with_export = Reflow2Check.git_repo_with_export

    @classmethod
    def setUpClass(cls):
        if BIN is None:
            raise unittest.SkipTest("reflow2-mcp not built")

    def second_export(self, repo: pathlib.Path):
        """Export again into the repo, after a real change, so the working file
        genuinely replaces the committed one."""
        s = Server(BIN, str(self.tmp / "graph2"))
        try:
            coherent(s)
            s.call("add_requirement", {"id": "req:later", "name": "Later",
                                       "statement": "added after the first export"})
            path = repo / "design.json"
            s.call("export_graph", {"path": str(path), "overwrite": True})
        finally:
            s.close()
        return path

    def test_a_sound_chain_passes(self):
        repo, _ = self.git_repo_with_export()
        export = self.second_export(repo)
        doc = json.loads(export.read_text())
        self.assertIsNotNone(doc.get("prev_content_hash"),
                             "exporting onto the committed file must link to it")
        r = self.gate(export, root=repo, cwd=repo)
        self.assertNotIn("LINEAGE", r.stdout, f"a sound chain must not complain\n{r.stdout}")

    def test_a_severed_chain_fails(self):
        """The exact shape of the July 2026 mistake: export elsewhere, copy the
        file into place, and the link is simply absent."""
        repo, _ = self.git_repo_with_export()
        export = self.second_export(repo)
        doc = json.loads(export.read_text())
        doc["prev_content_hash"] = None
        export.write_text(json.dumps(doc, sort_keys=True, indent=2, ensure_ascii=False))

        r = self.gate(export, root=repo, cwd=repo)
        self.assertEqual(r.returncode, 1, f"a severed chain must fail\n{r.stdout}")
        self.assertIn("LINEAGE", r.stdout)
        self.assertIn("records nothing", r.stdout, "it must say what it found")

    def test_a_chain_linked_to_the_wrong_thing_fails(self):
        """The subtler half, and the one that actually happened last: the link is
        present but points at a file that was never the predecessor."""
        repo, _ = self.git_repo_with_export()
        export = self.second_export(repo)
        doc = json.loads(export.read_text())
        doc["prev_content_hash"] = "sha256:" + "0" * 64
        export.write_text(json.dumps(doc, sort_keys=True, indent=2, ensure_ascii=False))

        r = self.gate(export, root=repo, cwd=repo)
        self.assertEqual(r.returncode, 1, f"a wrong link must fail\n{r.stdout}")
        self.assertIn("LINEAGE", r.stdout)

    def test_unchanged_content_is_not_a_break(self):
        """The chain deliberately does not advance while content is unchanged —
        a check that called that a break would fire on every commit that touches
        anything else, and would be turned off within a day."""
        repo, committed = self.git_repo_with_export()
        r = self.gate(committed, root=repo, cwd=repo)
        self.assertNotIn("LINEAGE", r.stdout, f"unchanged content is not a break\n{r.stdout}")

    def test_outside_a_git_repo_the_question_is_skipped(self):
        """An unanswerable question is skipped, never guessed — a consumer
        without git must still be able to run the gate."""
        export = self.export(coherent)
        r = self.gate(export)
        self.assertNotIn("LINEAGE", r.stdout, f"no git, no claim\n{r.stdout}")

if __name__ == "__main__":
    unittest.main(verbosity=2)
