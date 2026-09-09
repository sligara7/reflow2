# Upgrading to v0.55.0

> ## ⚠️ SUPERSEDED BY v0.56.0 — READ THIS FIRST
>
> **If you are upgrading to v0.56.0 or later, the migration below is almost
> certainly unnecessary. Try opening your graph first.**
>
> This document was written for a guard that refused on the schema **stamp**.
> Every graph written before the retirement names `QualityGate` in its stamp
> whether or not it ever held one, so every graph was refused — which is why
> this note said the action was required of everyone.
>
> **v0.56.0 fixed that.** The guard now asks the store whether the retired type
> is actually there, and refuses only when it is. By every measurement taken,
> no design ever held a `QualityGate`: it had no typed constructor, so the only
> way to create one was the generic escape hatch.
>
> **Go straight to v0.56.0 and open your graph.** If it opens, you are done and
> nothing below applies. If it is refused, your graph genuinely holds one — and
> then the migration below is exactly right.
>
> The original text is kept unchanged: it was correct for the release it was
> written for, and a reader on v0.55.0 or v0.55.1 still needs it.


🛑 **Every existing graph must be migrated before it will open — including yours,
and including one that never held a `QualityGate`.** One export and one import.
Nothing else in this release needs anything from you.

## What changed

`QualityGate` has been **removed from the schema**. It is the first node type
this project has ever removed; every previous stamp move added types and left
existing graphs readable untouched.

| | v0.54.0 | v0.55.0 |
| --- | --- | --- |
| Node types | 29 | **28** |
| Edge types | 65 | 65 |

## Why it affects you even with zero instances

**The guard reads the graph's *stamp*, not its contents.** A stamp records the
*schema the writing binary had* — so every graph written by any earlier reflow2
names `QualityGate`, whether or not a single one was ever created. Instance count
never enters the check.

reflow2's own graph holds **zero** `QualityGate` nodes and was refused outright.

You will see this on the first open:

```
this graph names types this reflow2 cannot read, so opening it could silently
show you less of your design than it holds — refused.
 • This graph predates a schema change: it uses QualityGate, which this reflow2
   RETIRED, and your reflow2 is current — migrate the graph …
```

**The refusal is correct and it is not a bug.** reflow2 refuses to open a store
whose vocabulary is wider than its own rather than show you less of your design
than it holds. Read it as an instruction, not a fault.

## The migration

Run with **your current reflow2** for step 1 and **v0.55.0** for step 2.

```bash
# 1. Export with a reflow2 that still knows the type (v0.54.0 or earlier).
reflow2-mcp --graph-path ./.reflow2/graph --export > design.json

# 2. Import into a FRESH path with v0.55.0. A retired type is dropped and named.
reflow2-mcp --graph-path ./.reflow2/graph-new --import design.json

# 3. Read the counts back and compare before you swap.
reflow2-mcp --graph-path ./.reflow2/graph-new --export | \
  python3 -c 'import json,sys; d=json.load(sys.stdin); print(len(d["nodes"]), "nodes,", len(d["edges"]), "edges")'

# 4. Only when the counts match, swap it in.
mv .reflow2/graph .reflow2/graph-preretirement
mv .reflow2/graph-new .reflow2/graph
```

**Verified end to end on reflow2's own graph before this shipped:** 3,989 nodes
and 21,555 edges in, the same out, **nothing lost**, stamp 29 → 28.

⚠️ **Keep the old graph until you have opened the new one.** Step 4 renames
rather than deletes, deliberately.

⚠️ **If you move a graph by hand, move its sidecar too.** The stamp lives in
`<graph-path>.meta.json` — *beside* the store directory, not inside it. Copying
a store directory over an old sidecar reproduces the refusal on a graph you have
already migrated. Importing into a fresh path avoids this, because it writes its
own. This was found by falling into it.

### If you actually have a `QualityGate` node

Almost nobody will: the type never had a typed constructor, so the only way to
create one was the generic `create_node` escape hatch. If you do, the import
**names it and drops it** rather than failing — re-express what it recorded
before you swap. A phase gate's conditions are the sort of thing `detect_gaps`
and `detect_defects` now compute continuously.

## Why the type went

`QualityGate` modelled the **stage gate** — a dated, waivable judgement at a
phase boundary. It was never a duplicate of `Verification`; a test does not get
waived, a review board's gate does.

It went unused for structural reasons rather than for lack of interest:

- it participated in **no edge type at all**, so a gate could be created and
  attached to nothing;
- its `criteria` was a JSON array inside a string, which nothing could evaluate;
- and decisively, **nothing in a reflow2 design has ever carried a phase**, so
  the gate had nothing to gate.

Meanwhile its own extraction hint's examples — *"DAG is acyclic"*, *"all
Requirements have coverage"* — are literally what `detect_gaps` and
`detect_defects` evaluate on every call. A computed check never goes stale where
a stored `passed` does. The gate was not abandoned; it dissolved into the loop.

**It can come back.** `dec:qualitygate-is-retired-the-phase-gate-dissolved-into-the-detectors`
records the condition: a project needing a signed, auditable judgement at a
boundary — the case continuous computation cannot serve, because the auditable
thing is a person's decision at a moment rather than a condition's current truth.
It also records what the type would need that it never had.

## Everything else in this release is additive

The environment compliance layer adds six tools and two detectors over vocabulary
the schema already declared, and `link_artifact` gained an optional `description`
and stopped overwriting an artifact's name when you omit one. No action needed
for either.
