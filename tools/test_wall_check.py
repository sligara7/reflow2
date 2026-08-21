#!/usr/bin/env python3
"""Tests for wall_check — the walls a design declares, checked against its source.

The tool answers "is my decomposition sound", so the thing most worth pinning is
not that it finds cycles. It is that **it never quietly guesses**: what it could
not read, could not resolve and could not map is counted and named, because a
file the tool cannot parse is not a file with no dependencies.

Standard library only, and hermetic — every case builds its own tiny design plus
its own tiny source tree in a temp directory, so nothing here depends on
reflow2's own graph or on the repo layout.
"""

from __future__ import annotations

import importlib.util
import json
import os
import subprocess
import sys
import tempfile
import textwrap
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location("wall_check", os.path.join(HERE, "wall_check.py"))
wall_check = importlib.util.module_from_spec(spec)
spec.loader.exec_module(wall_check)


def design(components, realizes, contains=(), levels=None, depends=()):
    """A minimal export: components, the files they point at, and containment."""
    levels = levels or {}
    nodes, edges = [], []
    for c in components:
        nodes.append(
            {
                "node_id": c,
                "node_type": "Component",
                "properties": {"name": c, "level": levels.get(c, "component")},
            }
        )
    for i, (comp, path) in enumerate(realizes):
        art = f"art:{i}"
        nodes.append(
            {"node_id": art, "node_type": "Artifact", "properties": {"location": path}}
        )
        edges.append({"from_id": art, "to_id": comp, "edge_type": "REALIZES", "properties": {}})
    for parent, child in contains:
        edges.append(
            {"from_id": parent, "to_id": child, "edge_type": "CONTAINS", "properties": {}}
        )
    for a, b in depends:
        edges.append({"from_id": a, "to_id": b, "edge_type": "DEPENDS_ON", "properties": {}})
    return {"nodes": nodes, "edges": edges}


