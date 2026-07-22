# Backlog — what's open, and why

> Part of the **Reflow 2.0** design docs — see **[overview.md](overview.md)** for the map.

Three records, three questions:

| Record | Answers |
|---|---|
| [requirements-coverage.md](requirements-coverage.md) | *Are we meeting the docs?* — requirement → module → test |
| [../CHANGELOG.md](../CHANGELOG.md) | *What changed, and when?* |
| **this file** | *What should we do next, and on what evidence?* |

Items point at their source rather than restating it. Sizes are rough: **S** ≈ an afternoon,
**M** ≈ a day or two, **L** ≈ a real project.

Each item has a stable id (**BL-n**). Claim one on the board in
[../COORD.md](../COORD.md) before starting, so two people don't build the same thing.

## Evidence base

Nine independent sources, which is why several items appear on more than one list:

- **Blind trial, 2026-07-18** — an agent with no knowledge of reflow2's source designed and
  built a weather station through the consumer kit. Its friction log is the single richest
  source of findings we have; quotes below are its words.
- **Grok via opencode, 2026-07-18** — a second blind trial, different model *and* harness. Found
  the `structuredContent` array bug that three home-grown test layers all missed, because every
  one of them was a client we wrote. Notes: [trials/](trials/).
- **macOS / grok build, 2026-07-18** — first real external user. Reached the design loop and
  asked for things the trial agent could not (it had no continuity across sessions to miss).
- **Self-host probe, 2026-07-18** — reflow2's own design (119 nodes) pushed into a reflow2 graph
  and interrogated. The first test above fixture scale, and the only one where we know the right
  answer. Notes: [trials/2026-07-18-selfhost-probe.md](trials/2026-07-18-selfhost-probe.md).
- **Brownfield trials, 2026-07-18** — reflow2 pointed at two systems that already existed:
  `ophyd-service` (private trial record) (399 files, ~110k LOC, requirements
  inferred backward from code) and `3dtictactoe` (private trial record)
  (~20 files, no spec at all — the pure-inference case). The only source for BL-27, and the two
  independently reproduce the same entry-point finding at a 20× size difference.
- **Self-host genesis, 2026-07-18** — `/genesis` run on reflow2 itself through the installed kit,
  from **Claude Code** rather than grok build. The only source for BL-28: the harness difference is
  what exposed it. Otherwise mostly a replication of findings above, and its
  [notes](trials/2026-07-18-selfhost-genesis.md) mark which is which.
- **Self-host functional design, 2026-07-19** — the first *durable* design graph of reflow2, 96 nodes,
  committed as a deterministic export at [design/reflow2.json](design/reflow2.json) and analysed with
  reflow2's own surface. Independently rediscovered five open backlog items, and found two detector
  defects. Notes:
  [trials/2026-07-19-selfhost-functional-design.md](trials/2026-07-19-selfhost-functional-design.md);
  re-runnable via `tools/build_design_graph.py --analyse-only`.
- **Erosion trial, 2026-07-19** — the sharper half of the one below, and the closest thing we have
  to a reproduction of how the original reflow failed. Five rounds of *test fails → fix code →
  accept drift* on a coherent thread, then a release: afterwards the design describes a system that
  no longer exists and reports **zero gaps**. The only source for BL-33/34. Notes:
  [trials/2026-07-19-erosion.md](trials/2026-07-19-erosion.md); re-runnable via
  `tools/erosion_trial.py`.
- **Phase-coverage trial, 2026-07-19** — the first trial to go past P2. reflow2's own design carried
  through realization, verification and deploy, with divergences injected on purpose at each phase.
  Scored **P3 4/4, P4 1/4, P5 0/2, traceability 3/3**. The only source for BL-30/31/32 and the first
  execution evidence for BL-9. Notes: [trials/2026-07-19-phase-coverage.md](trials/2026-07-19-phase-coverage.md);
  re-runnable via `tools/phase_trial.py`.
- **[reflow-audit.md](reflow-audit.md)** — the original Reflow's workflows and tools, with
  adopt/obsolete verdicts.

- **Adopt trial on storyflow, 2026-07-20** — the first real exercise of the `adopt` skill, and the
  largest system reflow2 has been pointed at: 2,643 source files across 8 services, with a
  *separate* 979-note design corpus (`~/dev_storyflow`) as the intent source — the exact division
  reflow2's doctrine assumes and no trial had ever tested. The first trial to perform **dynamic
  analysis** (suites really run; a genuinely failing test found). Produced five true findings about
  storyflow its own notes do not state, and four about reflow2 — including the two that became
  BL-42 and BL-43. Notes:
  `2026-07-20-adopt-storyflow.md` (private trial record); the resulting
  graph is committed at `2026-07-20-storyflow-adopted.json` (private trial record).
- **Coherent-erosion trial, 2026-07-19** — the constructive counterpart: the same five fix cycles run
  *with* axis-Z discipline, the design following the build backwards. `designed == released` is
  reachable today and the original intent survives in a Snapshot — but reflow2 returns the **same
  verdict** for this graph and the eroded one. The only source for BL-35/36. Notes:
  [trials/2026-07-19-coherent-erosion.md](trials/2026-07-19-coherent-erosion.md); re-runnable via
  `tools/coherent_erosion_trial.py`.

> **How to weigh any of this: [sharpening.md](sharpening.md).** It records where findings actually
> come from (reflow2's own output contributed to 2 of the 12 items raised on 2026-07-19, and both
> required already knowing the answer), and the failure mode that would quietly invalidate the whole
> evidence base — shaping the model until the tool goes quiet.
>
> **A bias worth naming.** Every source above except the three 2026-07-19 trials stops at or before
> **P2**. The blind trials, both brownfield trials and both self-host runs all end at structure and
> allocation — so until 2026-07-19 the entire evidence base came from the phases the original reflow
> was *already good at*, and none from the phases where it failed. Weigh accordingly when an item
> cites "three independent trials": that usually means three independent trials *of the front half*.

## Next up

