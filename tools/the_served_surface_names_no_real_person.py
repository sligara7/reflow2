#!/usr/bin/env python3
"""Nothing reflow2 SERVES may name a real person from reflow2's own design.

⭐ WHY THIS GATE EXISTS. Anthony asked on 2026-09-20 whether he was somehow
turning up in other people's reflow2 projects by default. Measured against the
SHIPPED v0.65.0 binary: the kit tarball contains the string "ajs" zero times,
the server never creates a Contributor by itself and never derives one from the
environment, the git config or the OS user — and the binary nonetheless carried
his real id in four places, all of them text served to every client. The worst
of them is the worked example on the exact field that creates contributors:

    add_contributor.id — "Stable id (e.g. `who:ajs`, `who:claude-code`)."

`who:ajs` is Anthony Sligar, a real row in this project's own contributor
table, and that line is the only example an agent reads at the moment it
invents a contributor id. `@ajs` was offered the same way on `handle`, and
`describe_schema` repeated `who:ajs` on `Constraint.limit_source`.

🛑 THE CLASS, AND IT WAS VISIBLE IN THE SAME FILE. Twelve lines below the Grok
alias in `reflow2_init.py` sits the Copilot claim, recorded as measured end to
end. The examples in the served surface were drawn from whatever real project
was at hand — `art:ifc-shell-model` and `ifc_quantify` in `limit_source` are
bhome's artifact and an IfcMCP tool, a *different* customer's identifiers
shipped to everybody. reflow2 is a wrench for other people's projects, and its
worked examples were coming from the hands that made it.

⚠️ WHAT THIS CAN AND CANNOT SEE. It reads this project's own committed export
for Contributors whose `kind` is `person`, and fails if any of their id, handle
or display name appears in text reflow2 serves. It therefore protects the
people this design knows about, which is the population that can actually leak
from here. It CANNOT know that `art:ifc-shell-model` belongs to somebody else,
because bhome's design is not in this graph — those were removed by hand and
nothing here would catch their return. A placeholder convention is the only
thing that covers that case, and it is stated in AGENTS.md rather than checked.

WHAT COUNTS AS SERVED: the committed toolsnaps (goldens of `tools/list`, which
`toolsnap.py` already keeps in step with the real surface), the schema files
`describe_schema` reads out, and the skill bodies `get_skill` returns.

Usage (from the repo root):

    python3 tools/the_served_surface_names_no_real_person.py

Exits 0 when no served text names a real person, 1 on any hit — or on having
found no people to check, because a gate with nothing to look for reads exactly
like one that ran clean.
Standard library only.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent

# Display names too short or too common to match on without drowning the gate
# in false hits. A first name of three characters or fewer is not evidence of
# anything, and refusing to guess is better than a check nobody can keep green.
MIN_NAME_CHARS = 4


def real_people(export: pathlib.Path) -> dict[str, list[str]]:
    """{needle: [why]} for every Contributor of kind `person` in the design."""
    doc = json.loads(export.read_text(encoding="utf-8"))
    needles: dict[str, list[str]] = {}
    for node in doc.get("nodes", []):
        if node.get("node_type") != "Contributor":
            continue
        props = node.get("properties") or {}
        if props.get("kind") != "person":
            continue
        who = props.get("name") or node["node_id"]
        needles.setdefault(node["node_id"], []).append(f"the id of {who}")
        if handle := props.get("handle"):
            needles.setdefault(handle, []).append(f"the handle of {who}")
        name = props.get("name") or ""
        if len(name) >= MIN_NAME_CHARS:
            needles.setdefault(name, []).append(f"the display name of {who}")
    return needles


def served_texts() -> list[tuple[str, str]]:
    """[(label, text)] — everything reflow2 puts in front of another project."""
    out: list[tuple[str, str]] = []
    for snap in sorted((REPO / "tools" / "toolsnaps").glob("*.json")):
        out.append((f"served tool schema {snap.name}", snap.read_text(encoding="utf-8")))
    for schema in sorted((REPO / "schema").glob("*.yaml")):
        out.append((f"describe_schema reads {schema.name}", schema.read_text(encoding="utf-8")))
    # getting-started/skills is the SOURCE; .claude/skills and .grok/skills are
    # byte-identical mirrors skill_lint already keeps in step, and the kit ships
    # the source. Scanning a mirror would pass while the shipped copy leaked.
    for skill in sorted((REPO / "getting-started" / "skills").glob("*/SKILL.md")):
        out.append((f"get_skill serves {skill.parent.name}", skill.read_text(encoding="utf-8")))
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--export", default="docs/design/reflow2.json")
    a = ap.parse_args()

    export = REPO / a.export
    if not export.exists():
        print(f"FAIL: no export at {export} to read this design's people from.", file=sys.stderr)
        return 1

    needles = real_people(export)
    if not needles:
        print(
            "FAIL: this design records no Contributor of kind `person`, so the gate had "
            "nobody to look for and checked nothing. That is not a pass.",
            file=sys.stderr,
        )
        return 1

    texts = served_texts()
    if len(texts) < 50:
        print(
            f"FAIL: found only {len(texts)} served text(s) — that is a broken read of the "
            f"surface, not a small surface.",
            file=sys.stderr,
        )
        return 1

    hits: list[str] = []
    for label, text in texts:
        for needle, whys in needles.items():
            # Word-boundaried so `who:alex` does not fire on `alexander`, and a
            # display name does not fire inside a longer word.
            if re.search(rf"(?<![\w:@-]){re.escape(needle)}(?![\w-])", text):
                line = next(
                    (l.strip() for l in text.splitlines() if needle in l), ""
                )
                hits.append(f"{label}: {needle} — {whys[0]}\n      {line[:160]}")

    print(f"checked {len(texts)} served text(s) against {len(needles)} needle(s) "
          f"from {sum(1 for n in needles.values() if n[0].startswith('the id'))} real person(ies)")

    if hits:
        print(
            f"\nFAIL: reflow2 serves text naming a real person — {len(hits)} hit(s). "
            f"Every project reflow2 is installed into receives this.",
            file=sys.stderr,
        )
        for h in hits:
            print(f"  {h}", file=sys.stderr)
        print(
            "\n  Use a placeholder. The house convention is the type prefix with a slug —\n"
            "  `who:<slug>` — and an agent id like `who:claude-code` is fine, because the\n"
            "  population this protects is PEOPLE.",
            file=sys.stderr,
        )
        return 1

    print("OK: nothing reflow2 serves names a real person from this design.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
