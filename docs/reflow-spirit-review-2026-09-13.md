# Has reflow2 kept the spirit of the original reflow? — a review, 2026-09-13

> Part of the **Reflow 2.0** design docs — see **[overview.md](overview.md)**. The item-by-item
> pass over reflow's workflows and tools is **[reflow-audit.md](reflow-audit.md)** (2026-07-18)
> and its summary **[reflow-v3-nuggets.md](reflow-v3-nuggets.md)**; those asked *does reflow2
> have this?*. This document asks a different question, put by Anthony on 2026-09-13:
> *"Many times a project can drift over time away from its initial purpose … I want to see if
> we've maybe got away from the 'spirit' and 'intent' of the original reflow."*
>
> Registered in the design as a dated review and **parked** (`dec:a-dated-field-report-is-registered-and-parked`).
> The verdict and the numbers live on `fact:reflow2-keeps-reflows-spirit-and-inherits-its-one-real-drift`.

## How it was read

The whole of `/home/ajs7/project/reflow` (215 commits, 2025-10-16 → 2025-12-06, v3.0 → v4.1.2):
the three founding documents read directly, and five independent readers over `workflows/` +
`workflow_steps/`, `tools/` + `schemas/` + `specs/`, `definitions/` + `templates/` + `CLAUDE.md`
+ `CHANGELOG.md`, `docs/` (~170 files), and `scientific-reflow/` + `test_systems/` + `tests/`.
Each was asked the same thing: what did this part say reflow was *for*, what did it warn against,
and where did reflow itself drift. reflow2's side is its own design graph, including the seven
requirements the July read pinned under `epoch:reflow-workflow-review` and the ~20 requirements
that cite reflow's files.

**Not verified:** reflow's tools were not re-run; reflow's test-system claims are quoted from its
own reports, which round up (see §5).

---

## 1. Verdict

**reflow2 has kept faith with reflow's founding spirit and with most of its principles. Where
reflow2 has drifted, it has drifted in the same direction reflow itself did: toward working on
itself.** That is the one finding worth acting on.

Two places reflow2 sits *behind* reflow's intent rather than beside it: the mandatory
human-readable view (reflow: "stakeholders cannot validate machine-readable JSON"), and three of
reflow's lifecycle ideas that reflow2 read in July and left `proposed` — intent re-injected into a
long build, a bounded build loop, and operational scenarios as design objects.

---

## 2. What reflow was, underneath the accretion

**The axiom** — `definitions/core_abstraction.json`:
> "principle": "Every system can be modeled as nodes (black boxes with functions) connected by edges (interfaces/interactions)" — "purpose": "Enable universal graph analysis regardless of domain-specific framework"

**The founding diagnosis** — `docs/archive/instructions/INDEX.md`, 2025-10-19:
> "The original approach had three critical issues: 1. Context Overload - 1600+ line files overwhelmed LLM agents 2. Repository Clutter - No enforcement of clean directory structure 3. Instruction Non-Adherence - Critical rules got buried and forgotten"

**The product was the gap** — `tools/system_of_systems_graph_v2.py:610-620`:
> "1. Orphaned interfaces: Consumed interface with no provider 2. Unmet dependencies … 3. Implied mediators: Two components interact but incompatible interfaces (missing translator) 4. Missing feedback … 5. Structural holes: High betweenness nodes (fragile single points of contact) 6. Unexplained outputs"

**Function before structure** — `specs/functional/functional_requirements.json`:
> "Define WHAT Reflow must do (functional capabilities) before defining HOW it does it (system architecture)"

**Drift is the enemy** — `workflows/workflow_master.json`:
> "purpose": "Prevent architecture drift, enforce phase transitions, manage iteration budgets"

**The human chooses the regime** — `workflows/resync_protocol.json`:
> "flexible": "Implementation reveals requirements; architecture should reflect reality" / "rigid": "Architecture is approved; implementation must conform"

**Compute structure, hand back meaning** — `tools/analyze_functional_architecture.py:428-431`:
> "Found {n} cycles. Review to determine if these are intentional iterative refinement loops or problematic circular dependencies."

**Humans cannot validate JSON** — `workflows/01-systems_engineering.json`:
> "visualization_critical": "Human-readable visualizations (BPMN process flows, UML diagrams, decision trees) are MANDATORY, not optional. Machine-readable JSON is insufficient for stakeholder communication and validation."

**History is never destroyed** — `workflow_steps/systems_engineering/SE-02-ServiceArchitecture.json`:
> "preservation_rule": "NEVER delete old versions - they are historical record and enable rollback"

