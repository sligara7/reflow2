# Viewpoints — the catalogue of pure projections

> Part of the **Reflow 2.0** design docs — see **[overview.md](overview.md)** for the map.

**One model, many viewpoints.** The graph stores every design detail; a view is a *projection*
of it, DoDAF/UAF-style — and the agent's only job is to render. This is the SYNTHESIZE process
held to a no-extrapolation standard (BL-40; doctrine recorded in [sharpening.md](sharpening.md)
§3.5, from the project's author, a UAF/DoDAF-trained systems engineer):

> If rendering requires extrapolating or filling in missing details, that is a sign something is
> missing or wrong inside reflow2 — not a prompt to improvise.

So every renderer is also a **probe**. Anything a viewpoint needs but the graph cannot supply is
a *confession* — a modelling gap, a reflow2 gap, or a true design gap, printed loudly and
rendered into the page. The first run of the seed renderer confessed exactly BL-37 (no ordered
process was expressible) before that item had a fix; a confession list is a work list.

The standing renderer is `tools/render_views.py` → [design/views.html](design/views.html).

```bash
python3 tools/render_views.py                          # the committed design export
python3 tools/render_views.py some-export.json         # any export document
python3 tools/render_views.py --graph-path .reflow2/graph   # a live graph directory,
  # via `reflow2-mcp --export`. The graph is single-writer: stop any running MCP
  # session first — the error says so if you forget.
```

## The catalogue

DoDAF/UAF names are approximations on purpose — reflow2's vocabulary is domain-neutral, and the
mapping is there so a viewpoint-trained reader knows where to look, not to claim conformance.

| Viewpoint | ≈ DoDAF | Projected from | Status |
|---|---|---|---|
| **Functional** | OV-5 | `Requirement` ← `SATISFIES` ← `Capability`, with `status` and `priority` | ✅ rendered |
| **Operational flow** | OV-5b / OV-6 | `Flow` + `PART_OF_FLOW.step_order` + `TRIGGERS.role` among members; cycles as SCC clusters, **reported never judged** | ✅ rendered (BL-37); confessed when the graph holds no Flow |
| **Structural** | SV-1 | `Component` `CONTAINS` tree + `Interface` with `PROVIDES` / `CONSUMES` both sides | ✅ rendered |
| **Traceability** | SV-5 | `ALLOCATED_TO`, `REALIZES` (both P3 shapes), `VERIFIES` with outcome | ✅ rendered |
| **As-released** | SV-8-ish | `Release` `INCLUDES` with `as_checksum` frozen at cut, `DEPLOYED_TO`, and the built-but-not-shipped diff | ✅ rendered (BL-34) |
| **Decisions** | *(no clean DoDAF box — the record of why, which DoDAF leaves to AV-1 prose)* | `Decision` (`decision`, `rationale`, `status`) + incoming `GOVERNED_BY` | ✅ rendered |
| **Evolution / epoch timeline** | SV-8 proper (axis Z) | `DesignEpoch` ordered by `PRECEDES` (solid arrows) or `sequence` (dotted, labelled with its source — the two are cross-checked and a disagreement is confessed); what happened at each via `AT_EPOCH` / `OCCURS_DURING`; a ChangeEvent pinned to no epoch is confessed as the axis-Z discipline broken | ✅ rendered |
| **Provenance** | AV-2-ish | `provenance` per `Requirement` / `Capability` / `Component` / `Interface` (unstated origins confessed, `inferred` listed by name), `Fragment` + `YIELDED` with the action taken (a mute Fragment and a dangling YIELDED are confessed) | ✅ rendered |
| **As-fielded** | actual deployment reality | `DEPLOYED_TO` declarations per `Environment` (a statusless declaration is confessed), plus every unresolved deployment `DriftEvent` recorded by `reconcile_deployment` (BL-9) — only Releases run, only Environments host | ✅ rendered |
| **Measures / budgets** | SV-7 | budget `Constraint`s (`quantity`/`limit`/`direction`) with their `CONSTRAINS` spenders, stated totals and an honest verdict — `incomplete` when any contribution is unstated (BL-11); `budget_report` is the typed read side and carries the worst-path analysis | ✅ rendered |

## Rules for adding a viewpoint

1. **Only what the graph states.** A projection may sort, group, count and draw — it may not
   infer, default, or improvise. If the view needs something the graph cannot supply, `confess()`
   it; the confession *is* the deliverable when the model is incomplete.
2. **Do not assert what is not stated.** The renderer's own first drafts broke this twice, and
   both are worth remembering: rendering an SCC as an arrowed path asserts a walk order the graph
   never stated (a cluster is mutual reachability — braces, not arrows); and rendering a
   `PART_OF_FLOW` edge whose capability node does not exist as if it were a step papers over a
   dangling edge. Sorting for determinism is rendering; ordering presented as *meaning* must come
   from a property (`step_order`) or an edge.
3. **Absence the graph states honestly is not a confession.** A graph with no `Release` renders
   an empty as-released view with a pointer to the phase detector that asks about it; a
   confession is reserved for things the view *needed while rendering* and could not find.
4. **Watch the instrument-accommodation trap** ([sharpening.md](sharpening.md) §4): a renderer
   that reaches zero confessions by only asking questions the graph can already answer has been
   tuned to the instrument. New viewpoints should ask for what a viewpoint-trained reader
   actually expects, and confess the misses.
5. **Add the row here** and keep [overview.md](overview.md)'s SYNTHESIZE row pointing at this
   file, so the catalogue has one home.

## Direction (the rest of BL-40)

Rendering currently runs from the export document (or a live graph directory via `--export`).
The single-writer lock means an external script can never see a graph an editor session holds —
only the agent *inside* the session can. The recorded direction: once the catalogue's shape has
settled here, move the projection data into core as typed read tools on the MCP surface (the
`flow_report` shape — facts plus confessions — is the template), so the in-session agent renders
views deterministically instead of re-deriving them through an LLM, which is exactly where
extrapolation would creep back in.
