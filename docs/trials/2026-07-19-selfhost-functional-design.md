# reflow2's own functional design, analysed by reflow2

**What:** a real, durable graph of reflow2's functional design — 96 nodes, coarse-grained — built by
[`tools/build_design_graph.py`](../../tools/build_design_graph.py), committed as a deterministic
export at [`docs/design/reflow2.json`](../design/reflow2.json), and then put through reflow2's whole
analysis surface.

**Why this is different from every prior trial.** All of them, including today's three, build a
throwaway graph in `/tmp` to *test* reflow2 and delete it. None produced a design we keep and work
from. `.reflow2/graph` held an 18-node genesis stub. So every backlog item raised so far came from
reading code and running probes — **not** from interrogating a design graph. For a tool whose claim
is keeping designs coherent, that is worth naming.

The export is the durable artifact rather than the RocksDB directory: exports are sorted and
byte-identical for an unchanged design (verified — two builds diff clean), so the design is
reviewable and diffable in git, and a working copy is one `import_graph` away. `.reflow2/` is
gitignored; the JSON is not.

**Granularity** is deliberately coarse per [BL-23](../backlog.md): one Component per module, one
Artifact per module, one Verification per test file. Never one Artifact per source file, which made
22 of 25 gaps noise the last time this repo was modelled.

```
96 nodes · Project 1 · Requirement 10 · Capability 22 · Component 23
           Interface 3 · Artifact 19 · Verification 16 · Release 1 · Environment 1
33 gaps · 36 defects
```

---

## 1. The graph independently rediscovered the backlog

The strongest positive result. Five capabilities were seeded `planned` because they genuinely do not
exist, and the analysis surfaced exactly them as `unallocated_capability` / `unverified_capability`:

| Capability reported open | Backlog item raised independently |
|---|---|
| Recover a design from an existing system | **BL-27** adopt |
| Say when a claim was last confirmed | **BL-35** freshness |
| Reconcile against what was proven | **BL-30** `reconcile_verification` |
| Reconcile against what is running | **BL-9** `reconcile_deployment` |
| Model a process, not only a product | **BL-37** `Flow` write side |

Those five items were derived over this session from trials and code reading. The design graph,
built from the docs and the module layout, names the same five. Two independent routes to one list is
the first real evidence that the loop produces the answer a human would.

## 2. `unrealized_capability` fires on capabilities that are demonstrably built — **new**

11 of the 33 gaps were "Nothing builds capability X" for `cap:detect`, `cap:heal`, `cap:change`,
`cap:propagate` and friends — all shipped, tested, and running in the binary that reported it.

**Cause, verified in isolation.** The golden thread has two schema-valid shapes at P3 and the
detector accepts only one. `REALIZES` is declared `from: Artifact, to: "*"`:

```
Artifact REALIZES Component  : capability reported unrealized?  True
…plus    REALIZES Capability : capability reported unrealized?  False
```

Modelling *the file realizes the module* — which is how code is actually organised, and what
`link_artifact` invites by taking any `target_type` — leaves every capability looking unbuilt. The
connecting path is right there and is not traversed:

```
art:detect -REALIZES-> cmp:detect <-ALLOCATED_TO- cap:detect
```

`detect_unrealized_capabilities` asks only for `incoming(cap, REALIZES)`. It should also accept an
artifact realizing a component the capability is allocated to — otherwise the detector quietly
mandates one of two equally valid modellings and floods anyone who picks the other. Raised as
**BL-38**.

## 3. `single_point_of_failure` still over-fires, now at real scale

22 of 36 defects, post-[BL-5](../backlog.md). At 96 nodes nearly every requirement and mid-level
capability is called a single point of failure — `req:coherence`, `req:golden-thread`, `cap:detect`,
`cap:change`, and so on. BL-5 fixed the fixture-scale case by asking whether removal *increases* the
count of non-trivial components; on a real design the golden thread is a tree, so most internal nodes
still separate subsystems by that test. The prior fix was measured on an 8-defect graph and this is
the first look at it above fixture scale. Folded into BL-5, reopened.

## 4. `dead_end` on a subsystem whose only edges are `CONTAINS`

`cmp:mcp` and `cmp:kit` are legitimate parents holding modules, and both report *"not connected to
anything"*. This is a documented invariant colliding with a normal shape: PROPAGATE and the topology
view deliberately exclude `CONTAINS`, because *"decomposition is not traceability"* and including it
makes the Project a hub. Correct in the general case, and it means a pure container — the standard
way to express a subsystem — is structurally invisible. Also folded into BL-38.

## 5. Two negative results worth recording

**`possible_duplicate` did not fire.** Today's new detector saw 23 real components, several with
overlapping capability sets, and reported nothing. That is the outcome its two thresholds
(≥2 shared, Jaccard ≥0.8) were chosen for, tested for the first time against a design nobody
constructed to trip it.

**`hierarchy_issues` and `surprising_connections` returned 0**, correctly — the levels are
consistent and there are no unexpected couplings.

## 6. `evaluate_allocation` says nothing without capability dependencies

Every weight came back `0.0`, because the model has no `DEPENDS_ON` between capabilities — my
omission, not a defect. Worth recording anyway: allocation scoring is inert until the functional DAG
is modelled, so an adopt pass or a genesis run that stops at requirements-and-capabilities gets no
value from it and is not told why.

---

## What to do with this

The graph is now a standing record. `python3 tools/build_design_graph.py --analyse-only` re-imports
the committed export and re-runs the analysis, so a change to reflow2's design is a diff on
`docs/design/reflow2.json` and the effect on gaps is one command.

Raised: **BL-38** (the REALIZES modelling ambiguity, plus `dead_end` on pure containers). Reopened:
**BL-5** at scale.
