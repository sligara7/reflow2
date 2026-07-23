# BL-83(b) — adopt dogfood: recovering reflow2's as-built design, then comparing to as-designed

*Trial date 2026-07-23. The `adopt` skill run against a stripped copy of reflow2 itself
(`/home/ajs7/project/reflow2-bl83b/`, code + schema + kit, with reflow2's own design record and
self-study tooling removed so the design had to be *recovered*, not read). This is BL-83 move (b);
move (c) diffs the recovered as-built model against the as-designed functional model from move (a).*

## What this trial is, and how to weigh it

The largest and most self-referential dogfood reflow2 has run: the design tool recovering its own
design from its own code, blind to its own answer. Two things make it strong evidence — the answer
(move (a), the functional decomposition) was known independently, so (c) *measures* the divergence;
and the recovering agent was a fresh session with the design record stripped from its inputs.

**Verification note (this half by the real-repo session, not the adopt session).** Agent output is
data, not fact — so the load-bearing claims here were re-checked against the real repo:

- **Move (c) numbers reproduced exactly**: `--diff docs/design/reflow2.json reflow2-asbuilt.json`
  → `design_added 117, design_removed 186, design_changed 18`; component 42-vs-20, capability
  33-vs-33, requirement 19-vs-22, and the recovered graph carries a `Flow` (`flow:coherence-loop`)
  where as-designed carries `sys:coherence-loop` — the functional concept relocating to a different
  node type, exactly as F.2 reports. Confirmed.
