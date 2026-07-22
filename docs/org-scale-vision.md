# reflow2 at organization scale — one graph the whole company plans in

*A note on where the multi-user thread could go. Not scheduled work — the destination that
orders it.*

## The idea

Today reflow2 keeps one project coherent for one person (or two, taking turns): intent at the
top, everything below traceable to it, and detectors that fire when the two drift apart.

The idea: host that same graph for a **whole organization**, so everyone — leadership, program
managers, engineers — works in one shared design instead of a hundred disconnected documents.

## The part that makes it different from every planning tool

The golden thread doesn't have to stop at "project requirements." It can start at the top of
the company:

- **The CEO states direction as top-level objectives in the graph** — real nodes, not a slide
  deck. Divisions trace their goals to them; programs trace to divisions; every project's
  requirements trace to a program; every component, decision, and line of code traces upward
  from there.
- **Every decision at every level is made in sight of the objectives it serves.** A decision
  node links to what it's governed by. When someone proposes work that traces to nothing, the
  graph says so — that's the existing `unmotivated` detector, pointed at strategy instead of
  code.
- **When leadership changes direction, propagate computes who is affected** — the actual blast
  radius of a strategy change, down to the team level, instead of an all-hands and a hope.
  And it works upward too: a slipped technology readiness at the bottom ripples up to the
  objective it threatens.
- **The roadmap falls out instead of being asserted.** Because readiness and dependencies live
  in the same graph as the objectives, "when can we deliver X" is computed from the thread —
  and defended, node by node.

Nobody in the org needs to know systems engineering. That's already reflow2's core rule: the
graph carries the discipline; people answer plain questions.

## Why the pieces already line up

Everything hard about this is already on the backlog as an increment, in order:

1. **Shared server** (BL-12) — the core is deliberately surface-neutral; a hosted MCP endpoint
   in front of the same engine, self-hosted on the org's own machine (their designs never
   leave their control).
2. **Claims** (BL-44) — teams check out clusters of the graph ("Alex has this cluster") so
   hundreds of people don't collide; the graph computes what a claim board does socially.
3. **Trust** (BL-41) — who may assert what: a requirement moves to *accepted* only on the
   owner's say-so, verification status only from CI, etc.
4. **Parallel alternatives** (BL-70) — competing options held open as branches until a decision
   point; the org sees the analysis of alternatives, not just the winner.
5. **Design-vs-design diff/merge** (BL-71) — what disagreement between two writers resolves
   through.

## Why it fits your licensing idea

This is the natural paid tier of exactly the model you proposed: personal and school use stays
free and unlimited (the embedded, single-writer reflow2 of today — unchanged); the org-scale
hosted deployment is what commercial and government licenses buy. The free tier is the on-ramp;
the org tier is the product.
