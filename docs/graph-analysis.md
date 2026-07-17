# Graph Analysis — the prescriptive graph (weights + the analysis crates)

> Part of the **Reflow 2.0** design docs — see **[overview.md](overview.md)** for the full map and reading order.

> **Status: design / candidate process. Nothing here is built yet** — this doc records the
> direction so it isn't lost. Each part lists the increment that would realize it. Deferral
> discipline (AGENTS.md): a recorded direction, not a silent stub.

## The idea in one line

reflow2 already holds the whole design as a graph. So the graph shouldn't only **record**
design decisions — it should help **make** them. Where classic systems engineering allocates
functions to services "by domain" or "by function" (manual clustering by a chosen metric),
a graph-backed design can let the design's *own structure* propose the boundaries — and score
them for cohesion, efficiency, reliability, or avoidance of "god nodes" — using graph theory.

This is the shift from a **descriptive** graph to a **prescriptive** one, and it is the
vision taken to its conclusion: *"the user never needs to know systems engineering; the graph
does."* Allocation is the headline example, but the same lens applies across reflow2.

The classic SE goal — **high cohesion, low coupling** — is not an analogy for community
detection; it is the same optimization. That is why this is worth doing properly.

---

## Prerequisite: edge weights (do this first)

Topology alone gives *candidates*, not good answers. To rank allocations, minimize the right
cut, or find the real god-nodes, the interaction edges need **weights** — how strongly two
things are coupled. Today the schema has almost none: inference edges carry `confidence`
(`inference.yaml`), and `DEPENDS_ON` carries `dependency_type`/`optional`, but no structural
edge carries a numeric coupling weight.

### 1. Two weight *meanings* — the load-bearing correctness rule

`dynograph-graph`'s own docs are explicit that an edge weight means one of two things
depending on the algorithm, and **the caller must supply the right kind**:

- **strength** (higher = *stronger* tie) — for `degree_centrality`, `pagerank`,
  `eigenvector_centrality`, and as **capacity** for `max_flow_min_cut`;
- **cost** (higher = *farther*) — for the shortest-path measures `closeness_centrality`,
  `betweenness_centrality`, and `shortest_path`, which require strictly positive weights.

Coupling is naturally a **strength**. So for cohesion/clustering/min-cut we feed strength
directly; for co-location / shortest-path we must feed a **derived cost** (e.g. `1 /
strength` or `max_strength − strength`). Mixing these up silently corrupts every result — this
is exactly the kind of quiet-wrong that the no-silent-fallbacks discipline forbids. The
analysis layer must convert strength→cost explicitly and never hand a strength to a cost
algorithm.

### 2. What to add to the schema

