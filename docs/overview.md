# Reflow 2.0 — Documentation Overview

**Start here.** This is the map that ties the rest of `docs/` together. Reflow 2.0 is a
clean-room rebuild ([github.com/sligara7/reflow2](https://github.com/sligara7/reflow2))
that captures the **entire lifecycle of a design — concept → operations — in one graph**,
so that when anything changes, the ripple effects are automatically found, surfaced to the
user as plain questions, and healed back to coherence. The user never needs to know
systems engineering; the graph does. The full argument is in [vision.md](vision.md).

## Three ways to think about the system

These are complementary lenses on the same graph, not competing models:

| Lens | Question it answers | Where it lives |
|---|---|---|
| **Phases** (P0–P5) | *Where is the project in its lifecycle?* — WHAT ▸ WHERE ▸ BUILD ▸ VERIFY ▸ OPERATE | the schema, layered by phase |
| **Three axes** (X / Y / Z) | *How is the graph structured?* — network / decomposition / change-over-time | [three-axes.md](three-axes.md) |
| **The coherence loop** | *How does the system behave when something changes?* | [vision.md](vision.md) + the process docs |

Phases are the linear spine; the axes are the structure; the coherence loop is the engine
that keeps everything in agreement.

## The coherence loop and the six universal processes

The **coherence loop** — `CHANGE → PROPAGATE → DETECT → SURFACE → RESOLVE/HEAL →
COHERENCE` — is reflow2's operating loop. Storyflow
([github.com/sligara7/storyflow](https://github.com/sligara7/storyflow)) contributed the
insight that a small set of **universal processes** recur in every domain; they map onto
the loop like this:

| Universal process | Loop step(s) | reflow2 doc | Status |
|---|---|---|---|
| **GENESIS** — bootstrap the graph from a brief | seeds the first CHANGE | — | acknowledged; not yet detailed |
| **INGEST** — extract content/edits into the graph | CHANGE | [extraction-plan.md](extraction-plan.md), [artifact-linking.md](artifact-linking.md), [sme-augmentation.md](sme-augmentation.md) | detailed |
| *(reflow2 addition)* — ripple along the golden thread | PROPAGATE | [impact-propagation.md](impact-propagation.md) | detailed |
| **DIAGNOSE → PROMPT** — find weaknesses, ask the user | DETECT + SURFACE | [gap-surfacing.md](gap-surfacing.md) | detailed |
| **HEAL** — detect & repair structural defects | RESOLVE/HEAL | [heal-process.md](heal-process.md) | detailed |
| **SYNTHESIZE** — graph → artifacts (docs, diagrams, as-built) | reporting side-output | — | acknowledged; not yet detailed |

## Reading order

### 1 · Vision — *why*
- [vision.md](vision.md) — the north star: one coherent graph, concept → operations, and the coherence loop with a worked example.

### 2 · Design — *what the graph is*
- [three-axes.md](three-axes.md) — the X (network) / Y (decomposition) / Z (change-over-time) structure.
- [operating-environment.md](operating-environment.md) — the environment's authoritative ruleset the design must comply with (Kennewick vs. Mars).
- [artifact-linking.md](artifact-linking.md) — how real files (code, specs, docs, tests) and the note layer link to entities.
- [interaction-surfaces.md](interaction-surfaces.md) — how a human drives the system (MCP/skills vs. hosted app vs. …) and the LLM-sourcing consequence; a deliberately deferred decision.
- `../schema/*.yaml` — the 10 composable dynograph domains (26 node types, 52 edge types); run `../tools/validate_schema.py` to check them.

### 3 · Process — *how it runs (the coherence loop)*
- [extraction-plan.md](extraction-plan.md) — INGEST: the phase-aware multi-pass extraction pipeline.
- [sme-augmentation.md](sme-augmentation.md) — the LLM-as-subject-matter-expert that surfaces considerations the user didn't state.
- [impact-propagation.md](impact-propagation.md) — PROPAGATE: walk the golden thread to find a change's blast radius.
- [gap-surfacing.md](gap-surfacing.md) — DIAGNOSE → PROMPT: turn weaknesses into constructive questions.
- [heal-process.md](heal-process.md) — HEAL: propose and apply structural repairs.

### 3½ · Status — *are we meeting the docs?*
- [requirements-coverage.md](requirements-coverage.md) — traceability matrix: every doc requirement → the module + test that meets it, with an honest Met/Partial/Deferred status. A living document.

### 3¾ · Direction — *the prescriptive graph* (candidate, not yet built)
- [graph-analysis.md](graph-analysis.md) — use graph-theory tools (`dynograph-graph`/`-vector`/`-cluster`/`-game`) to *make* design decisions, not just record them — e.g. allocate functions to services by cohesion/coupling instead of by domain. Starts with edge **weights**.

### 4 · Heritage — *where the ideas came from*
- [reflow-v3-nuggets.md](reflow-v3-nuggets.md) — ideas carried over from the original Reflow.
- [chain-reflow-nuggets.md](chain-reflow-nuggets.md) — ideas from Chain Reflow (matryoshka, causality, linking).

## Heritage & references

All the projects reflow2 draws on are the author's own work under
**[github.com/sligara7](https://github.com/sligara7)**:

| Repo | What reflow2 takes from it |
|---|---|
| [reflow2](https://github.com/sligara7/reflow2) | this project (the clean-room rebuild) |
| [integrated_reflow](https://github.com/sligara7/integrated_reflow) | the prior graph+MCP iteration this redesign grew out of |
| [storyflow](https://github.com/sligara7/storyflow) | the extraction pipeline, universal processes, cosmology/ruleset, SME analysis, notes, three axes |
| [dynograph-foundation](https://github.com/sligara7/dynograph-foundation) | the schema-driven graph **store** (and `dynograph-extract`) reflow2 builds on |
| [chain_reflow](https://github.com/sligara7/chain_reflow) | matryoshka nesting, correlation-vs-causation, creative linking, system-of-systems |
| [reflow](https://github.com/sligara7/reflow) | the original systems-engineering workflow: phases, as-designed/built/fielded, framework packs |
| [dev_storyflow](https://github.com/sligara7/dev_storyflow) | storyflow's design docs (five universal processes, entity model) |

**Third-party exceptions** (dependencies, not conceptual content): dynograph-foundation
builds on external Rust crates (RocksDB, Tantivy for BM25, an HNSW vector index, serde);
extraction calls external LLM providers (e.g. OpenRouter and the models it routes to).
Everything conceptual in these docs is the author's own.
