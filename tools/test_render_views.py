#!/usr/bin/env python3
"""Tests for tools/render_views.py — the viewpoint renderer (BL-40, BL-88).

Hermetic and stdlib-only, like test_init.py and test_loop_nudge.py: each case
writes a hand-built export document to a temp file and runs the renderer as a
subprocess. The *file form* is a pure projection of the JSON and needs no
binary, so these tests are fast and self-contained.

What is pinned is the doctrine, not the HTML's exact shape: **a view is a
projection of the graph** (dec:views-are-projections). The renderer may only
emit what the graph states, and it must CONFESS — loudly, in the page and on
stdout — anything a viewpoint needs but the graph does not supply. A renderer
that quietly filled a gap would be the exact extrapolation the doctrine forbids,
so the confession *is* the contract under test.
"""

from __future__ import annotations

import json
import pathlib
import subprocess
import sys
import tempfile
import unittest

SCRIPT = pathlib.Path(__file__).resolve().parent / "render_views.py"


def node(node_type: str, node_id: str, **props) -> dict:
    return {"node_type": node_type, "node_id": node_id, "properties": props}


def edge(edge_type: str, from_id: str, to_id: str, **props) -> dict:
    return {"edge_type": edge_type, "from_id": from_id, "to_id": to_id,
            "properties": props}


class RenderViews(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory(prefix="render-views-test-")
        self.tmp = pathlib.Path(self._tmp.name)

    def tearDown(self):
        self._tmp.cleanup()

    def render(self, nodes: list[dict], edges: list[dict]):
        """Run the renderer over a hand-built export; return (result, html)."""
        export = self.tmp / "design.json"
        export.write_text(json.dumps(
            {"graph_id": "g", "nodes": nodes, "edges": edges}))
        out = self.tmp / "views.html"
        r = subprocess.run(
            [sys.executable, str(SCRIPT), str(export), "-o", str(out)],
            capture_output=True, text=True, timeout=60,
        )
        html = out.read_text() if out.exists() else ""
        return r, html

    def test_projects_a_project_with_its_counts_and_names(self):
        nodes = [
            node("Project", "proj:x", name="Weather Station"),
            node("Requirement", "req:a", name="Stay offline",
                 statement="Must work without a network."),
            node("Capability", "cap:a", name="Poll sensors",
                 description="reads the outdoor unit"),
        ]
        edges = [edge("SATISFIES", "cap:a", "req:a")]
        r, html = self.render(nodes, edges)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertTrue(html, "the page was written")
        # Only what the graph states — the real names, projected verbatim.
        self.assertIn("Weather Station", html)
        self.assertIn("Poll sensors", html)
        self.assertIn("Stay offline", html)
        self.assertIn("wrote", r.stdout)

    def test_a_requirement_nothing_satisfies_is_confessed_not_invented(self):
        # The functional view must not manufacture a satisfier; it confesses the
        # gap and shows the requirement dangling.
        nodes = [
            node("Project", "proj:x", name="P"),
            node("Requirement", "req:orphan", name="Unmet need",
                 statement="nobody built this"),
        ]
        r, html = self.render(nodes, [])
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn("what satisfies `req:orphan`", r.stdout,
                      "an unsatisfied requirement must be confessed")
        self.assertIn("req:orphan", html, "and shown, never hidden")

    def test_a_satisfied_requirement_is_not_confessed(self):
        # The mirror: when the graph does supply the link, there is nothing to
        # confess about it — the renderer does not cry wolf.
        nodes = [
            node("Project", "proj:x", name="P"),
            node("Requirement", "req:a", name="Met need", statement="s"),
            node("Capability", "cap:a", name="Doer", description="does it"),
        ]
        r, _ = self.render(nodes, [edge("SATISFIES", "cap:a", "req:a")])
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertNotIn("what satisfies `req:a`", r.stdout)

    def test_no_project_node_is_confessed(self):
        r, html = self.render([], [])
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn("no Project node", r.stdout,
                      "an absent Project is a confession, not a crash")
        # Counts still project honestly: zero nodes, zero edges.
        self.assertIn("0", html)

    def test_a_missing_release_view_says_nothing_to_project(self):
        # A viewpoint with no source data states that plainly rather than
        # rendering an empty table that reads as "nothing shipped".
        nodes = [node("Project", "proj:x", name="P")]
        r, html = self.render(nodes, [])
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn("Nothing to project", html,
                      "an empty viewpoint says so instead of extrapolating")

    def test_a_decision_is_projected_verbatim(self):
        # No-extrapolation, positive form: the rationale text appears exactly as
        # the graph holds it, and a string never stated is nowhere in the page.
        nodes = [
            node("Project", "proj:x", name="P"),
            node("Decision", "dec:a", name="Chose RocksDB",
                 decision="Embed the store as a repo file",
                 rationale="a service is operational cost we do not need yet",
                 status="accepted"),
        ]
        r, html = self.render(nodes, [])
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn("a service is operational cost we do not need yet", html)
        self.assertNotIn("ELASTICSEARCH", html.upper().replace("RENDER", ""),
                         "the renderer invents nothing the graph never stated")

    def test_a_broken_export_fails_loud(self):
        # Not a projection error — a malformed document must not render a page
        # that looks fine. It fails rather than inventing structure.
        bad = self.tmp / "bad.json"
        bad.write_text("{ not json at all")
        out = self.tmp / "views.html"
        r = subprocess.run(
            [sys.executable, str(SCRIPT), str(bad), "-o", str(out)],
            capture_output=True, text=True, timeout=60,
        )
        self.assertNotEqual(r.returncode, 0, "a broken export must not pass")


if __name__ == "__main__":
    unittest.main(verbosity=2)
