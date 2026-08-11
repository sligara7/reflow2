# Impact Propagation — the PROPAGATE step of the coherence loop

> Part of the **Reflow 2.0** design docs — see **[overview.md](overview.md)** for the full map and reading order.

> **This document is a RENDERED VIEW.** Everything below the line marked *rendered* is projected
> from reflow2's own design graph and source, not written by hand. It is the first worked example
> of the projection doctrine in [viewpoints.md](viewpoints.md): *the graph stores the detail, a
> view is a projection of it, and the agent's only job is to render.* Anything a view needs and
> the graph cannot supply is **confessed** rather than filled in — the confessions at the end are
> the deliverable, not a defect in the page.
>
> Rendered 2026-08-11 against `docs/design/reflow2.json` and `crates/reflow2-core/`.

When something changes in any phase, PROPAGATE walks the **golden thread** to find everything
the change touches — the *blast radius* — so DETECT can flag the new gaps and HEAL can bring the
graph back to coherence. It is the connective tissue of [the vision](vision.md).

Two entry points, one engine:

- **Reactive** — `propagate_change(change_event_id)`. A `ChangeEvent`'s `CHANGED` targets are the
  seeds. A ChangeEvent id that does not resolve is **refused**, not answered with an empty radius:
  "no such event" and "this change harmed nothing" must not share a reply.
- **Speculative** — `propagate_from(seed_ids)`. The same traversal without writing anything, so
  you can see the blast radius before committing to the change.

---

## rendered · what actually propagates

*Projected from `crates/reflow2-core/src/nodes.rs::structural_rule`. Sixteen edge types carry
impact; every other edge type in the schema does not.*

| Edge | Walked forward | Walked backward |
|---|---|---|
| `SATISFIES` | upstream | downstream |
| `DECOMPOSES` | upstream | downstream |
| `REALIZES` | upstream | downstream |
| `VERIFIES` | upstream | downstream |
| `GOVERNED_BY` | upstream | downstream |
| `INCLUDES` | upstream | downstream |
| `CALIBRATED_AGAINST` | upstream | downstream |
| `CONSTRAINS` | downstream | upstream |
| `ALLOCATED_TO` | downstream | upstream |
| `DEPLOYED_TO` | downstream | upstream |
| `REQUIRES_RESOURCE` | downstream | upstream |
| `SCHEDULED_FOR` | downstream | upstream |
| `PROVIDES` · `CONSUMES` · `DEPENDS_ON` · `PART_OF_FLOW` | lateral | lateral |

Any **inference** edge (`CAUSES`, `RISKS`, `MITIGATES`, `CONTRADICTS`, `ANTICIPATES`, `MASKS`, …)
is classified `causal` regardless of direction.

**`CONTAINS` is deliberately absent**, and the source says why: it is decomposition (axis Y), not
traceability. Propagating along it would make the Project a hub that short-circuits every sibling
to about two hops.

> **Two recorded scars, kept because they are the same scar twice.** `INCLUDES` was missing from
> this table until v0.5.0, which made every Release+Environment pair a disconnected island.
> `SCHEDULED_FOR` was missing until 2026-07-31 and produced the identical failure the first time a
> Release was modelled from its schedule rather than a manifest. Twice a new edge type has reached
> a Release without anyone asking whether the impact table should know about it, **and nothing
> checks that question.**

---

## rendered · a worked blast radius

*Projected from `propagate_from(["cap:capture-registers-its-source"], max_depth: 2)`, run against
the live graph. This capability was added on 2026-08-11, so its thread is small enough to show
whole.*

**The direct ring — five nodes, each explained by the edge that reached it:**

| Impacted | Direction | Via |
|---|---|---|
| `cmp:skills` | downstream | `ALLOCATED_TO` |
| `req:no-idea-goes-quiet` | upstream | `SATISFIES` |
| `art:capture-intent-skill` | downstream | `REALIZES` |
| `dec:idea-should-a-requirement-trace-back-to-its-demand-signal` | upstream | `GOVERNED_BY` |
| `dec:idea-how-is-a-source-outside-the-repo-registered` | upstream | `GOVERNED_BY` |

Read as prose: changing this capability reaches **down** to the component that hosts it and the
file that implements it, **up** to the requirement it serves and the two open questions that shape
it. Nothing is listed without the edge chain that put it there.

**At depth 2 the shape changes character.** The walk crosses `cmp:skills` and picks up every other
capability allocated there, every Release that ever included it, and the component's own check —
plus one lateral hop:

```
boundary_crossings: ["ifc:mcp-tools"]
truncated_beyond_depth: 286
```

Two things worth reading off that. The lateral `CONSUMES` hop reaches a **published boundary**, so
the walk flags it — a change here is visible on the far side of a declared contract. And **286
further nodes exist beyond depth 2 and are counted, not hidden**: bounding the traversal is
allowed, silently truncating it is not.

