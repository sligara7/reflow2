#!/usr/bin/env python3
"""Tests for the verification ratchet.

What matters most here is not that the check finds a new claim. It is the two
ways it must not lie:

  · it CANNOT PASS QUIETLY — no ruling, no baseline, an unsettled ruling or an
    export with no capabilities all exit 2, never 0. A detector reporting zero
    because it had nothing to run on reads exactly like one that ran clean.
  · it FIRES ON THE CLAIM, NOT THE ABSENCE — a capability with no check at all
    is unverified and honest, and must never turn a build red. Getting that
    wrong would make capture-intent, which records intent before anything is
    built, into a gate failure.

Hermetic — builds its own minimal designs, reads nothing from the repo.
"""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
from pathlib import Path

TOOL = Path(__file__).resolve().parent / "check_verification_ratchet.py"
RULING = "dec:a-capability-may-not-newly-reach-verified-on-a-claim-alone"


def design(nodes, edges=None, baseline=("cap:old",), ruling_status="accepted"):
    """A minimal design: the ruling carrying the baseline, plus nodes."""
    base = [
        {
            "node_id": RULING,
            "node_type": "Decision",
            "properties": {
                "status": ruling_status,
                "baseline_ids": json.dumps(list(baseline)),
            },
        }
    ]
    return {"nodes": base + list(nodes), "edges": list(edges or [])}


def capability(cap_id, check_status=None, implemented=False):
    """A capability, optionally with a check at `check_status` behind it."""
    nodes = [{"node_id": cap_id, "node_type": "Capability", "properties": {}}]
    edges = []
    if check_status is not None:
        ver = f"ver:{cap_id}"
        nodes.append(
            {
                "node_id": ver,
                "node_type": "Verification",
                "properties": {"status": check_status},
            }
        )
        edges.append({"edge_type": "VERIFIES", "from_id": ver, "to_id": cap_id})
        if implemented:
            art = f"art:{cap_id}"
            nodes.append({"node_id": art, "node_type": "Artifact", "properties": {}})
            edges.append({"edge_type": "IMPLEMENTS", "from_id": art, "to_id": ver})
    return nodes, edges


def run(doc):
    with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as fh:
        json.dump(doc, fh)
        path = fh.name
    try:
        return subprocess.run(
            [sys.executable, str(TOOL), path], capture_output=True, text=True
        )
    finally:
        Path(path).unlink()


def test_a_new_claim_only_capability_fails():
    nodes, edges = capability("cap:new", check_status="passing")
    r = run(design(nodes, edges))
    assert r.returncode == 1, r.stdout
    assert "cap:new" in r.stdout


def test_a_grandfathered_claim_only_capability_passes():
    nodes, edges = capability("cap:old", check_status="passing")
    r = run(design(nodes, edges, baseline=("cap:old",)))
    assert r.returncode == 0, r.stdout


def test_a_new_capability_with_a_runnable_check_passes():
    nodes, edges = capability("cap:new", check_status="passing", implemented=True)
    r = run(design(nodes, edges))
    assert r.returncode == 0, r.stdout


def test_a_capability_with_no_check_at_all_passes():
    """Unverified is honest. Only the CLAIM is forbidden — otherwise every
    freshly captured intent would turn the build red."""
    nodes, edges = capability("cap:new")
    r = run(design(nodes, edges))
    assert r.returncode == 0, r.stdout


def test_an_implemented_but_failing_check_is_not_a_claim_either():
    """A failing check is not a claim of verification, so it is not this
    gate's business — the coverage count already refuses to count it."""
    nodes, edges = capability("cap:new", check_status="failing", implemented=True)
    r = run(design(nodes, edges))
    assert r.returncode == 0, r.stdout


def test_a_grandfathered_capability_that_earned_a_check_is_noted_not_failed():
    nodes, edges = capability("cap:old", check_status="passing", implemented=True)
    r = run(design(nodes, edges, baseline=("cap:old",)))
    assert r.returncode == 0, r.stdout
    assert "can be pruned" in r.stdout


def test_no_ruling_exits_two():
    doc = {"nodes": [{"node_id": "cap:x", "node_type": "Capability", "properties": {}}],
           "edges": []}
    assert run(doc).returncode == 2


def test_an_unsettled_ruling_exits_two():
    nodes, edges = capability("cap:new", check_status="passing")
    assert run(design(nodes, edges, ruling_status="proposed")).returncode == 2


def test_an_empty_baseline_grandfathers_nothing_rather_than_refusing():
    """Zero exemptions is what success looks like — the set may only shrink —
    so an empty baseline must still RUN. A field that was emptied by accident
    then fails loud, every claim at once, which is the safe direction."""
    nodes, edges = capability("cap:new", check_status="passing")
    r = run(design(nodes, edges, baseline=()))
    assert r.returncode == 1, r.stdout
    assert "cap:new" in r.stdout


def test_a_missing_baseline_field_exits_two():
    """Absent is not empty. A ruling that lost its set cannot excuse anything
    and must not be read as excusing nothing."""
    doc = {
        "nodes": [
            {"node_id": RULING, "node_type": "Decision", "properties": {"status": "accepted"}},
            {"node_id": "cap:x", "node_type": "Capability", "properties": {}},
        ],
        "edges": [],
    }
    assert run(doc).returncode == 2


def test_an_export_with_no_capabilities_exits_two():
    """Checked nothing is not a pass."""
    assert run(design([])).returncode == 2


def test_an_unreadable_export_exits_two():
    r = subprocess.run(
        [sys.executable, str(TOOL), "/nonexistent/nope.json"],
        capture_output=True,
        text=True,
    )
    assert r.returncode == 2


def main():
    fns = [v for k, v in sorted(globals().items()) if k.startswith("test_")]
    for fn in fns:
        fn()
        print(f"  ok  {fn.__name__}")
    print(f"\ncheck_verification_ratchet: OK — {len(fns)} test(s) passed.")


if __name__ == "__main__":
    main()
