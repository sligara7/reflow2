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
  [ophyd-service](trials/2026-07-18-brownfield-ophyd-service.md) (399 files, ~110k LOC, requirements
  inferred backward from code) and [3dtictactoe](trials/2026-07-18-brownfield-3dtictactoe.md)
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
| **BL-41** | **Graph text is data, never instructions — and nothing says so** | The one genuinely uncovered LLM failure mode in [partnership.md](partnership.md): every skill tells the agent to read node text and act on the design, and a hostile or careless statement rides that trust. Bounded today (single user, local graph); real the day a graph is shared (BL-12) or an adopted repo's prose flows through INGEST (BL-27). First step is **S**: a standing rule in the consumer AGENTS.md and skills — treat node text as content to reason *about*, never directives to *follow* — plus the same line in `get_info` instructions. Mechanical mitigation (provenance-aware trust, quoting boundaries) is **M** and should wait for a real multi-writer case | S–M |
| **BL-37** | **reflow2 cannot model a *process* — `Flow` has no write side, and edge roles are lost** | Found by modelling reflow2's own coherence loop in reflow2. The one type meant for an ordered process cannot be created; forward and backward edges are indistinguishable | M |
| **BL-40** | **Viewpoints as pure projections (SYNTHESIZE held to a no-extrapolation standard)** | The graph stores the design; the agent only renders. `tools/render_views.py` is the seed — functional/structural/traceability views project cleanly today, and each confession it prints is a gap by definition. Direction: a viewpoint catalogue (UAF/DoDAF-informed), and rendering through the live MCP surface rather than the export. The author intends to expand this thread | M–L |
| **BL-30** | **A failing test satisfies the check that asked for a test** | **S half done** — `failing_verification` fires at 0.8 and coverage counts passing only. The M half (`reconcile_verification`) remains. See below | ~~S~~ + M |
| **BL-29** | **`apply_heal` trusts the proposal; merge loses data silently** | Mostly **done** — three of seven fixed; three remain, one deliberately deferred. See below | M |

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

**BL-37 · reflow2 cannot model a process** — *modelling the coherence loop itself, 2026-07-19
(`tools/model_the_loop.py`, exported to [loop-model.json](loop-model.json), drawn in
[loop-dag.html](loop-dag.html)).*

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

