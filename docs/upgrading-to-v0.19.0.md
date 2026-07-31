# Upgrading to v0.19.0 — upgrade everywhere, together

> Part of the **Reflow 2.0** design docs — see **[overview.md](overview.md)** for the full map and
> reading order.

**Short version: this one is not optional if you share a design.** Nothing in your repository
changes and your existing design opens fine. But v0.19.0 adds an **edge type**, and that moves the
version stamp — so a reflow2 older than this can no longer open a design written by it. Upgrade
every machine and every session that touches a shared graph, together.

If you work alone on one machine: update and carry on, and read the rest for what you gained.

## The one thing that needs attention

`SCHEDULED_FOR` (Requirement/Capability → DesignEpoch/Release) takes the schema from **56 edge
types to 57**. The node types are unchanged at 28, and the `dynograph-foundation` pin stays at
`v0.12.0` — there is no slow first build.

An older reflow2 opening a design written by this one will **refuse** — loudly, naming the
mismatch, not garble it and not silently show you less. That refusal is deliberate: it is the check
that stops a design being quietly half-read (see [upgrading-to-v0.11.0.md](upgrading-to-v0.11.0.md)
and BL-94). But it means a mixed-version team has the older side **locked out**, not degraded.

| Change | Does it lock out an older reflow2? |
|---|---|
| New **edge type** (`SCHEDULED_FOR`) | **Yes** — the stamp counts edge types |
| New **property** (`DesignEpoch.status`) | No — the stamp does not count properties |
| New **enum value** (`expected`, `required`) | No — the stamp does not count property values |

`SCHEDULED_FOR` is additive; nothing was retired, and no existing node changed meaning. Every epoch
written before this version reads as `arrived`, which is a *record* rather than a default choice:
`add_epoch` has only ever meant "record the point I am at".

So: upgrade together. If you are stuck on an older binary somewhere, export the design with a
current one and work from that file; the refusal message names the recovery paths.

## What you gained

**The time axis runs forward.** Until now an epoch could only record a point that had already
happened, so a roadmap had nowhere to live. `plan_epoch` creates a point that has *not* happened,
`set_epoch_status` moves it to `arrived`, and `schedule_for` hangs work off it — a Requirement or
Capability scheduled against a DesignEpoch (time) or a Release (capability increment), carrying
`modality`: `expected` (a plan) or `required` (an obligation whose miss at arrival is a computed
violation — the scheduling face of a KPP).

There is deliberately **no `achieved` modality**: delivery is computed from the golden thread and
never asserted, so a schedule that recorded its own success would be a second source of truth able
to disagree with the first.

**`arrival_delta` answers "what didn't we achieve that we were supposed to?"** Every scheduled item
comes back as **delivered**, **deferred** (and where to), **discontinued**, or **outstanding** — the
fifth outcome beside the four originally sketched, for the commonest case of all: nobody touched it
and it did not happen. Calling that *discontinued* would put a withdrawal on the record nobody made,
and *deferred* would invent a date nobody chose, so it is reported as itself and put to you.

The baseline is the target's **first** snapshot, not the last. The last would measure only the most
recent revision: two replans leave an epoch holding `{A,B,C}` then `{A,C}`, so reading the last says
the plan was always `{A,C}` and the slip vanishes from the very report meant to show it.

**A lossy schedule edit is now refused** while the plan is unrecorded — removing a `SCHEDULED_FOR`,
re-pointing it, or rewriting its modality, through either `delete_edge` or `delete_node`. The
refusal names the `record_change` that unblocks it. **Adding** to a plan destroys no earlier claim
and is deliberately free.

**The design can hold what it points at** (`content_put`, `content_get`, `content_exists`,
`content_manifest`). A content-addressed store, committed to your repo, for the documents and
diagrams a Decision points at — what *informed* the design, as against what it produced.

There is a new flag, **`--content-path`**, defaulting to **`./reflow2-content`** — a directory
inside your repo, created on first write, not at startup. If you never store anything, nothing
appears and nothing changes. Two things are worth knowing:

- It is deliberately **not** derived from `--graph-path`. The graph lives under `.reflow2/`, which
  is gitignored, and blobs are committed so they travel with the design — deriving would have put
  your diagrams somewhere git ignores.
- **Add this line to your repo's `.gitattributes` before you store anything binary:**

  ```
  reflow2-content/** binary
  ```

  The point is not the diff. Without it, line-ending conversion can silently corrupt a PNG on a
  CRLF checkout — data loss on someone else's machine that nothing in the history would explain.
  reflow2's own repo carries the rule; `reflow2_init.py` does not write it into yours.

**What bounds that store is what you put in it, not a size limit.** `content_put` refuses past
100 MB with `accept_large` as a recorded override, and the refusal says in its own text that the cap
is *not* what keeps the store small. Measured here: session transcripts run about 4 MB each and 29
of them already exceed the entire `.git` history. Store transcripts by exception, not by default.

**A release that promises nothing no longer reports itself ready to cut.** If you use
`release_report`'s readiness, it now requires at least one `required` obligation as well as an empty
miss list, and reports `required_count` beside the answer.

## What you have to do

Beyond upgrading together: nothing, unless you intend to use the content store — in which case add
the `.gitattributes` rule above before you store your first binary. No tool lost a parameter, no
existing node changed meaning, and your design opens as it did.
