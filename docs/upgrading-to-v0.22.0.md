# Upgrading to v0.22.0 — upgrade everywhere, together

> Part of the **Reflow 2.0** design docs — see **[overview.md](overview.md)** for the full map and
> reading order.

**Short version: this one is not optional if you share a design.** Nothing in your repository
changes and your existing design opens fine. But v0.22.0 adds a **node type** and **two edge
types**, and that moves the version stamp — so a reflow2 older than this can no longer open a
design written by it. Upgrade every machine and every session that touches a shared graph,
together.

If you work alone on one machine: update and carry on, and read the rest for what you gained.

## The one thing that needs attention

`ReadinessAssessment` takes the schema from **28 node types to 29**, and `GATED_ON` +
`HAS_READINESS` take it from **58 edge types to 60**. This is the first release since v0.4.0 to
move *both* halves of the stamp at once. The `dynograph-foundation` pin is unchanged — there is no
slow first build.

An older reflow2 opening a design written by this one will **refuse** — loudly, naming the
mismatch, not garble it and not silently show you less. That refusal is deliberate: it is the check
that stops a design being quietly half-read (see [upgrading-to-v0.11.0.md](upgrading-to-v0.11.0.md)
and BL-94). But it means a mixed-version team has the older side **locked out**, not degraded.

| Change | Does it lock out an older reflow2? |
|---|---|
| New **node type** (`ReadinessAssessment`) | **Yes** — the stamp counts node types |
| New **edge types** (`GATED_ON`, `HAS_READINESS`) | **Yes** — the stamp counts edge types |
| New **edge properties** (`GATED_ON.kind`, `.min_level`, `.rationale`) | No — the stamp does not count properties |
| New **node properties** (`TemporalFact.basis`, `TemporalFact.confidence`) | No — same reason |
| New **report** (`readiness_report`) | No — reports are computed, never stored |

Everything here is **additive**. Nothing was retired, and no existing node or edge changed meaning.

### The data-migration question, answered

AGENTS.md requires this to be asked rather than assumed: *what happens to a graph written by the
previous version?*

**It reads correctly, and nothing is backfilled.** The one existing type that gained properties is
`TemporalFact`, and both additions are safe by construction:

- `basis` defaults to `measured`. Every `TemporalFact` written before v0.22.0 recorded something
  that had happened, so the default **states what those records already meant** rather than
  inventing a claim about them — the same argument `DesignEpoch.status` made for defaulting to
  `arrived` (`req:defaults-do-not-assert`).
- `confidence` has no default. Absent reads as *unstated*, never as *certain*.

Note the foundation's own behaviour, which is why this matters: defaults apply **on create, not
retroactively** (`engine/tests.rs:1325`). So an old fact does not physically gain a `basis` key —
it is simply absent, and every reader treats absent as `measured`. There is no migration step, no
rewrite pass, and nothing to run.

## What you gained

**A delivery date you can argue with.** Until now reflow2 could say what an increment *contained*
and what had been *delivered*, but the epoch an increment would arrive on was an assertion — the
same slide-drawn roadmap the tool exists to replace. v0.22.0 makes it a computation.

Three pieces, and the split between them is the whole design:

1. **`add_readiness`** records an *observation* — a TRL or MRL level, 1–9, about an enabling
   technology. An input fact, in the same family as a checksum.
2. **`gate_on`** states a *judgement* — "this increment needs that technology at TRL 7". It rides
   an edge rather than either endpoint, so one increment can demand TRL 7 of one technology and
   TRL 4 of another, and a demonstrator and a fielded increment can demand different levels of the
   *same* technology.
3. **`forecast_readiness`** records a *projection* — "TRL 7 by 2035" — as a `TemporalFact` marked
   `basis: forecast`, because `observed_at` says *observed* and nobody observed anything in 2035.

Then **`readiness_report`** derives the answer: the earliest epoch by which every gating technology
clears the level demanded of it, naming the one that decided it —

> *"rel:v2-fielded cannot deliver before 2035, because cmp:conversion is TRL 3 today, is projected
> TRL 7 at 2035 (author-stated confidence 0.4), and this increment needs TRL 7."*

### Two refusals you should expect, because they are the point

reflow2 **will not invent the judgement half.**

- An increment with no stated threshold reports **`ungated`** — never "ready". Silence about a gate
  is not evidence there is none. If you want a date, state a threshold.
- A gate whose technology has no level and no clearing forecast makes the whole answer
  **`indeterminate`**, not a date computed from the gates that *do* have evidence. Dropping the
  inconvenient gate would return an optimistic date built by ignoring half the record.

Likewise, forecast **confidence is yours to state**. reflow2 does not derive one from how far away
the epoch is: a decay curve is a judgement about risk appetite, and inventing one would assert a
risk model nobody chose. The precedent is `Interface.medium`, which once defaulted to `REST` and
thereby made two silent boundaries "agree" on a value neither had chosen — a defaulted readiness
threshold is that defect with a roadmap attached.

### One thing that changed quietly, and is worth knowing

`GATED_ON` is a **traceability edge**, so a technology whose readiness slips now appears in the
blast radius of every increment gated on it. `propagate_change` on that component will reach the
releases waiting for it. That is deliberate: an edge every traversal stepped over would let a
technology fall from TRL 7 to 3 with the roadmap reporting nothing.

## How to upgrade

Same as always — the installer replaces the binary in place:

```bash
curl -fsSL https://raw.githubusercontent.com/sligara7/reflow2/main/tools/install.sh | sh
```

Then restart any session with a running MCP server; a `/mcp` reconnect does **not** replace an
already-running stdio server.

If you are stuck on an older binary somewhere, export the design with a current one and work from
that file; the refusal message names the recovery paths.
