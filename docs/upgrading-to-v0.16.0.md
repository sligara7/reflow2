# Upgrading to v0.16.0 — upgrade everywhere, together

> Part of the **Reflow 2.0** design docs — see **[overview.md](overview.md)** for the full map and
> reading order.

**Short version: this one is not optional if you share a design.** Nothing in your repository
changes and your existing design opens fine. But v0.16.0 adds an **edge type**, and that moves the
version stamp — so a reflow2 older than this can no longer open a design written by it. Upgrade
every machine and every session that touches a shared graph, together.

If you work alone on one machine: update and carry on, and read the rest for what you gained.

## The one thing that needs attention

`PERFORMED_IN` (Verification → Environment) takes the schema from **55 edge types to 56**.

An older reflow2 opening a design written by this one will **refuse** — loudly, naming the
mismatch, not garble it and not silently show you less. That refusal is deliberate: it is the check
that stops a design being quietly half-read (see [upgrading-to-v0.11.0.md](upgrading-to-v0.11.0.md)
and BL-94). But it means a mixed-version team has the older side **locked out**, not degraded.

| Change | Does it lock out an older reflow2? |
|---|---|
| New **edge type** (`PERFORMED_IN`) | **Yes** — the stamp counts edge types |
| New **enum value** (`simulation`, `demonstration`, `observation`) | No — the stamp does not count property values |

So: upgrade together. If you are stuck on an older binary somewhere, export the design with a
current one and work from that file; the refusal message names the recovery paths.

## What you gained

**You can drive the extraction pipeline yourself** (`ingest_step`). INGEST has existed for months
and was unreachable from a session: it needs a language model, reflow2 ships no provider, and the
calling agent could not be reached mid-run. Now your agent answers the prompts itself, over three
or four rounds. Nothing is written until the last one, so an abandoned run leaves nothing behind.

Use it for anything document-shaped instead of calling `add_*` by hand — it is what gives you
provenance back to the source text, snapshot-before-overwrite when a re-ingest changes something,
and the resolution work below.

**It now recovers rationale and test evidence**, not just requirements. `Decision` and
`Verification` extraction passes mean a document saying *"we chose cache-aside because
write-through amplified writes"* produces the choice **and** its reasoning, and one describing a
load test produces the check. Both land unasserted — a recovered decision is `proposed`, a recovered
check is `planned` — because an extraction is your agent's reading of somebody's document, not your
signature on it.

**Near-matches are asked about instead of guessed.** Ingest reads the two thresholds your schema
has always declared: below the first, a new node; above the second, a merge; **between them, a
reported candidate**. And a structural pass catches what similarity scoring cannot — `Gateway`
versus `API Gateway` scores 74, below any threshold, because ratios fall as length differs.

**reflow2 can say what it has never been told about** (`coverage_report`). Sweep your tree, hand it
the paths, and it reports the regions no node claims — rolled up and ranked by size. Every other
check reasons about what is *already* in the graph, so without this a design covering a third of a
system reported the same `0 open gaps` as one covering all of it. The `adopt` skill now ends by
asking. It is not a score: an artifact whose location is a directory claims everything beneath it,
so modelling a vendored tree as one opaque unit is correct.

**A check can say where it ran** (`PERFORMED_IN`, `evidence_report`) — the reason for the schema
change. Testing in simulation first is only worth it if you can still tell model from reality
afterwards, and reflow2 could not: a rig run and a production run were both simply `passing`. It
now flags capabilities **proven only in simulation**, and treats a check that names no environment
as *unplaced* rather than assuming it was real.

**The design's own history is checked** (BL-107). Each committed export records the hash of the one
it replaced; the gate now verifies that link and fails a build that severs it. If you run
`reflow2_check.py` in CI, this is free.

## What you have to do

Beyond upgrading together: nothing. No tool lost a parameter, no existing node changed meaning, and
your design opens as it did.
