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

    def test_a_drifted_artifact_past_the_first_page_is_still_drift(self):
        """The gate swept one reply and called it the tree.

        `scan_nodes` answers with as many nodes as FIT and reports what it held
        back — `total` against `returned`, plus `omitted` and `next_offset`. The
        gate's JSON-RPC client unwraps the `{count, items}` envelope to the items
        and drops those fields, so a capped page arrived looking exactly like a
        complete set, and the gate then passed `exhaustive: true` over it.

        Measured on reflow2's own design 2026-08-04: total 144, returned 124,
        `capped_by: "size"` — twenty artifacts never hashed. It was not
        theoretical: `art:tools-built` had drifted the previous day and the gate
        had reported OK twice over it. A checker that measures 86% of the tree
        and speaks as though it measured all of it is the exact failure this
        file exists to catch, arriving in the checker itself.

        So: enough artifacts to force the cap, with the drifted one placed where
        only a paging sweep reaches it.
        """
        # Long names are what makes the payload big — the same shape as the real
        # design, where an artifact's name carries a sentence.
        filler = "a name long enough that the reply fills up before the tree does. " * 6
        clean = []
        for i in range(200):
            p = self.tmp / f"bulk-{i:03d}.txt"
            p.write_text(f"bulk artifact {i}")
            clean.append((f"art:aaa-{i:03d}", p.name, short_sha(p)))

        drifted = self.tmp / "zzz-last.txt"
        drifted.write_text("the last one, v1")
        drifted_sum = short_sha(drifted)

        def build(s):
            coherent(s)
            for art_id, name, checksum in clean:
                s.call("create_node", {"node_type": "Artifact", "id": art_id, "props": {
                    "name": f"{name} — {filler}", "location": name, "checksum": checksum}})
            # Sorts last, so only a sweep that pages ever reaches it.
            s.call("create_node", {"node_type": "Artifact", "id": "art:zzz-last", "props": {
                "name": f"zzz-last.txt — {filler}", "location": drifted.name,
                "checksum": drifted_sum}})
            s.call("create_edge", {"edge_type": "REALIZES", "from_type": "Artifact",
                                   "from_id": "art:zzz-last", "to_type": "Capability",
                                   "to_id": "cap:a"})

        export = self.export(build)
        # Everything registered matches — then the LAST one changes, unaccepted.
        drifted.write_text("the last one, v2 — edited, design not reconciled")

        r = self.gate(export)
        self.assertEqual(r.returncode, 1, f"drift past the first page must fail\n{r.stdout}")
        self.assertIn(
            "art:zzz-last", r.stdout,
            f"the drifted artifact is past the first reply — a gate that stops at one "
            f"page reports OK over it\n{r.stdout}")

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




    # ---- a directory artifact, and the exit-code contract -----------------

    def test_a_directory_artifact_does_not_crash_the_gate(self):
        """reflow2's own graph has 182 artifacts with a location and NOT ONE
        resolves to a directory, so CI has run this gate on every push for weeks
        and never executed the directory branch. dev_storyflow has 11, and the
        gate died on the first one with IsADirectoryError. Reported by the
        api-boss fleet 2026-08-15 and reproduced here before the fix."""
        def with_dir_artifact(s):
            coherent(s)
            s.call("create_node", {"node_type": "Artifact", "id": "art:dir",
                                   "props": {"name": "memory", "location": "memory",
                                             "artifact_type": "document"}})
            s.call("create_edge", {"edge_type": "REALIZES", "from_type": "Artifact",
                                   "from_id": "art:dir", "to_type": "Capability",
                                   "to_id": "cap:a"})

        export = self.export(with_dir_artifact)
        (self.tmp / "memory").mkdir()
        r = self.gate(export)

        self.assertNotIn("IsADirectoryError", r.stderr,
                         f"a directory must not crash the gate\n{r.stderr}")
        self.assertNotEqual(r.returncode, 2,
                            f"the gate must still RUN\n{r.stdout}\n{r.stderr}")
        # Present and unjudgeable is a NOTE, never a failure and never a silent
        # pass: the design says a directory is there, and it is.
        self.assertIn("no_baseline", r.stdout,
                      f"a directory is present-but-unhashable\n{r.stdout}")

    def test_an_unexpected_fault_is_exit_2_not_a_gate_failure(self):
        """THE COUNTERWEIGHT THAT MATTERS, and it took two attempts to make it
        mean anything. A dedicated code exists for "could not run" so CI can tell
        a drifted design from a broken tool; every unhandled exception used to
        land in 1, the code reserved for "gate failed".

        THE FIRST VERSION OF THIS TEST PASSED WITH THE FIX REMOVED. It forced the
        fault with an unreadable export, which `die(2, ...)` already handles — so
        it exercised a path that was correct before the change and proved
        nothing. Every fault this tool KNOWS about is already routed to 2; the
        guard exists for the ones it does not, so the only honest exercise is to
        hand it one directly."""
        import importlib.util

        spec = importlib.util.spec_from_file_location("reflow2_check_under_test", CHECK)
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)

        def boom() -> int:
            raise RuntimeError("something nobody anticipated")

        mod.main = boom
        self.assertEqual(
            mod.run_guarded(), 2,
            "an unanticipated fault is could-not-run (2), never gate-failed (1)",
        )

    def test_an_explicit_exit_code_still_wins_over_the_guard(self):
        """COUNTERWEIGHT to the counterweight: the guard must not swallow a
        DELIBERATE exit. `die()` raises SystemExit to carry its own code, and a
        guard that caught it would turn every honest 2 — and every gate failure —
        into whatever it felt like. SystemExit is a BaseException for exactly
        this reason, and this pins that it stays that way."""
        import importlib.util

        spec = importlib.util.spec_from_file_location("reflow2_check_exit_test", CHECK)
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)

        def deliberate() -> int:
            mod.die(2, "a fault this tool does know about")

        mod.main = deliberate
        with self.assertRaises(SystemExit) as caught:
            mod.run_guarded()
        self.assertEqual(caught.exception.code, 2, "an explicit exit passes through")


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
        genuinely replaces the committed one.

        THE SAME STORE as the first export, deliberately. It used to be a second
        one (`graph2`), which meant every "second export" in this fixture was a
        DIFFERENT design with its own minted `graph_id` — so the chain tests
        below were checking the lineage of two unrelated designs and passing.
        Found 2026-08-02 when the identity check (BL-169) was added and
        immediately failed on the fixture: a design replacing itself is what the
        chain is about, and modelling it as two designs made the fixture prove
        less than it claimed."""
        s = Server(BIN, str(self.tmp / "graph"))
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

    def test_a_silently_renamed_design_fails(self):
        """BL-169. A rename passes every OTHER check here, which is why it
        shipped: the chain links across it perfectly, the content hash matches
        its own content, and both CI jobs went green on a design that had
        stopped being called what it was called."""
        repo, _ = self.git_repo_with_export()
        export = self.second_export(repo)
        doc = json.loads(export.read_text())
        was = doc["graph_id"]
        doc["graph_id"] = "05a6fbe860bf7a23"  # what a temp-graph replay produced
        # Re-hash so integrity still passes — the point is that identity is the
        # ONLY check that can catch this one.
        doc["content_hash"] = "sha256:" + hashlib.sha256(json.dumps(
            {"edges": doc["edges"], "graph_id": doc["graph_id"], "nodes": doc["nodes"]},
            sort_keys=True, ensure_ascii=False, separators=(",", ":"),
        ).encode()).hexdigest()
        export.write_text(json.dumps(doc, sort_keys=True, indent=2, ensure_ascii=False))
        r = self.gate(export, root=repo, cwd=repo)
        self.assertIn("IDENTITY", r.stdout,
                      f"a silent rename must fail the gate\n{r.stdout}")
        self.assertIn(was, r.stdout, "the failure must name what it was called")
        self.assertNotIn("INTEGRITY", r.stdout,
                         "integrity must still pass — proving identity is the check that caught it")
        self.assertNotEqual(r.returncode, 0)

    def test_an_unchanged_name_does_not_complain(self):
        """The counterweight: identity must not fire on ordinary content change,
        or it would be a second lineage check with a worse message."""
        repo, _ = self.git_repo_with_export()
        export = self.second_export(repo)
        r = self.gate(export, root=repo, cwd=repo)
        self.assertNotIn("IDENTITY", r.stdout,
                         f"same name, changed content — not a rename\n{r.stdout}")

    def test_an_unidentified_document_is_not_a_rename(self):
        """A hand-authored document may legitimately carry no `graph_id`
        (BL-138). Absence of a name is not a different name."""
        repo, _ = self.git_repo_with_export()
        export = self.second_export(repo)
        doc = json.loads(export.read_text())
        doc["graph_id"] = ""
        export.write_text(json.dumps(doc, sort_keys=True, indent=2, ensure_ascii=False))
        r = self.gate(export, root=repo, cwd=repo)
        self.assertNotIn("IDENTITY", r.stdout,
                         f"an unidentified document is not a rename\n{r.stdout}")

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
