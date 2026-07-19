# The same cycle done right — is `designed == released` reachable?

**What:** the [erosion trial](2026-07-19-erosion.md)'s five fix cycles run again, but with the
discipline axis Z was built for: every fix a `record_change` at its own epoch, and the fix that
changed *behaviour* also updating the **function** (P1 capability) — the design following the build,
backwards. Then a `release_cut` epoch. Driven by
[`tools/coherent_erosion_trial.py`](../../tools/coherent_erosion_trial.py).

**Why:** the erosion trial showed the failure. It did not show whether the target state is even
reachable. The user's framing supplied the missing half — *updating the design backwards is fine,
because Z keeps the original.* That turns "don't let the design drift" into something achievable
rather than a counsel of perfection, and it is worth knowing whether the machinery delivers it.

## Result — 4 of 9, and the split is the point

**The target state works today.** With discipline alone:

```
YES  the design now describes what was actually built    "…within 7 days."
YES  the ORIGINAL intent is still recoverable from Z     6 snapshots, "…within 24h." preserved
YES  every fix is on the record, typed                   6 ChangeEvents, all test_failure_fix
YES  the graph is quiet, because it is genuinely coherent
```

So the architecture is right, and the user's instinct about axis Z is confirmed by execution. You
*can* let the build teach the design what it actually is, arrive at release with
`designed == released`, and lose no intent doing it — the original description is sitting in a
Snapshot pinned to the baseline epoch. The vocabulary was already waiting: `ChangeType::TestFailureFix`
is documented as *"a fix forced by a failed verification"*, `ChangeType::Resync` as *"a re-sync back
to coherence"*, and `EpochType::ReleaseCut` as *"the epoch a release was cut at."* Somebody designed
for exactly this loop.

## 1. Nothing distinguishes the coherent run from the eroded one

This is the finding.

| | eroded run | coherent run |
|---|---|---|
| design describes what shipped | **no — fiction** | yes |
| `detect_gaps` | `[]` | quiet |
| reflow2's verdict | **coherent** | **coherent** |

Two graphs, one describing what shipped and one describing a system that no longer exists, and
reflow2 returns the same answer for both. **The entire difference between them is developer virtue**,
which is precisely the thing that does not survive cycle 40 of a release crunch — and precisely what
the original reflow was relying on without knowing it.

So the gap is not the ability to update the design backwards. That works. The gap is that **nothing
obliges, prompts, or even notices** whether you did.

## 2. What the graph is missing is a *date on its own claims*

Structural completeness is all reflow2 measures: is there a Capability, does something satisfy the
Requirement, does an Artifact realize it. Every one of those is true in the eroded graph.

What it has no notion of is **when a claim was last confirmed against reality.** `cap:charge`'s
description was written at the baseline epoch and never revisited while its artifact drifted five
times — and that is a different, worse state than the same description confirmed at the release
epoch, with no way to tell them apart.

Axis Z already has everything needed to compute it: epochs are ordered and sequenced, snapshots are
pinned to epochs, drift is dated. *Last confirmed at epoch N* versus *artifact last moved at epoch M*
is subtraction. The data is there and nothing reads it that way.

## 3. A `test_failure_fix` cannot say whether it moved the design

All six ChangeEvents carry the same `change_type`. Propagating from each reaches `cap:charge`
regardless — the artifact REALIZES the capability, so *every* file fix reaches it:

```
change events that reached the capability:
  ['chg:art1','chg:art2','chg:art3','chg:art4','chg:art5','chg:cap4']
```

Only `chg:cap4` actually *moved the design*; the rest touched a file. The distinction lives in what
each ChangeEvent `CHANGED` (a Capability versus an Artifact) and nothing surfaces it. "Five fixes, one
of which changed what the system does" is the sentence a release review needs and cannot get.

## 4. `precedes` is unreachable from any client

`DesignGraph::precedes` (`temporal.rs:181`) orders one epoch after another. It has **no MCP tool**,
so the `PRECEDES` chain cannot be drawn by any consumer — epoch ordering survives only via the
`sequence` integer. The tenth instance of the recurring lesson, and on the axis that exists to make
history legible.

## 5. And the release still records nothing about what it contains

Confirmed again from the other direction: the `release_cut` epoch is not linked to the `Release`, and
the Release is not linked to the artifacts it shipped. Even in the fully disciplined run, "what did
v1.0 actually contain?" is unanswerable — [BL-34](../backlog.md).

## What this says to build

The erosion trial said what breaks. This one says the target is reachable and names the five things
between here and it. Ranked:

1. **[BL-35]** a design claim needs a *last-confirmed epoch*, so coherent and unexamined stop
   looking identical. The deepest of the five, and the data is already on axis Z.
2. **[BL-33]** accepting drift must ask the second question, and when the answer is "the design
   moved", it should write exactly what the coherent run writes here — a `record_change` at an
   epoch. **This trial is the specification for that behaviour.**
3. **[BL-34]** the release must record what it shipped, or `designed == released` stays unprovable.
4. **[BL-36]** expose `precedes`.
5. A ChangeEvent should say whether it moved the design or only a file (folded into BL-35).
