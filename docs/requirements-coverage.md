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
2. **Automated gates** — `cargo test --no-default-features` (39 tests), `cargo clippy`,
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
| IP-9 | Rank up by centrality (SPOF) | ⬜ | `structure` has SPOF but not wired into ranking |
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
| GS-6 | Traceability gaps | 🟡 | unsatisfied_requirement, unallocated_capability, unrealized_capability, unverified_capability ✅; **interfaceless_dependency deferred** |
| GS-7 | Structural gaps (asked) | 🟡 | signals computed in HEAL (`orphan_node`, `dead_end`, `disconnected_community`, `single_point_of_failure`); **surfacing as questions deferred with PROMPT** |
| GS-8 | Quality/risk gaps | ⬜ | `contradiction` detected in HEAL; the gap-surfacing forms (unmitigated_risk, unvalidated_causal_claim, dimension_blind_spot, violated_constraint) deferred |
| GS-9 | Compliance gaps | ⬜ | needs the environment layer (EnvironmentRule / OPERATES_IN) |
| GS-10 | SME gaps | ⬜ | LLM (SME augmentation) |
| GS-11 | Decomposition/hierarchy gaps | ⬜ | needs `Component.level` analysis (see 3AX-9/10) |
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
| HEAL-3 | Defect catalog (12 categories) | 🟡 | orphan_node, contradiction, duplicate, unresolved_setup, disconnected_community, single_point_of_failure, dead_end ✅ (7); **unreachable, weak_connection, missing_link, missing_entity, missing_embedding deferred** |
| HEAL-4 | Strategies + max_operations + priority_categories | 🟡 | `HealStrategy` + `max_operations` ✅; **priority_categories deferred** |
| HEAL-5 | `HealProposal` shape | 🟡 | target_id/strategy/issues_addressed/operations/generated_content/skipped_operations/confidence/requires_human_review/summary ✅; **validation_report_id + separate skipped_bridges deferred** |
| HEAL-6 | Propose, then apply | ✅ | separate `apply_heal` |
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
| EX-2 | Discovery gate (orthogonal booleans, anchor-required) | 🟡 | `Discovery` classifier gates phase-2; only `components` gate consumed — the rest parsed but their passes deferred (`#[allow(dead_code)]` marks it) |
| EX-P1 | Phase-1 passes: project_intent, requirements, constraints, capabilities | ✅ | `ingest` · `full_ingest_builds_a_golden_thread_from_text` |
| EX-P2 | Phase-2 passes: components (+ALLOCATED_TO) | 🟡 | components ✅; **flows, interfaces, actors, decisions, artifacts, resources deferred** |
| EX-P3 | Phase-3 passes: satisfies (+SATISFIES) | 🟡 | satisfies ✅; **dependencies, verifications, inference, dimensions, changes deferred** |
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
| EX-R1 | Resolution: matched-unchanged / matched-evolved / genuinely-new | ⬜ | **deferred**; currently create-or-replace by id only |
| EX-R2 | `fuzzy_then_vector` dedup + embedding generation | ⬜ | **deferred** — needs an embedding generator (a stub even in storyflow; tied to the surface decision) |
| EX-I1 | One typed integration payload | ✅ | single `ingest` path |
| EX-I2 | MERGE + provenance on a Fragment | ✅ | Fragment + `YIELDED` + provenance stamp · `full_ingest_...` |
| EX-I3 | Unknown/phantom edges dropped + surfaced | ✅ | `dropped_edges` · `phantom_edge_is_dropped_not_written` |
| EX-Z1 | Ingest in an active `DesignEpoch` context | 🟡 | `IngestOptions.epoch_id` → `OCCURS_DURING`; not required |
| EX-Z2 | The `changes` pass (ChangeEvent extraction) | ⬜ | deferred |
| EX-Z3 | Time-aware integration (matched-evolved → snapshot, never silent overwrite) | ⬜ | **deferred** — depends on EX-R1/R2; today re-ingest with new ids duplicates, with same ids overwrites-in-place without a snapshot |

