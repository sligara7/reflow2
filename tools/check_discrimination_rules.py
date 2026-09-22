#!/usr/bin/env python3
"""The schema's discrimination rules and capture-intent's routing table are ONE
contract, and this is what stops them drifting apart.

⭐ WHY THIS EXISTS. flo2 F24: `describe_schema` served the node TYPES and not the
rules that tell them apart, so a consumer generating an extraction prompt from
the schema got six type names and nothing that separates them. On 2026-09-19
that cost a real misclassification on a live site — "the water can never drop
below 68 degrees" filed as a Requirement, which is a numeric prohibition and
therefore a Constraint.

The fix put those rules in the schema, per type, as `discrimination`. That
created the hazard this gate closes: the SAME rules now exist in two places —
the schema (for a consumer) and capture-intent's markdown routing table (for an
agent reading a skill). Two hand-kept records of one contract is exactly the
defect BL-159 was filed for, and `skill_lint.py` already keeps AGENTS.md and
ci.yml honest for the same reason. This is that, for the routing table.

🛑 AND THE ORDER MATTERS, WHICH IS THE LESSON OF THE CHANGE THAT ADDED IT. Found
2026-09-22 while lifting the table into the schema: the table CONTRADICTED
ITSELF. Row 1 routed "it must never…" to Requirement while row 7 routed
"we must never…" to Constraint saying "Not a Requirement" — so the rule turned
on the pronoun, and flo2's sentence matched the wrong row. Serving the table
unchanged would have propagated a wrong rule to every consumer at once. The
rules were corrected first, then served, and this gate is what keeps them
corrected in both places.

WHAT IT CHECKS
  1. Every type the routing table routes to declares `discrimination` in the
     merged schema, with at least one cue and at least one confused_with.
  2. Every `confused_with.type` names a node type that actually exists — a
     discriminator pointing at a type that is not there is worse than none.
  3. Every cue phrase the schema declares appears in the routing table, so the
     agent's prose and the consumer's data say the same thing.

⚠️ RUNNING ZERO CHECKS IS A FAILURE, NOT A PASS — same rule as every other
instrument here. If the table cannot be parsed or no type carries a block, that
is a red build and not a clean one.

Usage (from the repo root):

    python3 tools/check_discrimination_rules.py

Exits 0 when the two records agree, 1 on any disagreement or on having checked
nothing. Requires PyYAML, which ci.yml already installs for validate_schema.py.
"""

from __future__ import annotations

import glob
import os
import pathlib
import re
import sys

try:
    import yaml
except ModuleNotFoundError:
    sys.exit("PyYAML required: pip install pyyaml")

REPO = pathlib.Path(__file__).resolve().parent.parent
SCHEMA_DIR = REPO / "schema"
SKILL = REPO / "getting-started" / "skills" / "capture-intent" / "SKILL.md"

failures: list[str] = []
checked = 0


def fail(msg: str) -> None:
    failures.append(msg)
    print(f"  FAIL  {msg}")


def ok(msg: str) -> None:
    print(f"  ok    {msg}")


def merged_node_types() -> dict:
    """Every node type across the schema domains, later files overlaying."""
    types: dict = {}
    for path in sorted(glob.glob(os.path.join(SCHEMA_DIR, "*.yaml"))):
        with open(path) as fh:
            doc = yaml.safe_load(fh) or {}
        for name, defn in ((doc.get("schema") or {}).get("node_types") or {}).items():
            types.setdefault(name, {}).update(defn or {})
    return types


def routing_table() -> str:
    """The routing table's text, lowercased. Raises if it cannot be found."""
    body = SKILL.read_text()
    rows = [ln for ln in body.splitlines() if ln.startswith("| ")]
    if len(rows) < 8:
        raise SystemExit(
            f"could not find the routing table in {SKILL} — {len(rows)} table row(s). "
            "This gate cannot pass having checked nothing; fix the parse or the table."
        )
    return "\n".join(rows).lower()


def main() -> int:
    global checked

    table = routing_table()
    types = merged_node_types()

    # The types the table actually routes to: the bolded target in each row.
    routed = sorted(set(re.findall(r"\*\*([A-Z][A-Za-z]+)\*\*", SKILL.read_text())) & set(types))
    if not routed:
        raise SystemExit(
            "the routing table names no node type this schema declares — nothing to check, "
            "which is a failure and not a pass"
        )

    for name in routed:
        d = (types.get(name) or {}).get("discrimination")
        checked += 1
        if not d:
            fail(
                f"{name} is routed to by the capture-intent table and declares no "
                f"`discrimination` block, so a consumer reading the schema cannot tell it "
                f"from its neighbour"
            )
            continue

        cues = d.get("cues") or []
        confused = d.get("confused_with") or []
        if not cues:
            fail(f"{name}: discrimination declares no cues")
        if not confused:
            fail(f"{name}: discrimination names no type it is confused with")

        for c in confused:
            other = (c or {}).get("type")
            if other not in types:
                fail(
                    f"{name}: confused_with names `{other}`, which is not a declared node "
                    f"type — a discriminator pointing at a type that does not exist is "
                    f"worse than none"
                )
            if not (c or {}).get("why"):
                fail(f"{name}: confused_with `{other}` gives no reason they differ")

        # The drift check: the agent's table and the consumer's data must agree.
        for cue in cues:
            # Compare on the distinctive head of the cue, before any em-dash
            # gloss, and without the trailing ellipsis the table writes.
            head = cue.split("—")[0].strip().strip("…").strip().lower()
            if len(head) < 4:
                continue
            checked += 1
            if head not in table:
                fail(
                    f"{name}: cue {cue!r} is in the schema and NOT in the routing table. "
                    f"The two records of this contract have drifted; change both or neither"
                )

    print()
    print(f"checked {checked} rule(s) across {len(routed)} routed type(s): {', '.join(routed)}")
    if checked == 0:
        print("discrimination rules: FAILED — checked nothing, which is not a pass")
        return 1
    if failures:
        print(f"discrimination rules: FAILED — {len(failures)} disagreement(s)")
        return 1
    print("discrimination rules: OK — the schema and the routing table say the same thing")
    return 0


if __name__ == "__main__":
    sys.exit(main())
