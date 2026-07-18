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

0. **Bootstrap once (GENESIS).** On a brand-new project (empty graph), start with the **genesis**
   skill: call the `genesis` tool to scaffold the Project + temporal anchor, seed the opening
   brief into Requirements + Capabilities (P0/P1, *not* Components), capture deployment/platform
   context as Requirements, then run `detect_gaps`. Skip this on an existing design.
1. **Capture intent.** When the user gives a brief or a new idea, extract it into the graph:
   - `add_requirement` (what must be true), `add_capability` (what the system does),
     `add_component` (what part owns it), `add_interface` (the contract where two components
     meet — an API, event, data feed, save format, physical or human connection point).
   - Link the golden thread: `satisfies` (Capability→Requirement), `allocate`
     (Capability→Component), `contains` (Project→child), and `provides`/`consumes`
     (Component→Interface) for **both** sides of every contract. Use `create_node`/`create_edge`
     for any other schema type. Use stable ids: `req:…`, `cap:…`, `cmp:…`, `ifc:…`, `proj:…`.
   - Whenever two components talk to each other, model the Interface between them and record
     both sides. An unrecorded contract is invisible: change one component later and nothing
     will tell you the other one just broke.
2. **DETECT gaps and ask.** Run `detect_gaps`. For each gap, call `gap_to_prompt` to turn it
   into a plain question (see the handshake below), ask the **user**, then write their answer
   back as a Requirement or a node property. Do this **before** building.
3. **Build only what the graph specifies, and link the files back.** Implement the
   capabilities/components the graph holds — nothing it doesn't. After creating each real file,
   register it with `link_artifact` (Artifact + provenance + `REALIZES` the capability it
   implements) **including a `checksum`**, so as-designed vs as-built stays honest and later
   edits are detectable; re-run `detect_gaps` to confirm the `unrealized_capability` gap closed.
   When you return to a project or suspect files changed outside the loop, hash them and call
   `reconcile_artifacts` — its `propagation_seeds` walk the change back up to the Capability and
   Requirement behind it. (See the **link-artifacts** skill.)
4. **On ANY change or new idea, check impact first.** Record it with `add_change_event`, then
   `propagate_change` (or `propagate_from` for a speculative "what would this touch?"). Update
   **only** the impacted capabilities/components/tests the blast radius names — then re-run
   `detect_gaps` to confirm nothing rotted.
5. **Keep it healthy.** After any structural change, and before a build push, run the
   **check-health** skill: `detect_defects` → `propose_heal` → `apply_heal`. It finds defects in
   the design's *shape* rather than its meaning — circular dependencies, single points of
   failure, disconnected clusters, duplicates. Only `duplicate` is machine-fixable; everything
   else is a design decision `propose_heal` leaves in `generated_content` for the user, so read
   `requires_human_review` and `skipped_operations` before acting. `graph_report` answers "what
   should I look at?"; `hierarchy_issues`, `surprising_connections`, `dimension_drifts` surface
   decomposition, coupling, and quality drift.

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
- **Build:** `add_project`, `add_requirement`, `add_capability`, `add_component`,
  `add_interface`, `satisfies`, `allocate`, `contains`, `provides`, `consumes`, `create_node`,
  `create_edge`, `get_node`, `scan_nodes`, `delete_node`, `apply_heal`.
- **As-built:** `link_artifact`, `add_artifact`, `realizes`, `reconcile_artifacts`,
  `set_artifact_checksum`.
- **Change over time:** `add_epoch`, `add_change_event`, `record_change`.
- **Ask the user:** `gap_to_prompt`.

Tool results are the payload directly (no wrapper). Partial-success fields (`unknown_seeds`,
`skipped_operations`, `rephrase_degraded`, …) are always present — read them; nothing is
silently dropped.

## Why bother (don't skip the graph)

A stateless agent re-derives the design every session and decides silently over a scope bigger
than its memory. That's how "add wind" quietly breaks the render pipeline and the roster model.
reflow2 is the memory and the blast-radius map. Use it every time.
