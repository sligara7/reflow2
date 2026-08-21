#!/usr/bin/env python3
"""Design intent moves only on the owner's word — the check for it.

Enforces `rule:design-intent-moves-only-on-the-owners-word`, which Anthony
marked ENFORCED on 2026-08-09 ("if a session recorded a decision as accepted
without you saying so, should that stop the build, or is it advice?" — stop the
build). The rule was gate-blocking for twelve days with nothing checking it.

WHAT COUNTS AS SETTLING INTENT, from the rule's own statement: moving a
Requirement off `proposed`, marking a Decision `accepted`, or recording a
DesignRule's `enforced`. An agent may create, draft, measure, argue and
recommend without limit; it may not do those three on Anthony's behalf.

WHAT MAKES IT HIS WORD: an AUTHORED_BY edge with `role=approver`. That is the
graph saying in structure that a named person signed, rather than prose
claiming it.

GRANDFATHERED, ON HIS INSTRUCTION: "Grandfather it: the check should ignore
everything settled up to now and only guard what gets settled from here. Do not
backfill the 121, and do not put the rule back to advice."

`dec:the-authority-check-guards-forward-not-backward` recorded that ruling on
2026-08-11 and deliberately left the mechanism to build time — "the cutoff epoch
is to be recorded WHEN THE DETECTOR IS BUILT, not now: minting it in advance
would date the rule to a moment nothing enforced." This is that build, and the
boundary is now on that node.

The exempt set is NOT a constant in this file. It is read from the graph,
because which items were already settled is a fact about the design and belongs
in the design — and because a list living here could be edited to silence a
finding without that edit ever surfacing in a design review.

WHY THE SET IS MATERIALISED AND NOT COMPUTED FROM THE EPOCH ALONE. The decision
names a cutoff on the design's own time axis, since a Decision carries no
timestamp. But "settled after the cutoff" can only be evaluated per node if
something recorded WHEN each node settled, and status moves are not reliably
written as ChangeEvents — so an epoch comparison alone would exempt every node
that has no such record, which is exactly the new violations this exists to
catch. The epoch names the boundary; the materialised set is its contents,
computed once at the moment the decision said to compute it.

WHY A SET AND NOT A COUNT. Counting settled-without-approver nodes and failing
above a baseline is smaller and wrong: add an approver to one old node and
introduce one new violation and the count is unchanged, so the check passes
while the thing it exists to catch has happened. Two states that look identical
is the failure mode this whole rule guards, and the check must not reproduce it.

Exit 0 clean, 1 on a violation, 2 if it could not run — never a silent pass.
"""

import json
import sys

RULE_ID = "rule:design-intent-moves-only-on-the-owners-word"
GRANDFATHER_ID = "dec:the-authority-check-guards-forward-not-backward"
GRANDFATHER_FIELD = "grandfathered_ids"


def settles_intent(node):
    """Does this node assert settled intent? (the rule's three cases)"""
    t, p = node["node_type"], node.get("properties", {})
    if t == "Requirement":
        return p.get("status") not in (None, "proposed")
    if t == "Decision":
        return p.get("status") == "accepted"
    if t == "DesignRule":
        return p.get("enforced") is not None
    return False


def load(path):
    with open(path, encoding="utf-8") as fh:
        return json.load(fh)


def check(design):
    nodes = design.get("nodes", [])
    edges = design.get("edges", [])
    by_id = {n["node_id"]: n for n in nodes}

    if RULE_ID not in by_id:
        return 2, [
            f"{RULE_ID} is not in this design. The check cannot run, and a check "
            f"that cannot run must not report success."
        ], []

    ruling = by_id.get(GRANDFATHER_ID)
    if ruling is None:
        return 2, [
            f"{GRANDFATHER_ID} is not in this design, so the grandfathered set "
            f"is unknown. Refusing to guess: treating it as empty would fail the "
            f"build on hundreds of items Anthony explicitly said not to revisit, "
            f"and treating it as everything would pass unconditionally."
        ], []
    if ruling.get("properties", {}).get("status") != "accepted":
        return 2, [
            f"{GRANDFATHER_ID} is not `accepted`. A proposed decision is somebody "
            f"thinking out loud, and a musing must not license an exemption."
        ], []

    raw = ruling.get("properties", {}).get(GRANDFATHER_FIELD)
    if not raw:
        return 2, [f"{GRANDFATHER_ID} carries no `{GRANDFATHER_FIELD}`."], []
    grandfathered = set(json.loads(raw) if isinstance(raw, str) else raw)

    approved = {
        e["from_id"]
        for e in edges
        if e.get("edge_type") == "AUTHORED_BY"
        and (e.get("properties") or {}).get("role") == "approver"
    }

    settled = [n for n in nodes if settles_intent(n)]
    violations = [
        n
        for n in settled
        if n["node_id"] not in approved and n["node_id"] not in grandfathered
    ]
    violations.sort(key=lambda n: n["node_id"])

    stale = sorted(g for g in grandfathered if g not in by_id)
    notes = [
        f"{len(settled)} node(s) assert settled intent; "
        f"{len(approved & {n['node_id'] for n in settled})} carry an approver; "
        f"{len(grandfathered)} are grandfathered."
    ]
    if stale:
        notes.append(
            f"{len(stale)} grandfathered id(s) no longer exist — retired since the "
            f"boundary was drawn, which is expected and is not a violation."
        )
    return (1 if violations else 0), notes, violations


def main(argv):
    if len(argv) != 2:
        print("usage: check_intent_authority.py <export.json>", file=sys.stderr)
        return 2
    try:
        design = load(argv[1])
    except (OSError, ValueError) as exc:
        print(f"FAIL  could not read '{argv[1]}': {exc}", file=sys.stderr)
        return 2

    code, notes, violations = check(design)
    for n in notes:
        print(f"  note  {n}")

    if code == 2:
        for n in notes:
            print(f"FAIL  {n}", file=sys.stderr)
        return 2
    if violations:
        print()
        for v in violations:
            name = v.get("properties", {}).get("name", v["node_id"])
            print(f"  FAIL  {v['node_id']} ({v['node_type']}) — {name[:90]}")
        print(
            f"\nintent authority: FAILED — {len(violations)} node(s) reached a settled "
            f"state with nobody's name on them.\n"
            f"\nThis is `{RULE_ID}`, which Anthony marked ENFORCED. An agent may "
            f"propose anything and settle nothing.\n"
            f"\nTo make it green HONESTLY: ask him, and record his answer with "
            f"`authored_by(role='approver')`. Moving the status back to `proposed` is "
            f"also honest. Adding the id to the grandfathered set is NOT — that set is "
            f"a dated boundary, not a place to put today's work."
        )
        return 1

    print("\nintent authority: OK — every settled node since the boundary carries a name.")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
