# Operating Environment & Ruleset — the design's "physics"

> Part of the **Reflow 2.0** design docs — see **[overview.md](overview.md)** for the full map and reading order.

Adapted from storyflow's **Universe / CosmologyConcept** model
(`storyflow/services/dynograph/schemas/domains/core.yaml`,
`modules/diagnose/law_consistency.py`). A design never exists in a vacuum — it must
function inside a real-world **operating environment** that imposes an authoritative
rule-set the design *must* comply with.

## The idea

> A house in **Kennewick, WA** must meet city ordinances, county rules, WA state
> requirements, building & fire codes, seismic provisions, and occupancy limits.
> The *same* house on **Mars** faces a completely different rule-set: 0.38 g, no
> breathable atmosphere, radiation shielding, pressure vessels, life-support. Change
> the environment → change the entire constraint space.

storyflow calls this a **Universe** with **cosmology laws** its stories are grounded
against. In reflow2 it's the **Environment** and its **EnvironmentRules**.

| storyflow | reflow2 |
|---|---|
| `Universe` (scopes stories, shares rules) | `Environment` (the operating context a project functions in) |
| `CosmologyConcept` (a universe law) | `EnvironmentRule` (a code / regulation / physical law) |
| `HAS_LAW` (Universe → law) | `IMPOSES` (Environment → EnvironmentRule) |
| generation "grounds HARD against laws" | environment-aware extraction + compliance check |
| `VIOLATES` (Fragment/Event → law, lifecycle) | `VIOLATES_RULE` (design node → rule, lifecycle) |

## Three kinds of rule — keep them distinct

| Node | Origin | Example |
|---|---|---|
| `Constraint` | **self-imposed** by the project | "budget under $500k", "finish by Q3" |
| `DesignRule` | **chosen** convention/standard the design adopts | "use timber frame", "REST over gRPC" |
| `EnvironmentRule` | **externally imposed** by the operating environment/authority | "IBC seismic category D", "Mars: maintain 1 atm internal pressure" |

Only `EnvironmentRule` is dictated by the *world*; the design cannot negotiate it, only
comply, seek a variance, or fail. This is exactly storyflow's HARD cosmology law.

## How it flows through the system

- **Scope**: `Project OPERATES_IN Environment`; `Environment IMPOSES EnvironmentRule[]`.
- **Extraction (environment-aware)**: the applicable ruleset is threaded into the
  relevant passes (as storyflow threads `universe_context`), so the design is captured
  *against* its environment. A **compliance pass** emits `VIOLATES_RULE` (status
  `proposed`) for anything that contradicts a mandatory rule — **flag-don't-drop**, even
  if the rule node can't be resolved (surface it anyway, per principle #2).
- **Gap-surfacing** (new detectors):
  - `unchecked_compliance` — a design element in scope of a mandatory rule with no
    `COMPLIES_WITH` and no `VIOLATES_RULE` ("has the egress width been checked against
    the fire code?");
  - `open_violation` — a `VIOLATES_RULE` still `proposed` ("this exceeds the occupancy
    limit — seek a variance or redesign?").
- **Heal**: compliance is human-triaged — `confirmed` = an accepted **variance/waiver**
  (kept, documented); `rejected` = **must fix** (retained for audit). HEAL may *propose*
  a fix but never auto-grants a variance.
- **Impact / coherence loop**: the environment is **swappable**, and swapping it is a
  first-class `ChangeEvent{change_type: environment_change}`. Retarget Kennewick → Mars
  and impact-propagation re-evaluates every `COMPLIES_WITH`: rules that no longer apply
  are dropped, and every new Martian `EnvironmentRule` with no compliance surfaces as an
  `unchecked_compliance` gap. This is the vision's phase-coherence, applied to the
  *environment* dimension.

## Why this matters for "design anything"

The operating environment is *the* thing that makes "design a house" concrete. Without it,
"design a house" is unanswerable; with it, the graph knows the real constraint space and
can guide the user, check compliance, and re-check it the moment the environment changes.
It is the design-world analogue of storyflow's "everything obeys the universe's physics."

## Reuse vs. build

| storyflow asset | plan |
|---|---|
| Universe/CosmologyConcept/HAS_LAW/VIOLATES schema | **re-key** to Environment/EnvironmentRule/IMPOSES/VIOLATES_RULE |
| VIOLATES lifecycle (proposed/confirmed/rejected, flag-don't-drop) | **reuse verbatim** |
| cosmology-aware extraction (`universe_context` threading) | **re-key** to environment-aware extraction |
| `law_consistency` diagnostic | **re-key** to the compliance detectors above |
