# Upgrading to v0.59.0

🛑 **Upgrade everywhere, together.** This is the first release whose schema stamp moves in a way
an **older binary cannot see**: `Decision.status` gains the value `deferred`, and no type or edge
count changes with it.

## Why this one needs the warning

Before v0.59.0 the version guard compared **types** — how many node types, how many edge types,
their names. A new enum *value* moves none of those. So a v0.58.0 (or earlier) binary that opens
a graph written by v0.59.0 sees a stamp identical to its own, opens the graph **with no warning
at all**, compares `status == "proposed"` as it always has — and every `deferred` decision simply
**vanishes** from `loop_status`, `what_next` and the gap detectors. Nothing errors. That is the
exact harm the guard's own refusal names, arriving through a door the old guard did not watch.

v0.59.0 closes that door going forward: its stamp records every enum vocabulary the schema
declares, and it refuses to open a graph that *stores* a value it does not know — naming the
value and both versions. It cannot protect a binary older than itself. **Only upgrading every
seat does that.**

| | v0.58.0 | v0.59.0 |
| --- | --- | --- |
| Node types | 28 | 28 |
| Edge types | 65 | 65 |
| `Decision.status` | proposed, accepted, superseded, rejected | + **deferred** |
| Stamp records enum values | no | **yes** |

## What you do

1. Update every binary that opens the design — every machine, every harness. The installer's
   update does it; a hand-built copy needs rebuilding.
2. Open the graph. A v0.59.0 binary opens a v0.58.0 graph untouched: the old stamp carries no
   enum record, an absent record is not a claim, and the graph is stamped on the first write.
3. Nothing to migrate. No export, no import. The value only exists once somebody defers a
   decision on purpose (`set_decision_status … deferred`, with the approver named).

## If you cannot upgrade a seat yet

Do not defer any decision from a v0.59.0 seat until every seat can read it. A deferred decision
is invisible to the older binary, not refused by it — the one failure this note exists to name.

## Also in this release, needing nothing from you

- **`/feedback`** — a served skill on every harness; the server keeps a usage ledger beside the
  store (`<graph>.usage.jsonl`, verb never object). The file appears on the first tool call
  after upgrading; nothing leaves the machine.
- **Lessons at the step** — `steps` on `record_finding` / `add_design_rule`; `get_skill` and the
  tool list deliver them. Optional; a lesson naming no step is unchanged.
- **`--export-to`** — the installer's update adds it to the MCP config; the server then keeps the
  committed export current on its own and never overwrites a hand edit.
- **A session serves the release binary** in reflow2's own repo; installed projects already did.
