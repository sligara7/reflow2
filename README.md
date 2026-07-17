# Reflow Redesign — "Design Anything, Build Anything"

A clean-room rebuild of Reflow's core idea, starting fresh so nothing in the
existing repos (`../` reflow, `../../storyflow`, `../../dynograph-foundation`)
is disturbed.

## Vision (start here)

Capture the **entire lifecycle — concept → operations — in one graph**, tied together by
the systems-engineering *golden thread*. When anything changes in any phase, the ripple
effects are **automatically detected, surfaced to the user as plain questions, and healed**
back to coherence — so concept through operations always stays in agreement. The user
never needs to know systems engineering; the graph does. See
[docs/vision.md](docs/vision.md) — it's the north star for everything below.

The engine is the **coherence loop**: `CHANGE → PROPAGATE → DETECT → SURFACE → HEAL →
COHERENCE` — where PROPAGATE walks the golden thread to find a change's blast radius
([docs/impact-propagation.md](docs/impact-propagation.md)).

## What this is

A graph-backed workflow engine that partners with an LLM agent to **design and
build anything** — not just software. It keeps Reflow's phase spine
(**WHAT → WHERE → BUILD → VERIFY → OPERATE**) but swaps two foundations:

1. **Store**: `dynograph-foundation` (schema-driven Rust graph engine:
   RocksDB + HNSW vectors + BM25 text + fuzzy/vector entity resolution)
   replaces Neo4j.
2. **Design capture**: instead of hand-calling CRUD tools, freeform design
   input is **extracted** into the graph via the storyflow / `dynograph-extract`
   pattern — schema-driven, phase-aware, multi-pass, with graph-informed
   dedup.

## The design vocabulary

Domain-neutral node types, layered by the phase they feed:

| Phase | Nodes |
|-------|-------|
| P0 · Intent | `Project`, `Requirement`, `Constraint`, `DesignRule` |
| P1 · Function (WHAT) | `Capability`, `Flow`, `Actor` |
| P2 · Structure (WHERE) | `Component`, `Interface`, `Decision`, `Anchor` |
| P3 · Realization (BUILD) | `Artifact`, `Fragment` |
| P4 · Verification | `Verification`, `QualityGate`, `DriftEvent` |
| P5 · Operation | `Release`, `Environment`, `Resource` |
| Cross-cutting | `DimensionAssessment`, `PhaseEvent` |

**Structural edges:** CONTAINS, PROVIDES, CONSUMES, ALLOCATED_TO, REALIZES,
VERIFIES, DEPENDS_ON, SATISFIES, PART_OF_FLOW, DEPLOYED_TO, REQUIRES_RESOURCE,
GOVERNED_BY.

**Inference ("why") edges** (wildcard endpoints): CAUSES, ENABLES, BLOCKS,
TRIGGERS, CONTRADICTS, VALIDATES, VIOLATES, RISKS, MITIGATES, EVOLVES_INTO,
OBSOLETES, DUPLICATES, CONSTRAINS, ANTICIPATES, MASKS.

## Layout

```
redesign/
  README.md
  schema/            # composable dynograph schema domains (one concern each)
    core.yaml        #   P0 — intent, constraints, rules
    functional.yaml  #   P1 — capabilities, flows, actors
    structure.yaml   #   P2 — components, interfaces, decisions
    build.yaml       #   P3 — artifacts, fragments
    verify.yaml      #   P4 — verifications, gates, drift
    operate.yaml     #   P5 — releases, environments, resources
    temporal.yaml    #   axis Z — epochs, time-bounded facts, snapshots, change events
    inference.yaml   #   the "why" edge layer
    dimensions.yaml  #   quality-axis assessments + per-epoch observations
  docs/
    vision.md            # north star: one coherent graph, concept → operations
    three-axes.md        # X (network) / Y (nesting) / Z (change over time)
    extraction-plan.md   # how phase-aware extraction populates the graph (INGEST)
    impact-propagation.md# ripple a change along the golden thread (PROPAGATE)
    gap-surfacing.md     # find graph weaknesses, ask the user questions (DIAGNOSE→PROMPT)
    heal-process.md      # self-repair of the design graph (HEAL)
    reflow-v3-nuggets.md # ideas carried over from the original Reflow project
    chain-reflow-nuggets.md # ideas from Chain Reflow (matryoshka, causality, linking)
```

## Three structural axes

Beyond phases and processes, every design is sliced along three independent axes
([docs/three-axes.md](docs/three-axes.md)):

- **X — who relates to whom**: the horizontal network of entities + typed/inference edges
- **Y — how it's built**: the vertical decomposition spine (Project ▸ Component ▸ Capability ▸ Artifact)
- **Z — how it changes**: the time axis — epochs, time-bounded facts, snapshots, change events ([schema/temporal.yaml](schema/temporal.yaml))

## Phases and processes

Reflow's **phases** (P0–P5) are the *linear lifecycle spine* — where a project is.
Storyflow contributes six *universal graph processes* — the *cyclic engine* that runs
on the graph regardless of phase:

- **GENESIS** — bootstrap the graph from a brief
- **INGEST** — extraction ([docs/extraction-plan.md](docs/extraction-plan.md))
- **DIAGNOSE → PROMPT** — find graph weaknesses & ask the user questions ([docs/gap-surfacing.md](docs/gap-surfacing.md))
- **SYNTHESIZE** — graph → artifacts (docs, diagrams, as-built)
- **HEAL** — detect & repair structural defects ([docs/heal-process.md](docs/heal-process.md))

## Status

Bootstrapping. Schema-first: the node/edge vocabulary is the foundation
everything else builds on.
