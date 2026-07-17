# SME Augmentation — the LLM as subject-matter expert

Adapted from storyflow's **supplementary analysis** (`modules/extraction/
graph_informed_prompts.py` PART 4, `supplementary_fragment.py`, `GroundedClaim`). This is
the capability that most directly serves the vision's premise — *the user may know very
little about systems engineering* — so the system must **proactively supply the expertise
the user doesn't have**.

## The idea

storyflow's analogy: extracting a courtroom witness statement, the LLM can also act as the
**subject-matter expert** that *supports or counters* the statement with domain knowledge.
For design: when the user describes a concept, the LLM — beyond faithfully capturing what
they said — surfaces the **additional considerations they didn't think of**.

> "You want to build a house on **Mars**. Beyond the structure itself, here's what you'll
> need to consider: getting supplies there (launch windows, payload mass budget), the
> build method (teleoperated robots vs. suited crew), in-situ resource use, a
> return/resupply cadence, radiation and dust mitigation during construction…" — dozens to
> thousands of **logistics** constraints the user never stated.

storyflow calls it *creative amplification, NOT fact-checking*: show where the design
aligns with reality, plausibly extends it, or diverges.

## Is "logistics" its own category, or part of deployment?

Neither exactly — it's **broader than deployment**. In systems engineering this is
**Integrated Logistics Support (ILS)**: it spans production, transport, deployment,
operation, maintenance, and disposal. So logistics is a **cross-cutting concern**, not a
phase. In the schema it doesn't need its own node type — it *decomposes* into what we
already have:

| Logistics consideration | Modeled as |
|---|---|
| get supplies to Mars | `Resource` + `Constraint(concern: logistics)` |
| build method (robots vs. suited crew) | `Decision` + `Capability` + `Component` |
| launch from Earth / transit | `Flow` + `Capability` + `Resource` |
| resupply & maintenance cadence | `Requirement(concern: sustainment)` + `Verification` |
| imposed by the destination | `EnvironmentRule` (Mars) → drives the above |

To keep these filterable, `Requirement` and `Constraint` now carry a **`concern` facet**
(`core / logistics / sustainment / safety / security / cost / schedule / manufacturability
/ usability / environmental`). "Show me all the logistics constraints" is then a query.

## How the SME augmentation works

A gated **SME pass** runs after faithful extraction (mirrors storyflow's Part 4):

1. The LLM adopts the relevant expert lenses (structural, thermal, propulsion, regulatory,
   logistics, safety, …) for the project's domain and **operating `Environment`**.
2. It surfaces considerations the user didn't state — proposed `Requirement`s,
   `Constraint`s, `Risk`s (RISKS edges), missing `Capability`s, `EnvironmentRule`s, and
   `Decision`s to make.
3. Each proposal is labeled on the **grounding spectrum** (from storyflow):
   - `verified` — established engineering/physics/regulation,
   - `extrapolated` — plausible extension from known practice,
   - `speculative` — reasonable but ungrounded,
   - `contradicts_known` — flags a stated design choice that violates known facts
     (neutrally — the user may intend it).
   plus `domain`, `evidence`, `confidence`, and the entities it relates to.

## Disciplines (keep the user in control)

1. **Amplify, don't fabricate silently.** SME output is *proposals*, never auto-merged as
   fact. It lands as a supplementary Fragment (`provenance: inferred`) linked to the source
   via **`SUPPLEMENTS`** (carrying `grounding` + `domain`), and surfaces to the user as
   **gap-surfacing questions** (`sme_consideration`) to accept, edit, or dismiss.
   Accepting INGESTs it as normal nodes — closing the loop.
2. **Distinct provenance, always visible.** SME-suggested content is tagged `inferred` and
   its grounding is queryable, so the user always knows what *they* said vs. what the SME
   *proposed*. (Same integrity bar as the extraction no-silent-fallback rule.)
3. **Cross-domain by design.** The point is to cover expertise the user lacks — the SME
   reaches outside the user's likely field (a software person designing a habitat gets the
   structural/life-support considerations).
4. **Grounded, not noise.** Only substantive considerations; aim for a focused set, ranked
   by impact × confidence, not an exhaustive dump.

## Why it's central to the vision

The whole promise is that *someone with only an idea* can be guided from concept to
operations without knowing systems engineering. SME augmentation is the mechanism: it turns
the LLM into the systems engineer, logistician, and code expert sitting beside the user —
surfacing the thousands of considerations (like Mars logistics) that separate a napkin
sketch from a buildable design, and feeding them into the same coherence loop as everything
else.

## Reuse vs. re-key

| storyflow asset | plan |
|---|---|
| supplementary-analysis prompt (Part 4) + grounding spectrum | **re-key** to a systems-engineering / domain SME pass |
| `GroundedClaim` (claim/grounding/domain/evidence/confidence/related_entities) | **reuse** shape for SME proposals |
| supplementary Fragment + `SUPPLEMENTS` edge, provenance=llm_analysis | **reuse** as `SUPPLEMENTS` + `provenance: inferred` |
| "author needn't be an expert in everything" | **the core rationale** for the whole feature |
