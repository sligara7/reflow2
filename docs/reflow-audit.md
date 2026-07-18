# Audit of the original Reflow — workflows and tools

> Part of the **Reflow 2.0** design docs — see **[overview.md](overview.md)** for the full map and
> reading order. This is the deep pass behind [reflow-v3-nuggets.md](reflow-v3-nuggets.md), which
> summarised the same repo at a much higher level.

A full read of [github.com/sligara7/reflow](https://github.com/sligara7/reflow) — all 25 workflow
JSON files (~800 KB), the 7 phase definitions, and all 51 tools plus the
`bayesian_optimization/` and `contracts/` subdirectories — asking one question per item: **does
reflow2 have this, does reflow2's design make it unnecessary, or is it genuinely missing?**

The point of writing down the *negative* verdicts is that they are the expensive half. A tool that
looks valuable and isn't costs more to rediscover than one that is.

## The headline

**reflow's workflows worked through systems design and broke down after it**, and the reason is
structural rather than a matter of authoring quality.

In P0–P2 the deliverable *is* the document. Process compliance and task completion are the same
act, so an instruction to produce a functional architecture produces one. In P3–P5 the deliverable
is working code, and the design update becomes overhead the agent must be nagged into. Every
downstream mechanism reflow had — the architecture anchor, the drift script, the resync protocol,
the operations counter — was an instruction issued to the agent at the moment it was least
inclined to comply. That is the failure they were written to prevent, stated in reflow's own
CLAUDE.md:

> "LLMs optimize for 'getting to the answer' not 'following the process' — TC-004 showed 46% time
> lost to friction."

reflow2's answer has to be structural: make the coherence query something the agent *wants* to run
because it answers a question it already has ("what else does this change touch?"), not a
compliance ritual that only produces bookkeeping.

## Three things reflow claimed that it did not actually have

Worth recording, because the workflows treat all three as load-bearing.

- **The foundational-alignment gate.** `validate_foundational_alignment.py` was MANDATORY and
  BLOCKING in the change workflow. Two of its five checks (`user_scenario_coverage`,
  `success_criteria_impact`) were never implemented and sit at `"not_checked"` forever; the
  reporter only escalates on `"fail"`, so they could never block. The one implemented semantic
  check is bag-of-words overlap against the mission's whole vocabulary with a magic `0.3`
  threshold, and `if not mission_keywords: return 1.0` — an empty mission file scores *perfect*
  alignment.
- **The matrix gap solver.** `matrix_gap_detection.py` (938 lines) solves `B = C · A⁻¹` for "the
  missing system". `reflow_gap_closure.py`, the tool built to feed it, contains
  `# Placeholder - would need full implementation`, computes its inputs, **discards them**, and
  emits name-mangled templates instead — while reporting `"gaps_closed": len(templates)`.
- **Bayesian architecture optimization.** 3,415 lines of real GP/acquisition machinery, zero
  tests, and **zero callers** anywhere in the repo — an orphan behind an "OPTIONAL" flag in the P2
  phase definition.

## Adopt — ranked

| # | Idea | Source | Why | Effort |
|---|---|---|---|---|
| 1 | **Finish the write side for the types DETECT asks about** | (found while auditing) | Not from reflow — surfaced by it. `Verification`, `Release`, `Environment`, `Resource`, `Decision` have no typed constructors, yet DETECT emits gaps demanding exactly those. The system asks for things it gives no guided way to provide. | small |
| 2 | **As-fielded view** | `generate_as_fielded_architecture.py` | The heritage doc's "adopt (high)", and it needs **zero new schema** — `operate.yaml`'s `Release`/`Environment`/`Resource`/`DEPLOYED_TO`/`REQUIRES_RESOURCE` are defined, classified in `propagate.rs`, and already gap-detected in `detect.rs`. Model to copy is the declared-vs-running three-way merge. | medium |
| 3 | **Root-cause classification of drift** | `version_architecture.py` / D-06.5 | `drift.rs` detects divergence but has no notion of *why*, so no notion of which side is wrong. Seven categories ending in a decision rule: `developer_mistake` ⇒ fix the code; `performance_optimization` **with test evidence** ⇒ fix the design; `requirements_creep` ⇒ only after stakeholder validation. | medium |
| 4 | **Path-cumulative budget analysis** | 3 independent tools | `propagate.rs` walks impact but never *accumulates a quantity* along source→sink paths. reflow used it for token budgets; the general form is the classic SE budget rollup — latency, mass, power, cost, error. | medium |
| 5 | **Scalar coherence score + severity bands** | `compare_architectures.py` | Needed to make #3 gateable: `DriftReport` is a findings list with no aggregate and no gate. Bands `≥0.95` / `0.7–0.95` / `<0.7` are a reasonable starting calibration. | small |
| 6 | **Foundational-alignment gate, rebuilt** | `validate_foundational_alignment.py` | The concept only. reflow2 can do deterministically what reflow faked: trace upward for severed intent paths, violated `Constraint`s, invalidated `Verification`s — judgement behind `LlmBackend`, blocking keyed off `Project.mode`. | medium |
| 7 | **Typed gap resolution strategies** | `analyze_integration_gaps.py` | Every gap carrying *what class of fix closes it* (adapter / mediator / modify / align / relax) turns a gap list into a work plan. | small |
| 8 | **Abstraction-gap → strategy** | `analyze_abstraction_level.py` | Distance between abstraction levels selects the kind of transformation. Generalizes past code translation: a one-level hop is mechanical, a two-plus-level hop means a rung is missing — a principled trigger for `requires_human_review`. | medium |
| 9 | **Document round-trip** | `parse_functional_documentation.py` | reflow2 has graph→Markdown (`report.rs`) and prose→graph (`ingest.rs`) but no closed loop where *editing the emitted document* edits the design. Strong ergonomics for a non-developer. Needs stable id anchors + a diff-and-propose ingest mode. | medium |
| 10 | **`coordination_complexity` as a dimension** | `bayesian_optimization/` | Normalized variance of distinct peers per component — catches what cohesion/coupling misses, where coupling looks fine on average but one component is a hub. Dimensionless, so it drifts well. | small |
| 11 | **Pre-flight precondition checks on apply** | `validate_component_deltas.py` | Don't create what exists, don't modify what doesn't — validated against caller-supplied observations before mutating. The natural convergence of `heal.rs` and `drift.rs`. | small |
| 12 | **MCP resources and prompts, not only tools** | `reflow_mcp_server.py` | A design snapshot is something an agent wants ambiently, not as a tool call. | small |

## Do not port — and why

- **`matrix_gap_detection.py`'s `B = C · A⁻¹`.** Multiplying *adjacency* matrices is path-counting,
  so `B` is not "the missing system"; `pinv` always returns something, so the confidence score
  cannot fail; and the interpretation layer keys fixed claims off eigenvalues ("dominant
  eigenvalue > 1.5 → trophic cascade") with nothing establishing transfer from the ecological
  source domain. Treat the abandoned integration as the verdict its author reached in practice.
- **`bayesian_optimization/`.** Real BO machinery applied to a *cheap deterministic* objective —
  a surrogate costs more than evaluating directly. With no inverse map from feature space to a
  graph, the acquisition function only ranks 20 randomly-mutated neighbours: GP-guided random
  hill-climbing, where Leiden already constructs good partitions in near-linear time. Rescue the
  two metrics (#4, #10); drop the other ~3,300 lines. Resolves the heritage doc's "explore (later)".
- **Same-network all-pairs edge inference** (`generate_as_fielded_architecture.py`). Manufactures
  O(n²) low-confidence edges into the very traceability set PROPAGATE and `structure.rs` traverse.
  Runtime topology must come from observed traffic or not at all.
- **`migrate_framework.py`.** One fixed schema makes it meaningless. Its `probability: 0.5` /
  `weight: 5` placeholder injection is a rule-4 anti-pattern: invented values indistinguishable
  from real ones.
- **`version_architecture.py`'s versioned-copy-plus-symlink scheme.** `temporal.rs` supersedes it
  immutably; symlink-swapping versioned JSON is the fragility reflow2 exists to delete.
- **Literal source-code templates** (`generate_component_deltas.py`'s Flask strings,
  `generate_interface_abc.py`, `generate_python_project.py`). reflow2 writes no code by design.
- **The whole context-management layer** — `working_memory.json`, operations counters, the
  8-step PAUSE→…→RESUME refresh, `rag_agent_wrapper.py`'s regex detection of the agent's own
  confusion (`"what system am I working on"`). Every one compensates for state not surviving a
  session. A persistent graph deletes the category.
- **File-format validators** (`validate_architecture_format.py` and five siblings). Six tools
  existed because a JSON file could be shaped wrong. Schema-validated CRUD makes those states
  unrepresentable.

## Cautions carried forward

Concrete traps found in reflow's own code, worth remembering while building the items above.

- **Deployment kind gates validation.** reflow's port registry classifies `service_type` ∈
  `deployed_service` / `library_plugin` / `sidecar` / `external_dependency`, with
  `REQUIRES_PRIMARY_PORT = {'deployed_service', 'external_dependency'}` — added in v3.23.0 to fix
  a real bug where library plugins failed for having no port. A `reconcile_deployment` that
  expects every Component to appear as a running thing will reproduce that bug as false drift.
- **Don't borrow retrieval thresholds as identity thresholds.** reflow's RAG used
  `min_similarity` 0.7–0.8 for *retrieval relevance*. The deferred `EmbeddingBackend` needs a
  threshold for "are these two Requirements the same entity", which is a different question with
  no evidence here. (Its stack — local MiniLM/384-dim, normalized inner product, content-hash-gated
  incremental rebuild — *is* useful prior art for a no-network, no-API-key seam.)
- **Three unrelated things are called "drift".** reflow conflated LLM attention decay with
  architecture divergence; reflow2 adds a third in `dimensions.rs` (quality slope). Name carefully.
- **Rule-4 counter-examples**, all from reflow, all the exact failure the discipline exists to
  prevent: the as-fielded `health_summary` folds `unknown` into `unhealthy`;
  `compare_architectures.py` computes modified-edge deltas then never classifies them;
  `bayesian_optimization` truncates path enumeration at `paths[:100]` silently; the alignment gate
  scores an empty mission file as perfect.

## What this audit validated

Choices reflow2 had already made that the audit independently confirms: propose-then-apply
(reflow's `auto_apply: false, review_required: true`), never overwrite the past ("NEVER delete old
versions"), bounded-and-reported traversal, fail-loud validation, and the selectivity lesson —
reflow's naive `structural_holes` fires on every internal node of a tree-shaped thread, which is
why `single_point_of_failure` and `circular_dependency` are deliberately narrow.
