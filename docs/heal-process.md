# HEAL Process — self-repair for the design graph

> Part of the **Reflow 2.0** design docs — see **[overview.md](overview.md)** for the full map and reading order.

Adapted from storyflow's **Process 5: HEAL**
(design notes in [github.com/sligara7/dev_storyflow](https://github.com/sligara7/dev_storyflow)
`docs/architecture/five-universal-processes.md`) and its implementation in
[github.com/sligara7/storyflow](https://github.com/sligara7/storyflow)
(`services/generation_plus/src/routers/healing.py`,
`schemas/healing.py`, and the Rust `compat/story_validation.rs`).

storyflow found that six **universal processes** recur in every domain. They are the
cyclic engine that operates on the graph; Reflow's phases (P0–P5) are the linear
lifecycle spine. They are complementary — phases say *where you are*, processes say
*what runs on the graph continuously*:

| Universal process | Reflow analogue | Purpose |
|---|---|---|
| GENESIS | seed P0/P1 from a brief | bootstrap the graph so there's something to work with |
| INGEST | the extraction pipeline (`extraction-plan.md`) | content → entities/edges |
| DIAGNOSE | `find_gaps`, `analyze_impact` | rank issues & opportunities |
| PROMPT | `assemble_work_context`, made active | brief the human/agent to fill a gap |
| SYNTHESIZE | as-built architecture, diagrams, docs | graph → artifacts |
| **HEAL** | this doc | detect structural defects & repair them |

---

## What HEAL does

Detect structural defects in the design graph — then repair them, automatically for
safe fixes or via a human-reviewed **proposal** for anything generative. HEAL never
mutates directly; it emits a proposal the caller applies atomically.

```
DIAGNOSE/validate → HEAL (propose) → [human review if needed] → apply-fixes (atomic)
                                                                        ↓
                                                            INGEST the generated content
```

---

## Defect catalog (storyflow categories → design vocabulary)

Each detected issue carries `id`, `category`, `severity` (critical / warning / info),
a human message, and a `suggested_fix_type`.

| Category | In a design graph, this is… | Severity | Suggested fix |
|---|---|---|---|
| `orphan_node` | a Capability not `ALLOCATED_TO` any Component; an Artifact that `REALIZES` nothing; a Requirement with no `SATISFIES` | critical for Fragment, else warning | create edge / generate owner |
| `dead_end` | a Flow step with no downstream; a Component nothing depends on and that provides nothing | warning | create edge |
| `unreachable` | a Capability unreachable from any Flow entry point | warning | create edge |
| `disconnected_community` | a cluster of nodes with no link to the rest of the design | warning | create bridging edge |
| `weak_connection` | a subsystem hanging by a single edge | warning | create edge |
| `single_point_of_failure` | a Component every path routes through | warning | add redundancy / note |
| `missing_link` | two Components that clearly should share an `Interface` | info | create Interface + edges |
| `contradiction` | two Requirements/Decisions joined by `CONTRADICTS` | warning | generate resolving Decision |
| `unresolved_setup` | an `ANTICIPATES` with no follow-through — a planned need never built | info | generate Capability/Artifact |
| `duplicate` | two Capabilities/Components covering the same ground (`DUPLICATES`) | warning | merge (entity resolution) |
| `missing_entity` | an entity referenced but absent (e.g. an Interface named but not modeled) | warning | generate entity |
| `missing_embedding` | a node with no vector (breaks similarity/resolution) | info | embed |

The first six are lifted almost verbatim from `story_validation.rs`; the rest cover the
design-specific gaps DIAGNOSE/`find_gaps` already knows about in Reflow today.

---

## Strategies (verbatim from storyflow)

| Strategy | Fixes | Content generation |
|---|---|---|
| `conservative` | CRITICAL only | minimal |
| `balanced` (default) | CRITICAL + WARNING | generate bridges & missing entities where needed |
| `aggressive` | all, incl. INFO | generate content liberally |

Plus `max_operations` (cap the proposal size) and `priority_categories` (focus on
specific defect types).

---

## The proposal (mirrors `HealingProposalResponse`)

```
HealProposal {
  target_id                # project/subgraph being healed
  validation_report_id
  strategy
  issues_addressed[]       # issue ids this proposal targets
  operations[]             # graph ops: create_edge, create_node, merge, embed …
  generated_content[]      # LLM-produced design content awaiting INGEST
  skipped_operations[]     # dropped ops + the ref + WHY (never silently dropped)
  skipped_bridges[]        # generative fills that couldn't be grounded + why
  confidence               # 0..1
  requires_human_review    # gate — true whenever generated_content is non-empty
  summary                  # human-readable
}
```

### Non-negotiable disciplines (same integrity bar as extraction)

1. **Propose, then apply.** HEAL computes; a separate atomic `apply-fixes` mutates.
2. **No silent drops.** An operation whose endpoint can't be resolved to a real node
   (an LLM-invented placeholder ref, an issue-id in an id field, an ambiguous name) is
   moved to `skipped_operations` **with the offending ref + reason** — never emitted as
   a phantom edge that the atomic apply would 404 on.
3. **Human-review gate.** Any proposal that *generates* content (new Capability,
   Interface, Decision, Verification) sets `requires_human_review = true`. Structural-only
   fixes (create a `SATISFIES` edge between two existing nodes) can auto-apply under
   `conservative`/`balanced`.
4. **Post-repair verification.** After apply, re-check the defect is gone (storyflow uses
   shortest-path to confirm a disconnected node is now reachable) before marking resolved.
5. **Provenance.** Everything HEAL creates enters via a Fragment with
   `provenance: healed` (the schema already carries this value) so healed content is
   distinguishable from authored/extracted content and reversible.
6. **Mode-aware.** In `rigid` project mode, HEAL only *proposes* (never auto-applies)
   and records a change request; in `flexible` mode it may auto-apply structural fixes.

---

## Generative healers (storyflow → design)

storyflow ships two content generators; we generalize them:

| storyflow | design-graph equivalent |
|---|---|
| **bridge scene** (connect two disconnected fragments) | **bridge edge/interface** — synthesize the `Interface` + `PROVIDES`/`CONSUMES` (or a `DEPENDS_ON`) that connects two orphaned subsystems, with a rationale |
| **missing character** (referenced but absent) | **missing entity** — synthesize the spec for a referenced-but-unmodeled Capability/Component/Interface, wired to what referenced it |

Additional design-native healers worth having: **contradiction resolver** (propose a
`Decision` that reconciles two `CONTRADICTS`-linked nodes) and **verification filler**
(propose a `Verification` for an unverified Capability).

**Guarded creative-bridge healer (from chain_reflow).** For *orthogonal* subsystems with
no obvious technical link, HEAL may propose a cross-domain bridge by structural analogy /
synesthetic mapping ("receptors are like event listeners"). This is strictly guarded:
`requires_human_review = true` always, marked speculative provenance, never auto-applied,
and it must cite the analogy as a *hypothesis* to validate — the disciplined form of
"close this gap" for genuinely unrelated domains. For missing **hierarchy** links it
proposes the *missing intermediate* `Component` (a new `Engine` subsystem), not a
cross-level edge.

---

## Reuse vs. build

| storyflow asset | plan |
|---|---|
| validate categories + severity + suggested_fix_type (`story_validation.rs`) | **reuse structure** — dynograph-foundation's `dynograph-graph` crate already has centrality/components/paths to power detection |
| proposal shape, strategies, skipped-op surfacing (`schemas/healing.py`) | **reuse verbatim**, re-typed to design vocabulary |
| bridge/missing-character generators (`llm/healing_generation.py`) | **re-key** to bridge-edge / missing-entity / contradiction-resolver / verification-filler |
| duplicate detection + merge (entity-resolution endpoints) | **reuse** — dynograph `resolution: fuzzy_then_vector` + a merge op |
| apply-fixes atomicity | **reuse** — dynograph batch ops (`BatchOp`) |
