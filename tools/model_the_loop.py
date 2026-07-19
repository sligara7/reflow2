#!/usr/bin/env python3
"""Model reflow2's own coherence loop — as a design, in reflow2.

Distinct from the self-host trials, which modelled reflow2's *product* (it has a
detect capability, a heal capability, a component per module). This models
reflow2's **process**: the DAG of how the lifecycle phases feed each other,
including the backward edges where the build teaches the design what it actually
is, and the axis-Z spine that makes those backward edges safe.

The claim under test is reflow2's own: *design and build anything*. Its own
operating model is a design. Can it hold it?

Emits `docs/loop-model.json` (the export) and prints the friction.

Run:  python3 tools/model_the_loop.py
"""

from __future__ import annotations

import json
import pathlib
import shutil
import sys
import tempfile

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from smoke_mcp import Server  # noqa: E402

REPO = pathlib.Path(__file__).resolve().parent.parent

# The phases, as activities. Each is something the loop DOES.
PHASES = [
    ("cap:p0-intent", "P0 · Capture intent", "Turn what the user wants into Requirements."),
    ("cap:p1-function", "P1 · Decompose function", "Turn intent into Capabilities — the WHAT."),
    ("cap:p2-structure", "P2 · Allocate to structure", "Assign Capabilities to Components — the WHERE."),
    ("cap:p3-realize", "P3 · Realize", "Build Artifacts that deliver the Capabilities."),
    ("cap:p4-verify", "P4 · Verify", "Prove the Capabilities actually work."),
    ("cap:p5-operate", "P5 · Release and operate", "Ship it, run it, learn from it."),
]

# The forward spine, and the feedback edges that are the whole point.
FORWARD = [
    ("cap:p0-intent", "cap:p1-function"),
    ("cap:p1-function", "cap:p2-structure"),
    ("cap:p2-structure", "cap:p3-realize"),
    ("cap:p3-realize", "cap:p4-verify"),
    ("cap:p4-verify", "cap:p5-operate"),
]
FEEDBACK = [
    ("cap:p4-verify", "cap:p3-realize", "a failing test forces a fix — the inner loop"),
    ("cap:p3-realize", "cap:p1-function", "the fix changed behaviour: the FUNCTION must follow"),
    ("cap:p3-realize", "cap:p2-structure", "responsibility moved: the ALLOCATION must follow"),
    ("cap:p5-operate", "cap:p0-intent", "what users actually do teaches what they actually need"),
]


