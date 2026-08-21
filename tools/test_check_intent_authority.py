#!/usr/bin/env python3
"""Tests for the intent-authority check.

The rule it enforces — `rule:design-intent-moves-only-on-the-owners-word` — is
the one inviolable rule in a one-developer project, and the thing it protects
Anthony from is not another person but an agent writing in his voice. So the
behaviour that matters most here is not that the check finds violations. It is
that it CANNOT PASS QUIETLY: every way of not knowing exits 2, never 0.

Hermetic — builds its own minimal designs, reads nothing from the repo.
"""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
from pathlib import Path

TOOL = Path(__file__).resolve().parent / "check_intent_authority.py"
RULE = "rule:design-intent-moves-only-on-the-owners-word"
RULING = "dec:the-authority-check-guards-forward-not-backward"


def design(nodes, edges=None, grandfathered=("dec:old",), ruling_status="accepted"):
    """A minimal design: the rule, the ruling carrying its boundary, plus nodes."""
    base = [
        {"node_id": RULE, "node_type": "DesignRule", "properties": {"enforced": True}},
        {
            "node_id": RULING,
            "node_type": "Decision",
            "properties": {
                "status": ruling_status,
                "grandfathered_ids": json.dumps(list(grandfathered)),
            },
        },
    ]
    # The rule and the ruling are themselves settled intent, so the fixture
    # signs them — otherwise every test trips over its own scaffolding. (The
    # first draft did exactly that, which is the check working.)
    base_edges = [
        {
            "edge_type": "AUTHORED_BY",
            "from_id": n,
            "to_id": "who:ajs",
            "properties": {"role": "approver"},
        }
        for n in (RULE, RULING)
    ]
    return {"nodes": base + list(nodes), "edges": base_edges + list(edges or [])}


def run(doc):
    with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as fh:
        json.dump(doc, fh)
        path = fh.name
    return subprocess.run(
        [sys.executable, str(TOOL), path], capture_output=True, text=True
    )


def approver(node_id, who="who:ajs"):
    return {
        "edge_type": "AUTHORED_BY",
        "from_id": node_id,
        "to_id": who,
        "properties": {"role": "approver"},
    }


def settled_decision(node_id="dec:new"):
    return {
        "node_id": node_id,
        "node_type": "Decision",
        "properties": {"status": "accepted", "name": node_id},
    }


# ---- the two directions ---------------------------------------------------


def test_a_settled_decision_with_no_approver_fails():
    r = run(design([settled_decision()]))
    assert r.returncode == 1, r.stdout + r.stderr
    assert "dec:new" in r.stdout


def test_a_settled_decision_with_an_approver_passes():
    r = run(design([settled_decision()], edges=[approver("dec:new")]))
    assert r.returncode == 0, r.stdout + r.stderr


def test_a_grandfathered_node_is_exempt():
    """Anthony's actual instruction: do not reopen what came before."""
    old = settled_decision("dec:old")
    r = run(design([old], grandfathered=("dec:old",)))
    assert r.returncode == 0, r.stdout + r.stderr


def test_the_other_two_ways_of_settling_intent_are_covered():
    """The rule names three acts, not one — a requirement off `proposed` and a
    rule's `enforced` are settlings too, and an early draft that checked only
    Decisions would have passed on both."""
    req = {
        "node_id": "req:new",
        "node_type": "Requirement",
        "properties": {"status": "accepted", "name": "req:new"},
    }
    assert run(design([req])).returncode == 1
    assert run(design([req], edges=[approver("req:new")])).returncode == 0

    rule = {
        "node_id": "rule:new",
        "node_type": "DesignRule",
        "properties": {"enforced": False, "name": "rule:new"},
    }
    assert run(design([rule])).returncode == 1, "enforced=false is still a ruling"
    assert run(design([rule], edges=[approver("rule:new")])).returncode == 0


def test_a_proposed_requirement_is_not_a_settling():
    req = {
        "node_id": "req:draft",
        "node_type": "Requirement",
        "properties": {"status": "proposed", "name": "req:draft"},
    }
    assert run(design([req])).returncode == 0, "an agent may propose anything"


def test_a_reviewer_edge_is_not_an_approver_edge():
    """`role` is load-bearing: a review is not a signature."""
    e = approver("dec:new")
    e["properties"]["role"] = "reviewer"
    assert run(design([settled_decision()], edges=[e])).returncode == 1


# ---- it must never pass quietly -------------------------------------------


def test_a_missing_rule_exits_two_not_zero():
    doc = design([])
    doc["nodes"] = [n for n in doc["nodes"] if n["node_id"] != RULE]
    r = run(doc)
    assert r.returncode == 2, "no rule means it cannot run, not that all is well"


def test_a_missing_ruling_exits_two_not_zero():
    """Without the boundary, 'exempt everything' and 'exempt nothing' are both
    wrong, and guessing either would be a lie in one direction."""
    doc = design([settled_decision()])
    doc["nodes"] = [n for n in doc["nodes"] if n["node_id"] != RULING]
    assert run(doc).returncode == 2


def test_a_ruling_that_is_only_proposed_exits_two():
    """A proposed decision is somebody thinking out loud; a musing must not
    license an exemption."""
    doc = design([settled_decision()], ruling_status="proposed")
    assert run(doc).returncode == 2


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
    print(f"\ncheck_intent_authority: OK — {len(fns)} test(s) passed.")


if __name__ == "__main__":
    main()
