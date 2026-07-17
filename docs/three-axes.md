# The Three Axes — and why Z (change over time) matters most for design

From the user's model of storyflow (`~/Downloads/storyflow_entities.html`, grounded in
`storyflow/services/dynograph/schemas/domains/*.yaml`). storyflow slices any story along
three independent axes; a design graph has the exact same three, plus a fourth "depth".

| Axis | storyflow framing | In integrated_reflow 2.0 | Status in our schema |
|---|---|---|---|
| **X — who relates to whom** | the horizontal web of entities + typed ties | the design-as-network: DEPENDS_ON, SATISFIES, ALLOCATED_TO, REALIZES, PROVIDES/CONSUMES + the inference "why" edges | **covered** (core/functional/structure/inference) |
| **Y — how it's built** | the vertical spine: universe ▸ story ▸ arc ▸ chapter ▸ fragment | the decomposition spine: Project ▸ Component(subsystem▸assembly▸part) ▸ Capability ▸ Artifact | **partly** (CONTAINS + Component.kind; formalized below) |
| **Z — how it changes** | the time axis: epochs, time-bounded facts, snapshots | design evolution: requirement creep, new features, fixes to failed tests, scope changes | **MISSING** (only SUPERSEDES + DriftEvent + status) |
| *depth* (4th) | 10-dim profile per entity, drifting over time | DimensionAssessment, now with per-epoch observations | **enhanced below** |

---

## Axis Z is the one we were missing

A design is never static. Requirements creep, features get added, a failed test forces a
fix, scope shifts. Today our schema only knows the *current* state (status enums) plus a
thin drift signal. It cannot answer:

- *What did the design look like two revisions ago?*
- *Which requirement was added late (creep), and what did it cascade into?*
- *This Component's maintainability has been sliding — since when?*
- *This Artifact changed because a Verification failed — show that chain.*

storyflow solved exactly this with a temporal layer: **named epochs**, **time-bounded
facts** (`valid_from` → `valid_to`), and **snapshots** pinned to epochs — "the graph
doesn't overwrite the past; it remembers it." We adopt the same, design-neutral, in
[`../schema/temporal.yaml`](../schema/temporal.yaml):

- **`DesignEpoch`** — a named version/milestone of the design ("v1 baseline",
  "v1.1 requirements creep", "post-load-test hardening"). Generalizes the P2→P3
  `Anchor` (which becomes *one kind of* epoch snapshot).
- **`TemporalFact`** — a time-bounded assertion: an edge/fact true only between two
  epochs. Requirement creep = a `SATISFIES` fact whose `valid_from` is a later epoch; a
  dropped requirement = a fact with `valid_to` set.
- **`Snapshot`** — the state of any node captured at an epoch, so you can diff a
  Component across revisions.
- **`ChangeEvent`** — a first-class record of *why the design changed*:
  `requirement_creep`, `new_feature`, `test_failure_fix`, `refactor`, `scope_change`,
  `constraint_change`, `deprecation`. Wired to what it changed (`CHANGED`) and, via the
  existing inference edges, to what caused it (a failed `Verification` `CAUSES` a
  `ChangeEvent` that `CHANGED` an `Artifact`).

### Change over time also applies to *depth*

Quality isn't static either. We add **`DimensionObservation`** (immutable, per-fragment,
per-epoch) that rolls up into the current `DimensionAssessment` — so a Component's
`maintainability` *drift* across epochs is queryable. This mirrors storyflow's
observation→assessment rollup (§6 of the entities page).

---

## Axis Y — formalize the decomposition spine

storyflow's Y is a strict nesting (universe ▸ story ▸ arc ▸ chapter ▸ fragment). Ours is
looser: `Project CONTAINS *` and `Component.kind ∈ {subsystem, assembly, part, …}`. That
is enough to nest, but we make the *zoom levels* explicit so DIAGNOSE/gap-surfacing and
context-assembly can reason by level (this is the `scope` field already used in
gap-surfacing: `project / phase / component / capability`). No new nodes needed —
`CONTAINS` between Components already expresses the spine; we just name the levels.

**Chain Reflow's matryoshka insight sharpens this.** `Component.level ∈ {component,
subsystem, system, system_of_systems, enterprise}` makes the spine explicit — and enables
the highest-value Y detector: a gap is often a **missing intermediate level**, not a
missing peer link (the *carburetor-to-body problem* — don't link a component straight to a
system; find the missing subsystem between them). See
[gap-surfacing.md](gap-surfacing.md) (`missing_intermediate_level`) and
[chain-reflow-nuggets.md](chain-reflow-nuggets.md).

---

## Axis X — already the strongest

The horizontal web is what Reflow always did well and what our core/functional/structure/
inference domains already encode. storyflow's lesson here is *breadth of edge vocabulary*
(149 edge types!) — but we deliberately keep a tight structural set + the wildcard
inference layer, and let extraction attach rich `evidence`/`confidence` rather than
minting hundreds of edge types. If a domain needs more, add an edge type to a domain
pack (the schema is composable), exactly as storyflow did.

---

## Why this matters for "design anything"

The whole promise of a graph-backed design tool is that it **holds the history**, not
just the snapshot. Anchors + drift detection were Reflow's first stab at Z; storyflow
shows the mature version. With the Z axis in place, integrated_reflow 2.0 can:

- show a design's evolution and diff any two epochs,
- attribute every change to its cause (creep vs. fix vs. new feature),
- feed DIAGNOSE ("this requirement was added late and nothing verifies it yet"),
- and give HEAL/gap-surfacing a time-aware view ("this bond went stale three revisions
  ago").
