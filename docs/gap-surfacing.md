# Gap Surfacing — DIAGNOSE → PROMPT (the scenarioRunner, for design)

Adapted from storyflow's **scenarioRunner** (`generation_plus/src/services/scenario_gaps.py`,
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
| `unallocated_capability` | a Capability is not `ALLOCATED_TO` any Component |
| `interfaceless_dependency` | two Components `DEPENDS_ON` each other with no `Interface` between them |
| `unrealized_capability` | a Capability marked designed has no `Artifact` `REALIZES`-ing it |
| `unverified_capability` | a realized Capability/Artifact has no `Verification` |

### Structural gaps (shared signals with HEAL, but ASKED not fixed)
`orphan_node`, `dead_end`, `disconnected_cluster`, `single_point_of_failure` — surfaced
as "should these connect?" questions rather than auto-repaired.

### Quality / risk gaps
| gap_source | Fires when… |
|---|---|
| `dimension_blind_spot` | a central node has too few `DimensionAssessment`s (reuse `find_blind_spots`) |
| `unmitigated_risk` | a `RISKS` edge with no `MITIGATES` response |
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
