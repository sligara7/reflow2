#!/usr/bin/env python3
"""Model reflow2's own coherence loop — as a design, in reflow2.

Distinct from the self-host trials, which modelled reflow2's *product* (it has a
detect capability, a heal capability, a component per module). This models
reflow2's **process**: the DAG of how the lifecycle phases feed each other,
including the backward edges where the build teaches the design what it actually
is, and the axis-Z spine that makes those backward edges safe.

The claim under test is reflow2's own: *design and build anything*. Its own
operating model is a design. Can it hold it?

This started as a friction log — its 2026-07-19 run found four ways the answer
was no, raised as BL-37. It is now the **probe** for that item: the flow is
created (`add_flow` / `part_of_flow`), the transitions carry roles (`TRIGGERS`
+ `role`), the cycles are read back as facts (`flow_report` — reported, never
judged), and the phase nudge no longer calls a structured process
concept-without-design. Any friction line below is a regression.

Emits `docs/loop-model.json` (the export) and exits non-zero on friction.

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
    ("cap:p0-intent", "P0 · Capture intent", "Turn what the user wants into Requirements.", 1),
    ("cap:p1-function", "P1 · Decompose function", "Turn intent into Capabilities — the WHAT.", 2),
    ("cap:p2-structure", "P2 · Allocate to structure", "Assign Capabilities to Components — the WHERE.", 3),
    ("cap:p3-realize", "P3 · Realize", "Build Artifacts that deliver the Capabilities.", 4),
    ("cap:p4-verify", "P4 · Verify", "Prove the Capabilities actually work.", 5),
    ("cap:p5-operate", "P5 · Release and operate", "Ship it, run it, learn from it.", 6),
]

# The forward spine, and the feedback edges that are the whole point. The role
# is what BL-37's second friction was about: forward and backward were both
# bare TRIGGERS, and the graph could not tell the direction of influence.
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

        for cid, name, desc, _order in PHASES:
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

        # ---- The process itself (BL-37's write side) ------------------------
        s.call("add_flow", {
            "id": "flow:loop", "name": "The coherence loop",
            "description": "CHANGE → PROPAGATE → DETECT → SURFACE → RESOLVE/HEAL, phase by phase, "
                           "with the build teaching the design backwards.",
            "flow_type": "process",
            "entry_point": "cap:p0-intent", "exit_point": "cap:p5-operate"})
        for cid, _name, _desc, order in PHASES:
            s.call("part_of_flow", {"capability_id": cid, "flow_id": "flow:loop",
                                    "step_order": order})

        # Transitions carry their meaning. Forward feeds; backward is the
        # subject of the model, and each backward edge says what it forces.
        for a, b in FORWARD:
            s.call("create_edge", {"edge_type": "TRIGGERS", "from_type": "Capability",
                                   "from_id": a, "to_type": "Capability", "to_id": b,
                                   "props": {"role": "feeds"}})
        for a, b, why in FEEDBACK:
            s.call("create_edge", {"edge_type": "TRIGGERS", "from_type": "Capability",
                                   "from_id": a, "to_type": "Capability", "to_id": b,
                                   "props": {"role": "forces resync", "evidence": why}})

        # Every phase transition is recorded on Z (cross-cutting, not a step).
        for cid, _n, _d, _o in PHASES:
            s.call("create_edge", {"edge_type": "TRIGGERS", "from_type": "Capability",
                                   "from_id": cid, "to_type": "Capability",
                                   "to_id": "cap:z-change", "props": {"role": "records"}})

        # ---- What does reflow2 make of its own loop? -------------------------
        print("== reflow2 holding its own operating model ==")
        rep = s.call("graph_report")
        print(f"   {rep['total_nodes']} nodes, {rep['gap_count']} gaps, {rep['defect_count']} defects")
        gaps = s.call("detect_gaps")
        gap_sources = sorted({g["gap_source"] for g in gaps})
        print(f"   gaps: {gap_sources}")
        if "concept_without_design" in gap_sources:
            friction.append(
                "concept_without_design fired on a process with a Flow — the phase nudge is "
                "product-shaped again. A process's structure IS its flow (BL-37).")

        flow = s.call("flow_report", {"flow_id": "flow:loop"})
        steps = [st["capability_id"] for st in flow["steps"]]
        print(f"   flow: {' → '.join(s.removeprefix('cap:') for s in steps)}")
        if steps != [cid for cid, _n, _d, _o in PHASES]:
            friction.append(f"flow_report lost the stated step order: {steps}")

        roles = {(t["from_id"], t["to_id"]): t["role"] for t in flow["transitions"]}
        backward = [(a, b) for a, b, _w in FEEDBACK]
        if any(roles.get(t) != "forces resync" for t in backward):
            friction.append(
                "the backward edges are indistinguishable again — TRIGGERS.role did not "
                "survive the round trip (BL-37's second friction).")
        else:
            print(f"   transitions: {len(flow['transitions'])} "
                  f"({sum(1 for r in roles.values() if r == 'feeds')} feed, "
                  f"{len(backward)} force a resync)")

        if not flow["cycles"]:
            friction.append(
                "The feedback loops are INVISIBLE as loops — flow_report returned no cycles "
                "for a deliberately cyclic process (BL-37's third friction).")
        else:
            for c in flow["cycles"]:
                caught = ", ".join(x.removeprefix("cap:") for x in c["members"])
                walk = " -> ".join(x.removeprefix("cap:") for x in c["path"])
                print(f"   cycle (reported, never judged): {{{caught}}}")
                print(f"     one walk through it: {walk}")
            # F7: the walk may be shorter than the cluster, and on this very
            # model it omitted the hand-off to the human — the reason the
            # process is a loop at all. The report must name both.
            missed = [x for c in flow["cycles"] for x in c["members"] if x not in c["path"]]
            if missed:
                print(f"     (the walk omits {', '.join(missed)} — "
                      f"which is why members is the honest answer)")
        defects = s.call("detect_defects")
        if any(d["category"] == "circular_dependency" for d in defects):
            friction.append(
                "a TRIGGERS cycle leaked into detect_defects — the process's design is being "
                "judged as a product defect, against the BL-37 decision.")

        if flow["confessions"]:
            friction.append(f"the model is fully stated, yet the report confesses: {flow['confessions']}")

        doc = s.call("export_graph")
        out = REPO / "docs/loop-model.json"
        out.write_text(json.dumps(doc, indent=2, sort_keys=True) + "\n")
        print(f"\n   exported {len(doc['nodes'])} nodes / {len(doc['edges'])} edges -> {out.relative_to(REPO)}")

    finally:
        s.close()
        shutil.rmtree(tmp, ignore_errors=True)

    if friction:
        print("\n== friction ==")
        for f in friction:
            print(f"  - {f}\n")
        return 1
    print("\n== no friction: the loop can hold its own operating model ==")
    return 0


if __name__ == "__main__":
    sys.exit(main())
