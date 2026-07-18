# Surface Plan — how a coding agent (grok build / claude code) uses reflow2

> Part of the **Reflow 2.0** design docs — see **[overview.md](overview.md)** for the full map and reading order.

> **Status: decision + plan for the next build phase.** The deterministic core is built and
> tested; this doc records the deliberately-deferred **interaction-surface** decision
> ([interaction-surfaces.md](interaction-surfaces.md)) now that a concrete use case forces it,
> and lays out what has to exist for a coding agent to drive reflow2 end-to-end. No surface
> code exists yet.

## The trial use case (what forced the decision)

Two people, two agents, **one shared design**:

- **grok build** (uses `AGENTS.md`) — the primary builder.
- **claude code** — the second agent on the same design.
- **The project:** a **Unity softball video game** for the author's brother's daughters —
  realistic physics, imported real-player images, and (his words) "a million ideas" that
  arrive over time.

## Why this beats "just ask an agent to build a softball game"

The critique is fair: you *can* tell grok "build a softball game with real physics in Unity"
and it will attempt it. The point of reflow2 is **not** that it writes better Unity code — the
agent still writes every line. reflow2 is the **durable, coherent design brain** the agent
lacks:

1. **A game exceeds any context window.** The agent can't hold the whole design; session 2
   forgets session 1. reflow2 is the persistent memory of the entire design (concept →
   operations) that outlives any context window.
2. **The agent decides silently.** "Real physics" → spin/Magnus? bat-ball collision fidelity?
   wind? The agent guesses and moves on. reflow2's **DETECT** turns those into explicit
   questions *before* code is written, and the answers become traced Requirements.
3. **The killer feature = the "million ideas" problem.** Every new idea is where a stateless
   agent breaks things. reflow2's **PROPAGATE** answers "if I add wind, what does it touch?"
   → exactly these capabilities/components/tests — so the agent updates only those and reflow2
   confirms nothing else rotted.
4. **The golden thread.** "Import real player images" ripples into the roster model, the
   render pipeline, and a *licensing constraint*. reflow2 holds those links; the raw agent
   re-derives them (or misses them) every time.

One line: **the raw agent is stateless and makes silent decisions over a scope bigger than its
memory; reflow2 is the persistent design brain that surfaces the decisions, keeps concept→game
coherent, and tells the agent precisely what to build and what a change breaks.**

## How the actual artifacts (the game) get produced

```
brother's ideas ─► GENESIS / INGEST ─► the design graph (reflow2, persisted)
                                          │  DETECT gaps → agent asks him → he answers → re-INGEST
                                          ▼
                        SYNTHESIZE a precise, traceable build brief
                                          ▼
                grok build / claude code writes the Unity project  ◄── the real artifacts
                                          ▼
              Artifact / Fragment nodes link the real files back (REALIZES); provenance stamped
                                          ▼
                   VERIFY (as-designed vs as-built) ─► idea #547 ─► CHANGE → PROPAGATE → re-heal
```

**reflow2 never writes Unity/C#.** The agent does. reflow2 decides *what* to build, in what
order, keeps it coherent, and tracks which real files realize which capabilities
([artifact-linking.md](artifact-linking.md)) so as-designed vs as-built stays honest.

## The decision: an **agent-native** surface

Per [interaction-surfaces.md](interaction-surfaces.md), the consumers are coding agents that
*are* the reasoning engine — so:

- **Surface = MCP tools / agent skill** (not a hosted web app). grok build and claude code
  call reflow2's operations as tools.
- **`LlmBackend` = the ambient agent.** The LLM-reasoning ops (extraction/INGEST passes, SME,
  gap-question phrasing, generative HEAL content) become **skill instructions the agent
  executes in-context** and hands back as structured JSON the tools ingest — through the same
  `LlmBackend` seam already built, implemented as "ask the ambient agent." **No external LLM
  provider, no OpenRouter bill** (IS-6).
- **Persistence = RocksDB.** The design must survive across sessions, so flip on
  `dynograph-storage`'s `rocksdb` feature; the in-memory backend is dev/test only.

## Shared graph across two agents (open design point)

grok build and claude code work the *same* design. So reflow2 must be a **shared** graph, not
a per-session one. Two candidate shapes, to decide next session:

- **Repo-file graph** — the RocksDB store (or an exported graph file) lives in the project
  repo; both agents sync via git. Simple, offline, but concurrent-edit merge is manual.
- **Small shared service** — a `dynograph-service`-style process both agents connect to over
  `/v1/*`. Handles concurrency, but is infrastructure to run.

Recommendation to weigh: start with the **repo-file** shape (matches the git-based dev flow;
lowest infra) and move to a service only if concurrent editing demands it.

## What must be built (next-phase build order)

1. **Persistence** — `DesignGraph::open_rocksdb(path)` (feature-gated), so the graph is a
   durable on-disk store. Small, unblocks everything.
2. **`LlmBackend` = ambient-agent adapter** — the backend that returns the prompt to the
   calling agent and takes back its structured answer (the in-context "pass"). This is what
   turns every deferred LLM-reasoning op (INGEST content, SME, gap PROMPT, generative HEAL)
   live without a provider.
3. **The surface layer** — an MCP server (or CLI) exposing the core ops as tools: `ingest`,
   `detect_gaps` / `to_prompt`, `propagate_change`, `propose_heal` / `apply_heal`,
   `evaluate_allocation` / `propose_allocation`, `graph_report`, `hierarchy_issues`,
   `surprising_connections`, dimension drift.
4. **A consumer `AGENTS.md` / skill** for the softball repo — teaches grok the loop: *ingest
   intent → DETECT → ask the user the gaps → build only what the graph specifies → link
   artifacts back → on any change, PROPAGATE before touching code.* (Distinct from **this**
   repo's AGENTS.md, which is about developing reflow2 itself.)
5. **GENESIS** — bootstrap the graph from the opening brief (the one universal process not yet
   built).
6. **Artifact linking wiring** — connect `Artifact`/`Fragment` nodes to the real Unity files
   the agent produces (`REALIZES`, provenance) so the loop closes on real code.

**Status: steps 1–6 all complete** (see [requirements-coverage.md](requirements-coverage.md),
SP-1…SP-6). Future improvements, tracked there: **SP-3b** (`ingest` programmatic LLM extraction
with a transactional prepare pass) and **SP-6b** (as-built drift detection / filesystem
reconcile + `DriftEvent`).

Deferred still: real hosted LLM providers (not needed for agent-native), the optional
embedding seam (semantic dedup/retrieval), generative-HEAL *content*.

## Open questions for the next session

- Shared-graph shape: repo-file vs service (above).
- MCP tool granularity: one coarse "run the loop" tool vs many fine-grained ops the agent
  orchestrates? (Leaning fine-grained — the agent orchestrates, per the loop.)
- How the agent-native `LlmBackend` round-trips a "pass" (tool returns a prompt + schema; agent
  calls back with the filled JSON) — the exact handshake.
- GENESIS: how much to seed from a one-paragraph brief before the first DETECT round.