**Design the real world upfront** — same file:
> "critical_principle": "Operational environment is an ARCHITECTURAL DECISION, not an operational problem."

**The mission statement itself was retrofitted.** `docs/summaries/META_ANALYSIS_SUMMARY.md`
(2025-10-25) lists `SYSTEM_MISSION_STATEMENT.md`, `USER_SCENARIOS.md` and
`SUCCESS_CRITERIA.md` under *new files* — generated by reflow running its own setup workflow on
itself, nine days after the earliest docs. Reflow's spirit is in the axiom and the diagnosis, not
in the mission page.

---

## 3. Where reflow itself drifted (its own record)

| Drift | Evidence |
|---|---|
| **The knife sharpening the knife** | 97 (GAN test), 98 (self feature-update), 99 (meta-analysis, 55 KB — larger than the development workflow). `99-meta_analysis.json` v4.0.0 analyses "phase DAG integrity, transition rules, iteration budgets, resync protocol, architecture anchors" — its own control structures. |
| **Accretion, one shrink ever** | `CHANGE_PROPOSAL_20251025_tool_cleanup.md`: 24 → 16 tools ("Cognitive Overload: 24 tools overwhelm users/LLM agents"). Then 28, 29, 39, **49+** by 2025-11-24. Workflows 1 → 6 → 15 → 20+. |
| **Instruction file became a changelog** | `CLAUDE.md`: 814 lines / 41 KB, 14 stacked "NEW in vX" banners; the operative rules are ~60 lines repeated four times. |
| **PASS over emptiness** | `docs/validation/v3.7.0_systems_cohesion_validation.md`: `"num_nodes": 15, "num_edges": 0` … "Graph is a valid DAG (0 nodes with edges means trivially acyclic)" — "✅ VALIDATION PASSED". |
| **Referenced but absent** | 39 tools named by workflows do not exist; 38 step files do not exist, including `LM-03`, which the migration workflow calls "the foundation" and says "NEVER skip". `tool_readiness_index.json`: `validated_tools: 0, tested_tools: 0`. |
| **Crossed its own handoff boundary** | `README_STANDALONE.md` (2025-10-16) ends at "Handoff to Development". By v3.20.0 reflow chose hatchling, ruff, pytest, pnpm, Cargo, go mod; by v4.1.0 it was porting a 27,741-line C++ NES game to Python (`TRANSLATION_LEARNINGS.md`, the last doc written). |
| **Broke its own absolute rule** | `1-behavioral-rules.json` (2025-10-19): "NEVER create files with names containing: report, summary, status, update, overview … exceptions: NONE". Six days later: "~25,000 words" of reports, 30+ files named REPORT/SUMMARY. |
| **Restructuring declared complete while dropping substance** | `CONTEXT_MANAGEMENT_ADDENDUM.md` (2025-10-24): "~40% of context management features transferred"; `VERSIONING_AND_HUMAN_DOCS_ADDENDUM.md`: "~15% of versioning features transferred". `RESTRUCTURING_SUMMARY.md`, same day: "✅ COMPLETE / PRODUCTION READY". |
| **The problem it never solved, in its own words** | `META_ANALYSIS_LLM_COMPLIANCE_20251124.md`: "Reflow has excellent post-hoc validation but insufficient pre-execution constraints … Root Cause: LLMs optimize for 'getting to the answer' not 'following the process.'" TC-004: "46% time lost to tool friction". |

The scientific fork (`scientific-reflow/`) inverted the value system — "Treats knowledge gaps as
DESIRED FEATURES (not bugs)" — and never ran its inference: "IF we implement … THEN Scientific
Reflow WOULD infer: Exciton Energy ~1.47 eV" (`nips3_rixs_validation/docs/gap_closure_analysis.md`).
Its three-beamline "3/3 PASS" checks four structural booleans and no physics.

---

## 4. reflow2 against reflow's intent

### Kept faith

