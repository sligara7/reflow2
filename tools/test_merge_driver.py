#!/usr/bin/env python3
"""Tests for `reflow2-mcp --merge-driver` — reflow2 as git's merge driver.

Hermetic and stdlib-only, but deliberately NOT a unit test: it builds two real
git branches that each edit a different part of the same committed design export
and runs `git merge`, because the thing being tested is the *contract with git*,
not the merge algorithm (which `tests/merge.rs` already pins). Three home-grown
test layers once agreed with each other and were all wrong because each was a
client we wrote; git is the client here, so git runs the test.

The contract, from git's docs: a driver exits 0 meaning "merged, result is in
%A", and non-zero meaning "conflicts remain, leave the path unmerged". So the
cases are: disjoint edits merge with no human at all; a genuine both-sides
conflict leaves the path unmerged, names the conflict ids, and does not silently
pick a side.

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

REPO = pathlib.Path(__file__).resolve().parent.parent
BINARY = REPO / "target" / "debug" / "reflow2-mcp"

EXPORT = "design.json"


def doc(nodes):
    """A minimal export document — the shape --merge/--merge-apply already read."""
    return {
        "graph_id": "reflow2",
        "nodes": nodes,
        "edges": [],
    }


def node(node_id, name, extra=None):
    props = {"name": name}
    if extra:
        props.update(extra)
    return {"node_id": node_id, "node_type": "Requirement", "properties": props}


def git(cwd, *args, check=True):
    return subprocess.run(
        ["git", *args],
        cwd=cwd,
        check=check,
        capture_output=True,
        text=True,
    )


class MergeDriverTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if not BINARY.exists():
            raise unittest.SkipTest(f"{BINARY} not built (cargo build -p reflow2-mcp)")
        if not shutil.which("git"):
            raise unittest.SkipTest("git not on PATH")

    def setUp(self):
        self.dir = pathlib.Path(tempfile.mkdtemp(prefix="reflow2-merge-driver-"))
        self.addCleanup(shutil.rmtree, self.dir, ignore_errors=True)
        git(self.dir, "init", "-q", "-b", "main")
        git(self.dir, "config", "user.email", "test@example.com")
        git(self.dir, "config", "user.name", "Test")
        # The pair git needs: .gitattributes names the driver, config defines it.
        (self.dir / ".gitattributes").write_text(f"{EXPORT} merge=reflow2\n")
        git(self.dir, "config", "merge.reflow2.name", "reflow2 design export merge")
        git(
            self.dir,
            "config",
            "merge.reflow2.driver",
            f"{BINARY} --merge-driver %O %A %B",
        )

    def commit_export(self, document, message):
        (self.dir / EXPORT).write_text(json.dumps(document, indent=2) + "\n")
        git(self.dir, "add", "-A")
        git(self.dir, "commit", "-q", "-m", message)

    def branch_edit(self, branch, document, message):
        git(self.dir, "checkout", "-q", "-b", branch, "main")
        self.commit_export(document, message)
        git(self.dir, "checkout", "-q", "main")

    def read_export(self):
        return json.loads((self.dir / EXPORT).read_text())

    def test_disjoint_edits_merge_without_a_human(self):
        """The case that made this worth building.

        Two people work different clusters of one design. Nothing about that is a
        conflict — but as one JSON file it is a textual collision, which is why
        claiming nodes never protected anyone from git.
        """
        self.commit_export(doc([node("req:a", "A"), node("req:b", "B")]), "base")
        self.branch_edit(
            "hers",
            doc([node("req:a", "A, refined by her"), node("req:b", "B")]),
            "she edits A",
        )
        self.branch_edit(
            "his",
            doc([node("req:a", "A"), node("req:b", "B, refined by him")]),
            "he edits B",
        )

        git(self.dir, "merge", "-q", "hers")
        merged = git(self.dir, "merge", "his", check=False)

        self.assertEqual(
            merged.returncode,
            0,
            f"disjoint edits must merge cleanly:\n{merged.stdout}\n{merged.stderr}",
        )
        names = {n["node_id"]: n["properties"]["name"] for n in self.read_export()["nodes"]}
        self.assertEqual(names["req:a"], "A, refined by her", "her edit survived")
        self.assertEqual(names["req:b"], "B, refined by him", "his edit survived")

    def test_a_real_conflict_stops_and_names_itself(self):
        """Both sides changed the same property.

        Git's contract is that the driver leaves the path unmerged; reflow2's rule
        4 is that the failure says what to do. Both are asserted, because a driver
        that exits non-zero with an unreadable message is how someone reaches for
        `--ours` and deletes a teammate's afternoon.
        """
        self.commit_export(doc([node("req:a", "A")]), "base")
        self.branch_edit("hers", doc([node("req:a", "A, her way")]), "she rewords A")
        self.branch_edit("his", doc([node("req:a", "A, his way")]), "he rewords A")

        git(self.dir, "merge", "-q", "hers")
        merged = git(self.dir, "merge", "his", check=False)

        self.assertNotEqual(merged.returncode, 0, "a real conflict must not report success")
        combined = merged.stdout + merged.stderr
        self.assertIn("merge:", combined, f"the conflict id must be named: {combined}")
        self.assertIn(
            "--merge-apply",
            combined,
            f"the message must say how to finish the job: {combined}",
        )
        status = git(self.dir, "status", "--porcelain").stdout
        self.assertTrue(
            any(line.startswith(("UU", "AA")) for line in status.splitlines()),
            f"git must leave the export unmerged for the human: {status}",
        )
        # And it must NOT have quietly picked a side.
        self.assertNotIn(
            '"name": "A, his way"',
            (self.dir / EXPORT).read_text(),
            "the driver must not decide a both-sides conflict on its own",
        )

    def test_a_node_added_on_each_side_keeps_both(self):
        """The commonest real case after disjoint edits: two people each capture
        something new. Neither addition is in dispute, so both must survive —
        losing one silently is the failure COORD.md's merge=union exists to stop
        for the text records."""
        self.commit_export(doc([node("req:a", "A")]), "base")
        self.branch_edit("hers", doc([node("req:a", "A"), node("req:h", "Hers")]), "she adds")
        self.branch_edit("his", doc([node("req:a", "A"), node("req:m", "His")]), "he adds")

        git(self.dir, "merge", "-q", "hers")
        merged = git(self.dir, "merge", "his", check=False)

        self.assertEqual(merged.returncode, 0, f"{merged.stdout}\n{merged.stderr}")
        ids = {n["node_id"] for n in self.read_export()["nodes"]}
        self.assertEqual(ids, {"req:a", "req:h", "req:m"}, "both additions survive")

    def test_without_the_config_git_falls_back_rather_than_failing(self):
        """`.gitattributes` travels with the repo but the driver definition does
        not, so a fresh clone has the attribute and no driver. That must degrade
        to git's normal text merge — an unconfigured clone that could not merge at
        all would be a worse failure than the one this fixes."""
        git(self.dir, "config", "--unset", "merge.reflow2.driver")
        self.commit_export(doc([node("req:a", "A"), node("req:b", "B")]), "base")
        self.branch_edit(
            "hers", doc([node("req:a", "A, hers"), node("req:b", "B")]), "she edits A"
        )
        git(self.dir, "checkout", "-q", "main")

        merged = git(self.dir, "merge", "hers", check=False)
        self.assertEqual(
            merged.returncode,
            0,
            f"a fast-forward must still work with no driver configured: {merged.stderr}",
        )


if __name__ == "__main__":
    os.chdir(REPO)
    unittest.main(verbosity=2)
