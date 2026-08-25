# Gap Surfacing — DIAGNOSE → PROMPT (the scenarioRunner, for design)

> Part of the **Reflow 2.0** design docs — see **[overview.md](overview.md)** for the full map and reading order.

Adapted from storyflow's **scenarioRunner**
([github.com/sligara7/storyflow](https://github.com/sligara7/storyflow):
`services/generation_plus/src/services/scenario_gaps.py`,
`scenario_service.py`, `scenario_generation.py`, `llm/proactive_scenario.py`,
`schemas/scenario.py`) — its DIAGNOSE→PROMPT half of the six universal processes.

## Why this matters for "design anything"

A user won't think of every step of **concept → design → develop → test → deploy →
operate**. This process reads the graph, finds where it's thin or unbalanced, and
**poses questions back to the user** so they fill the gaps with their own intent:

> "You've laid out a concept and a design, but nothing about how you'll **deploy and
> operate** it. What environment will this run in, and what does it depend on?"

The user's answer is then **INGESTed** (the extraction pipeline) back into the graph —
closing the loop:

```
DIAGNOSE (find gaps) → PROMPT (ask a constructive question) → user answers
   → INGEST (extract answer into graph) → DIAGNOSE again → …
```

**Distinct from HEAL.** HEAL *repairs mechanical defects itself* (auto or proposed).
Gap Surfacing *asks the human* for the things only they can decide — intent, priorities,
lifecycle choices. HEAL fills structure; Gap Surfacing elicits meaning.

---

## The candidate (mirrors `ScenarioCandidate`)

Every detected gap becomes a ranked candidate:

```
GapCandidate {
  id                    # deterministic hash(gap_source + affected ids) — stable dedup/cache
  gap_source            # category (see taxonomy below)
  scope                 # project / phase / component / capability — the zoom level
  severity              # 0..1 composite → ranking; the most important gap surfaces first
  title                 # human-readable summary
  description           # WHY this matters
  affected_ids/names    # the nodes involved
  suggested_depth       # 1..5 — how deep an answer to ask for (storyflow's "heat")
  evidence              # raw analytics backing the gap (auditable)
  anchor                # grounds the question in the user's OWN prior material
}
```

### The question (mirrors `ScenarioPrompt`)

A candidate is rephrased into a prompt the user actually answers:

```
GapPrompt {
  context_setter    # 1-2 sentences placing the user back in their own design
  question          # the specific thing to answer (never graph-jargon)
  hints             # optional scaffolding / examples
  relevant_context  # graph slice the user might need
  candidate         # the gap this addresses
}
```

---

## Design gap taxonomy (re-keyed from narrative → design)

Grouped by what the gap is about. The **phase-coverage** group is the direct answer to
the user's deploy/operate ask.

### Phase-coverage gaps — "you've done X but not Y"
| gap_source | Fires when… | Example question |
|---|---|---|
| `concept_without_design` | Requirements/Capabilities exist, but no Components (WHERE) | "You've defined what it does — how should it be structured into buildable parts?" |
| `design_without_build` | Components exist, but no Artifacts realize them | "Your design is laid out — what actually gets built to realize `<Component>`?" |
| `build_without_verification` | Artifacts/Capabilities exist, but no Verification targets them | "How will you confirm `<Capability>` actually works?" |
| `no_deploy_operate` | Design/build exists, but no Release / Environment / Resource | "You have a concept and design — how do you plan to **deploy and operate** it?" |
| `no_decisions_recorded` | Non-trivial structure exists, but zero Decisions capture the rationale | "Why this structure? Record the key decision behind `<Component>`." |

### Traceability gaps
| gap_source | Fires when… |
|---|---|
| `unsatisfied_requirement` | a Requirement has no `SATISFIES` from any Capability/Component |
| `unmotivated_capability` | a Capability `SATISFIES` no Requirement — the mirror of the row above, and the direction DETECT was blind in. Rare in greenfield, where capabilities are created *from* requirements; **the dominant direction of error when reading a system backwards**, where an unjustified capability is either a requirement nobody wrote down or dead code. Severity reads `Capability.provenance`: 0.55 authored (a half-finished thought), 0.70 `inferred` (a feature in production nothing asked for) |
| `unallocated_capability` | a Capability is not `ALLOCATED_TO` any Component |
| `unallocated_component` ✅ | **LEAF** Components no Capability is `ALLOCATED_TO` — structure with no function in it, and the mirror of the row above. The two rows either side of it left this uncovered: `concept_without_design` fires only at ZERO components and goes silent once a design grows one, while `unallocated_capability` is gated the other way and stays quiet until a component exists. Between them they cover a capability with no home and a design with no structure, and neither covers structure with no function — on reflow2's own design **33 of 95 components** were leaf boxes owning no capability, and every detector reported clean. **Leaf-only, and the filter is the finding**: a parent grouping is allocated THROUGH its children, so counting parents would turn every well-formed hierarchy into a finding (40 components against 33 on the same design). One aggregate rollup keyed on the rule, not N alarms — per-component keying is the more honest key for the answer people give ("this box is a namespace, not a functional part") and loses to the BL-73 anti-flood lesson at 33 findings at once. It asks **what the system is FOR before naming any method** — the ility decides which grouping is right and the four disagree (performance: least chatter; reliability: no articulation point, possibly duplicating a function; maintainability: co-change; security: trust boundaries), so allocating without asking silently picks performance (`dec:idea-the-ility-chooses-the-allocation-graph`). `propose_allocation` is named as the PERFORMANCE answer specifically, with the warning that it clusters capability-to-capability `DEPENDS_ON` — 1 edge across 210 capabilities on reflow2's own design. It proposes no allocation itself: the tool clusters capabilities by coupling and the user accepts or overrules it (`cap:no-fabricated-repair`). The finding distinguishes **never started** (nothing allocated at all — the deferred step never picked back up) from **partial**, because those are acted on differently. Silent when the design has no capabilities: there is no allocation to have performed, and that phase is `concept_without_design`'s ground |
| `possible_duplicate` | two Components carry the same (or ≥80% the same, min 2 shared) set of allocated Capabilities — probably two implementations of one thing. **Asked, not repaired:** HEAL's `duplicate` fires on a `DUPLICATES` edge a human drew, and its merge is safe *because* the endpoints were asserted; this is a heuristic, and `apply_heal` deletes a node. A pair already joined by that edge is skipped, so the two compose — this asks, the user confirms by drawing the edge, HEAL merges. Complements rather than replaces the planned semantic rule (`resolution: fuzzy_then_vector`), which finds things *described* alike where this finds things *wired* alike |
| ~~`interfaceless_dependency`~~ | **planned under this name, SHIPPED as [`undeclared_seam`](#) below** (2026-08-13). Never implemented under this key — no code and no acknowledgement ever hashed it, so nothing is stranded by leaving it unbuilt. The shipped rule is the stricter reading: not merely "no `Interface` between them" but *no `Interface` carrying **both** a `PROVIDES` and a `CONSUMES`*, because one-sided is exactly the unrecorded contract the capture skill warns about, and that is what `maturity_report`'s `seams` band has always counted as declared. Kept as a row rather than deleted so that anyone who read the taxonomy before that date finds where the rule went |
| `unprovided_interface` | an `Interface` something `CONSUMES` that no Component `PROVIDES` — the two sides of a contract disagree |
| `unconsumed_interface` | an `Interface` a Component `PROVIDES` that nothing `CONSUMES` — a deliberate public contract, or a leftover |
| `undeclared_seam` ✅ | two Components `DEPENDS_ON` each other and **no `Interface` carries both a `PROVIDES` and a `CONSUMES` between them** — the seam exists in the build and is written down nowhere. The other direction from the two rows above, which both need an `Interface` to exist already, so a design that never declared one is invisible to them. The set is `maturity_report`'s `seams` band: it divides `declared` by `couplings` on every run and used to discard the difference (`req:an-undeclared-coupling-is-named-not-just-counted`). **It names the pair and asks — it never drafts the Interface**, because reflow2 can see *that* two parts are coupled and cannot know *what* crosses the boundary (`cap:no-fabricated-repair`). One project rollup keyed on the rule, not N alarms: reflow2's own design has 73, and `unexpected_coupling` was retired for exactly that flood. Silent when no two Components depend on each other — an absence, not a deficiency |
| `unrealized_capability` | a Capability marked designed has no `Artifact` `REALIZES`-ing it |
| `decomposition_coverage` ✅ | a Requirement has `DECOMPOSES` children and **nothing has ever asked whether they amount to it** (`req:decomposition-covers-its-parent`). Delivery rolls UP a decomposition — `report.rs` treats a parent as delivered exactly when every child is — so a requirement split into two children addressing a tenth of it reports `delivered` the moment both close, inside `req:completion-computed`, the number this project trusts *because* it is computed rather than asserted. THE MECHANISM IS GENERAL: a decomposition by SUBJECT drops what belongs to no single subject, because cross-cutting content has no natural child to land in. Measured instance (2026-07-28, reflow's `01-systems_engineering.json`): a monolith split into 01a–01f, with `context_management` and `self_improvement` present in all six originals and absent from all seven children, unnoticed for months. **It asks and never names what is missing** — reflow2 can see *that* the question is unanswered and cannot know *what* fell between the children, and a plausible wrong guess is worse than the question because it gets recorded as the answer (`cap:no-fabricated-repair`). Per-parent, not one rollup: the question is answerable only about one parent. Severity 0.50, or **0.70 once the parent already reports delivered**, where the risk has stopped being hypothetical. Keyed on the parent AND its children, so changing the split re-asks — the earlier answer was about *those* children. **Decomposition only, never derivation**: a DERIVED requirement adds new technical necessity and is not expected to cover anything (`req:requirement-lineage`), and keying on the edge gets that for free. Silent on a design that has never decomposed anything — including reflow2's own, which has zero `DECOMPOSES` edges, so the self-host cannot exercise this rule (`rule:the-self-host-always-trails-what-it-teaches`) |
| `change_axis_unstated` ✅ | a `ChangeEvent` carries no `subject` — nothing says whether the **SYSTEM changed** or whether **only the design's record of it did** (a re-sync, an accepted drift, a question finally settled). **THE FIRST DETECTOR WRITTEN DELIBERATELY AS THE THIRD LEG.** `fact:vocabulary-needs-three-legs-and-a-users-project-gets-none-of-it` measured that optional vocabulary reaches a user's design only with a TYPED TOOL + an INSTRUCTION + a DETECTOR THAT NOTICES ABSENCE, and that reflow2's detectors largely check the *consistency of what exists* — so optional fields stay empty in every project, forever, invisible to the loop that exists to surface gaps. **It must fire at ZERO usage, which is what `decomposition_coverage` structurally cannot do**: that one keys on a Requirement already carrying a `DECOMPOSES` edge, so a project that never decomposed anything reads clean with all three legs apparently present. This one keys on the population of ChangeEvents, so the design that has recorded changes and never stated an axis is the LOUDEST case rather than the silent one. **AGGREGATE**, for the same reason as `unvalidated_capability`, and more sharply: ChangeEvents are the fastest-growing node type in any active design, so per-event keying would expire the user's standing judgement on every single write. One acknowledgement settles the practice. Reports its denominator (`N of M`), because a numerator alone cannot distinguish a design that is drifting from one that has just started. Severity 0.30 — nothing here is *wrong*; the changes are recorded and findable. What is missing is a distinction nobody can reconstruct later, because only the person making the change knew it. Never inferred from `change_type`: the mapping is not total, and a `resync` is honestly either axis |
| `failing_verification` | a `Verification` with `status: failing` — reality contradicting the design, which no absence-shaped gap can say. Severity 0.8, above every absence gap: work proven broken outranks work not started. Anchored to the check *and* its targets. Note `build_without_verification` still closes when a check exists — the "how will you confirm?" question is answered; this gap is what fills the silence when the answer is "it doesn't" (BL-30) |
| `unverified_capability` | a realized Capability has no `Verification` |
| `status_contradiction` | a status making a claim the structure denies: Capability `verified` with no *passing* check, or Requirement `met` with nothing satisfying it — the latter otherwise invisible, since `met` silences `unsatisfied_requirement` by design. Severity 0.70 (self-contradiction: below reality-contradiction, above absence). Scoped to the unambiguous cases; `realized`-without-artifact is already an absence gap (BL-31) |
| ~~`unverified_artifact`~~ | **retired** (BL-23). Per-file coverage is counted by `graph_report`'s *Verification coverage* line, not asked as a gap: one `VERIFIES` edge per source file is bookkeeping nobody writes, and it was 22 of 25 gaps on reflow2's own design. The key string is kept because acknowledgement ids hash it |

### Structural gaps (shared signals with HEAL, but ASKED not fixed)
`orphan_node`, `dead_end`, `disconnected_cluster`, `single_point_of_failure` — surfaced
as "should these connect?" questions rather than auto-repaired.

### Quality / risk gaps
| gap_source | Fires when… |
|---|---|
| `dimension_blind_spot` | a central node has too few `DimensionAssessment`s (reuse `find_blind_spots`) |
| `unmitigated_risk` | a `RISKS` edge with no `MITIGATES` response |
| `unvalidated_capability` ✅ | a capability with a passing verification-kind check but no passing validation-kind check — built right, but the right thing? Reads `Verification.kind` (`dec:edge-orthogonality`). One project rollup, not N alarms |
| `unresolved_contradiction` | two nodes `CONTRADICTS` with no resolving `Decision` |
| `violated_constraint` | a `VIOLATES` edge on a Constraint/DesignRule with no remediation |
| `unvalidated_causal_claim` | a high-impact causal edge with `basis=correlational` + `validation_status=unvalidated` (chain_reflow: don't trust correlation as causation) |

### Compliance gaps (operating environment — from storyflow's cosmology)
| gap_source | Fires when… | Example question |
|---|---|---|
| `unchecked_compliance` | a design element in scope of a mandatory `EnvironmentRule` has neither `COMPLIES_WITH` nor `VIOLATES_RULE` | "Has the egress width been checked against the fire code?" |
| `open_violation` | a `VIOLATES_RULE` is still `proposed` (not triaged) | "This exceeds the occupancy limit — seek a variance or redesign?" |
| `no_operating_environment` | the Project has no `OPERATES_IN` Environment, so no ruleset applies yet | "Where will this operate? (Kennewick? Mars?) — its codes drive the design." |

### SME considerations (LLM-as-subject-matter-expert)
| gap_source | Fires when… | Example question |
|---|---|---|
| `sme_consideration` | the SME augmentation pass surfaced a consideration the user hasn't addressed (a proposed logistics constraint, risk, or missing capability) | "Building on Mars needs a supply/transport plan (launch mass budget, resupply cadence) — add these constraints?" |

SME considerations carry the grounding label (`verified`/`extrapolated`/`speculative`/`contradicts_known`) + `domain` so the user can weigh them; accepting one INGESTs it. See [sme-augmentation.md](sme-augmentation.md).

### Decomposition / hierarchy gaps (Axis Y — matryoshka, from chain_reflow)
| gap_source | Fires when… | Example question |
|---|---|---|
| `missing_intermediate_level` | a `CONTAINS`/`DEPENDS_ON` skips ≥2 `Component.level`s (the carburetor-to-body problem) | "`Carburetor` connects straight to `Body` — is there a missing `Engine` subsystem between them?" |
| `level_mismatch` | two linked components sit at incompatible levels for that edge | "These are wired peer-to-peer but one is a system and one a part — which level is wrong?" |
| `orphan_level` | a level exists with no parent above or children below it | "`Subsystem X` has no parent system — what contains it?" |

Adding a detector = one enum value + one `_detect_*` method, per storyflow's convention.

---

## Non-negotiable disciplines (scenarioRunner lessons — keep verbatim)

1. **Detectors read COMPUTED signals, not raw edge-name filters.** storyflow's biggest
   trap: a detector filtered on a *comment-alias* edge type that the real feed never
   carried → the detector was DEAD on live data while looking correct. Detect via graph
   algorithms/aggregate queries (centrality, components, type-population counts) over the
   actual schema, and **prove each detector fires on real data**.
2. **Rank by composite severity.** Surface the most important gap first; cap and page the
   rest. Users won't act on 40 undifferentiated prompts.
3. **Anchor in the user's own material.** "Earlier you specified `<Requirement>` …" beats
   an abstract "there is a missing verification." Concrete > abstract.
4. **Graceful degrade with an explicit flag.** If LLM rephrase or anchor resolution fails,
   fall back to the raw gap AND set `rephrase_degraded = true` — never silently ship an
   un-enhanced question as if it were polished, never drop the candidate.
5. **Never speak graph-jargon to the user.** Translate node/edge/score language into plain
   questions. No "orphan node with betweenness 0.0" — "this piece isn't connected to
   anything; is that intentional?"
6. **Deterministic gap ids + caching.** Hash(source + affected ids) so the same gap is
   stable across runs; cache the candidate set keyed by a graph-state hash with a short
   TTL (storyflow: 10 min) so re-opening the panel is instant but stays fresh after edits.
7. **Validate ids at the boundary.** storyflow validates `story_id` as a real UUID at the
   schema edge because it flows into cache-key/SCAN glob construction — a `*`/`?` in an id
   would cross-match other users' cache. Any id that reaches a key/pattern must be
   validated first (OWASP: injection via cache-key).
8. **Two modes.** *Retroactive* (gap-driven — "fix what's thin") and *proactive*
   (forward-looking — "you're at the design stage; here's what comes next"). The
   deploy/operate nudge is a proactive, phase-coverage prompt.
9. **Adjustable depth** (storyflow's "heat" 1-5): how thorough an answer to ask for — a
   quick one-liner vs. a full lifecycle plan.

---

## Reuse vs. build

| storyflow asset | plan |
|---|---|
| `GapDetector` + candidate cache + ranking | **reuse structure**; swap detectors |
| `ScenarioCandidate` / `ScenarioPrompt` shapes | **reuse verbatim**, re-typed |
| constructive-rephrase + `rephrase_degraded` degrade path | **reuse verbatim** |
| anchor resolution (ground in prior fragments) | **reuse** — dynograph text/vector search finds the anchoring Fragment |
| narrative detectors (arc pacing, foils, reveals) | **replace** with the design taxonomy above |
| analytics feeds (health/metrics/forces/…) | **replace** with dynograph-graph algorithms (components, centrality) + type-population queries |
