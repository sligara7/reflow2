# Extraction Plan — leveraging storyflow's battle-tested pipeline

> Part of the **Reflow 2.0** design docs — see **[overview.md](overview.md)** for the full map and reading order.

This is a **direct adaptation of storyflow's multi-pass extraction process**, not a
reinvention. Storyflow's pipeline encodes a lot of hard-won lessons; we keep the
architecture and the disciplines verbatim, and only swap the *domain vocabulary*
(narrative entities → design entities from `../schema/`) and the *gate categories*
(narrative categories → design/phase categories).

Source of truth we are mirroring, in
[github.com/sligara7/storyflow](https://github.com/sligara7/storyflow):
`services/generation_plus/src/modules/extraction/` (multi_pass_orchestrator,
`passes/`, `graph_informed_*`) and the domain-neutral `dynograph-extract` Rust crate
(from [dynograph-foundation](https://github.com/sligara7/dynograph-foundation)).

---

## The loop (unchanged from storyflow)

```
input → EXTRACT (multi-pass) → RESOLVE (graph-informed) → INTEGRATE (typed payload)
        → dynograph MERGE → graph informs the next extraction
```

Input for us is any **design material**: a brief, a brainstorm transcript, a spec, a
review note, an existing code/CAD/doc file, or an agent's own design reasoning.

---

## Three-phase fan-out (mirrors `orchestrate_extraction`)

Every pass is small, focused, does ONE job, reads prose only unless it needs a roster,
and returns a strict JSON shape. Passes run in parallel within a phase.

**Phase 1 — always-run (parallel), read input only**
- `project_intent` — Project + objective + mode
- `requirements` — Requirement[]
- `constraints` — Constraint[] + DesignRule[]
- `capabilities` — Capability[] (the WHAT roster; carries inputs/outputs/type)
- `discovery` — the **gate classifier** (see below); pins to a FAST non-reasoning model

**Phase 2 — entity passes gated by Phase-1 discovery flags (parallel)**
- `flows` (gated: flows present) — Flow[] + PART_OF_FLOW
- `components` (gated: structure present) — Component[] + ALLOCATED_TO
- `interfaces` (gated: interfaces present) — Interface[] + PROVIDES/CONSUMES
- `actors` (gated: actors present) — Actor[] + INTERACTS_WITH
- `decisions` (gated: decisions present) — Decision[] + GOVERNED_BY
- `artifacts` (gated: realizations present) — Artifact[] + REALIZES
- `resources` (gated: resources present) — Resource[] + REQUIRES_RESOURCE

**Phase 3 — typed-edge + profile passes gated by Phase-1+2 rosters (parallel)**
- `dependencies` — DEPENDS_ON edges over the capability/component roster (keep DAG acyclic)
- `satisfies` — SATISFIES edges (Capability/Component → Requirement/Constraint)
- `verifications` (gated) — Verification[] + VERIFIES
- `inference` — the "why" layer: CAUSES/ENABLES/BLOCKS/RISKS/MITIGATES/... over all rosters
- `dimensions` (gated) — DimensionObservation[] per node (per-epoch, evidence-anchored); they
  roll up into the current DimensionAssessment (Axis-Z depth drift)
- `changes` (gated: change signals present) — ChangeEvent[] + `CHANGED` edges, with the
  change wired to its cause via the inference edges (a failed Verification `CAUSES` a
  `test_failure_fix`). See **Axis Z** below.

Adding a pass = one named constant + one gating entry, per storyflow's centralized
phase-key convention (no naked strings across the file).

### SME augmentation (supplementary analysis) — a gated post-pass

After faithful extraction, an optional **SME pass** (mirrors storyflow's Part-4
supplementary analysis) has the LLM act as a cross-domain subject-matter expert for the
project's domain and operating `Environment`, surfacing considerations the user never
stated — proposed `Requirement`s/`Constraint`s (incl. `concern: logistics`), `RISKS`,
missing `Capability`s, `EnvironmentRule`s. Each is labeled on the grounding spectrum
(`verified` / `extrapolated` / `speculative` / `contradicts_known`) with `domain` +
`confidence`. Output lands as a supplementary Fragment (`provenance: inferred`) linked via
`SUPPLEMENTS`, and surfaces to the user as `sme_consideration` gap questions to accept /
edit / dismiss — amplify, never silently merge as fact. See
[sme-augmentation.md](sme-augmentation.md).

---

## The discovery gate (mirrors `passes/discovery.py`)

A single T=0 classifier answers "what design content is present?" as orthogonal
booleans, gating Phase-2/3 so we don't spend LLM calls looking for structure in a
pure requirements brief.

> **Anchor-required gating (critical lesson).** Return `true` ONLY when the input names
> a concrete instance. Recall-bias collapses the classifier to constant-true. For us:
> "components" is true when a named unit/module/service is described *acting as a unit*,
> not when architecture is merely alluded to; "interfaces" needs a named contract, not
> the word "API".

Design categories (orthogonal): `flows`, `components`, `interfaces`, `actors`,
`decisions`, `artifacts`, `verifications`, `resources`, `dimensions`, `changes`.

---

## Non-negotiable disciplines (storyflow lessons — keep verbatim)

1. **One shared LLM-call helper** (`call_pass_llm` equivalent). Model swap, headers,
   retry, caching, and error-enveloping live in ONE place.
2. **Never-raises + error envelopes.** Every failure (HTTP, connection, JSON decode,
   provider `error` block) returns a sentinel-keyed dict (`_error` / `_parse_error` /
   `_raw` / `_timeout`) alongside default-empty arrays. One bad pass must never
   cancel-cascade the `asyncio.gather` — siblings' results survive intact.
3. **Per-pass timeout budget** (`_bounded_pass`). A hung/504 pass fills only its own
   slot with `{_timeout: True}`.
4. **NO SILENT FALLBACKS (principle #2).** A recoverable empty (finish_reason=length /
   reasoning-shaped `content=None`) → retry ONCE at ~4× budget. Still empty → surface a
   LOUD `_error`, never a silent all-default. Silent data loss is an integrity breach.
5. **Keep the gate off reasoning models.** The discovery classifier must run on a fast
   NON-reasoning model — a reasoning model burns the small budget on reasoning tokens,
   returns empty, and silently zeroes every gated pass.
6. **Focused prompts, orthogonal axes.** Each pass has a strict output JSON shape, tuned
   temperature (T=0 for classification, ~0.3 for extraction), and "pick ONE value per
   axis" instructions. List fields are ALWAYS arrays, never joined strings.
7. **Implicit prefix caching.** Put the unchanging input FIRST in the message array so
   parallel passes on the same fragment share a prefix (cache hits ≈0.25×). Prefix
   should clear ~1024 tokens.
8. **Type/enum tuples from the schema (single source of truth).** Drive allowed
   node/edge/enum values from `../schema/*.yaml`; fail LOUD on schema drift rather than
   silently emptying a tuple and dropping every emitted entry.
9. **Symmetric-edge auto-inverse.** For symmetric relations (e.g. a bidirectional
   DEPENDS_ON or CONTRADICTS), auto-emit the inverse — the LLM only narrates one
   direction.
10. **Per-fragment metrics.** Sum `_usage` / `_timing_ms` sentinels into a stable
    cost/cache-hit/timing summary (grep/jq now, Prometheus later).
11. **Selective context threading.** Thread extra context (project domain, prior graph
    state, mode) only into passes whose output depends on it — avoid prompt noise/cost
    on invariant passes.

---

## Graph-informed resolution (mirrors `graph_informed_*`)

Runs AFTER extraction, as a separate post-step, so re-ingesting an updated brief
*updates* nodes instead of duplicating them:

1. Summarize extracted entities.
2. Fetch known state from the graph concurrently (known components, capabilities,
   communities) + a vector-match probe on the input.
3. Ask the LLM what to look up (query generation); a FAILED query-gen call must NOT
   early-return — degrade to empty graph context AND record it.
4. LLM resolves each candidate into **one of three outcomes** (not just match-vs-new):
   - **matched-unchanged** — same node, same content → no-op.
   - **matched-evolved** — same node, changed content → this is an Axis-Z *change*, not a
     duplicate. Snapshot the prior state, close the superseded facts, open new ones, and
     emit a `ChangeEvent` (see below). Re-ingesting an updated brief must record the
     change, never silently overwrite.
   - **genuinely-new** — create it.
   dynograph's `resolution: fuzzy_then_vector` does the mechanical dedup; the LLM
   adjudicates the ambiguous middle and the changed-vs-same call.
5. Accumulate non-fatal degradations into `partial_failures` and return
   `status:"partial"` — never a silent "ok".

> **Dependency note — embedding generation is required, and it is a separate slot from
> vector storage.** Nearly every node type in `../schema/` declares an `embedding_field`
> and a `resolution: { strategy: fuzzy_then_vector }` (see `core.yaml`, `structure.yaml`,
> `operate.yaml`, `environment.yaml`, `dimensions.yaml`). The `_then_vector` half and the
> "vector-match probe" above both require an embedding vector per node — so **something
> must turn `description`/`statement`/`purpose` text into a vector**. dynograph-foundation
> supplies vector *storage + HNSW nearest-neighbor search* (and the
> `resolve_entity(…, embedding)` / `find_similar` plumbing), but it does **not** generate
> embeddings. That generation slot is filled by an embedding service — storyflow uses
> [embeddings-rs](https://github.com/sligara7/embeddings-rs) (stateless HTTP, nomic-embed-text-v1.5,
> 768-dim) wired to dynograph via `EMBEDDING_URL`. Because the slot is decoupled behind
> that contract, reflow2 can use embeddings-rs or any equivalent provider; **which one is
> tied to the deferred interaction-surface decision** (agent-native surface may reuse the
> agent's own embedding access; a hosted surface will want a self-contained generator like
> embeddings-rs — see [interaction-surfaces.md](interaction-surfaces.md)). Caveat: in
> storyflow this is still a stub — the extraction pipeline currently passes `None` for the
> embedding (`// TODO: generate embeddings`), so the HTTP call is not yet wired. Honoring
> `fuzzy_then_vector` in reflow2 means closing that seam, not just declaring it.

---

## Integration (mirrors `dag_client.integrate_fragment`)

- ONE typed payload shared by `integrate_fragment` and `create_and_integrate_fragment`
  so a new field can't fail to propagate to one path.
- dynograph MERGEs nodes/edges via `create_or_resolve_node`; provenance flag
  (`authored` / `planned` / `inferred` / …) rides on the Fragment.
- LLM-invented edge types not in schema are dropped but surfaced as a WARN
  (prompt-tuning signal), never silently swallowed.

---

## Axis Z — recording change over time (INGEST × temporal)

Every ingest happens **in the context of an active `DesignEpoch`** (the current version
of the design). The epoch is threaded into the extraction context (selective-threading
discipline #11) so the `changes` pass and time-aware resolution can attribute edits
correctly. A new epoch is opened at a version/phase boundary or when the caller declares
one ("start v1.1").

**The `changes` pass** reads the input for evolution signals and emits `ChangeEvent`s
with a typed `change_type` — `requirement_creep`, `new_feature`, `test_failure_fix`,
`refactor`, `scope_change`, `constraint_change`, `deprecation`, `resync` — each wired to
the node it `CHANGED` and, through the inference layer, to its cause (a failing
`Verification` `CAUSES` the fix; a new stakeholder ask `TRIGGERS` the creep).

**Time-aware integration** — when resolution returns *matched-evolved*, integration does
not overwrite in place. Instead, at the epoch boundary it:
1. writes a `Snapshot` of the node's prior property bag (`HAS_SNAPSHOT` → `AT_EPOCH` the
   closing epoch) — the past is remembered, not clobbered;
2. sets `VALID_TO` on the `TemporalFact`s that stop being true, and opens new ones
   `VALID_FROM` the current epoch;
3. MERGEs the node's new state and links the `ChangeEvent` (`CHANGED` → node,
   `AT_EPOCH`/`OCCURS_DURING` → epoch);
4. stamps the Fragment `provenance` (`authored` for user edits, `reconciled` for
   drift-driven, etc.) so the change's origin is auditable.

**Discipline:** a *matched-evolved* result that lands with no `Snapshot` + no closed
fact is a silent overwrite — treated as an integrity breach, same bar as the extraction
no-silent-fallback rule. The prior state must always be recoverable.

This is what lets DIAGNOSE ask time-aware questions ("this requirement was added in v1.1
and still has no verification") and lets the graph diff any two epochs.

---

| 3-phase orchestrator + gating + metrics aggregation | **verbatim structure** |
| `graph_informed_*` resolution flow | **verbatim structure** |
| `dynograph-extract` schema-driven `extract_and_integrate` | **reuse the Rust crate** |
| Pass *prompts* (characters, events, therapeutic, …) | **re-key** to design passes above |
| Discovery *categories* | **re-key** to design/phase categories |
| storyflow temporal layer (NarrativeEpoch / TemporalFact / snapshots) | **reuse structure** as `DesignEpoch` / `TemporalFact` / `Snapshot` / `ChangeEvent` |
| DimensionObservation → DimensionAssessment rollup | **reuse verbatim** (per-epoch depth drift) |
| Node/edge *vocabulary* | **replace** with `../schema/` |
