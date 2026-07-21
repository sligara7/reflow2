# Interaction Surfaces & LLM Sourcing — the option analysis (decision since made)

> Part of the **Reflow 2.0** design docs — see **[overview.md](overview.md)** for the full map and reading order.
>
> **⚠️ Superseded on the decision, kept for the analysis.** When this was written, *how* a
> human drives reflow2 was deliberately left open. That decision was **made on 2026-07-18 —
> the agent-native MCP surface** — and built and shipped at v0.5.0. See
> **[surface-plan.md](surface-plan.md)** for the decision and build order, and
> `docs/requirements-coverage.md` (SP-1…SP-6) for what shipped. This doc remains a useful
> record of *why* agent-native won and what the alternatives cost; read it as history, not as
> an open question.

*How* a human drives Reflow 2.0 was intentionally left open when this was written. This doc
explains why that choice could wait, what the options were, and the one architectural
consequence it carries — the LLM-provider dependency — so the decision could be made late
without reworking the core.

## Principle: the core is surface-agnostic

Everything in `schema/` and the process docs describes a **core** that knows nothing about
how it's driven:

- the **graph store** (dynograph-foundation) + the **schema** (27 node types, 54 edges);
- the **coherence-loop operations**: extraction, impact propagation, gap-surfacing, heal.

A **surface** sits on top of the core and lets a human interact with it. Swapping surfaces
must not touch the core. So we build the core first and pick the surface last.

## Split the work: deterministic ops vs. LLM-reasoning ops

The key to keeping the core neutral is recognizing that the loop's work divides in two:

| Kind | Examples | Needs a model? |
|---|---|---|
| **Deterministic ops** | graph CRUD, impact-propagation BFS, fuzzy/vector entity resolution, schema validation, drift detection, compliance checks | **No** — pure code/queries |
| **LLM-reasoning ops** | extraction passes, SME augmentation, phrasing gap questions, resolution adjudication, heal proposals | **Yes** — needs an LLM |

Only the second group needs a model — and *which* model is the pluggable part.

## The LLM backend is pluggable (already)

dynograph-foundation's `dynograph-extract` crate defines a **pluggable `LlmBackend`
trait** (OpenAI / Anthropic / local / mock). Every LLM-reasoning op goes through that
abstraction, so the *same* core runs whether the model is the driving coding agent or an
external API. This is what makes the interaction decision safe to defer.

## Candidate surfaces (and the LLM-provider consequence)

| Surface | Who drives | Who is the LLM | External LLM API? | Best for |
|---|---|---|---|---|
| **MCP tools / agent skills** | a coding agent (Copilot CLI, Claude Code, …) | **the agent's own model** | **No** | developers already in an agent; lowest infra |
| **Hosted web app / service** (like [storyflow](https://github.com/sligara7/storyflow)) | its own frontend + backend | a server-side provider | **Yes** (OpenRouter / OpenAI / Anthropic / local) | non-developers, broad audience |
| **CLI / IDE extension** | the tool, agent-driven | the agent (or a bundled/local model) | usually No | terminal/IDE workflows |
| **Library / embedded API** | a host application | whatever the host wires up | host's choice | embedding reflow2 in another product |

### The secondary effect you noticed

- **Agent-native (MCP/skills):** the coding agent *is* the reasoning engine. The
  `LlmBackend` delegates to the ambient agent, the MCP tools do the deterministic graph
  work, and the extraction/SME/gap-surfacing "passes" become **skill instructions the
  agent executes in-context**. → **No OpenRouter or other external LLM API required.** This
  is the route [integrated_reflow](https://github.com/sligara7/integrated_reflow)'s MCP
  server took.
- **Hosted web app:** there's no ambient agent, so the service must supply its own model
  via an `LlmBackend` implementation — i.e. it **does** need a provider (OpenRouter or
  equivalent), with the attendant cost, keys, and rate limits. This is storyflow's model.

Same core, same processes — only the `LlmBackend` implementation and the surface differ.

## Implication for the build order

Because the surface plugs in last, the implementation plan is:

1. **Store + schema** — stand up dynograph-foundation with the 10 schema domains.
2. **Deterministic core** — graph CRUD, resolution, impact-propagation, validation,
   drift/compliance checks (no LLM).
3. **LLM-reasoning ops behind `LlmBackend`** — extraction, SME, gap-surfacing, heal, with
   a `mock` backend for tests.
4. **Surface (deferred)** — MCP/skills, hosted app, CLI, or library. Pick per audience;
   the LLM-provider question is answered *by* this choice, not before it.

## Recommendation framing (for the decision-maker)

- If the primary users are **developers working inside a coding agent** → MCP/skills:
  simplest, no external LLM bill, the agent brings the intelligence.
- If the goal is a **product for non-developers** → hosted web app: richer UX, but you own
  an LLM provider integration and its costs.
- You can support **both** over time — they share the entire core; only the thin surface
  and the `LlmBackend` wiring differ. Start with the audience you'll serve first.
