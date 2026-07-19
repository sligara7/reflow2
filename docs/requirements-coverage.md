# Requirements Coverage — traceability from docs → code → tests

> Part of the **Reflow 2.0** design docs — see **[overview.md](overview.md)** for the full map and reading order.

This is the **golden thread applied to reflow2 itself**: every discrete requirement and
non-negotiable discipline stated in the process docs, traced to the module that implements
it and the test that proves it, with an honest status. It answers *"are we meeting the
docs?"* as an auditable table rather than a judgment call, and — per the project's own
**no-silent-drops** discipline — every unmet requirement is named, not omitted.

**This is a living status document.** Update it in the same change that moves a requirement's
status. It reflects the deterministic, LLM-free core built so far (build-order steps 1–2 of
[interaction-surfaces.md](interaction-surfaces.md)).

## How coverage is confirmed

1. **This matrix** — the requirement→code→test mapping below. Requirement IDs (`[IP-1]`, …)
   are extracted from the process docs.
2. **Automated gates** — `cargo test --no-default-features` (163 tests), `cargo clippy`,
   `cargo fmt --check`, and `python3 tools/validate_schema.py` (schema conforms to
   dynograph-core). These are the executable evidence the matrix cells point at.
3. **The deferral list** — everything marked ⬜/🟡 is a named, tracked gap; nothing that the
   docs require is silently treated as done.

**Deferral discipline (binding).** When work is deferred, it is recorded here as ⬜/🟡 **in
the same change that defers it** — and, where it lives in code (an unused field, a stubbed
branch), annotated at the site with a pointer back here. A deferral that isn't written down
is a silent stub, which this project treats as the same integrity breach as a silent drop.
"Partial/Deferred and recorded" is acceptable; "looks done but quietly isn't" is not.

**Legend:** ✅ Met · 🟡 Partial (core met, a stated facet deferred) · ⬜ Deferred (not yet
built) · ➖ N/A here (a deferred *decision* or a facet gated on the surface/LLM choice).

Deferred items cluster on three fronts, all expected at this stage: **the `LlmBackend` and
its ops** (extraction/INGEST, SME, question phrasing, generative heal content — build-order
step 3), **the interaction surface** (a deliberately deferred decision), and a few
**schema-present-but-no-code-yet** areas (dimensions/depth, `Component.level` matryoshka).

---

## Impact Propagation — [impact-propagation.md](impact-propagation.md)

Implemented in `crates/reflow2-core/src/propagate.rs`; tests in `tests/propagate.rs`.

| ID | Requirement | Status | Evidence / note |
|----|-------------|--------|-----------------|
| IP-1 | Reactive + speculative triggers, one engine | ✅ | `propagate_change` / `propagate_from` · `reactive_propagation_uses_change_event_targets_as_seeds` |
| IP-2 | Traverse the golden-thread structural edges | ✅ | `structural_rule` (SATISFIES, CONSTRAINS, ALLOCATED_TO, PROVIDES/CONSUMES, REALIZES, VERIFIES, DEPLOYED_TO, REQUIRES_RESOURCE, GOVERNED_BY, DEPENDS_ON, PART_OF_FLOW) |
| IP-3 | Also follow inference "why" edges w/ confidence | 🟡 | inference edges traversed as `Causal` via `schema.inference_edge_types()`; **confidence-weighting deferred** |
| IP-4 | Classify each hop into 4 directions | ✅ | `ImpactDirection` · `inference_edges_propagate_as_causal_and_flag_risk` |
| IP-5 | Edge-type-aware direction; blast radius *explained* | ✅ | `structural_rule` + `Hop.via` · `every_impact_is_explained_by_its_edge_chain` |
| IP-6 | Tag each node with an impact **kind** | ⬜ | deferred (pairs with DETECT; noted in module docs) |
| IP-7 | Rank by distance; confidence decays with depth | 🟡 | distance ranking ✅; **depth-decay deferred** |
| IP-8 | Amplify paths crossing risk edges | ✅ | `RISK_EDGES` + `crosses_risk_edge` sort · `inference_edges_..._flag_risk` |
| IP-9 | Rank up by centrality (SPOF) | ✅ | each `ImpactedNode` carries its design-network betweenness `centrality`; ranking is distance → risk → centrality → id · `centrality_ranks_a_hub_impact_above_a_leaf_at_the_same_distance` |
| IP-10 | Rank by criticality (priority/severity) | ⬜ | not inherited into propagate ranking |
| IP-11 | Runs in current epoch, flagged vs the ChangeEvent | 🟡 | seeded from a `ChangeEvent`; **per-epoch temporal filtering deferred** |
| IP-12 | Report cause → change → blast radius | 🟡 | change→radius ✅; **cause (`CAUSES`→ChangeEvent) not surfaced** |
| IP-13 | Snapshot prior state for before/after diff | 🟡 | `temporal::snapshot_node` exists; **speculative diff not wired** |
| IP-14 | Bound depth, never silently truncate | ✅ | `max_depth` + `truncated_beyond_depth` · `depth_bound_reports_truncation_never_hides_it` |
| IP-15 | Explain every impact (via chain + kind) | 🟡 | via chain ✅; **kind deferred (IP-6)** |
| IP-16 | Scope per project | 🟡 | scoped to one `graph_id` (one design = one graph); **indexed `project_id` prefilter deferred** |
| IP-17 | Deterministic + cacheable | 🟡 | deterministic ✅ (stable ordering); **caching deferred** |
| IP-18 | Feed the loop, don't fix | ✅ | propagate only computes/tags; DETECT/HEAL are separate |
| IP-19 | change_type → removal=orphaning, add=coverage-gap | 🟡 | `ChangeAction {Added,Modified,Removed}` exists; **kind specialization deferred (IP-6)** |

