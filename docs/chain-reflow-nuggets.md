# Nuggets from Chain Reflow

Review of `~/git_projects/chain_reflow` — the project for **linking independently-built
architectures** together via touchpoint discovery, then running reflow analysis on the
merged whole. (Its modules `creative_linking`, `causality_analysis`, `matryoshka_analysis`,
`matrix_gap_detection` were later referenced by integrated_reflow's component index.)
Several ideas sharpen the redesign — especially Axis Y and the inference layer.

## Adoption table

| Nugget | Value for the vision | Map to redesign | When |
|---|---|---|---|
| **Matryoshka / missing-intermediate detection** (the "carburetor-to-body problem") | a gap is often a *missing hierarchy level*, not a missing peer link — a precise, high-value gap type | new gap-surfacing detectors + formalize Axis-Y levels | **adopt (now)** |
| **Correlation vs. causation** analysis (competing hypotheses A→B / B→A / A↔B / spurious + validation experiments) | rigor on the "why" layer — don't assert unproven causation | inference edges carry `basis` + `validation_status`; a causality-check capability | **adopt (now, schema)** |
| **Creative linking** (synesthetic / structural-analogy bridges for orthogonal domains, guarded) | bridge orthogonal subsystems + a "design anything" cross-domain feature | a *guarded* generative healer + cross-domain touchpoint suggestions | **adopt (concept)** |
| **System-of-systems touchpoint discovery** (link independently-developed subgraphs) | acquisitions programs *are* systems-of-systems — teams design parts independently, then integrate | Project↔Project linking; touchpoint discovery emits cross-project `Interface`/`DEPENDS_ON` | **adopt (medium)** |
| **Orthogonality assessment** (how related are two architectures → pick strategy) | choose standard vs. creative linking automatically | a scored input to touchpoint discovery | **later** |
| **Interactive executor** (`interactive_executor.py`) | a non-LLM workflow-runner option | note for when we build the runtime | **later** |

## The big one: matryoshka (Axis-Y depth) and the missing-intermediate insight

Chain Reflow's sharpest idea. Architectures nest hierarchically —
**component → subsystem → system → system-of-systems → enterprise** — not just peer-to-peer.
The revelation:

> When two things seem unrelated, the gap is often a **missing intermediate level**, not a
> missing peer link.

The *carburetor-to-body problem*: don't link a Carburetor (component) directly to the Body
(system) — the real gap is the missing **Engine System** that should contain the
carburetor and sit peer-to the body. And any apparent gap might actually be:

- **missing documentation** (the intermediate exists but isn't captured),
- **missing design** (it should exist but doesn't),
- **wrong hierarchy level** (metadata is off), or
- **the actual integration point** (where systems connect).

This gives Axis Y real teeth. Two concrete folds (applied):

1. **Formalized Y levels** on `Component` (`component / subsystem / system / system_of_systems / enterprise`) so the spine is explicit.
2. **New gap-surfacing detectors** (Decomposition group): `missing_intermediate_level`,
   `level_mismatch` (a `DEPENDS_ON`/`CONTAINS` that skips ≥2 levels), and `cross_level_link`
   — surfaced as questions ("Carburetor connects straight to Body — is there a missing
   Engine subsystem between them?").

## Correlation vs. causation — rigor for the inference layer

Chain Reflow refuses to assert causation from correlation. For each observed correlation it
generates competing hypotheses (A→B, B→A, A↔B, spurious) and designs validation experiments
(observational, intervention, mechanism, temporal). We adopt the discipline: causal-category
inference edges (`CAUSES`, `ENABLES`, `BLOCKS`, `TRIGGERS`) now carry:

- `basis ∈ {observed, correlational, causal, spurious}` — how strongly the link is grounded,
- `validation_status ∈ {unvalidated, hypothesis, validated, refuted}`,

so the graph distinguishes "these move together" from "this proves that," and gap-surfacing
can ask the user to validate high-impact but unvalidated causal claims.

## Creative linking — a guarded healer

For genuinely orthogonal parts (e.g. a biological pathway and a software pipeline), Chain
Reflow finds cross-domain bridges by metaphor/structural analogy — but *only* with user
consent, *always* marked exploratory, *requiring* validation. We fold this into HEAL as a
**guarded creative-bridge healer**: it may *propose* a bridge `Interface`/`DEPENDS_ON`
across orthogonal subsystems, always with `requires_human_review = true` and a speculative
provenance, never auto-applied. This is the disciplined version of "close this gap."

## System-of-systems linking

Our schema models one Project. Chain Reflow's premise — independently-developed subgraphs
linked into a whole — is exactly an acquisition **system-of-systems**. Fold: allow
Project↔Project links, and a touchpoint-discovery pass that proposes cross-project
`Interface`/`DEPENDS_ON` edges (standard technical matching first; creative linking only for
orthogonal, guarded). The merged graph is then diagnosed/healed like any other.

## What we already do better

- **Store & merge**: dynograph is one live multi-graph store with real entity resolution;
  Chain Reflow merged JSON files with bespoke tooling.
- **Gaps**: `matrix_gap_detection` is superseded by gap-surfacing; we keep its matryoshka
  and cross-domain ideas as detectors.