- **A.1's *symptom* was real, its *cause* was confabulated**: the failing `open_rocksdb_without_
  feature_fails_loud` test was real (391 → 390), but the "committed RocksDB test fixture 28/55" does
  not exist — the trigger was a stale `/tmp/reflow2-should-not-be-created.meta.json` stamp from a
  pre-orthogonality binary, plus an ordering bug (the stamp was written before the feature gate).
  This is the trial's most useful caution: **adopt surfaced a real problem and then invented a
  plausible, specific, wrong root cause for it.** Findings need verification gating.
- **Already fixed in the real repo** (surfaced by this trial): the stamp-ordering bug (commit
  `f4109cb`) and the misleading "knows more of the schema" refusal message (`dc9bf96`, BL-86).
- **E.2 confirmed** a real BL-57-family bug: `import_graph` requires `document.stamp` but the
  published tool schema declares `document` as a bare object → captured as BL-87.

What follows is the adopt session's notes, verbatim.

---

# reflow2 — adopt dogfood notes (BL-83b)

Running the `adopt` skill against reflow2's own stripped codebase. This file records
**issues and opportunities to improve** — both in reflow2-the-tool and in the `adopt`
doctrine/skill itself — observed while doing the recovery. Newest context appended at the end.

Session date: 2026-07-22. Served by reflow2-mcp 0.9.0 (28 node types / 53 edge types).

---

## A. Findings about the *system under adoption* (reflow2)

Real as-built issues surfaced by the recovery; candidates for reflow2's own backlog.

1. **Schema type-count drift across doc / binary / fixture (real).**
   Three sources disagree on the vocabulary size:
   - `README.md` claims **27 node / 54 edge types**.
   - Authoritative build (`validate_schema.py` + MCP `served_by` stamp): **28 node / 53 edge types**.
   - A committed RocksDB test fixture was written by a build knowing **28 node / 55 edge types**.
   The mismatch is not cosmetic: it makes `tests/persistence.rs::open_rocksdb_without_feature_fails_loud`
   **fail** — the fixture-open path raises a schema-version-mismatch error *before* it can raise
   the missing-`rocksdb`-feature error the test asserts. So the core suite is **390/391**, not
   green, in this checkout. Opportunity: pin the edge-type count in one place; regenerate the
   fixture on schema change; fix the stale README count.

2. **CI references harnesses that aren't in the tree (here).**
   `.github/workflows/ci.yml`'s `full` job invokes `phase_trial`, `model_the_loop`,
   `coherent_erosion_trial`, `erosion_trial`. None exist under `tools/` in this copy. (Expected —
   BL-83b stripped the trial scripts — but a fresh clone of *this* tree would have a red `full`
   job, and it's worth confirming the real repo's CI and tool inventory haven't drifted.)

3. **MCP + smoke layer unverified this session (cost, not a defect).**
   `tests/tools.rs` (38 tests) and `smoke_mcp.py` drive the real binary and need the
   `rocksdb`+`fulltext` build (~14 min C++ compile). Not run here; recorded honestly as
   `ver:mcp-tools = planned` rather than claimed passing. Opportunity: a cached prebuilt binary
   in the repo would let an adopter verify the surface without paying the RocksDB compile.

## B. Findings about the *adopt skill / doctrine* (the dogfood point)

1. **Doctrine's 78-nodes-for-110k-LOC scale target under-fits a dense tool.**
   reflow2 is ~34k LOC but exposes **93 MCP tools** and **28 node types** — feature density far
   above line density. An honest coarse model still lands near ~100 nodes. "Granularity is the
   scale answer" keys off LOC; it should key off *distinct contracts/capabilities*, which is what
   actually drives node count.

2. **The BL-83 thesis held, with a twist.** The dependency graph (from `use crate::` imports, not
   prose) yields a **module/file-tree** decomposition — as predicted. But it *also* reveals a
   genuine **layered** structure (a foundation cluster `nodes/schema/graph/vocabulary/provenance`
   that everything depends on, over a flat operation layer). That layering is structural, not
   lifted from the "coherence loop / golden thread" prose — so recording it is legitimate, and is
   itself the finding: adopt re-derived modules *and* their layering from imports alone.

3. **`import_graph` one-shot worked well; the node-by-node warning is right.** Learning the export
   envelope by exporting the empty graph first (`export_graph` → `{graph_id,nodes,edges,stamp}`)
   was the fast path. Minor friction: no single tool returns "required properties per node type"
   compactly — `describe_schema` returns a very large payload per type; I fell back to reading
   `schema/*.yaml` directly. Opportunity: a `describe_schema {required_only:true}` mode.

## C. Phase-2 analysis findings (detectors run against the recovered graph)

1. **`no automated test` for two real kit components (real coverage gap in reflow2).**
   `render_views.py` (928 LOC) and `reflow2_check.py` (275 LOC, the CI gate) have **no test
   suite** in the tree — `unverified_capability` fires on `cap:render-views` and `cap:ci-gate`,
   and it's true: nothing proves them. Notable because reflow2's whole ethos is "unexamined is a
   visible state." Its own CI gate is unexamined.

2. **Detectors behaved textbook-correctly on an adopted graph.**
   - `unmotivated_capability` fired on 17 capabilities with no SATISFIES — this is exactly the
     recovery engine the doctrine promises: it turns "code with no stated intent" into questions.
     Some are honest under-linking (defect-detect/heal/diagnostics genuinely serve the README's
     coherence requirement); a hard core (search, merge, alternatives, render-views, ingest,
     genesis, the CLI surfaces) genuinely lack a stated requirement and are the real
     design-recovery questions to put to the user.
   - `single_point_of_failure` fired on `ifc:schema-vocab`, `ifc:graph-persist`, `cmp:core-store`
     — all `library`/`data`-medium foundations where "add redundancy" is meaningless. The
     library-medium caveat in the doctrine is real and necessary; even so the SPOF detector still
     fires on data/library hubs. Opportunity: SPOF detector could suppress or down-rank hubs whose
     only contracts are `medium: library|data`.
   - `disconnected_community` correctly identified the Python consumer kit as a loosely-coupled
     satellite (it touches reflow2 only through MCP/CLI/export contracts, not code) — a true
     structural fact, not a defect to heal.

3. **Model errors the detectors caught in my own recovery (fixed in Phase 3):** I over-claimed
   `verified` on `mcp-surface`/`cli-modes`/`llm-seam` without a passing wired check
   (`status_contradiction`) — corrected to `realized`. Good: the graph caught the adopter
   over-stating, which is the whole point.

## D. Phase-3/4 outcome (intent recovery + validation)

- The author confirmed all 15 recovered requirements as real intent (promoted proposed->accepted,
  provenance kept `inferred`), and confirmed the 7 "unmotivated" capability clusters as real
  features — 7 author-stated requirements added (provenance `authored`) and the golden thread wired.
  Gaps fell **40 -> 18 -> 7 open** after acknowledging the 11 component-granularity gaps.
- **Reconcile family agrees with reality:** artifacts 15/15 unchanged, verification 13 agreements,
  hierarchy 0 issues, allocation modularity 1.0. The one honest divergence (persistence test
  failing) is recorded, not hidden.
- **7 gaps left deliberately OPEN** (the correct adopt end-state, not failure):
  1. `ver:core-persistence` failing (the schema-count-drift finding in A.1) — real bug to fix.
  2-7. `cli-modes`, `one-shot-cli`, `render-views`, `ci-gate`, `mcp-surface`, `llm-seam` have no
     *passing executed* verification. `render_views.py` and `reflow2_check.py` genuinely have no
     test (A/B finding); `mcp-surface` has a suite that needs the 14-min RocksDB build (not run
     here). These are real, examined, and now visible — which is the whole point.

## E. Extra adopt-doctrine observations

1. **Adding the operate layer (Release/Environment) spawned 11 `unreleased_component` gaps** until
   I wired every shipped component into the Release with INCLUDES. Reasonable, but a hint: a
   Release that INCLUDES a subsystem could optionally imply its CONTAINS-children are shipped,
   instead of forcing an explicit edge per leaf. Minor friction, not a bug.
2. **`import_graph` requires a `stamp` but the tool schema doesn't say so** — first import failed
   with "missing field `stamp`" and no hint about the shape. I recovered it from `export_graph`
   on the empty graph. Opportunity: either default the stamp on import, or name it in the schema.
3. **Overall: adopt worked.** It did NOT emit "The X module." vacuity — descriptions came from
   signatures/imports/contracts. It flagged what it couldn't know (17 unmotivated capabilities ->
   real questions) instead of inventing intent, and it caught me, the adopter, over-claiming
   `verified`. The BL-83 thesis held: the recovered decomposition is module/file-tree shaped
   (layered), which is exactly what move (c)'s diff against the functional as-designed model should
   quantify.

## F. Move (c) result — as-designed (a) vs as-built (b), the BL-83 finding made mechanical

`reflow2-mcp --diff docs/design/reflow2.json reflow2-asbuilt.json`:
`design_added 117, design_removed 186, design_changed 18` — the two models barely overlap.

The decomposition axis is where they split, and the numbers are clean:

| Layer | as-designed (a) | as-built (b, recovered) | shared ids |
|-------|----------------:|------------------------:|-----------:|
| **Component** | **42** (35 `cmp:<module>` + **7 `sys:<function>`**) | **20** (5 crate/dir + 9 core clusters + 6 kit) | **1** (`cmp:schema`) |
| **Capability** | 33 | 33 | 9 |
| Interface | 3 | 6 | 2 |
| Requirement | 19 | 22 | 4 |

Findings:
1. **The structural decomposition diverged almost completely (1 of ~42 shared).** As-designed carries
   a *dual* decomposition: one component per Rust module (35) **plus a 7-node functional `sys:*` layer**
   (sys:store, sys:vocabulary, sys:coherence-loop, sys:intake, sys:agent-surface, sys:human-channel,
   sys:time-history). As-built, from the artifact alone, recovered a **single layered module-cluster**
   decomposition (crate -> cluster) and **did not independently re-derive the 7 functional subsystems
   as structural nodes**. BL-83's thesis, confirmed and measured: from the code, adopt re-derives
   *modules*, not the *functions* the designer layered on top.
2. **But the functional concepts didn't vanish — they landed on different node types.** Where (a)
   reifies "the coherence loop" as a `sys:*` **Component**, (b) recovered it as a **Flow**
   (flow:coherence-loop); "vocabulary" became an **Interface** + store cluster; "golden thread" became a
   component cluster. Adopt *did* lift the pervasive functional language out of the code — onto
   Flow/Interface/cluster, not onto structural subsystems. (The runbook flagged this exact possibility.)
3. **The functional surface converged on the count: 33 capabilities in both** (only 9 ids match).
   Strong signal that *what the system does* is more intrinsic to the artifact than *how it is
   decomposed* — capability count is recoverable; component boundaries are a modeling choice two honest
   passes make differently.

**Not yet done:** the runbook's final step is to record this under BL-83 in the *real* repo's
`docs/backlog.md` and close the item. That edits a different repo, so I left it for the user to confirm
rather than doing it unprompted.

---

## Disposition (real-repo session, 2026-07-23)

- **BL-83 closed** — all three moves done; the finding is measured (see move (c) above / §F).
- **Fixed this session**: stamp ordering (`f4109cb`), refusal message (`dc9bf96` / BL-86). So the
  trial's D.1 / A.1 "`ver:core-persistence` failing" is resolved in the real repo (the copy still
  has the old code); real suite is 391/391.
- **Captured**: BL-87 (import_graph stamp, from E.2), BL-88 (reflow2's own CI gate + render_views
  untested, from C.1/D), BL-89 (adopt-doctrine tweaks: scale-by-contracts B.1, describe_schema
  required_only B.3, Release-INCLUDES-children E.1). C.2's SPOF-on-library/data-hubs folded into
  BL-84 (structural detectors over-firing on non-operational nodes).
- **Kept as validation, not action**: C.3 (`status_contradiction` caught the adopter over-claiming
  `verified`), C.2's correct `disconnected_community` on the Python kit, and §F's convergence of the
  capability count (33 = 33) with near-total structural divergence (1 shared component of 42) — the
  clearest single statement of BL-83's thesis reflow2 has produced.
