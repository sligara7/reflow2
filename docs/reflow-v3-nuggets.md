# Nuggets from the original Reflow (v3.17.0)

> Part of the **Reflow 2.0** design docs — see **[overview.md](overview.md)** for the full map and reading order.

Review of [github.com/sligara7/reflow](https://github.com/sligara7/reflow) (the first
project — Nov/Dec 2025, now dormant),
focused on `tools/`, `workflows/`, `workflow_steps/`. It was already
"LLM-driven systems engineering with proactive drift prevention," framework-agnostic
across UAF, systems biology, social networks, ecosystems. Several ideas are worth
carrying into the redesign; a few directly *validate* choices we already made.

## Adoption table

| Nugget (from v3) | Value for the vision | Map to redesign | When |
|---|---|---|---|
| **As-designed / as-built / as-fielded** architecture, each `--compare-to` the design (`generate_as_built_architecture.py`, `generate_as_fielded_architecture.py`) | **THE acquisitions coherence problem** — the gap between what was designed, what got built, and what's fielded is exactly the build→operate failure the vision targets | New: model three **fidelity views** over the same graph; comparison runs through impact-propagation/DriftEvent | **adopt (high)** |
| **Root-cause change classification** (`version_architecture.py`: `requirements_creep`, `performance_optimization`, `refactor`) + "mandatory resync when similarity < 0.7" | validates our Axis-Z `ChangeEvent.change_type`; the similarity threshold is a concrete drift trigger | enrich `ChangeEvent.change_type`; add a similarity-threshold resync trigger to HEAL/impact | **adopt (now, small)** |
| **Framework packs** (UAF 1.2, Systems Biology, Social Networks, Ecological, Complex Adaptive, Decision Flow, Custom) | the concrete mechanism for "design anything" — a *framework* supplies the vocabulary | our composable `schema/*.yaml` domains **already are this**; bundle them into named **framework packs** (ship a UAF pack for SE) | **adopt (concept now)** |
| **GAN-inspired validation** (Generator executes blind ↔ Discriminator validates vs. ground truth; similarity 0–1, strict/relaxed) | objective validation with "no conflict of interest" — separates making from judging | apply to validating extraction output, HEAL proposals, and generated fill content; feeds `QualityGate` | **adopt (later)** |
| **Bayesian architecture optimization** (`tools/bayesian_optimization/`: optimize DAG trade-offs — complexity, coupling, context, coordination) | turns "is this a good structure?" into a principled multi-objective search | objectives = our `DimensionAssessment` axes; a future "suggest a better decomposition" capability | **explore (later)** |
| **Interface Contract Documents + port registry + language-native contracts** (Python ABC / TS / Rust / C++ / Java / Go) | interface rigor — the contract is generated + checkable, not prose | `Interface` node → SYNTHESIZE generates ICDs; a project-level port/version registry | **adopt (later)** |
| **Bottom-up integration / reverse engineering** workflows (`01b`, `01e`) | ingest an existing system into the graph | an INGEST mode that AST-scans code → nodes (also the source of the *as-built* view) | **adopt (medium)** |
| **10 quality gates (7 blocking)** | phase-gate discipline | our `QualityGate` node; wire blocking gates into phase transitions | **adopt (medium)** |
| **NetworkX system-of-systems analysis** | centrality/graph analytics | already covered by `dynograph-graph` crate | **have it** |

## The big one: three fidelity views

Reflow v3's strongest, most acquisitions-relevant idea. The **same** system exists in
three fidelities, and drift *between them* is what breaks programs:

| View | What it is | How the graph gets it |
|---|---|---|
| **as-designed** | the intent — requirements, design, allocations | authored/extracted (our normal INGEST) |
| **as-built** | what was actually realized | reverse-engineered from code/artifacts (INGEST from source) |
| **as-fielded** | what's actually deployed & operated | captured from the running system, per `Environment` |

Coherence = these three agreeing. A `Capability` that's as-designed but has no as-built
`Artifact` is a build gap; an as-fielded `Release` whose `Resource` usage diverges from
as-designed is an operate gap. This is a **sharper framing of drift** than a single
`DriftEvent`: instead of "graph vs. filesystem," it's "intent vs. realization vs.
reality," across all phases.

**Proposed modeling (for confirmation, not yet applied):** tag realization/operation
nodes with a `fidelity ∈ {as_designed, as_built, as_fielded}` (or keep three linked
snapshots per node), and let impact-propagation compute the *inter-view* gaps that
gap-surfacing then raises as questions ("the fielded system uses 8 GB RAM but the design
budgeted 4 GB — reconcile?").

## Also worth stealing

- **Semantic-versioned architecture** with an audit trail (`version_architecture.py`) —
  maps cleanly onto `DesignEpoch` + `ChangeEvent`; keep the semantic-version label on
  epochs.
- **`matrix_gap_detection` / `reflow_gap_closure`** — an earlier DIAGNOSE/HEAL pair; the
  gap-surfacing + heal docs supersede them, but confirm we cover their gap categories.
- **`analyze_abstraction_level`** — checks nodes sit at a consistent level of abstraction;
  a useful gap-surfacing detector ("this Capability mixes strategy and implementation
  detail").
- **Pixi** for fast reproducible envs — a build-tooling choice for when we write code.

## What we already do better

- **Store**: dynograph-foundation (RocksDB + HNSW + BM25 + resolution) vs. v3's JSON
  graph files + NetworkX. The graph is now a real, queryable, versioned store.
- **Extraction**: v3 parsed docs with bespoke tools; we adopt storyflow's battle-tested
  multi-pass, graph-informed pipeline.
- **Change over time**: v3 versioned whole architecture files; Axis-Z models change at
  node/edge granularity with cause attribution.