## Three Axes — [three-axes.md](three-axes.md)

Axis Z in `temporal.rs` (tests `tests/temporal.rs`); axis-X topology in `structure.rs`;
schema in `schema/*.yaml`.

| ID | Requirement | Status | Evidence / note |
|----|-------------|--------|-----------------|
| 3AX-1 | Model X / Y / Z + depth | 🟡 | X (edges, propagate/structure) ✅, Z (temporal) ✅, Y in schema (`Component.level`) 🟡, **depth/dimensions schema-only** |
| 3AX-2 | Add Axis Z | ✅ | `schema/temporal.yaml` + `temporal.rs` |
| 3AX-3 | Epochs/facts/snapshots — never overwrite the past | ✅ | `record_change_preserves_pre_change_state` |
| 3AX-4 | `DesignEpoch` (generalizes Anchor) | ✅ | `add_epoch` / `EpochType` |
| 3AX-5 | `TemporalFact` valid_from/valid_to | 🟡 | schema ✅ + edge constants; **typed helpers/usage deferred** |
| 3AX-6 | `Snapshot` cross-revision diff | ✅ | `snapshot_node` + `parse_snapshot_state` |
| 3AX-7 | `ChangeEvent` + reason taxonomy + CHANGED + CAUSES | 🟡 | `ChangeType` taxonomy + `CHANGED` ✅; **`CAUSES`→ChangeEvent wiring deferred** |
| 3AX-8 | `DimensionObservation` rollup | ⬜ | schema only |
| 3AX-9 | Formalize Axis Y `Component.level` | 🟡 | schema ✅; **level-based code deferred** |
| 3AX-10 | Missing-intermediate-level detector | ⬜ | matryoshka |
| 3AX-11 | Tight X edge set + wildcard inference + evidence/confidence | ✅ | schema design; respected by code |

---

## Interaction Surfaces & build order — [interaction-surfaces.md](interaction-surfaces.md)

| ID | Requirement | Status | Evidence / note |
|----|-------------|--------|-----------------|
| IS-1 | Core is surface-agnostic | ✅ | `reflow2-core` is a library; no surface/LLM in it |
| IS-2 | Core = store + schema + loop ops | ✅ | schema + storage + propagate/detect/heal (deterministic loop ops) |
| IS-3 | Split deterministic vs LLM ops | ✅ | everything built is deterministic; LLM ops explicitly deferred |
| IS-4 | `LlmBackend` trait for LLM ops | 🟡 | `llm::LlmBackend` (object-safe, sync) + `MockLlmBackend` + `complete_json`; first op (`to_prompt`) wired · `tests/llm.rs`. **Real provider backends deferred (surface decision)** |
| IS-5 | Candidate surfaces preserved | ➖ | deferred *decision* (documented, not code) |
| IS-6 | Agent-native vs hosted consequence | ➖ | deferred *decision* |
| IS-7 | Build order (1 store+schema → 2 det. core → 3 LLM → 4 surface) | 🟡 | **steps 1 + 2 complete; step 3 started** (LlmBackend + mock + first op); real backends + step 4 (surface) not started |

---

## Cross-cutting project rules — [AGENTS.md](../AGENTS.md)

| Rule | Status | Evidence / note |
|------|--------|-----------------|
| Schema-first; `validate_schema.py` passes | ✅ | schema loads via `Schema::from_multiple_yamls`; `schema::tests::all_domains_merge_and_validate` (26 nodes / 52 edges) |
| No silent fallbacks / no silent drops | ✅ | fail-loud CRUD (`unknown_node_type_fails_loud`, `missing_required_property_fails_loud`), truncation reporting (IP-14), `skipped_operations` (HEAL-7), unknown-seed surfacing |
| Terminology matches the schema | ✅ | `nodes::{node,edge}` constants mirror schema names |
