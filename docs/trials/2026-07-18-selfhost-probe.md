# Self-host probe — reflow2 modelling reflow2, 2026-07-18

**What:** reflow2's own design pushed into a reflow2 graph and interrogated with the shipped
detectors. Read-mostly: the graph lived in `/tmp`, the docs stayed the source of truth, nothing
in the repo was written and reflow2 was *not* installed into itself (`reflow2_init.py` would have
overwritten this repo's own `AGENTS.md` with the consumer kit's).

**Why:** [BL-13](../backlog.md) names scale as untested — every fixture is 3–10 nodes. This is
119. The question was whether the detectors say anything useful about a real project, or drown it.

**Harness:** a throwaway script driving the built MCP binary over stdio, the same protocol path an
agent uses. Not committed; the numbers below are what matters.

## How the design was modelled

Derived from what already exists, not invented:

| Node | Source | Count |
|---|---|---|
| Requirement | one per row of `requirements-coverage.md`, status from the ✅/🟡/⬜/➖ column | 72 |
| Capability | one per documented concern (the section each row sits under) | 7 |
| Component | one per crate, `subsystem` level, `CONTAINS`-ed by the Project | 2 |
| Artifact | one per `crates/reflow2-core/src/*.rs`, checksummed, `REALIZES` its capability | 22 |
| Verification | one per `tests/*.rs`, `VERIFIES` the capability it covers | 15 |

**Confound, stated up front:** these mappings are judgement calls and they shape the results.
Linking each test to a *capability* rather than to individual files is the choice that produces
finding 1 below — though it is also the modelling the typed tools nudge you toward, and the one a
person would naturally write.

## What it said

```
119 nodes · 25 gaps · 8 structural defects

  22  unverified_artifact          ← 88% of the gap list
   2  orphan_level
   1  no_deploy_operate

   7  single_point_of_failure
   1  disconnected_community

   0  surprising couplings
```

## Findings

### 1. One detector is 88% of the gap list, and it is not a bug

Every one of the 22 source files raises "No verification covers *X*". The detector is behaving
exactly as specified — linking a test directly to `art:detect` clears that one gap and only that
one. So this is not a defect; it is a **demand for 22 bookkeeping edges** that say nothing a
person would otherwise record.

This is the Grok trial's §6 complaint reproduced at scale, and it shows the limits of what
[BL-6](../backlog.md) fixed. BL-6 corrected the *label* ("Nothing verifies reading.py" →
"No verification covers reading.py"). The **volume** is untouched, and volume is what makes a list
get skimmed. The blind trial's warning applies directly: a gap list that can never reach zero
trains you to ignore it.

The Grok trial already proposed the fix and it remains unimplemented: *"Prefer only
capability/requirement verification gaps, or collapse artifact-level ones under the capability
they realize."* A per-file `VERIFIES` edge is a plausible thing to *want* on a safety-critical
build; it should not be the default demand on a 22-module crate whose capabilities are all tested.

### 2. `single_point_of_failure` fires on leaf capabilities — BL-5, with the mechanism visible

Seven of them, on a design with two components. The message is the tell:

> every path between subsystems routes through `cap:ingest` — a single point of failure

`cap:ingest` is a *leaf*. The only thing it separates is the two Requirements hanging off it.
`structure.rs` claims selectivity — firing "only when a node separates ≥2 real subsystems" — but
two requirements attached to one capability are counted as a subsystem. This is
[BL-5](../backlog.md) ("all 15 defects vanished at once when I added two bookkeeping edges")
with a concrete cause rather than an anecdote: the threshold is on *count of separated groups*,
not on whether the groups are substantial. `surprises.rs` already learned this lesson and has
`MIN_COMMUNITY = 3`; `structure.rs` has no equivalent.

### 3. A Component under a Project is "floating" — new

Both crates raise `orphan_level`. They are `subsystem`-level Components `CONTAINS`-ed by the
Project, which is the modelling the tools lead you to — but `hierarchy_issues` only looks at
`Component CONTAINS Component`, so a top-level subsystem has no parent *of a kind it recognises*
and is reported as floating.

So the natural shape for any project — a Project holding a few subsystems — produces one false
`orphan_level` per subsystem. Either Project containment should count as a parent for the level
check, or `orphan_level` should not fire on a component the Project directly contains. Not
previously reported; it needed a design with real top-level structure to show up.

### 4. What worked, and is worth saying

- **`no_deploy_operate` is correct.** reflow2 genuinely has no Release, Environment or Resource
  modelled. One true gap, correctly found.