def main() -> int:
    tmp = tempfile.mkdtemp(prefix="reflow2-loop-")
    s = Server(str(REPO / "target/debug/reflow2-mcp"), str(pathlib.Path(tmp) / "graph"))
    friction: list[str] = []
    try:
        s.call("genesis", {"project_id": "proj:loop", "name": "The reflow2 coherence loop",
                           "objective": "Keep every phase in agreement with what was released",
                           "domain": "process"})

        s.call("add_requirement", {
            "id": "req:released-eq-designed", "name": "Released equals designed",
            "statement": "At release, the design must describe what was actually released."})
        s.call("add_requirement", {
            "id": "req:intent-preserved", "name": "Original intent is never lost",
            "statement": "Updating the design to match the build must not erase what was first intended."})
        s.call("add_requirement", {
            "id": "req:coherence-is-measurable", "name": "Coherence is measurable",
            "statement": "A design that matches reality must be distinguishable from one nobody checked."})

        for cid, name, desc in PHASES:
            s.call("add_capability", {"id": cid, "name": name, "description": desc,
                                      "status": "realized"})
            s.call("satisfies", {"from_id": cid, "to_id": "req:released-eq-designed"})

        # The Z spine — what makes the backward edges safe rather than lossy.
        s.call("add_capability", {
            "id": "cap:z-change", "name": "Z · Record the change",
            "description": "Snapshot the prior state at an epoch, then edit. The past is kept, "
                           "so the design may follow the build without losing intent.",
            "status": "realized"})
        s.call("satisfies", {"from_id": "cap:z-change", "to_id": "req:intent-preserved"})

        s.call("add_capability", {
            "id": "cap:reconcile", "name": "Reconcile against reality",
            "description": "Compare what the design claims against what is actually built, "
                           "tested and deployed.",
            "status": "planned"})
        s.call("satisfies", {"from_id": "cap:reconcile", "to_id": "req:coherence-is-measurable"})

        # ---- The DAG itself -------------------------------------------------
        # PART_OF_FLOW is the modelled way to say "these form an ordered
        # process". It needs a Flow node, which has no constructor and no tool.
        try:
            s.call("add_flow", {"id": "flow:loop", "name": "The coherence loop"})
            friction.append("add_flow exists after all")
        except RuntimeError:
            friction.append(
                "NO Flow write side. `Flow` is fully specified in functional.yaml "
                "(flow_type: process/control_flow/decision_flow, entry_point, exit_point) and "
                "PART_OF_FLOW runs Capability -> Flow — but there is no constructor in core and "
                "no MCP tool, so the ONE type meant for 'an ordered process linking capabilities "
                "end to end' cannot be created. This model is the exact use case for it.")

        # Fall back to TRIGGERS (declared * -> *) for the ordering.
        for a, b in FORWARD:
            s.call("create_edge", {"edge_type": "TRIGGERS", "from_type": "Capability",
                                   "from_id": a, "to_type": "Capability", "to_id": b,
                                   "props": {}})
        for a, b, _why in FEEDBACK:
            s.call("create_edge", {"edge_type": "TRIGGERS", "from_type": "Capability",
                                   "from_id": a, "to_type": "Capability", "to_id": b,
                                   "props": {}})
        friction.append(
            "Edge SEMANTICS are lost. Forward 'feeds' and backward 'forces a resync' are both "
            "TRIGGERS, because TRIGGERS is `* -> *` and nothing carries a role. The backward "
            "edges are the entire subject of this model and the graph cannot tell them from the "
            "forward ones.")

        # Every phase transition is recorded on Z.
        for cid, _n, _d in PHASES:
            s.call("create_edge", {"edge_type": "TRIGGERS", "from_type": "Capability",
                                   "from_id": cid, "to_type": "Capability",
                                   "to_id": "cap:z-change", "props": {}})

        # ---- What does reflow2 make of its own loop? -------------------------
        print("== reflow2 holding its own operating model ==")
        rep = s.call("graph_report")
        print(f"   {rep['total_nodes']} nodes, {rep['gap_count']} gaps, {rep['defect_count']} defects")
        gaps = s.call("detect_gaps")
        print(f"   gaps: {sorted({g['gap_source'] for g in gaps})}")
        defects = s.call("detect_defects")
        cats = sorted({d["category"] for d in defects})
        print(f"   defects: {cats}")

        cycles = [d for d in defects if d["category"] == "circular_dependency"]
        if not cycles:
            friction.append(
                "The feedback loops are INVISIBLE as loops. This DAG is deliberately cyclic — "
                "P4 forces P3, P3 updates P1 — and circular_dependency does not fire, because it "
                "walks DEPENDS_ON and contracts, not TRIGGERS. A process model's cycles are its "
                "most important feature and nothing reads them.")
        else:
            print(f"   cycles found: {len(cycles)}")

        doc = s.call("export_graph")
        out = REPO / "docs/loop-model.json"
        out.write_text(json.dumps(doc, indent=2, sort_keys=True) + "\n")
        print(f"\n   exported {len(doc['nodes'])} nodes / {len(doc['edges'])} edges -> {out.relative_to(REPO)}")

    finally:
        s.close()
        shutil.rmtree(tmp, ignore_errors=True)

    print("\n== friction ==")
    for f in friction:
        print(f"  - {f}\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