---

## Gap Surfacing / DETECT — [gap-surfacing.md](gap-surfacing.md)

DIAGNOSE half in `crates/reflow2-core/src/detect.rs`; tests in `tests/detect.rs`. The PROMPT
half (question phrasing, anchors) is LLM-gated and deferred.

| ID | Requirement | Status | Evidence / note |
|----|-------------|--------|-----------------|
| GS-1 | DIAGNOSE→PROMPT→INGEST loop | 🟡 | DIAGNOSE (`detect_gaps`) ✅, PROMPT (`to_prompt`) ✅, INGEST spine ✅ (see Extraction section); **re-ingest of an answer as time-aware update deferred (EX-R1/EX-Z3)** |
| GS-2 | Asks the human; distinct from HEAL | ✅ | `detect_gaps` vs `heal`; `full_coherence_loop` leaves the unmet requirement for the human |
| GS-3 | `GapCandidate` shape | 🟡 | id/gap_source/scope/severity/title/description/affected_ids/suggested_depth/evidence ✅; **`anchor` deferred** |
| GS-4 | `GapPrompt` shape | 🟡 | `GapPrompt` + `GapCandidate::to_prompt` via `LlmBackend` · `gap_becomes_a_plain_question_via_the_backend`; **`relevant_context` graph-slice deferred** |
| GS-5 | Phase-coverage gaps | 🟡 | concept_without_design, design_without_build, build_without_verification, no_deploy_operate ✅; **no_decisions_recorded deferred** |
| GS-6 | Traceability gaps | 🟡 | unsatisfied_requirement, **unmotivated_capability** (BL-27 — the reverse direction; severity reads `Capability.provenance` so a capability inferred from running code that nothing asked for leads the list, while an authored one sits below the requirement gaps · `tests/detect.rs`, `smoke_mcp.py`), **possible_duplicate** (BL-27 — computed from shared allocation sets, ≥2 shared and Jaccard ≥0.8; asked not repaired, because HEAL's merge deletes a node and is safe only on human-asserted endpoints. Pairs already carrying a `DUPLICATES` edge are left to HEAL, so the two compose rather than double-count · `tests/detect.rs`, `smoke_mcp.py`), unallocated_capability, unrealized_capability (BL-38 — accepts both P3 shapes: direct `REALIZES` on the capability, or an artifact realizing a component it is allocated to · `an_artifact_realizing_the_component_counts...`), unverified_capability ✅ (capabilities only; the artifact rule was retired in BL-23 after the self-host probe made it 22 of 25 gaps — per-file coverage is now a `graph_report` statistic. The capability key is frozen because gap ids hash it and acknowledgements hang off the id); **interfaceless_dependency deferred** |
| GS-6b | Interface pairing gaps | ✅ | `unprovided_interface` / `unconsumed_interface` from `detect_interface_pairing` — keyed on Interface node identity, so a shared name cannot mask a break · `tests/interface.rs` |
| GS-7 | Structural gaps (asked) | 🟡 | `unexpected_coupling` **retired as a gap** (BL-6b), reported as a signal by `graph_report` instead — it was never in the structural-gap list of [gap-surfacing.md](gap-surfacing.md) (`orphan_node`/`dead_end`/`disconnected_cluster`/`single_point_of_failure`), having been volunteered by the graph-analysis work, and both blind trials found it firing on correct architecture. Two rounds of tightening did not fix it. `surprising_connections` itself remains **selective** (since the 2026-07-18 blind trial — `PROVIDES`/`CONSUMES` are declared structure so contracts are collapsed to the components they couple rather than flagged, both communities must be ≥3 nodes to count as *parts* of the design, and provenance nodes (`Fragment`, `DriftEvent`) are excluded from the topology) · `a_cross_community_coupling_is_a_signal_not_a_gap`, `tests/surprises.rs`, `tests/report.rs`; `orphan_node`/`dead_end`/`disconnected_community`/`single_point_of_failure` still computed in HEAL only |
| GS-8 | Quality/risk gaps | 🟡 | `declining_dimension` surfaced as a gap (from `dimension_drifts`) · `a_declining_dimension_is_surfaced_as_a_gap...`; `contradiction` detected in HEAL; unmitigated_risk / unvalidated_causal_claim / dimension_blind_spot / violated_constraint still deferred |
| GS-9 | Compliance gaps | ⬜ | needs the environment layer (EnvironmentRule / OPERATES_IN) |
| GS-10 | SME gaps | ⬜ | LLM (SME augmentation) |
| GS-11 | Decomposition/hierarchy gaps | ✅ | `hierarchy_issues` → `missing_intermediate_level` / `level_mismatch` / `orphan_level`, surfaced as gaps by `detect_gaps` · `tests/hierarchy.rs` |
| GS-12 | Adding a detector = one enum + one method | ✅ | `GapSource` + `detect_*` methods |
| GS-13 | Detectors read computed signals; prove they fire | ✅ | detectors gated on type-population counts · `early_graph_..._not_per_node_floods`, `traceability_fires_per_node_once_the_phase_exists` |
| GS-14 | Rank by composite severity | ✅ | severity sort · `unsatisfied_requirement_ranks_by_priority` |
| GS-15 | Anchor in the user's own material | ⬜ | needs text/vector search (anchor) |
| GS-16 | Graceful degrade + `rephrase_degraded` | ✅ | `GapCandidate::to_prompt` degrades to raw wording + flag · `prompt_degrades_gracefully_when_the_backend_fails` |
| GS-17 | Never speak graph-jargon to the user | 🟡 | titles/descriptions are plain; `evidence` deliberately carries jargon; **polished question is the deferred PROMPT step** |
| GS-18 | Deterministic gap ids + caching | 🟡 | deterministic FNV ids ✅ (`gap_ids_are_deterministic_across_runs`); **caching deferred** |
| GS-19 | Validate ids at the boundary | ➖ | no cache-key/glob path from external ids yet; relevant when the surface/caching lands |
| GS-20 | Two modes: retroactive + proactive | ✅ | phase-coverage (proactive) + per-node traceability (retroactive) in one pass |
| GS-21 | Adjustable depth ("heat" 1–5) | 🟡 | `suggested_depth` emitted per candidate; **not yet an input knob** |

---

## HEAL — [heal-process.md](heal-process.md)

`crates/reflow2-core/src/heal.rs` (+ `structure.rs` for graph-topology defects); tests in
`tests/heal.rs` and `tests/structural.rs`.

| ID | Requirement | Status | Evidence / note |
|----|-------------|--------|-----------------|
| HEAL-1 | Detect → propose (never mutate) → atomic apply | ✅ | `propose_heal` / `apply_heal` · `proposal_computes_without_mutating` |
| HEAL-2 | Issue fields (id/category/severity/message/fix_type) | ✅ | `HealIssue` |
| HEAL-3 | Defect catalog (13 categories) | 🟡 | orphan_node, contradiction, duplicate, unresolved_setup, disconnected_community, single_point_of_failure, dead_end, circular_dependency ✅ (8); **unreachable, weak_connection, missing_link, missing_entity, missing_embedding deferred**. `circular_dependency` walks a *directed* dependency view (DEPENDS_ON + contracts collapsed through their Interface), deliberately excluding the other traceability edges so the golden thread closing on itself isn't reported as a loop · `tests/cycles.rs` |
| HEAL-4 | Strategies + max_operations + priority_categories | 🟡 | `HealStrategy` + `max_operations` ✅; **priority_categories deferred** |
| HEAL-5 | `HealProposal` shape | 🟡 | target_id/strategy/issues_addressed/operations/generated_content/skipped_operations/confidence/requires_human_review/summary ✅; **validation_report_id + separate skipped_bridges deferred** |
| HEAL-6 | Propose, then apply | ✅ | separate `apply_heal`, and the split is now **enforced**: every operation must match one HEAL derives from the graph as it stands, refused before any write, so a forged or stale proposal cannot mutate (BL-29 — a hand-written proposal naming two non-duplicates was verified deleting a node before the fix) · `a_merge_no_detector_asked_for_is_refused`, `a_proposal_goes_stale_when_the_defect_is_resolved_by_hand`, `smoke_mcp.py` §9b |
| HEAL-6b | A merge reports what it could not carry | ✅ | `HealReport.discarded` — the removed node's properties, edges to unknown endpoints, and upsert collisions on a shared edge triple; each with a reason rather than dropped in silence (BL-29, rule 4) · `merge_reports_the_properties_it_could_not_keep`, `merge_reports_an_edge_whose_properties_are_overwritten` |
| HEAL-7 | No silent drops → skipped_operations w/ ref+reason | ✅ | `max_operations_cap_surfaces_overflow_never_drops_it` + unresolvable-endpoint skip |
| HEAL-8 | Human-review gate on generated content | ✅ | `requires_human_review` · `generative_fixes_require_human_review_and_are_not_applied` |
| HEAL-9 | Post-repair verification | ✅ | `apply_heal` re-detects + `verified` · `apply_merge_repoints_edges_and_verifies` |
| HEAL-10 | Provenance: healed via Fragment | ⬜ | merge is structural; Fragment-provenance for generated content deferred |
| HEAL-11 | Mode-aware (rigid = propose-only) | ✅ | `project_mode` · `rigid_mode_proposes_but_never_auto_applies` |
| HEAL-12 | Generative healers (bridge/entity/contradiction/verification) | ⬜ | detected as review-gated stubs; **content generation is LLM-deferred** |
| HEAL-13 | Guarded creative-bridge healer | ⬜ | LLM + speculative provenance |
| HEAL-14 | Missing-intermediate Component for hierarchy | ⬜ | matryoshka (needs `Component.level`) |

The one **content-free** structural repair — `duplicate` → **merge** — is fully applied and
verified (`apply_merge_repoints_edges_and_verifies`, `merge_carries_a_unique_edge_onto_the_survivor`).

---

## Extraction / INGEST — [extraction-plan.md](extraction-plan.md)

`crates/reflow2-core/src/ingest.rs`; tests in `tests/ingest.rs`. This increment builds the
EXTRACT→INTEGRATE spine (a representative subset of passes) via the `LlmBackend` seam. The
graph-informed *resolution* stage and the remaining passes are deferred — spelled out below
so none is a silent stub.

| ID | Requirement | Status | Evidence / note |
|----|-------------|--------|-----------------|
| EX-1 | Three-phase fan-out (P1 always · P2 gated · P3 edges) | 🟡 | orchestration in `ingest`; a **subset** of passes implemented (below) |
| EX-2 | Discovery gate (orthogonal booleans, anchor-required) | 🟡 | `Discovery` classifier gates phase-2; `components` and `interfaces` gates consumed — the rest parsed but their passes deferred (`#[allow(dead_code)]` marks it) |
| EX-P1 | Phase-1 passes: project_intent, requirements, constraints, capabilities | ✅ | `ingest` · `full_ingest_builds_a_golden_thread_from_text` |
| EX-P2 | Phase-2 passes: components (+ALLOCATED_TO), interfaces (+PROVIDES/CONSUMES) | 🟡 | components ✅; interfaces ✅ — both sides of a contract in one pass, gated on discovery **and** on components existing; an ungrounded side is left unpaired for DETECT rather than guessed · `tests/ingest.rs` (7 interface tests); **flows, actors, decisions, artifacts, resources deferred** |
| EX-P3 | Phase-3 passes: satisfies (+SATISFIES), dependencies (+weighted DEPENDS_ON) | 🟡 | satisfies ✅; **dependencies ✅** (emits weighted DEPENDS_ON — `dependencies_pass_captures_weighted_coupling_edges`); **verifications, inference, dimensions, changes deferred** |
| EX-SME | SME augmentation post-pass | ⬜ | LLM; see sme-augmentation.md |
| EX-D1 | One shared LLM-call helper | ✅ | `run_pass` |
| EX-D2 | Never-raises + error envelopes; siblings survive | ✅ | `PassError` · `a_failed_pass_is_enveloped_and_siblings_survive` |
| EX-D3 | Per-pass timeout budget | ⬜ | no timeout (sync mock); lands with a real async backend |
| EX-D4 | No silent fallbacks; retry-once on recoverable-empty then loud | 🟡 | loud `PassError` ✅; **retry-once deferred** |
| EX-D5 | Keep the gate off reasoning models | ➖ | backend-choice; N/A until a real backend |
| EX-D6 | Focused prompts; lists are arrays | ✅ | strict per-pass JSON shapes |
| EX-D7 | Prefix caching — unchanging input first | ✅ | `pass_prompt` puts INPUT first |
| EX-D8 | Enum tuples from schema; fail loud on drift | 🟡 | LLM enum values validated with loud-skip→`warnings`; **value sets are local consts, not read from `schema/*.yaml`** (drift risk noted) |
| EX-D9 | Symmetric-edge auto-inverse | ⬜ | not needed by current edges; deferred |
| EX-D10 | Per-fragment metrics | ⬜ | deferred |
| EX-D11 | Selective context threading | 🟡 | rosters threaded only into edge passes ✅; epoch threading partial |
| EX-R1 | Resolution: matched-unchanged / matched-evolved / genuinely-new | ✅ | id-based resolution in `integrate_node` · `reingest_with_changed_content_evolves_and_snapshots`, `reingest_identical_content_is_a_noop_no_snapshot` |
| EX-R2 | `fuzzy_then_vector` cross-id dedup + embedding generation | 🟡 | **fuzzy leg done** — `fuzzy_match` uses `token_sort_ratio` (no network) to merge a new id onto an existing same-type node by name at ≥90; merges are recorded in `IngestReport.fuzzy_merges` and edges redirect via aliases · `a_new_id_with_a_matching_name_is_fuzzy_merged_and_edges_redirect`, `a_new_id_with_a_dissimilar_name_is_not_merged`. **Vector tiebreaker for the ambiguous band still deferred** (optional embedding seam — see note below) |
| EX-I1 | One typed integration payload | ✅ | single `ingest` path |
| EX-I2 | MERGE + provenance on a Fragment | ✅ | Fragment + `YIELDED` + provenance stamp · `full_ingest_...` |
| EX-I3 | Unknown/phantom edges dropped + surfaced | ✅ | `dropped_edges` · `phantom_edge_is_dropped_not_written` |
| EX-Z1 | Ingest in an active `DesignEpoch` context | 🟡 | `IngestOptions.epoch_id` → `OCCURS_DURING`; not required |
| EX-Z2 | The `changes` pass (ChangeEvent extraction) | ⬜ | deferred |
| EX-Z3 | Time-aware integration (matched-evolved → snapshot, never silent overwrite) | ✅ | `integrate_node` snapshots the prior state + records a `ChangeEvent` before applying an evolved node; lazily opens an epoch if none given · `reingest_with_changed_content_evolves_and_snapshots`. (Cross-id evolution still needs EX-R2.) |

### Note: are embeddings (an embeddings-rs sidecar) actually needed?

Deliberately kept **optional**. The core needs no embeddings today:

- **Topology / clustering** is pure graph theory — `dynograph-graph` ships `leiden`
  community detection (v0.10.0; connected communities) plus the components/cuts used by
  HEAL and the allocation analysis. No vectors.
- **Time-aware resolution** (matched-evolved) is id-based (above). No vectors.
- **Near-identical cross-id dedup** can use `dynograph-resolution`'s fuzzy string matching
  (`token_sort_ratio`/`jaro_winkler`) — still no network.

Embeddings earn their keep only for two things: **semantic** dedup (matching entities that
are the same idea but worded differently, beyond fuzzy strings) and **semantic retrieval /
"local search"** (finding entry-point nodes from a free-text query — e.g. gap-surfacing
anchors, the resolution vector-probe). Both are tied to the deferred interaction-surface
decision: an **agent-native** surface can reuse the agent's own embedding access; a **hosted**
surface would run a self-contained generator like
[embeddings-rs](https://github.com/sligara7/embeddings-rs) (768-dim nomic, `POST /embed`).

**Plan:** keep embeddings behind an optional, pluggable `EmbeddingBackend` seam (mirroring
`LlmBackend`) — the core runs fully without it; plug in embeddings-rs (or agent-native
embeddings) to upgrade EX-R2 to `fuzzy_then_vector` and enable semantic retrieval. Not wired
yet — decide it with the surface.

## Three Axes — [three-axes.md](three-axes.md)

Axis Z in `temporal.rs` (tests `tests/temporal.rs`); axis-X topology in `structure.rs`;
schema in `schema/*.yaml`.

| ID | Requirement | Status | Evidence / note |
|----|-------------|--------|-----------------|
| 3AX-1 | Model X / Y / Z + depth | ✅ | X (edges, propagate/structure/allocate), Y (`hierarchy` — matryoshka over `Component.level`), Z (temporal), depth (`dimensions` — drift/rollup) all have code |
| 3AX-2 | Add Axis Z | ✅ | `schema/temporal.yaml` + `temporal.rs` |
| 3AX-3 | Epochs/facts/snapshots — never overwrite the past | ✅ | `record_change_preserves_pre_change_state` |
| 3AX-4 | `DesignEpoch` (generalizes Anchor) | ✅ | `add_epoch` / `EpochType` |
| 3AX-5 | `TemporalFact` valid_from/valid_to | 🟡 | schema ✅ + edge constants; **typed helpers/usage deferred** |
| 3AX-6 | `Snapshot` cross-revision diff | ✅ | `snapshot_node` + `parse_snapshot_state` |
| 3AX-7 | `ChangeEvent` + reason taxonomy + CHANGED + CAUSES | 🟡 | `ChangeType` taxonomy + `CHANGED` ✅; **`CAUSES`→ChangeEvent wiring deferred** |
| 3AX-8 | `DimensionObservation` rollup + per-epoch drift | ✅ | `dimensions` module: `add_dimension_observation`, `dimension_drift`/`dimension_drifts` (`linear_regression_slope`), `rollup_assessment` (`mean`) · `tests/dimensions.rs` |
| 3AX-9 | Formalize Axis Y `Component.level` | ✅ | schema + `hierarchy` module (`Level` rank arithmetic); `CONTAINS` broadened to `Component` so the component spine is expressible |
| 3AX-10 | Missing-intermediate-level detector | ✅ | `hierarchy_issues` `MissingIntermediateLevel` (CONTAINS/DEPENDS_ON skipping ≥2 levels) · `a_system_containing_a_part_directly_is_a_missing_intermediate` |
| 3AX-11 | Tight X edge set + wildcard inference + evidence/confidence | ✅ | schema design; respected by code |

---

## Interaction Surfaces & build order — [interaction-surfaces.md](interaction-surfaces.md)

| ID | Requirement | Status | Evidence / note |
|----|-------------|--------|-----------------|
| IS-1 | Core is surface-agnostic | ✅ | `reflow2-core` is a library; no surface/LLM in it |
| IS-2 | Core = store + schema + loop ops | ✅ | schema + storage + propagate/detect/heal (deterministic loop ops) |
| IS-3 | Split deterministic vs LLM ops | ✅ | everything built is deterministic; LLM ops explicitly deferred |
| IS-4 | `LlmBackend` trait for LLM ops | 🟡 | `llm::LlmBackend` (object-safe, sync) + `MockLlmBackend` + `complete_json`; first op (`to_prompt`) wired · `tests/llm.rs`. Agent-native backend now real (`agent::AgentBackend`, SP-2). **Real *external* provider backends still deferred (not needed for agent-native, IS-6)** |
| IS-5 | Candidate surfaces preserved | ➖ | deferred *decision* (documented, not code) |
| IS-6 | Agent-native vs hosted consequence | ➖ | deferred *decision* |
| IS-7 | Build order (1 store+schema → 2 det. core → 3 LLM → 4 surface) | 🟡 | **steps 1–3 complete for the agent-native route** (persistence SP-1; deterministic core; LlmBackend + mock + agent-native adapter SP-2); step 4 (the MCP surface, SP-3) not started; external providers deferred |

### Agent-native surface — [surface-plan.md](surface-plan.md)

Next-phase build order for the agent-native surface (the deferred surface decision, now made).

| ID | Requirement | Status | Evidence / note |
|----|-------------|--------|-----------------|
| SP-1 | Persistence: `DesignGraph::open_rocksdb(path)`, feature-gated, fail-loud when off | ✅ | `graph::DesignGraph::open_rocksdb` delegates to `StorageEngine::new_rocksdb` · `tests/persistence.rs` (`open_rocksdb_without_feature_fails_loud` in the default suite; `rocksdb_round_trips_across_reopen` under `--features rocksdb`) |
| SP-2 | `LlmBackend` = ambient-agent adapter (returns prompt+schema, takes back JSON) | ✅ | `agent::{PromptCollector, AgentBackend, AgentPrompt, AgentAnswer, prompt_id}` — collect-then-serve handshake (prepare pass harvests prompts, agent fills, serve pass replays under determinism); fail-loud on desync + `unused_answers` accounting · `tests/agent.rs`. `LlmBackend` unchanged (sync/object-safe). **Content-dependent multi-turn ops need repeated rounds (documented).** |
| SP-3 | Surface layer: MCP server / CLI exposing core ops as tools | ✅ | `reflow2-mcp` crate — rmcp 2.x stdio server (`ReflowService`) exposing ~30 fine-grained tools over a RocksDB graph (`--graph-path`): DETECT/analyze (detect_gaps, propagate_*, graph_report, heal detect/propose, allocate, hierarchy, surprises, dimensions), build (add_*/create_*/CRUD/apply_heal, incl. `add_interface`/`provides`/`consumes` so a contract's two sides are modelled from the surface, not only from `ingest`), CHANGE (epoch/change_event/record_change), and the `gap_to_prompt` collect-then-serve handshake (SP-2). No-envelope, no-silent-fallback results · `tests/tools.rs`. **`ingest` handshake deferred → SP-3b.** |
| SP-4 | Consumer `AGENTS.md` / skill for the softball repo | ✅ | `getting-started/` — drop-in `AGENTS.md` (the loop), `SETUP.md` (macOS + Debian toolchain, build, verify), `mcp.json`, and `skills/{genesis,capture-intent,detect-and-ask,link-artifacts,impact-check,check-health}/SKILL.md` (installed to both `.claude/skills/` and `.grok/skills/` — BL-22) — one per loop step, incl. **check-health** covering the HEAL step (`detect_defects`→`propose_heal`→`apply_heal`), which had MCP tools but no skill to invoke them. Distinct from this repo's dev AGENTS.md. Grok Build reads Claude-style `.mcp.json` (confirmed); OpenCode and VS Code need their own files, which `reflow2_init.py` now writes too (BL-22). **Not yet exercised in a real softball repo.** |
| SP-5 | GENESIS: bootstrap the graph from the opening brief | ✅ | `genesis::DesignGraph::genesis` (thin core op) + `genesis` MCP tool + `getting-started/skills/genesis/` (agent-native brief-expansion) · [genesis.md](genesis.md) · `tests/genesis.rs`, `reflow2-mcp/tests/tools.rs::genesis_bootstraps_then_detect_hands_off`. Guarded/idempotent (no re-init clobber); seeds P0/P1 only → DETECT `concept_without_design` hand-off; deployment captured as Requirements. No schema change. |
| SP-6 | Artifact linking wiring (`REALIZES`, provenance) to real files | ✅ | `artifact::DesignGraph::{add_artifact, realizes, link_artifact}` (link_artifact atomic: Artifact + provenance Fragment + `YIELDED` + `REALIZES`, fail-loud on missing target) + `add_artifact`/`realizes`/`link_artifact` MCP tools + `getting-started/skills/link-artifacts/` · [artifact-linking.md](artifact-linking.md) · `tests/artifact.rs`, `reflow2-mcp/tests/tools.rs::link_artifact_closes_the_unrealized_capability_gap`. Added missing `edge::{SPECIFIES,DOCUMENTS,PRODUCES,ANNOTATES}` constants. **Link-only; drift comparison lives in `drift.rs` (SP-6b), with the `checksum` baseline recorded here.** |

**Future improvements (planned, not yet built).** The six-step surface plan (SP-1…SP-6) is
complete; these deepen it and are tracked here so they aren't lost.

| ID | Requirement | Status | Evidence / note |
|----|-------------|--------|-----------------|
| SP-3b | `ingest` programmatic LLM extraction over the MCP surface | ⬜ | needs a **transactional prepare pass** (`begin_batch`→collect→`discard_batch`) so the mutating collect pass rolls back, plus a unified serve-or-collect backend, then wire `ingest` as a multi-round handshake tool. Pairs with GENESIS. Today the agent extracts intent in-context and writes via GENESIS + `add_*`/`create_*` (EX-P2 artifact extraction also deferred). |
| SP-6b | As-built drift detection / reconcile | 🟡 | `drift::DesignGraph::reconcile_artifacts` — compares caller-supplied observations against the `Artifact.checksum` baseline (new schema property, recorded by `link_artifact`), emitting `missing_artifact` / `checksum_change` / `undocumented_addition` findings, plus `no_baseline` when a comparison is impossible (surfaced, never silently passed). Optionally records `DriftEvent` nodes (deterministic ids, `DEPENDS_ON` their Artifact). `propagation_seeds` carry the realized design nodes, so drift walks **upstream** via `REALIZES` to the Capability/Requirement — the code→design feedback path. `reconcile_artifacts`/`set_artifact_checksum` MCP tools + `link-artifacts` skill · `tests/drift.rs` (12), `reflow2-mcp/tests/tools.rs::reconcile_surfaces_a_code_change_back_to_the_design`. **The core performs no I/O by design** — the caller (agent/CI/CLI) supplies observations, same seam pattern as `LlmBackend`. **Deferred: `spec_mismatch`/`status_mismatch` drift types (need content comparison, not just hashing), and discovery of unregistered files (the caller must name what it observed — reflow2 cannot enumerate a filesystem it never reads).** |

---

## Write-side coverage — can the user record what DETECT asks for?

Surfaced by the [reflow audit](reflow-audit.md): reflow2's read side runs ahead of its write
side. Several node types are counted by `detect.rs`, listed by `report.rs`, and classified by
`propagate.rs`, but have **no typed constructor and no MCP tool** — so a gap demanding one can be
reported but not ergonomically closed. Generic `create_node` works, but the agent must hand-type
property names against a schema it cannot see, which in practice means the gap stays open and the
agent learns to ignore gap output.

**"A schema it cannot see" is now addressed (WS-6, BL-1).** `describe_schema` exposes the
vocabulary — node types with their required properties, edge types with their legal endpoints —
and failed `create_node` / `create_edge` calls name the alternatives rather than only the
rejection. The typed-constructor gaps below are unchanged; what closed is the discovery problem
that made the generic escape hatch unusable.

| ID | The gap DETECT emits | Asking the user to record | Write side | Status |
|----|----------------------|---------------------------|-----------|--------|
| WS-1 | `build_without_verification`, `unverified_capability` | `Verification` (+ `VERIFIES`) | `verify.rs`: `add_verification`, `verifies`, `set_verification_status` + MCP tools | ✅ |
| WS-2 | `no_deploy_operate` | `Release` / `Environment` / `Resource` (+ `DEPLOYED_TO`, `REQUIRES_RESOURCE`) | `operate.rs`: `add_release`, `add_environment`, `add_resource`, `deploy_to`, `require_resource` + MCP tools | ✅ |
| WS-3 | HEAL `contradiction` → "Decision" content stub | `Decision` (+ `GOVERNED_BY`) | `graph.rs`: `add_decision`, `governed_by` + MCP tools | ✅ |
| WS-4 | GS-9 compliance gaps (deferred) | `EnvironmentRule` (+ `OPERATES_IN`, `IMPOSES`, `COMPLIES_WITH`, `VIOLATES_RULE`) | none | ⬜ |
| WS-5 | — (no detector asks for it) | `QualityGate` | none | ⬜ |
| WS-7 | — (the write side of axis Y) | `Component.level` + `Component CONTAINS Component` | `graph.rs`: `add_component(level)`, `contain_component` + MCP tools — `hierarchy_issues` previously had no writer and returned `[]` for want of input | ✅ |
| WS-8 | `unsatisfied_requirement` (suppression side) | `Requirement.status` | `graph.rs`: `set_requirement_status` + MCP tool; HEAL's orphan scan now honours it too | ✅ |
| WS-9 | — (the loop's own memory) | a question already put to the user, and whether it was answered | `detect.rs`: `record_asked_question`, `answer_question`, `withdraw_question`, `open_questions` + MCP tools; written by `gap_to_prompt`'s serve pass so it cannot be forgotten (BL-4) | ✅ |
| WS-11 | — (a design that outlives the session must be loadable into one) | the design itself, from a file or stdin | `reflow2-mcp --import` as the sibling of `--export`, upsert, reporting skipped edges by name; the RocksDB lock translated into what to do about it (BL-39) · `smoke_mcp.py` §10 |
| WS-10 | — (the write side of adoption: a graph that cannot say "this already exists" describes a running system as unbuilt) | `Capability.status` at creation, and `provenance` on `Requirement` / `Capability` / `Component` / `Interface` | `graph.rs`: `add_capability(status)`, `set_capability_status`, `set_provenance` + MCP tools; `import_graph` carries both at create time for bulk adoption (BL-27) | ✅ |
| WS-6 | — (the write side's precondition: you cannot record what you cannot name) | the vocabulary itself — node types, their required properties, and which edge types join which endpoints | `vocabulary.rs`: `describe_vocabulary`, `describe_node_type`, `edge_types_between` + the `describe_schema` MCP tool; enriched `create_node` / `create_edge` failures | ✅ |

Evidence for WS-1..3: `tests/write_side.rs` (12) asserts the round trip that matters — the gap
fires, the user records what it asked for, the gap closes — plus
`reflow2-mcp/tests/tools.rs::the_write_side_can_answer_what_detect_asks_for` over the surface, and
the typed tools are named per-gap in the `detect-and-ask` skill.

**WS-4 and WS-5 stay deferred on purpose.** Nothing reads or asks for either type: GS-9 compliance
detection is itself deferred, and `QualityGate` has no detector at all. Adding constructors for
types nothing consumes would build the mirror image of the problem this section exists to record —
a write side with no read side. Each should land with its detector, not before.

## Cross-cutting project rules — [AGENTS.md](../AGENTS.md)

| Rule | Status | Evidence / note |
|------|--------|-----------------|
| Schema-first; `validate_schema.py` passes | ✅ | schema loads via `Schema::from_multiple_yamls`; `schema::tests::all_domains_merge_and_validate` (26 nodes / 52 edges) |
| No silent fallbacks / no silent drops | ✅ | fail-loud CRUD (`unknown_node_type_fails_loud`, `missing_required_property_fails_loud`), truncation reporting (IP-14), `skipped_operations` (HEAL-7), unknown-seed surfacing |
| Terminology matches the schema | ✅ | `nodes::{node,edge}` constants mirror schema names |
