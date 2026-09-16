#!/usr/bin/env python3
"""Tests for tools/impact_of_diff.py — a git diff read as a design change.

Hermetic and stdlib-only, the same shape as test_reflow2_check.py: each case
builds a small design with the *real* reflow2-mcp binary over stdio, exports
it into a real git repository, commits, changes files, and runs the driver as a
subprocess. The contract pinned here is the one that makes the driver worth a
file at all:

- a changed file that a registered Artifact points at reaches, through the
  golden thread, the REQUIREMENT it serves — that is the commit-to-intent hop
  the driver exists to make;
- a changed file NOTHING points at is named, counted, and reported FIRST, and
  never folded into a clean result — a blast radius that omits it would read
  as "safe" when the truth is "invisible" (dec:idea-diff-driven-impact);
- `--fail-on-unmapped` turns that report into exit 1, and without the flag the
  same run exits 0 — the flag is where a gate opts in, so the driver never
  fails a build that did not ask it to (dec:idea-allocation-waits-for-the-
  last-responsible-moment);
- the design record itself is set aside, not reported as unmapped: it changes
  on every design write by construction.

Skips cleanly when the binary is absent; CI's `full` job builds it first.
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

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from smoke_mcp import Server  # noqa: E402

DRIVER = pathlib.Path(__file__).resolve().parent / "impact_of_diff.py"
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


@unittest.skipUnless(BIN, "reflow2-mcp binary not found (build it: cargo build -p reflow2-mcp)")
class ImpactOfDiff(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory(prefix="impact-of-diff-test-")
        self.tmp = pathlib.Path(self._tmp.name)
        self.repo = self.tmp / "repo"
        self.repo.mkdir()
        for args in (["init", "-q", "-b", "main"], ["config", "user.email", "t@t"],
                     ["config", "user.name", "t"]):
            self._git(*args)

    def tearDown(self):
        self._tmp.cleanup()

    def _git(self, *args):
        subprocess.run(["git", *args], cwd=self.repo, check=True,
                       capture_output=True, timeout=60)

    def _rev(self) -> str:
        return subprocess.run(["git", "rev-parse", "HEAD"], cwd=self.repo, check=True,
                              capture_output=True, text=True, timeout=60).stdout.strip()

    def build_and_export(self) -> None:
        """A minimal golden thread — need -> capability -> part -> file — and a
        second file the design never registers. Both exist on disk."""
        (self.repo / "src").mkdir()
        (self.repo / "src" / "gauge.py").write_text("print('gauge v1')\n")
        (self.repo / "src" / "orphan.py").write_text("print('nobody registered me')\n")
        s = Server(BIN, str(self.tmp / "graph"))
        try:
            s.call("add_project", {"id": "prj:w", "name": "Widget", "description": "measure things"})
            s.call("add_requirement", {"id": "req:accurate", "name": "Accurate readings",
                                       "statement": "readings are accurate to 1%"})
            s.call("add_capability", {"id": "cap:gauge", "name": "Gauge",
                                      "description": "reads the gauge",
                                      "satisfies": "req:accurate"})
            s.call("add_component", {"id": "cmp:sensor", "name": "Sensor", "description": "senses"})
            s.call("allocate", {"from_id": "cap:gauge", "to_id": "cmp:sensor"})
            s.call("link_artifact", {"artifact_id": "art:gauge", "name": "gauge.py",
                                     "location": "src/gauge.py", "artifact_type": "code",
                                     "target_id": "cap:gauge",
                                     "checksum": "sha256:" + "0" * 64})
            s.call("export_graph", {"path": str(self.repo / "design.json"), "overwrite": True})
        finally:
            s.close()

    def run_driver(self, *extra) -> subprocess.CompletedProcess:
        cmd = [sys.executable, str(DRIVER), "--export", "design.json", "--root", str(self.repo),
               "--bin", BIN, *extra]
        return subprocess.run(cmd, capture_output=True, text=True, timeout=180, cwd=str(self.repo))

    def commit_all(self, msg: str) -> str:
        self._git("add", "-A")
        self._git("commit", "-qm", msg)
        return self._rev()

    # ------------------------------------------------------------------

    def test_a_registered_file_reaches_the_requirement_it_serves(self):
        self.build_and_export()
        base = self.commit_all("baseline")
        (self.repo / "src" / "gauge.py").write_text("print('gauge v2')\n")
        head = self.commit_all("change the gauge")

        run = self.run_driver("--range", f"{base}..{head}", "--json")
        self.assertEqual(run.returncode, 0, run.stderr)
        out = json.loads(run.stdout)
        self.assertEqual([m["artifact_id"] for m in out["mapped"]], ["art:gauge"])
        self.assertIn("cap:gauge", out["seeds"], out)
        ring = {n["node_id"] for n in out["impact"]["direct_ring"]}
        self.assertIn("req:accurate", ring,
                      "the diff must reach the requirement the changed file serves — "
                      "that hop is the whole point of the driver")

    def test_an_unregistered_file_is_named_first_and_never_folded_in(self):
        self.build_and_export()
        base = self.commit_all("baseline")
        (self.repo / "src" / "orphan.py").write_text("print('changed, and nobody will know')\n")
        head = self.commit_all("change the orphan")

        run = self.run_driver("--range", f"{base}..{head}")
        self.assertEqual(run.returncode, 0, run.stderr)
        text = run.stdout
        self.assertIn("src/orphan.py", text)
        self.assertIn("CANNOT SEE", text)
        self.assertLess(text.index("CANNOT SEE"), text.index("nothing to propagate"),
                        "the unmapped file is reported before the radius, not after it")
        self.assertNotIn("every changed file maps", text)

    def test_fail_on_unmapped_is_an_exit_code_and_only_when_asked(self):
        self.build_and_export()
        base = self.commit_all("baseline")
        (self.repo / "src" / "orphan.py").write_text("v2\n")
        (self.repo / "src" / "gauge.py").write_text("v2\n")
        head = self.commit_all("both")

        quiet = self.run_driver("--range", f"{base}..{head}")
        self.assertEqual(quiet.returncode, 0, quiet.stderr)
        loud = self.run_driver("--range", f"{base}..{head}", "--fail-on-unmapped")
        self.assertEqual(loud.returncode, 1, loud.stderr)
        self.assertIn("src/orphan.py", loud.stdout)
        self.assertIn("art:gauge", loud.stdout, "the mapped half is still reported on a failing run")

    def test_the_design_record_is_set_aside_not_reported_as_unmapped(self):
        self.build_and_export()
        base = self.commit_all("baseline")
        doc = json.loads((self.repo / "design.json").read_text())
        doc["nodes"][0]["properties"]["name"] = doc["nodes"][0]["properties"].get("name", "") + " (touched)"
        (self.repo / "design.json").write_text(json.dumps(doc))
        head = self.commit_all("touch the record")

        run = self.run_driver("--range", f"{base}..{head}", "--json")
        self.assertEqual(run.returncode, 0, run.stderr)
        out = json.loads(run.stdout)
        self.assertEqual(out["unmapped"], [])
        self.assertEqual(out["record_only"], ["design.json"])

    def test_a_deleted_registered_file_is_still_a_change(self):
        self.build_and_export()
        base = self.commit_all("baseline")
        (self.repo / "src" / "gauge.py").unlink()
        head = self.commit_all("delete the gauge")

        run = self.run_driver("--range", f"{base}..{head}", "--json")
        self.assertEqual(run.returncode, 0, run.stderr)
        out = json.loads(run.stdout)
        self.assertEqual(out["mapped"], [{"path": "src/gauge.py", "artifact_id": "art:gauge", "present": False}])
        self.assertIn("cap:gauge", out["seeds"])

    def test_it_refuses_rather_than_guesses_when_git_cannot_answer(self):
        self.build_and_export()
        run = self.run_driver("--range", "..HEAD")
        self.assertEqual(run.returncode, 2, "no base is 'could not run', never an empty radius")


if __name__ == "__main__":
    unittest.main(verbosity=2)
