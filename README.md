# Reflow Redesign — "Design Anything, Build Anything"

> ### 👉 Just want to use it? Read **[getting-started/SETUP.md](getting-started/SETUP.md)**.
> The **[getting-started/](getting-started/)** folder is the complete, non-developer setup:
> build the server, connect grok build / claude code, verify it works. Everything below is
> about reflow2's own internals.

A clean-room rebuild of Reflow's core idea
([github.com/sligara7/reflow2](https://github.com/sligara7/reflow2)), starting fresh so
nothing in the source projects is disturbed — all of them the author's own work:
[reflow](https://github.com/sligara7/reflow),
[storyflow](https://github.com/sligara7/storyflow),
[chain_reflow](https://github.com/sligara7/chain_reflow), and
[dynograph-foundation](https://github.com/sligara7/dynograph-foundation).

**New to the internals? Read [docs/overview.md](docs/overview.md) first** — it maps all the
documents and how they fit together.

## Vision

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

## "How do you know the LLM didn't just hallucinate something?"

It's the first question anyone asks about a tool that works with an LLM, and it deserves a
straight answer rather than reassurance. reflow2 is built as a **three-party system** — you, the
graph, and the LLM — where each party checks the others, and the LLM is structurally barred from
the jobs it is bad at. The honest version, objection by objection:

**"It invents things."** Structurally, it can't land them. The graph's vocabulary is
schema-enforced: an invented node type, edge type or property is *refused*, loudly, with the real
alternatives named. Structural repairs execute only if the engine independently re-derives them
from the graph at apply time — a hallucinated merge is rejected before a single write. And what the
LLM *can* freely write — descriptions, statements, prose — is attributed, dated, and marked with
provenance (`inferred` when it was read out of code rather than stated by you), so a claim is never
just a sentence: it is a sentence with a paper trail.

**"It forgets, and it drifts."** The graph is the memory, not the context window. Questions you
were asked persist across sessions *in the exact words you saw*; decisions are recorded with their
rationale; the same graph produces byte-identical exports and the same gaps in any session, on any
machine. The deterministic core — not the LLM — does all counting, ranking and graph analysis, so
there is no arithmetic to hallucinate.

**"It just agrees with you."** The detectors don't negotiate. A gap re-fires every run until the
structure actually changes or a *human* accepts it with a recorded reason — agreement has to leave
a Decision node, not a pleasant sentence. When built code drifts from the design, accepting the new
reality **requires** answering "did the design move too?" — the agreeable silent path was removed
on purpose, because it is how a design erodes into fiction while reporting zero problems.

**What it can't do — said plainly.** No mechanism here stops an LLM from writing a false sentence
into a description. What the graph guarantees instead is that the sentence is *checkable*: the
confirmation ledger shows, per capability, whether anyone has examined the claim against reality —
and `unexamined` is a visible state, never silently equal to "fine." The judgment seat belongs to
you; the machinery's job is making sure nothing reaches you unattributed, uncounted, or quietly
forgotten.

The full map — every known LLM failure mode against the mechanism that checks it, including the
ones still uncovered — is **[docs/partnership.md](docs/partnership.md)**. It is kept honest the
same way everything else here is: coverage is claimed only where a named mechanism enforces it.

## The design vocabulary

Domain-neutral node types, layered by the phase they feed (26 types across 10 schema
domains; see [docs/overview.md](docs/overview.md) and `../tools/validate_schema.py`):

| Phase / layer | Nodes |
|-------|-------|
| P0 · Intent | `Project`, `Requirement`, `Constraint`, `DesignRule` |
| P1 · Function (WHAT) | `Capability`, `Flow`, `Actor` |
| P2 · Structure (WHERE) | `Component`, `Interface`, `Decision`, `Anchor` |
| P3 · Realization (BUILD) | `Artifact`, `Fragment` |
| P4 · Verification | `Verification`, `QualityGate`, `DriftEvent` |
| P5 · Operation | `Release`, `Environment`, `Resource` |
| Operating environment | `EnvironmentRule` |
| Axis Z · change over time | `DesignEpoch`, `TemporalFact`, `Snapshot`, `ChangeEvent` |
| Cross-cutting | `DimensionAssessment`, `DimensionObservation` |

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
    environment.yaml #   operating environment + its authoritative ruleset (codes/laws)
    temporal.yaml    #   axis Z — epochs, time-bounded facts, snapshots, change events
    inference.yaml   #   the "why" edge layer
    dimensions.yaml  #   quality-axis assessments + per-epoch observations
  docs/
    overview.md          # START HERE — maps all docs and how they fit together
    vision.md            # north star: one coherent graph, concept → operations
    three-axes.md        # X (network) / Y (nesting) / Z (change over time)
    extraction-plan.md   # how phase-aware extraction populates the graph (INGEST)
    artifact-linking.md  # link real files (code, specs, docs, tests) to entities
    sme-augmentation.md  # LLM-as-SME surfaces considerations the user didn't state
    impact-propagation.md# ripple a change along the golden thread (PROPAGATE)
    gap-surfacing.md     # find graph weaknesses, ask the user questions (DIAGNOSE→PROMPT)
    heal-process.md      # self-repair of the design graph (HEAL)
    operating-environment.md # the environment's ruleset the design must comply with
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
on the graph regardless of phase. They map onto the coherence loop; see
[docs/overview.md](docs/overview.md) for the full reconciliation.

- **GENESIS** — bootstrap the graph from a brief ([docs/genesis.md](docs/genesis.md))
- **INGEST** — extraction ([docs/extraction-plan.md](docs/extraction-plan.md))
- **DIAGNOSE → PROMPT** — find graph weaknesses & ask the user questions ([docs/gap-surfacing.md](docs/gap-surfacing.md))
- **SYNTHESIZE** — graph → artifacts (docs, diagrams, as-built) *(acknowledged; not yet detailed)*
- **HEAL** — detect & repair structural defects ([docs/heal-process.md](docs/heal-process.md))
- *(reflow2 addition)* **PROPAGATE** — ripple a change along the golden thread ([docs/impact-propagation.md](docs/impact-propagation.md))

## Status

The deterministic core and the **agent-native surface are built** — the full surface plan
(persistence, ambient-agent LLM seam, the `reflow2-mcp` MCP server, consumer kit, GENESIS,
artifact linking) is complete and cold-start-verified. See
[docs/requirements-coverage.md](docs/requirements-coverage.md) for the living status matrix and
[docs/surface-plan.md](docs/surface-plan.md) for what's built vs. the tracked future
improvements (SP-3b ingest extraction, SP-6b as-built drift). To *use* it, see
[getting-started/](getting-started/).
