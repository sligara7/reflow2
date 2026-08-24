# Upgrading to v0.40.0

**This doc exists because the schema stamp moved.** v0.39.0 stamps 61 edge types; v0.40.0 stamps
**64**. Three edge types were added — `IMPLEMENTS`, `COMPLEMENTS` and `INVALIDATES` — and none was
removed. Node types are unchanged at 29, and `schema_version` is still `1`.

## What you must do

**Nothing, for your graph.** No existing node or edge is reinterpreted, nothing is migrated, and an
export written by v0.39.0 imports into v0.40.0 unchanged. Every change in this release is additive.

**Something, if you have a v0.39.0 binary anywhere.** A graph written by v0.40.0 will be **refused**
by it, with a message naming the unknown edge types and telling you to update. That refusal is
correct and it is the reason this release is safe — see the last section, which is the part worth
reading even if you never draw one of these edges.

## What is new

### `IMPLEMENTS` — Artifact → Verification

*"This file IS the executable form of that check."*

A `Verification` records that something was checked. Until now there was no way to say which script,
test or harness actually runs it. `REALIZES` was wrong (that says a file implements a **capability**;
a check interrogates one rather than providing it), `DOCUMENTS` was wrong (this is executable, not
prose), and `CALIBRATED_AGAINST` was wrong (nothing was fitted to the check's output).

```
create_edge IMPLEMENTS  Artifact art:check-plot-thread  Verification ver:plot-thread-audit
```

Carries `covers: whole | partial` and a `note`. Use `partial` when a check has a repeatable half and
a judgement half — it stops a green script being read as the whole check having passed.

**What reads it:** `loop_status.verifications` gains **`no_executable_form`**. `never_run` used to
be one number covering two very different debts — a check somebody wrote a script for and has not
run, and a check with nothing to run at all. The first is a scheduling problem; the second means the
check exists only as a sentence. Those now separate.

### `INVALIDATES` — any record → any finding

*"The work I just did answered that finding, so stop proposing it."*

A finding survives the work that answered it. A `failing` check goes on reading as the live state of
the system after its defects are fixed; a measured `TemporalFact` goes on reading as current after
the thing it measured has moved. Nothing in the vocabulary joined the repair to the finding, and
`describe_schema(from: Constraint, to: Verification)` returned **zero exact matches**.

```
invalidates  Constraint con:forge-scaling  Verification ver:the-walk
             note="the clamp landed in a1b2c3d; this verdict predates it"
             at="2026-08-23"
```

Endpoints are deliberately open. Draw it from whatever recorded the work — a Constraint carrying a
repair, a ChangeEvent, a Decision — to whatever recorded the finding: a Verification whose last run
found it, or a TemporalFact that measured it. A design's findings live wherever that design put them.

**⭐ IT SAYS THE RESULT IS STALE AND NOTHING MORE.** A repair does not make a check pass; only a
re-run can say what is true now. That is why it is not called `RESOLVES`, why it never touches the
target's `status`, and why **the check stays listed and counted in `loop_status`** — silencing it
would replace one wrong reading with another. What changes is that the row now carries
`invalidated_by`, `rerun_owed`, and a sentence telling you not to quote the verdict as current.

**`rerun_owed` IS THREE-VALUED AND THE THIRD VALUE IS THE POINT.** `true` = the repair postdates the
last run, so re-run before believing the verdict. `false` = the run already postdates the repair, so
the verdict is current. **`null` = one side carries no date, so nobody can tell** — never read it as
`false`.

**What reads it:** the new `invalidated_findings` tool, `loop_status.verifications` (per-row plus a
`rerun_owed` summary), `graph_report`, and the `where-am-i` skill, which now calls it before quoting
any failing verdict.

**Where it came from:** a session ran `where-am-i`, read a check with status `failing` and a
`last_run_at` of that same day, and reported its two defects to its user as the live state of the
system. Both had been repaired hours earlier and written up on two Constraint nodes. Every node in
that graph was right; the composition was wrong.

### `COMPLEMENTS` — DesignRule → DesignRule

*"These two stand beside each other on purpose and must never be merged."*

Two governance rules can cover adjacent ground for different reasons — one binding what you may
**claim**, its neighbour what must be **true** whether or not anyone claims anything. To anything
comparing text they look like near-duplicates.

**What reads it, and this is the point: HEAL now REFUSES the merge.** `DUPLICATES` is declared
`* -> *`, so two rules can be joined by it, and HEAL will then propose a merge that **deletes one,
irreversibly**. With a `COMPLEMENTS` edge between them, both `propose_heal` and `apply_heal` refuse
and say why. Before this, the only protection was a paragraph somebody wrote asking future readers
not to merge.

Carries `evidence` — say why they must not be merged. An edge asserting "do not merge" with no
reason is one a later reader cannot check, and this edge exists precisely to be obeyed by machinery
that would otherwise act.

It is declared **narrowly**, DesignRule to DesignRule. Requirements and Constraints plausibly want
it too, but widening an enumeration later is safe while narrowing one strands every edge already
written under the broader form.

### `SUPERSEDES` now accepts Verification → Verification

The edge whose **name** describes the relation exactly used to be refused for this pair. Folding two
one-off probes into one table-driven check had no modelled way to record that the broader one
replaced the narrower; the honest fallback was `EVOLVES_INTO` through a double wildcard, which
cannot be told apart from "these two happen to be linked".

⚠️ Enumerated `from`/`to` is a **cross product, not a pair list**, so this also admits
`Fragment → Verification` and `Verification → Fragment`, which are meaningless. The schema has no
way to express "these pairs only". Both nonsense combinations are writable and neither is read by
anything.

### `Verification.status` gains `superseded`

A check replaced by a broader one is not `skipped` — that means a run was deliberately not made —
and it must not go on reporting `passing`.

**What reads it:** every coverage computation filters on `== "passing"`, so a superseded check stops
counting as live coverage the moment you set it. It also **no longer appears in
`loop_status.verifications.attention`**, which lists checks that are not passing: a retired check is
not a quiet failure, and surfacing it beside the failing ones would dilute the loud-first list.

## The part worth reading even if you draw none of these edges

**Adding an edge type is the *safe* kind of schema change, and the ones that do not move the stamp
are the dangerous ones.** `GraphStamp::current` counts `schema.node_types.keys()` and
`schema.edge_types.keys()` — **type names only**.

| change | stamp sees it? | what an older binary does |
|---|---|---|
| **new edge type** | **yes** | refuses the graph, names the type, says migrate or update |
| widen an edge's endpoints | **no** | opens fine, then faults on that one edge at import |
| new enum value | **no** | opens fine, then faults on that value |
| new property | **no** | accepts it silently |

Three of this release's four changes are in the invisible rows. They ship **alongside** the two new
edge types deliberately: the stamp move forces an older binary to refuse the whole graph up front,
which is a far better failure than discovering a `superseded` status or a widened `SUPERSEDES` edge
one import fault at a time.

If you are pinning reflow2 for other consumers, that is the reason to take this release whole rather
than cherry-picking.
