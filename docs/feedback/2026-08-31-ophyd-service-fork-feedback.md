# reflow2 feedback — alternate design forks / exploration designs

Running log for the "forks off the main design" update. Dated entries, newest first.
Design content redacted to shapes per the `report-friction` skill, except where the
point is unintelligible without a named scenario.

**Environment (2026-08-31):** reflow2 0.45.0; client claude-code 2.1.251, MCP negotiated
2025-11-25 (from `.reflow2/graph.client.json` in the ophyd-service repo); Linux.

---

## 2026-08-31 — exploring an architecture alternative fights the single live design

### What I was doing

A brainstorm session produced a real architecture fork: a proposal to consolidate
services that, if taken, reverses two settled Decisions and re-homes several accepted
Requirements. The user wants to *explore* this alternative seriously — sketch its
requirement set, its component shape, maybe its gaps — while the main design stays
authoritative and untouched. Today's session recorded it as an exploratory Decision
(kind: `exploratory`) with the counters in prose, which is the right capture for an
*idea*, but is too shallow a vessel for an *exploration*: an alternative you are
seriously weighing needs its own requirements, components and gap analysis, not three
paragraphs inside one node.

### What happened (measured)

- The exploratory Decision's one honest `CONTRADICTS` edge to an accepted Decision
  raised the **main design's structural-defect count from 1 to 2** the moment it was
  drawn. The edge is true and the detector is doing its job — but the signal lands on
  the main design's health report, where it reads as damage rather than as a live
  exploration. `loop_status` now carries that +1 for as long as the idea stays open.
- Going further — actually modeling the alternative — would require either flipping
  accepted Requirements' status in place (corrupting the delivery line, the
  confirmation ledger, and `where-am-i`'s "what's settled" narration for everyone
  reading the MAIN design) or holding the exploration outside the graph entirely
  (a git branch was created for code; the design side has nowhere equivalent to go).

### What already exists (checked via `find_tools`, so this is not filed blind)

The branch-by-**file** family covers part of this, and it deserves credit:

| tool | what it gives |
| --- | --- |
| `register_alternative` (BL-70) | an alternative as an Artifact *pointer* to a separate design export, GOVERNED_BY the decision point, CONTRADICTS its siblings |
| `analyze_alternatives` | compare parallel alternatives on the same measures |
| `compare_designs` | diff two as-designed records |
| `merge_designs` / `apply_merge` (BL-80) | three-way merge of a divergent design against a common ancestor |
| `describe_designs` | identify what design lives at each store path |

Notably, the brainstorm skill *does* surface `register_alternative` at the right
moment ("if a brainstormed option grows a design of its own"). So the discovery path
exists; the friction is in what the mechanism can hold.

### The gap, as experienced

Branch-by-file means the alternative lives in a **different store**, and that costs:

1. **No one-call fork.** Getting from "live design at HEAD" to "an alternative store I
   can explore in" is a manual `export_graph` → new store → `import_graph` dance.
   Compare `git switch -c`: the cheapness of creating a branch is what makes people
   actually branch.
2. **A session binds to one design.** Exploring the alternative means *leaving* the
   main design — no asking "how does the alternative's delivery line compare" in one
   breath, no drawing a relation from a fork node to a main node (the fork's nodes
   are strings in a file, not addressable nodes).
3. **The health machinery is fork-blind.** `detect_gaps`, `detect_defects`,
   `loop_status`, the delivery line, `where-am-i` — none can be asked "under fork F".
   A fork that flips a settled requirement either mutates main or is invisible to
   every computation that makes reflow2 worth using. This is the core of it: **the
   analysis tools are the reason to model the alternative at all**, and they only run
   on the one live design.
4. **CONTRADICTS across the exploration boundary scores against main.** An honest
   edge from an exploratory idea to a settled decision is the *expected shape* of an
   exploration, yet it lands in the same defect count as a genuine incoherence.
   (Arguably a detector fix independent of forks: a CONTRADICTS whose source is an
   OPEN `kind: exploratory` Decision could be reported as "live exploration" rather
   than "structural defect".)

### What the desired workflow looks like (user's words, paraphrased)

Alternate design forks exist **simultaneously in the graph** with the main design:
fork off "main" at a point, explore the alternative (its own requirement statuses,
its own components, its own gaps) without reversing anything settled in main, and
later merge it back or discard it — with the history right either way.

### Suggested shapes (suggestions only — the maintainer's call)

- **Overlay/fork as a first-class scope**: nodes and edges carry an optional fork
  label; a fork *overlays* main (unchanged nodes are inherited, changed ones are
  shadowed), the way a git branch shares objects with its parent. Reads and
  computations take an optional `fork:` scope; unscoped = main, exactly as today, so
  nothing existing changes behavior.
- **`fork_design` / `discard_fork` / promote-via-`apply_merge`**: one call to fork at
  HEAD (reusing the export machinery under the hood is fine — the point is the one
  call), and the existing BL-80 merge as the promotion path, so the new surface stays
  small.
- **Fork-scoped health**: `detect_gaps` / `loop_status` / delivery line accept the
  fork scope; main's report gains one line ("2 exploration forks open") instead of
  inheriting the forks' contradictions as defects.
- **Cross-fork comparison is the payoff**: `analyze_alternatives` pointed at fork
  scopes instead of export files — "main vs. fork on the same measures" is the
  question the whole exercise exists to answer.
- The existing `register_alternative` decision-point anchoring is the right *entry*:
  a fork born under a proposed Decision keeps the "why does this fork exist"
  traceable, and `collapse_decision` choosing a fork is a natural promotion trigger.

### Why it matters / what it cost

A real architecture question (service consolidation) is now **parked** — recorded as
prose-level ideas, code branch created and idle — specifically because modeling it
properly would damage the main design's signals. The tool's own strength (the health
machinery) is what makes the workaround (model it anyway, in place) unacceptable.

### Diagnosis

Mixed, honestly: partly `tool_not_found`-adjacent (the BL-70/BL-80 family covers
compare-and-merge and was findable), but the simultaneous-in-graph fork with scoped
computations is `tool_missing` — no served tool holds it, confirmed via `find_tools`
before writing this.