class WallCheck(unittest.TestCase):
    def run_in(self, tmp, doc, files):
        """Write a design and a source tree, then run the tool and return stdout."""
        for path, body in files.items():
            full = os.path.join(tmp, path)
            os.makedirs(os.path.dirname(full), exist_ok=True)
            with open(full, "w", encoding="utf-8") as fh:
                fh.write(textwrap.dedent(body))
        export = os.path.join(tmp, "design.json")
        with open(export, "w", encoding="utf-8") as fh:
            json.dump(doc, fh)
        out = subprocess.run(
            [sys.executable, os.path.join(HERE, "wall_check.py"), "--export", export],
            capture_output=True,
            text=True,
            cwd=tmp,
        )
        self.assertEqual(out.returncode, 0, out.stderr)
        return out.stdout

    def test_the_mapping_comes_from_the_graph_not_from_a_name_match(self):
        """A component whose file is named nothing like it is still mapped.

        This is the whole point of the rewrite: the design SAYS which file
        belongs to which part, and a tool that guesses instead can disagree with
        it — measured at 4 one way and 7 the other on reflow2's own graph.
        """
        with tempfile.TemporaryDirectory() as tmp:
            doc = design(
                ["cmp:alpha", "cmp:beta"],
                [("cmp:alpha", "src/wildly_different.py"), ("cmp:beta", "src/other.py")],
            )
            out = self.run_in(
                tmp,
                doc,
                {
                    "src/wildly_different.py": "import other\n",
                    "src/other.py": "x = 1\n",
                },
            )
            self.assertIn("2 of them point at a file", out)
            self.assertIn("1 coupling edge(s) found", out)

    def test_a_language_it_cannot_read_is_counted_and_named(self):
        """Never silently skipped — a file it cannot parse is not a file with
        no dependencies, and reporting it as clean would be the vacuous zero."""
        with tempfile.TemporaryDirectory() as tmp:
            doc = design(["cmp:a"], [("cmp:a", "src/thing.rb")])
            out = self.run_in(tmp, doc, {"src/thing.rb": "require 'x'\n"})
            self.assertIn("COULD NOT READ", out)
            self.assertIn(".rb", out)

    def test_a_component_pointing_at_no_file_is_named(self):
        with tempfile.TemporaryDirectory() as tmp:
            doc = design(["cmp:a", "cmp:ghost"], [("cmp:a", "src/a.py")])
            out = self.run_in(tmp, doc, {"src/a.py": "x = 1\n"})
            self.assertIn("point at NO file", out)
            self.assertIn("cmp:ghost", out)

    def test_a_cycle_between_two_parts_is_found_and_its_evidence_named(self):
        with tempfile.TemporaryDirectory() as tmp:
            doc = design(
                ["sys:one", "sys:two", "cmp:a", "cmp:b"],
                [("cmp:a", "src/a.py"), ("cmp:b", "src/b.py")],
                contains=[("sys:one", "cmp:a"), ("sys:two", "cmp:b")],
                levels={"sys:one": "subsystem", "sys:two": "subsystem"},
            )
            out = self.run_in(
                tmp, doc, {"src/a.py": "import b\n", "src/b.py": "import a\n"}
            )
            self.assertIn("CYCLE", out)
            self.assertIn("sys:one", out)
            self.assertIn("sys:two", out)
            # and it names the module pair that produced it, not just the parts
            self.assertIn("cmp:a->cmp:b", out.replace(" ", ""))

    def test_prose_is_never_structure(self):
        """A module name inside a docstring or comment must not become an edge.

        The adopt run's phantom fourth cycle was a rustdoc link in a comment.
        """
        with tempfile.TemporaryDirectory() as tmp:
            doc = design(
                ["cmp:a", "cmp:b"], [("cmp:a", "src/a.py"), ("cmp:b", "src/b.py")]
            )
            out = self.run_in(
                tmp,
                doc,
                {
                    "src/a.py": '''
                        """This module is nothing like: import b

                        and neither is this comment.
                        """
                        # import b
                        s = "import b"
                        x = 1
                        ''',
                    "src/b.py": "y = 2\n",
                },
            )
            self.assertIn("0 coupling edge(s) found", out)

    def test_a_declared_pair_the_source_lacks_is_reported_and_not_called_a_defect(self):
        """A contract can be real without an import — a process boundary, a file
        format, a human step. The tool reads imports only, and must say so
        rather than reporting the difference as an error."""
        with tempfile.TemporaryDirectory() as tmp:
            doc = design(
                ["cmp:a", "cmp:b"],
                [("cmp:a", "src/a.py"), ("cmp:b", "src/b.py")],
                depends=[("cmp:a", "cmp:b")],
            )
            out = self.run_in(tmp, doc, {"src/a.py": "x = 1\n", "src/b.py": "y = 2\n"})
            self.assertIn("1 pair(s) the DESIGN has and the source does not", out)
            self.assertIn("NOT defects", out)

    def test_an_undeclared_pair_the_source_has_is_reported(self):
        with tempfile.TemporaryDirectory() as tmp:
            doc = design(
                ["cmp:a", "cmp:b"], [("cmp:a", "src/a.py"), ("cmp:b", "src/b.py")]
            )
            out = self.run_in(
                tmp, doc, {"src/a.py": "import b\n", "src/b.py": "y = 2\n"}
            )
            self.assertIn("1 pair(s) the SOURCE has and the design does not", out)
            self.assertIn("cmp:a -> cmp:b", out)

    def test_a_leaf_level_does_not_claim_zero_edges_inside_itself(self):
        """At the leaf level every edge crosses BY CONSTRUCTION. Printing
        '0 inside' there states an arithmetic certainty as if it were a
        measurement, which is the shape of every false all-clear in this
        codebase."""
        with tempfile.TemporaryDirectory() as tmp:
            doc = design(
                ["cmp:a", "cmp:b"], [("cmp:a", "src/a.py"), ("cmp:b", "src/b.py")]
            )
            out = self.run_in(
                tmp, doc, {"src/a.py": "import b\n", "src/b.py": "y = 2\n"}
            )
            self.assertIn("edge(s) between them", out)
            self.assertNotIn("edge(s) inside one", out)

    def test_an_empty_design_says_so_rather_than_reporting_clean(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = self.run_in(tmp, design([], []), {})
            self.assertIn("0 component(s) in the design", out)

    def test_a_missing_export_is_said_not_assumed_empty(self):
        out = subprocess.run(
            [sys.executable, os.path.join(HERE, "wall_check.py"), "--export", "/nope/x.json"],
            capture_output=True,
            text=True,
        )
        self.assertEqual(out.returncode, 0)
        self.assertIn("nothing to check against", out.stdout)


if __name__ == "__main__":
    unittest.main(verbosity=2)
