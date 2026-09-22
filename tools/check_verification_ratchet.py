#!/usr/bin/env python3
"""A capability may not NEWLY reach "verified" on a check nothing can re-run.

⭐ WHY THIS EXISTS, WITH THE NUMBER. Measured on this design 2026-09-22, off the
committed export: 291 capabilities, 241 "verified" by their own passing check —
and **21** of those by a check anything IMPLEMENTS. The other 220 rest on an
assertion nothing can re-run. Of 312 Verifications, 311 read `passing` and 30
have an executable form.

The coverage line said "241/288 capability(ies) verified", and a reader met that
as a thoroughly verified design. The code underneath was genuinely at 85% line
coverage: the TESTS are real, the RECORD of them was not, and that gap is the
half of an outside reader's "this sounds like an AI coding project" that lands.

WHAT IS FORBIDDEN IS THE CLAIM, NOT THE ABSENCE. A capability with no check at
all is unverified and honest, and capture-intent must stay free to record intent
before anything is built. This fires only on a capability asserting a PASSING
check that nothing implements — and only on one that was not already doing so on
2026-09-22.

WHY A SET AND NOT A COUNT — the argument `check_intent_authority.py` records for
its own grandfathered set, and it applies unchanged here. Counting claim-only
capabilities and failing above a baseline is smaller and wrong: implement a
check for one old capability and introduce one new claim, and the count does not
move, so the gate passes while the thing it exists to catch has happened. Two
states that look identical is the whole failure being guarded.

WHY THE BASELINE LIVES IN THE GRAPH — likewise. Which capabilities were already
claim-only is a fact about the design and belongs in the design; a list living
in this file could be edited to silence a finding without that edit ever
surfacing in a design review. It is read from
`dec:a-capability-may-not-newly-reach-verified-on-a-claim-alone`.

🛑 THIS DOES NOT JUDGE WHETHER A CHECK IS ANY GOOD.
`dec:non-goal-reflow2-does-not-judge-whether-a-check-is-meaningful` stands
untouched. It reports a fact the graph already holds — the same signal
`has_executable_form` has carried in the loop digest all along — and refuses to
let that fact get worse.

⚠️ RUNNING ZERO CHECKS IS A FAILURE, NOT A PASS. An export with no capabilities,
or a ruling with no baseline, exits non-zero rather than reporting a clean
sweep. A detector reporting zero because it had nothing to run on reads exactly
like one that ran clean.

Usage (from the repo root):

    python3 tools/check_verification_ratchet.py docs/design/reflow2.json

Exit 0 clean, 1 on a new claim-only capability, 2 if it could not run.
"""

from __future__ import annotations

import collections
import json
import sys

RULING_ID = "dec:a-capability-may-not-newly-reach-verified-on-a-claim-alone"
BASELINE_FIELD = "baseline_ids"


def load(path: str) -> dict:
    with open(path) as fh:
        return json.load(fh)


def claim_only(design: dict) -> tuple[set[str], int, int]:
    """Capabilities whose only passing check has no executable form.

    Returns (claim_only_ids, runnable_count, capability_count). The two counts
    come back so the report can say what it swept — a bare verdict cannot be
    told from a verdict on nothing.
    """
    nodes = {n["node_id"]: n for n in design.get("nodes", [])}
    verifies: dict[str, list[str]] = collections.defaultdict(list)
    implemented: set[str] = set()
    for e in design.get("edges", []):
        kind = e.get("edge_type")
        if kind == "VERIFIES":
            verifies[e["to_id"]].append(e["from_id"])
        elif kind == "IMPLEMENTS":
            implemented.add(e["to_id"])

    def passing(vid: str) -> bool:
        n = nodes.get(vid)
        return bool(n) and (n.get("properties") or {}).get("status") == "passing"

    claims: set[str] = set()
    runnable = 0
    caps = 0
    for n in design.get("nodes", []):
        if n.get("node_type") != "Capability":
            continue
        caps += 1
        checks = verifies.get(n["node_id"], [])
        if not any(passing(v) for v in checks):
            continue  # unverified, and honest about it
        if any(passing(v) and v in implemented for v in checks):
            runnable += 1
        else:
            claims.add(n["node_id"])
    return claims, runnable, caps