---

## rendered · what a bound actually costs

*Projected from `propagate_from(["req:coherence"], max_depth: 3)`.*

`req:coherence` — *"design stays coherent across its lifecycle"* — is near the root of the thread.
Its blast radius at depth 3:

| | |
|---|---|
| impacted | **246** |
| truncated beyond depth 3 | **484** |
| by distance | 10 · 74 · 162 |
| by direction | downstream 161 · upstream 64 · lateral 19 · **causal 2** |
| paths crossing a risk edge | 2 |
| published boundaries crossed | **0** |

The distance profile is the reason bounding is not optional: each hop roughly doubles the answer,
and two thirds of what a depth-3 walk finds is at the outer ring.

**Causal 2 and boundary 0 are both findings about this design, not about the engine.** Only two
inference edges are reachable from the requirement that governs coherence — the "why" layer is
sparse here. And zero published boundaries are crossed anywhere in 246 nodes, which is the
`seams: 0%` frontier from `maturity_report` showing up from a second direction: nothing in this
design couples through a declared `Interface`.

---

## rendered · confessions

*What this document asserted before it was rendered, that the graph and the build do not support.
Each is a real deferral tracked in [requirements-coverage.md](requirements-coverage.md).*

- **`ENABLES` is a retired edge type.** The previous version of this page listed it among the
  inference edges. It was real when this page was written on 2026-07-17 and **retired five days
  later** — `dec:edge-orthogonality`, 2026-07-22, folded into `CAUSES` because the two were the
  same causal axis and no computation read them apart (`schema/inference.yaml:35`). The doc was
  not wrong when written; it went stale, and then stayed stale for three weeks while the schema
  moved underneath it. **Nothing could tell** — no check reads a doc's vocabulary against the
  schema's, so a retired edge type kept being taught to every reader of this page.

- **The impact-kind table does not exist.** This page used to carry eight named impact kinds —
  `unmet_requirement`, `stale_verification`, `violated_constraint`, `orphaned_artifact`,
  `phase_desync`, `introduced_contradiction`, `undersized_resource`, `coverage_gap` — as though
  they were the design. **None of them is implemented.** That is `IP-6` (⬜), and `IP-15` and
  `IP-19` are partial *because* of it. Impacted nodes today carry a direction and an edge chain,
  and no kind at all.

- **Confidence does not decay with depth** (`IP-7`, deferred) and **inference-edge confidence is
  not weighted** (`IP-3`, deferred). Ranking is: distance → risk-edge crossing → centrality → id.
  Nothing multiplies a confidence along a chain.

- **Criticality is not inherited** (`IP-10`, deferred). A `critical` Constraint and an `info` one
  rank identically.

- **The cause is not surfaced** (`IP-12`, partial). The page promised *cause → change → blast
  radius*; the `CAUSES`→ChangeEvent wiring is not there, so only *change → radius* is real.

- **Speculative before/after diff is not wired** (`IP-13`, partial). `temporal::snapshot_node`
  exists; nothing diffs a speculative run against it.

- **Propagation does not filter by epoch** (`IP-11`, partial), **does not prefilter by project**
  (`IP-16`, partial — scoping is one graph per design), and **is not cached** (`IP-17`, partial:
  deterministic, uncached).

---

## The disciplines — prose, and deliberately not rendered

These are commitments about how the engine must behave. No query produces them, and a projection
of them would say nothing.

1. **Bound the traversal, never silently truncate.** `truncated_beyond_depth` is a count, always
   reported. The 286 and the 484 above are that rule working.
2. **Explain every impact.** Each impacted node carries its `via` edge chain. "This is affected"
   with no path is not an answer.
3. **Feed the loop, don't fix.** PROPAGATE computes and tags. Turning tags into questions is
   [gap-surfacing](gap-surfacing.md); repair is [heal](heal-process.md). Keeping the three apart is
   what stops an engine that finds problems from also deciding them.
4. **Refuse, don't return empty.** An unresolvable seed or ChangeEvent id fails loudly, because a
   typo and a harmless change must never produce the same reply.

---

## How this page is maintained

Every table above names the tool call or source location it came from. Re-running those against a
changed graph produces a different page; the prose sections do not move. Nothing here is a number
a person typed and must remember to update — which is the whole point, and is why the retired
`ENABLES` edge and the eight unbuilt impact kinds survived in this file for three and a half weeks.

**One caveat this page has to state about itself.** It was rendered by an agent, not by a
deterministic renderer, so re-rendering it will produce different *wording* for the same facts —
which means `git diff` on this file no longer cleanly means "the design changed". `viewpoints.md`
names that trade: `tools/render_views.py` cannot improvise by construction, and an agent can. The
open question is `dec:idea-outward-docs-are-rendered-from-the-graph`, and this page is its first
piece of evidence rather than its conclusion.
