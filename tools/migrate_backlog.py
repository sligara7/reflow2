#!/usr/bin/env python3
"""Turn the open backlog rows into the graph nodes they always were.

`dec:backlog-is-retired` (accepted 2026-08-07, Anthony's word) chose option (a) of
`dec:idea-backlog-belongs-in-the-graph`: route each row to the type it already is,
add no vocabulary, and delete the file.

THE STANDARD THIS MUST MEET, quoted from the decision it settles, because it is the
whole risk: "A row moved in as one long `description` rebuilds backlog.md inside the
graph with worse ergonomics and no gain — and that failure would be invisible,
because it would look like progress."

So every row emitted here carries THREE things, not one:

  - a fulltext-indexed CLAIM (`statement` / `description` / `decision`), so
    search_design finds it by content rather than by filename;
  - an EDGE to the node it bears on, so propagate_from reaches it and the orphan
    detector does not count it as unattached;
  - a LIFECYCLE — a TemporalFact with no VALID_TO is still true, a Capability at
    `planned` is unbuilt, a `proposed` Decision is unanswered.

The classification below is JUDGEMENT and is the only part a machine could not do.
The prose is lifted verbatim from the row, so nothing is paraphrased away.

WHAT IS NOT MIGRATED, deliberately: rows already marked DONE/CLOSED inside the table
(50 of 138), and the whole "Closed" section, which is already only stable-id pointers
into the CHANGELOG. A finished item's value is history, and git holds it.

    python3 tools/migrate_backlog.py > /tmp/backlog-import.json
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
BACKLOG = REPO / "docs" / "backlog.md"
EPOCH = "epoch:backlog-retired"

# --- the judgement -----------------------------------------------------------
#
# kind: "defect"   -> TemporalFact, fact_type "defect" (built, and wrong)
#       "unbuilt"  -> Capability at `planned` (does not exist yet)
#       "question" -> Decision at `proposed` (a fork nobody has chosen)
#       "limit"    -> DesignRule (a discovered limit that binds future work)
#
# target: the node the row bears on. This is what makes the row STRUCTURE rather
# than prose, so a row with no defensible target does not get migrated — it gets
# reported as unplaced at the bottom, for a human to place.

CLASSIFY: dict[str, tuple[str, str]] = {
    # --- defects against things that are built -------------------------------
    "BL-214": ("defect", "sys:coherence-loop"),
    "BL-215": ("defect", "cmp:export"),
    "BL-218": ("defect", "sys:store"),
    "BL-217": ("defect", "cmp:export"),
    "BL-216": ("defect", "cmp:identity"),
    "BL-210": ("defect", "cmp:ingest"),
    "BL-209": ("defect", "cmp:heal"),
    "BL-207": ("defect", "cmp:artifact"),
    "BL-204": ("defect", "cmp:skills"),
    "BL-198": ("defect", "cmp:export"),
    "BL-200": ("defect", "cmp:provenance"),
    "BL-201": ("defect", "cmp:vocabulary"),
    "BL-202": ("defect", "cmp:nodes"),
    "BL-203": ("defect", "cmp:vocabulary"),
    "BL-187": ("defect", "cmp:temporal"),
    "BL-189": ("defect", "cmp:ingest"),
    "BL-192": ("defect", "cmp:service"),
    "BL-193": ("defect", "cmp:detect"),
    "BL-194": ("defect", "cmp:detect"),
    "BL-195": ("defect", "cmp:heal"),
    "BL-196": ("defect", "cmp:genesis"),
    "BL-197": ("defect", "cmp:skills"),
    "BL-178": ("defect", "cmp:skills"),
    "BL-177": ("defect", "cmp:init"),
    "BL-171": ("defect", "cmp:verify"),
    "BL-168": ("defect", "cmp:export"),
    "BL-167": ("defect", "cmp:service"),
    "BL-164": ("defect", "cmp:schema"),
    "BL-114": ("defect", "cmp:detect"),
    "BL-115": ("defect", "cmp:detect"),
    "BL-116": ("defect", "cmp:detect"),
    "BL-120": ("defect", "cmp:vocabulary"),
    "BL-121": ("defect", "cmp:nodes"),
    "BL-122": ("defect", "cmp:detect"),
    "BL-124": ("defect", "cmp:heal"),
    "BL-127": ("defect", "cmp:vocabulary"),
    "BL-128": ("defect", "cmp:vocabulary"),
    "BL-130": ("defect", "cmp:operate"),
    "BL-131": ("defect", "cmp:nudge"),
    "BL-132": ("defect", "cmp:service"),
    "BL-133": ("defect", "cmp:skills"),
    "BL-134": ("defect", "cmp:artifact"),
    "BL-137": ("defect", "cmp:temporal"),
    "BL-156": ("defect", "cmp:export"),
    "BL-154": ("defect", "cmp:skills"),
    "BL-152": ("defect", "cmp:skills"),
    "BL-149": ("defect", "cmp:propagate"),
    "BL-151": ("defect", "cmp:structure"),
    "BL-139": ("defect", "cmp:graph"),
    "BL-140": ("defect", "cmp:detect"),
    "BL-142": ("defect", "cmp:schema"),
    "BL-143": ("defect", "cmp:vocabulary"),
    "BL-144": ("defect", "cmp:export"),
    "BL-145": ("defect", "cmp:detect"),
    "BL-146": ("defect", "cmp:scope"),
    "BL-147": ("defect", "cmp:service"),
    "BL-148": ("defect", "cmp:service"),
    "BL-113": ("defect", "cmp:service"),
    "BL-112": ("defect", "cmp:nudge"),
    "BL-110": ("defect", "cmp:export"),
    "BL-105": ("defect", "cmp:degraded"),
    "BL-94": ("defect", "cmp:identity"),
    "BL-12b": ("defect", "cmp:service"),
    "BL-158": ("defect", "cmp:artifact"),
    "BL-157": ("defect", "cmp:drift"),
    "BL-159": ("defect", "cmp:docs"),
    "BL-160": ("defect", "cmp:artifact"),
    "BL-161": ("defect", "cmp:nudge"),
    "BL-162": ("defect", "cmp:heal"),
    "BL-163": ("defect", "cmp:nudge"),
    "BL-165": ("defect", "cmp:schema"),
    "BL-166": ("defect", "cmp:artifact"),
    "BL-169": ("defect", "cmp:identity"),
    "BL-170": ("defect", "cmp:scope"),
    "BL-176": ("defect", "cmp:heal"),
    "BL-199": ("defect", "cmp:skills"),
    "BL-205": ("defect", "cmp:report"),
    "BL-206": ("defect", "cmp:scope"),
    "BL-208": ("defect", "cmp:export"),
    "BL-213": ("defect", "cmp:merge"),
    "BL-123": ("defect", "cmp:service"),

    # --- work that does not exist yet ---------------------------------------
    "BL-186": ("unbuilt", "cmp:ingest"),
    "BL-211": ("unbuilt", "cmp:genesis"),
    "BL-180": ("unbuilt", "cmp:structure"),
    "BL-150": ("unbuilt", "cmp:budget"),
    "BL-108": ("unbuilt", "cmp:agent"),
    "BL-109": ("unbuilt", "cmp:compare"),
    "BL-97": ("unbuilt", "cmp:ingest"),
    "BL-99": ("unbuilt", "cmp:claims"),
    "BL-100": ("unbuilt", "cmp:skills"),
    "BL-101": ("unbuilt", "cmp:skills"),
    "BL-102": ("unbuilt", "cmp:docs"),
    "BL-8": ("unbuilt", "cmp:registry"),
    "BL-12": ("unbuilt", "cmp:claims"),
    "BL-13": ("unbuilt", "cmp:verify"),
    "BL-40": ("unbuilt", "cmp:report"),
    "BL-184": ("unbuilt", "cmp:dimensions"),
    "BL-179": ("unbuilt", "cmp:structure"),
    "BL-181": ("unbuilt", "cmp:service"),
    "BL-182": ("unbuilt", "cmp:structure"),
    "BL-190": ("unbuilt", "cmp:vocabulary"),
    "BL-172": ("unbuilt", "cmp:scope"),
    "BL-174": ("unbuilt", "cmp:skills"),
    "BL-175": ("unbuilt", "cmp:init"),

    # --- forks nobody has chosen --------------------------------------------
    # BL-95 is the same shape as req:the-tool-finds-its-own-blind-spots: a
    # detector that cannot see what it was never told about.
    "BL-95": ("defect", "cmp:detect"),
    # BL-14 is a BUNDLE pointing at docs/reflow-audit.md, not one item. Migrated
    # whole rather than split, because splitting it means reading that audit and
    # deciding four things — and inventing four nodes from a one-line summary
    # would assert intent nobody stated. The row's own text carries the list.
    "BL-14": ("unbuilt", "cmp:service"),

    "BL-98": ("question", "cmp:claims"),
    "BL-103": ("question", "cmp:search"),
    "BL-10": ("question", "cmp:drift"),
    "BL-155": ("question", "cmp:service"),
    "BL-185": ("question", "cmp:verify"),
    "BL-189b": ("question", "cmp:ingest"),

    # --- discovered limits that bind future work ----------------------------
    "BL-212": ("limit", "cmp:ingest"),
    "BL-135": ("limit", "sys:agent-surface"),
    "BL-41": ("limit", "sys:agent-surface"),
    "BL-188": ("limit", "cmp:artifact"),
    "BL-191": ("limit", "cmp:artifact"),
}

# CASE-SENSITIVE, and read only against the ITEM column. Both of those are
# corrections to a first pass that silently dropped 11 OPEN rows:
#   - `re.I` matched the word "shipped" or "done" occurring anywhere in a row's
#     prose, so rows explaining why something else shipped were read as closed;
#   - checking the `why` column at all is what made that possible. The convention
#     puts the marker in the Item column ("— DONE 2026-08-04"), nowhere else.
# A filter that removes real work and says nothing is exactly the failure this
# whole migration exists to stop, so it is worth the comment.
DONE = re.compile(r"DONE \d{4}|CLOSED \d{4}|— done\b|SHIPPED")
ROW = re.compile(r"^\| \*\*(BL-[0-9a-z]+)\*\* \|")


def clean(s: str) -> str:
    """Strip the markdown emphasis the table uses, keep the words."""
    s = re.sub(r"\*\*|⭐|`", "", s)
    return re.sub(r"\s+", " ", s).strip()


def rows():
    """Yield (id, item, why, size) per open row.

    Split from BOTH ENDS rather than left to right: several rows carry a literal
    `|` inside the Why prose, and a plain 4-way split loses them. The id is the
    first field and the size estimate the last; everything between the item and
    the size is the why, rejoined.
    """
    for line in BACKLOG.read_text().splitlines():
        if not ROW.match(line):
            continue
        parts = [p.strip() for p in line.strip().strip("|").split("|")]
        if len(parts) < 4:
            continue
        bl, item, size = parts[0], parts[1], parts[-1]
        why = " | ".join(parts[2:-1])
        bl = re.sub(r"\*\*", "", bl).strip()
        if DONE.search(item):
            continue  # already finished; git holds the history
        yield bl, clean(item), clean(why), clean(size)


def main() -> int:
    nodes, edges, unplaced = [], [], []
    counts = {"defect": 0, "unbuilt": 0, "question": 0, "limit": 0}

    for bl, item, why, size in rows():
        entry = CLASSIFY.get(bl)
        if entry is None:
            unplaced.append((bl, item[:80]))
            continue
        kind, target = entry
        counts[kind] += 1
        # The row's own words, kept whole. `provenance` says where it came from so
        # a reader can tell a migrated row from something authored in the graph.
        origin = f"Migrated from docs/backlog.md {bl} on 2026-08-07 ({EPOCH}). Size estimate carried from the row: {size or 'unstated'}."

        if kind == "defect":
            nid = f"fact:{bl.lower()}"
            nodes.append({
                "node_type": "TemporalFact", "node_id": nid,
                "properties": {
                    "subject_id": target,
                    "fact_type": "defect",
                    "basis": "measured",
                    "statement": f"[{bl}] {item}",
                    "value": json.dumps({"evidence": why, "origin": origin}),
                },
            })
            # Both directions, as dec:defects-are-temporal-facts prescribes: the
            # indicted node carries the fact, and the fact points back at it.
            edges.append({"edge_type": "HAS_TEMPORAL_FACT", "from_id": target, "to_id": nid, "properties": {}})
            edges.append({"edge_type": "ABOUT_ENTITY", "from_id": nid, "to_id": target, "properties": {}})
            # No VALID_TO: absent means still true. VALID_FROM is the migration
            # epoch and NOT the observation date, which is unrecoverable per row —
            # the real date, where the row states one, is in the evidence text.
            edges.append({"edge_type": "VALID_FROM", "from_id": nid, "to_id": EPOCH, "properties": {}})

        elif kind == "unbuilt":
            nid = f"cap:{bl.lower()}"
            nodes.append({
                "node_type": "Capability", "node_id": nid,
                "properties": {
                    "name": f"[{bl}] {item[:120]}",
                    "description": f"{item}\n\n{why}\n\n{origin}",
                    "status": "planned",
                    "provenance": "imported",
                },
            })
            edges.append({"edge_type": "ALLOCATED_TO", "from_id": nid, "to_id": target, "properties": {}})

        elif kind == "question":
            nid = f"dec:{bl.lower()}"
            nodes.append({
                "node_type": "Decision", "node_id": nid,
                "properties": {
                    "name": f"OPEN [{bl}] — {item[:120]}",
                    "decision": f"{item}\n\n{why}\n\n{origin}\n\nUNANSWERED. Migrated as a `proposed` Decision because the row states a fork rather than a defect or a need: nobody has chosen, and recording it as intent would assert an answer.",
                    "status": "proposed",
                },
            })
            edges.append({"edge_type": "GOVERNED_BY", "from_id": target, "to_id": nid, "properties": {}})

        elif kind == "limit":
            nid = f"rule:{bl.lower()}"
            nodes.append({
                "node_type": "DesignRule", "node_id": nid,
                "properties": {
                    "name": f"[{bl}] {item[:120]}",
                    "statement": f"{item}\n\n{why}\n\n{origin}",
                },
            })
            edges.append({"edge_type": "CONSTRAINS", "from_id": nid, "to_id": target,
                          "properties": {"note": origin}})

    doc = {"nodes": nodes, "edges": edges}
    json.dump(doc, sys.stdout, indent=2, ensure_ascii=False)
    print(file=sys.stdout)

    print(f"nodes {len(nodes)}  edges {len(edges)}  {counts}", file=sys.stderr)
    if unplaced:
        print(f"UNPLACED — {len(unplaced)} row(s) have no classification and were NOT "
              f"migrated. They need a target before they can become structure:", file=sys.stderr)
        for bl, t in unplaced:
            print(f"  {bl}  {t}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