def check(design: dict) -> tuple[int, list[str]]:
    ruling = next(
        (n for n in design.get("nodes", []) if n["node_id"] == RULING_ID), None
    )
    if ruling is None:
        return 2, [
            f"the export holds no `{RULING_ID}` — the ruling that sets this bar and "
            f"carries its baseline. Nothing to check against, which is a failure and "
            f"not a pass."
        ]
    if (ruling.get("properties") or {}).get("status") != "accepted":
        return 2, [
            f"{RULING_ID} is not accepted. A bar nobody settled must not stop a build."
        ]
    props = ruling.get("properties") or {}
    if BASELINE_FIELD not in props:
        return 2, [f"{RULING_ID} carries no `{BASELINE_FIELD}`."]
    raw = props[BASELINE_FIELD]
    try:
        baseline = set(json.loads(raw) if isinstance(raw, str) else raw)
    except (TypeError, ValueError) as exc:
        return 2, [f"{RULING_ID}'s `{BASELINE_FIELD}` is not a list of ids: {exc}"]
    # An EMPTY baseline is a legitimate state and not a broken one: the set may
    # only shrink, so reaching zero is what success looks like, and refusing to
    # run then would break the gate at the moment it finally had nothing to
    # excuse. A field that vanished instead exits 2 above. A field that was
    # emptied by accident therefore fails LOUD — every claim-only capability at
    # once — which is the safe direction for this mistake to fall.
    baseline = baseline

    claims, runnable, caps = claim_only(design)
    if caps == 0:
        return 2, ["the export holds no Capability — checked nothing, which is not a pass."]

    new = sorted(claims - baseline)
    earned = sorted(baseline - claims)

    print(f"  swept {caps} capability(ies): {runnable} verified by a check something re-runs, "
          f"{len(claims)} by a claim nothing implements")
    print(f"  baseline grandfathers {len(baseline)}")

    notes: list[str] = []
    if earned:
        # NOT a failure. The ratchet only forbids growth; shrinking is the
        # point. Pruning these keeps the exemption honest, and the honest limit
        # of not forcing it is recorded on the ruling.
        notes.append(
            f"{len(earned)} grandfathered capability(ies) have since earned a runnable "
            f"check and can be pruned from `{BASELINE_FIELD}`: "
            + ", ".join(earned[:8])
            + ("" if len(earned) <= 8 else f", and {len(earned) - 8} more")
        )
    for n in notes:
        print(f"  note  {n}")

    if new:
        print()
        for c in new:
            print(f"  FAIL  {c}")
        print(
            f"\nverification ratchet: FAILED — {len(new)} capability(ies) newly claim a "
            f"passing check that nothing can re-run.\n"
            f"\nTo make it green HONESTLY: name the file that IS the check — the test, the "
            f"script, the tool call that measures it — with `link_artifact`, and draw an "
            f"IMPLEMENTS edge from that artifact to the Verification. `link_artifact` "
            f"draws REALIZES and only REALIZES, so the edge is a second, separate call.\n"
            f"\nMoving the check off `passing` is also honest — an unverified capability is "
            f"a true statement. Adding the id to `{BASELINE_FIELD}` is NOT: that set is a "
            f"dated boundary, not a place to put today's work."
        )
        return 1, notes

    print("\nverification ratchet: OK — no capability newly claims a check nothing can re-run.")
    return 0, notes


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print("usage: check_verification_ratchet.py <export.json>", file=sys.stderr)
        return 2
    try:
        design = load(argv[1])
    except (OSError, ValueError) as exc:
        print(f"FAIL  could not read '{argv[1]}': {exc}", file=sys.stderr)
        return 2

    code, notes = check(design)
    if code == 2:
        for n in notes:
            print(f"FAIL  {n}", file=sys.stderr)
    return code


if __name__ == "__main__":
    sys.exit(main(sys.argv))
