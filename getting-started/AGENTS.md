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

0a. **On an existing design, orient first.** Start with `open_questions` — anything there was
   already put to the user and is still waiting, so follow it up rather than asking again. Then,
   if the graph holds a Project, run the **where-am-i** skill: read the graph and tell the user what the design
   says, what has been decided, and what is still open. They cannot see the graph — this is the
   only way they learn what a previous session concluded. Do it again whenever they ask "where
   are we?".

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
     (Component→Interface) for **both** sides of every contract. Use stable ids: `req:…`,
     `cap:…`, `cmp:…`, `ifc:…`, `proj:…`.
   - For any other schema type, **call `describe_schema` first, then `create_node`/`create_edge`.**
     Ask `describe_schema {"from": "Release", "to": "Component"}` and it tells you which edge
     types may join those two — and, just as importantly, whether any of them actually *models*
     that pair or merely accepts it through a `*` wildcard. Never guess an edge type until one
     validates: several will, and validating is not the same as meaning what you intended. If
     nothing models the relationship, that is real information — leave the edge out rather than
     asserting one that is wrong.
   - Whenever two components talk to each other, model the Interface between them and record
     both sides. An unrecorded contract is invisible: change one component later and nothing
     will tell you the other one just broke.
2. **DETECT gaps and ask.** Run `detect_gaps`. For each gap, call `gap_to_prompt` to turn it
   into a plain question (see the handshake below), ask the **user**, then write their answer
   back as a Requirement or a node property. Do this **before** building. If the user judges a
   gap acceptable, record that with `acknowledge_gap` (+ their reason) so it moves to
   `reviewed_gaps` — the open list must keep meaning "still needs attention".
3. **Build only what the graph specifies, and link the files back.** Implement the
   capabilities/components the graph holds — nothing it doesn't. After creating each real file,
   register it with `link_artifact` (Artifact + provenance + `REALIZES` the capability it
   implements) **including a `checksum`**, so as-designed vs as-built stays honest and later
   edits are detectable; re-run `detect_gaps` and check that the capability you linked is no
   longer in an `unrealized_capability` gap's `affected_ids` — the *total* gap count will rise
   after the first link, because that detector switches on once the build phase starts.
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

- **Discover the vocabulary:** `describe_schema` — no arguments for every node and edge type,
  `{"node_type": "X"}` for one type's properties and the edges it can carry, or
  `{"from": "X", "to": "Y"}` for what may join them. Call it before writing anything unusual;
  it is cheaper than a guess and far cheaper than a wrong edge.
- **Detect / analyze:** `detect_gaps`, `propagate_change`, `propagate_from`, `graph_report`,
  `graph_report_markdown`, `detect_defects`, `propose_heal`, `evaluate_allocation`,
  `propose_allocation`, `hierarchy_issues`, `surprising_connections`, `dimension_drifts`,
  `dimension_drift`.
- **Decomposition:** `contain_component` nests one Component inside another (the assembly
  spine). Set `level` on `add_component` — `component` (default), `subsystem`, `system`,
  `system_of_systems`, `enterprise` — and nest one level at a time; `hierarchy_issues` compares
  the levels either side and will otherwise report every nesting as a mismatch.
- **Questions already asked:** `open_questions` returns the questions put to the user that still
  bear on something open, with the wording they saw. **Read it before `detect_gaps` at the start
  of a session.** Two kinds: `status: asked` — they have not replied, so follow it up rather than
  asking again; `status: answered` — they replied but the gap is still open, so either write their
  answer into the design or `acknowledge_gap` if they judged it fine as it stands. Their reply
  comes back with it. `answer_question` records what they said; `withdraw_question` retires one
  overtaken by events. `gap_to_prompt` records the question itself, so you do not have to.
- **Requirement lifecycle:** `set_requirement_status` — `proposed` / `accepted` / `deferred` /
  `dropped` / `met`. Use it when a requirement is provisional or abandoned instead of writing
  that into the statement text; `dropped` and `met` stop it being reported as unsatisfied.
- **Build:** `add_project`, `add_requirement`, `add_capability`, `add_component`,
  `contain_component`, `set_requirement_status`, `add_interface`, `satisfies`, `allocate`, `contains`, `provides`, `consumes`, `create_node`,
  `create_edge`, `get_node`, `scan_nodes`, `delete_node`, `apply_heal`.
- **As-built:** `link_artifact`, `add_artifact`, `realizes`, `reconcile_artifacts`,
  `set_artifact_checksum`.
- **Verify & operate:** `add_verification`, `verifies`, `set_verification_status`, `add_release`,
  `add_environment`, `add_resource`, `deploy_to`, `require_resource`.
- **Decisions:** `add_decision`, `governed_by` — record why a choice was made, not just what.
- **Change over time:** `add_epoch`, `add_change_event`, `record_change`.
- **Ask the user:** `gap_to_prompt`, `acknowledge_gap`, `reviewed_gaps`,
  `withdraw_gap_acknowledgement`.
- **Report back:** `graph_report`, `graph_report_markdown` — raw material for the
  **where-am-i** summary; rewrite it in the user's words rather than pasting it.

Tool results are the payload directly (no wrapper). Partial-success fields (`unknown_seeds`,
`skipped_operations`, `rephrase_degraded`, …) are always present — read them; nothing is
silently dropped.

## Why bother (don't skip the graph)

A stateless agent re-derives the design every session and decides silently over a scope bigger
than its memory. That's how "add wind" quietly breaks the render pipeline and the roster model.
reflow2 is the memory and the blast-radius map. Use it every time.
