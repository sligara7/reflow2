---
name: check-health
description: Use after any structural change to the design (new components, new contracts, a resync after impact) and periodically before a build push. Runs reflow2's HEAL detectors to find structural defects the design can't see in itself — circular dependencies, single points of failure, duplicates, disconnected clusters — then applies only the mechanical fixes and brings the rest to the user. Distinct from detect-and-ask: that asks what the design *means*, this checks how the design is *shaped*.
---

# Check the design's structural health

`detect-and-ask` finds gaps in *meaning* — things the design never said. This finds defects in
*shape* — things the design says that don't hold together. Both are needed; neither substitutes
for the other.

Run this **after any structural change** (a new component, a new contract, edits following an
`impact-check`) and before a significant build push.

## 1. Look

Call `detect_defects`. It returns `HealIssue`s with `category`, `severity`
(`critical`/`warning`/`info`), a `message`, `affected_ids`, and a `suggested_fix_type`.

What the categories mean, and what to do about each:

| Category | What it means | Who resolves it |
|---|---|---|
| `circular_dependency` | parts that depend on each other in a loop — directly, or through the contracts they provide and consume. The `message` shows the loop as `a → b → c → a` | **user** — see step 4 |
| `single_point_of_failure` | every path between subsystems routes through one part | user |
| `disconnected_community` | a cluster with no link to the rest of the design | user |
| `dead_end` | a component connected to nothing at all | user |
| `orphan_node` | a Capability allocated nowhere, an Artifact realizing nothing, a Requirement satisfied by nothing | user |
| `contradiction` | two nodes joined by `CONTRADICTS` with no resolving Decision | user |
| `unresolved_setup` | an `ANTICIPATES` with no follow-through — a planned need never built | user |
| `duplicate` | two nodes marked `DUPLICATES` | **machine** — merged by `apply_heal` |

Also worth a look, and read the same way: `hierarchy_issues` (decomposition — a level skipped or
mismatched), `surprising_connections` (coupling that crosses otherwise-distant parts of the
design), `dimension_drifts` (quality trending down over time), and `graph_report` for the
overall picture.

If `detect_defects` returns nothing, the design's shape is sound — say so and move on.

## 2. Propose (this never changes anything)

Call `propose_heal`. Optional `strategy`: `conservative` (critical only), `balanced` (default —
critical + warning), `aggressive` (everything). Optional `max_operations` to cap the plan.

Read the whole proposal, not just the operations:

- `operations` — mechanical graph edits. Today the only one is merging a `duplicate`.
- `generated_content` — defects whose fix needs *judgement*, left deliberately unwritten. Each
  says what would need to be decided. **This is most of them.**
- `skipped_operations` — anything dropped, with a reason (a cap hit, an endpoint that doesn't
  resolve). Never ignore this list; nothing is dropped silently, so a non-empty list means
  something real was set aside.
- `requires_human_review` — true whenever `generated_content` is non-empty.

## 3. Apply only the mechanical part

If there are `operations` and `requires_human_review` is false, call `apply_heal` with the
proposal. It applies atomically and then re-checks its own work.

**Pass the proposal back exactly as you received it.** Every operation is checked against what HEAL
proposes for the graph as it stands, and anything else is refused before a single write. So do not
hand-edit a proposal, do not assemble one yourself, and do not reuse one from earlier in the session
if the graph has changed since — re-run `propose_heal` instead. A merge deletes a node and cannot be
undone, which is why the check exists.

Read the `HealReport` back:

- `blocked_by_mode: true` — the project is in **rigid** mode, so nothing was applied by design.
  The proposal stands as a record. Take it to the user; do not try to route around it.
- `verified: false` or a non-empty `unresolved_issue_ids` — the repair did not achieve what it
  claimed. Report that plainly rather than treating the run as a success.
- `discarded` non-empty — the merge could not carry everything onto the survivor: the removed
  node's properties, an edge whose other endpoint is unknown, or an edge both nodes already had
  whose properties were overwritten. Usually that is fine, but it is a real loss and the user
  should hear about anything that looks like it mattered.

If `requires_human_review` is true, **do not call `apply_heal` expecting it to resolve those
issues** — the generative half is not built. Go to step 4.

## 4. Bring the judgement calls to the user

Everything in `generated_content` is a design decision, not a repair. Ask about it plainly, in
the user's own terms, one at a time — the same discipline as `detect-and-ask`, without the
`gap_to_prompt` handshake (these are `HealIssue`s, not `GapCandidate`s, so phrase them
yourself).

A circular dependency is the most important of these. Do not "fix" it by deleting an edge —
that discards real information. Show the loop and offer the three real ways out:

- **invert one dependency** — which of these parts should own the relationship?
- **introduce a contract** — put an `Interface` between them so one side depends on an agreed
  boundary rather than on the other part's internals.
- **make it event-driven** — one part emits, the other reacts, and the loop opens.

Then record the answer: create the `Interface` (`add_interface` + `provides`/`consumes`), or
redirect the dependency edge, or capture the decision as a node. Re-run `detect_defects` to
confirm the loop is gone.

## 5. Confirm

Re-run `detect_defects` (and `detect_gaps`) at the end. A defect you reported and then resolved
should be absent; anything still present should be named to the user, not quietly left.

---

A structural defect is not a style problem. A dependency loop means neither part can be built,
tested, or reasoned about alone — and it is exactly the kind of thing that looks fine in any
single file and only shows up in the whole graph. That's what reflow2 is for.