| Reflow principle | reflow2 today |
|---|---|
| The black-box axiom | `req:recursive-black-box-decomposition`, accepted, with its own measured shortfall (depth 2, seams 13 of 72) recorded rather than asserted |
| Function before structure | genesis stops at capabilities; `concept_without_design`; the maturity ladder intent → function → allocation → seams → realization → assurance → operation |
| The gap is the product | `detect_gaps`, `detect_defects`, HEAL — and `dec:report-dont-judge`, no fabricated repairs, every sweep names what it could not have found |
| Never PASS over emptiness | modularity reads "not measurable" where reflow printed 1.00; an unknown contributor is refused; every empty answer says which empty it is (`fact:defect-allocation-health-is-green-over-an-empty-structure`) |
| Drift prevention | checksums, `reconcile_artifacts`, two-sided `set_artifact_checksum`, DriftEvents, the coherence gate on every commit — mechanised, not instructed |
| The human chooses the regime | `Project.mode` rigid/flexible; `rule:design-intent-moves-only-on-the-owners-word` ENFORCED; a settling status without an approver is refused |
| History is never destroyed | snapshot per revision, ChangeEvent with cause, `VALID_TO` retirement — a founding requirement |
| Instructions must not sprawl | skills and instructions served from the binary; `CLAUDE.md` is a pointer (the exact repair for reflow's 41 KB file) |
| Referenced ⇒ exists | every served report must be reachable from an instruction; every skill has a command; `coverage_report` names files no node points at |

### Dropped on the record in July, and rightly

`req:framework-is-chosen-not-defaulted` (registry: a label must never gate computation; only the
"domain as a hint to phrasing" half survives), `req:context-is-a-modelled-quantity` (context as a
static edge weight: "precise and wrong" — measure instead), `req:drift-rolls-up-to-a-score` (no
single similarity scalar), plus friction baseline/bar, partial blocking, edge defaults. Reflow's
own later admissions confirm each.

### Inherited reflow's drift

| Reflow's failure | reflow2, measured |
|---|---|
| The knife sharpening the knife | 7,472 of 9,335 recorded tool calls (80%) are the self-host; 700 Decisions and 4,380 nodes about itself; the standing rule `rule:reflow2-is-built-for-other-projects-not-for-itself` has been restated twice |
| Tools accrete, never retire | 181 served tools; 40 never called in 47 sessions (`dec:bl-155`, which cannot tell unused from unreachable); one retirement ever (the content store) |
| Validation without execution | 249 of 254 verifications have no executable form; 42 never ran; "a green gate is the weakest evidence in the building" (Anthony) |
| Enforcement layers stacked on an unsolved problem | hint → lesson → nudge → Stop hook → skill_hint; `epoch:skill-use-still-needs-reminding`; `req:skill-use-survives-a-long-session` accepted and NOT built — reflow's "LLMs optimize for getting to the answer", unsolved on both sides |
| Neutral vocabulary, software-shaped delivery | `dec:idea-reflow2-serves-a-project-that-is-not-software`; Artifact types model/drawing/diagram used 0 times; the Environment/EnvironmentRule layer designed and never built |
| A record that reads confident after the thing moved | reflow: "PASS - production-ready" beside "unusable for large codebases"; reflow2: 106 of 108 Components read `planned` on a shipped system (found sideways during this review) |

### Behind reflow's intent

1. **The human-readable view.** Reflow made BPMN/UML visualisation MANDATORY and BLOCKING because
   "stakeholders cannot validate machine-readable JSON". reflow2's human view is prose
   (`where-am-i`) and `graph_report_markdown`; SV-1/SvcV-1 and SV-4/SvcV-4 rendering is
   `epoch:the-systems-views-are-rendered-from-the-graph`, planned and unbuilt, and now unblocked
   by the seam work. Anthony's own doctrine (views are pure projections; a renderer fill-in is a
   defect) is stricter than reflow's, and unrealised.