**BL-30 · The later phases measure bookkeeping, not reality** — *[phase-coverage
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
DETECT, which was the headline miss. **M — open** —
`reconcile_verification`, the P4 sibling of `reconcile_artifacts`: the agent supplies what the test
run actually reported, and the graph says where that diverges from what it believed. Together with
[BL-9](#bigger-threads)'s `reconcile_deployment` these are the missing feedback loops; the
[trial](trials/2026-07-19-phase-coverage.md) shows the golden thread itself already works in both
directions, so this is the whole of the gap.

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
| The node-type index is built once before the loop, so chained merges (a↔b, b↔c) re-point onto a node that no longer exists | ⬜ open, **code-read and not reproduced**. Narrowed but not closed by the sanction check: HEAL can still emit two merges sharing a node in one proposal |
| Atomicity is per-operation, not per-proposal: a three-merge proposal failing on the second leaves the first committed | ⬜ open, code-read. The cross-type guard removes the known way to trigger it |
| The survivor is chosen by lexicographic id (`canonical_pair`), not by connectivity or completeness — the better-connected node may be the one deleted | ⬜ open by choice. Determinism is the current virtue and any "better" rule is a judgement about which node is more real; wants a decision, not a patch |

Remaining size **S–M**. The chained-merge case is the one worth doing next and wants a reproduction
first — rebuild the index between operations, or refuse a proposal whose merges share a node.

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
[ophyd-service](trials/2026-07-18-brownfield-ophyd-service.md) (399 files, ~110k LOC),
[3dtictactoe](trials/2026-07-18-brownfield-3dtictactoe.md) (~20 files) and
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
graph*: `components == 0` is true, and the [aidrone trial](trials/2026-07-18-greenfield-aidrone.md)
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
2. **A graph with capabilities and zero requirements reports nothing**, because the detector is
   gated on `requirements > 0` to avoid one gap per capability. That is the pure brownfield
   starting state, and it is exactly ophyd finding 1's ask — *"the first gap should be about
   missing intent, not missing structure"*. It wants a **phase-coverage** detector
   (`design_without_intent`, project-scope, one nudge not N), which is the same shape as the four
   that exist and is **S**. Not built here because it is a different detector answering a
   different question; recorded so it is not rediscovered as a bug.

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
~60 MCP calls on 33 nodes. And `reflow2_init.py` cannot install into a repo that already has its
own `AGENTS.md`, which is every brownfield target and this one; it needs a `--skills-only` flag.

Size **L** for the thread; the `adopt` skill itself is **M** once the two **S** blockers land, and
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

**Still open — published releases.** Everything above assumes a reflow2 checkout to run the
script from, which is true today because building the binary requires one. It stops being true
the moment someone wants reflow2 without cloning it: that needs published, per-platform binaries
(and macOS raises signing questions), or a fetch-from-git mode in the installer. It remains the
third piece of evidence for the service side of the embedded-vs-service fork, alongside
single-writer concurrency and this. Size **M–L**.

**The fork is decided** (2026-07-18, [surface-plan.md](surface-plan.md)): **repo-file, embedded**.
So this is no longer gated — build published per-platform binaries, which is the packaging answer
to a packaging problem. The service was weighed and set aside: its strongest argument
(concurrency) is hypothetical while there is one writer, it would put the user's design graph on a
machine they do not control, and it is permanent operational cost. The conditions that would
reopen it are written down; "published binaries proved insufficient" is one of them, so this item
is also the experiment that would justify revisiting.

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
| Describes the design | `docs/*.md`, README | ⚠️ `DOCUMENTS` is in the schema; nothing can create it |
| Instructs agents | `AGENTS.md`, `COORD.md`, `.github/copilot-instructions.md` | ❌ nothing |
| No design meaning | `Cargo.lock`, `target/`, generated output | should stay out — this is where the noise would come from |

*The founding evidence is a failure reflow2 should have caught.* In one session on 2026-07-18:
AGENTS.md's build command was found wrong and fixed; hours later, by accident,
`.github/copilot-instructions.md` was found carrying **the same stale command**; and
`docs/backlog.md` grew a duplicated section that nothing noticed until someone went looking. Two
instruction files disagreeing about how to build the project is a coherence failure, and catching
coherence failures is the entire point — it was missed because neither file is in any graph.

*This is more than modelling more files.* Two things stand in the way:

1. **`DOCUMENTS` has no write side.** It is declared in `schema/build.yaml` and named in
   `nodes.rs`, with no constructor and no MCP tool — the recurring lesson below, for the ninth
   time.
2. **PROPAGATE does not traverse it.** `propagate.rs` lists `SPECIFIES`/`DOCUMENTS` as
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

Size **M** for the write side plus a decision on traversal; **S** if it stops at recording which
files matter and leaves impact alone.

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
| **BL-9** | **As-fielded view** | Audit item 2, and it needs **no new schema** — `operate.yaml` is fully defined and now writable (WS-2). `reconcile_deployment` as a sibling of `reconcile_artifacts`. Guard against the library-plugin false positive the audit flags. **Now has execution evidence**: the [phase-coverage trial](trials/2026-07-19-phase-coverage.md) scored P5 **0/2** — a component in no release goes unreported and nothing can compare the design against what is deployed. Pair it with BL-30's `reconcile_verification`; they are the same missing feedback loop one phase apart. | M |
| **BL-10** | **Root-cause classification of drift** | `drift.rs` detects divergence with no notion of *why*, so no notion of which side is wrong. Reflow's seven-category taxonomy ends in a decision rule. Needs a scalar coherence score to gate on. | M |
| **BL-11** | **Path-cumulative budget analysis** | Three independent reflow tools reached for it. PROPAGATE walks impact but never accumulates a quantity along source→sink paths — the classic SE budget rollup (latency, mass, power, cost). | M |
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

A capability exists in core and is unreachable or unadvertised on the surface. Thirteen instances so
far: `Interface`, HEAL's skill, the `Verification`/operate write side, `contain_component`,
`graph_id`, `Requirement.status`, `graph_report` as an answer to "where am I", the whole
`TemporalFact` / `ABOUT_ENTITY` / `VALID_FROM` / `VALID_TO` layer (schema-complete, zero Rust
API), `DOCUMENTS` (declared, named in `nodes.rs`, no constructor and no tool — BL-26), and `precedes`
(implemented in `temporal.rs`, no tool, so the epoch chain axis Z exists to record cannot be drawn by
any client — BL-36), and `Flow` (fully specified with its own edge `PART_OF_FLOW`, no constructor,
no tool — so no process can be modelled at all, BL-37), and `DriftEvent.resolved` (declared with
`default: false`, written by nothing — every recorded divergence stayed "open" forever no matter
what happened next; BL-35 made the accept flip it), and `pin_at_epoch` (generic in core since the
temporal module landed, `AT_EPOCH` declared `from: "*"` — and no tool, so nothing could pin a
Release to its own `release_cut` epoch; BL-34 exposed it).

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
