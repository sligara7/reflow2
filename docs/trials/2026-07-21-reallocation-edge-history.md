# Trial: divergent direction over time — reallocation and edge history (2026-07-21)

A user question, run to ground on a throwaway graph through the real `reflow2-mcp` binary:
*"Over 8–9 months a design grows organically. Seven months ago I said Service A does functions
X, Y, Z. Now I decide Z should move to Service B. Since the graph keeps the old guidance, what
happens — does it cut off the old node, or (axis Z) keep it and add a new one?"*

## Scenario driven

1. **Original design** — Project; capabilities Authorize (`cap:x`), Settle (`cap:y`),
   Reconcile (`cap:z`); components Service A (`cmp:a`), Service B (`cmp:b`); all three
   capabilities `ALLOCATED_TO cmp:a`; a Decision `dec:own-v1` "Service A owns X, Y, Z" with
   `cap:z GOVERNED_BY dec:own-v1`.
2. **New direction — the right way** — `add_epoch` (revision); `record_change` snapshots
   `cap:z`'s prior state at the epoch and opens the `chg:move-z` (`scope_change`) event;
   `propagate_change` shows the blast radius *before* editing; then `delete_edge` the old
   `ALLOCATED_TO cmp:a`, `allocate cap:z → cmp:b`, `add_decision dec:own-v2`,
   `dec:own-v2 OBSOLETES dec:own-v1`, `dec:own-v1` status → `superseded`, re-point
   `cap:z GOVERNED_BY dec:own-v2`.

## What resulted

- **The past was preserved, not cut.** Both decisions coexist — `dec:own-v1` (superseded, text
  verbatim: "Authorize, Settle and Reconcile all live in Service A") and `dec:own-v2`
  (accepted). History is walkable via `dec:own-v2 --OBSOLETES--> dec:own-v1`. Answer to the
  user's either/or: **preserve + evolve**, never a silent cut.
- **Impact-check did real work.** `propagate_change` from the event returned
  `[cmp:a, dec:own-v1, cap:x, cap:y]` **before** the edit — i.e. moving Z touches Service A,
  the decision that governed Z, and the *sibling* functions X and Y still living on A. Exactly
  the "what else is on this service" signal you want before splitting one.

## Finding → BL-63

The snapshot of `cap:z` held only its **properties** (`name, status, tier, …`) — **not** the
`ALLOCATED_TO` edge it lost. A reallocation is an *edge* move, and `cap:z`'s own properties
never changed, so the snapshot was near-vacuous for this change. The **only** durable record
that "A once owned Z" was the hand-authored `dec:own-v1`. A lazy reallocation (delete + add, no
Decision) would leave Z on B with no trace it was ever on A.

Confirms the axis-Z edge-history gap in practice (was BL-58 idea I4) and promotes it to
**BL-63**: capture a changed node's edges into the snapshot, so link history survives without
depending on the modeller remembering to record a Decision. Interim mitigation: say so loudly
in `snapshot_node` docs and the revise/retire skills — *for a reallocation, the history lives
in the Decision you record.*
