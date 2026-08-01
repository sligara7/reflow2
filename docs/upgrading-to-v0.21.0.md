# Upgrading to v0.21.0 — upgrade everywhere, together

> Part of the **Reflow 2.0** design docs — see **[overview.md](overview.md)** for the full map and
> reading order.

**Short version: this one is not optional if you share a design.** Nothing in your repository
changes and your existing design opens fine. But v0.21.0 adds an **edge type**, and that moves the
version stamp — so a reflow2 older than this can no longer open a design written by it. Upgrade
every machine and every session that touches a shared graph, together.

If you work alone on one machine: update and carry on, and read the rest for what you gained.

## The one thing that needs attention

`CALIBRATED_AGAINST` (any node → Artifact/Verification) takes the schema from **57 edge types to
58**. The node types are unchanged at 28, and the `dynograph-foundation` pin is unchanged — there
is no slow first build.

An older reflow2 opening a design written by this one will **refuse** — loudly, naming the
mismatch, not garble it and not silently show you less. That refusal is deliberate: it is the check
that stops a design being quietly half-read (see [upgrading-to-v0.11.0.md](upgrading-to-v0.11.0.md)
and BL-94). But it means a mixed-version team has the older side **locked out**, not degraded.

| Change | Does it lock out an older reflow2? |
|---|---|
| New **edge type** (`CALIBRATED_AGAINST`) | **Yes** — the stamp counts edge types |
| New **edge properties** (`VERIFIES.pinned`, `VERIFIES.swept`) | No — the stamp does not count properties |
| New **report fields** (freshness, scope, consumed checks) | No — reports are computed, never stored |

`CALIBRATED_AGAINST` is additive; nothing was retired, and no existing node or edge changed
meaning. Every design written before this version reads exactly as it did, and every new field is
absent rather than defaulted — an unstated input scope reports as **unscoped**, never as broad.

So: upgrade together. If you are stuck on an older binary somewhere, export the design with a
current one and work from that file; the refusal message names the recovery paths.

## What you gained

**Your evidence can now say what it covers.** Until now reflow2 recorded that a check *exists* and
that it *passes*, and nothing else — so a green tick looked identical whether the check had run
this morning across the whole input space against an independent source, or eighteen months ago at
one fixed seed against the very data the thing under test was fitted to. Three axes close that:

- **TIME.** `confirmation_ledger` now reports `last_verified_at` and `verification_freshness` per
  claim — `current`, `stale`, or `unknown`. Stale means the newest passing check predates the
  newest accepted change to what it covers. Undated on either side is `unknown` and is **counted**,
  never quietly treated as a pass.

- **INPUT.** `set_evidence_scope` records what a check held fixed (`pinned`) and what it varied
  (`swept`), on the `VERIFIES` edge — so the same suite can be broad about one capability and
  narrow about another. `evidence_report` then names any parameter every passing check pinned and
  none ever swept. A check that states no scope is reported **unscoped**: silence about coverage is
  not coverage.

- **INDEPENDENCE.** `calibrated_against` records what a value was *fitted* to. Any passing check
  that is, or produced, that same evidence is reported **consumed — a fit, not a test** and does
  not count toward independent evidence. If every check of a claim is consumed, `evidence_report`
  says the claim is not independently verified.

### The one that is worth understanding before you use it

The independence axis is **recorded, never detected**, and that is not a shortcut. The project this
came from built four independent internal diagnostics against its own circular fit — a back-solve,
a coefficient locus, a space-time frontier and a physical-floor proof. Every one narrowed the
question correctly. None of them could have found it. Only the outside source could.

No check inside a design can establish its own independence, so there is nothing for an analytic
detector to compute. What reflow2 can do is make the circle **structurally visible** — hold what a
value was fitted to, and refuse to let that same evidence be counted as its validation. If you
never record the calibration, reflow2 will not infer it, and it will not pretend to.

The stakes, from the same project: a single-point fit does not merely get a number wrong. Their
fitted coefficient silently absorbed a *structural* error — the real relation had a constant term
that did not scale at all — and agreed with the data anyway. A model can be right for the wrong
reason while every status reads green.

### Nothing new fires as a gap

All three axes report **facts**, not gaps. Pinning a seed is normal; refactoring after a check ran
is normal; calibrating against a published anchor is normal. A detector that fired on those would
punish correct work, and a list that can never reach zero gets skimmed — which is the failure the
gap workflow exists to prevent. You will see these in `confirmation_ledger` and `evidence_report`,
and nowhere in `detect_gaps`.

### If you use `propagate_change`

`CALIBRATED_AGAINST` is a **traceability** edge, so it participates in blast radius, centrality and
community detection. Correcting or superseding an anchor now reaches every value fitted to it —
which is the point, and the only moment a wrong functional form hiding inside a fitted coefficient
becomes findable. If you have a large calibrated design, expect impact reports to reach further
than before along exactly those edges.
