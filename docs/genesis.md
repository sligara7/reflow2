# GENESIS — bootstrapping the graph from a brief

> Part of the **Reflow 2.0** design docs — see **[overview.md](overview.md)** for the map and
> reading order. GENESIS is one of the six universal processes; it maps to the loop step that
> **seeds the first CHANGE**. Built in SP-5 (see [surface-plan.md](surface-plan.md)).

GENESIS is the coherence loop's **front door**: it turns a one-paragraph opening brief into a
seeded design graph, so the loop has something to work with and the first DETECT round is
productive. Without it, every design starts from an empty graph.

## Shape: a thin deterministic core op + an agent-native skill

GENESIS is deliberately split, consistent with the agent-native surface decision:

- **Core op** (`crates/reflow2-core/src/genesis.rs`, `DesignGraph::genesis`) — the deterministic
  half. It guarantees the invariants and does **no LLM work**: it creates the Project scaffold,
  plants the genesis Epoch (the axis-Z anchor), and returns a `GenesisReport` with a next-steps
  checklist. Exposed as the `genesis` MCP tool.
- **Skill** (`getting-started/.grok/skills/genesis/`) — the LLM half, run agent-natively. The
  ambient agent expands the brief into Requirements/Capabilities through the existing write tools
  (`add_requirement`, `add_capability`, `satisfies`, …) and drives the first DETECT round.

This keeps brief-expansion (LLM reasoning) on the agent side and off the deterministic core —
the same discipline that deferred `ingest`'s programmatic extraction (SP-3b) — so GENESIS needs
no LLM backend and no schema changes.

## Guarded and idempotent

GENESIS is a one-shot, high-stakes process (it lays the foundation). Like the predecessor's
`ir2-init`, it **refuses to clobber** an already-initialized graph: if a Project already exists
and `rescan` is not set, `genesis` is a no-op that reports `already_initialized: true` (never a
silent overwrite). This guard is what makes it safe to expose as an MCP tool rather than hiding
it behind a CLI.

## How much to seed (the resolved open question)

surface-plan.md asked "how much to seed from a one-paragraph brief before the first DETECT
round." The answer is driven by DETECT's own phase-coverage logic:

- Seed **P0 (Requirements) and P1 (Capabilities + `satisfies`)** — **stop before P2
  (Components)**.
- With Requirements + Capabilities present but no Components, DETECT's `detect_phase_coverage`
  (`crates/reflow2-core/src/detect.rs`) fires **`concept_without_design`** (severity 0.70) as the
  first gap — exactly the productive next question, *"how should this be structured?"*, to answer
  *with the user* rather than by guessing.

The core op plants only the Project + Epoch; the **skill** performs the P0/P1 seeding and stops
at the right depth.

## Deployment is a first-class requirement

A hard-won lesson from building reflow2 itself: *how* and *where* a design will run (target
platform, the driving agent, invocation, persistence) is a requirement, and if it isn't captured
early it surfaces late and forces rework. So the GENESIS skill explicitly captures
deployment/consumer context as `Requirement` nodes during bootstrap, and the `GenesisReport`'s
checklist reinforces it.

## Inputs / outputs

- **Input:** a brief's framing → `GenesisOptions { project_id, name, domain?, objective?, mode?,
  rescan? }`. `mode` is `flexible` (design evolves with the build) or `rigid` (design is the
  source of truth).
- **Output:** `GenesisReport { project_id, epoch_id, already_initialized, created, project_mode,
  next_steps }`, plus the seeded Project and genesis Epoch in the graph.

## Not here

- Brief extraction in core (that's the skill's job; the mutating-extraction/rollback path is
  SP-3b's `ingest`).
- A dedicated `ChangeType::Genesis` — the Baseline genesis Epoch is the temporal anchor; no
  schema change was needed.