A composable weight facet on the **interaction** edges (structural coupling + flows):
`DEPENDS_ON`, `PROVIDES`, `CONSUMES`, `PART_OF_FLOW`, `ALLOCATED_TO`. Proposed properties
(all optional, so extraction can omit what it can't estimate):

```yaml
weight:     { type: float, range: [0.0, 1.0], description: "Coupling STRENGTH (higher = tighter). Interpreted as strength/capacity; invert to a cost for shortest-path/closeness/betweenness." }
frequency:  { type: float, description: "Interactions per unit time, if known (raw signal behind weight)." }
data_volume:{ type: float, description: "Bytes/records per interaction, if known (raw signal behind weight)." }
weight_basis: { type: enum, values: [estimated, evidence, measured], default: estimated, description: "How the weight was set — an LLM estimate, grounded in evidence, or measured from telemetry. Mirrors the inference-edge basis/confidence rigor." }
```

Inference edges already have `confidence` — reuse it as the weight where an inference edge
participates in an analysis (a low-confidence `RISKS` should carry less weight than a
validated one).

### 3. Where weights come from (sourced, never faked)

- **Extraction estimate** — the INGEST passes can propose a `weight` with `weight_basis:
  estimated` (an LLM-reasoning op through `LlmBackend`).
- **Evidence-grounded** — bumped to `evidence` when a Fragment/artifact substantiates it.
- **Measured** — `measured` from real telemetry (call counts, payload sizes) once an
  operating system feeds back (axis-Z, `DriftEvent`).

**Discipline:** an unweighted edge is *not* silently treated as weight `1.0`. Analyses run on
a weighted subgraph and must **report** how many edges were unweighted (`weight_basis`
coverage), the same way PROPAGATE reports truncation — an allocation scored over mostly-
`estimated` weights is a weaker claim than one over `measured` weights, and the caller must
see that.

**First increment:** add the weight facet to a schema domain (likely `functional.yaml` /
`structure.yaml`), run `tools/validate_schema.py`, and have the INGEST `dependencies` pass
emit `weight` + `weight_basis: estimated`. No analysis yet — just start capturing the signal.

---

## The analysis crates and how reflow2 uses each

All four are pure, dependency-free math (no storage/service deps), matrix/graph in →
result out — a natural fit for the deterministic core (the hard math is LLM-free; only
*naming* clusters and *adjudicating* tradeoffs need the LLM).

### `dynograph-graph` — topology (the allocation engine)

Already a dependency (used by HEAL's structural detectors). The allocation objective → method
map:

| Objective | Method | Notes |
|---|---|---|
| Cohesion/coupling ("group what talks most") | `louvain` community detection | Each community = a candidate Component; naturally hierarchical → maps to `Component.level` (axis Y) |
| Minimize cross-service chatter | `max_flow_min_cut` | Boundaries along the lightest cut (weight = capacity = strength) |
| Co-locate for latency | `shortest_path` / `closeness_centrality` | Feed **cost** (inverted strength), not strength |
| Find "god nodes" | `cut_structure` (articulation points) + `betweenness_centrality` | A node everything routes through. HEAL already detects selective articulation-point SPOFs — reuse |
| Change-impact ranking | `pagerank` / `eigenvector_centrality` / `betweenness` | This is deferred **IP-9** in PROPAGATE — weight a change's blast radius by how central the hit node is |
| Keep the dependency DAG acyclic | `topological_sort` / `find_cycle` | `DEPENDS_ON` between capabilities must stay acyclic |
| Suggest missing links | `link_prediction_all` / `link_prediction_from` | Propose edges the topology implies but the graph lacks — feeds HEAL's `missing_link` / creative linking |

> **Leiden > Louvain (planned foundation work).** `dynograph-graph` currently ships
> **`louvain`**; **Leiden** (Traag et al. 2019) is the better choice for the allocation
> *proposer* — it fixes Louvain's known flaw of returning **badly-connected or internally
> disconnected communities** (a "service" whose functions don't even connect), gives higher-
> quality partitions, and is what GraphRAG/graspologic use. Adding Leiden to
> `dynograph-graph` is a **separate dynograph-foundation effort** (that repo, not here). Until
> it lands, the interim guard is cheap and already available: run `connected_components` on
> each Louvain community and split any that come back disconnected — never emit a disconnected
> community as a candidate Component.

### `dynograph-vector` — the numeric toolbox (weights, depth, resolution)

More than embeddings — the math behind several reflow2 features, embeddings-optional:

- **Distances** (`cosine_similarity`, `dot_product`, `euclidean_distance`,
  `manhattan_distance`, and `_f64` variants) — the vector leg of `fuzzy_then_vector`
  resolution (cosine over embeddings), *and* general distance math for building the distance
  matrix `dynograph-cluster` consumes.
- **Vector algebra** (`add`, `subtract`, `scale`, `hadamard`, `l2_normalize`, `negate`) and
  `centroid` — a cluster/community's centroid is its representative vector; `centroid` also
  rolls `DimensionObservation`s up into a `DimensionAssessment` (axis-4 depth).
- **Statistics** (`mean`, `variance`, `std_dev`, `median`, `percentile`, `softmax`,
  `pearson_correlation`, `spearman_rank_correlation`, `linear_regression_slope`) — the depth
  axis: `linear_regression_slope` over per-epoch `DimensionObservation`s gives **quality
  drift** ("maintainability has been sliding since v1.1"); `pearson`/`spearman` find
  correlated dimensions; `percentile` ranks nodes by a metric. None of this needs an
  embedding sidecar — it's pure math over numbers we already have.

### `dynograph-cluster` — density clustering (DBSCAN)

DBSCAN over a **precomputed N×N distance matrix** (the caller computes distances — e.g. via
`dynograph-vector` — this crate is matrix in, labels out). It finds **arbitrarily-shaped
clusters and flags noise**. Complements `louvain`:

- Louvain partitions by *graph edges* (topology); DBSCAN partitions by a *distance matrix*
  (which can be graph distance, coupling-derived distance, or vector distance).
- **Noise flagging** = outlier/orphan detection — entities that belong to no cluster (a
  different signal than HEAL's edge-based `orphan_node`).
- Use it when the similarity you want to cluster on isn't a graph edge (e.g. dimensional
  similarity, or embedding similarity) — allocation by "what clusters well" in feature space,
  not just topology.

### `dynograph-game` — strategic-tension analysis (the experimental one)

Closed-form normal-form game theory: dominant strategies, pure-strategy **Nash equilibria**,
**Pareto-optimal** outcomes, the headline `nash_is_pareto_suboptimal` (the prisoner's-dilemma
"rational play → collectively worse" signal), and the 2×2 mixed Nash. Framed for design (most
speculative — demand-pull it):

- **Design-tradeoff / tension analysis** — model two competing choices, teams, or constraints
  in tension (a `CONTRADICTS` pair) as a payoff matrix; `nash_is_pareto_suboptimal` flags a
  *design prisoner's dilemma* — where each party optimizing locally makes the whole worse
  (two teams each hardening their own component while the seam between them rots). That is a
  precise, computable signal for a class of architecture smell.
- **Allocation as a game** — each function "chooses" a service to minimize its own cost; a
  pure Nash is a *stable* allocation (no function wants to move). A useful stability check on
  a proposed allocation.
- Pairs with HEAL's `contradiction` and the `Decision` node: the game analysis can *justify*
  a proposed reconciling Decision ("this is Pareto-dominated; here's the outcome that
  dominates it").

---

## The three modes (how analysis plugs into the loop)

1. **Evaluate** an existing allocation/design — score coupling/cohesion, flag god-nodes and
   fragile cuts, report drift. This is **DIAGNOSE/DETECT applied to architecture quality**
   (findings surfaced as gaps), and it is the safest first thing to build.
2. **Propose** — run `louvain`/`max_flow_min_cut` → candidate allocations, ranked and
   *explained*. This is a **SYNTHESIZE** specialization (graph → a design decision), emitted
   as proposals (like HEAL) for a human to accept.
3. **Re-evaluate on change** — when a capability/dependency changes, PROPAGATE the change and
   re-check whether the allocation still holds. **The coherence loop applied to the
   architecture itself.**

---

## Concepts to mine from graphify

[graphify](https://github.com/sligara7) (a codebase-→-knowledge-graph tool) computes several
graph analyses that transfer directly. Some reflow2 already has (and does better, because it
classifies *why* an edge matters); a few are genuinely new and worth adding. Recorded here as
candidates (nothing built from this list yet).

| graphify concept | reflow2 status | reflow2 form / algorithm |
|---|---|---|
| **Blast radius** (`affected` — reverse-relation walk with depth) | ✅ have (better) | PROPAGATE — direction-classified, risk-flagged, no-silent-truncation |
| **God nodes** (top-degree hubs) | ✅ have (better) | HEAL *selective* SPOF (articulation points that split ≥2 subsystems) + `betweenness` |
| **Communities** (Leiden, colored) | 🟡 partial | `louvain` (Leiden planned) → allocation clusters |
| **Explained edges** (`EXTRACTED` / `INFERRED` / `AMBIGUOUS`) | ✅ have | inference-edge `basis`/`confidence` + Fragment `provenance` (`YIELDED`) |
| **Extraction diagnostics** (missing / dangling / duplicate edges) | ✅ mostly | INGEST `dropped_edges` (phantom/dangling) + fuzzy dedup; could add an exact-duplicate-edge count |
| **Surprising connections** — an edge bridging two otherwise-distant communities (high *edge* betweenness / cross-community) | ⬜ **new** | **Unexpected-coupling detector**: a `DEPENDS_ON`/coupling edge whose endpoints sit in different `louvain` communities is either a hidden coupling to flag (DETECT) *or* a creative-link opportunity (chain_reflow). Powered by cross-community detection + edge betweenness |
| **Peripheral→hub** — a low-degree node unexpectedly reaching a high-degree hub | ⬜ **new** | A leaf capability wired straight to a god-component, skipping intermediate structure — ties to the matryoshka **`missing_intermediate_level`** gap (chain_reflow) and god-node dependence. Degree/level anomaly |
| **Graph report** (highlights: key concepts, surprising connections, suggested questions) | ⬜ **new** | A **SYNTHESIZE** rollup artifact: communities/allocation + god-nodes + surprising couplings + DETECT gaps + suggested questions — the "what should I look at?" summary |

The pattern worth borrowing wholesale is graphify's **"every edge is explained"** ethos and
its **surprising-connection** analysis — the design-world analogue ("these two subsystems look
independent but there's a hard coupling between them, or there *should* be a link and isn't")
is high-value for both DETECT (flag it) and HEAL's creative-bridge healer (propose it).

---

## Non-negotiable disciplines

1. **Weights fail loud.** No silent default-`1.0`; report `weight_basis` coverage so a claim
   over `estimated` weights isn't mistaken for one over `measured`.
2. **Strength vs. cost is explicit.** Never hand a strength to a shortest-path/closeness/
   betweenness algorithm; convert to a cost at the boundary. A silent mix-up is a quiet-wrong
   integrity breach.
3. **Candidates, not answers.** Allocation is multi-objective (min-cut vs. reliability vs.
   balanced load *conflict*). Output a small set of **scored, explained** options; a human/SME
   picks the tradeoff. Never claim "the optimal allocation."
4. **The graph informs; it doesn't dictate.** Domain/Conway boundaries sometimes *should*
   win. The graph's real value is revealing *when a domain-based allocation fights the actual
   coupling* — surface that tension, don't override the human.
5. **Deterministic core, pluggable judgment.** The math is LLM-free; only naming clusters and
   adjudicating tradeoffs go through `LlmBackend`. Same split as the rest of reflow2.
6. **Record what's missing** (weight coverage, objectives not scored, algorithms not run) —
   no silent caps.

---

## Build order (increments, smallest-signal first)

1. **Weights facet** ✅ **done** — the `weight`/`frequency`/`data_volume`/`weight_basis`
   facet is on the interaction edges (`functional.yaml` DEPENDS_ON/PART_OF_FLOW,
   `structure.yaml` ALLOCATED_TO/PROVIDES/CONSUMES), and INGEST's `dependencies` pass emits
   weighted `DEPENDS_ON` (`weight_basis: estimated`). Signal captured; no analysis yet.
2. **Allocation evaluator** ✅ **done** — the `allocate` module scores the current
   `ALLOCATED_TO` allocation over the weighted `DEPENDS_ON` graph: per-component
   cohesion/coupling + `modularity`, **misplaced** capabilities (coupled more across a
   boundary than within), and selective **god-components** (`cut_structure` articulation
   points that split ≥2 subsystems). Reports unweighted-edge coverage and multi-allocation.
   `evaluate_allocation()` → `AllocationReport`. No proposer yet.
3. **Centrality-weighted PROPAGATE (IP-9)** ✅ **done** — each `ImpactedNode` carries its
   design-network betweenness `centrality`; PROPAGATE ranks distance → risk → centrality →
   id, so a change landing on a routing hub out-ranks a leaf at the same distance.
4. **Allocation proposer** — community detection (`louvain` now, **Leiden** once the
   foundation ships it; guard Louvain with a `connected_components` split) and/or
   `max_flow_min_cut` → candidate allocations, scored + explained, emitted as proposals; LLM
   names the clusters.
5. **Depth/drift analytics** — `dynograph-vector` stats over per-epoch `DimensionObservation`s
   (`linear_regression_slope` drift, `centroid` rollup).
6. **DBSCAN / game-theory** — demand-pulled when a concrete need appears (feature-space
   clustering; design-tension analysis), not before.
