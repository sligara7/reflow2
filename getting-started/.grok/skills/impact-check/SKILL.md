---
name: impact-check
description: Use BEFORE changing or removing anything in an existing design — a new feature, a tweaked requirement, "what if we add wind?". Records the change and runs reflow2's propagate to show the blast radius, so you edit only what's actually affected and confirm nothing else rotted.
---

# Check impact before you change code

Every new idea is where a stateless agent breaks things. reflow2 tells you exactly what a
change touches — use it first, edit second.

1. Frame the change. For a real, decided change, record it:
   - `add_change_event` with an id, a name, and a `change_type` (e.g. `new_feature`,
     `scope_change`, `constraint_change`).
   - For a speculative "what would this touch?", skip straight to `propagate_from` with the
     seed node ids the idea starts from.
2. Compute the blast radius:
   - `propagate_change` from the ChangeEvent, or `propagate_from` from seed ids.
   - Read the result: `impacted` (each with `distance`, `direction`, `via` chain,
     `crosses_risk_edge`), plus `unknown_seeds` and `truncated_beyond_depth` — always check
     these partial fields; nothing is dropped silently.
3. Edit **only** the impacted capabilities/components/tests the radius names. Do not touch what
   isn't in it, and do not miss what is.
4. If the change adds or removes intent, also run **capture-intent** to update the graph, then
   **detect-and-ask** for any new gaps.
5. After editing, re-run `detect_gaps` (and optionally `graph_report`) to confirm the design is
   still coherent.

"If I add wind, what does it touch?" is a `propagate_from` call, not a guess.
