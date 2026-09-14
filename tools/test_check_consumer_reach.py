#!/usr/bin/env python3
"""The consumer-reach gate's own regression net."""
from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from check_consumer_reach import check, kit_shipped_tools  # noqa: E402


def design(cap_text, realizer_locations, status="realized"):
    nodes = [
        {"node_id": "cap:x", "node_type": "Capability",
         "properties": {"name": "x", "description": cap_text, "status": status}},
    ]
    edges = []
    for i, loc in enumerate(realizer_locations):
        nodes.append({"node_id": f"art:{i}", "node_type": "Artifact",
                      "properties": {"name": loc, "location": loc}})
        edges.append({"edge_type": "REALIZES", "from_id": f"art:{i}", "to_id": "cap:x"})
    return {"nodes": nodes, "edges": edges}


class ConsumerReach(unittest.TestCase):
    def test_a_claim_realized_only_by_a_tools_script_fails(self):
        self.assertEqual(len(check(design("works on any project", ["tools/x.py"]), shipped=set())), 1)

    def test_the_failure_names_the_claim_and_the_realizers(self):
        [f] = check(design("a reflow2 user gets this", ["tools/x.py", "tools/test_x.py"]), shipped=set())
        self.assertIn("'a reflow2 user'", f)
        self.assertIn("tools/x.py", f)

    def test_a_served_realizer_satisfies_it(self):
        self.assertEqual(check(design("works on any project", ["tools/x.py", "crates/reflow2-mcp/src/x.rs"])), [])

    def test_a_kit_realizer_satisfies_it(self):
        self.assertEqual(check(design("works on any project", ["getting-started/skills/x/SKILL.md"])), [])

    def test_no_claim_means_no_verdict(self):
        self.assertEqual(check(design("a dev-only sweep over this repo", ["tools/x.py"])), [])

    def test_a_planned_capability_is_not_judged(self):
        self.assertEqual(check(design("works on any project", ["tools/x.py"], status="planned")), [])

    def test_a_tools_file_the_release_copies_into_the_kit_counts_as_reachable(self):
        self.assertEqual(check(design("every project on a machine", ["tools/reflow2_install.py"]), shipped={"tools/reflow2_install.py"}), [])
        self.assertEqual(len(check(design("every project on a machine", ["tools/reflow2_install.py"]), shipped=set())), 1)

    def test_the_shipped_set_is_read_off_the_release_workflow(self):
        wf = "steps:\n  run: |\n    cp tools/reflow2_install.py kit-stage/reflow2-kit/tools/\n    cp -r getting-started kit-stage/reflow2-kit/\n"
        self.assertEqual(kit_shipped_tools(wf), {"tools/reflow2_install.py"})
        self.assertIn("tools/reflow2_install.py", kit_shipped_tools())

    def test_nothing_realizing_it_is_a_different_gap(self):
        self.assertEqual(check(design("works on any project", [])), [])


if __name__ == "__main__":
    unittest.main(verbosity=1)