2. **Three lifecycle ideas read in July and never settled** (`proposed` since 2026-07-28):
   `req:intent-is-reinjected` (reflow's architecture_anchor + every-N-operations refresh — its
   payload arrived 2026-09-13 as `req:the-implementing-agent-holds-a-compact-design-reference-derived-from-the-graph`),
   `req:build-loop-is-bounded` (reflow's iteration budget, P3↔P4), `req:scenarios-are-modelled`
   (reflow's USER_SCENARIOS as validation targets).

---

## 5. The readers' ranked quotes (the trail)

### Workflows and workflow_steps
1. `workflow_master.json` — "purpose": "Prevent architecture drift, enforce phase transitions, manage iteration budgets"
2. `01-systems_engineering.json:1799` — "Define WHAT the system does (functions) before HOW it does it (system allocation)"
3. `phase_transitions.json` — "You are implementing an architecture designed with specific intent. Before making changes that affect service boundaries, interfaces, or core functionality, VERIFY against this anchor."
4. `01-systems_engineering.json` — "Human-readable visualizations … are MANDATORY, not optional. Machine-readable JSON is insufficient for stakeholder communication and validation."
5. `resync_protocol.json` — flexible: "Implementation reveals requirements; architecture should reflect reality" / rigid: "Architecture is approved; implementation must conform"
6. `01-systems_engineering.json:1820` — "Jumping directly to system decomposition without functional architecture causes issues later requiring revisiting service definitions (discovered in real-world content creation services project)."
7. `01-systems_engineering.json` — "VALIDATION TOOLS ARE NOT OPTIONAL … ITERATE until validation passes with ZERO errors - not 'mostly works'"
8. `D-02-CoreAndDomain.json` — "Development tasks are long. LLMs can 'forget' they're executing a Reflow workflow. This section defines checkpoints to re-anchor."
9. `phases/P4_validation/phase_definition.json` — "P4 is where drift becomes visible - tests reveal reality … Forcing resync is protective, not punitive"
10. `SE-02-ServiceArchitecture.json` — "Operational environment is an ARCHITECTURAL DECISION, not an operational problem."

Inside reflow's workflows: two orchestration layers that do not reference each other
(`workflows_master_index.json` v3.7.0 linear order vs `workflow_master.json` v1.0.0 phase DAG); a
dead third index from 2025-10-16; four deprecated monoliths kept (241 KB of 944 KB), one of them
still a listed prerequisite; the same rationale blocks byte-identical in three files after a
refactor justified by "No code duplication"; a coverage gate loosened from blocking to 60% "to
support meta-analysis scenarios (Reflow analyzing itself)"; BLOCKING in 48 of 80 files, MANDATORY
in 24, and keys named `🚨_MANDATORY_STRUCTURE_REQUIREMENTS_READ_FIRST` — warnings about warnings.

### Tools, schemas, specs
1. `definitions/core_abstraction.json:5` — the axiom (above)
2. `tools/generate_service_contracts.py:13-20` — "Purpose: Proactive drift prevention - warn LLMs BEFORE changes, not AFTER"
3. `specs/functional/functional_requirements.json:8` — WHAT before HOW
4. `specs/functional/functional_architecture.json:17` — "uses context consumption as primary edge weight to enable AI agent feasibility analysis"
5. `system_of_systems_graph_v2.py:610-618` — the six gap types
6. `tools/verify_component_contract.py:19-24` — "Integration Guarantee: If verification passes with no critical issues, component integration will succeed."
7. `create_lessons_learned_issues.sh:72-76` (LESSON-01) — "Framework selection is an ARCHITECTURAL DECISION, not a configuration choice."
8. `analyze_functional_architecture.py:428-431` — cycles handed back for judgement
9. `schemas/service_architecture_schema.json` — "All function_ids in allocated_functions MUST exist in functional_architecture.json" — the golden thread
10. `tools/bayesian_optimization/README.md` — "architectural DAGs have many interconnected properties … difficult to optimize manually"

Inside reflow's tooling: `rag_agent_wrapper.py` and `reflow_mcp_server.py` cannot run (missing
modules, missing `decision_flow.json`); two full interface-generation stacks (~1,800 LOC) for one
concept; three contract layers with two verifiers; gap detection implemented three ways with
different vocabularies; field aliases institutionalised in the schemas instead of one vocabulary;
2,552 LOC of Bayesian optimisation integrated into zero workflows; 2 tool contracts for 60+ tools.
Verdict: "reflow's analysis was domain-agnostic; reflow's production was a microservice factory."

### Definitions, templates, CLAUDE.md, changelog
1. `core_abstraction.json:6` — the axiom
2. `core_abstraction.json:5` / `framework_registry.json:5` — "one analysis engine, many vocabularies — never many engines"
3. `core_abstraction.json` implied_mediator — "Missing mediator node (like dark matter)"
4. `CLAUDE.md:45` — "LLMs optimize for 'getting to the answer' not 'following the process' - TC-004 showed 46% time lost to friction."
5. `CLAUDE.md:320` — "working_memory.json contains THE ONLY SOURCE OF TRUTH for paths."
6. `CLAUDE.md:301` — "Framework Selection is Architectural - DO NOT default to UAF!"
7. `CLAUDE.md:39` — "Functions and interfaces are language-agnostic. Extract the functional architecture first, then swap implementations while preserving interfaces."
8. `templates/operational_environment_template.json` — "Operational environment is NOT an afterthought."
9. `context/gap_analysis_context_management.md` — "Context IS modeled … Context health status is a DEAD-END - calculated but never used downstream."
10. `templates/self_improvement_template.json` — "What gates caught real issues before they became problems? What gates were burdensome without adding value? What should have been a gate but wasn't?" — asked as questions, and the automated meta-analysis never answered them.

Inside the definitions: four of seven frameworks never built (definitions files missing for
decision_flow, ecology, CAS, custom); an eighth framework (`functional_flow`) invented in working
memory and never registered; UAF-only templates are a third of all template bytes in a project
whose rule was "DO NOT default to UAF"; `*_enhanced_*` / `*_complete_*` / `*_nested_*` duplicates
kept beside their originals; `"purpose": "REPLACE_WITH_PURPOSE_DESCRIPTION"` still in the flagship
template; version identity split across four files (4.1.2 / 3.11.0 / 3.12.0 / 3.14.0).

### Docs (proposals, restructurings, reports)
1. `archive/instructions/INDEX.md` — the founding diagnosis
2. `reports/META_ANALYSIS_LLM_COMPLIANCE_20251124.md` — "excellent post-hoc validation but insufficient pre-execution constraints"
3. `archive/instructions/1-behavioral-rules.json` — "NEVER create files with names containing: report, summary … exceptions: NONE"
4. `old_documentation/README_STANDALONE.md` — "stand-alone, rigorous system architecture workflow framework that incorporates the battle-tested rigor from architecture_workflow.json"
5. `WORKFLOW_STATUS_SUMMARY.md` (2025-10-19) — "Reality: You (primary user) have never used decision_flow.json to build and deploy a working system … DO NOT claim either workflow is 'production ready'"
6. `restructuring/CONTEXT_MANAGEMENT_ADDENDUM.md` — "~40% of context management features transferred"
7. `META_ANALYSIS_LLM_COMPLIANCE_20251124.md` — "TC-002 worked because validation was objective and measurable. TC-004 failed because validation required 'reading the tool's mind' about field names."
8. `changes/WORKFLOW_FIX_FORWARD_LOOKING_TEMPLATES.md` — the user: "when it gets to the steps where it is supposed to run the tools, it says it has to go back and reformat" → "NEVER RETURNS to SE-06"
9. `proposals/CONTEXT_FLOW_ANALYSIS_v3.9.0.md` — the user: "Can we include 'context' as a 'flow' … Then AI agent context is a parameter that is purposefully built into the architecture."
10. `changes/CHANGE_PROPOSAL_20251025_tool_cleanup.md` — "Cognitive Overload: 24 tools overwhelm users/LLM agents" — made once, never invoked again

Fifteen proposals, all adopted, one made reflow smaller. Every restructuring cited the same reason
(the agent loses the thread) and answered with a new layer. The one external validation planned
(the BOT scenario, 2025-10-19) has no completion report; self-analysis replaced it.

### scientific-reflow, test systems, tests
1. `scientific-reflow/README.md` — "Treats 'knowledge gaps' as DESIRED FEATURES (not bugs)"
2. `tests/FRAMEWORK_MATTERS_ANALYSIS.md` — "YES, framework matters significantly! … Feedback loops are how biology works!"
3. `tests/execution_audit/AGENT_A_META_ANALYSIS.md` — "GAN test → 'Is the output right?' / Execution audit → 'Does the process work?' … BOTH tests are necessary"
4. same — "Tools built for human use, not agent/automation use … All tools must work in non-interactive mode by default"
5. `nips3_rixs_validation/docs/gap_closure_analysis.md` — "IF we implement … THEN Scientific Reflow WOULD infer: Exciton Energy ~1.47 eV" (never inferred)
6. `AGENT_A_META_ANALYSIS.md` — "Most users/agents would give up or incorrectly blame themselves for these tool failures. … You are NOT doing it wrong."
7. `VALIDATION_RESULTS_THREE_BEAMLINES.md` — "Differences are configuration parameters, NOT different system types!"
8. TC-003 `AGENT_A_META_ANALYSIS.md` — "WORKFLOW VALIDATED, TOOLING NEEDED … Tooling gap is the only blocker."
9. `scientific-reflow/docs/MASTER_INDEX.md` — the preconditions for inference, and "Potentially under-constrained ⚠️" on 3 unknowns / 1 observable
10. `tests/fixtures/knowledge_gaps/README.md` — "Total Tests: 2/6 implemented (33%)"; TC-004 — "~46% of execution time" lost to friction

The recorded GAN run (`tests/validation_report.json`, 2025-11-20) is **red**: three functions
silently dropped, one renamed. The execution audits' unit of measurement is friction in minutes;
their repeated verdict is "the thinking held up; the plumbing did not."

---

## 6. What this review did by hand

There is no served tool that answers *"has this project drifted from its original purpose?"* —
`find_tools` returns `set_project_mode`, `compare_designs`, `coverage_report`. The comparison of
stated intent over time was done by five readers and a hand synthesis, and is recorded with
`report_manual_work` as `tool_missing`. Whether it should be a tool is not decided here; the
observation that reflow2 cannot ask the question of itself is the finding.
