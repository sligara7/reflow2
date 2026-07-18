---
name: capture-intent
description: Use whenever the user shares a new idea, feature, brief, or requirement for this project. Turns their words into reflow2 design-graph nodes (Requirements, Capabilities, Components) and links the golden thread — before any code is written.
---

# Capture intent into the reflow2 graph

When the user describes something they want, do NOT jump to code. Record the intent in reflow2
first, so it becomes durable, traceable design.

1. Read the user's message and identify:
   - **Requirements** — what must be true (a constraint, a must-have). → `add_requirement`
   - **Capabilities** — what the system does. → `add_capability`
   - **Components** — the part that will own a capability. → `add_component`
2. Create each node with a stable id (`req:…`, `cap:…`, `cmp:…`) and a clear name/statement.
3. Link the golden thread:
   - `satisfies` — Capability → Requirement it fulfills.
   - `allocate` — Capability → Component that implements it.
   - `contains` — Project → each child (`add_project` first if the project node is missing).
4. If a piece of intent is ambiguous or under-specified, do NOT invent an answer — leave it as
   a gap for the **detect-and-ask** workflow to surface.
5. Confirm back to the user what you captured (ids + names), briefly.

Extraction happens in your context (you read the brief and decide the nodes) — reflow2 stores
and validates them against its schema. Unknown types or missing required fields fail loud; fix
the node rather than working around the error.