- **Allocation health reads true**: modularity 1.00 across 2 components, 0 misplaced capabilities,
  no god-components. That matches the real core/surface split.
- **Requirement status carried the coverage matrix faithfully** — 32 met, 27 accepted, 10
  deferred, 3 dropped, straight from the ✅/🟡/⬜/➖ column. This was BL-3, shipped hours earlier,
  and it is what let the graph express a partial state at all.
- **119 nodes was not a performance problem.** Build and full interrogation ran in seconds.
- **No surprising couplings** — but the model has no lateral `DEPENDS_ON` edges, so this neither
  confirms nor challenges [BL-6b](../backlog.md).

## The honest verdict

At this scale the gap list is **1 true gap, 24 arguable ones**. A person opening it would read
three lines of "No verification covers …", conclude the tool does not understand the project, and
stop reading — the exact failure the gap/reviewed split exists to prevent.

None of the three noisy detectors is *wrong* by its own specification. All three demand
bookkeeping that a design's author would not otherwise write, and at 119 nodes that bookkeeping
dominates. The pattern across findings 1–3 is one thing: **thresholds tuned on 3–10 node fixtures
do not hold at 100+.** `surprises.rs` already hit this and answered it with `MIN_COMMUNITY = 3`;
the same lesson has not reached `detect.rs`'s artifact rule or `structure.rs`'s articulation-point
rule.

Self-hosting is worth continuing, but the graph should not become authoritative until
[BL-4](../backlog.md) lands — a design brain that forgets between sessions is worse than a
document you can read.

## Outcome, same day

Re-run against the same 119-node graph after each fix:

| | gaps | |
|---|---|---|
| as probed | 25 | 22 artifact-coverage, 2 orphan_level, 1 true |
| after **BL-23** | 3 | per-file coverage became a `graph_report` statistic |
| after **BL-24** | **1** | `no_deploy_operate` — correct; reflow2 has no operate layer |

Both fixes were small and neither weakened detection: capabilities are still asked about, and a
component nothing contains is still an orphan. What changed is that the two rules stopped
demanding bookkeeping the design's author would not write.

**BL-5 too**, and the cause was not the one this probe guessed. Finding 2 above blamed the
`≥2 nodes` threshold, by analogy with `surprises.rs`. Reproducing the shape in a test showed
otherwise: two capabilities correctly *not* flagged became single points of failure the moment an
unrelated second crate was added beside them. The test asked "are there ≥2 non-trivial components
*after* removal?", which assumes the design was connected to begin with — one island already
satisfies it, so every articulation point anywhere else reports. Measuring against the baseline
fixes it.

That is also the trial's *"15 defects vanished at once"* from the other side: those two
bookkeeping edges attached an island, the count fell under the threshold, and the list cleared.

Final state of reflow2's own design:

| | gaps | defects |
|---|---|---|
| as probed | 25 | 8 |
| after BL-23, BL-24, BL-5 | **1** | **2** |

The one gap is `no_deploy_operate`, which is correct. Both defects are correct: `cmp:core` really
does hold the subsystems together, and the MCP crate really is disconnected in the *design*
network (it depends on the core in code, which is not a design edge — arguably a modelling gap in
the probe rather than a finding about reflow2).


## Re-run after BL-4, same day

The detector numbers are unchanged (1 gap, 2 defects — BL-4 adds a capability, not a detector), so
the probe was pointed at the new capability instead: put its one real gap to the user, end the
session, reopen the graph.

```
gap: No plan to deploy and operate it
open after asking: 1
-- new session --
open questions: 1
  asked 2026-07-18T18:00:00Z about gap:3c0d949e1e45b37d
  "reflow2 has no Release or Environment modelled — is it meant to be deployed,
   or is it a library people build from source?"
```

The question and its exact wording survived the process boundary, which is BL-4 working on a real
design rather than a fixture.

**And it immediately found the next problem.** Answering *"it is a library you build from source;
no deploy layer is intended yet"* does not change the design, so the gap stays open while the
question becomes `answered`. A third session then sees:

| | |
|---|---|
| `detect_gaps` | 1 open |
| `open_questions` | 0 |
| `reviewed_gaps` | 0 |

— a bare open gap with no sign it was ever asked, so it re-asks. BL-4's problem displaced by one
step. The record is in the graph (`scan_nodes(Question)` holds both question and answer) but
nothing on the surface points there. Filed as **BL-25**.

Partly this is incomplete usage: an answer meaning "this is fine as it is" should be followed by
`acknowledge_gap`, which would move the gap to `reviewed_gaps` and close the loop properly. But an
agent will take the path taken here, and the probe's value is precisely that it takes the path an
agent would.
