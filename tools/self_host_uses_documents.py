#!/usr/bin/env python3
"""Guard reflow2's own graph against losing the only case that proves [BL-176].

WHY THIS EXISTS, and it is the point of [BL-199] rather than a tidiness rule.

`orphan_node` used to count outgoing `REALIZES` and nothing else, so an Artifact
filed the way the served **link-artifacts** skill prescribes — a design doc with
`DOCUMENTS`, a machine-readable contract with `SPECIFIES` — reported as an
orphan. A real user hit it at 26 of ~756 documents (defects 13 -> 39, false
positives 46% -> 82%) and stopped work.

reflow2 could not have found that bug on itself. Its own graph attached **32 of
its 35** document and spec artifacts with `REALIZES`, against the instruction its
own skill gives, so it never once did the thing that fires the defect. Eleven
releases of `0 gaps, 0 defects, loop clean` were true and uninformative.

**Self-host is blind wherever the self-host graph does not follow the practice
the skills prescribe** — both sides of every instrument's comparison were built
by the same hand. This check exists so that blindness cannot come back silently:
it fails if reflow2's own design record stops exercising `DOCUMENTS`.

It is deliberately a check on the RECORD, not on the code. `tests/orphan_attachment.rs`
proves the detector behaves; this proves this repo still contains the shape that
would catch the detector regressing. Both are needed and neither replaces the
other — that is the whole lesson of BL-199.
"""

from __future__ import annotations

import json
import sys
from collections import defaultdict
from pathlib import Path

# Edges that do NOT attach an Artifact — bookkeeping drawn by the machinery
# rather than by anyone saying what a file is FOR. Kept in step with
# ARTIFACT_BOOKKEEPING in crates/reflow2-core/src/heal.rs.
BOOKKEEPING = {"INCLUDES", "CHANGED", "YIELDED", "AT_EPOCH"}

# Edges that say what a document is about.
DESCRIBES = {"DOCUMENTS", "SPECIFIES"}

# The floor, not a target. At the time of writing 19 of 35 document/spec
# artifacts carry a describing edge; requiring a handful means an honest
# re-model can move nodes around without tripping this, while dropping to
# REALIZES-only cannot pass.
MIN_DESCRIBED = 5


def main(argv: list[str]) -> int:
    export = Path(argv[1] if len(argv) > 1 else "docs/design/reflow2.json")
    if not export.is_file():
        print(f"FAIL: no export at {export}", file=sys.stderr)
        return 2

    doc = json.loads(export.read_text())
    artifacts = {
        n["node_id"]: n.get("properties", {})
        for n in doc["nodes"]
        if n["node_type"] == "Artifact"
    }
    outgoing: dict[str, list[str]] = defaultdict(list)
    incident: dict[str, list[str]] = defaultdict(list)
    for e in doc["edges"]:
        if e["from_id"] in artifacts:
            outgoing[e["from_id"]].append(e["edge_type"])
            incident[e["from_id"]].append(e["edge_type"])
        if e["to_id"] in artifacts:
            incident[e["to_id"]].append(e["edge_type"])

    docspec = [
        a
        for a, p in artifacts.items()
        if p.get("artifact_type") in ("document", "spec")
    ]
    described = [
        a for a in docspec if any(t in DESCRIBES for t in outgoing[a])
    ]
    # Artifacts that would be FALSELY reported by the pre-BL-176 rule. Their
    # existence is what makes this repo able to catch that regression at all.
    canaries = sorted(a for a in described if "REALIZES" not in outgoing[a])
    # The real orphans under the shipped rule — must stay empty here.
    orphans = sorted(
        a for a in artifacts if not any(t not in BOOKKEEPING for t in incident[a])
    )

    print(f"artifacts {len(artifacts)} · document/spec {len(docspec)}")
    print(f"  carrying DOCUMENTS or SPECIFIES ... {len(described)}")
    print(f"  attached ONLY by a describing edge  {len(canaries)}  <- the canaries")
    print(f"  orphans under the shipped rule .... {len(orphans)}")

    failed = False

    if len(described) < MIN_DESCRIBED:
        print(
            f"\nFAIL: only {len(described)} document/spec artifact(s) carry a "
            f"DOCUMENTS/SPECIFIES edge (floor is {MIN_DESCRIBED}).\n"
            "  reflow2's own graph has stopped following the link-artifacts skill "
            "it serves.\n"
            "  That is exactly how [BL-176] stayed invisible for eleven releases: "
            "a design\n"
            "  that never uses DOCUMENTS can never notice DOCUMENTS being "
            "mishandled.",
            file=sys.stderr,
        )
        failed = True

    if not canaries:
        print(
            "\nFAIL: no artifact is attached ONLY by DOCUMENTS/SPECIFIES.\n"
            "  Every document also carries a REALIZES, so reverting the BL-176 "
            "fix would\n"
            "  produce no finding here and the regression would ship unnoticed. "
            "At least\n"
            "  one honestly-described document must stand on its describing edge "
            "alone.",
            file=sys.stderr,
        )
        failed = True

    if orphans:
        print(
            f"\nFAIL: {len(orphans)} artifact(s) are attached to nothing at all: "
            f"{orphans}\n"
            "  Every edge they carry is bookkeeping (INCLUDES/CHANGED/YIELDED/"
            "AT_EPOCH),\n"
            "  so nothing in the graph says what the file is FOR.\n"
            "\n"
            "  TWO WAYS ON, and DELETING THE ARTIFACT IS NEITHER — that is the repair\n"
            "  that looks clean and loses the evidence:\n"
            "    1. It describes or implements something -> draw the true edge\n"
            "       (DOCUMENTS / SPECIFIES / REALIZES / SATISFIES).\n"
            "    2. It correctly describes and implements NOTHING -- a dated field\n"
            "       report, an incident write-up, a customer complaint, a vendor's test\n"
            "       certificate -> say so with governed_by(..., ruling='parks') pointing\n"
            "       at an ACCEPTED Decision. The artifact is then counted in\n"
            "       swept.parked instead of reported here, and DOCUMENTS is NOT the\n"
            "       answer for it: DOCUMENTS claims the file should stay in step with\n"
            "       the design, and a dated observation is correct precisely by not\n"
            "       tracking it.",
            file=sys.stderr,
        )
        failed = True

    if failed:
        return 1

    print(
        f"\nOK: the design record still exercises DOCUMENTS, and {len(canaries)} "
        "artifact(s)\n"
        "    would falsely fire under the pre-BL-176 rule — so this repo can "
        "still catch\n"
        "    that regression on itself."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
