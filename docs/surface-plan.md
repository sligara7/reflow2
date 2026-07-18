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

## Shared graph across two agents — DECIDED 2026-07-18: repo-file

The two candidate shapes were a **repo-file graph** (the RocksDB store lives beside the
project; agents sync through git — simple, offline, manual merge) and a **small shared
service** (a process both agents connect to over `/v1/*` — handles concurrency, but is
infrastructure to run).

**The repo-file shape is the decision.** It is not a deferral: the work that was waiting on
this fork (BL-15 published releases, BL-18 staleness check, BL-19 version stamp, BL-20
export/import) should now be built on the embedded assumption.

### Why, against the three arguments for a service

| Argument | Verdict |
|---|---|
| **Single-writer concurrency** (BL-12) | The strongest argument, and still hypothetical. Two people work this repo but only one writes the graph; the second is a consumer. It also fails *loud* — RocksDB's LOCK means a second server exits immediately, with no corruption and no split-brain. A real cost would be a real second writer, and there isn't one. |
| **Distribution** (BL-15) | Real and recurring — everything assumes a checkout and a ten-minute RocksDB build. But that is a *packaging* problem, and published per-platform binaries answer it at **M**. A service is **L** plus permanent operational cost. |
| **Migration** (BL-19/20) | Real. Export/import fixes it for the embedded shape too, and is already scoped as BL-20. A service would centralise it, but that is a side effect, not a reason. |

Two costs specific to the service also weigh against it here: it puts the user's design graph
on a machine they do not control — which sits badly beside the redaction discipline the
friction-reporting skill takes seriously — and it is ongoing operational work for a project
with one writer.

### What would reopen it

The core is surface-agnostic on purpose ([interaction-surfaces.md](interaction-surfaces.md)),
so nothing here forecloses a service. Build one when **any** of these becomes true:

1. **A second person actually writes to one graph** — not "might", but has tried and been
   blocked by taking turns.
2. **Someone wants reflow2 without a checkout, and published binaries have proved
   insufficient** — the packaging answer has to fail before the infrastructure answer is
   justified.
3. **A team rather than two people needs shared access**, at which point read-only secondaries
   stop being enough.

The lighter middle option stays available and unbuilt: **RocksDB secondary/read-only mode**, so
other agents can read a graph while one writes. Worth reaching for before a service if
"let me look while you work" turns out to be the actual need.

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

- ~~Shared-graph shape: repo-file vs service~~ — **decided 2026-07-18: repo-file** (above),
  with the conditions that would reopen it written down. Single-writer remains true and remains
  the strongest argument for a service; it stays hypothetical until a second writer exists.
- MCP tool granularity: one coarse "run the loop" tool vs many fine-grained ops the agent
  orchestrates? (Leaning fine-grained — the agent orchestrates, per the loop.)
- How the agent-native `LlmBackend` round-trips a "pass" (tool returns a prompt + schema; agent
  calls back with the filled JSON) — the exact handshake.
- GENESIS: how much to seed from a one-paragraph brief before the first DETECT round.
