#!/usr/bin/env python3
"""Tests for link_tests — which of a project's tests does the design know about?

The thing most worth pinning is NOT that it finds mappings. It is that **it
refuses to guess**, and that its refusals are honest ones. Two failure modes
would each be invisible in ordinary use:

  🛑 A GUESS DRESSED AS AN ANSWER. A scoring heuristic attributes every test to
     something, so the report looks complete. On reflow2 that put `cmp:graph` at
     the top of most files, because opening a graph is setup and not subject.

  🛑 A PARSING BUG THAT SHRINKS THE EVIDENCE. It reads exactly like an honest
     "no evidence found". One really happened: stripping string literals across
     the whole file let a single unbalanced quote swallow real code, reducing
     `heal.rs` from dozens of visible functions to one, after which the tool
     reported the heal test as calling nothing heal.rs defines. That case is
     `test_an_unbalanced_quote_does_not_swallow_the_code_after_it`.

Standard library only, and hermetic — every case builds its own tiny design and
its own tiny source tree in a temp directory.
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
spec = importlib.util.spec_from_file_location("link_tests", os.path.join(HERE, "link_tests.py"))
link_tests = importlib.util.module_from_spec(spec)
spec.loader.exec_module(link_tests)


def design(realizes):
    """A minimal export: components, and the source files that realize them."""
    nodes, edges = [], []
    for i, (comp, path) in enumerate(realizes):
        if not any(n["node_id"] == comp for n in nodes):
            nodes.append(
                {"node_id": comp, "node_type": "Component", "properties": {"name": comp}}
            )
        art = f"art:{i}"
        nodes.append(
            {"node_id": art, "node_type": "Artifact", "properties": {"location": path}}
        )
        edges.append({"from_id": art, "to_id": comp, "edge_type": "REALIZES", "properties": {}})
    return {"nodes": nodes, "edges": edges}


class LinkTests(unittest.TestCase):
    def run_in(self, tmp, doc, files, emit="text"):
        for path, body in files.items():
            full = os.path.join(tmp, path)
            os.makedirs(os.path.dirname(full), exist_ok=True)
            with open(full, "w", encoding="utf-8") as fh:
                fh.write(textwrap.dedent(body))
        export = os.path.join(tmp, "design.json")
        with open(export, "w", encoding="utf-8") as fh:
            json.dump(doc, fh)
        out = subprocess.run(
            [sys.executable, os.path.join(HERE, "link_tests.py"), "--export", export,
             "--emit", emit],
            capture_output=True, text=True, cwd=tmp,
        )
        self.assertEqual(out.returncode, 0, out.stderr)
        return json.loads(out.stdout) if emit == "json" else out.stdout

    def test_a_matching_name_that_calls_the_module_is_attributed(self):
        with tempfile.TemporaryDirectory() as tmp:
            doc = design([("cmp:heal", "src/heal.rs")])
            r = self.run_in(tmp, doc, {
                "src/heal.rs": "impl G { pub fn propose_heal(&self) {} }\n",
                "tests/heal.rs": "fn t() { g.propose_heal(); }\n",
            }, emit="json")
            self.assertEqual(len(r["proposals"]), 1)
            self.assertEqual(r["proposals"][0]["component"], "cmp:heal")
            self.assertIn("propose_heal", r["proposals"][0]["evidence"])

    def test_a_matching_name_that_calls_nothing_is_refused(self):
        """CLAUSE 2 IS THE WHOLE POINT. Without it this is name-matching, which
        was measured wrong in both directions on reflow2's own graph."""
        with tempfile.TemporaryDirectory() as tmp:
            doc = design([("cmp:heal", "src/heal.rs")])
            r = self.run_in(tmp, doc, {
                "src/heal.rs": "impl G { pub fn propose_heal(&self) {} }\n",
                "tests/heal.rs": "fn t() { something_else(); }\n",
            }, emit="json")
            self.assertEqual(r["proposals"], [])
            self.assertIn("calls nothing that file defines", r["unattributed"][0]["reason"])

    def test_a_test_with_no_name_match_is_reported_not_scored(self):
        """A behaviour-named test gets NO attribution rather than the
        best-scoring one. Guessing here would make the per-subsystem table look
        complete while quietly filing tests under the wrong part."""
        with tempfile.TemporaryDirectory() as tmp:
            doc = design([("cmp:heal", "src/heal.rs")])
            r = self.run_in(tmp, doc, {
                "src/heal.rs": "impl G { pub fn propose_heal(&self) {} }\n",
                "tests/a_deliberate_state_is_not_a_defect.rs": "fn t() { g.propose_heal(); }\n",
            }, emit="json")
            self.assertEqual(r["proposals"], [])
            self.assertIn("no source file of this name", r["unattributed"][0]["reason"])

    def test_an_unbalanced_quote_does_not_swallow_the_code_after_it(self):
        """THE REGRESSION. A file-wide string strip pairs quotes greedily, so one
        raw string or stray quote eats every definition after it — and the tool
        then reports an honest-looking "no evidence found"."""
        with tempfile.TemporaryDirectory() as tmp:
            doc = design([("cmp:heal", "src/heal.rs")])
            r = self.run_in(tmp, doc, {
                "src/heal.rs": '''
                    fn noise() { let s = r#"an unbalanced " lives here"#; }
                    impl G { pub fn propose_heal(&self) {} }
                    ''',
                "tests/heal.rs": "fn t() { g.propose_heal(); }\n",
            }, emit="json")
            self.assertEqual(len(r["proposals"]), 1, "the definition after the quote was lost")

    def test_prose_is_never_evidence(self):
        """A test file's header essay names other modules constantly. Reading it
        as calls would attribute nearly every file to nearly every component."""
        with tempfile.TemporaryDirectory() as tmp:
            doc = design([("cmp:heal", "src/heal.rs")])
            r = self.run_in(tmp, doc, {
                "src/heal.rs": "impl G { pub fn propose_heal(&self) {} }\n",
                "tests/heal.rs": '''
                    //! This test is about propose_heal() in the abstract.
                    // g.propose_heal();
                    fn t() { let s = "propose_heal()"; }
                    ''',
            }, emit="json")
            self.assertEqual(r["proposals"], [])

    def test_an_ambiguous_name_is_refused_rather_than_picked(self):
        with tempfile.TemporaryDirectory() as tmp:
            doc = design([("cmp:a", "a/heal.rs"), ("cmp:b", "b/heal.rs")])
            r = self.run_in(tmp, doc, {
                "a/heal.rs": "pub fn x() {}\n",
                "b/heal.rs": "pub fn y() {}\n",
                "tests/heal.rs": "fn t() { x(); }\n",
            }, emit="json")
            self.assertEqual(r["proposals"], [])
            self.assertIn("ambiguous", r["unattributed"][0]["reason"])

    def test_a_test_the_design_already_knows_is_marked_as_such(self):
        """Registering is not the same as attributing. A file the design has
        heard of may still say nothing about which part it is about, and that
        second gap has to stay visible after the first is closed."""
        with tempfile.TemporaryDirectory() as tmp:
            doc = design([("cmp:heal", "src/heal.rs")])
            doc["nodes"].append({"node_id": "art:t", "node_type": "Artifact",
                                 "properties": {"location": "tests/heal.rs"}})
            r = self.run_in(tmp, doc, {
                "src/heal.rs": "impl G { pub fn propose_heal(&self) {} }\n",
                "tests/heal.rs": "fn t() { g.propose_heal(); }\n",
            }, emit="json")
            self.assertTrue(r["proposals"][0]["already_known"])

    def test_a_design_that_maps_no_source_says_so_rather_than_reporting_clean(self):
        """The vacuous zero. Nothing to attribute a test TO is a statement about
        the DESIGN, and must never read as 'no tests need attributing'."""
        with tempfile.TemporaryDirectory() as tmp:
            out = self.run_in(tmp, design([]), {"tests/heal.rs": "fn t() {}\n"})
            self.assertIn("0 source file(s) are mapped", out)
            self.assertIn("not a clean result", out)

    def test_a_missing_export_is_said_not_assumed_empty(self):
        out = subprocess.run(
            [sys.executable, os.path.join(HERE, "link_tests.py"), "--export", "/nope/x.json"],
            capture_output=True, text=True,
        )
        self.assertEqual(out.returncode, 0)
        self.assertIn("nothing to check against", out.stdout)

    def test_a_source_file_is_never_mistaken_for_a_test(self):
        """`src/heal.rs` must not be offered as a test of itself."""
        with tempfile.TemporaryDirectory() as tmp:
            doc = design([("cmp:heal", "src/heal.rs")])
            r = self.run_in(tmp, doc, {
                "src/heal.rs": "impl G { pub fn propose_heal(&self) {} }\n",
                "tests/heal.rs": "fn t() { g.propose_heal(); }\n",
            }, emit="json")
            self.assertEqual([p["test"] for p in r["proposals"]], ["tests/heal.rs"])

    def test_python_projects_are_read_too(self):
        """Nothing here is Rust-specific — the design says where the code is,
        and the language follows from the extension."""
        with tempfile.TemporaryDirectory() as tmp:
            doc = design([("cmp:thing", "pkg/thing.py")])
            r = self.run_in(tmp, doc, {
                "pkg/thing.py": "def do_the_thing():\n    return 1\n",
                "tests/thing.py": "def t():\n    assert do_the_thing() == 1\n",
            }, emit="json")
            self.assertEqual(len(r["proposals"]), 1)
            self.assertEqual(r["proposals"][0]["component"], "cmp:thing")


if __name__ == "__main__":
    unittest.main(verbosity=2)