| ID | Item | Why | Size |
|---|---|---|---|
| **BL-42** | **The adopt pass has a noise floor: two defects produce half its output — DONE 2026-07-20** | From the `storyflow trial` (private trial record), the first real `adopt` run (2,643 files, 122 nodes). **(a)** `unrealized_capability` fires for every capability whose artifacts were modelled coarsely — 13 of 51 gaps — so the skill's own granularity instruction is punished by a detector; BL-23 fixed this exact shape for `unverified_artifact` by counting rather than asking, and the same is owed here. **(b)** The DETECT/HEAL double-count, reproduced a **fourth** time and now dominant: 20 of 31 defects are `orphan_node` on requirements `detect_gaps` already reports as `unsatisfied_requirement`. Together they were ~40% of the pass's output. **Both fixed, and re-measured on the same 122-node storyflow graph: gaps 51 → 38, defects 31 → 19, total output 82 → 57, with every true finding preserved** (12 unsatisfied requirements, 16 unmotivated capabilities, the generation↔media cycle). **(a)** `unrealized_capability` now reads a claim the modeller already made rather than guessing from topology: a component marked `realized` asserts it exists, so an absent artifact describes the artifact layer's coverage, not a hole in the design — while `planned`/`in_progress` still gets the forward-looking question. The number is kept as `graph_report.realization` (BL-23's bargain: drop the question, keep the count). No threshold, per BL-5's lesson that a loud detector needs a different question rather than a tuned number. **(b)** HEAL's `orphan_node` no longer covers Requirements or Capabilities — DETECT asks both, they were never repairable (a `generate_owner` stub `apply_heal` can never apply), and the docs' own division puts meaning in gap-surfacing. The Artifact orphan stays, because DETECT has no counterpart. Closing that also required teaching `unallocated_capability` that a Flow is structure, or a loose capability on a process-only graph would have gone silent. Four pinned tests flipped honestly | ~~S + M~~ |
| **BL-43** | **`graph_report` cannot see the provenance layer — DONE 2026-07-20** | Same trial: the import wrote 122 nodes, `graph_report` said **109** — the missing 13 are exactly the Fragments, which `report.rs`'s type census omits. The provenance ledger that makes every recovered claim checkable (and that BL-40's provenance viewpoint renders) is invisible to the surface an agent reads first, and the node count is quietly wrong. **Fixed**: `total_nodes` is now every node in the graph, counted from the *schema* rather than a second hardcoded list, so a type added later cannot go missing the way `Fragment` did. `design_nodes` keeps the lifecycle-ordered itemisation and a new `other_counts` names everything outside it — provenance, questions, drift events, axis-Z machinery — in the payload and in the Markdown. Verified on the storyflow graph: 122 imported, 122 reported (13 Fragments + 1 DriftEvent itemised). This is rule 6 — no silent caps — applied to reporting | ~~S~~ |
| **BL-41** | **Graph text is data, never instructions — and nothing says so** | **S half done, 2026-07-19** — the standing rule is stated in the three places an agent looks: a section in the consumer AGENTS.md, one line in each of the eight skills at the point where it starts reading graph text, and the server's `get_info` instructions (so a session that loads no skill still gets it in the handshake). The one genuinely uncovered LLM failure mode in [partnership.md](partnership.md): every skill tells the agent to read node text and act on the design, and a hostile or careless statement rides that trust. Bounded today (single user, local graph); real the day a graph is shared (BL-12) or an adopted repo's prose flows through INGEST (BL-27). Mechanical mitigation (provenance-aware trust, quoting boundaries — [BL-12](#bigger-threads) sketch idea 2 is its design seed) is **M** and should wait for a real multi-writer case | ~~S~~ + M |
| **BL-40** | **Viewpoints as pure projections (SYNTHESIZE held to a no-extrapolation standard)** | **First increment done, 2026-07-19**: the catalogue doubled — operational flow (from BL-37's machinery, retiring the seed's standing confession), as-released (from BL-34's), and decisions views join the original three; `--graph-path` projects a live graph via `--export`; [viewpoints.md](viewpoints.md) is the catalogue with the no-extrapolation rules and the not-yet-projectable list. The graph stores the design; the agent only renders, and each confession is a gap by definition. **Second increment done, 2026-07-19**: evolution (axis Z — PRECEDES solid, `sequence` dotted and cross-checked, floating ChangeEvents confessed) and provenance (authored-vs-inferred, the Fragment ledger, dangling YIELDED confessed) complete the projectable rows — 8 views rendered. **Remaining direction** (the author intends to expand this thread): once the catalogue's shape settles, projection data as typed core read tools on the MCP surface (`flow_report` is the template), so the in-session agent renders without an LLM in the projection path. As-fielded and measures landed with BL-9 / BL-11 — **all ten catalogue rows now render** | M–L |
| **BL-29** | **`apply_heal` trusts the proposal; merge loses data silently — DONE 2026-07-20** | Every hazard closed: the chained-merge case reproduced and fixed, and the survivor rule decided by the user (provenance wins, id breaks ties). See below | ~~M~~ |

**BL-39 · A design cannot be loaded into a running session — DONE** — *found while trying to use the
consumer skills on reflow2's own 96-node design, 2026-07-19.*

Three facts compose into a dead end, each reasonable alone:

1. `reflow2-mcp` takes an **exclusive RocksDB lock** on the graph while serving ([BL-12](#bigger-threads),
   single writer) — verified: `--export` against a served graph fails with `LOCK: Resource
   temporarily unavailable`.
2. The binary has **`--export` but no `--import`**, so a script can read a design out without
   speaking MCP and cannot write one back.
3. The only bulk write path is the `import_graph` **tool**, which takes the entire document as one
   argument.

So a design produced by any means other than the live session — a script, another machine, a
committed export, a backup — can only be loaded by passing the whole document through the tool
boundary. For reflow2's own design that is a 42 KB argument. The practical effect is that the
consumer skills (`where-am-i`, `check-health`, `detect-and-ask`) can only ever see a graph the
session itself built, which is exactly backwards for a tool whose selling point is that a design
outlives the session.

**Done.** `reflow2-mcp --graph-path <dir> --import <file>` is the sibling of `--export`, and takes
`-` for stdin so an export on one machine pipes into an import on another. Upsert, matching the
tool. It reports what landed *and what did not* — an import that quietly skipped half a design would
be the worst kind of success — so `skipped_edges` is printed by name.

The lock stays, because single-writer is the storage model rather than an oversight, but it is no
longer a mystery: the raw RocksDB error ("Resource temporarily unavailable") is translated into
*"another process already has the design graph open… stop that server and run this again."* That was
the actual friction — the failure gave neither the cause nor the fix.

Verified end to end in `smoke_mcp.py`: reflow2's own 116-node design imports from a file and from
stdin, the CLI round trip is byte-identical, a held graph is refused with the explanation, and a
document that is not an export is refused by name.

*What it unblocks.* The consumer skills (`where-am-i`, `check-health`, `detect-and-ask`) run against
the live graph, so before this they could only ever see a design the session itself built. A
committed export, a backup, or a design built elsewhere is now one command away from being the graph
the skills read — which is the point of a design that outlives the session.

**BL-38 · The golden thread has two valid shapes at P3 and the detector accepts one — DONE** —
*[self-host functional design](trials/2026-07-19-selfhost-functional-design.md), 2026-07-19.*

Verified in isolation. `REALIZES` is declared `from: Artifact, to: "*"`, so both of these are
schema-valid, and `link_artifact` invites either by taking any `target_type`:

```
Artifact REALIZES Component  : capability reported unrealized?  True
…plus    REALIZES Capability : capability reported unrealized?  False
```

Modelling *the file realizes the module* — which is how code is actually organised — makes every
capability report `unrealized_capability`, 11 of 33 gaps on reflow2's own design, for capabilities
shipping in the binary that reported them. The connecting path exists and is not walked:
`art:detect -REALIZES-> cmp:detect <-ALLOCATED_TO- cap:detect`. `detect_unrealized_capabilities` asked
only for `incoming(cap, REALIZES)`. **Fixed**: a capability also counts as realized when an artifact
realizes a Component it is allocated to — the indirect form is the coarser claim (the file builds
the part that owns the capability), which is exactly the granularity BL-23 pushes designs toward.
Measured on the design graph: **33 gaps → 16**, and every surviving `unrealized_capability` is one
of the five genuinely unbuilt capabilities — the graph now reports exactly the open backlog with
zero noise. The true case is pinned: artifacts elsewhere, nothing realizing this capability or its
component, still reported.

Same trial, same item, also **fixed**: `dead_end` no longer fires on a subsystem whose only edges
are `CONTAINS`. The design network's CONTAINS-exclusion stands (*decomposition is not
traceability*); the exemption is scoped to **assemblies** — a component containing other components
speaks through its children, which are flagged individually if disconnected. A contained *leaf*
hosting nothing is the true case and still fires; there is a test for each direction. Defects on the
design graph: **36 → 34**.

**BL-5, second pass · `single_point_of_failure` above fixture scale — DONE** — *the
[self-host functional design trial](trials/2026-07-19-selfhost-functional-design.md).* 22 of 36
defects on a 96-node design, post-first-fix: nearly every requirement and mid-level capability named.
The [original fix](#closed) asked whether removal *increases* the count of non-trivial components,
which is the right question about *topology* — and a golden thread is a tree, so most internal nodes
still pass it. It turned out to need a different question, not a threshold: **only things that
operate can fail.** The suggested fix is literally `add_redundancy`, and redundancy is only coherent
for running parts — a second copy of a sentence adds no resilience, and a capability's failure *is*
its component's failure, already reported there. An intent node being an articulation point is the
golden thread working: every Requirement is *supposed* to be the hub of what satisfies it. SPOF
candidates are now scoped to `Component` / `Interface` / `Resource` / `Environment`, on top of the
existing separation test.

Measured: **22 → 4**, and the survivors are exactly the ones judged plausible before the change —
`cmp:service` (all agent access through one surface), `cmp:init` (the only installer), `cmp:export`
and `ifc:graph-export` (the sole core→kit bridge). With this, **every one of the instrument's 16
gaps and 14 defects is true** — the first instrument at zero known-false output. Two of the
surviving defects are themselves worth noting: the `rel:v020 + env:dev` island is reflow2
independently reporting [BL-34](#next-up)'s consequence, and the `cmp:verify` / `cmp:operate`
islands found a genuine omission in the committed design model (the P4/P5 write side has no stated
capability — fix the model, on the record, per [sharpening.md](sharpening.md) §2).

**BL-37 · reflow2 cannot model a process — DONE** — *modelling the coherence loop itself, 2026-07-19
(`tools/model_the_loop.py`, exported to [loop-model.json](loop-model.json), drawn in
[loop-dag.html](loop-dag.html)).*

**Done, to two decisions taken 2026-07-19.** The write side is `add_flow` + `part_of_flow`
(`step_order` was already in the schema); `TRIGGERS` gains a free-form `role` property — a
backward-compatible property addition, counts stay 27/54 — so *feeds* and *forces a resync* are
distinguishable, which was the entire subject of the model. The cycle question was decided as
**report, don't judge**: `flow_report` states a flow's cycles as facts of the process (one
representative per strongly-connected cluster, deterministic), and `circular_dependency` stays
scoped to `DEPENDS_ON` and contracts, where a cycle really is a defect. Two diagnostics stopped
assuming the subject is a product: `concept_without_design` counts a Flow as structure, and
HEAL's `orphan_node` counts flow membership as an anchor. Anything unstated is confessed —
including a `PART_OF_FLOW` edge to a capability that does not exist, which the smoke layer caught
the report silently tolerating (the storage engine accepts dangling edges; only the published
surface shows it). Measured: the loop model's 4 frictions → **0** (`model_the_loop.py` is now the
fifth instrument, non-zero on regression), its defects 10 → 4 with every survivor true — the
remainder is the recorded A14 day-one shape on [BL-27](#bigger-threads), and wider process-aware
diagnostics stay with [BL-16](#bigger-threads). The other four instruments are unchanged. *Original
entry:*

Distinct from the self-host trials, which modelled reflow2's **product** — it has a detect
capability, a component per module. This modelled reflow2's **process**: the DAG of how the phases
feed each other, including the backward edges where the build teaches the design what it is. Its own
operating model is a design, and *design anything* is the claim. Four things got in the way, all
verified by attempting it:

| Friction | Detail |
|---|---|
| **`Flow` has no write side** | Fully specified in `functional.yaml` — `flow_type: process/control_flow/decision_flow`, `entry_point`, `exit_point`, with `PART_OF_FLOW` running Capability → Flow — and there is no constructor in core and no MCP tool. `node::FLOW` appears only in `report.rs` (counted) and edge classification. **The one type meant for "an ordered process that links Capabilities end to end" cannot be created**, and this model is its exact use case. Recurring lesson, eleventh instance |
| **Edge roles are lost** | Forward *feeds* and backward *forces a resync* both had to become `TRIGGERS`, declared `* → *` with no role property. The backward edges are the entire subject of the model and the graph cannot tell them from the forward ones. `PART_OF_FLOW` would not fix this either — it carries membership, not order or direction of influence |
| **Cycles are invisible** | The loops are the point, and `circular_dependency` does not fire: it walks `DEPENDS_ON` and contracts, not `TRIGGERS`. A process model's cycles are its most important feature and nothing reads them. Note the tension — in a *product* a cycle is a defect, in a *process* it is the design, so this is not simply "add TRIGGERS to the walk" |
| **The diagnostics are product-shaped** | `concept_without_design` fired on zero Components. A process has no Components, so the phase detectors assume the subject is a product. Execution evidence for [BL-16](#bigger-threads), which asks whether "design anything" survives contact with a non-software domain — here it does not survive contact with a non-*product* one |

Size **M**: a `Flow` constructor and tool are **S**, but edge roles and process-aware diagnostics are
the real content, and the cycle question needs a decision before code.

**BL-35 · A design claim has no last-confirmed date — DONE** — *[coherent-erosion
trial](trials/2026-07-19-coherent-erosion.md), 2026-07-19. The deepest of the phase-coherence items.*

*The good news first, because it changes what the rest of these are for.* The
[coherent-erosion trial](trials/2026-07-19-coherent-erosion.md) ran the same five fix cycles with
axis-Z discipline — every fix a `record_change` at its own epoch, and the behaviour-changing fix also
updating the **P1 capability**, the design following the build backwards. It works: the design ends
describing what was actually built, **the original intent is still recoverable** from a Snapshot
pinned to the baseline epoch, and every fix is on the record. `designed == released` is reachable
today, and letting the build teach the design costs no intent because Z keeps the past. The
vocabulary was already waiting for this loop — `ChangeType::TestFailureFix` is documented as *"a fix
forced by a failed verification"* and `ChangeType::Resync` as *"a re-sync back to coherence."*

*And the problem.* Run both versions and reflow2 returns **the same verdict**:

| | eroded run | coherent run |
|---|---|---|
| design describes what shipped | no — fiction | yes |
| `detect_gaps` | `[]` | quiet |
| reflow2's verdict | **coherent** | **coherent** |

The entire difference is developer virtue, which is exactly what does not survive cycle 40 of a
release crunch — and exactly what the original reflow was relying on without knowing it.

*The missing concept is a date on the design's own claims.* Structural completeness is all that is
measured — is there a Capability, does something satisfy the Requirement, does an Artifact realize
it — and every one of those is true in the eroded graph. What is absent is **when a claim was last
confirmed against reality**. A description written at the baseline epoch and never revisited while
its artifact drifted five times is a different and worse state than the same description confirmed at
the release epoch, and nothing tells them apart.

**Done — as the confirmation ledger** (`confirm.rs`, `confirmation_ledger` on the surface, a
rollup in `graph_report`). Per capability with built artifacts, three states that were previously
one: **`drifting`** (an observed divergence is unanswered — also a persistent 0.75 gap,
`unresolved_drift`, because the session that reconciled may not be the session that answers),
**`confirmed`** (examined, with the claim history visible: design_holds vs design_updated counts,
design edits, `last_claim_at` from dated claims), and **`unexamined`** (nobody has ever looked —
*no longer the same as confirmed*, which was the whole point). Two supporting facts landed with it:
`DriftEvent.resolved` — declared in the schema with `default: false` and never written by anything,
the **twelfth** recurring-lesson instance — is now flipped by the accept that answers the drift; and
an accept's `CHANGED` edge is marked `accepted_baseline: true`, so a disposition claim is
distinguishable from ordinary change history on the same artifact.

Deliberately *not* built: lie detection. Five `design_holds` claims with zero design edits is the
erosion signature and the ledger makes it legible — but judging whether a specific claim was false
is semantic, and a deterministic detector would fire on every stable design with cosmetic churn
(the `unexpected_coupling` lesson). The ledger reports; the human judges.

Measured: erosion **5/8** ("the design reports how the code moved and how each move was answered" —
the signature line reads *5 drifts, 5 claims, 0 edits*), coherent-erosion **6/9** ("which fix moved
the design" — *1 design-updating accept vs 4 design-holds, cycle 4 is the one, and the ledger says
so*), smoke green with the full drift → gap → answer → ledger loop over the real binary.

**BL-36 · `precedes` is unreachable, so the epoch chain cannot be drawn — DONE** — the `precedes`
tool orders one epoch after another, and the coherent-erosion trial now draws the chain cycle by
cycle and walks it back out of the export: `baseline → fix1 … fix5 → release`. With it the coherent
instrument reached **9/9 — the first instrument fully green**, every YES a genuine read. (Its probe
was nearly shipped as a hardcoded `True` and caught in review — the instrument-accommodation trap
from [sharpening.md](sharpening.md) §4, live again.)

**BL-33 · Accepting drift is one-sided; the drift record overwrites itself — DONE** — *[erosion
trial](trials/2026-07-19-erosion.md), 2026-07-19. The mechanism behind the user's account of
reflow1, and the load-bearing item of the three.*

*The failure is not an event.* It is `write → test → fix → test → fix → … → release`, where every
step is legitimate — a test failed, someone fixed the code — and nobody ever decides to diverge.
Detecting "this file changed" barely helps, because the answer is always *"yes, I know, I fixed a
bug."* **Verified:** five fix cycles on a coherent thread, the fourth quietly widening an
idempotency window from 24h to 7 days, then a release. Afterwards `detect_gaps` returns **`[]`** —
the design describes a system that no longer exists and reports perfect coherence, because what is
measured is whether the bookkeeping is complete, never whether it is true.

*Two halves.*

**Accept is one-sided (M).** Each cycle ends at `set_artifact_checksum` — "an accepted change is the
new baseline" — which updates the code-side baseline and **asks nothing about the design**. That is
locally reasonable and globally fatal. Nothing ever poses the second half: *the code moved, should
the design move too, or was the code wrong?* **Done, to the coherent-erosion trial's specification.** `set_artifact_checksum` now requires a
`DriftDisposition`: `design_holds` (the change carries no design meaning — recorded as a dated
`ChangeEvent` claim, deterministic id so re-accepting the same state is idempotent) or
`design_updated` (naming the `record_change` event from the design-side edit, which is then
`CHANGED`-linked to the artifact — **one change, both sides**, the first `ChangeEvent` in the
codebase originating from the build). A phantom `design_change_event_id` is refused before the
baseline moves — and the refusal caught the coherent trial itself accepting before recording, live.
The claim can still be wrong (the erosion trial's careless actor claims `design_holds` five times,
including the lie), but it can no longer be silent: it is dated, typed and auditable, which is
exactly what BL-35's freshness check reads. Measured: erosion 3/7 → **4/8** (new probe: every
accept answers the second question), coherent-erosion 4/9 → **5/9** ("anything prompted the update"
is now genuinely yes — the tool poses the question at the moment it matters). The principle it was
meant to embody — the capability description is updated to match what was built, or the
divergence is marked a defect in the code. The third option, "accept the file, leave the design
alone, say nothing," is the one that erodes and should not exist. Note this is the first thing in
the codebase that would make a `ChangeEvent` originate from the *build* side rather than the design
side, which is the right shape: a fix is a change, and CHANGE is a first-class axis.

**The record overwrites itself (S) — done.** The mechanism was subtler than first recorded: the id
(`artifact | kind`, no discriminator) didn't overwrite, it **skipped** — `write_drift_event` returns
early when the node exists, intended as dedup for re-observing the *same* unresolved divergence. The
defect was that a **new** drift hashed to the same id and was silently dropped: five drifts left one
event. Fixed by making the observed checksum part of a `checksum_change` event's identity — the
event *is* "the artifact became X while the design believed Y", so re-observing the same X dedups
and a later drift to X′ is a new event. State-shaped kinds (`missing_artifact`,
`undocumented_addition`) stay keyed on artifact + kind: "still missing" re-observed is the same
unresolved divergence. Measured: the erosion trial retains **5 events for 5 drifts**, and its probe
was tightened from `> 0` (which one surviving event weakly satisfied) to an exact count. Axis Z's
*never overwrite the past* now holds on the as-built side; "drifted once" and "drifted N times" are
different graphs, which is the data BL-35's freshness computation needs.

**BL-34 · There is no as-released view, and no vocabulary for one — DONE** — *same trial.* Checked two
ways: **`DEPLOYED_TO` (Release → Environment) is the only edge in the schema involving `Release`.**
Nothing links a Release to the Artifacts or Components it shipped, though `Release`'s own
extraction hint says *"A packaged, operable version of some Components/Artifacts"* — the intent is
prose with no edge to carry it. So *"does what we released match what we designed?"* is not an
unimplemented query; it is inexpressible. reflow2 has as-designed and a partial as-built, and the
third view — the one the user actually lives with — has no structure at all.

**Done.** `INCLUDES` (`Release → [Artifact, Component]`) is edge type **54** — the first edge-type
addition since the stamp existed, so the BL-19 mechanism now applies for real: a graph written by
this schema is **refused by older binaries**, loudly, with what wrote it. The upgrade order in
SETUP.md matters for the first time. `as_checksum` on the edge freezes the artifact's hash *as
shipped*, because the artifact node's own checksum is the live drift baseline and moves with every
accept — without the frozen copy a past release's manifest would quietly rewrite itself, the axis-Z
sin again. Write side `release_includes`; read side `release_report` — shipped artifacts with
cut-time checksums, capabilities covered (both P3 shapes), **`built_capabilities_not_covered` as
the as-released diff**, deployments. A new gap, `unreleased_component` (0.5), fires for a built
component no release includes — double-gated on releases existing *and* contents being modelled,
so day one of the first Release node is not a flood (the ophyd-A14 lesson). `pin_at_epoch` is also
on the surface now (thirteenth recurring-lesson instance: `AT_EPOCH` is `from: "*"` and the core
fn existed with no tool), so a Release joins its `release_cut` epoch. Three pinned history tests
flipped honestly: BL-1's own example pair — *"nothing models Release → Component"* — now has its
exact fit, which is the trial's question answered two items later. Measured: phase **10/13** (P5
1/2), erosion **7/8**, coherent-erosion **8/9** — the single remaining coherent miss is BL-36's
`precedes`.

**BL-30 · The later phases measure bookkeeping, not reality — DONE** — *[phase-coverage
trial](trials/2026-07-19-phase-coverage.md), 2026-07-19. The direct answer to "how do we know
reflow2 doesn't repeat reflow1?"*

**Verified, three times, twice in isolation from the harness.** `build_without_verification` fires
when a capability has no `Verification`. Attach one and set its status to `failing`, and the gap
**closes**:

```
no verification at all      : ['build_without_verification', 'no_deploy_operate']
a verification that FAILS   : ['no_deploy_operate']
```

The gap asks *"How will you confirm `<Capability>` actually works?"* — and is answered by a test
proving it does not. The failure is invisible everywhere else too: with status written as `failing`,
`detect_gaps`, `detect_defects` and `graph_report` are byte-identical to the `passing` case.

The general form, and the reason this is the thread's most important item: **the P4/P5 detectors ask
whether a node exists, never what it says.** A design that counts test nodes and ignores test results
is precisely one you may as well have ignored once building started.

Two pieces. **S — done.** A `failing` verification now raises `failing_verification` at severity
0.8 — above every absence-shaped gap, because a requirement nothing satisfies is work not started
while a failing check is work *proven broken* — anchored to both the check and what it checks.
`build_without_verification` still closes when the check exists (the "how will you confirm this?"
question *is* answered); the difference is the silence is now filled with the right signal instead
of nothing. And `verification_coverage` counts a check that **passes**, not one that exists —
`planned`, `failing`, `skipped` and `blocked` all mean "not currently confirmed". Measured:
`phase_trial` P4 1/4 → 2/4, `erosion_trial` 2/7 → 3/7 (whose coverage probe also went from a
hardcoded fail to a genuine check). Passing and failing graphs are no longer byte-identical to
DETECT, which was the headline miss. **M — done, 2026-07-19** —
`reconcile_verification` (`verify.rs`), the P4 sibling completing the reconcile family: the agent
supplies what the run actually reported (`passed`/`failed`/`skipped` per check; anything else
rejected by name, the batch survives) and the graph names each divergence from what it believed.
"Recorded `passing`, run reported `failed`" — believed proven, actually broken, the reflow1
failure in miniature — sorts first and records at severity high. Divergences are persistent
`unresolved_drift` gaps with P4-appropriate advice, auto-resolved when a later run agrees; the
event identity is the (declared, observed) pair, so flapping history stays visible per axis Z.
A partial run is never read as absence; `exhaustive` names the passing/failing claims the run
did not cover. Measured: **phase trial 13/13 — the first fully-green run of the instrument that
exists to measure the failure that sank the original reflow**, and its P4 probe now injects the
divergence rather than checking a tool exists. With BL-9, all three feedback loops (P3/P4/P5)
now close; this is also adoption's dynamic-analysis receptor (see the RE-lifecycle mapping under
[BL-27](#bigger-threads)).

**BL-31 · A `status` field is a claim nothing checks — DONE** — `status_contradiction` (0.70,
self-contradiction family: below reality-contradiction at 0.75/0.8, above absence). Scoped to the
two unambiguous cases — a Capability `verified` that no *passing* check verifies, and a Requirement
`met` that nothing satisfies. The second matters doubly: `met` silences `unsatisfied_requirement`
by design, so before this a lying `met` was invisible to everything. Deliberately not extended to
`realized`-without-artifact, which is already an absence gap — double-reporting would be the
DETECT/HEAL double-count in a new costume. **Its first catch was our own model**: `cap:kit` claimed
`verified` in the committed design graph and nothing automated checks the installer — ruled per
[sharpening.md](sharpening.md) §2 (the status was wrong, downgraded to `realized` on the record),
the second true self-report and the first lie caught in our own graph. Measured: phase **11/13**
(P4 3/4). *Original entry:* — *same trial.* `Capability.status` set to
`verified` on a capability with no `VERIFIES` edge raises nothing. `unverified_capability` fires, but
it fires either way — "this is unverified" is not "the design contradicts itself", and only the
second is a coherence failure. Same for `Requirement.status = met` with nothing satisfying it, and
`Component.status = realized` with no Artifact. Sharpened by [BL-27](#bigger-threads), which made
these fields easy to write for the first time. A `status_contradicts_structure` detector, **S**, and
it belongs in DETECT — the answer is either "fix the status" or "fix the structure", which is a
question for the user.

**BL-32 · A running MCP server silently serves a stale surface — DONE** — *same trial, found by nearly
running it against the wrong binary.* Rebuild `reflow2-mcp` mid-session and the already-running
server keeps serving the surface it started with: tools added since are absent, detectors keep the
old behaviour, and nothing says so. Distinct from [BL-18](#bigger-threads), which compares an
installed kit against the remote HEAD — this is process-lifetime skew and hits agents and developers
mid-session. `tools/smoke_mcp.py` cannot catch it by construction, since it spawns a fresh binary per
run — the fourth "a client we wrote agreed with itself" in this repo's history. **Done.** `graph_report` carries `served_by` — the crate version compiled in, plus the binary's
mtime (best-effort, `None` over a guess) — so a session can see it is talking to a binary older
than the code around it, and the upgrade doc's step 4 makes checking it the post-restart ritual.
The consistency check that pins it (handshake version == report version) immediately caught a
pre-existing bug: `Implementation::from_build_env()` expands in **rmcp's** build env, so the server
had introduced itself as the MCP *library's* version ("2.2.0") since the surface existed — the one
field a client could see, wrong all along. It now reports its own name and version.


**BL-29 · `apply_heal` trusts the proposal, and merge loses data silently** — *found 2026-07-19
while scoping [BL-27](#bigger-threads)'s duplicate detection; the reason `possible_duplicate` is a
DETECT gap and not a HEAL defect.*

**The headline was verified by running it, and is fixed.** A hand-crafted `HealProposal` — a made-up
`issue_id`, a `Merge` naming two capabilities with no `DUPLICATES` edge, which `detect_defects`
reported only as `OrphanNode` — was accepted and applied: `applied=true, operations_applied=1`, node
gone. `ApplyHealReq` deserializes caller JSON straight off the MCP surface, so any client could do
it, and a merge has no snapshot and no undo. `apply_heal` now re-derives what HEAL would propose for
the graph as it stands and refuses anything that does not match, **before any write**. The
issue→operation mapping is shared by propose and apply so the two cannot drift.

Related and worth remembering: `requires_human_review` is computed per-*proposal* and `apply_heal`
has never read it. It reports that generative stubs exist; it is not and never was a gate on
applying the structural half.

| Hazard | Status |
|---|---|
| A proposal HEAL never made is applied verbatim | ✅ fixed — refused before any write; stale proposals fail the same way |
| `remove`'s node properties are discarded entirely, so name/description/status vanish with no report | ✅ fixed — reported in `HealReport.discarded` |
| Edges to nodes absent from the index are dropped with no report | ✅ fixed — reported in `discarded` |
| `create_edge` is an upsert on `(graph, type, from, to)`, so where both nodes had the same triple, `remove`'s edge properties overwrite `keep`'s | ✅ **reported**, not prevented — `discarded` names the collision. Preventing it means deciding which side wins, which is a merge-policy question, not a bug fix |
| `DUPLICATES` declared `from: "*" to: "*"` yields a schema-valid cross-type merge | ✅ fixed in code — refused at proposal time with a reason. **The schema is still `*`/`*`**; narrowing it is the tighter fix and was left alone, since it would reject edges existing graphs may already hold |
| The node-type index is built once before the loop, so chained merges (a↔b, b↔c) re-point onto a node that no longer exists | ✅ **reproduced, then fixed** (2026-07-20). Worse than code-read suggested: both merges are individually sanctioned, the dangling edge is *accepted* by the storage layer, and the report said `applied=2, verified=true` over a corrupt graph — silent corruption with a green verification, one hash-ordering away from `propose_heal`'s own output. Now: propose emits one merge per chain (rest deferred with the reason stated), apply refuses any proposal whose merges share a node before a single write, and a third-party `DUPLICATES` edge is re-pointed onto the survivor so the chain's unresolved claim survives and the loop converges one round per link. Two silent drops found in the same repro also fixed: the chain claim vanishing with the merged node, and a real pair-joining edge dying without a `discarded` entry |
| Atomicity is per-operation, not per-proposal: a three-merge proposal failing on the second leaves the first committed | ⬜ open, code-read — but **no known trigger remains**: the shared-node refusal makes a proposal's merges node-disjoint, the cross-type guard makes every re-pointed edge type-valid, and unknown endpoints are discarded rather than errored, so a mid-proposal failure now needs a storage-level error. Closing it fully means one batch per proposal, which would make mid-batch reads stale — trading a real correctness property for a theoretical one. Revisit only with a reproduction |
| The survivor is chosen by lexicographic id (`canonical_pair`), not by connectivity or completeness — the better-connected node may be the one deleted | ✅ **decided by the user and built** (2026-07-20): **provenance wins, id breaks ties.** A merge keeps only the survivor's properties, so the choice decides whose words are kept — and the old rule let an `inferred` stub delete an `authored` node's text on id order alone. The rank follows how directly a human stands behind the text (`authored` > `planned` > `imported` > `reconciled` > `inferred` > `healed`); equal rank falls back to the smaller id, so the choice stays fully deterministic and pre-provenance graphs behave exactly as before (absent property = the schema default, `authored`). Connectivity/completeness rules were considered and rejected as unstable — one bookkeeping edge would flip the winner. Pinned in three directions: authored beats inferred against the id order, the graded order (inferred > healed), and the tie fallback |

With that, every hazard on this item is closed — the last one by the user's survivor-rule
decision (option 2 of the alternatives put to them). **BL-29 is done.** The decision itself
belongs in the design graph as a Decision node; add it in the first live-server session
alongside the stub-survivor reconciliation.

## Closed

Kept as a short pointer so a stable id never dangles; the detail is in the CHANGELOG.

- **BL-28 · Every `JsonValue` tool parameter was unusable from Claude Code** — done. Six params
  (`gap_to_prompt.gap`, `apply_heal.proposal`, `import_graph.document`, `create_node.props`,
  `create_edge.props`, `reconcile_artifacts.observed[]`) published an untyped schema, so each
  client guessed: grok build sent an object, Claude Code sent a string, the string was rejected.
  Now declared as JSON objects; a stringified object is still refused rather than accepted, since
  taking both shapes would be the silent fallback rule 4 forbids. The regression guard asserts the
  published schema (no advertised property without a type) — the behavioural layers were all green
  while the bug was live. Detail: [trial](trials/2026-07-18-selfhost-genesis.md) §1.
- **BL-22 · Skills are not reliably discoverable** — done. The kit installed `.grok/skills/`
  alone, the narrowest-reach of four harnesses, so a project opened in Claude Code had an
  AGENTS.md naming seven skills the agent could not load. `reflow2_init.py` now installs to
  `.claude/skills/` (read by Claude Code, OpenCode and Copilot) and `.grok/skills/`, and writes
  `.mcp.json`, `opencode.json` and `.vscode/mcp.json` from one generator. Configs are merged, not
  overwritten — which also fixed a silent failure where a project that already had any MCP server
  never got reflow2 installed while the run reported success. Tables and the reasoning:
  [skills/README.md](skills/README.md).
- **BL-20 · Graph export / import** — done. `export_graph` / `import_graph` in core and on the
  surface. Deterministic throughout — node types, ids, edges and property keys all sorted, which
  is why the exported types use `BTreeMap` rather than the store's `HashMap` — so two exports of
  an unchanged graph are byte-identical and a backup directory under git shows *what changed in
  the design* rather than a fresh blob each run. Import is upsert and atomic: a document that
  fails validation leaves the graph untouched, and an edge whose endpoints are missing is named
  rather than dropped. The document carries a `GraphStamp`, so it says which reflow2 wrote it.
  This is the migration mechanism BL-19 wanted: export with the old build, import with the new.
- **BL-21 · The agent can report its own friction** — done. A `report-friction` skill plus the
  trigger in the consumer AGENTS.md, since a skill alone is not reliably found (BL-22). Redaction
  is the load-bearing part: a friction report naturally quotes the graph, and the graph is the
  user's design, so the skill reports reflow2-shaped facts — tool, argument *shapes*, node
  *types*, counts, masked errors — and asks before including anything of theirs. It never files
  without asking, searches for duplicates first, and degrades to a local file when `gh` is absent
  or the repo is unreachable, **which is the normal case: the repo is private**. Also folded skill
  frontmatter validation into `reflow2_init.py`, because a malformed `name` makes a skill fail to
  load with no error anywhere.
- **BL-25 · An answered question stays visible while its gap is open** — done. `open_questions`
  now returns two kinds: `asked` (still waiting) and `answered` **whose gap is still open**, with
  the reply attached. Answering settles nothing by itself — either the answer gets written into
  the design and the gap closes, or the gap is acknowledged; until one happens there is something
  outstanding and the list says so. A question whose gap has closed or been acknowledged drops out
  of the list but stays in the graph. Verified on reflow2's own design: the third session now sees
  the question and the reply, and acknowledging takes it to **0 gaps, 0 outstanding, 1 reviewed**.
- **BL-4 · Asked questions outlive the session** — done. `gap_to_prompt` was the only tool that
  never touched the graph: it phrased a question, returned it, and forgot, so the next session
  re-derived the same gap and asked again. Its serve pass now records a `Question` node at a
  derived id (`question:{gap hash}`), `ASKS_ABOUT` the nodes the gap concerned, with the wording
  the user actually saw. `open_questions` / `answer_question` / `withdraw_question` are on the
  surface, and `where-am-i` reads them first. **New node type** — 27 node types, 53 edge types —
  purely additive, so per BL-19 it is safe for existing graphs. Re-asking updates the wording but
  cannot reopen an answered question; there is a test for that.
- **BL-5 · `single_point_of_failure` measured against the baseline** — done. Not the cause the
  self-host probe guessed (it blamed the `≥2` threshold, by analogy with `surprises.rs`).
  Reproducing the shape showed the real one: the test asked whether ≥2 non-trivial components
  exist *after* removal, which assumes a connected design. One unrelated island already satisfies
  that, so every articulation point elsewhere reported — and attaching the island cleared them all
  at once, which is exactly the trial's *"15 defects vanished when I added two bookkeeping
  edges."* It now asks whether removal **increases** the count. reflow2's own design: 8 defects → 2,
  both true.
- **BL-24 · A Component the Project contains is not floating** — done. `orphan_level` only
  recognised a *Component* parent, and the Project carries no `Component.level` because it sits
  above all of them — so the shape the tools lead you to (a Project holding a few subsystems)
  reported one false gap per subsystem. The Project now counts as a parent; a component nothing
  contains is still an orphan, and there is a test for each direction. Together with BL-23 this
  took reflow2's own design from **25 gaps to 1**, and that one is true.
- **BL-23 · Per-file verification coverage is counted, not asked** — done. One `VERIFIES` edge
  per source file was 22 of 25 gaps on reflow2's own design, on a crate whose capabilities are all
  tested. The rule was not wrong, it was loud, and volume is what makes a list get skimmed.
  `graph_report` now carries a `Verification coverage` line and the gap is gone. Measured on the
  same 119-node graph: **25 gaps → 3**, of which one is true and two are BL-24.
- **BL-6b · `unexpected_coupling` demoted to a signal** — done. The decisive fact was not the
  trials but the spec: [gap-surfacing.md](gap-surfacing.md) names `orphan_node`, `dead_end`,
  `disconnected_cluster` and `single_point_of_failure` as the structural gaps — this was never
  among them, having been volunteered by the graph-analysis work. It is now reported by
  `graph_report` under its own heading, which already existed, so no information was lost. Two
  earlier rounds of tightening had not stopped it firing on correct architecture; an `Interface`
  bridges two clusters by construction, so modelling contracts as instructed made the detector
  penalise every one. `reviewed_gaps` now reports acknowledgements whose detector has been
  retired rather than dropping them, since a trial had already accepted one.
- **BL-2 · Expose `contain_component`** and **BL-3 · `Requirement.status` reachable** — done
  `9ab3da3`. Both needed more than the entry said. BL-2 also had to expose `Component.level`:
  shipping the containment alone would have flagged a false `level_mismatch` on every nesting,
  since everything defaults to `component` — worse than the silence it replaced. BL-3 also had to
  fix HEAL, which unlike DETECT ignored a `dropped` requirement, so marking one would have
  silenced half the system while the other half kept nagging. Recorded as **WS-7**/**WS-8**.
- **BL-6 · Split `unverified_capability`** — done `9ab3da3`. Artifacts now report as
  `unverified_artifact` with wording of their own; detection is unchanged, because proving a
  capability works still does not prove *this file* delivers it. The capability key is frozen
  deliberately: gap ids hash it and acknowledgements are stored under the resulting id, so a
  rename would silently expire every acknowledgement and orphan the Decision where neither
  `detect_gaps` nor `reviewed_gaps` looks. A test pins both keys.
- **BL-1 · Schema discovery tool** — done `9440929`, consumer kit `f00fac7`. `describe_schema`
  plus rejections that name the alternatives. The design turned on one detail worth remembering:
  `EdgeEndpoint::accepts()` returns true for the `*` wildcard, so the naive answer to the trial's
  question would have been `DEPENDS_ON` — the very edge it chose and distrusted. Matches are
  labelled exact vs wildcard for that reason. Recorded as **WS-6** in the coverage matrix.

## Bigger threads

**BL-27 · Adopting a system that already exists** — *user, 2026-07-18.* "reflow2 was designed for
greenfield projects... hoping a `/reverse-engineer` skill would allow you to fill in the graph
based on what's already there." Two sub-problems named with it: codebases with no requirements
documentation, and codebases too large to model in one pass.

All three brownfield trials —
`ophyd-service` (private trial record) (399 files, ~110k LOC),
`3dtictactoe` (private trial record) (~20 files) and
[reflow2 on itself](trials/2026-07-18-selfhost-genesis.md) — had to run GENESIS
backwards, and each recorded the same entry-point finding independently. Call the skill **`adopt`**
rather than `reverse-engineer`: producing the graph is one output, but the job is bringing an
existing system under design control, and it is the sibling of `genesis`, not of a code tool.

*The seeding order inverts, and the gap ranking assumed it hadn't.* **Fixed.** GENESIS deliberately
stops before P2 so `concept_without_design` fires as the productive first gap ("how should this be
structured?"). In brownfield the Components are the only thing that indisputably exists, so that
detector fired at severity **0.7 — above the genuinely valuable gap at 0.6** — and an agent working
the list top-down did the useless thing first. It reproduced on a 20-file project as well as a
110k-LOC one, so it is a property of the path, not of scale. The
[self-host run](trials/2026-07-18-selfhost-genesis.md) added `build_without_verification` (0.65)
firing the same way — "no way to confirm any of it actually works" of a repo with 15 test files and
a smoke test — so the top **two** gaps outranked the third.

*The fix was not the one this entry originally proposed*, and the difference is worth keeping.
The entry blamed the shared maturity inference — both are `scope: phase` detectors reading a
node-type census — and called that the thing to fix. But the inference is *correct about the
graph*: `components == 0` is true, and the `aidrone trial` (private trial record)
recorded the greenfield behaviour as **worth not regressing** ("the skill and the detector agree,
the gap arrives as a question rather than a complaint"). Suppressing the detector would have broken
a case a trial called correct.

The real defect was comparing two incommensurable numbers. Phase nudges carry fixed literals;
`unsatisfied_requirement` computes `0.5 + priority_bump`, which for the default `medium` is exactly
the 0.60 the trials saw — and until [BL-28](#closed) no client on one major harness could write
`priority` at all, so the losing number was a default nobody chose.
[gap-surfacing.md](gap-surfacing.md) already had the distinction: discipline 8 names *retroactive*
(gap-driven) versus *proactive* ("here's what comes next") and puts phase-coverage in the proactive
group, and discipline 3 says concrete beats abstract. So the sort now bands on **anchoring**: a gap
naming nodes describes something wrong *now* and outranks a project-level nudge about what comes
*next*, with severity ordering within each band. Greenfield/brownfield-neutral, and the nudge is
demoted rather than suppressed — with nothing anchored to report it is still the first thing asked.
Pinned in both directions by `tests/detect.rs` and over the real MCP path in `smoke_mcp.py`.

*And the phase problem is not brownfield-only.* Ophyd A14 already reports HEAL emitting maximum
noise on a mid-construction graph, and proposes suppressing allocation-orphan defects when
Component count is 0. The self-host run reproduces that on the **greenfield** path at 18 nodes —
following GENESIS's "do not create Components yet" yields one `orphan_node` per seeded capability,
so genesis → check-health flags a graph that is exactly what genesis prescribed. So A14's fix
should not be scoped to an `adopt` mode; it fires on any project on day one. Related: on that graph
`propose_heal` returns 0 mechanical operations and 14 awaiting generation, so `check-health` has
nothing to apply at all until the LLM backends land.

*Requirements must not be inferred from the implementation.* A requirement backed out of the code
that implements it is satisfied by construction, and a graph of those can never say anything.
3dtictactoe is the controlled proof in the other direction: its one high-value finding —
`game_mode='level_assigned'` validated, stored, and **never read again** — came from
`description.txt`, a source *outside* the code, and turned on the discipline *do not create a
`satisfies` edge you cannot point at code for*. That gives the division of labour:

| Layer | Source | Note |
|---|---|---|
| Capability, Component, Interface | the code | satisfied-by-definition is fine — this is the *as-built* view ([reflow-v3-nuggets.md](reflow-v3-nuggets.md)) |
| Requirement | anything **but** the implementation | the user; tests (a test is a written-down expectation); READMEs and spec files; issues and commit messages; config and deployment; and error handling, validation, retries and locking, where the unwritten NFRs live |

Ophyd is the caution against trusting a found document: its traceability matrix was another org's
PDR, 7 of 25 rows out of scope, and it **omitted device locking — arguably the system's central
correctness property**. An agent seeding only the matrix produces a graph whose most important
invariant is absent. A second caution from the same trial: inferring component *identity* from
source comments produced a phantom external system, because stale naming outlives stale code.
Structure from imports and calls; never from prose.

*Scale is a granularity problem, not a context problem.* Neither trial ran out of context. Ophyd's
~110k LOC modelled as **~78 nodes**: 124 REST endpoints → 9 Interfaces (one per OpenAPI contract,
*not* per endpoint), 1,573 test functions → 8 Verifications, and the vendored queueserver fork —
75k of the 110k LOC — deliberately left as **one opaque Component**. [BL-23](#closed) is why: one
Artifact per source file made 22 of 25 gaps `unverified_artifact`, 88% noise from a *complete*
model. The user's instinct to explore incrementally is right, but the first pass should be
**breadth at deliberately coarse granularity over the whole repo**, because the payoff findings in
both trials were structural and came from breadth, not depth — a *critical* `circular_dependency`
between two ophyd services that the project's own architecture docs never name, surfaced only
because both sides of two Interfaces were recorded, and 3dtictactoe's absent `satisfies` edge.
Then deepen **on demand** — the subtree the user is actually working in — rather than by rotation,
so coverage tracks value and there is a natural stopping point.

*Incremental adoption is blocked until the frontier is modelled.* A partial graph emits gaps
indistinguishable from real ones. Ophyd finding 6 states the general form: the tool "cannot yet
tell 'no capability delivers this' from 'nobody has drawn the edge yet'." Finding 14 adds that the
detectors have no notion of a graph mid-construction — following `check-health` literally would
have fabricated Components over a graph whose real structure had simply not been entered yet, and
the operator declined to run `apply_heal`. Marking unexplored regions so detectors stay quiet
there is a **precondition** for the deepening stage, not a refinement of it. The
opaque-Component treatment of the vendored fork is the existing precedent.

*The orphan-Capability fix, and two things it deliberately left alone.* `unmotivated_capability`
is the mirror of `unsatisfied_requirement`, and its severity reads `Capability.provenance` — 0.55
authored, 0.70 `inferred`. Ophyd asked for it to outrank `unsatisfied_requirement` *"on a
brownfield graph"*, and a fixed number cannot honour that qualifier: the same structure means a
half-finished thought on one path and a feature in production nobody asked for on the other.
Provenance is exactly what separates them, which is the first thing to consume that property.

1. **HEAL was not given the symmetric check**, though it is blind in the same direction. Two
   reasons, and they should be revisited together rather than piecemeal. There is no mechanical
   repair for "no requirement asked for this" — the proposal would be one more
   `requires_human_review` stub on a graph where `propose_heal` already returns 0 applicable
   operations and 14 awaiting generation. And DETECT/HEAL double-counting is *already* a recorded
   complaint (ophyd 15 / 3dtictactoe 10, reproduced a third time in the self-host run); adding a
   fifth pair would deepen it. This is the docs' own division — *HEAL fills structure; Gap
   Surfacing elicits meaning* — and a missing requirement is meaning. If the double-count is fixed
   first, revisit.
2. ~~**A graph with capabilities and zero requirements reports nothing**~~ — **built,
   2026-07-19**: `design_without_intent`, the fifth phase-coverage nudge, at 0.72 — the top
   nudge on an adopted graph, exactly ophyd finding 1's ask (*"the first gap should be about
   missing intent, not missing structure"*). One project-level nudge, never one per
   capability; it yields the moment a requirement exists; the wording directs intent to
   sources **outside** the implementation, per this thread's core discipline. Verified over
   the live binary: on a capabilities-plus-component graph with zero requirements the gap
   list leads with the anchored gap, then this at the top of the nudge band.

*Duplicate detection: HEAL's rule computed nothing.* **Fixed**, and the root cause is a fresh
variant of the recurring lesson — not *unreachable on the surface* but **reachable and hollow**.
`heal.rs` iterated existing `DUPLICATES` **edges**, so it reported a conclusion somebody had already
reached and recorded, and could never fire on a duplicate nobody had found — which is every
duplicate an adoption pass exists to discover. That is
[gap-surfacing.md](gap-surfacing.md) discipline 1 verbatim, the trap it names as storyflow's
biggest: *detectors read computed signals, not raw edge-name filters* — "the detector was DEAD on
live data while looking correct."

The computed half is `possible_duplicate`, and it landed in **DETECT, not HEAL**. Three reasons,
and the first is the serious one: `HealCategory::Duplicate` maps to an *applicable* `HealOp::Merge`
that `apply_heal` executes — deleting a node and re-pointing its edges, with no snapshot and no
undo. Merge is content-free and safe only *because a human asserted the endpoints*; feeding a
heuristic into that path would let the machine delete a component it merely suspects. Second, a
HEAL issue cannot be dismissed — gaps can be acknowledged, defects cannot — and `unexpected_coupling`
([BL-6b](#closed)) is the cautionary tale of a detector firing on correct architecture with no way
to make it stop. Third, "are these the same thing?" is meaning, and the docs' own division is that
HEAL fills structure while gap-surfacing elicits meaning.

So they compose instead of overlapping: DETECT asks, the user confirms by drawing the `DUPLICATES`
edge, and HEAL's existing merge — whose "endpoints known" precondition now genuinely holds —
repairs it. A pair already carrying the edge is skipped, so nothing is double-counted.

The rule is structural (≥2 shared capabilities, Jaccard ≥ 0.8 over allocation sets), which needs
nothing deferred. [heal-process.md](heal-process.md) plans duplicate detection on
`resolution: fuzzy_then_vector`; that needs the deferred `EmbeddingBackend` and finds a different
population — things *described* alike, where this finds things *wired* alike. Complements, not
rivals. Scoped to Components deliberately: two Capabilities satisfying one Requirement is
decomposition, the normal case, and a rule there would fire on almost every correct design.

*A skill alone would ship a graph that lies.* Five fixes gate it, and each is the recurring lesson
below again:

| Blocker | Evidence | Size |
|---|---|---|
| ~~`add_capability` hardcodes `status: "planned"`~~ — **done** | ophyd's 15 shipped, under-test capabilities made the graph "assert that a production system is entirely unbuilt". Optional `status` at creation plus `set_capability_status`; nothing hardcoded it, the constructor never set the property and took the schema default | S |
| ~~`detect_gaps` walks Requirement→Capability only, so an **orphan Capability is never reported**~~ — **done (DETECT)** | "in greenfield that direction is rare… in brownfield it is the dominant direction of error" — a feature in production no requirement justifies is exactly what an adoption exercise is for. Now `unmotivated_capability`; see the note below on why HEAL was deliberately left alone | M |
| ~~No duplicate detection~~ — **done** | did not fire on a textbook duplicate; "duplicate implementations are *the* characteristic brownfield defect". Now `possible_duplicate`, computed from shared allocation sets and **asked** rather than repaired — see below | M |
| ~~`concept_without_design` severity ordering~~ — **done** | above. Fixed by banding the sort rather than touching the detectors: a gap that names nodes outranks a project-level phase nudge, severity within each band | S |
| ~~Provenance has nowhere to go~~ — **done** | ophyd smuggled `[EXTERNAL — …]` into statement text, "which is not queryable" | S |

That last one had a cheap answer worth taking regardless, and it is taken. The schema's mechanism
was `Fragment.provenance` (its enum already includes `inferred`) plus a `YIELDED` edge — the
intended pattern, but 2 writes per node with no bulk tool. A `provenance` **property** on
`Requirement` / `Capability` / `Component` / `Interface`, reusing that same enum, is
backward-compatible: adding a node or edge *type* bumps `GraphStamp` and makes older binaries
refuse the graph, but adding a property does not ([BL-19](#bigger-threads)). Confirmed — the counts
stay at 27/53. `set_provenance` writes it incrementally and `import_graph` carries it at create
time, which is the bulk path this thread already points an adopt pass at.

Related, for whoever picks this up: `import_graph` is the only bulk write path and is an atomic
upsert, so an adopt pass should build the export document and import it once — 3dtictactoe spent
~60 MCP calls on 33 nodes.

*The conversion step itself, probed for real* — *2026-07-19, installing into a scratch repo shaped
like every brownfield target (own `AGENTS.md`, own `.mcp.json`, source tree).* The earlier note
here — "cannot install into a repo that already has its own AGENTS.md; needs `--skills-only`" —
is **stale and corrected**: the sidecar path works. The install lands clean: the project's
`AGENTS.md` untouched, kit instructions to `REFLOW2.md`, skills to all four harness locations,
the existing `.mcp.json` merged not overwritten. Three real defects were found — **all three
fixed 2026-07-19**, verified by re-running the probe (fresh install, second-run idempotency,
greenfield unregressed, `--check` consistent):

1. ~~**Nothing points at `REFLOW2.md`**~~ — BL-22's sibling lesson verbatim: shipping the file
   is not shipping the capability. Fixed by the same rule as the merged MCP configs: one
   marked pointer line appended to the project's own instruction file, idempotent by content,
   reported — never overwritten. **Widened 2026-07-20** after the storyflow trial found the
   first fix protected the wrong filename: the pointer now goes into *every* convention the
   project has (`AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, copilot-instructions, cursor/windsurf
   rules), because storyflow carries `CLAUDE.md` and no `AGENTS.md`, so the installer saw
   nothing to protect and left the file Claude Code reads first with no mention of reflow2.
2. ~~**`.reflow2/` is not gitignored**~~ — the installer had no `.gitignore` handling at all,
   so a converted repo started tracking a RocksDB directory. Now appended or created,
   idempotent, with the reason in the comment: the graph is machine-local state; the durable
   record is an export.
3. ~~**The closing "Next:" text is greenfield-only**~~ — now branches on **the project**
   (a bounded source-file count), not on whether reflow2 wrote a sidecar. A repo with code
   gets the `adopt` path with its evidence stated; an empty directory gets genesis; and an
   *update* whose graph is still empty gets the adopt hint too — the case that would
   otherwise repeat the failure for anyone who installed before the skill shipped.
   **Rewritten 2026-07-20**: the first version keyed off our own install artifact, so
   storyflow — 2,643 files — was told to describe what it wanted to build.

Converting a project is now: build/point at the binary (BL-15's published-binaries gap is the
remaining wall for machines without a checkout), run `reflow2_init.py`, open the agent. Then
the graph is empty and everything after that *is* this thread's skill. The first gap an
adopted graph raises is `design_without_intent` — built the same day, see below.

*The accepted reverse-engineering lifecycle, mapped* — *user, 2026-07-19, from research into
standard practice.* Across hardware and software the accepted process is two stages —
**redocumentation** (break the existing product down) and **design recovery** (deduce the
original concepts) — through five steps: information gathering → disassembly/scanning →
analysis (static *and* dynamic) → modeling & reconstruction → validation. The user's framing
of the hard case: large codebases with no requirements and no record of why choices were made —
*"you only get what you see."* The mapping onto what this thread already holds, and what it
exposes:

| RE lifecycle step | reflow2 mechanism | Status |
|---|---|---|
| **Information gathering** | The division-of-labour table above: requirements from anything *but* the implementation; found documents trusted per the ophyd caution (its PDR omitted the system's central invariant); sources recorded as Fragments with provenance — the provenance viewpoint renders exactly this ledger | mechanisms landed (BL-27 blockers, BL-40) |
| **Disassembly / scanning** | For source-available software the disassembler is the repo read: structure from imports and calls, never prose (the phantom-component caution); breadth at deliberately coarse granularity (ophyd: 110k LOC → ~78 nodes, vendored fork opaque); one `import_graph` for the bulk write | discipline recorded above; the skill must encode it |
| **Analysis — static** | allocation/coupling/`possible_duplicate`/`hierarchy_issues` over the scanned structure | landed |
| **Analysis — dynamic** | **the gap this framework exposed — now closed.** All three brownfield trials were static-only. The receptors exist end to end: run the tests → `reconcile_verification` (BL-30, done 2026-07-19 — the typed way in, divergences named and persistent); run the thing → `reconcile_deployment` (BL-9). The adopt skill's "run it and record what you saw" phase has its full machinery | landed |
| **Modeling & reconstruction** | The graph *is* the model. Design recovery deliberately terminates at the human: a requirement inferred from its implementation is satisfied by construction, so recovered intent is marked `inferred` and `unmotivated_capability` routes "why does this exist?" to the person — recovered rationale lands as Decisions, provenance-marked. *You only get what you see* becomes a property of the graph: it confesses what it cannot know instead of improvising past it | landed (the projection doctrine, BL-40) |
| **Validation** | **the second exposure — the current plan ends at "deepen on demand" with no closing step.** The validator is the reconcile family plus the detectors: checksums match (`reconcile_artifacts`), tests agree (P4), deployments agree (`reconcile_deployment`), and every gap the model fires is either true of the system or an error in the model — which is precisely how the trials scored themselves. The skill should end by running it and reporting the verdict | mechanisms landed; the skill must close the loop |

The two-stage split lands on an existing line: **redocumentation** is the as-built layer
(Capability/Component/Interface from code — satisfied-by-definition is fine there), and
**design recovery** is the intent layer, where the *"never infer a requirement from the
implementation"* discipline is exactly the stage boundary. reflow2's position, sharpened by
this mapping: redocumentation is automatable; design recovery is question-generation — the
machine drafts, marks provenance, and asks; it never fills in the why.

*The `adopt` skill itself — built, 2026-07-19.* Nine skills now ship in the kit. It is the
five-phase RE lifecycle above made operational: gather (sources as Fragments with trust
weighed), scan (breadth-first coarse, structure from imports never prose, one `import_graph`),
analyze (static detectors + the dynamic receptors — run the tests into
`reconcile_verification`, observe deployments into `reconcile_deployment`), recover (intent
only from outside the code, `design_without_intent` and `unmotivated_capability` as the
question engine, rationale as provenance-marked Decisions, found limits as budget Constraints,
found processes as Flows), validate (the reconcile family agreeing + every remaining
gap true-of-the-system or a model error). Its close states the doctrine: *adopt is done when
the graph and the system agree and every remaining gap is acknowledged or genuinely open —
a system adopted honestly usually should have open gaps.* The installer's brownfield
next-steps and the consumer AGENTS.md step 0 both point at it. **Not yet exercised on a real
system** — the next brownfield trial should run through this skill, which is also what the
still-open deepening/frontier work (below) is waiting behind.

Size **L** for the thread; the `adopt` skill itself is ~~**M** once the two **S** blockers land~~ **done**, and
the deepening stage is a separate **M** behind the frontier work.

**BL-15 · Project bootstrap and kit updates** — *from the external user, 2026-07-18.* "You should
be able to launch a project from reflow, which bootstraps everything into a new repo... And maybe
it adds a script for pulling in releases. You won't know up front what project type it is though."

Two problems, and his caveat is the design.

*Bootstrap.* Today the kit installs by three hand-run `cp`s, one of which needs the binary path
edited in, plus a hidden `.grok/` that `cp *` misses. That should be one command.

*The caveat is the product, not an obstacle.* You don't know the project type up front **because
that is a design decision the loop is supposed to make.** So bootstrap only what is type-neutral
— AGENTS.md, skills, MCP config, `.reflow2/`, `.gitignore`, a brief template — and deliberately
scaffold no `src/` layout, build file, or language choice. Those come *out* of the design, and
both blind trials produced exactly that: a structure that fitted what had been designed. A
scaffold that guessed would commit a design decision before the design existed.

*Updates are the sharper half, and currently absent.* The kit is copied, so a consumer's copy
freezes at install time — the first external user's copy is already stale by a day of skill
fixes and nothing tells him. Text (AGENTS.md + skills) is easy to refresh; the binary needs a
~10-minute RocksDB build, so it wants either published release binaries or a pinned-version
check. Bears directly on the embedded-vs-service fork: a service would make this disappear.

**Bootstrap and in-place updates: done** (`tools/reflow2_init.py`) — one command installs or
refreshes the design environment, resolves the binary path itself, records a kit version so
staleness is detectable, and leaves the graph, user files and a customised `.mcp.json` alone.
It installs no `src/`, build file or language choice, on purpose.

**Update-skew detection: done.** `reflow2_init.py` now reports whether the *binary* is older
than the source it was built from — the quiet failure where you pull, re-run init, and forget to
rebuild, leaving current instructions driving an old server. SETUP.md documents the three-step
update in the order that matters.

~~**Still open — published releases.**~~ **Built 2026-07-20** (the first release run is the
verification): `.github/workflows/release.yml` builds `reflow2-mcp` for linux-x86_64 /
macos-arm64 / macos-x86_64 on every version tag (or a dispatch naming an existing tag — the
tag must match Cargo.toml's version or the kit job refuses), packages the consumer kit as a
tarball in the same `tools/` + `getting-started/` sibling layout the init script resolves,
stamps it with `KIT_VERSION.json` (a tarball's stand-in for git metadata), and attaches
everything plus sha256 checksums to the GitHub release under **version-less asset names**, so
`tools/install.sh` needs no API parsing. The installer prefers `gh release download` (repo is
private; unauthenticated curl is the path that starts working the day it isn't), verifies
checksums or says plainly that it could not, installs to `~/.local/bin` + `~/.local/share/
reflow2/kit`, and re-running it replaces binary and kit *together* — the BL-32/BL-18 skew pair
cannot open between them. `reflow2_init.py` grew the three checkout-independences: `--binary` /
PATH fallback, `KIT_VERSION.json`, and installed-mode update advice (re-run the installer, not
git pull + cargo build). Verified end to end from a simulated tarball: install, idempotent
re-run, `--check`, brownfield/greenfield branching all correct. macOS note: unsigned binaries
are fine through the installer (curl sets no quarantine xattr); browser downloads would hit
Gatekeeper — signing stays open if that path ever matters. **Follow-up (S–M, deliberate):**
embed the kit in the binary (`include_str!` is already the schema's pattern) so `reflow2-mcp
init` replaces the Python script entirely and one artifact carries everything.

**The fork is decided** (2026-07-18, [surface-plan.md](surface-plan.md)): **repo-file, embedded**.
So this is no longer gated — build published per-platform binaries, which is the packaging answer
to a packaging problem. The service was weighed and set aside: its strongest argument
(concurrency) is hypothetical while there is one writer, it would put the user's design graph on a
machine they do not control, and it is permanent operational cost. The conditions that would
reopen it are written down; "published binaries proved insufficient" is one of them, so this item
is also the experiment that would justify revisiting.

*Distribution mechanics, distilled from a 2026-07-20 discussion (user + brother + outside
research), so the build of this item starts where the thinking stopped.* The standalone-repo
proposal blends three questions with different answers:

1. **Where the code lives — already answered.** reflow2 *is* a standalone repo; a consumer
   project carries none of its code, `.reflow2/` is gitignored, the committed export is small
   JSON. The real residue is (2) and the kit files — and the kit files are the product's UX,
   which must live where harnesses look (BL-22's lesson).
2. **Where the binary comes from — this item.** The stack's advantage: Rust+RocksDB compiles to
   a zero-dependency native binary — no Node, no Python, no toolchain. Plan: CI builds
   per-platform binaries on each tag (v0.4.0 exists to publish); a `curl | sh` installer
   (rustup/uv pattern) that detects platform and drops `reflow2-mcp` on PATH; `cargo install
   --git` as the zero-infrastructure path for Rust developers; macOS signing still an open
   question. **Embed the kit in the binary** (`include_str!` is already the schema's pattern) so
   `reflow2-mcp init` replaces the checkout-bound `reflow2_init.py` and one artifact carries
   everything — then a consumer `.mcp.json` says `"command": "reflow2-mcp"`, no checkout
   anywhere, and kit updates ride binary updates (which also simplifies BL-18's staleness story
   to one version instead of three).
3. **Where the graph lives — the only genuinely open question, deliberately NOT this item.**
   The proposal's global `~/.reflow/` + per-project thin reference. Note before the queued
   Decision conversation: the live RocksDB dir is already machine-local working state; what
   `req:persistence` actually protects is *the durable record travels with the project*, which
   a global graph dir preserves iff exports stay committed in-project. What it costs:
   discoverability and the backup-beside-the-graph story. What it does **not** buy:
   concurrency — stdio servers spawn per client and the single-writer lock is per graph either
   way; a "global server" is only a global binary. `--graph-path` already makes the thin
   pattern available today; the question is only the default. Test against
   `dec:repo-file-embedded`'s reopening conditions, on the record.

> **Unblocked 2026-07-18.** BL-18, BL-19 and BL-20 were all waiting on the embedded-vs-service
> fork. It is decided — **repo-file, embedded** ([surface-plan.md](surface-plan.md)) — so build
> them for that shape. Export/import is now the migration story rather than a stopgap until a
> service centralises it.

**BL-26 · Which files does the design depend on, and is `DOCUMENTS` traversable?** — *user,
2026-07-18.* Prompted by the question "should every document in a repo be captured in the graph —
what is the purpose of each file?"

*Not every file.* [BL-23](#closed) is the caution: modelling 22 source files as Artifacts made
them 88% of the gap list. Capturing everything is how a list becomes something people skim. The
criterion is not "is it a file" but **"would something be wrong if this drifted out of step with
the design?"** That splits a repo four ways:

| Group | Example | Today |
|---|---|---|
| Produces the design | `crates/**/src/*.rs` | ✅ `Artifact` + `REALIZES` + checksum + `reconcile_artifacts` |
| Describes the design | `docs/*.md`, README | ✅ **write side done 2026-07-20**: `documents` (core fn + MCP tool, `doc_kind` carried, both endpoints checked — the storage engine accepts dangling edges, so the fail-loud check is the only one there is) |
| Instructs agents | `AGENTS.md`, `COORD.md`, `.github/copilot-instructions.md` | ✅ same mechanism: `artifact_type=document`, `doc_kind=agent_instructions`; the link-artifacts skill states the criterion and the boundary against `SPECIFIES` |
| No design meaning | `Cargo.lock`, `target/`, generated output | should stay out — this is where the noise would come from |

*The founding evidence is a failure reflow2 should have caught.* In one session on 2026-07-18:
AGENTS.md's build command was found wrong and fixed; hours later, by accident,
`.github/copilot-instructions.md` was found carrying **the same stale command**; and
`docs/backlog.md` grew a duplicated section that nothing noticed until someone went looking. Two
instruction files disagreeing about how to build the project is a coherence failure, and catching
coherence failures is the entire point — it was missed because neither file is in any graph.

*This is more than modelling more files.* Two things stand in the way:

1. ~~**`DOCUMENTS` has no write side.**~~ **Done 2026-07-20** — `documents` core fn + MCP tool
   (78th on the surface), endpoints fail-loud, `doc_kind` carried; pinned in core, over the
   tool surface, and the ghost-endpoint refusal in both. Was the recurring lesson's ninth
   instance, now closed.
2. **PROPAGATE does not traverse it — still open, and it is the M half.** `propagate.rs` lists `SPECIFIES`/`DOCUMENTS` as
   *"intentionally not traversed in this increment"*, so even fully wired a change would not
   ripple to the documents describing it. Making docs coherence-checked means **deciding
   `DOCUMENTS` is traversable**, and deciding what that implies for blast radius — a change to a
   Component reaching every doc that mentions it could be useful or could be the next flood.
   Weigh it against BL-23 before switching it on.

*The self-referential case is the best test available.* reflow2's own records — CHANGELOG,
backlog, requirements-coverage, COORD — are a hand-maintained golden thread, and four separate
lapses in one session went uncaught. The self-host probe already models
`requirements-coverage.md`'s **contents** as 72 Requirements but not the **file** as an Artifact
documenting them: the graph knows the requirements and not the document that is supposed to track
them. Extending the probe to the instruction and record files would test this before any of it
ships.

Size: ~~**S**~~ the write side is **done** (2026-07-20) — recording which files matter is now
possible, and the self-referential test (this repo's own records as DOCUMENTS artifacts) is
unblocked. **M remains**: the traversal decision — whether a change ripples to every doc that
mentions it — weighed against BL-23's flood lesson, and it wants the user.

**BL-19 · The graph must survive an upgrade** — *user, 2026-07-18.* **Blocks BL-18**: an
"you're out of date" nudge shipped before this exists drives users into an upgrade path with no
migration story.

*What is actually true today* (verified against dynograph-foundation v0.10.0). The schema lives
in the **binary** — reflow2 embeds the ten YAMLs via `include_str!` and re-merges them on every
open — while the RocksDB directory holds only nodes and edges. `new_rocksdb(schema, path)` takes
the schema from the caller and **stamps nothing on disk**: not a schema version, not a foundation
version. Validation runs on write, never on read.

So the reassuring half first: **upgrading reflow2 does not delete anyone's graph.** The feared
catastrophe is not the failure mode.

*The quieter hazard is real, though.* The foundation's own test (`engine/tests.rs:1325`) pins the
behaviour: add a required property with a default, and the default is applied **on create, not
backfilled**. Existing nodes keep the old shape. A schema change therefore leaves mixed-vintage
nodes with no error and no marker — detectors read `None` on old ones and a value on new ones.
That is a silent drop, which AGENTS.md rule 4 forbids everywhere else in this codebase.

*And the destructive case has no guard at all.* If dynograph-foundation changes its key encoding
(`keys.rs`) or value serialization, an existing store may be misread — and because nothing stamps
a version on the graph directory, there is no way to **detect** that a store predates the format,
let alone refuse to open it.

**The stamp and the check are done.** A `GraphStamp` — reflow2 version, schema version, node and
edge type counts — is written to `<graph>.meta.json`, a *sibling* of the store rather than a file
inside a directory RocksDB owns. `open_rocksdb` reads it, compares, refreshes it, and the MCP
server reports any difference on stderr and in the log.

*What it refuses, and deliberately does not.* Refusing on any mismatch would be worse than the
problem: schema growth here is additive, so a graph written before a type existed reads perfectly,
and refusing would lock someone out of their own design over a change that cannot hurt them. The
line is drawn at **a graph from the future** — one written by a reflow2 whose schema knew *more*
than the running one. That graph can hold nodes this binary has no vocabulary for, so reading it
means silently seeing less than is there. Refused loudly, with what wrote it and what to do.
An unreadable stamp is reported and never overwritten; it may be the only record of what wrote the
graph.

The declared schema `version` was not usable as the signal — it is 1 in every domain and has never
been bumped. Type counts are what actually move, and they caught the 26→27 change from BL-4.

**Backup-before-upgrade is done.** `reflow2_init.py` exports the design to
`.reflow2/backups/design-<utc>.json` before it changes anything — beside the graph, never `/tmp`,
which systemd-tmpfiles clears. A failed export is reported and does not abort the update: the
update may be exactly what fixes the binary that could not read the graph. `reflow2-mcp --export`
prints the document to stdout, so a script can take a backup without speaking MCP.

**Backfill is done, and it needed no new code.** Importing applies the *current* schema's
defaults, so a document written before a property existed comes back carrying it. That is why
export/import is the migration path rather than bespoke per-change code: export with the old
build, import with the new, and mixed-vintage nodes resolve themselves. Pinned by
`importing_an_old_document_backfills_new_defaults`.

**BL-18 · Am I running the current reflow2? — DONE** — *user, 2026-07-18.* Extends the update half of
BL-15, whose local machinery is already built and whose remaining gap this names precisely.

`reflow2_init.py` stamps `.reflow2/kit-version.json` with `reflow2_version`, the short `commit`
and `committed_at`, and `binary_is_stale()` compares source mtime against binary mtime. Every one
of those checks is **local**: a consumer copy can tell that its binary predates its source, but
never that its source predates upstream. That is the one an installed copy actually needs —
the first external user's kit went stale in a day of skill fixes and nothing told him.

The check is cheap because the stamp already exists: `git ls-remote` for the remote HEAD, compare
against the stamped commit. No clone, no auth, one round-trip.

**Done.** `reflow2_init.py` reports it on `--check` and after every install: `git ls-remote`
against the remote HEAD, compared to the stamped commit. No clone, no fetch.

*It fires where someone deliberately asks*, not on every server start. A network call per MCP
session would be intrusive and would hang offline, and this script *is* the act of asking. Any
failure — offline, no access, no git, slow network — reports "could not check" rather than
silence, because **"I could not check" must never look like "you are up to date"**. It never
blocks an install.

When behind, it prints the three-step update in the order that matters, because doing them out of
order leaves current instructions driving an old server.

*What it must not promise.* Unlike `claude update`, there is nothing to pull: the binary needs a
~10-minute RocksDB build, so the check can only report staleness, not resolve it. A real
`reflow2 update` needs published per-platform binaries — BL-15's still-open half, and a decision
that belongs with the embedded-vs-service fork. Keep the two apart: this item is **S** and
useful now; that one is **M–L** and gated on the fork.

**BL-16 · Domain-appropriate artifacts — the non-coding design problem** — *user, 2026-07-18.*

Coding is the *natural* domain here because agents are trained on it, so "design and build
anything" is quietly load-tested only on its easiest case. Ask for a rocket and the question
"what are the artifacts?" has no obvious answer. Ask for a 3D-printed object and one artifact
should probably be an `.stl` — but nothing in reflow2 knows that, and the agent may not either.

The gap is not the `Artifact` type, which is domain-neutral already. It is that **nothing helps
the agent decide what set of artifacts a given design concept actually calls for.** For software
it free-associates correctly from training; for a rocket it may need retrieval to find that the
answer involves things like a mass budget, a trajectory sim, drawings, a test plan — and for
hardware some artifacts are *physical*, which the as-built/drift machinery (`reconcile_artifacts`
checksums files) has no notion of.

Bears on P3 realization, on `unverified_capability` (what counts as verifying a weld?), and on
BL-9's as-fielded view. Likely wants a per-domain artifact-kind prompt or a retrieval step at
GENESIS, not a hardcoded taxonomy — the whole point is that the project type is a design output.
Size **M–L**, and it is the sharpest test of the "design anything" claim we have.

**BL-17 · Engineering principles as a separate, design-general file** — *user, 2026-07-18.*
Ported from `~/dev_storyflow/PROTOCOL.md`, whose "⭐ Engineering Principles" section is the
generalizable part (the fleet/bus/worker-pool/LEDGER/docker machinery around it is storyflow
infrastructure with no analogue here — reflow2 has COORD.md and two people).

Two of the seven are already reflow2 invariants: *no silent fallbacks* is AGENTS.md rule 4, and
*no silent caps/truncation* is implemented as `truncated_beyond_depth` / `skipped_operations`.
The four not yet written down here are **root-cause-before-fix** (name the mechanism, never
pattern-match a fix onto a symptom), **done = end-to-end** (merged ≠ done), **verify your own
claims by execution before reporting**, and **modular, no monoliths**.

Keep them in their own file rather than inlining into AGENTS.md, exactly as suggested: they need
tailoring away from coding. `PROTOCOL.md` phrases them for a web stack — "lens-exposed",
Playwright on the real surface, `npm run check`, unit-vs-live tests. For a rocket or a document,
"end-to-end" and "the real path" mean something else, and verification is a `Verification` node
rather than a test run. A separate file can generalize; a section buried in AGENTS.md will drift
back toward code. AGENTS.md then points at it. Size **S**.

| ID | Item | Why | Size |
|---|---|---|---|
| **BL-7** | **`ingest` over MCP** (SP-3b) | The multi-pass extraction pipeline is unreachable agent-native, so provenance, fuzzy dedup and time-aware resolution never run. Needs a transactional prepare pass. Closely tied to #4 and to session continuity. | L |
| **BL-8** | **Session state / multi-project** | Select a graph per project; give agents memory across sessions. Core already supports `graph_id`; nothing exposes it. See the memory note and [reflow-audit.md](reflow-audit.md). *Partial precedent, 2026-07-19:* the design graph now carries the session **distillate** — 8 Decision nodes with rationale, each linking the session transcript URL (which every commit also carries as a `Claude-Session:` trailer). The doctrine: the graph holds decisions, not tape; a transcript is an artifact outside the graph, one link away. The remaining BL-8 work is the live memory (questions, working state) across sessions, and the `Fragment`/`YIELDED`/`TemporalFact` layer is the schema-complete, zero-write-side machinery for it | L |
| **BL-9** | **As-fielded view — DONE 2026-07-19** | `reconcile_deployment` (`fielded.rs`), the P5 sibling of `reconcile_artifacts`: per-environment observations vs `DEPLOYED_TO`, three divergence kinds, unknown ids reported, partial observation never read as absence. The library-plugin false positive is impossible by construction — only Releases run, only Environments host (the audit's caution, honored by shape rather than a flag). Recorded divergences are persistent `unresolved_drift` gaps that an agreeing observation auto-resolves; the design-side answer is `deploy_to` with the true status. Measured: **P5 2/2, phase trial 12/13** — the probe now injects a divergence instead of checking the tool exists. The as-fielded viewpoint renders. The last of the three feedback loops is BL-30's `reconcile_verification` | ~~M~~ |
| **BL-10** | **Root-cause classification of drift** | `drift.rs` detects divergence with no notion of *why*, so no notion of which side is wrong. Reflow's seven-category taxonomy ends in a decision rule. Needs a scalar coherence score to gate on. | M |
| **BL-11** | **Path-cumulative budget analysis — DONE 2026-07-19** | `budget.rs`: a budget is a `Constraint` (`quantity`/`limit`/`direction` — new backward-compatible properties) spent through `CONSTRAINS` edges (`contribution`/`basis`). `budget_report` gives the stated total, basis coverage, the worst dependency path (contracts collapsed), and an honest verdict — `incomplete` whenever a contribution is unstated (listed, never zeroed: the graph-analysis discipline), `ungated` without a limit, and a cycle refuses the path claim by name. `Constraint` had no write side at all — the fourteenth recurring-lesson instance; `add_constraint`/`constrains` close it. The measures viewpoint (≈ SV-7) renders, closing the catalogue's last row. Not built: a budget-exceeded DETECT gap — `budget_report` reports it, and whether an exceeded budget should *nag* like a contradiction is a decision for a real use first | ~~M~~ |
| **BL-12** | **Concurrent multi-agent / team access** | Deliberate future effort, and the trigger for revisiting the embedded/service fork — *decided 2026-07-18 as repo-file*. RocksDB is single-writer and fails loud, so agents take turns; that is only a real cost once a **second writer actually exists**, which it does not. Reach for RocksDB read-only secondaries before a service if the need turns out to be "let me look while you work". **First design sketch below** — four consensus-mechanism ideas that survived a 2026-07-19 thought exercise. | L |
| **BL-13** | **Advanced testing tiers** | Comprehension (partly answered by the blind trial), scale (all fixtures are 3–10 nodes), messy input, longitudinal. | M |
| **BL-14** | **`tools/` sweep follow-ups** | The remaining adopt-list items in [reflow-audit.md](reflow-audit.md): typed gap resolution strategies, abstraction-gap → strategy, document round-trip, MCP resources/prompts. | M |

**BL-12 · design notes — what crypto consensus mechanisms lend the multi-writer future** —
*thought exercise, 2026-07-19, from the author's question: could XRP / Hedera hashgraph /
Proof-of-Stake trust machinery extrapolate to reflow2?* These mechanisms all answer one question —
*how do parties who cannot fully trust each other maintain a shared ledger without a referee?* —
which is BL-12's question with different nouns: a human, an LLM, and a second human+LLM pair on one
design. Four ideas survived contact with the analogy; the rest was vocabulary tourism.

1. **Claims reference what the claimant had seen** (hashgraph's gossip-about-gossip). Hashgraph's
   core move: never vote on truth — record who-knew-what-when as a DAG (each event hashes what its
   author had already seen) and *compute* consensus deterministically. The confirmation ledger
   already computes trust states from a claim DAG the same way. The extrapolation: every accept or
   design edit carries the **export hash of the graph state it was made against**. Then the
   question that kills shared designs becomes computable: was a conflicting claim made *in
   ignorance of* mine (both honest on stale views — merge mechanically) or *in defiance of* it (a
   real disagreement — route to the humans as a question)? COORD.md answers this socially today
   ("pull before you claim"); the graph could answer it structurally. The best single idea here.
2. **Trust topology per claim type** (XRP's Unique Node List). The supermajority half is
   meaningless at n=2–4, but explicit per-claim-type trust maps cleanly, and exists in embryo:
   `unmotivated_capability` already weights `inferred` (0.70) differently from `authored` (0.55).
   Extrapolated: *who may assert what* — "verification status is only credible from a CI run,
   never from the agent"; "a requirement moves to `accepted` only on the human's say-so". This is
   also [BL-41](#next-up)'s mechanical half: text from a party not authorized for a claim type
   simply does not count as that claim.
3. **A finality boundary at release cuts** (ledger close). BL-34's frozen manifest is finality
   without the word — a past release's contents cannot be rewritten. The extrapolation with teeth:
   nothing before the last `release_cut` epoch may be *mutated*, only superseded. `temporal.rs`
   snapshots the past but does not yet enforce its immutability.
4. **Computed track records, shown not enforced** (Proof-of-Stake slashing, inverted). Nobody
   bonds capital, but every party accrues a record the graph already holds — *five `design_holds`
   claims, zero design edits* is a legible signature. Slashing softens to reputation: a party
   whose claims keep being overturned has that history computed and displayed, never auto-punished
   — judging a claim false is semantic, and automated slashing would seat the graph in the
   judgment chair that `dec:report-dont-judge` reserves for the human.

*Where the analogy breaks, so nobody rebuilds it wrong:* BFT is built for many anonymous
adversaries; BL-12 has two-to-four named collaborators whose threat model is **error and drift,
not malice**. Staking economics need a scarce resource nobody here has. And consensus mechanisms
exist to *automate agreement*, while reflow2's philosophy routes disagreement to humans as
questions — auto-resolving a design conflict by quorum would be sycophancy-by-majority, the wrong
party in the judgment seat ([partnership.md](partnership.md)). Borrow from the *evidence* side
(what was seen, who may claim, what is final); never from the *verdict* side.

**BL-12 · 2026-07-21 addendum (user)** — the reopening condition `dec:repo-file-embedded` wrote
down has now been *asked for in so many words*: "could multiple sessions (same machine or
remote) all use a common MCP — a common/centrally hosted MCP server?" That is the second writer
materializing as a request rather than a hypothesis, which is what the embedded decision said
would reopen the fork. Shape of the answer when picked up: **(a)** stdio MCP is 1:1 by
construction, so a shared server means the streamable-HTTP/SSE transport in front of the same
core — the surface-neutral seam exists for exactly this; **(b)** the cheap first rung is still
the recorded one — RocksDB read-only secondaries if the need is "let me look while you work",
a full service only for true concurrent *writers*; **(c)** a central host puts the design on a
machine the user doesn't control — the decision's strongest objection — so self-hosted-first;
**(d)** the moment two writers are real, BL-44's claims, BL-41's mechanical trust half, and
sketch idea 1 (claims reference the graph state they saw) stop being future work and become the
write-path prerequisites. Sequencing note: BL-71's design-vs-design diff is also the merge
primitive a shared graph needs the day two writers disagree.

**BL-44 · Node-level claims — parallel work on one design** — *user, 2026-07-20. Concept-only by
their own framing; the details are the work.*

The idea in the user's words: an agent **claims the nodes in the graph it is working**. A task
strictly internal to a node claims only that node; work on an interface — an edge between two or
more nodes — claims **all affected nodes**. Anyone else may work any unclaimed node, and
"theoretically, their work should not interfere."

What this is: [COORD.md](../COORD.md)'s claim board moved *into the graph*, at node granularity.
COORD coordinates two humans over files, socially ("commit the claim line before the work");
this coordinates N agents over design nodes, and the graph can compute what COORD relies on
people to do. It is BL-12's first concrete write-side mechanism sketch, and it composes with the
consensus notes above rather than replacing them.

Tensions to resolve before building — the flesh-out list, so whoever picks this up starts where
the thinking stopped:

1. **Claims partition intent, not I/O.** The store is single-writer (BL-12), so simultaneous
   *writes* still take turns whatever the claims say. The claim layer is what would make
   fast-alternating writers *safe*; whether the alternation itself is acceptable (one server,
   short turns) or needs read-only secondaries is still BL-12's storage question, unchanged.
2. **"Should not interfere" is a claim PROPAGATE exists to check.** A change internal to a node
   can still ripple — blast radius is `propagate_change`'s whole subject, and
   `surprising_connections` exists because the graph's edges under-state real coupling. So a
   node claim may need to be *sized* by the blast radius at claim time (claim the propagation
   frontier, not the node), or at least validated against it — and the interference the graph
   has not drawn an edge for yet is precisely what it cannot protect anyone from. The
   edge-work rule (claim all endpoint nodes) is the user's own instance of this at depth 1.
3. **A claim needs an identity to belong to, and the graph has none.** No Person/Agent node
   type exists. Whatever carries it — a `Claim` node with `CLAIMS` edges (first-class,
   axis-Z-recordable, expirable) over a bare property on the claimed node (invisible to
   history) — this introduces *who* into the schema for the first time, which is the same
   missing piece BL-12's sketch idea 2 (who may assert what) and BL-41's mechanical half both
   need. One identity mechanism should serve all three; building it for claims alone would be
   the recurring lesson in reverse.
4. **Stale claims.** COORD's rule is "a week with no commits, anyone may take it." The graph
   equivalent wants to be *computable*: a claim that records the epoch or export-hash it was
   made against (sketch idea 1 above) goes stale by fact, not by calendar — the graph moved
   under it, or its holder stopped moving the graph.
5. **Advisory or enforced?** Refusing a write to a node another agent claims is tempting and is
   also the graph seating itself in the judgment chair. The repo's own doctrine
   (`dec:report-dont-judge`, disagreement routed to humans as questions) argues for advisory
   first: the write lands, the claim violation is *reported* — loudly, to both holders — and
   enforcement waits for evidence that reporting wasn't enough.

Prior art to mine: COORD.md itself (claim-before-work, one line per claim, merge=union — each
has a graph analogue), and the ophyd/storyflow adoption discipline of marking unexplored
frontier (a claim is "mine, in progress"; a frontier mark is "nobody's, not yet real" — the two
partitions should not be confused, and both quiet the detectors differently).

Size **M** for an advisory claim layer once the identity decision is made; the decisions —
identity (3), granularity vs blast radius (2), advisory-first (5) — are the real content, and
they want a session with the user, not a patch.

**2026-07-21 addendum (user)** — two sharpenings from a second pass on the same idea. **(a)** The
claim unit may be a **cluster**, not a node: "Alex is working that cluster — Bobby is working that
node." The regions are computable with machinery the graph already runs — a blast radius
(`propagate_from`) or a community (the allocation clustering) — which promotes tension 2 from
*validation* to *granularity*: claim the island, not the node list. **(b)** A claim licenses
**design authority**, not just edit intent: the holder "goes off and makes design choices for
their area", so Decisions recorded inside a claimed cluster belong to its holder — identity
(tension 3) joined to BL-12 sketch 2's who-may-assert-what, applied to a region instead of a
claim type. And a checkout that *explores an option* rather than progressing the baseline is
[BL-70](#bigger-threads)'s fork wearing work clothes — the claim layer and the alternatives
layer likely want the same scoping primitive, and should not be built twice.

**BL-45 · System-of-systems: external dependencies between reflow2 projects** — *user,
2026-07-20. Explicitly a thought exercise; the mechanics are the open question.*

The idea in the user's words, compressed: a reflow2 project should be able to declare
**external dependencies** on *other reflow2 projects* — possibly in different repos, owned by
different people or organizations — the way `pyproject.toml`/`pixi.toml` declare software
dependencies (and noting that in non-software domains the analogue is not obvious). One
project "interfaces" with another; groups focus internally on their own project but link
outward, building **system-of-systems architectures** — and when two or more systems interface
through the same contract, *standards between systems can come into existence*. A project
would publish an **external-facing interface spec** — "something synonymous to OpenAPI
specs/docs".

This is the package-manager shape applied to the oldest SE artifact there is: the published
surface is an **ICD**, and the graph already holds most of the vocabulary —

| Piece | Already exists | The SoS gap |
|---|---|---|
| The contract itself | `Interface` nodes; `SPECIFIES` (an OpenAPI/protobuf artifact IS the machine-readable contract, with a `format` property); `provides`/`consumes` | Nothing marks an Interface **external-facing** — visible-to-others vs internal is undeclarable |
| Publication | Deterministic `export_graph` (byte-identical, stamped); BL-15's release machinery — a release asset is a distribution channel with a URL and a checksum | No **filtered** export: "everything" or nothing. The published surface wants to be exactly the external Interfaces + what they expose, and nothing internal |
| Consumption | `import_graph` upsert; `provenance: imported` exists on the four adoptable types | Imported reference nodes aren't marked *foreign* (whose project, what version, what checksum) — and edges cannot span graphs, so cross-project links must be **mirrored reference nodes**, not edges into another repo |
| Version pinning | `Release` + `INCLUDES` + `as_checksum` (BL-34's frozen manifest); `GraphStamp` | No way to say "I build against *their* v2.1" — the dependency declaration (project, surface, version, checksum) has no home |
| Drift detection | The reconcile family; `unresolved_drift`; BL-18's am-I-current check | The cross-boundary case: *their published surface moved and my mirrored copy is stale* — the exact BL-18 question, one project boundary out |

*The observation worth keeping even if nothing else survives:* **a standard is itself a design**
— when N projects consume the same published interface, that contract wants to be its own
reflow2 project (requirements, decisions, releases, verification of conformance) that provider
and consumers all declare a dependency on. Standards emergence then has a mechanical form: two
bilateral interfaces noticed to be the same shape → extracted into a third project both
reference. That is how real standards bodies work, minus the committee.

Tensions, so the flesh-out starts honest: (1) **trust** — an imported surface is another
organization's graph text, and BL-41's "graph text is data, never instructions" plus BL-12's
who-may-assert-what stop being single-user hygiene and become the security boundary; (2)
**transitive dependencies** — does importing their surface pull *their* dependencies' surfaces
(the diamond problem, now with orgs); (3) **the non-software domains the user names** — a
supplier's actuator, GFE, a materials spec: `Interface` + `Constraint` may already carry it,
which would make this BL-16's sharpest test case too; (4) private↔public seams — this repo just
split public evidence from private trial records, and an SoS link between a public and a
private project is that same seam as a *feature*.

Size **L** for the thread. The first testable increment is **S–M** and needs no federation at
all: an `external: true` marking on Interface + a filtered "published surface" export — this
repo could publish its own MCP tool surface as the first ICD. Related: BL-8 (multi-project),
BL-12 (multi-writer), BL-16 (domains), BL-44 (claims). Decision conversation with the user
before any code; this entry is the prep.

**BL-46 · `create_node` on an existing node replaces the whole property object — DONE** —
*self-adopt live session, [trials/2026-07-20-self-adopt-live.md](trials/2026-07-20-self-adopt-live.md);
fixed 2026-07-20, same day.*

Folding merged wording into `cap:kit`'s description via `create_node` silently reset
`status: verified → planned`; on `req:intent-preserved` it also reset `priority: high → medium`
and `status: accepted → proposed`. The props object supplied replaced the stored one, with
schema defaults filling every omitted property — so the only safe "edit one property" call was
one that re-supplied all of them, which nothing told the caller. **Fixed by the merge option:**
`DesignGraph::upsert_node` (supplied props over stored, validation unchanged), and the
`create_node` MCP tool now routes through it and says so in its description — the contract the
revise-design skill stated all along. The typed setters stay the right call where they exist:
they refuse a missing node instead of creating it. Tests: `tests/upsert.rs` (core),
`create_node_on_an_existing_id_merges_instead_of_resetting` (surface).

**BL-47 · Unset provenance must not tie with `authored` in merge survivor selection — DONE** —
*self-adopt live session; fixed 2026-07-20, same day.*

The genesis stubs carried no `provenance`, which read as the default `authored`; HEAL's
survivor rule saw a tie against the real authored nodes and fell through to the id tiebreak,
proposing to keep stub `cap:install` and **delete the authored, verified `cap:kit`** (same for
`cap:artifacts` over `cap:reconcile-built`). Caught only because the proposal was reviewed
before apply. **Fixed:** `provenance_rank` now takes an `Option` and slots `None` (a
pre-provenance vintage — defaults materialize on create, so nothing newer lacks the property)
strictly between explicit `authored` and everything else. A vintage pair still ties and falls
to the id, so pre-provenance graphs are unchanged; an explicit `authored` now beats its
vintage twin outright, and the machine provenances still never delete probable human words.
The related clobber is fixed too: **a colliding edge is no longer re-pointed** — the
survivor's edge and its properties are kept, the drop reported in `discarded` (previously the
removed node's `action: removed` overwrote the survivor's `modified` after being "reported").
Semantics pinned on `dec:merge-survivor-provenance`; the unset slot is unit-pinned at the
`provenance_rank` seam because today's API cannot build a vintage node.

**BL-48 · `graph_report_markdown` returns malformed `structuredContent` — DONE 2026-07-20** —
*self-adopt live session.* Size **S**.

From Claude Code the tool failed client-side schema validation: `structuredContent` arrived as a
string where the MCP result contract wants a record — the fifteenth recurring-lesson instance
(the capability exists and one harness cannot reach it), and the same response-side shape as the
original array `structuredContent` bug. **Fixed at both layers**: the tool now returns the
report as plain text content with no `structuredContent` (a prose document has no structure to
declare — `ok_markdown`), and `ok_json` — the choke point every other tool returns through —
wraps any remaining scalar as `{value}` so no future tool can leak one. `smoke_mcp.py` now
asserts the envelope on **every** call it makes (`structuredContent`, when present, must be an
object) and fetches the Markdown report over the real wire — the check that would have caught
this the day it shipped. Reproduced live in this session before fixing (the tool failed as the
first call of a where-am-i pass).

**BL-49 · Unbounded read-tool results overflow the agent boundary — DONE 2026-07-20** —
*self-adopt live session.* Size **M**.

`propagate_change` returned 70k chars (142 impacted nodes), `export_graph` 93k — both
overflowed the tool-result budget and were readable only through the harness's spill-to-file
fallback plus `jq`. A blast radius nobody can read inside the loop is a blast radius that
doesn't get read. **Both propagate tools now answer with a summary by default** — counts by
distance (every impacted node in a band, `total_impacted` checked against the full walk in
tests), the distance-1 ring with the edge that reached each node, risk crossings at any
distance, `unknown_seeds`/`truncated_beyond_depth` carried through — with the full per-node
`via` dump behind `full: true`. The summary is computed in core (`BlastRadius::summarize`), not
shaped at the surface. **`export_graph` takes an optional `path`**: it writes the document as
deterministic sorted-key JSON (byte-identical on an unchanged graph — pinned in tests) and
returns a `{path, bytes, nodes, edges, stamp}` receipt instead of the payload. The impact-check
skill teaches the summary-first contract. `max_nodes` was not added: the summary removes the
size driver (hop chains), and a cap on the ring would be a silent truncation with extra steps.

**BL-50 · Tool-boundary paper cuts from the self-adopt live session — DONE 2026-07-20** —
grouped, each **S**.

(1) `DUPLICATES.confidence: 1` was rejected with "expected Float, got int" — every LLM writes
`1`, JSON has one number type. **Integer literals now widen losslessly to floats at the core
write seam** (`create_node`/`create_edge`, schema-aware, so it covers every surface), and only
there: a non-exact integer still fails loud, the range check still applies after widening, and
a property the schema does not declare float is never touched. The foundation stays pinned —
the coercion is reflow2's, not a validator change. (2) **`add_change_event` takes an
`affected` list** and draws its CHANGED edges in the same call — validated whole before
anything is written (storage accepts dangling edges, so the tool's check is the only one), so
a bad entry refuses the event rather than leaving a partial record; the result names each edge
and its action. And **`describe_schema` now counts half-exact matches**: CHANGED names its
from-side and is open on the to-side *by design*, and the note now calls such an edge the
modelled fit instead of lumping it with both-sides wildcards. Bonus from the same envelope
discipline: `delete_node`/`delete_edge` returned bare booleans (the BL-48 defect shape); they
now return `{deleted}`. (3) The kit's **SessionStart hook recipe** is documented in
getting-started/AGENTS.md step 0a — the where-am-i ritual lands in the session's context at
startup on harnesses with hooks; the rest keep the written convention. Not auto-installed:
writing into a consumer's `settings.json` is not a thing the installer gets to do.

**BL-51 · Frictionless install and update — the Claude Code model** — *user, 2026-07-20.*
Size **S + M**, priority deliberately low ("may not be important right now... can we get there
eventually").

The user named the target explicitly: Claude Code installs with one `curl -fsSL <url> | bash`
from a stable public URL, updates with a single `claude update`, and ships frequent, very minor
versions so updating is routine rather than an event. "I like frequent and very minor updates
(it is very iterative)." Recorded as `req:frictionless-update` (proposed, low), partially
satisfied by the install capability with the delta on the edge's evidence. The concrete gaps,
now that the repo is public and BL-15's machinery exists:

- **(S) A stable public one-liner.** `install.sh` is already checksum-verified and pulls
  published binaries; what is missing is the documented, tested
  `curl -fsSL https://raw.githubusercontent.com/sligara7/reflow2/main/install.sh | bash`
  path (or a short redirect domain later) in SETUP.md and the README, exercised by a probe.
- **(M) One-word update.** Today updating is "rerun the installer" or
  `reflow2_init.py <project>` in place. Wants `reflow2-mcp update` (or a thin `reflow2`
  wrapper verb) that re-runs the checksum-verified fetch and swaps binary + kit together —
  the staleness detection half already exists (`served_by`/BL-32, `KIT_VERSION.json`/BL-18),
  so this is the acting half.
- **(cadence, no code) Frequent minor cuts.** release.yml makes a cut cheap; the practice is
  cutting often and keeping CHANGELOG sections small. Nothing to build — but the one-word
  update is what makes a frequent cadence tolerable to consumers, so it gates the practice.

**BL-52 · CI enforces the gates; skills get contract lint — DONE 2026-07-20** — *user asked
"do we have legitimate CI tests for the skills?"; the answer was that there was no CI at all.*
Size **S + M**.

`.github/workflows/ci.yml` (the repo's first CI): a fast core job — core tests on the
in-memory backend, clippy `-D warnings` both crates, fmt, schema validation, the installer
suite, skill lint — and a full job that pays the cached RocksDB build for `cargo test
--workspace`, then drives the REAL binary: `smoke_mcp`, `phase_trial` (13/13 gate),
`model_the_loop`, `coherent_erosion_trial`. `erosion_trial` deliberately excluded (non-zero by
design until the ledger-judgement decision changes). `tools/skill_lint.py` checks what a skill
has that IS mechanically checkable — its contract with the surface: every backtick tool name
resolves against the served `#[tool]` set (with a committed, both-ways-enforced allowlist for
field/gap/enum terms, so a tool rename leaving prose behind fails loudly and the list cannot
rot), mirrors byte-identical to `getting-started/skills/` (the recurring "stale mirrors"
chore, now a gate), frontmatter valid, and BL-41's standing rule present in all 11 skills.
**Deliberately NOT built: LLM-driven skill evals** — a synthetic eval is another client we
write (the three-agreeing-clients lesson); semantic skill quality stays evidenced by real-use
trials per sharpening.md. Verified: all checks green on the current tree, negative test
confirms a bogus tool ref fails with exit 1, and the first live CI run on GitHub is the
end-to-end proof.

**BL-53 · A self-loop DUPLICATES edge makes HEAL delete the node — DONE 2026-07-21** —
*deep review (verified in source).* Size **S**, was **critical**. Fixed: equal endpoints
are refused in `merge_op_for` with a reason naming delete_edge as the correction — one
guard covers propose and apply, which both derive through it; regression test pins the
node's survival end to end.
`merge_op_for` (heal.rs) guards unresolvable endpoints and cross-type merges but not
`keep == remove`. `x DUPLICATES x` is schema-valid (`*→*`); the merge repoints nothing
(every edge "already points at the survivor") and then `delete_node(x)` removes the survivor
and all its edges, while the report says applied/verified. Merges have no snapshot and no
undo. Fix: refuse equal endpoints in `merge_op_for` (covers propose AND apply, which both
derive through it) + a regression test.

**BL-54 · The installer can destroy user content and die mid-run — DONE 2026-07-21** —
*deep review.* Size **M**. All four fixed: install now records a per-file sha256 manifest
in the stamp; ownership is proven by hash (edited kit files are LEFT ALONE with a report,
never overwritten; delete the file to accept the kit copy); the sidecar obeys the same
rule; files the kit no longer ships are pruned only when untouched; non-dict server values
report left-alone instead of crashing, and --check agrees with the run. Pre-manifest
installs keep the old heading heuristic for exactly one update, then the manifest closes
the window. Three regression tests. Four related defects in `reflow2_init.py`: (a) kit-file ownership is judged by
first-heading match, so a consumer's edits to an installed AGENTS.md or skill are clobbered
on update and reported as a routine refresh; (b) the `REFLOW2.md` sidecar is itself written
with no ownership check; (c) files removed from the kit are never pruned downstream, so
stale skills load forever; (d) a non-dict `mcpServers` value raises AttributeError mid-run,
leaving a partial install — and `--check` promises a write the real run refuses. Fix: a
per-file hash manifest recorded in the install stamp — "ours to refresh" = hash matches what
we installed; anything else sidecars with a report; the manifest also enables pruning; plus
the type-check `--check` already has.

**BL-55 · First-contact integrity: install.sh and the release flow — DONE 2026-07-21** —
*deep review (mechanism verified live).* Size **S + S**. Fixed: `try_download` returns
instead of exiting, so a missing checksums.txt reaches the honest-skip message; a binary
that cannot execute now fails loudly with the build-from-source recipe instead of printing
success; release.yml creates a draft, uploads, asserts all five assets present, then
publishes — a partial upload can no longer become `releases/latest`. (a) `install.sh`: a missing
`checksums.txt` silently kills the whole install — `download()`'s `fail` exits the script
even inside an `if`, and the call site's `2>/dev/null` swallows the message; the "checksums
NOT verified" honest-skip branch is unreachable. Also a binary that cannot execute still
prints "installed:". (b) `release.yml` creates the release live before uploading assets, so
a partial upload leaves `releases/latest` with checksums and no binaries. Fix: a
non-exiting `try_download` for the optional asset + a loud warn when `--version` fails;
draft → upload → assert four assets → publish.

**BL-56 · Destructive and leaky defaults in the test harnesses** — *deep review.* Size
**S + S**. **(a) DONE 2026-07-21**: `--graph-path` now refuses an existing directory
unless `--wipe` is passed. (b) orphaned servers + undrained stderr pipe still open. (a) `smoke_mcp.py --graph-path` rmtree's whatever directory it is given, before
any prompt — pointing it at a live `.reflow2/graph` destroys a real design. Wants
refuse-unless-`--wipe`. (b) On any mid-run failure the spawned servers are orphaned
(no try/finally around `Server`), and stderr is a never-drained PIPE that can deadlock the
test under a warn-storm; all four trial harnesses inherit both via the shared class. Wants a
context-manager Server that drains stderr and kills the child.

**BL-57 · Tool-boundary honesty batch — DONE 2026-07-21** — *deep review.* Size **M**. All
seven fixed: (a) `dyno_err` is variant-aware at the one choke point — caller-shaped errors
(NodeNotFound/Unknown*/Validation/EdgeValidation/InvalidEdge/InvalidKeySegment/EdgeNotFound)
→ invalid_params, genuine faults → internal_error; ~60 tools stop blaming the server for a
caller's typo. (b) Every request struct (65) carries `deny_unknown_fields`, so a typo'd
optional param is rejected — schemars now publishes additionalProperties:false, and a smoke
check asserts none regress; it immediately caught a real latent bug (the smoke suite passed
`at` to reconcile_artifacts, silently ignored — the field is `detected_at`). (c) export_graph
refuses to overwrite an existing file without `overwrite:true`, uses invalid_params for an
unwritable caller path, and reports the canonicalized path. (d) The serve path now gets
`explain_open_failure`, so the everyday two-session lock collision reads plainly. (e) get_node
returns one named `{node: <obj|null>}` shape both ways (was bare-object vs {value:null}) —
strengthening smoke checks that were previously always-true. (f) resolved by category:
"remove-if-present" tools (delete_*, both withdraws) report a boolean — withdraw_gap_ack
aligned from `was_reviewed` to `withdrawn`; answer_question correctly errors (a silent
{answered:false} would be the drop the project forbids), now documented in its description. (a) `dyno_err` maps
every core error to `internal_error`; ~60 of 78 tools report caller typos as server faults —
make it variant-aware at the one choke point (`NodeNotFound`/`Unknown*`/`Validation`/
`InvalidEdge` → invalid_params). (b) No request struct declares `deny_unknown_fields`, so a
typo'd optional param (`ful`, `record_events`, `path`) is silently swallowed and the tool
quietly does something else — add it everywhere, and a smoke check that every published
inputSchema carries `additionalProperties: false`. (c) `export_graph path:` writes/overwrites
any path with no guard — require `overwrite: true` or refuse non-export targets. (d) The
serve path bypasses `explain_open_failure`, so the everyday two-session lock collision gets a
raw RocksDB error. (e) `get_node` absent returns `{value: null}` vs a bare object when
present — one named shape both ways. (f) Sibling tools disagree on missing records
(error vs `{withdrawn:false}` vs `{was_reviewed:false}`) — pick the boolean-report style.
(g) `parse_enum` and the ~17 typed edge tools reject without naming what would have worked.

**BL-58 · Core silent-failure batch — DONE 2026-07-21** — *deep review.* Size **M–L** (each
piece S). All twelve items fixed with tests: (a) ingest re-ingest merges via `upsert_node`
instead of resetting; (b) snapshots serialize sorted (BTreeMap) for byte-stable exports;
(c) `propagate_change` errors on a missing event instead of returning empty; (d) `apply_heal`
is one atomic batch across all operations (merge_nodes made batch-free); (e) swallowed
edge-creation errors in acknowledge_gap / record_asked_question / ingest provenance /
ensure_epoch / fielded now surface; (f) budget rejects non-finite contributions at the write
seam and reports a provable overrun instead of Incomplete (max_by uses total_cmp);
(g) integer widening rejects the i64::MAX saturation edge (bound at 2^53); (h)
`truncated_beyond_depth` documented honestly as the one-hop frontier lower bound; (i) drift
skips the dangling DEPENDS_ON for undocumented additions; (j) missing-intermediate gaps get
distinct ids per producing edge (relation folded into the hash); (k) a reused ingest
fragment_id is refused up front; (l) node_type_index scans in sorted order. Old body:
(a) ingest matched-evolved uses `create_node` replace — the BL-46 reset failure, still live;
route through `upsert_node` merge. (b) `snapshot_node` serializes a HashMap — snapshot bytes
are process-random, breaking byte-identical exports of identical history; BTreeMap it.
(c) `propagate_change` on a nonexistent ChangeEvent returns an empty radius
indistinguishable from "impacts nothing" — check existence. (d) `apply_heal` batches per-op,
so a mid-proposal failure commits earlier merges while the error implies nothing happened —
one batch, or a partial-application report. (e) `let _ =`/`.is_ok()` swallow edge-creation
failures in `acknowledge_gap`, `record_asked_question`, ingest provenance edges, and
`ensure_epoch` treats a read *error* as "exists" — swallow only already-exists.
(f) budget: a provable Exceeded is masked as Incomplete when any contribution is unstated;
NaN contributions can panic `max_by` — reject non-finite at the write seam and decide the
provable side first. (g) `widen_ints_for_float_props` i64::MAX saturation edge.
(h) `truncated_beyond_depth` counts one ring, docs claim all — make the number or the doc
honest. (i) drift's `undocumented_addition` writes a dangling DEPENDS_ON to a node that
doesn't exist. (j) duplicate gap ids when CONTAINS and DEPENDS_ON level-skips share a pair —
fold edge type into the hash. (k) `IngestOptions::default()` reuses `frag:ingest`, letting a
second run overwrite the first's snapshots — make fragment_id required. (l) sort
`node_type_index` type order; surface id collisions.

**BL-59 · Analysis-pass efficiency at adopt scale** — *deep review.* Size **M**. The SPOF
check rebuilds the full design network (with a whole-graph type scan) twice per articulation
candidate and recomputes the invariant baseline per candidate; `graph_report` runs detectors
redundantly (`dimension_drifts` twice); every `propagate_from` recomputes betweenness
centrality (O(V·E)). Storyflow-scale adopt (2,643 files) is where this bites. Fix: one
`AnalysisContext { node_type_index, design_network }` threaded through a detect pass;
centrality lazy or cached on a mutation counter. Also: paging (`limit` echoed in result) on
`scan_nodes` / `detect_gaps` / `confirmation_ledger`, the BL-49 convention extended.

**BL-60 · Docs truth pass — DONE 2026-07-21** — *deep review.* Size **M** (writing only),
was **critical for new readers**. Fixed across AGENTS.md (Current state rewritten to v0.5.0
reality — surface shipped, full module list, GENESIS built, two crates, v0.10.0 pin, 54
edges, INCLUDES in the traceability set), README (27 types + Question, layout tree shows the
real repo, path fix), requirements-coverage (IS-5/6/7 → ✅, preamble + deferral list
refreshed, tool/test/schema numerals), surface-plan + interaction-surfaces (superseded
banners), overview (routing + private-repo delinking + heritage table), SETUP (public repo,
commit-an-export story), getting-started/README (all 11 skills), and three skill
contradictions (link-artifacts full:true, detect-and-ask → retire-from-design, check-health
apply gate). skill_lint allowlist gained blocked_by_mode. All gates green. AGENTS.md "Current
state" still says no surface/service/LLM wiring exists and the interaction surface is an
open decision (78 tools ship); the module list omits two-thirds of src/ and calls GENESIS
unbuilt; the foundation pin is quoted v0.9.4 vs the manifest's v0.10.0; "53 edge types"
survives in four places vs the schema's 54; coverage matrix IS-7 says "not started" vs SP-3
✅ in the same file; `interaction-surfaces.md` carries no superseded label and overview.md
still routes to it as a live decision; README says 26 node types (omits Question), its
layout tree shows a docs-and-schema repo, and `../tools/` link is broken from root; SETUP.md
still says the repo is private and tells users to commit the graph the installer
force-gitignores (pick one story); getting-started/README lists 8 of 11 skills; skills:
link-artifacts step 6 needs `full: true`, detect-and-ask's dead-capability branch should
route through retire-from-design, check-health's apply gate self-contradicts; "(180 nodes)"
in three places vs 212; upgrading-to-v0.2.0 docs lack the breadcrumb.

**BL-61 · skill_lint is blind to single-word tool names — DONE 2026-07-21** — *deep review,
same day the lint shipped.* Size **S**. The `"_" in term` filter exempted `allocate`,
`satisfies`, `genesis`, `documents`, `precedes`, `provides`, `realizes`, `verifies`,
`consumes`, `contains`, `constrains` — 11 served tools, 10 referenced in skills, none checked.
Filter dropped; the allowlist gained the ~58 legitimate single-word non-tool terms (statuses,
enum values, field names, CLI/format words), the both-ways unused-guard keeping it exact.
Negative-tested: a renamed single-word tool now fails the lint (exit 1). The `"_" in term` filter means `allocate`, `satisfies`, `genesis`,
`documents`, `precedes`… are never checked — the rename-leaves-prose-behind case the lint
exists for. Drop the filter; extend the allowlist with legitimate single-word terms.

**BL-62 · Surface test-coverage gaps — DONE 2026-07-21** — *deep review.* Size **M**. 14 of 78 tools have no
coverage in tests/tools.rs or smoke_mcp.py (add_epoch, add_resource, delete_node,
dimension_drift(s), evaluate_allocation, pin_at_epoch, precedes, propose_allocation,
realizes, record_change, require_resource, surprising_connections, withdraw_question); plus
untested behaviors: get_node absent shape, and create_node/scan_nodes/search_design over real
stdio. **All 14 now covered**: two tests/tools.rs tests (a temporal/resource/realization/
analysis/delete walk + an ask→withdraw question round trip) and a smoke_mcp `§9c` section that
drives create_node/scan_nodes/search_design/delete_node/get_node over the real stdio boundary
— the blind spot smoke exists for. (export_graph overwrite guard is BL-57's, tested there when
it lands; get_node's absent shape is pinned to today's `{value:null}` with a BL-57 pointer.)

**BL-63 · Snapshots capture properties but not edges, so a re-allocation loses its history —
DONE 2026-07-21** — *user question + live demo, 2026-07-21 (promoted from BL-58 idea I4).*
Size ~~M~~.

**Built**: `snapshot_node` captures the node's design edges into a new optional
`Snapshot.edges` property beside `state` — direction, edge type, other endpoint (id and type),
and the edge's properties, sorted for byte-stable exports (the BL-58 discipline). Bookkeeping
neighbours (Snapshot/ChangeEvent/DesignEpoch/TemporalFact/dimensions/Fragment/DriftEvent/
Question) are excluded — a snapshot captures design structure, not the audit trail, and would
otherwise grow with each prior snapshot of the same node. `parse_snapshot_edges` +
`SnapshotEdge` join the core API; a pre-BL-63 snapshot reads as an empty capture, never an
error. **One deliberate deviation from the entry's lean**: full capture (bookkeeping excluded)
rather than changed-edges scope — at snapshot time the caller cannot know which edges the
coming edit will touch, capture is cheap at design scale (a hub is a few KB), and the exclusion
list bounds the noise; simpler than an opt-in split and loses nothing. Tests pin the demo shape
end-to-end (lazy reallocation: the snapshot alone now proves "A once owned Z"), bookkeeping
exclusion + deterministic order, and old-snapshot tolerance. The revise-design links guidance
dropped its pre-BL-63 workaround ("leave a formerly-true edge") for record-first-then-delete,
and both CRUD skills say edges are captured. Schema: `Snapshot.edges` optional → next cut is
minor per the versioning policy.

`snapshot_node` serializes a node's **properties** into `Snapshot.state`; it captures none of
the node's **edges**. Axis Z's promise is that the past is recoverable, and for a node whose
*properties* changed that holds — but a large class of design change is an **edge** move, not a
property edit, and those lose their history unless the modeller deliberately records a Decision.

Demonstrated end-to-end (docs/trials, reallocation demo): "Service A does X, Y, Z" → later
"Reconcile (Z) moves to Service B." The right-way sequence (impact-check → record_change →
delete the old `ALLOCATED_TO`, add the new one → a superseding Decision) worked, and the
Decision chain (`dec:own-v2 OBSOLETES dec:own-v1`, v1 marked `superseded`) preserved the
ownership history perfectly. **But** the snapshot of `cap:z` held only its properties
(name/status/…), not the `ALLOCATED_TO cmp:a` edge it lost — so the *only* durable record that
"A once owned Z" was the hand-authored Decision. A lazy reallocation (delete_edge + allocate,
no Decision) would leave Z on B with **no trace** it was ever on A. This is exactly the
long-lived-design case (storyflow, 8–9 months of shifting allocations) where it bites.

**The fix**: capture the affected node's edges into the snapshot alongside its properties, so a
`Modified`/`Removed` `record_change` preserves the link structure, not just the text. Design
decisions to make:
- **Scope** — snapshot *all* of a node's edges (complete but noisy/expensive for a hub like the
  Project), or only the edges the change actually touched (cheap, but needs the change to name
  them — pairs with BL-50's `affected` list and the `field`-scoped `CHANGED` edge). Lean toward
  the changed-edges scope with a full-capture opt-in.
- **Storage** — extend `Snapshot.state` (today a JSON string of props) with an `edges` section,
  serialized sorted for byte-stability (same discipline as BL-58's property fix). Update
  `parse_snapshot_state` and any reader.
- **Honesty in the meantime** — until built, say so loudly on `snapshot_node`'s docs and in the
  revise-design / retire-from-design skills: "a reallocation's history lives in the Decision you
  record, not the snapshot — model it as a Decision." (Cheap, do first.)

Not a silent-drop fix like BL-58 — the current behaviour is honest, just incomplete; this
completes axis-Z coverage for the edge dimension of change.

**BL-64 · The lifecycle stops at Operation — no disposal / retirement phase** — *user, UAF
lifecycle-breadth analysis, 2026-07-21.* Concept; size **M–L**; needs the user on vocabulary.

reflow2's phase spine is P0 Intent → P5 Operation, full stop. UAF's sixth phase — decommission:
retirement timelines, data-migration pathways, unwinding dependencies, sunsetting the
capabilities a system provided to *others* — has no representation (node-type probe: no
`Disposal`/`Retirement`/lifecycle-state construct). Do not confuse with the `retire-from-design`
skill: that retires something from the *model*; this is about modelling the *system's* end of
life.

The insight that makes this cheap: **reflow2 already has the retirement-impact engine** —
`propagate_from` answers "what breaks if we remove X." What's missing is (a) the *vocabulary*
to say "this Component/Capability/Release is planned for retirement" (a `lifecycle_state`:
`active` / `sunsetting` / `retired` / `disposed`, or a first-class phase), (b) a *detector* —
"a node marked `sunsetting` still has active dependents / consumers who were never told" (the
retirement analog of `unsatisfied_requirement`), reusing the detect-and-ask loop, and (c)
modelling the *replacement/migration* (which capability supersedes it, where the data goes) as
Decisions + `EVOLVES_INTO`/`OBSOLETES` — the same pattern BL-63 showed is how ownership history
should be recorded. Interim mitigation (cheap, do first): document that the removal blast radius
(`propagate_from` with a removal framing) *is* the retirement impact-check today.

**BL-65 · Risk & security are inference edges, not a lifecycle-spanning concern** — *user, UAF
+ DevSecOps analysis, 2026-07-21.* Concept; size **L**; needs the user on vocabulary.

Two commercial/defense lineages converge on the same gap. **UAF** embeds Security & Risk
viewpoints at *every* phase (concept → design → field), never bolted on. **DevSecOps** makes
that continuous and automated — SAST/SCA on every commit, security as a shift-left gate, not an
end-of-line compliance check. reflow2 has neither shape: risk exists only as inference *edges*
(`RISKS`, `MITIGATES`, `BLOCKS`), there is no `Risk` / `Threat` / `Control` / `SecurityAsset`
node (node-type probe confirms), and — the load-bearing gap — **the coherence loop has no
detector for the *absence* of a risk/security assessment.** It flags an unsatisfied requirement;
it never flags "this capability crosses a trust boundary or handles sensitive data and no risk
was assessed here." The seed exists: `EnvironmentRule` + `COMPLIES_WITH` / `VIOLATES_RULE` is a
compliance layer, and `cap:freshness`'s confirmation ledger is the pattern for "a claim nobody
has re-checked."

Fix, three layers: (a) **a first-class `Risk` node** (likelihood / impact / status), linked via
`RISKS` / `MITIGATES` to what it threatens and `CONSTRAINS` to what bounds it — optionally
`Threat` / `Control` for a fuller security model. (b) **A cross-cutting detector** —
`unassessed_risk`: a node past a phase gate, crossing a boundary or marked sensitive, with no
linked risk assessment, fires a gap through the *existing* detect-and-ask loop (this is the UAF
"every phase" principle expressed as reflow2's native "detect the silence" move). (c) **Continuous
automated governance** (the DevSecOps angle): compliance/security is reconciled the same way
artifacts are — a caller (CI, a scanner) supplies observations, reflow2 reports drift, and a
**security-debt ledger** (mirroring the confirmation ledger) shows what is `assessed` /
`drifting` / `unexamined` per node. Interim: document that `RISKS`/`MITIGATES` + `EnvironmentRule`
are today's tools and that reflow2 does not yet detect their *absence*.

Both BL-64 and BL-65 deliberately reuse propagate + detect-and-ask rather than inventing
subsystems; the genuinely new part in each is *vocabulary* (a lifecycle state; a Risk node),
which is a design decision for the user — hence "concept, needs the user."

**BL-66 · Design coherence as a consumer CI gate (shift-left the golden thread) — DONE
2026-07-21** — *user, commercial-practice analysis (DevOps/shift-left), 2026-07-21.* Size ~~S–M~~.

**Built**: `tools/reflow2_check.py` — stdlib-only, self-contained (embeds the reflow2_cli stdio
client so it ships alone in the kit tarball; release.yml carries it). Imports the **committed
export** into a temp graph (decision made by evidence, not preference: `.reflow2/` is
gitignored so CI *cannot* open it, and the committed export is the design the team actually
reviewed), rehashes every registered Artifact from the working tree — truncating sha256 to each
registered checksum's own length, so any registration dialect works — reconciles, runs
`detect_gaps`. **Fails (exit 1)** on unaccepted `checksum_change`/`missing` (an accepted drift
updates the export, so red = the two-sided accept was skipped) and on open **anchored** gaps at
severity ≥ 0.8 (`--gap-threshold`); `acknowledge_gap` is the sanctioned way to go green without
fixing, so the gate inherits DETECT's own review mechanism instead of inventing a mute flag.
Phase nudges and `no_baseline` print as notes, never gate; exit 2 (cannot run) is loud, never a
silent pass. Verified three-way on reflow2 itself: clean tree passes, a doctored `budget.rs`
fails with the named artifact, a missing export refuses with instructions. Shipped as the
**ci-gate** skill (setup, GitHub Actions snippet, and the honest ways to turn red green —
including the two launderings it names and forbids) plus a SETUP.md pointer. There is
deliberately no flag to skip the drift check.

DevOps' deepest principle is that verification runs on **every commit**, not at a milestone
gate. reflow2 gave *itself* CI (BL-52), but a **consumer** project has no documented, one-step
way to run reflow2's detectors as a build gate on its own commits — so the golden thread is
checked periodically (a session), not continuously. Every piece already exists: the CLI
(`reflow2-mcp --import/--export`, `reflow2_cli.py`), `reconcile_artifacts` (caller supplies the
observed hashes — a CI step computes them), `detect_gaps`, the two-sided drift accept. What is
missing is the *assembly*: a single `reflow2 check` verb (or a documented pipeline step) that
(a) recomputes artifact hashes from disk, (b) reconciles against the committed export, (c) runs
`detect_gaps`, and (d) **exits non-zero** when a design drifts from its build with no two-sided
accept, or a new critical/anchored gap appears — fail-loud, never a silent pass. Ship it as a
consumer skill + a copy-paste CI snippet (the SessionStart-hook pattern from BL-50 (3) is the
model). Decisions: what severity fails the build (unaccepted `checksum_change`? a new critical
gap? a regressed `unrealized_capability`?), and read-from-committed-export vs open the live
`.reflow2/graph` (single-writer) in CI. This makes everything reflow2 already does
*continuous*, which is the whole point of the frictionless-cadence thread ([BL-51], [BL-15]).

**BL-67 · Requirements as live measured objectives — SLO/SLI reconciliation (as-operating)** —
*user, commercial-practice analysis (SRE), 2026-07-21.* Concept; size **M–L**; needs the user
on the vocabulary call.

SRE's move is that a spec is not a static statement (MTBF) but a **live objective** (SLO)
measured by **live indicators** (SLIs) against an **error budget**. This is the one commercial
practice that genuinely *extends what reflow2 can be* — from design ↔ built ↔ fielded, to design
↔ **running reality**: *is the deployed system meeting its measured objectives right now?* And
it reuses reflow2's own architecture almost entirely:
- **The SLO is a `Verification(method=measurement)`** with a target — the schema already has
  `method: measurement` and a `passing`/`failing` status. No new node type strictly required.
- **A new reconcile-family op, `reconcile_operating`** — the caller (a monitoring system,
  Prometheus, a CI probe) supplies observed SLI values exactly the way `reconcile_artifacts`
  supplies checksums (the "core does no I/O, the surface observes" seam); reflow2 compares
  against the SLO target, sets the Verification `passing`/`failing`, records the divergence, and
  `propagation_seeds` walk **up** the thread to the Capability/Requirement — the as-fielded
  pattern, one axis further.
- **The error budget is a `Constraint`** (`direction: maximum`, `quantity` = the budgeted
  metric) whose contribution is the live-consumed budget — `budget_report` already rolls this up.
- **`dimension_drift`** already detects an SLI *trend* declining over time, and `cap:freshness`'s
  confirmation ledger is already SRE-adjacent ("a claim nobody re-checked is stale"). An
  **as-operating** viewpoint would join the as-designed/built/fielded/verified set in
  `render_views`.

So the vocabulary decision for the user is small — "is an SLO a measurement-Verification with a
target, or does it deserve its own node?" — and most of the machinery (reconcile seam, Constraint
budget, dimensions, freshness) is already there. Closes the loop from intent all the way to the
telemetry of the running system.

**BL-68 · Readiness-driven roadmapping — derive the delivery timeline, don't declare it**
(keystone) — *user, Space Force acquisitions/SE, 2026-07-21.* Concept; size **L**; the most
ambitious item on the board. Needs the user on vocabulary. Unifies and gives purpose to
[BL-64] (lifecycle phasing), [BL-65] (risk), and [BL-67] (the modelled future).

The problem, in the user's words: on real programs, "people didn't understand which *epoch* a
design would be delivered on." Roadmaps are drawn as slides, disconnected from the actual
maturity of the enabling technology, so the delivery timeline is an assertion nobody can defend.
Meanwhile a design is not static under incremental development — **Version A is achievable today
because its enabling tech is at acceptable TRL/MRL; Version X is a decade out because a key
technology is immature now and expected to mature later.** The LLM's job is to help the user say
"*here is what we can build today, and here is the improved version 10 years out — that is the
roadmap*," and to make that claim traceable.

Three parts:

- **(1) Readiness and the -ilities as first-class scored risk factors that GATE achievability.**
  TRL, MRL, affordability, maintainability, reliability — all *risk factors in a design choice*,
  same family as BL-65 (a low TRL *is* a risk). reflow2 already scores `maturity`/`reliability`/
  `maintainability` as `DimensionAssessment`s, but a dimension only *trends*; it does not *gate*.
  New: a readiness assessment (TRL/MRL 1–9) whose being below threshold marks a design increment
  **not buildable yet** — and a **forecast** of that score over time (TRL 3 now, 7 expected 2035)
  so the timeline can be computed forward.
- **(2) Design increments/alternatives as comparable first-class entities.** reflow2 models
  *one* coherent design; source selection and incremental development need a *family* of
  candidates (Version A vs X) that share requirements but differ in what is achievable when,
  each scored on TRL/MRL/-ilities. Today the only trace of this is `Decision.alternatives` — an
  opaque prose string ("options considered and why they lost"). New: increments/options as
  living nodes you can score, propagate through, and pin to epochs.
- **(3) The derived roadmap — the insight that is reflow2's to claim.** Because the golden thread
  runs capability → component → enabling technology, and each technology carries a readiness
  score with a forecast, **the epoch an increment can deliver on is *computable*, not declared**:
  it is the epoch by which every enabling technology on its thread reaches acceptable TRL/MRL.
  `propagate` already walks that thread; feeding it readiness turns "which epoch delivers which
  design" from an opinion into a traceable output — *"this increment is 2036 because THIS
  technology is TRL 3 today, projected 7 in 2035, and the capability cannot close without it."*
  That is exactly the legibility the user's programs lacked.

**The spine (state this in any design of the feature): the roadmap is a risk-burndown schedule.**
Each epoch is the point where enough readiness risk has retired to make the next increment
achievable; readiness maturing *is* the risk clearing. This single framing unifies BL-65 (risk),
BL-67 (the future), and this item: the roadmap is *when the risk clears*.

Worked example (the user's): "refuel a satellite by laser" → capabilities {high-power lasing,
beam pointing/tracking, power→light conversion, thermal management} → each traces to components
and technologies with a TRL. Today's design = the increment whose whole thread is mature now;
the 10-year design = the increment gated on (e.g.) high-efficiency power→laser conversion
maturing TRL 3 → 7. reflow2 propagates the gate and can name why the later increment is later.

Seeded vs new — **seeded**: the `maturity`/`reliability`/`maintainability` dimensions, the
temporal axis (epochs, `ANTICIPATES`, `EVOLVES_INTO`), propagate + the thread, the
`Decision.alternatives` prose. **New**: TRL/MRL as a *gating* readiness assessment; a readiness
*forecast* over time (the quantitative form of BL-67's "model the future"); increments/
alternatives as first-class comparable nodes; and the derived-roadmap computation.

Vocabulary decisions that are the user's to make (why this is concept, not spec):
1. Is an increment/alternative a **new node type**, a variant of `Release` (Releases are
   as-*built*; these are as-*planned* candidates — lean: new node), or a scoped sub-graph?
2. Is readiness a **new assessment kind** with gate-semantics, or a `trl`/`mrl` addition to the
   dimension enum? (Lean: its own construct, precisely because it *gates* rather than *trends*.)
3. How is a readiness **forecast over time** modelled so the roadmap computes forward
   (a TemporalFact series on the technology? a projected `DimensionObservation` per epoch?).

Why it matters: no roadmapping tool today *derives and defends* the delivery timeline from the
real readiness of the technology — they assert it. reflow2's thread + propagate makes derivation
possible, which is a capability, not a viewpoint.

**BL-69 · `single_point_of_failure` measured connectivity on the wrong graph — DONE 2026-07-21** —
*self-host review, 2026-07-21, while dispositioning the two SPOF warnings on reflow2's own graph.*
Size **S–M**. ~~S–M~~

Raised because `detect_defects` flagged `cmp:flow` and `cmp:service`, while an independent
articulation-point (Tarjan) analysis of the operational dependency graph said the true cut
vertices were `cmp:service`, `cmp:export`, `cmp:graph` — wrong in both directions at once. The
entry's first diagnosis (community bridges) was wrong about the mechanism; reading the source
corrected it: the detector already ran a genuine removal-splits-the-graph test, baseline-relative
(BL-5 pass 1) with operational candidates (pass 2) and the library filter (pass 3, F6) — but it
measured connectivity on the **full design network**, where intent edges are wrong in both
directions at once:

- **They donate mass.** Removing `cmp:flow` strands its own intent cluster (`cap:model-process` +
  `art:flow` + verification) — ≥2 nodes, so the non-trivial filter passed and a healthily-modelled
  leaf module fired. The severed "subsystem" was made of sentences.
- **They donate phantom connectivity.** Removing `cmp:export` severs `cmp:init`+`ifc:graph-export`
  operationally, but the design network kept them "connected" through
  `art:init REALIZES cap:kit SATISFIES …` — a path that carries nothing at run time — so a real
  cut vertex stayed silent.

**The fix (the fourth pass at this detector, and the same selectivity lesson one level deeper):**
connectivity and candidate enumeration both moved to the **as-built operational network** —
Components/Interfaces/Resources/Environments plus the Artifacts realizing them, joined by the
traceability edges that hold between such nodes. Intent nodes not only must not be *flagged*
(pass 2); they must not *participate in the connectivity being measured*. Artifacts are members
(a stranded part with its file is a real severed subsystem — the fixture for the pinned
interface-bridge test had already padded its subsystems with artifacts to pass the non-trivial
filter, so this codifies what the doctrine already practiced) but never candidates. Every prior
lesson kept: baseline-relative, non-trivial ≥2, library exclusion.

**Measured on reflow2's own graph** (`build_design_graph.py --analyse-only`, before → after):
SPOF `{cmp:flow, cmp:service}` → `{cmp:graph, cmp:export, ifc:graph-export, cmp:service}`.
`cmp:flow` stops firing; its community-bridge signal stays in `surprising_connections` under its
accurate name. Three findings are new-and-true: `cmp:export` and `ifc:graph-export` are the only
route from the kit's converter (`cmp:init`) to the design, and `cmp:graph` genuinely strands
`schema`/`search`/`vocabulary` (each with its file) plus the whole export chain. All four are now
answered on the record: `cmp:service` by `dec:service-spof-accepted`, `cmp:graph` by
`dec:graph-spof-accepted` (the single store handle is the architecture, and a second one would be
two write paths to one store), and the `cmp:export`+`ifc:graph-export` chain by
`dec:export-door-spof-accepted` (one canonical, deterministic portability format is the feature;
a second export path would be a second source of truth). The defect
count *rising* 2 → 4 while the false positive leaves is the fix: the count was previously wrong
in both directions. Two regression tests pin the two shapes (intent-cluster stranding must not
fire; a cut vertex hidden by intent edges must); the island-immunity fixture was rebuilt with
operational subsystems, preserving its lesson. All 14 structural tests, workspace suites, and the
instruments (phase 13/13, erosion 7/8, coherent 9/9, model_the_loop, smoke) at their baselines.

**BL-70 · Parallel alternatives — AoA branches held open until a decision point** — *user,
source-selection practice (analysis of alternatives / DOTMLPF-P), 2026-07-21.* Concept; size
**L**; the vocabulary decisions are the user's (and shared with [BL-68]'s question 1).

The idea in the user's words: an undecided design choice could hold **forks** — option A and
option B (and more) as live sub-designs — and, from military source selection, an analysis of
alternatives keeps two or more parallel designs *viable until some decision is made* — "almost
like a decision point."

More is seeded than expected:

- **`Decision.status = proposed` is a decision point in embryo** — the node can exist *before*
  the choice is made, with `GOVERNED_BY` edges already saying which parts of the design hang on
  it. Nothing today makes a `proposed` Decision *gate* anything; that is the missing teeth.
- **`Decision.alternatives` is the losers' obituary** — prose, post-hoc, written on the winner.
  The fork idea upgrades exactly this field: alternatives as live sub-graphs while the choice is
  open, collapsing into that record when it closes — real history instead of reconstruction.
- The edge vocabulary mostly exists: `CONTRADICTS` (opposing), `EVOLVES_INTO`, `OBSOLETES`,
  `ANTICIPATES` can wire branches to each other; `retire-from-design` is the losing branch's
  exit (superseded — genuine history retired on the record, not a mistake deleted).
- **The comparison machinery an AoA needs is the machinery that already exists** —
  `budget_report`, the dimension assessments, [BL-68]'s readiness scores — run *per branch*,
  they make alternatives comparable on the same measures instead of on advocacy. BL-68's
  vocabulary question 1 (increments/alternatives as first-class nodes) is this same question at
  roadmap scale; one answer should serve both.

What is genuinely missing is one primitive: **the graph is single-world.** Two Components both
`SATISFIES`-ing one Requirement *on purpose* is indistinguishable from the incoherence the
detectors hunt (`possible_duplicate`, allocation defects) — a second viable design held in the
same graph would be punished for existing. The need is a **scope**: nodes and edges tagged to an
alternative ("world"), DETECT running *within* a world, reports comparable *across* worlds, and
the decision point collapsing the superposition — winner merges into the baseline, loser retired
with its rationale. Cheapest first increment, no schema change: **one exported graph per
alternative plus a cross-export comparison report** — export/import already round-trips a whole
design deterministically, so branch-by-file works today and teaches what the real scoping
primitive must preserve.

**DOTMLPF-P is the breadth discipline for *generating* branches**: the alternative to a materiel
Component may be non-materiel — doctrine, organization, training, a process change. reflow2 is
unusually placed to hold that honestly: `req:design-anything` + `Flow` ([BL-37]) mean one branch
can be a process satisfying the requirement while a sibling branch is a product — the same
decision point gating a materiel and a non-materiel solution in one graph, which is exactly the
comparison a source selection is supposed to make and rarely gets tool support for.

Decisions that are the user's to make: branch as node-set tag vs sub-graph vs graph-per-branch;
does DETECT run per-world only, or is a cross-world absence itself a gap ("this requirement is
satisfied in only one alternative")?; and where the line falls between an alternative (design
space, `CONTRADICTS`) and an epoch (time, `EVOLVES_INTO`) — the AoA that keeps both branches is
describing space, not history, and the vocabulary should not conflate them. Connects to
[BL-44]: a cluster checkout that explores an option rather than progressing the baseline is this
item's fork — the two likely share the scoping primitive (see BL-44's 2026-07-21 addendum).

**BL-71 · Two models of one design: the curated rebuild clobbers the accumulated live graph** —
*found 2026-07-21 while modelling the v0.6/v0.7 Release nodes.* Size **M**; needs a
reconciliation decision before code.

`tools/build_design_graph.py` (full run) rebuilds the curated self-model from source — 184
nodes — and writes it to `docs/design/reflow2.json`, the same path the live sessions export the
**accumulated** graph to (247 nodes at the time: everything the curated model has *plus* the
session-written layer — freshness-claim ChangeEvents, the SPOF-acceptance Decisions, Questions,
the BL-63/66/69 change events, `art:check`). The full rebuild therefore silently **discards the
live layer from the committed record**; it happened live and was caught only because the node
count dropped, then restored from git. The two writers disagree about what the file *is*: the
rebuild treats it as a projection of source, the sessions treat it as the durable record of the
graph (SETUP.md's own doctrine: "the export is the durable record").

Sharper statement: the curated model and the live graph have **diverged as designs** — 18 vs 10
Requirements, 10 vs 9 Decisions — and nothing detects or reconciles that. This is drift between
two as-designed records, a case none of the three reconcile tools covers (they all compare
design against *reality* — disk, deployment, test runs — never design against design).

Decisions needed: **(a)** is the committed export the rebuild's output or the live graph's
export — one of them needs a different file, or the rebuild needs to *import-then-layer* rather
than replace (its own import is upsert, so rebuild-into-the-live-graph may already be the
answer: run the curated pass INTO the accumulated graph, then export the union); **(b)** should
`import_graph`/the export flow warn when an import would *shrink* the graph it replaces (a
node-count drop is exactly the silent-loss signature that caught this); **(c)** does
`reconcile_artifacts`' sibling — a design-vs-design diff — deserve to exist (it is also what
BL-70's cross-branch comparison needs, and the export is deterministic precisely so two of them
diff cleanly). Until then: **do not run the full `build_design_graph.py` after live-session
graph writes without re-exporting the live graph afterwards** — the tool's release model
(v0.4.0–v0.7.0, added 2026-07-21) reaches the live graph only via that reconciliation.

## Deliberate deferrals

Not gaps — decisions, recorded so they aren't rediscovered as bugs.

- **WS-4 `EnvironmentRule` / WS-5 `QualityGate`** — nothing reads or asks for either type, so a
  constructor would build the mirror image of the problem WS-1..3 fixed. Each lands with its
  detector.
- **Real LLM provider backends** — unnecessary agent-native; the ambient agent is the LLM.
- **`EmbeddingBackend`** — semantic dedup and retrieval. The audit has prior art on shape
  (local MiniLM/384-dim, normalized inner product, hash-gated rebuild) and one caution: retrieval
  thresholds are not identity thresholds.
- **Generative HEAL content** — proposals stay review-gated stubs.
- **Bayesian architecture optimization** — assessed and dropped; see the audit's do-not-port list.

## Recurring lesson

A capability exists in core and is unreachable or unadvertised on the surface. Fourteen instances so
far: `Interface`, HEAL's skill, the `Verification`/operate write side, `contain_component`,
`graph_id`, `Requirement.status`, `graph_report` as an answer to "where am I", the whole
`TemporalFact` / `ABOUT_ENTITY` / `VALID_FROM` / `VALID_TO` layer (schema-complete, zero Rust
API), `DOCUMENTS` (declared, named in `nodes.rs`, no constructor and no tool — closed 2026-07-20
by BL-26's write side), and `precedes`
(implemented in `temporal.rs`, no tool, so the epoch chain axis Z exists to record cannot be drawn by
any client — BL-36), and `Flow` (fully specified with its own edge `PART_OF_FLOW`, no constructor,
no tool — so no process could be modelled at all; BL-37 built the write side and `flow_report`), and `DriftEvent.resolved` (declared with
`default: false`, written by nothing — every recorded divergence stayed "open" forever no matter
what happened next; BL-35 made the accept flip it), and `pin_at_epoch` (generic in core since the
temporal module landed, `AT_EPOCH` declared `from: "*"` — and no tool, so nothing could pin a
Release to its own `release_cut` epoch; BL-34 exposed it), and `Constraint` (fourteenth: named in
`nodes.rs`, fully specified in the schema with a `budget` category — no constructor, no tool, so
no limit could ever be recorded; BL-11 built `add_constraint`/`constrains`).

Before building something new, the higher-yield question is usually: **what does the core
already do that nothing can reach?**

The sibling lesson, learned the same way: a capability can also be unreachable because nothing
*points at it*. The consumer kit's skills were installed where three of four harnesses never look
(BL-22), and `describe_schema` would have been invisible to the people who needed it had the kit
not been updated in the same change (BL-1). Shipping the code is not shipping the capability.

Third variant, from the [self-host genesis trial](trials/2026-07-18-selfhost-genesis.md): a
capability can be unreachable **on one harness only**. BL-28's untyped `JsonValue` parameters worked
from grok build and fail from Claude Code, because a schema that declares no type leaves
marshalling to the client and the two clients choose differently. The same shape appears on the
response side (the array `structuredContent` bug, `delete_node`, `graph_report_markdown`). The
generalisation: **anywhere the tool surface declines to state a type, a client is free to guess,
and our test client's guess is not evidence.** `tools/smoke_mcp.py` is green on all five broken
params. Asserting the *schema* — no advertised property without a type — is a different check from
asserting behaviour through a client we wrote, and it is the one that catches this class.
