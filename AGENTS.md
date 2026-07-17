# AGENTS.md — orientation for AI agents working on Reflow 2.0

Read this first. It tells you what this project is, how it's organized, and the rules to
follow so your changes stay coherent with the design.

## What Reflow 2.0 is

A graph-backed system that partners with an LLM agent to **design and build anything** —
software, hardware, a document, a full acquisition program. It captures the **entire
lifecycle of a design (concept → operations) in one knowledge graph**, tied together by
the systems-engineering *golden thread* (traceability from every artifact back to the
intent it serves).

The payoff: when **anything changes in any phase**, the ripple effects are automatically
found, surfaced to the user as plain questions, and healed back to coherence — so concept
through operations always stays in agreement. **The user never needs to know systems
engineering; the graph does.**

This is a clean-room rebuild ([github.com/sligara7/reflow2](https://github.com/sligara7/reflow2))
of ideas from the author's earlier projects (all under
[github.com/sligara7](https://github.com/sligara7)): `reflow`, `storyflow`,
`chain_reflow`, and the graph engine `dynograph-foundation`.

## The one mental model to hold: the coherence loop

```
CHANGE → PROPAGATE → DETECT → SURFACE → RESOLVE/HEAL → COHERENCE
```

- **CHANGE** — any edit becomes a `ChangeEvent` at a `DesignEpoch` (the old state is snapshotted, never overwritten).
- **PROPAGATE** — walk the traceability edges to compute the blast radius.
- **DETECT** — re-diagnose the touched region for new gaps/contradictions.
- **SURFACE** — turn those into constructive, plain-language questions for the user.
- **RESOLVE/HEAL** — the user answers (re-ingested) or HEAL proposes structural fixes.

Three complementary lenses on the graph: **phases** (P0–P5 lifecycle), **three axes**
(X = network, Y = decomposition, Z = change-over-time), and this **loop** (behavior).

## Current state (important)

This project is **early implementation**. The docs + schema remain the source of truth;
runtime code has begun with the **deterministic, LLM-free core** (build-order steps 1–2 of
[docs/interaction-surfaces.md](docs/interaction-surfaces.md)). Do not assume any surface,
service, or LLM wiring exists yet — none does.

- `crates/reflow2-core/` — Rust crate implementing the deterministic coherence-loop
  spine so far. Modules: `schema` (loads the 10 domains into one merged dynograph
  `Schema`), `graph` (`DesignGraph` over `dynograph-storage`, in-memory backend, schema-
  validated CRUD + typed golden-thread constructors), `temporal` (axis Z — `DesignEpoch` /
  `ChangeEvent` / `Snapshot` and `record_change`, the **CHANGE** step: snapshot the past,
  never overwrite), `propagate` (**PROPAGATE** — direction-classified bounded BFS over the
  golden thread → an explained `BlastRadius`; reactive from a `ChangeEvent` or speculative
  from seeds), `detect` (**DETECT** — deterministic gap detectors → ranked `GapCandidate`s;
  traceability + phase-coverage groups, gated on type-population counts), `heal` (**HEAL** —
  detect structural defects → a `HealProposal` (propose, never mutate) → atomic `apply_heal`
  with post-repair verification; mode-aware (rigid = propose-only), strategy-filtered,
  human-review-gated for generative fixes; content-free duplicate-merge is the applied
  repair). Consumes `dynograph-foundation` by git tag (`v0.9.4`), `dynograph-storage` with
  `default-features = false` so the RocksDB C++ build stays opt-in — mirrors the predecessor
  `ir2`. Fast dev/test build: `cargo test --no-default-features`. Keep it green, clippy-clean,
  and `cargo fmt`-ed. Not yet built (LLM/interaction-surface-gated or need `dynograph-graph`):
  the PROMPT/rephrase half of SURFACE, HEAL's generative healers + graph-algorithm defects,
  INGEST, SME, GENESIS.

**Open decision (deliberately deferred):** the *interaction surface* — MCP/skills for a
coding agent, a hosted web app, a CLI, or a library — is not yet chosen. It plugs in last
and determines whether an external LLM provider is needed (agent-native = no; hosted =
yes). The core is built to be neutral to this; see
[docs/interaction-surfaces.md](docs/interaction-surfaces.md). Don't hard-wire a surface or
an LLM provider into the core.

- `schema/*.yaml` — 10 composable [dynograph-foundation](https://github.com/sligara7/dynograph-foundation)
  schema domains (26 node types, 52 edge types). This is the foundation everything builds on.
- `docs/*.md` — the vision, design, and process specifications.
- `tools/validate_schema.py` — validates the schema against dynograph-core's rules.

## Where to look

**Always start with [docs/overview.md](docs/overview.md)** — it maps every document and the
reading order (Vision → Design → Process → Heritage). Then:

| You want to… | Read |
|---|---|
| understand the "why" | [docs/vision.md](docs/vision.md) |
| understand the graph structure | [docs/three-axes.md](docs/three-axes.md), `schema/` |
| know how content gets into the graph | [docs/extraction-plan.md](docs/extraction-plan.md), [docs/sme-augmentation.md](docs/sme-augmentation.md), [docs/artifact-linking.md](docs/artifact-linking.md) |
| know how change is handled | [docs/impact-propagation.md](docs/impact-propagation.md), [docs/gap-surfacing.md](docs/gap-surfacing.md), [docs/heal-process.md](docs/heal-process.md) |
| understand the operating environment/ruleset | [docs/operating-environment.md](docs/operating-environment.md) |
| know how a human drives it (and the LLM-sourcing tradeoff) | [docs/interaction-surfaces.md](docs/interaction-surfaces.md) |
| see where ideas came from | [docs/reflow-v3-nuggets.md](docs/reflow-v3-nuggets.md), [docs/chain-reflow-nuggets.md](docs/chain-reflow-nuggets.md) |

## Rules for changing this project

1. **Schema-first.** The node/edge vocabulary is load-bearing. After any `schema/*.yaml`
   edit, run `python3 tools/validate_schema.py` (needs PyYAML — on this machine use
   `~/miniconda3/bin/python`). It must print "OK".
2. **Keep docs cohesive.** Every doc carries a breadcrumb to `overview.md`; new docs must
   too, and must be added to the overview's document map and reading order.
3. **Terminology matches the schema.** Use the real node/edge names (e.g. `Capability`,
   `Component`, `Artifact`, `Verification`, `Environment`, `EnvironmentRule`,
   `ChangeEvent`). Do not reintroduce retired names from old Reflow (e.g. `PhaseEvent`,
   `ContractedFunction`, `APIEndpoint`).
4. **Honor the disciplines** the process docs call "non-negotiable" — most importantly
   **no silent fallbacks / no silent drops**: surface failures and skipped items loudly;
   never let data loss or an unstated assumption pass as success.
5. **References are the author's own** under `github.com/sligara7`. The only third-party
   pieces are dependencies (dynograph-foundation's RocksDB/Tantivy/HNSW/serde; LLM
   providers like OpenRouter) — never conceptual content.
6. **Don't touch the sibling source repos** (`../../storyflow`, etc.) — mine them for
   ideas, but all new work lands here.

## Provenance of the ideas (so you can trace any decision)

- **storyflow** → the extraction pipeline, the six universal processes, the operating-
  environment ruleset (its "cosmology"), SME/supplementary analysis, the note layer, the three axes.
- **chain_reflow** → matryoshka/missing-intermediate detection, correlation-vs-causation
  rigor, creative linking, system-of-systems.
- **reflow (v3)** → the phase spine, as-designed/as-built/as-fielded fidelity views,
  framework packs, root-cause change classification.
- **dynograph-foundation** → the schema-driven graph store (RocksDB + HNSW + BM25 +
  fuzzy/vector resolution) reflow2 targets.
