# AGENTS.md — build this project with reflow2 as the design brain

You are the builder. **reflow2 is the persistent, coherent design brain you don't have** — it
outlives any context window and remembers the whole design (concept → operations). It is
reached through the **`reflow2` MCP server** (tools listed below). You write every line of
code; reflow2 decides *what* to build, keeps concept→product coherent, and tells you exactly
what a change breaks.

## The one rule

**Consult and update the design graph before you write or change code.** Never make a silent
design decision. If something is ambiguous ("realistic physics" → spin? wind? collision
fidelity?), that is a *gap* — surface it as a question, don't guess.

## The loop

1. **Capture intent.** When the user gives a brief or a new idea, extract it into the graph:
   - `add_requirement` (what must be true), `add_capability` (what the system does),
     `add_component` (what part owns it).
   - Link the golden thread: `satisfies` (Capability→Requirement), `allocate`
     (Capability→Component), `contains` (Project→child). Use `create_node`/`create_edge` for
     any other schema type. Use stable ids: `req:…`, `cap:…`, `cmp:…`, `proj:…`.
2. **DETECT gaps and ask.** Run `detect_gaps`. For each gap, call `gap_to_prompt` to turn it
   into a plain question (see the handshake below), ask the **user**, then write their answer
   back as a Requirement or a node property. Do this **before** building.
3. **Build only what the graph specifies.** Implement the capabilities/components the graph
   holds — nothing it doesn't. (Linking the real files back to the graph is coming — SP-6.)
4. **On ANY change or new idea, check impact first.** Record it with `add_change_event`, then
   `propagate_change` (or `propagate_from` for a speculative "what would this touch?"). Update
   **only** the impacted capabilities/components/tests the blast radius names — then re-run
   `detect_gaps` to confirm nothing rotted.
5. **Keep it healthy.** `graph_report` answers "what should I look at?". `detect_defects` →
   `propose_heal` → `apply_heal` fixes structure the machine can. `hierarchy_issues`,
   `surprising_connections`, `dimension_drifts` surface decomposition, coupling, and quality
   drift.

## The gap → question handshake (`gap_to_prompt`)

reflow2 phrases the question; **you** are the language model that fills it in:

1. Call `gap_to_prompt` with the `gap` (a `GapCandidate` from `detect_gaps`) and empty
   `answers`. It returns `{ "status": "needs_llm", "prompts": [{ "id", "prompt", … }] }`.
2. For each prompt, produce the answer text in-context (that's your job as the agent).
3. Call `gap_to_prompt` again with the **same** `gap` and `answers: [{ "id", "text" }]`. It
   returns `{ "status": "ok", "prompt": { "question", … } }` — the polished question to ask
   the user. If `rephrase_degraded` is true, the raw wording is used; ask it anyway.

## Tools (the `reflow2` MCP server)

- **Detect / analyze:** `detect_gaps`, `propagate_change`, `propagate_from`, `graph_report`,
  `graph_report_markdown`, `detect_defects`, `propose_heal`, `evaluate_allocation`,
  `propose_allocation`, `hierarchy_issues`, `surprising_connections`, `dimension_drifts`,
  `dimension_drift`.
- **Build:** `add_project`, `add_requirement`, `add_capability`, `add_component`, `satisfies`,
  `allocate`, `contains`, `create_node`, `create_edge`, `get_node`, `scan_nodes`,
  `delete_node`, `apply_heal`.
- **Change over time:** `add_epoch`, `add_change_event`, `record_change`.
- **Ask the user:** `gap_to_prompt`.

Tool results are the payload directly (no wrapper). Partial-success fields (`unknown_seeds`,
`skipped_operations`, `rephrase_degraded`, …) are always present — read them; nothing is
silently dropped.

## Why bother (don't skip the graph)

A stateless agent re-derives the design every session and decides silently over a scope bigger
than its memory. That's how "add wind" quietly breaks the render pipeline and the roster model.
reflow2 is the memory and the blast-radius map. Use it every time.
