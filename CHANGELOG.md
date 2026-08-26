# Changelog

Notable changes to Reflow 2.0. Format follows [Keep a Changelog](https://keepachangelog.com);
versions follow [semver](https://semver.org), pre-1.0.

## Versioning — which bucket does a change go in?

Decide at cut time by the **highest** bucket present in `[Unreleased]`:

| Bucket | What goes in | What a consumer does |
|---|---|---|
| **Patch** `0.6.x` | Bug fixes, doc fixes, tests, refactors, perf — **and behavior changes that make a silent failure *loud*** (a fix, not a new contract) | Updates blindly; skills and calls unchanged |
| **Minor** `0.x.0` | A change to the **shape** of the tool surface (new/changed params, changed result shapes) or the **schema / graph model** (new node/edge type, new required prop); new capabilities or skills | May need to notice; a schema change also needs an upgrade doc |
| **Major** `1.0` | Deferred until a stability commitment — the design surface promising compatibility | — |

The load-bearing distinction: a behavior change is a **patch** when it only turns a swallowed
failure into a loud one (e.g. a tool that returned an empty result on a typo now errors — a bug
fix); it is a **minor** when the input or output **structure** changes (e.g. `get_node`'s result
reshaped, or a param that was silently ignored now rejected). A **schema change** is always at
least minor and additionally pulls in the upgrade-doc + foundation-migration checklist in
[AGENTS.md](AGENTS.md).

Two companion records, deliberately kept separate:

- **[docs/requirements-coverage.md](docs/requirements-coverage.md)** — *are we meeting the docs?*
  Every requirement → module → test, with an honest Met/Partial/Deferred status.
- **[docs/backlog.md](docs/backlog.md)** — *what should we do next?* Open work with its evidence
  and rough size.

This file is the third view: *what changed, and when*.

## [Unreleased]

## [0.41.0] — 2026-08-26

### Fixed — doing the right thing stops raising the alarm

**Minor.** No schema change: the stamp is unmoved at 29 node types / 64 edge types / schema_version 1, diffed field by field against v0.40.0. **No upgrade doc is owed.**

**hxm_program followed the `brainstorm` skill exactly and was punished for it.** Ideas recorded as `proposed` Decisions, linked with `CONTRADICTS` / `ANTICIPATES` / `DEPENDS_ON` exactly as step 4 instructs — and `detect_defects` read every one of those edges as a commitment. **Their defect count went 2 → 7 over a day of doing the prescribed thing**, repeated at every hand-off. reflow2's own session hit the same class the same day, neither knowing about the other.

**A `proposed` Decision asserts nothing, and three finding kinds now read that.** One predicate, `is_parked_idea`, and each kind states how it combines it — because the combinations differ and the difference is the content:

| finding | goes quiet when |
|---|---|
| `contradiction` | **both** endpoints are parked ideas |
| `unresolved_setup` | **either** endpoint is one |
| `unthreaded_cluster` | **every** island member is one |

`duplicate` and the three pure-topology rules deliberately do **not** read it, and say so.

    89 → 61   structural defects on reflow2's own design, with 33 declared suppressed

**⭐ And the sweep now says what it did NOT report.** `SweepScope.suppressed_by_parked_idea` counts suppressions by category, so `reported + suppressed` reconciles instead of findings vanishing. Without it the fix would have re-created, one rule over, the vacuous zero that `swept.parked` exists to prevent.

**The root cause was not any one detector.** The principle was already written down in FOUR places and implemented in ONE — `zero_degree_finding`'s *"a parked thought that correctly shapes nothing yet"*, the schema's *"ONLY AN ACCEPTED DECISION DISCONTINUES ANYTHING"*, `loop_status`'s silence on an unapproved proposal, and the `parks` ruling requiring an accepted Decision. Every detector re-derived its qualifier set by hand. **Writing a principle down four times is not implementing it.**

So the deliverable is the guard: `every_category_states_whether_it_reads_proposed` is an exhaustive match over `HealCategory`, and a new category **cannot compile** until its author has written down where it stands. Both answers are legitimate; an unconsidered category is not.

**Also fixed:** a gap replayed into `gaps_to_prompts` exactly as documented was refused with `missing field 'suggested_depth'` (hxm_program, F-01). The cause was not the field — `GapCandidate` stated a replay contract in its own doc comment and enforced it field by field, so it held for the fields somebody remembered. The pin is the **round trip**, so any future field added without a default fails it.

### Added — a surface that can refuse every write, and a session that can say what it went without

**`reflow2-mcp --read-only`** refuses every write and still answers every read. The transport and cross-machine reach were already built and verified; what has never existed is any answer to *who is calling*. Read-only splits that exposure and answers one half outright: **integrity** — no write means nothing to attribute, so a caller-supplied `contributor_id` is never accepted; **confidentiality** — not eliminated, *relocated* to the network, which is what a tailnet is for. It is not authentication and does not make authentication unnecessary; it is what lets a reachable surface ship before authentication exists.

Enforced at `write_lock` — the one point a write cannot avoid — so it covers every write that exists and every write added later. **Off by default**, and the compiler proved the sweep complete: making the guard fallible turned all 98 call sites into errors until each was updated, and caught a second assembly point where a per-client session had to *inherit* the mode rather than reset it.

**`report_manual_work` / `manual_work_report`** record the work a session did BY HAND that reflow2 already serves — the negative space. Every other measure of adoption looks at what a session *did* with reflow2, and `dec:bl-155` states the consequence: 40 of 132 tools never called, and it **cannot tell unused from unreachable**. Hand-rolled work discriminates, because it carries intent. The `diagnosis` is a closed set — `tool_missing` (build one) / `tool_not_found` (surface one) / `tool_refused` / `unknown` — and an unknown value is refused.

**`/next`** fronts the `what_next` tool, which had no slash command and was reachable only inside `/where`. It reads reflow2's own four bands back and forbids the agent substituting its own ranking.

### Changed

The `capture-session` skill asks, at session end, what you did by hand that reflow2 should have done. The environment **compliance** vocabulary is parked — deliberately unused until a user asks, not retired; the locating half (`Environment`, `DEPLOYED_TO`, `OPERATES_IN`) is untouched and live. `rule:an-issue-is-root-caused-then-pinned-by-a-test-then-fixed` is recorded **advisory**: nothing in a diff or a CI run can see whether a test failed *before* the fix, so an enforced version could only check something weaker and would read as enforcement while measuring something else.


## [0.40.0] — 2026-08-26

### Added — allocation follows the code, and a deliberate state stops reading as a defect

**Minor.** No schema change in this entry; the release's stamp moved earlier in the increment (see `docs/upgrading-to-v0.40.0.md`).

`unallocated_component` reported **"33 of 85 parts hold no function"** on reflow2's own design. The headline was actionable and wrong. **31 of the 33 were real source modules** with a linked, checksummed file, and for most the module header cited BY ID the very capability the design had filed against a neighbour — `coverage.rs` names `cap:coverage` (allocated to `cmp:drift`), `regions.rs` names `cap:a-session-with-no-seed-can-find-one` (allocated to `cmp:scope`).

**⭐ The measurement, and it is reusable on any design:** compare the ALLOCATION layer (`ALLOCATED_TO`) against the ARTIFACT layer (`REALIZES`), per capability. Across all 212 allocated capabilities: **59 agree, 23 DISAGREE, 130 carry no file evidence either way.** 19 of the 23 named a part the design was reporting as empty. 🛑 The 130 are **unmeasurable by this method, not clean** — reading their silence as agreement repeats the exact error.

    33 → 14   19 allocations moved to the module that implements them
    14 →  2   12 tool-surface slices ruled surface-not-implementer and parked
     2 →  0   the two byte-store backends ruled named-by-their-contract

Neighbours legitimately lost credit: **export 10→7, report 10→8, service 14→12.** That drop IS the correction.

**⭐⭐ `detect_unallocated_components` now reads a parking ruling, like its two siblings already did.** It never consulted `is_parked`, though `unsatisfied_requirement` reads a ruling and `unreviewed_ideas` excludes parked nodes — so twelve slices an accepted Decision declared correctly empty kept reporting as defects, and **recording the correct judgement degraded the instrument.** That is the same failure dev_storyflow's fleet reported when defects climbed 88 → 97 across ten correct writes. It now skips parked leaves and **REPORTS THE COUNT on the evidence line**, because a design with no empty parts must not read like one whose empty parts were ruled deliberate.

🛑 **Still open, recorded rather than fixed:** nothing checks that a NEW structural detector reads a ruling. The next one written will have the same hole for the same reason.

📌 **And a tool gap this exposed:** no reflow2 tool compares the allocation layer against the artifact layer. `reconcile_artifacts` compares design against DISK; `compare_designs` compares design against DESIGN. Neither compares two layers of ONE design, so the analysis above was done by hand in Python over the export.

`crates/reflow2-core/tests/structure_with_no_function_is_asked_about.rs` (14 cases; the three new ones are that a parked leaf is not a finding, that a **`proposed`** ruling parks nothing, and that the parked count reaches the evidence).


### Added — a design can say what it is FOR, and is asked at genesis if it hasn't

**Minor — schema change.** New optional `Decision.quality_target` (the nine axes of `DimensionAssessment.dimension`), new `set_quality_target` tool, new `quality_target_unstated` gap. Genesis asks the question; `detect-and-ask` carries its repair row.

Implements `dec:the-ility-target-is-a-governing-decision-asked-at-genesis`.

**The attribute a system is built for decides *which grouping is right*, and the four disagree** — performance wants least chatter across boundaries, reliability wants no articulation point and may deliberately **duplicate** a function across parts, maintainability wants what-changes-together-lives-together, security wants boundaries following trust rather than coupling. **Allocating without the answer silently picks performance** (`dec:idea-the-ility-chooses-the-allocation-graph`, 2026-08-08, whose co-change experiment measured **71%** of reflow2's own strongest maintainability signal crossing its functional boundaries).

**⭐ The distinction that earns a property rather than reusing one.** `DimensionAssessment` records a **score** on an axis — what the system *is*. This records what it is **for**. Nothing held the second, and giving `DimensionAssessment` a second meaning is the overload that made `Interface.designation` ambiguous. Measured when this was added: **0 `DimensionAssessment` and 0 `DimensionObservation` nodes exist** — the nine-axis vocabulary was fully defined and had never once been used.

**⭐⭐ Three states, not two, and the middle one is the point.** A user may not know at genesis, so *"I don't know yet"* is a first-class answer: a **proposed** Decision naming the candidate they lean toward. The finding then reads *"still being weighed"* at severity 0.40 rather than *"unstated"* at 0.55, and keeps coming back until it is settled. Collapsing a deferral into silence is exactly the failure `Decision.no_relation_note` exists to prevent on this same node type — *prose cannot be told apart from silence*.

**🛑 And a deferral must never be `acknowledge_gap`-ed.** That finding is aggregate-keyed, so one acceptance silences it permanently and for every capability added afterwards — the trap measured the same day on `unreviewed_ideas` and held open at `dec:idea-an-aggregate-acknowledgement-never-expires`. The finding text and the skill both say so.

**No migration.** The property is optional with **no default** — deliberately, because absence must read as *"nobody was asked"* for the detector to work at all, and because a default would be materialised onto every existing Decision when an export is read back (the failure `Capability.delivery` records: *a design whose export does not restore to itself is a backup that would not restore*). Old graphs simply carry none.

**Asked at genesis, not at allocation** — and this does **not** contradict `dec:idea-allocation-waits-for-the-last-responsible-moment`. That governs when you *allocate*; this governs when you *learn what the system is for*. Asking is cheap and shapes everything downstream; allocating is expensive and still waits.

`crates/reflow2-core/tests/a_design_says_what_it_is_for.rs` (8 cases; the load-bearing one is that weighing-without-settling reads differently from never-asking, at a lower severity and a different title).

### Added — `unallocated_component`: structure with no function in it is now asked about

**Minor.** No schema change. New gap source `unallocated_component`; new `cap:unallocated-component-detector` / `req:structure-with-no-function-is-asked-about`.

**The two existing allocation detectors were gated in opposite directions, and a design could sit in the space between them reporting clean.** `concept_without_design` fires only at ZERO components and goes silent forever once a design grows one. `unallocated_capability` is gated the other way and stays quiet until a component exists. Between them they cover *a capability with no home* and *a design with no structure* — and neither covers **structure with no function**.

Measured on reflow2's own design: **33 of its 95 components are leaf boxes owning no capability at all** (85 leaves, so 33 of 85 that the rule ranges over). Every detector reported clean.

**⭐ The leaf filter is the finding, not a tidy-up.** A parent grouping is allocated THROUGH its children — `sys:agent-surface` holding no capability directly is correct modelling. Counting parents would file every well-formed hierarchy as a defect: 40 components against 33 on the same design. It also composes honestly with the adopt nesting step added the same day (#335) — recovering a hierarchy MOVES components off this list by giving them children, because a box that groups other boxes has a job.

**One aggregate rollup, and the losing side is recorded in the code.** Per-component keying is the more honest key for the answer people actually give — *"this box is a namespace, not a functional part"* is a claim about ONE box — and it loses to BL-73 at 33 findings raised at once, because a per-node flood is acknowledged in bulk without being read. Aggregate keying also means the standing judgement survives somebody adding a component, which is the trap `unvalidated_capability` fell into and was re-acknowledged twenty times for.

The finding distinguishes **never started** (nothing allocated anywhere — the deferred allocation step never picked back up) from **partial**, because those are acted on differently. It is silent when the design has no capabilities: there is then no allocation to have performed, and that phase is `concept_without_design`'s ground.

**⭐⭐ The finding asks what the system is FOR before naming any method.** The quality attribute a system is built for decides *which grouping is right*, and the four disagree: performance wants least cross-boundary chatter, reliability wants no articulation point and may deliberately duplicate a function, maintainability wants co-change, security wants boundaries following trust rather than coupling. **Allocating without asking silently picks performance** (`dec:idea-the-ility-chooses-the-allocation-graph`, 2026-08-08, whose co-change experiment measured 71% of reflow2's own strongest maintainability signal crossing its functional boundaries). The first draft of this finding named `propose_allocation` as *the* method and made exactly that mistake; it now names it as the performance answer specifically, and warns that it clusters capability-to-capability `DEPENDS_ON` — **1 edge across 210 capabilities** on reflow2's own graph, so it returns one cluster per capability.

**⭐⭐ It shipped with its instruction, which is the whole point.** `propose_allocation` (clusters capabilities by coupling — doctrinal functional allocation, mechanised) and `evaluate_allocation` were served, correct, and named in **no skill** — the exact shape of `fact:vocabulary-needs-three-legs-and-a-users-project-gets-none-of-it`. So the `detect-and-ask` repair table gained a row naming both, in all three skill mirrors, in the same change. A detector that noticed the absence without telling anyone what to do about it would have reproduced the failure it was built to fix.

**This settles `dec:idea-does-the-deferred-structuring-step-need-a-skill-of-its-own` at option E** (make the absence visible first; decide about a skill later) over A/B/C. ⚠️ **E's own premise was half wrong and checking it is what produced the measurement:** E read *"nothing notices that a design has capabilities and no components, or components with no allocation"* — the first half was already covered by `concept_without_design`, and only the second half was real. **What is still open: whether the allocation step needs a skill of its own.** The detector makes the absence countable across real designs, and that evidence does not exist yet.

`crates/reflow2-core/tests/structure_with_no_function_is_asked_about.rs` (9 cases; the counterweights — a parent is not a finding, an exemption does not extend to its empty children, a design with nothing to allocate is not asked — carry more weight than the positives).

### Changed — the foundation absorption completes: reflow2 no longer depends on dynograph-foundation

**Minor.** No schema change. New components `cmp:foundation-vocabulary`, `cmp:byte-store`, `cmp:text-index` under `sys:foundation`; five `required` Interfaces retired on the record.

The final increment of `dec:absorb-the-foundation-subset-and-end-the-dependency`. `dynograph-core`, `dynograph-storage` and `dynograph-text` are absorbed at **v0.12.0** into `crates/reflow2-core/src/foundation/` — **7,869 lines** with their tests — and 62 files plus 12 out-of-crate test files were rewritten.

**The dependency is gone, verified three ways:** `cargo tree` reports zero dynograph crates for both `reflow2-core` and `reflow2-mcp`; `Cargo.lock` holds zero; no manifest names one. The only survivor was a stale crate *description* ("…over dynograph-foundation"), now corrected.

Seven external crates enter reflow2's own `[workspace.dependencies]` for the first time — `rocksdb`, `tantivy`, `rmp-serde`, `lru`, `serde_yaml`, `uuid`, `thiserror` — plus `tracing`, which nothing had declared. **`rocksdb` stays at 0.24, absorbed verbatim** (`dec:absorb-rocksdb-024-unchanged-then-switch-separately`): the maintained crate is `rust-rocksdb`, and switching is deliberately a separate PR so a failure in an 86-file migration has one cause.

**⭐⭐ The finding worth more than the migration: absorbing a `#[non_exhaustive]` type converts a runtime fallback into a compile-time guarantee.** Two wildcard match arms became unreachable — `keys.rs` had `_ => None`, `vocabulary.rs` had `other => "<unsupported endpoint>"`. Both existed *only* because the enums lived in another crate; the attribute has no effect inside the defining crate. `vocabulary.rs`'s own comment said *"a variant added **upstream**"* — and there is no upstream now. Both were removed, so a new `Value` or `EdgeEndpoint` variant is a **build error** instead of a string someone has to notice. Nothing in the plan predicted this.

**🛑 The plan was wrong, and the correction is on the record.** It promised *"five increments, each independently shippable, stop after any of"* — true for 1–3, **false for 4 and 5**. `dynograph-storage` re-exported `DynoError`, `Schema` and `Value` from core, and `StoredNode.properties` is a `HashMap<String, Value>` of that type: absorbing core while storage stayed external would give reflow2 two of each, with 48 files naming one and 33 the other. No ordering avoids it — storage *depends on* core. See `chg:increments-4-and-5-are-one-increment`; the prior plan is preserved in a snapshot. ⭐ **A dependency graph tells you what needs what to build; it does not tell you what breaks if you take one and not the other.**

**Three mechanical traps, each invisible to the gate that should have caught it:**
- **Integration tests are separate crates** — the blanket rewrite was right inside the lib and wrong in `tests/*.rs`, where `crate::` is the test binary. 12 files needed `reflow2_core::`.
- **`props!` is `#[macro_export]`**, so it lives at the crate root, not the module it is written in.
- **`--no-default-features` clippy does not compile feature-gated files.** The identical `props!` error sat behind `#[cfg(feature = "fulltext")]` and passed that gate **three times**. Same shape as increment 2's `#[cfg(test)]` defect: a gate trusted for coverage it structurally cannot provide.

**Five required Interfaces retired, not deleted.** `ifc:req-dyno-{core,storage,graph,resolution,vector}-api` were real for the whole life of the dependency, and half the surviving design is shaped by having consumed them across a boundary. Each carries a `deprecation` ChangeEvent, a snapshot of its final state and edges, and an `OBSOLETES` edge from the accepted Decision — so they read `discontinued: true` while their `spec` still says what the boundary was.

**This module is public where the other three absorptions are not.** `stats`, `fuzzy` and `graphalg` are `pub(crate)` because `ifc:core-api` records 277 public functions growing by default. That argument does not reach here: `lib.rs` already re-exported these types and `reflow2-mcp` names `DynoError` 35 times. Making them private would be a breaking change dressed as tidiness.

**No new capability or requirement was owed.** `cap:store` and `cap:search` already existed with requirements behind them (`req:persistence`, `req:deterministic-core`, `req:no-silent-fallback`, `req:agent-native`); the absorbed code now realizes them directly instead of a supplier doing it. Only increment 3 genuinely lacked a requirement.

### Changed — increment 3 of the foundation absorption: the graph algorithms come in-tree

**Patch.** No schema change, no surface change. New component `cmp:graph-algorithms` under `sys:foundation`; new requirement `req:graph-theory-undergirds-the-design-brain`, **accepted** on Anthony's word.

The closure behind the eight names reflow2 imports — `Graph`, `GraphBuilder`, `betweenness_centrality`, `connected_components`, `cut_structure`, `find_cycle`, `leiden`, `strongly_connected_components` — is absorbed from `dynograph-graph` at **v0.12.0** into `reflow2-core/src/graphalg/`, with its tests. `structure.rs`, `flow.rs` and `allocate.rs` import locally, and `dynograph-graph` leaves `Cargo.toml`.

**2,219 of 3,832 lines. 1,613 left behind** in nine files reflow2 never calls: pagerank, eigenvector, closeness, clustering, link prediction, max-flow, shortest path, toposort, degree centrality.

**⭐ `cut_structure` does NOT need `max_flow_min_cut`** — articulation points and bridges come from a DFS lowlink walk, so 248 lines stayed behind that the intuition *"cuts need flow"* would have dragged in. That is the one closure result worth checking rather than assuming.

**🛑 And the first closure was WRONG, in the same hour a warning about exactly this was written.** The grep that found reflow2's import sites matched only single-line `use dynograph_graph::X;` and missed the **multiline** block in `structure.rs`, which imports `betweenness_centrality`. That put two needed files — `betweenness.rs` and `paths.rs`, 375 lines — in the not-taken list. The same blind spot had been caught in the *dependency* scan minutes earlier and fixed in only one of the two greps. **Fixing a pattern's blind spot in one place does not fix it in the other.**

**The `cmp:graph` collision was real in the code, not just the model.** Every absorbed file said `crate::graph` meaning *its* graph module, which in reflow2 resolves to `DesignGraph`; and `super::` breaks inside nested `#[cfg(test)]` modules. Imports were rewritten to the unambiguous `crate::graphalg::`.

**⚖️ Eight unused items keep `#![allow(dead_code)]`, deliberately** — two error variants, three policy variants, two accessors, a helper. The files are kept **verbatim** so they stay diffable against upstream, which is what the provenance requirement rests on. Anthony, 2026-08-24: *"don't trim now — if we decide it isn't necessary, then we can trim in the future."* "Only take what we need" was already applied at the file level.

**A requirement that had gone unwritten for a week.** Absorbing the algorithms deletes the `required`-interface framing that legitimately explained why reflow2 had no requirement for them — a required interface says *"we NEED this"*, not *"we DO this"*. `req:graph-theory-undergirds-the-design-brain` captures Anthony's own 2026-08-17 words (*"I wanted graph theory to undergird everything"*), at `proposed` first and `accepted` on his explicit say-so. ⭐ Nothing would have surfaced it: no search found it, a **consequence** did.

Three of six crates gone. Remaining: `dynograph-core`, `dynograph-storage`, plus `dynograph-text` transitively. **Only increment 5 ends the dependency.**

### Changed — increment 2 of the foundation absorption: the fuzzy matcher comes in-tree, and two crates leave the build

**Patch.** No schema change, no surface change.

`token_sort_ratio` and its closure — `jaro`, `jaro_winkler`, `jaro_winkler_prepared`, `sort_tokens` — are absorbed into a new private `reflow2-core` module from `dynograph-resolution`'s `fuzzy.rs` at **v0.12.0 (`0bb3bca`)**, with eleven of its twelve tests. `seam.rs` and `ingest.rs` — the two call sites in the whole tree — import locally, and `dynograph-resolution` leaves `Cargo.toml`.

**Not taken:** `PreparedName` and its `score` method (built for the resolver's score-one-against-many loop, which reflow2 never runs), the test exercising it, and all of `resolver.rs` — **911 of the crate's 1,211 lines**, an `EntityResolver` reflow2 has never called.

**⭐ Increment 1's caveat is discharged, exactly as written.** That increment said *"one line out of `Cargo.toml` is not one crate out of the build"* — `dynograph-vector` had left the manifest but stayed in the build graph because `dynograph-resolution` declared it. Removing resolution removes the last path, so **two crates leave here, not one**. Verified with `cargo tree`: both now appear zero times.

Three of six crates are gone. Remaining: `dynograph-core`, `dynograph-graph`, `dynograph-storage`, plus `dynograph-text` reached transitively through storage's `fulltext` feature. **Only increment 5 ends the dependency**; 3 and 4 shrink it.

**Two things this increment is worth remembering for:**

- **Clippy caught a defect the test run structurally could not.** The extraction dropped `#[cfg(test)]`, so the test module was compiling into the library — and all 11 tests passed anyway, which is exactly why the test run was blind to it and `-D warnings` was not. The extraction's own assertions checked for *content* (functions present, `PreparedName` absent) and not for *attributes*.
- **Two rustdoc links needed rewriting, not deleting.** `jaro_winkler` and `sort_tokens` both linked to `PreparedName`, which is not coming across. Left alone they are broken intra-doc links; deleted, the reason the helper existed is lost. They became prose, and the module header says so.

### Changed — increment 1 of the foundation absorption: two statistics functions come in-tree

**Patch.** No schema change, no surface change; one crate leaves reflow2's direct dependencies.

First increment of `dec:absorb-the-foundation-subset-and-end-the-dependency` — Anthony, 2026-08-24: *"I kind of don't want to be dependent upon dynograph-foundation anymore"*, and *"by absorbing, I don't mean all dynograph-foundation crates — I mean only the ones that are needed."*

`mean` and `linear_regression_slope` are absorbed into a new private `reflow2-core` module from `dynograph-vector`'s `stats.rs` at **v0.12.0 (`19b6760`)**, with their seven tests, verbatim. `dimensions.rs` — the only consumer in the tree — imports locally, and `dynograph-vector` leaves `Cargo.toml`.

**Taken:** two functions of nine. **Not taken:** `pearson_correlation`, `variance`, `std_dev`, `percentile`, `median`, `softmax`, `spearman_rank_correlation`, and the whole of `hnsw.rs` (1,165 lines) and `distance.rs` (776). reflow2 calls none of them — measured, it used 436 of that crate's 2,377 lines and called two functions.

**⚠️ One line out of `Cargo.toml` is not one crate out of the build**, and the record says so in three places rather than implying otherwise. `dynograph-resolution` declares `dynograph-vector` itself, so cargo keeps building it until increment 2. Verified with `cargo tree -p reflow2-core -i dynograph-vector`, not assumed.

**Provenance is a requirement of this work, not a courtesy.** The recorded objection to absorbing anything is that *vendoring converts a visible dependency into an invisible one*: the pin carried a written reason for every bump, and in-tree code has no successor to that record. So the absorbed module carries a header naming repo, tag, commit, file, what was taken and what was deliberately left — and every increment will.

**Two decisions settled alongside it**, both from questions Anthony asked mid-increment:

- **Absorbed code is distributed BY FUNCTION** into the components that use it; no `sys:absorbed-foundation` is created, because a subsystem for absorbed code would rebuild in the model the boundary the absorption removes. ⭐ This resolves `dec:idea-is-the-foundation-a-subsystem-or-a-supplier` — open since 2026-08-17 — on its own terms: option B was rejected because *"a subsystem you do not build, cannot change, and release separately is not a subsystem"*, and absorption removes all three objections at once. The node stays open until the last increment, because the supplier still exists.
- **Writing the requirement is part of increments 3–5, not cleanup after them.** Those behaviours are modelled today as `required` interfaces — a legitimate way to have no requirement, since reflow2 claims to *need* them rather than to *do* them. Absorption deletes that framing, and `unmotivated_capability` would then fire. Increment 1 is clean: `stats.rs` sits beneath `cap:dimensions`, which already satisfies `req:coherence`.

Also: `dep:dynograph-foundation` gains its `graph_id`, which had been null — dynograph-foundation has had its own reflow2 design all along.

### Added — the graph asks what a session made FALSE, and the edge that had never once been drawn now gets asked for

**Minor.** New capability and one new served tool (`unclaimed_findings`); no schema change, stamp unmoved at 29 node / 64 edge types.

`req:the-graph-can-say-what-has-already-been-done` — Anthony's ask that *"reflow2 needs to be able to accurately show where it is (and that means knowing what it's done)"* — had **nothing satisfying it**, and `dec:idea-how-does-the-graph-learn-what-a-session-invalidated` held six costed options with none chosen. It now has two capabilities: `cap:arrival-delta` for PLANNED work and this one for OBSERVED findings, which is the asymmetry the requirement itself names.

**🛑 The measurement that reframed the question: option C was already built, and had never once been used.** `INVALIDATES` shipped in v0.39.0 *with* its reader, deliberately, so the marker would not become a comment nobody consults. Measured a day later, tool served the whole time:

    INVALIDATES edges in the graph:  0

Not one, by anybody. The open question was never *which of six mechanisms* — five of them were ways to make the sixth get used, and the sixth already existed.

**⭐⭐ Why, precisely — and this is bigger than the feature.** A design's vocabulary reaches real work only with **three legs**: a typed TOOL, an INSTRUCTION that names it, and a COMPUTATION THAT NOTICES ITS ABSENCE. The tool was served. Exactly one skill mentioned it (`where-am-i`) and only the *read* side. Nothing noticed that no edge existed. **A vocabulary can be reachable, documented in its own node, wired to a reader, and still dead — and nothing in the loop reports a zero-use edge.**

So this builds the two missing legs rather than a seventh mechanism:

- **`unclaimed_findings`** — given the ChangeEvent ids a session recorded, the OPEN observations their changed subjects carry that nobody has claimed. It reads the **`subject_id` property as well as the subject edges**, which is most of the coverage: 151 of 270 open observations are reachable by edge (56%), 261 once the property is read (**97%**). The first draft read edges only and would have answered barely half the question while looking exactly like one that answered all of it.
- **The instruction** — `capture-session` test 7 and `impact-check` step 4 name the tool and the moment. Test 7's "same-session only" limit is corrected: the computation reaches observations a session never read, which was the missing and more expensive half.
- **The delivery** — the Stop hook's async probe asks it and **rides along** on a nudge already firing, or is carried to the next SessionStart.

**⭐ Ask, don't block** (Anthony's word). The ask never arms an interruption of its own; it only appends to a message already going out. A brand-new trigger keyed on a computation nobody has field-tested is exactly what becomes wallpaper. 🛑 The honest cost, stated rather than glossed: a non-blocking Stop message reaches the **transcript**, not the agent, so the two paths that genuinely reach one are the ride-along and the next SessionStart.

**Every row is a candidate, never a verdict.** Nothing infers that an observation is false — only that the thing it describes has moved and nobody has said either way. TemporalFacts only, never Verifications, and deliberately **not a gap source**: that is what keeps it from reversing `dec:verification-freshness-not-a-gap`, which rules a stale-looking *check* a standing property that would fire on every legitimate refactor.

**⚠️ A number in this work was wrong in the flattering direction before it was right.** The "78% of events return nothing, median 1" figure that justified the session-scoped design was measured against the edge-only reader. Widening reach to 97% necessarily lengthened the shortlists. Honest figures, re-measured against what shipped: **71% silent, median 1, mean 4.3, p90 13, max 40** — the tail driven by hub subjects (`proj:reflow2` alone carries 25 open observations). *A measurement taken against a narrower implementation than the one that shipped is not evidence about the one that shipped.* It was caught by running the tool on real data, not by any unit test.

**And it found something on its first real use.** Run against this design's own committed export, it surfaced `fact:defect-a-verification-has-nowhere-to-put-its-evidence-so-the-name-became-the-report` — stale for eight days. Both its claims were verifiably answered (`description` and `findings` both exist and were used the same day; its median-name measurement re-measured to zero names above 60 words), so it was closed with **the first `INVALIDATES` edge ever drawn on this graph**. Its sibling `fact:defect-add-verification-cannot-reach-its-own-fields` was deliberately **left open** — `description` became reachable but `location` did not, and a partially superseded finding is not closed.

`ver:the-graph-asks-what-a-session-made-false` passing — 10 core cases plus `tools/test_loop_nudge.py` at 91 (was 80). Open gaps on this design: **1 → 0**.

### Added — the Stop hook stops guessing from a tool tally and asks the graph

**Minor.** New capability, no schema change; stamp unmoved at 29 node / 64 edge types. New kit
file `tools/graph_probe.py` — the release workflow stages it, and a test asserts that it does.

`dec:idea-a-capture-session-skill-the-user-types` chose a skill the user types as a **first step**
and named option C — a hook that decides *when* — as the destination. C never got built, and the
reason written down was that a hook **cannot read the graph**: "the session's own server holds the
single-writer lock." That was true before the shared server and has been false for a while. The
shared server answers stateless MCP over the URL in `.reflow2/graph.server.json`, and a hook can
read the graph perfectly well.

**The real obstacle was a number, and it is worth more than the fix.** Measured live on reflow2's
own design:

| call | wall clock |
|---|---|
| `loop_status` | **22.6s** (repeatably) |
| `loop_status` with `since_export` | 37.5s |
| `claim_report` | 0.04s |

A Stop hook that blocks for 23 seconds is worse than no hook, and nothing under a second answers
the question. ⇒ **A capability limit and a cost limit read the same in a comment and take opposite
fixes.** The old note pointed every reader at the wrong one for months.

**The async shape** (`dec:the-stop-hook-asks-the-graph-asynchronously`, Anthony's word): the hook
**spawns** `graph_probe.py` detached and returns immediately, never waiting. A session stops once
per *turn*, so the answer lands while the agent is still alive to act on it; a session that ends
first has its verdict reported at the next SessionStart, attributed to the session that caused it.

**⭐ The trigger is a DELTA, not a level, and that is the whole restraint.** reflow2's own design
carries 7 unsurfaced gaps and 60 structural defects, both standing for weeks. Keyed on the level
this would speak in every session forever and be a nag — the fire-on-correct-work failure
`ver:skill-triggers` exists to prevent. Keyed on a delta against a baseline taken at SessionStart,
it says *"this session took open gaps from 7 to 10 and never put the three new ones to the user"*,
which is a fact about the session. **No baseline means no nudge**: a delta needs two readings, and
one is not two. `structural_defects` is deliberately not a trigger class — HEAL's count moves on
edits nobody made to it.

It speaks **last**, only where every counting branch stayed silent — which is precisely the session
that did the loop's motions correctly and so tripped nothing — and it shares their once-per-session
budget, so no session is interrupted twice.

Silent by design where it cannot know: no shared server, an unreachable one, or a probe that never
returns all leave it saying nothing. `ver:stop-nudge-asks-the-graph` is passing (80 tests, up from
55), and the branch was proven end to end against the live server on this session's own numbers —
it reported the three gaps this session had just created.

**⭐ And then the same run found the feature's real limit, which no fixture would have.** Those three
gaps did not survive: wiring the thread closed them, and a `CONSUMES` edge closed a fourth that had
been standing, so the session **ended at 6 — below where it started**. A verdict collected from a
mid-session reading can therefore name debt already settled. Not fixable by waiting (that is the
23-second block this design exists to avoid), so the message states the reading's **age** and
directs the agent to confirm with `loop_status`.

### Fixed — the repair-tracking capability finally gets the thread it shipped without

**Patch.** Design record only; not one line of behaviour changed, and `chg:the-repair-capability-gets-its-thread`
carries `subject: record` to say so.

`cap:a-repair-can-say-what-it-invalidated` shipped in v0.39.0 as `realized` and was never wired into
the golden thread — no verification, no owning component, nothing recorded as building it. That
produced **three of the six open gaps** and the standing *"1 capability claiming built with no
check"*, while a dedicated 10-case suite sat green on disk the whole time.

Now allocated to `cmp:verify` (where `invalidated_findings` lives), with `REALIZES` from the five
files that implement it, and `crates/reflow2-core/tests/a_repair_can_say_what_it_invalidated.rs`
registered and pointed at `ver:a-repair-can-say-what-it-invalidated` — **run before being marked**,
10 passed / 0 failed. The capability moves `realized` → `verified`.

⭐ **The lesson is not "somebody forgot."** #321 did the harder half well: it shipped the reader
alongside the edge precisely so the marker would not become a comment nobody consults. What it left
half-drawn was the design's own account of the work, so the graph was **more pessimistic than
reality** — which erodes trust in the report exactly as fast as false optimism does. The Stop hook
above is the guard: a session that leaves a capability unwired now raises those counts against its
own baseline and is told before it finishes.

Two gaps acknowledged with reasons on the record, both Anthony's word:

- **The foundation subsystem** — verified done before acknowledging (all five kernel components sit
  under one subsystem, now `sys:foundation`). The gap fired because nothing *builds* it, and nothing
  ever will: its delivery is `model` and the deliverable was the decomposition. ⭐ The general form
  is worth more than the case — **a capability whose delivery is `model` cannot satisfy an
  artifact-shaped check**, and if this recurs the detector should read that field rather than
  needing a hand-written acknowledgement each time.
- **Nine skills verified only at component granularity** — accepted once, as an over-engineering
  call. Nine checks that all prove the same property are one check with extra bookkeeping. What
  would reopen it is recorded: a skill that computes something, or whose output a person acts on
  directly, earns its own proof.

**One gap is left standing deliberately**: `req:the-graph-can-say-what-has-already-been-done` still
has nothing satisfying it. The mechanism question is open
(`dec:idea-how-does-the-graph-learn-what-a-session-invalidated`, six options, none chosen), and
pointing a nearby capability at it to reach zero would make the design claim to deliver something it
does not. Open gaps went **6 → 1**.

### Fixed — the orientation call stops re-reading a hundred megabytes of stale records

**Patch.** No schema change; `sync_status` gains one reply field. Stamp unmoved at 29 node / 64
edge types.

`cap:loop-status` calls `loop_status` **ONE CHEAP CALL**, and the served instructions tell every
session to run it. Measured over the shared server on reflow2's own graph (3,083 nodes / 13,188
edges): **40.5 seconds**. Two independent causes.

**① A reader that asked a graph-wide question to produce a row-level answer.** `invalidated_findings`
(new last release) asked `incoming()` of every Verification and every TemporalFact, and each of
those walks the whole edge set — **483 nodes × a full-graph scan, 39s, to return 1.2 KB saying
nothing was claimed.** The rollups only annotate rows they are already showing (the checks that are
NOT passing — **one**, here), so they now ask about exactly those ids. The exhaustive form stays for
the standalone tool, where a caller asked for it deliberately. ⭐ **The fix is not a faster scan, it
is asking a smaller question.**

**② The list of synced records was unbounded.** This seat had **16 targets totalling 102 MB** — the
committed export, a backup, and **fourteen one-off probe dumps written by past sessions**, three of
them belonging to a different project — and re-read and re-parsed all of it every time. A roll now
opens the **6 most recently modified** and reports the rest in `not_checked`, naming them and saying
how to bring one back to the front. Ordered by the target file's own mtime, because the record
somebody is actually collaborating on is the one that moved recently.

⭐⭐ **THE RULE THIS SHIPPED WITH IS THE SECOND ONE TRIED, AND THE FIRST ONE'S FAILURE IS THE MORE
USEFUL RECORD.** The first attempt refused to track anything under the OS temp directory, reasoning
that scratch is not a SHARED record — which is what
`req:a-seat-learns-the-record-moved-before-it-writes` exists for. **Fifteen tests in
`the_record_moved_and_the_session_is_told` failed, and every one of them was right.** A hermetic
test puts a genuine shared record in a temp dir; so does a CI workspace and so does a container. One
of those tests is named *"the case the whole thing exists for — your brother pushed, you pulled."*
The rule looked correct against the paths on one machine and would have silenced the feature for
anyone whose workspace sits under `/tmp`. **The defect was never WHERE the records live — it is that
the list only ever grows**, so the bound is on COUNT and needs no assumption about anybody's
filesystem.

⭐⭐ **AND CAUSE ① SHIPPED PAST TEN PASSING TESTS**, because every one ran on an in-memory graph small
enough that 483 scans were instant. Correctness was verified; cost was never measured once.
AGENTS.md already says it — *"compiling is not the finish line, and neither is a green unit test"* —
and what caught it was not a gate but a user asking for something else. **A rollup that walks the
graph needs a measurement at real scale before it merges, not a passing assertion on twelve nodes.**

### Added — a repair can say what it invalidated, so a finding stops proposing work already done

**Minor, and it MOVES THE SCHEDULE STAMP AGAIN: 63 → 64 edge types.** `docs/upgrading-to-v0.40.0.md`
is owed and updated. Node types unchanged at 29; `schema_version` still 1. Additive — a v0.39.0
export imports unchanged.

**The failure, and it cost a real user a real instruction.** A session ran `where-am-i`, read
`ver:the_shardblade_walk` with status `failing` and `last_run_at` of that same day, and reported its
two defects as the live state of the system. **Both had been fixed hours earlier**, recorded
properly on two Constraint nodes with commit shas. The user replied "fix the forge scaling and chase
the 401" — acting on the report — and the first thing the session then did was discover the work was
already done. The graph was right and every node was right; **the composition was wrong**, because
nothing joined the repair to the check that found it. `describe_schema(from: Constraint, to:
Verification)` returned **zero exact matches**, and the nearest honest assertion available was
`CONTRADICTS` with `alignment: opposing` — which a defect detector reads as a design inconsistency
rather than as a re-run owed.

- **`INVALIDATES` (`* → *`)** — *this record makes that finding stale*. Endpoints open on purpose:
  the same relation closes a `failing` Verification and a superseded `TemporalFact`, which is how
  this project measured the identical problem independently six days earlier
  (`dec:idea-how-does-the-graph-learn-what-a-session-invalidated`, whose option C asked for exactly
  this edge from the other side). Two projects, three sightings, two node types, one relation.
- **`invalidates` and `invalidated_findings`** — the write and the read. A marker nothing consults
  is a comment, a failure already found in `enforced`, in `SUPERSEDES` and in `OBSOLETES`, so the
  reader ships with the edge rather than after it.
- **`loop_status.verifications`** annotates each attention row with `invalidated_by`, `rerun_owed`
  and a sentence, plus a top-level `rerun_owed` summary. `graph_report` gets it from the same code.
- **`where-am-i` now gathers `Constraint` and calls `invalidated_findings`** before quoting any
  failing verdict. On the reporting graph the repairs lived on Constraints, and the skill's gather
  list did not include them — so the orientation pass structurally could not see that the work was
  done.

⭐⭐ **WHY `INVALIDATES` AND NOT `RESOLVES`, WHICH WAS THE FIRST NAME TRIED.** A repair does not make
a check pass. It makes the last RESULT untrustworthy, and only a re-run can say what is true now.
`RESOLVES` would have reflow2 asserting an outcome nobody measured — `dec:non-goal-reflow2-does-not-judge-whether-a-check-is-meaningful`
one step along. So the edge claims exactly one thing, it never touches the target's `status`, and
**the check is NOT dropped from `attention`**: silencing it would swap one wrong reading for another
and would be the silent truncation `parks` was careful not to become.

⭐ **AND IT IS A CLAIM, NEVER AN INFERENCE — which is what a measurement decided.** The cheaper
option was a computed staleness bucket, and it is unbuildable honestly: inferring staleness means
ordering a change against a check's `last_run_at`, and of **439 hand-written ChangeEvents only 37
(8%) carried a date**, against **84% of the 192 written by the reconcile paths**. The difference was
not care — `add_change_event` had no `detected_at` parameter, so the ordinary path could not date
anything. **`add_change_event` now takes `detected_at`** (instance 7 of the unreachable-declared-vocabulary
pattern, and invisible to the stamp, so it ships alongside the edge that moves it deliberately).

`rerun_owed` is **three-valued**: `true` (repair postdates the run), `false` (run already reflects
it), **`null` (one side undated — nobody can say)**. Null is never collapsed to false, which would
read as "already covered".

### Fixed — two small things found while building the above

- `nodes.rs` had the doc comments on `SUPERSEDES` and `IMPLEMENTS` **swapped** — the "file IS the
  executable form of a check" paragraph sat above `SUPERSEDES`.
- `skill_lint`'s `NON_TOOL_TERMS` gains `failing`, `last_run_at` and `rerun_owed`, the field and
  status names `where-am-i` must now say out loud.


### Fixed — four places the tool surface knew something and would not say it

**Minor** (two tools gain a param; two replies gain a field). **No schema change** — `note` was
already declared on both edge types, which is the whole point of the first item. The stamp does not
move: still 29 node types / 63 edge types / `schema_version` 1, and no upgrade doc is owed by these
four.

Four reports from dev_storyflow's 2026-08-23 sessions, each verified against the code before it was
taken. What they have in common is not a subsystem — it is that **reflow2 held the information and
the surface withheld it**, so a session had to reconstruct by hand what the process already knew.

- **`constrains` and `governed_by` accept `note`.** Both edge types declared it in
  `schema/*.yaml`; neither typed constructor could reach it, and both structs carry
  `deny_unknown_fields`, so the error listed a shorter allowlist with no hint that a longer one
  existed. `describe_schema` advertised a field the typed write path refused. dev_storyflow wanted
  the note twice in one session and both times fell back to raw `create_edge`, which works and
  abandons the constructor's validation for the whole call — and an agent that does not think to
  run `describe_schema` simply drops the reasoning instead. **This is the trap `governed_by`'s own
  doc comment already documents at length** for `ruling`, hit again one field along: *a declared
  field nobody can reach is a declared field nobody writes to.*

- **A `deferred` requirement gets its own question instead of one it has already answered.**
  `unsatisfied_requirement` asked *"is it covered, deferred, or dropped?"* of requirements whose
  `status` said `deferred`. Measured on the whole class rather than sampled: of 28 open gaps of
  this kind, 14 were `accepted`, **8 were `deferred`**, 6 `proposed` — a 29% noise rate on one
  class, against `acknowledge_gap`'s own argument that a list which can never reach zero gets
  skimmed. **It is not silenced.** Adding `deferred` to the `dropped`/`met` skip was the obvious
  fix and is the wrong one: those two are FINISHED and this one is POSTPONED, so dropping the row
  would make live intent go quiet (`req:no-idea-goes-quiet`), and the served instructions name
  `dropped` and `met` as the stoppers in as many words. The gap now asks whether the parking still
  holds, at reduced severity, and `evidence` names the status. The gap id is a hash of source +
  affected ids, so **existing acknowledgements survive**.

- **`rephrase_degraded` says WHY, and unmatched answers come back.** The flag was honest and
  actionless: *the backend is down*, *you sent no answer* and *your answer did not match the prompt
  it was for* looked identical. The error was available at the point of failure and discarded one
  line later by an `Err(_)`; it is now bound and returned as `degrade_reason`. Separately,
  `AgentBackend::unused_answers` — documented as the way to surface stale answers "rather than
  dropping them silently" — **had no caller anywhere in `reflow2-mcp`**; `gap_to_prompt` and
  `gaps_to_prompts` now return `unused_answers`. The measured case: 4 of 5 prompts degraded because
  the gap objects had been TRIMMED FOR READABILITY between the two passes. An answer is keyed by a
  hash of the prompt text, the prompt text is built from the gap's own title and description, so a
  cosmetic edit re-keys every answer — and every one of those four would have come back as unused
  on the first read. Both tool descriptions now say to replay the gap unchanged, and the backend
  error names the cause.

- **`graph_report_markdown` cuts long gap prose, and says it did.** Titles at 20 words, descriptions
  at 40, with a line saying how many were cut and that `detect_gaps` carries the full text. "Top
  gaps (look here first)" is the FIRST thing the `where-am-i` skill reads, and three of five top
  gaps were single bullets carrying a ~500-word report each — twice over, once as the title and
  again as the description. `loop_status` already truncates Verification names at 25 words and
  announces it; this is that treatment one report along, and the same session that praised it there
  named this as the place that should borrow it.

⭐ **AND ONE REPORT IN THE SAME BATCH WAS ALREADY FIXED, WHICH IS THE MORE USEFUL FINDING.**
dev_storyflow reported `detect_gaps` returning 328,654 characters with "no `limit`, no `offset`, no
`brief`". It has had `budget_chars` (default 30,000) and a three-tier degrade since #303, merged at
01:29 the same day — about eighteen hours before the session that filed it. The report is stale, and
the reason it is stale is a defect the same entry names in its own environment note: *"`.reflow2/kit-version.json`
absent — could not read a version."* **An agent that cannot name the version it is running spends
its credibility on bugs that are already closed**, and no amount of budgeting fixes that.

### Added — relationships between records stop being wildcard leftovers

**Minor, and it MOVES THE SCHEMA STAMP: 61 → 63 edge types.** `docs/upgrading-to-v0.40.0.md` is
owed and written. Node types unchanged at 29; `schema_version` still 1. Every change is additive —
a v0.39.0 export imports unchanged.

dev_storyflow filed three independent reports across four days, none of them connecting the three,
and together they name a class: **relationships between RECORDS are thinner in the vocabulary than
relationships to Requirements.** The golden thread — Requirement ← Capability ← Component ←
Artifact — is richly modelled. Verification-to-Artifact, rule-to-rule and check-to-check were not,
and each time a session reached for one it found a `*` wildcard or a refusal.

- **`IMPLEMENTS` (Artifact → Verification)** — *this file is the executable form of that check*.
  ⇒ `loop_status.verifications` gains **`no_executable_form`**: `never_run` was one number covering
  a check somebody wrote a script for and has not run, and a check with **nothing to run at all**.
- **`COMPLEMENTS` (DesignRule → DesignRule)** — *these stand beside each other and must never be
  merged*. ⇒ **HEAL now refuses the merge**, in both `propose_heal` and `apply_heal`. `DUPLICATES`
  is declared `* -> *`, so two rules could be joined by it and one deleted irreversibly; the only
  prior protection was a paragraph asking readers not to.
- **`SUPERSEDES` accepts Verification → Verification**, and **`Verification.status` gains
  `superseded`** — a retired check stops counting as live coverage and stops appearing in
  `attention`, which lists checks that are *not passing*. A superseded check is not a quiet failure.

**Two candidates raised the same day were DECLINED**, and the reason is the entry fee:
`ChangeEvent → Decision` and a which-repo-does-this-govern property both answer real friction, and
neither has a computation that would read it. `dec:edge-orthogonality` — a vocabulary distinction
earns its keep only if something reads it — and adding unread vocabulary while
`req:a-vocabulary-distinction-proves-it-is-read` is open would be filing the defect and committing
it in one motion.

⭐ **THE FINDING WORTH MORE THAN THE EDGES: adding an edge type is the SAFE kind of schema change,
and the changes that DON'T move the stamp are the dangerous ones.** The stamp counts type NAMES
only, so a new edge type makes an older binary refuse the graph and name what it lacks — while a
widened endpoint, a new enum value or a new property all pass the stamp check, open fine, and then
fault one import at a time. Three of this release's four changes are in that invisible category and
ship alongside the two visible ones deliberately: the stamp move is what protects them.

## [0.39.0] — 2026-08-23

### Added — a boundary that could not name itself

**Minor** (a schema enum gains a value; nothing existing changes shape).

`ifc:mcp-tools-http` carried `medium: unspecified` because the vocabulary named no value for MCP
over streamable HTTP. That was the honest answer — `REST` means verbs on a resource tree, and this
is a **single endpoint carrying JSON-RPC 2.0** where every operation is a named tool — but a blank
is not neutral. `medium` is a pairing-key axis and unset reads as UNKNOWN, never as agreement, so
that boundary could pair with **nothing**.

- **`json_rpc`** added to `Interface.medium`, at all four sites the vocabulary is defined: the
  authoritative enum in `schema/structure.yaml`, `MEDIUM_VALUES` **and** the interface-extraction
  prompt in `ingest.rs`, and the `set_interface_spec` doc that becomes the served tool's schema
  description. Both halves of the ingest path had to learn the word, or the value would exist only
  for hand-written calls.
- **The stamp does not move.** It carries node and edge TYPE names, and an enum VALUE is neither —
  which is exactly why no upgrade note is owed for it, and exactly the blindness that had been used
  to argue against adding it.

**The objection that had blocked it was recorded twice, with opposite conclusions.**
`schema/structure.yaml` cites the stamp's blindness as evidence that adding a value *"locks nobody
out"* — a reason it is **safe**; the interface node cited the same fact as a reason to **decline**.
Settled by separating two questions: the missing word is fixed, and the stamp not counting
vocabulary is a **separate** defect with its own history, not repaired by leaving a boundary
unnameable.

Checked rather than assumed, per `dec:edge-orthogonality` — a vocabulary distinction earns its keep
only if a computation reads it. Seam pairing compares `medium` as a plain string, so `json_rpc`
matches its like and refuses `REST` with no new branch; `is_foundation_medium` special-cases only
`library` and `data`, so `json_rpc` correctly stays a runtime boundary that can fail on its own.

### Changed — the gate names the unmodelled files you just touched

**Patch** (a note's wording and scope; no interface changes, and severity is deliberately unchanged).

The unmodelled-source note was correct and was being skimmed. On this repo it reports **107 files**,
which reads as an institutional backlog rather than as anything the reader did.

Measured from the other side the same day: a dev_storyflow agent built a four-lane feature,
registered **zero artifacts**, and named that aggregate framing as one of seven reasons it stopped
reaching for reflow2 — *"391 is not a number anybody can act on ... three gaps on the node I just
edited would have been a task."*

- The finding now **leads with the subset this working copy touched** — usually two or three files.
  The whole-tree count still prints beneath it, so nothing is hidden; it stops being read first.
- Two sources unioned, because either alone lies: `git status --porcelain` sees a file written but
  not committed, the merge-base diff sees one committed earlier on the branch.
- **Outside a git tree it says the measurement was NOT COMPUTED**, rather than reporting zero. A
  detector with nothing to run on must not read like one that ran clean.

**Severity is unchanged and that is the point.** `dec:idea-allocation-waits-for-the-last-responsible-moment`
defers allocation, and failing the build here would reverse that ruling while appearing to implement
it. Narrowing what is SHOWN and DEMANDING action are different acts; this does the first, and the
wording says registering is an offer.


### Fixed — an acknowledgement says whose judgement it was

**Minor** (two tools gain optional parameters; nothing existing breaks).

`acknowledge_gap` mints an `accepted` Decision — settled intent by
`rule:design-intent-moves-only-on-the-owners-word` — and drew no `AUTHORED_BY` edge and had no
parameter to supply one. **The one write whose entire purpose is to record that somebody decided
something could not say who.**

Measured 2026-08-23: acknowledging 50 gaps in one detect-and-ask pass produced **49 nodes that
failed this project's own `check_intent_authority` gate in a single stroke.** It stayed invisible
because acknowledgements were rare enough that the gate's dated grandfather set absorbed the
historical ones — a defect that only shows at scale is one the tool's own dogfooding was too small
to find.

- `acknowledge_gap` and `acknowledge_gaps` take an optional **`approver`** (and `acted_at`) and draw
  `AUTHORED_BY role=approver` on the Decision. The bulk form carries it **per item**, for the same
  reason `reason` is per item.
- **An absent approver is allowed and REPORTED.** Refusing would leave a design that has modelled no
  `Contributor` unable to accept a gap at all — most solo designs on day one. So the reply says the
  acknowledgement carries nobody's name and will be reported by the gate, rather than accepting it
  in silence. That silence is what let 49 unattributed nodes be written in one pass.
- **An approver naming no Contributor is refused, and nothing is written.** A typo would otherwise
  attach the owner's authority to somebody who does not exist.
- Additive: the old `acknowledge_gap` signature is unchanged, so existing callers still work — they
  are simply now told when they recorded nobody.

**The test for the refusal failed on the first attempt**, and that is the finding worth keeping: the
Decision is minted before the approver can be checked, so the obvious implementation left an
accepted Decision with no name behind on every refusal — the exact state the parameter exists to
prevent, produced by the code meant to prevent it. The check now runs before anything is written.

### Added — a contract answers at the altitude you asked

**Minor.** One new served tool (`seam_coverage`); nothing existing changes shape.

`undeclared_seam` asks *"do these two exact modules share a contract?"*, and could only ever ask it
at module level. So a design that records its **dependencies** between modules and declares its
**contracts** at the subsystem boundary reads as having none at all —
`fact:coupling-and-contract-are-recorded-in-vocabularies-that-never-meet` measured the two sets as
**disjoint by construction**, which meant the number could not move however many contracts anyone
wrote.

- **`seam_coverage {altitude}`** lifts **both** the couplings and the contracts to the nearest
  container at a chosen `Component.level`, then compares them. The question becomes *"is this
  coupling covered by a contract declared at or below it?"*
- **`covered_by` names the leaf pair** where each contract actually sits, so *"yes, A and B
  interface"* always arrives with where to look.
- Measured on this design: **72 couplings / 42 contracts / 64 uncovered** at module level;
  **11 / 13 / 0** at `subsystem`.

**Lifting both sets is the whole correctness argument, and the half-fix looks right.** Rolling up
only the couplings leaves the vocabularies exactly as disjoint as before, because the contracts stay
between the leaves. The test for it was proven non-inert by reverting to that half-fix.

**A zero here is the most dangerous answer the tool can give, and it is designed around.** Every
reply carries the raw count, and an altitude that reaches nothing reports how many endpoints were
dropped — asked at `system`, which this design populates with nothing, it says *0 couplings
compared, 114 endpoints unreachable* rather than a clean bill.

**Nothing is written back.** The roll-up derives on every call; a stored edge between two subsystems
would make the graph assert a contract nobody declared. Pinned by a case comparing export content
hashes before and after.

Deliberately unchanged: `detect_undeclared_seams` and the maturity `seams` band still answer at
module level, so the 64 stays visible. Changing what a detector reports is a louder act than adding
a way to ask, and which altitude that gap is about is the owner's call.

### Changed — the same paragraph, sent once instead of fifty times

**Patch.** Nothing is withheld and no parameter is added; the same words are simply not repeated.

`repair_is_a_judgement` is `Option<&'static str>` — a fixed literal per detector branch, saying why
a category of structural defect has no mechanical repair. It was written per ROW, so a design with
two dozen orphan nodes received the same 797-character paragraph two dozen times. Measured:
`detect_defects` was 46,399 characters, 45,186 of them findings, and **52.3% of those were that one
field — 50 rows carrying 3 distinct values.**

**46,314 → 26,169 bytes on the wire, with all 50 findings intact.**

- **A different shape from the two fixes before it, and the difference is the point.**
  `detect_gaps` withholds prose when a reply will not fit; `graph_report` withholds a list unless
  asked. Nothing is withheld here — no list shortened, no prose truncated, no judgement about what
  a reader needs — so it needs no flag, no budget, and no note about what was lost. Worth asking
  whether a reply is *big* or merely *repetitive* before reaching for a budget.
- **The reader's rule is total:** `row.repair_is_a_judgement ?? repair_is_a_judgement[row.category]`.
  A row keeps its own text whenever that differs from its category's, so a future detector giving
  one category two explanations cannot silently hand a row the wrong one.
- **It does not fire where it would not pay.** A category with one row keeps its text inline; a map
  entry plus the sentence explaining the map costs more than the paragraph it replaces.
- Fires on the scoped reply as well as the unscoped one. `Scoped<T>` names its list `items` rather
  than `defects`, and the first version of this was a **silent no-op on every scoped call** — found
  by driving the built binary. The test then written for it **passed against the bug**, because its
  fixture produced no note-bearing findings inside the region and the assertion loop never ran. The
  shape handling is now pinned directly on both shapes.

### Changed — a report is not a list of every check

**Minor** (`graph_report` gains an optional parameter and its `verifications` field changes shape
by default).

`graph_report` answers *"what should I look at?"* and is what the **where-am-i** skill reads.
Measured after the duplicate emission was already gone: **166,934 characters, of which 152,803 —
91.5% — were the full verification roll.** 197 checks, 196 passing. The one read that exists to
point a session somewhere spent nine tenths of itself saying "196 passing, 1 planned".

93% of the roll was the `name` field. **113 of the 197 names run over 25 words; the longest is
654** — reports written into a name because `description` was declared and unreachable from
`add_verification` for a long time. The clue was already on the wire:
`graph_report_markdown` renders the same report for a human in 6,172 bytes, 27× smaller.

- **The roll is withheld by default**, replaced by the same digest `loop_status` already returns —
  counts by status, how many never ran, and every check NOT currently passing, in full. Reusing it
  is why the 25-word name truncation and its announcement are not written twice.
  **166,934 → 15,480 bytes on the wire.**
- **`include_verifications: true` returns the whole roll** (165,207 bytes), and
  `loop_status`'s `full_list` pointer now names that flag. A withheld list whose retrieval
  instruction has gone stale is worse than one that was never withheld, so both pointers are
  asserted by test.
- **The limit, stated rather than hidden:** the digest keeps every not-passing check in full, so it
  is flat in the *passing remainder*, not in the check count. Here 196 of 197 pass. On a design
  mid-build with two hundred `planned` checks this report would be large again — real, unmeasured,
  and deliberately not built for.
- Budgeting the report stops 113 essay-length names flooding a reply; **it does not move the
  reports out of the name fields.** New checks land correctly now that `add_verification` reaches
  `description` and `findings`; the existing 113 stay stranded.

### Changed — a reply goes out once

**Patch.** No tool's inputs or result shape changes; what changes is that the payload stops being
transmitted twice.

Every tool result carried its payload **twice, byte for byte** — once as `structuredContent`, once
as a text `content` block — on the reasoning that a client may read either. That was sound when
`structuredContent` was new. What it cost was never measured until 2026-08-23: unscoped
`detect_gaps` was 79,566 characters of payload and **157,785 bytes on the wire**, half of it a copy
no client read. The same tax was on every reply this server has ever sent.

- **The `content` block of a JSON tool result is now a one-line signpost** naming the field the
  payload is in. Measured on this design: **26,096 bytes on the wire, down from 53,181 after the
  reply budget and 157,785 before either** — a 6× reduction, with the payload itself unchanged.
- **A signpost rather than an empty block, deliberately.** Empty saves the same bytes and leaves a
  client reading the wrong field with silence — indistinguishable from reflow2 never having been
  configured, which is the outage `req:never-silently-absent` exists to end. ~450 bytes turns that
  into an instruction.
- **Prose tools are untouched.** `graph_report_markdown` and its siblings declare no structured
  output, so `content` is their only carrier and still holds the document. Pinned by a test.
- Not gated on the negotiated protocol version, which was the first proposal:
  `structuredContent` arrived in `2025-06-18` and rmcp still negotiates `2024-11-05` and
  `2025-03-26`, so a pre-structured client is genuinely possible — but the version lives on the
  `RequestContext`, and of 156 tool handlers exactly **one** takes one. Gating meant threading a
  context through the other 155 and all 151 `ok_json` call sites, to protect a client the signpost
  protects for one line.

### Changed — the first move of a session fits in the reply

**Minor** (the result shape of `detect_gaps` gains fields; one new optional parameter).

`detect_gaps` unscoped on this design returned **79,566 characters** and harnesses refused the
call outright. So the one call every instruction file tells a session to make first could not be
made at all on a mature design — and because the refusal came from the *client*, what the session
saw was a wall of harness text. reflow2 never got to say "pass `scope`".

`cap:bounded-reads` has said since 2026-07-25 that a read which would not fit answers with a
bounded page and says what it left out. It read `verified` — of `scan_nodes`, which is what
`ver:bounded-reads` actually drives — and was silent about the call the whole loop orbits.

- **`detect_gaps` now answers within a budget** (`budget_chars`, default 30,000 characters of
  payload; a client sees about twice that, because every reply is emitted as both
  `structuredContent` and a text block). The reply carries `budget`, which says which tier it
  landed in and what it withheld, in words.
- **A shorter answer is never a quieter one.** `count` and the new `by_source` describe every
  open gap in every tier. Only in the last resort is a gap absent from the list at all, and the
  reply says so and names how many.
- Cheapest information goes first, which is measured rather than assumed: a gap's `affected_ids`
  are capped at 8 (35% of the reply was id lists, almost all of it in three rollups, one of them
  enumerating 468 ChangeEvents whose own title already said "468 of 605"); then the prose goes
  from every row, never from some. Every row carries `affected_total` regardless.
- Scoped detection is budgeted the same way. Scoping is what an over-budget reader is *told* to
  do, and a Component at depth 3 holds 50–60 of this design's 83 gaps — so a scoped answer that
  would not fit either would have made the advice a dead end.
- `gap_to_prompt` and `gaps_to_prompts` **fill a withheld row back in from the graph.** Without
  it the ask half of the loop phrased questions from a blank description and recorded them
  against an empty anchor set.
- **`reflow2_check.py` read `affected_ids` to decide whether a gap was anchored** — and an
  anchored gap is the only kind that fails a build. Against a budgeted reply it would have called
  every real gap a phase nudge and gone GREEN on a design full of them, more reliably the bigger
  the design got. It reads `affected_total` now, asks for the whole reply
  (`--gap-reply-budget`), and fails outright if it was handed a list it knows is short.
- **The envelope unwrap in the Python clients was gutting rich payloads.** It unwrapped anything
  carrying `count` and `items`, which silently discarded `scan_nodes`' own `omitted` and
  `capped_by` — the very "what I left out" a bounded read exists to report. It now unwraps only
  the bare envelope.

## [0.38.0] — 2026-08-21

**Minor.** One new served tool, one new schema property, one new gap detector, a 21st served
skill, and a 4x faster test suite. Nothing removed; nothing renamed.

The theme is a sharper form of the one v0.37.0 ran into. That increment was about a report that
cannot say what it did not look at. This one is about **two states that look identical and are
not**: an idea nobody has opened versus one somebody judged and found genuinely new; a detector
that ran clean versus one that had nothing to run on; a parse that found nothing versus a parse
that was fed nothing. In each case the fix is a place to record the judgement, so the two stop
being the same.

### Added
- **`review_relations(node_type, node_id, links, note)`** and **`Decision.no_relation_note`**.
  Records what a node relates to — or, in writing, that nothing does. ONE DOOR for both
  outcomes, and it REFUSES when given neither. Relations come from the inference vocabulary
  (`CONTRADICTS`, `EVOLVES_INTO`, `DEPENDS_ON`, `ANTICIPATES`, …), each carrying its reason in
  `evidence`; `incoming` flips the direction, because direction is part of the claim. It draws
  nothing on its own and suggests nothing: a false neighbour is worse than a missing one, since
  anything searching by neighbourhood repeats it forever.
- **`unreviewed_ideas`** gap — proposed Decisions carrying neither a relation nor a note.
  AGGREGATE and severity 0.3: one finding naming the practice and listing the ideas, because
  per-idea it would have fired 115 times on reflow2's own graph the day it shipped. Detection is
  unconditional; the invitation waits for a boundary (`req:detecting-is-not-asking`), so thinking
  out loud still costs nothing at the moment of thinking.
- **`optimize` skill** and **`/optimize`** — the 21st served skill, and the first about improving
  something rather than describing it. Measure before forming an opinion (and be willing to
  conclude *nothing here*); measure the product surface before the developer surface; find the
  cause by falsifiable experiment on a copy; **write the budget down BEFORE the code**;
  re-measure against the budget rather than the starting point and STOP when it is met; leave a
  guard asserting the STRUCTURE that makes it fast, not a wall-clock duration; and when a rule
  refuses the change, pay it in prose rather than weakening it.
- **`tools/link_tests.py`** — which of a project's tests does the design know about? Zero
  configuration, on the same terms as `wall_check`. Attributes a test to a component only when
  its name matches a mapped source file AND it calls a function that file defines; everything
  else is reported unattributed WITH ITS REASON. A guessed mapping would make a per-subsystem
  table look complete while filing tests under the wrong part.
- **`docs/skills-and-tools.md`** — every skill and every tool with a one-line description,
  generated from the running server, plus who actually invokes what. (Short answer: nothing
  fires by itself. Skills are served but never auto-loaded, and hooks emit instructions rather
  than calls.)

### Changed
- **The schema is parsed once per process, not once per graph.** `open_in_memory` went from
  **41.3 ms to 266 µs** and the test suite's in-test time from **191.8 s to 96.5 s**. The eleven
  domain YAMLs are `include_str!`'d at compile time, so their bytes cannot parse to two different
  answers; the parsed `Result` is now cached and `load_schema()` hands out a clone, because
  `StorageEngine` takes the schema by value and two graphs must not share one.
  Governed by **`con:graph-construction-is-setup-not-work`** (≤ 1 ms) — the first performance
  budget this project has stated, written down before the code was touched.
- **The `brainstorm` skill links ideas at capture.** Step 4 spends the near-matches the dedup
  guard already returned instead of discarding them. Measured first: 145 brainstormed ideas
  joined by 12 edges, 111 of them reaching no other idea within two hops, and the most common
  edge on an idea was its author.
- **`tools/wall_check.py` reads both ways.** It walked design-outward, so a file no `Artifact`
  pointed at never entered its model — it could not report the gap because the gap was invisible
  to its own data structure. It now derives walk roots from claimed paths and separates "the
  design has never heard of this" from "known but not modelled as a part".
- **The CI gate notices unmodelled source** — as a NOTE and never a failure.

### Fixed
- `wall_check` and `link_tests` strip string literals line by line rather than file-wide. One
  unbalanced quote previously swallowed everything to the next quote, reducing `heal.rs` from
  dozens of visible functions to one — and **a parse that silently shrinks the evidence reads
  exactly like an honest "no evidence found"**.

## [0.37.0] — 2026-08-21

**Minor.** Two new served tools, one new schema property, and a changed result shape on
`detect_defects`. Nothing removed; nothing renamed.

The theme is the one the increment kept running into: **a report that cannot say what it did
not look at is indistinguishable from a report that looked and found nothing.** Four of the
items below are that same problem in different clothes.

### Added
- **`move_component(child_id, new_parent_id)`** — re-decomposition as an operation. Detaches
  EVERY parent the component had (a Project parent included) and names them. An empty
  `detached` means it was PLACED, not moved — a different fact, reported rather than folded
  into success. `level_note` reports the level relation at the moment of the move rather than
  in a later sweep; `history_note` names the `record_change` that preserves the previous parent.
  Previously the only route was `contain_component`, which ADDS a parent and removes nothing,
  so the discoverable path left the spine no longer a tree.
- **`set_capability_delivery(capability_id, delivery)`** and **`Capability.delivery`**
  (`artifact` | `model`; absent reads as `artifact`). Declares WHAT KIND of thing delivers a
  capability — never whether it was delivered, which stays computed. Both kinds still require a
  passing check. Work whose deliverable is a design change (a re-decomposition, a retirement, a
  governance ruling) can now be reported delivered instead of sitting `outstanding` forever.
- **`swept.rule_populations`, `swept.coupling_by_level` and `swept.coverage_note`** on
  `detect_defects`. What each rule actually walked, how much coupling exists at each declared
  decomposition level, and one line naming what the sweep could NOT have found. A rule can walk
  a large healthy population and still be silent about the level you asked about.
- **`tools/wall_check.py`** — holds a declared decomposition up against the real import graph of
  the files the design points at. Needs no configuration on any project: `Artifact.location` and
  `REALIZES` already say where the code is and which part owns it.

### Changed
- `arrival_delta` names `set_capability_delivery` when it reports an item as `outstanding` that
  has a passing check but nothing on disk — and stays silent when there is no check, because
  then the declaration would not help either.
- `gap_to_prompt`'s instruction now says to match the reader's domain and keep their field's
  vocabulary, dropping only reflow2's own internal terms. It previously said "for a
  non-engineer, no systems-engineering jargon", contradicting the tool's own description.
- `detect_defects`' description points at the three new `swept` fields.

### Fixed
- Three module cycles inside the crates, found by an adopt pass over reflow2's own source. A
  module cycle inside one crate is legal Rust and compiles silently forever.
- Two **subsystem** cycles that the module fix did not remove: the five-module kernel was
  bisected by the declared decomposition, and `compare` reached into `report` for a two-line
  lookup. The design-type list moved down to `nodes`, where it is made of `node::` constants.


## [0.36.0] — 2026-08-19

**The whole increment came from one user's field reports.** reflow2's second user — running it on
a large project at work, and about to show it to coworkers — reported three frictions, and each
turned out to be a defect in what reflow2 SERVES rather than in what it computes. No schema change
(stamp unchanged at 29/61/1), so no upgrade doc; but the served skills changed behaviour, which is
what a consumer feels.

### Changed

- **Orientation now asks what LENS the reader brings, and stops assuming who they are.**
  `where-am-i` carried a hardcoded audience — *"write it as prose for the person who described this
  project to you"* — which assumes the reader briefed the agent. That is false for exactly the
  newcomer it matters for. It now asks about their background once, records it on their
  `Contributor`, and does not ask again.

  ⭐ **The ask asks for a BACKGROUND, not a name, and carries an example of a good answer.** The
  first version asked *"who am I talking to?"* and was shipped for one day before the owner caught
  the hole: **"Bob" answers that correctly and tells the agent nothing.** A question a useless
  answer satisfies has not asked for what it wants. It now asks what you do day to day AND what you
  trained in, showing the shape wanted — *"software engineer, but my degree is in biology"* —
  because the two often diverge and **the divergence is the informative part.** A thin answer gets
  one follow-up and then it stops; this is an opening courtesy, not an intake form.

  ⚠️ **And the rule is a VOCABULARY SWAP, not simplification.** A systems engineer wants
  *requirement*, *interface* and *verification* KEPT — softening them is condescension. What gets
  dropped is reflow2's OWN vocabulary: gap, loop, detector, node id. Someone who knows another field
  entirely wants that field's terms. **None of them wants "simpler English"**, so a plain-language
  mode would have been wrong for every user observed. (#246, #247)

- **The register rule reaches every reply and the step where the user's question is written — and
  the build now checks it.** Measuring the above found it was one skill deep: register rules
  appeared in 3 of 20 served skills, only `where-am-i` read the stored background, and
  **`gap_to_prompt` — the tool that turns a gap into the question a person actually reads —
  mentioned vocabulary nowhere.** It said *"phrase a gap as a plain question"*, and plain is not the
  same as in your domain. There is now a standing rule in the served instructions (a peer of *"the
  one rule"*, so it binds whatever skill is loaded), the rule at the gap→question step itself and in
  `gap_to_prompt`'s description, and a seventh obligation in `detect-and-ask`'s ask contract —
  **which `skill_lint` now fails the build without.** The existing *"answer in the user's language"*
  rule is a different axis and says so: it picks English or Portuguese, not systems engineering or
  baseball. (#252)

- **Detecting a gap and inviting its closure are now separate acts.** Entering information about a
  project came back, every time, as *"I have those all recorded. There are open gaps with X, Y and
  Z. Do you want to fix them?"* — so the user had to decline repeatedly in order to keep doing what
  they came to do. **Not a bug: three served sources instructed it** (`capture-intent` step 7, the
  loop's step 2, `detect-and-ask`'s own trigger), each written for a real failure — the agent that
  only ever adds nodes and surfaces nothing. Together, on someone doing nothing but capture for
  days, they produce a prompt to close after every message. **Detection still runs on every
  capture; the invitation now waits for a boundary** — the user asks, pauses, says they are done, or
  turns to building. ⚠️ **This defers the invitation, never the record**: gaps stay counted and stay
  loud. (#250)

- **A `Constraint` need not be numeric, and a data model has a home.** Asked how to model "enums", an
  agent handed the question back to the user. **Both homes already existed and neither was
  instructed.** `SPECIFIES` is `Artifact → [Interface, Capability, Component]` with
  `format: json_schema` — an enum lives IN a schema file, so register the file and point the edge at
  what it defines. And `Constraint` requires only `name` and `statement`; every numeric field
  defaults. Three surfaces described it as a numeric budget — the routing table, the served
  instructions, and `add_constraint`'s own description — and a reader had already concluded the type
  was unusable and left eleven constitutional prohibitions as Requirements that would report
  unsatisfied forever. All three corrected; the routing table gains rows for a value set, a
  prohibition and a schema file. 🛑 **`SPECIFIES` still has no typed tool** — nine typed edge tools
  exist and it is not among them — so the row says `create_edge` and says why. (#248)

### Notes

- The design record also carries two brainstorms and one measurement pass with no shipped behaviour
  (#244, #245), and one redaction: an illustrative example in the persona decision named a real
  third party by full name and employer in a public repository, and now describes them generically.
  The design point never needed the name. (#251)

- **Not verified, and stated rather than left to be discovered:** every rule above is served prose.
  `skill_lint` proves the text is present and mirrored, and now fails without the domain obligation
  — but **nothing measures whether the translation is any good.** No trial has taken a real gap list
  and had it narrated to a non-technical reader. That trial is the missing instrument.

## [0.35.0] — 2026-08-18

### Changed

- **A revising write now says whether the state it replaced actually survived — computed, not
  assumed.** The `revision` block already told every caller *"`record_change` BEFORE the merge is
  what puts the old state in the design's own timeline"* — **unconditionally**, in identical words,
  to someone who had just done exactly that and to someone who had destroyed something. Advice that
  never varies is advice a reader learns to skim, and the one time it mattered looked like all the
  times it did not.

  It now reports `prior_state_preserved_in` — the Snapshot holding the replaced state — and says one
  of two different things:

  - preserved: *"the state it replaced IS PRESERVED, in `snap:…`. Nothing is lost… there is nothing
    to do about it."*
  - not: *"AND NO SNAPSHOT HOLDS THE STATE IT REPLACED — checked, not assumed… To undo: write the
    prior value back, then record_change, then re-apply."*

  **Matched by content hash, not by epoch.** "At the current epoch" needs a notion of *current* that
  reflow2 does not have, and answers a weaker question. The hash answers the one a caller actually
  has — *is what I just replaced recoverable?* — so a stale snapshot of a **different** state is
  correctly **not** counted as preservation.

  `req:a-discipline-is-delivered-at-the-tool-not-in-a-catalogue`, whose stronger form is to compute
  the *outcome* rather than track the *invocation*, because that survives an agent which ignores
  every hint. dev_storyflow's dragon Boss proposed the identical shape independently: *"report
  whether the target has a snapshot — NOT BLOCK, JUST SAY."* Nothing blocks: a tool that blocks
  becomes a tool people route around, and then the graph stops matching reality.


### Added

- **`set_capability_signature` — a capability can finally say what it takes in and puts out.**
  `capability_type`, `inputs` and `outputs` were declared in `schema/functional.yaml`, indexed and
  documented, and set on **0 of 170 capabilities**. Not unwanted — **unwritable**: `add_capability`
  writes name, description and status, and nothing anywhere else in either crate touched them. No
  project using reflow2 could record a capability's functional signature by any route the product
  offered.

  `req:recursive-black-box-decomposition` says every element of a design is a black box with inner
  function **and interfaces**. At the capability tier, these two lists *are* that interface.

  A **setter** rather than three more constructor parameters, for the reason `set_interface_spec` is
  one: a contract is enriched onto a node that already exists, usually long after it was created.
  `add_capability` also has **276 call sites**. Omitting a field leaves it alone, so two people
  describing a capability from opposite ends cannot overwrite each other; an **empty list is a
  statement** ("this takes nothing in" is a real claim about a source or generator) distinguishable
  from nobody having said; and an unknown capability is **refused**, not created.

  **No detector fires when a signature is missing, deliberately.** 170 capabilities lack one today,
  so a gap apiece would put 170 findings in front of a reader overnight — the wall-of-red failure
  the vocabulary-coverage trial was run to avoid. This is the *tool* leg; the *instruction* leg is
  planned as `epoch:the-discipline-arrives-at-the-tool`.

### Fixed

- **The tool catalogue was unstable under its own growth, and adding one tool exposed it.**
  `find_tools` weighted every query term equally regardless of how many tools mentioned it —
  `capability` appears in dozens of descriptions, `file` in a handful. Measured on *"register a file
  that realizes a capability"*, the top six scored **28, 27, 26, 26, 25, 24**: a one-point near-tie
  across the whole visible band, with a default limit of 5. So **any new tool mentioning a common
  word silently evicted whatever was fifth** — here, `link_artifact`, the actual answer.

  With 152 tools and rising, that meant any addition could displace the right answer for a query
  nobody was thinking about, and the failure is silent by construction: nothing tells you what you
  needed was sixth. `req:agent-native` promises every capability is reachable over one surface,
  which only holds if the agent can find the tool.

  Term weights are now `ln(1 + N/df)` — classic inverse document frequency, which `search_design`
  next door has had via BM25 for months. After: **49.5, 38.3, 32.1, 29.1, 29.1, 28.8**, and
  `link_artifact` moved from 6th to 3rd. Fixed at the root rather than by widening the assertion or
  trimming the new tool's description, both of which were available and both of which would have
  hidden it.


- **`vocabulary_coverage` — which of the design vocabulary has this design never used?** A question
  nothing else in reflow2 asks: the detectors check the *consistency of what exists*, and the
  absence-checkers there are (`unsatisfied_requirement`, `unallocated_capability`) check absences of
  **required** structure. Vocabulary a design has never touched was invisible to every computation,
  in every project.

  **Portable by construction** — the schema is embedded in the binary and the corpus comes from the
  caller's graph, so the answer needs nothing about any project's file layout.

  **Every decision about its shape came from a two-arm trial run on reflow2 itself**, not from
  argument:

  | | node types | edge types | properties | flat list |
  |---|---|---|---|---|
  | mature (2535 nodes) | 22/29 | 37/61 | 141/169 | 31 items |
  | day one (post-`genesis`) | 2/29 | 0/61 | 8/18 | **88 items** |

  - **The figures ship** — they passed both arms. *"0 of 61 edge types"* on a new design is a true
    and useful statement, not noise.
  - **The list is grouped** by the schema's own eleven domains, never a grouping the code invented.
    The trial found unused vocabulary clustering into whole subsystems, and the clusters turned out
    to *be* the domains — so a mature design reads about four findings instead of thirty-one.
  - **A domain is parkable**: an accepted Decision at `decision:vocab:<domain>` declares it
    deliberately unused. Needed because `OWNED_BY`'s absence is *already* ruled deliberate, so
    without this the report would name a settled decision as a hole forever. No new tool —
    `add_decision` + `set_decision_status`, and the id is stated on every finding.
  - **The flat list is withheld unless asked for.** It is *longest for the user least able to act on
    it* — 88 items on day one against 31 on a mature design — so pushing it would have made the
    feature worst for exactly the people it is for.
  - **A design under ten nodes is told it has barely started**, rather than shown a wall of red on
    its first read.

  Five probes, one per decision, and five mutations each killing exactly its own probe.


### Added

- **The first `SPECIFIES` edges in the graph's history — 44 of them.** The edge type was declared in
  `schema/build.yaml` for exactly this (*"an OpenAPI / protobuf / JSON-schema / IDL file SPECIFIES
  an Interface (its authoritative contract)"*), carries a `format` property listing `json_schema`,
  and had **zero instances**. It was written for INGEST — its `extraction_hint` is a prompt for
  pulling structure out of a user's documents — so it was built for other people's projects and
  never turned on this one. The 11 schema YAMLs now specify the four interfaces that carry the
  design vocabulary: `ifc:mcp-tools`, `ifc:mcp-tools-http`, `ifc:core-api`, `ifc:graph-export`.

- **`tools/vocabulary_reach.py` — what of the declared vocabulary can nothing write?** Reports in
  three buckets so no number can be over-quoted: **18 candidates** (the type is used, the property
  never is, and no typed tool accepts the name), **10 unused-but-offered**, and **46 that say
  nothing** because the node type has no instances at all. Plus 42 edge-vocabulary items with no
  instances.

  **The primary signal is corpus usage, not a name match**, and that is the design. The obvious
  instrument compares declared names against tool parameters; it was tried first and reported 70 of
  215 (32%), including `TemporalFact.valid_from`, which is perfectly writable. `create_node` and
  `create_edge` are excluded — they accept an arbitrary property bag, which is exactly why they hid
  all four known instances.

  **Validated against ground truth before its new findings were believed:** it reports `SUPERSEDES`
  (zero edges, known), `GOVERNED_BY.ruling` (added yesterday, 0 of 912), and `DECOMPOSES` (zero
  edges, while `req:decomposition-covers-its-parent` is accepted); it stopped reporting `SPECIFIES`
  the moment the 44 edges were exported; and it does **not** report `Verification.description`,
  which was unreachable until yesterday and now has a parameter and real uses.

  ⭐ **The standout finding:** `Capability.inputs`, `Capability.outputs` and
  `Capability.capability_type` are **0 of 170**, and spot-checking the source confirms nothing
  anywhere writes them. A capability's functional signature — what goes in and what comes out — is
  declared vocabulary that has never once been recorded. `req:recursive-black-box-decomposition`,
  accepted the same day, says every element is a black box with inner function *and interfaces*; at
  the capability tier those two properties **are** that interface.


### Changed — breaking

- **The default scope depth drops from 3 to 2, because 3 did not narrow.** Measured by driving the
  built binary over all 56 Components of reflow2's own design: at depth 3 **every one returned
  50–60 of the design's 84 gaps** (median 55) over regions of 595–903 nodes. The spread across all
  56 was 50..60 — indistinguishable — so `in_scope: 55, out_of_scope: 28` told every team the same
  thing about its own part. At the new default the same sweep returns **2–27 (median 4)**.

  **The old default had a stated reason and the reason was wrong.** `scope.rs` argued 3 was needed
  to reach "a contained child component's capabilities". It is not: `scope_region` puts the entire
  containment closure into the seed set *before* taking the radius, so a child three levels down is
  already a seed and its capabilities are one hop from one. Depth 1 was rejected separately — it
  stops short of the requirements a component's capabilities satisfy.

  Anyone passing an explicit `depth` is unaffected. `dec:the-default-scope-depth-should-be-two`.

### Added

- **A scoped answer now says whether it narrowed anything.** `Scoped` gains `share_of_anchored` —
  how much of everything the design has to say is in this answer — and a `narrowing_note` that
  appears in words when that exceeds half. The denominator is `in_scope + out_of_scope` and
  deliberately not `total`: unanchored findings could never fall in any region, so counting them
  would flatter every scoped answer by a constant.

  Run against the old default as a control, the note fires on **56 of 56** Components; at the new
  default, on **0 of 56**. `req:a-scoped-answer-actually-narrows`.

### Fixed

- **`detect_gaps` stopped claiming its region is "the same computation claim_region uses".** It was
  not, on two counts: the defaults differed (3 vs 2) and `scope_region` adds a containment closure
  that `claimed_region` omits. The sentence is gone. Reconciling the two computations for real is
  NOT bundled here — that changes what two people believe they each hold, and it needs its own
  decision.


## [0.34.0] — 2026-08-17

### Added

- **A session that holds nothing can ask where to stand.** New seedless read `design_regions`:
  no seed, no scope, no topic. It answers with the parts the design itself names — its Project
  and Components — each with its size, the gaps and defects open inside it, and who already
  claims it. A row's `seed_id` is then what every scoped call wants, which is the requirement in
  one line: *a session can FIND its seed rather than needing one to start.* Reported by
  dev_storyflow's fleet, 2026-08-08: *"the moment with the most time available — sitting
  AVAILABLE, nothing to do — is the moment the design brain is LEAST USABLE."*

  **It is not a partition and says so.** `coverage` reports how many nodes lie in NO region and
  how many lie in MORE THAN ONE, broken down by type. On reflow2's own design that is 1877 of
  2487 uncovered (mostly ChangeEvents and Snapshots no Component contains) and 376 of the 610
  covered sitting in several regions at once — rows that overlap that heavily are not the
  separate areas they look like. A design that names no parts yet gets `regions: []` **with a
  note saying that is why**, because an empty list is otherwise indistinguishable from a clean
  one.

  **The carve-up is the design's own, never one reflow2 inferred.** Leiden community detection
  was already in the crate and was refused: "here are your design's twelve clusters" is a claim
  from a heuristic with a resolution knob that the reader cannot check, which is the failure
  `epoch:instruments-stop-overstating` exists to remove.

- **A measured defect in the scoped detectors, reported and NOT fixed.** Driving the built binary
  over all 56 Components of reflow2's own design: at the default depth of 3, every one of them
  returns **50–60 of the design's 83 gaps** (median 55) over a region of 595–903 nodes. The
  answers are indistinguishable, so `in_scope: 55, out_of_scope: 28` tells every team the same
  thing about its own part. At depth 1 the same parts hold 0–19 gaps over 17–139 nodes; at depth
  2, 2–27 over 267–601. `scope.rs`' own justification for 3 does not survive reading
  `scope_region`, which puts the whole containment closure into the seed set *before* taking the
  radius — so a contained child's capabilities are one hop from an owned node, not the three the
  comment claims. Recorded as
  `fact:defect-a-scoped-detector-at-its-default-depth-returns-two-thirds-of-the-design` and put
  to the owner as `dec:the-default-scope-depth-should-be-two`; `design_regions` defaults to 1
  rather than inheriting a number the measurement says is wrong, and says where a reader will
  meet it that the two differ.

- **A ruling can declare a state deliberate, and the deliberate ones are COUNTED.** A `GOVERNED_BY`
  edge carrying `ruling: parks` records that a node's unattached or unsatisfied state is correct on
  purpose. `orphan_node` and `unsatisfied_requirement` skip it; `detect_defects` reports it in
  `swept.parked`. `governed_by` gains a `ruling` parameter.

  **The finding that reframed the work, reproduced before anything was designed:** an Artifact
  carrying *any* `GOVERNED_BY` edge **already** escaped `orphan_node` — the detector saw an edge,
  never a ruling. **Silence was already available and was never the problem.** What was missing was
  the count: "deliberately parked" and "never looked at" gave the identical answer. A reader can now
  see "34 defects, 8 parked" instead of "97", or instead of nothing.

  The measured failure: defects **88 → 97 across ten writes**, eight of them deliberate
  registrations a standing ruling prescribed, with 31 more owed. The correct action was degrading
  the instrument — and self-reinforcingly, since a reader watching the count climb has an incentive
  to stop registering documents at all.

  ⚠️ **A probe failed on its first run and found the real defect:** an *unsettled* parking claim was
  silencing a genuine finding, because the `GOVERNED_BY` edge itself counted as attachment. A
  `ruling: parks` edge no longer counts as attachment — it is a claim that the node is deliberately
  attached to *nothing*. The ruling must also be an **accepted** Decision; a `proposed` one is
  somebody thinking out loud, and a musing must not suppress a finding.

  **Not addressed:** music_graph F9/F14 — recording *planned* work manufactures gaps, so the list
  punishes writing a plan down. Same shape from a third direction, and a different fix.

- **A check has somewhere to put what it FOUND.** `Verification` gains `findings`;
  `add_verification` gains `description`; `set_verification_status` gains `findings`. Three fields
  for three things — `name` labels, `description` says what the check IS, `findings` says what a
  RUN found.

  **The requirement named its own precondition — *"would a new field alone change anything?"* — and
  the corpus answered no.** Measured before writing code: 164 Verifications, **median name 76
  words**, 72 over 100, longest 654 — and `description` was *already* declared, fulltext, and the
  node's embedding field, **used once in 164 nodes**.

  **The root cause was reachability, not neglect.** `add_verification(id, name, method, level)` had
  no parameter for `description`. The only route was raw `create_node`, and essentially nobody took
  it — authors wrote reports into `name` because **`name` was the only string the constructor
  accepted.** A `findings` field alone would have become the second unused field.

  Where each is written is the design: `description` on the constructor (stated once), `findings`
  on the status call (a finding belongs to a *run*). Omitting `findings` **leaves it alone**,
  exactly as `last_run_at` does, so re-marking a check passing without restating evidence does not
  erase it.

- **`loop_status` shortens a long check name — and says that it did.** Names past 25 words are cut
  in the rollup and carry `name_truncated`, `name_words`, and a top-level `names_truncated` note.
  The announcement is the point: silent truncation reads as "that is the whole name". The cause was
  fixed alongside the symptom rather than instead of it.

  **Bounds:** nothing is migrated — the 164 existing long names stay as they are, because a
  property once written cannot be removed and rewriting authored names is not a mechanical act. And
  `findings` is **not validated or parsed**: a `passing` status beside findings describing a failure
  is a contradiction only a reader can catch. Schema gains a *property*, not a type, so the version
  stamp does not move.

- **A write can be made conditional on what you read.** `create_node` accepts
  `expected_content_hash`: supply the `revision.prior_content_hash` from when you read the node and
  the write becomes a **compare-and-swap**, refused if the node moved in between, naming both
  hashes. Omit it and nothing changes.

  **The failure, measured from both sides of one collision:** a worker read a node, ninety seconds
  later another attached session wrote it, the write returned a normal success with the full node
  body — and **the winner was never told**. The loser found out only because `record_change`
  happens to return the snapshot it took: a diagnostic side-effect of an unrelated tool, not a
  guard.

  The `revision` block already *reported* an overwrite; that tells the loser afterwards and the
  winner nothing. `rule:fix-it-properly-while-it-is-still-cheap` is why this is a refusal rather
  than a fifth report. Four calls behind it: it guards the **upsert** (the path every surface
  writes through, not the raw create); **absent is a mismatch** rather than a create, because
  silently recreating a deleted node undoes somebody's removal; the hash **moved into the core**,
  since two implementations of one number would diverge only under contention; and it is
  **opt-in**, because a caller who never read the node has no honest expectation to state.

  ⚠️ **`create_node` also gained the `revision` block it never had.** The block was attached by the
  typed constructors and never by generic `create_node` — so for one commit the compare-and-swap
  existed with its precondition value unobtainable from the very tool demanding it. Every core test
  passed; the MCP probe failed on its first run. Mutation-checked: discarding the refusal fails
  exactly the two probes that should fail.

  Closes the `required` obligation of `epoch:instruments-stop-overstating`. **Honest bound:** the
  typed constructors still merge with no expectation available, which is the commonest write path,
  so the hole this closes is real but partial.

### Design

- **"Do it right" is now intent reflow2 carries, not just a habit this project has.**
  `req:a-fix-says-whether-it-corrected-the-cause` — accepted 2026-08-17 — says a repair must
  record *which kind* it was (cause corrected, or symptom worked around), that a workaround must
  name the correction it stands in for, and that the design must then be able to answer **"what is
  still resting on a patch?"** without asking a person.

  **The question is unanswerable today, and that is the case for it.** Measured on reflow2's own
  graph: 472 ChangeEvents across eleven change types, and *every one of them names what moved
  while none says whether it was the right fix*. A `test_failure_fix` is equally a root-cause
  rewrite and a shim that turned a red test green. (Checked and ruled out: the obvious hypothesis
  was that `test_failure_fix` is a silently-defaulted value — it isn't, all 55 auto-minted ones
  carry written reasons. The vocabulary is the gap, not the discipline.)

  The shape is already proven one layer over: `dec:two-sided-accept` made accepting drift a
  two-sided act because "accept the file, say nothing" is what erodes a design into fiction. **A
  fix is the same shape and is currently one-sided.** It must not judge — reflow2 records the
  claim and counts it, the human decides whether a patch was right — and it records what an author
  *says*, so it can never detect a patch reported as a correction.

  `dec:the-patch-record-binds-forward-not-backward` (accepted): the 472 existing events are not
  reclassified, on the same reasoning as the authority check's grandfather ruling — nobody
  remembers, so a backfill would be an agent's inference wearing the clothes of a record. The cost
  is that the number accrues slowly, which is why the reporting side owes a statement of its own
  coverage from day one: a low count must not read as "few patches" when it means "few fixes
  recorded yet".

- **reflow2 now says what it deliberately does NOT do.** Four non-goals, ACCEPTED on Anthony's word
  2026-08-17, because silence on a boundary reads as coverage — the same harm as a vacuous zero,
  arriving through absence. reflow2 **does not** verify that a named package or method exists;
  **does not** know whether an API is current or deprecated; **does not** judge whether a check is
  meaningful (a passing Verification is a claim it records, not one it validates — a design can
  reach 100% verified on tests that could not fail); and **does not** tell you your requirement is
  wrong, only that it disagrees with something you already settled.

  Each names whose job it is, and what reflow2 *does* do nearby, so it is a boundary rather than a
  shrug. Prompted by a taxonomy of AI-agent pitfalls: recording the cures while staying silent on
  the five reflow2 cannot reach would have implied coverage of all ten. **A non-goal is a statement
  about the current build** — retiring one is part of shipping whatever voids it, because a stale
  non-goal is a false statement in the other direction.

- **`rule:fix-it-properly-while-it-is-still-cheap` split into two clauses with different
  lifespans.** The first draft ran them together and said the whole thing expires at 1.0; Anthony
  corrected that — the philosophy is enduring. **Clause 1 never expires:** when something is
  wrong, go back to the drawing board and re-implement it properly. **Clause 2 changes at 1.0:**
  "it would break consumers" is not a reason to keep a defect — it is a reason to correct it now.
  At a stability commitment, clause 2 is replaced by a deprecation discipline rather than dropped.

> **This increment is a MINOR, not a patch, and both reasons are deliberate breaks.** The
> `disconnected_community` defect category is renamed, and unscoped `detect_defects` returns an
> object instead of an array. Both were declined earlier the same day *because* they break
> consumers; `rule:fix-it-properly-while-it-is-still-cheap` (Anthony, 2026-08-17) reversed that:
> before a stability commitment exists, "it would break consumers" is a reason to do it **now**,
> since the price of a break only rises with every user. Migration is two lines and is below.

### Changed — breaking

- **`disconnected_community` is now `unthreaded_cluster`.** The old name asserted unreachability
  and the detector computed something narrower — the nodes it named genuinely *were* reachable by
  an undirected walk, because the topology walk drops nine node types and every review record and
  does not count `CONTAINS` (measured: it covers 1133 of a 2413-node graph). Fixing the message
  alone left an identifier that still said the wrong thing. `unthreaded` names what is actually
  missing — a traceability edge, the golden thread — and cannot be misread as "unreachable in the
  graph".

  **Migration:** match on `unthreaded_cluster` instead of `disconnected_community`. The ility
  source string moves the same way (`detect_defects.unthreaded_cluster`). **No data migration** —
  the key was never persisted in any graph, only in code and prose, which is what made the rename
  cheap enough to be obviously right.

- **Unscoped `detect_defects` returns `{swept, defects}` instead of a bare list.** `swept.nodes`
  is what it examined, `swept.rules` names the checks that ran, `swept.design_network_nodes`
  reports the narrower topology walk rather than hiding it, and `swept.note` appears **only** when
  the sweep could not have found anything. So an empty result now says which empty it is —
  exercised and found nothing, or nothing to examine — instead of leaving a zero to be read as
  permission before `apply_heal`, which deletes nodes.

  The scoped call has answered this way since 2026-08-09 (`Scoped` carries `total`, `in_scope`,
  `region_size` and a vacuity note); one tool was answering the same question two ways and the
  honest half was the half fewer people call.

  **Migration:** `detect_defects` → `detect_defects.defects`. In Rust, `detect_defects()` returns
  `DefectSweep`; `open_defects()` is the bare list for callers that only want findings.

### Fixed

- **`detect_defects` no longer returns a clean bill over a node attached to nothing.** The
  degree-zero rule inside `orphan_node` ran on `Decision` alone, so dev_storyflow's fleet got
  `clean` back over a DesignEpoch carrying **no edges at all** — in two packages, through every
  health call of a session — from the pass whose whole job is structural soundness. It now runs on
  every node type nothing else asks about. On reflow2's own graph the rule goes from reporting 7
  zero-degree nodes to 21; the newly visible ones are epochs marking nothing, fragments with no
  source, a Verification counted among the passing that says what it checks to nobody, and seven
  TemporalFacts whose `subject_id` names a node that does not exist.

  Three bounds, because a wider sweep is only worth having if it is still true:
  **a pointer property is attachment** — `TemporalFact.subject_id`, `Snapshot.target_id` and
  `Question.gap_id` are required indexed properties, so those nodes name what they are about
  without an edge, and only a pointer that resolves to *nothing* is reported (the naive rule would
  have filed 48 of reflow2's own 212 facts); **DETECT keeps its own types** — Requirement,
  Capability and Interface are excluded, because asking there as well is the two-vocabularies
  duplicate that became 20 of 31 defects on the storyflow trial (BL-42); and **a resting state is
  not a defect** — a lone Project means the design is empty, which is what genesis produces on day
  one, and an advisory DesignRule may bind the process rather than a node.

  First half of `req:a-report-says-what-it-swept-and-whether-its-checks-ran`. Reported by
  dev_storyflow's fleet 2026-08-08 → 2026-08-15.

- **A structural finding now describes the walk that produced it.** `disconnected_community` said
  "disconnected from the rest of the design" while walking something much narrower — and both
  halves of the field report were true at once, because `design_network()` is not the graph: it
  drops nine node types and every review record, and `CONTAINS` is not a traceability edge, so a
  node reachable only through `AUTHORED_BY` is an island there and connected here. **Measured: the
  walk covers 1133 of 2413 nodes.** The message now says what it computed, how much of the graph
  it held, which exclusions most often explain a false island, and how many singletons it does not
  report. The category key is deliberately unchanged — consumers match on it.

- **An empty `open_questions` says which zero it is.** It returned 0 while the loop was owed 31
  other things, and it is the orientation call a session runs first. An empty answer now carries a
  `loop_hint` naming the other non-zero debt, or stating an all-clear explicitly. The existing
  hint throttle (`dec:read-hint-shape`) is right while a reader is being handed findings and
  inverts when the answer is empty — it removes the only sentence in the reply — so empty answers
  are exempt from it and non-empty ones are not.

- **A scoped answer stopped claiming nothing could be found when it found something.** The scoped
  detectors' vacuity note was gated on the region being one node, which was safe while every rule
  needed an edge to fire. The degree-zero rule above ended that, and the first scoped call against
  the new binary returned a real finding with "nothing could have been found here" beside it. Now
  gated on region-of-one **and** found-nothing. Caught by driving the built binary; nothing was
  comparing the prose to the number it described.

  These three complete `req:a-report-says-what-it-swept-and-whether-its-checks-ran`.

### Changed

- **AGENTS.md was wrong about how to pick up a rebuild, and it cost a deliberate action that did
  nothing.** It said a `/mcp` reconnect serves a fresh binary. Wherever a shared server is running
  — the normal case here — `--shared` re-attaches to the long-lived `--serve-shared` daemon, which
  keeps executing the image it started from, so the reconnect spawns a fresh *client* against a
  stale *server*. Measured: binary rebuilt 22:13, reconnect 22:16, and `detect_defects` went on
  answering the pre-change number from code no longer on disk; `--stop-shared` then produced the
  new one. `service.rs`'s own `STALE_NOTE` has said this correctly since 2026-08-11 and the doc
  contradicted it. Note `served_by.stale` rides on `graph_report` alone — `loop_status` and
  `detect_defects` carry no staleness block, which is left open rather than guessed at.

## [0.33.0] — 2026-08-16

Four merged changes, and the shape of the increment is that **three of the four exist because a
number was being ignored, not because anything was broken**. A loop that reported the same figure
every call, a date half the corpus wrote where nothing read it, a link that could not say whether
anyone had checked it, and a field that cried wolf. None of them threw an error; all of them taught
a reader to stop looking.

### Why this is a minor and not a patch

Stated rather than left implicit, because this file asks for the judgement to be shown. Three tool
surfaces changed **shape**: `loop_status` gains `since_export`, `realizes` and `link_artifact` gain
`conformance`, and `search_design` results gain `as_of` / `age_days` / `expired` on dated nodes.
Under this file's own table that is minor — the input or output *structure* moved, not merely a
swallowed failure becoming loud.

**Every change is additive and no existing call changes behaviour.** The two new arguments are
defaulted, so a caller who omits them gets exactly the previous result; the new result fields are
omitted entirely on nodes that carry no dates.

**The schema stamp did not move** — 29 node types, 61 edge types, `schema_version` 1. Two
*properties* were added (`TemporalFact.valid_from` / `.valid_to`, `REALIZES.conformance`) and
properties are not counted by the stamp, so **no upgrade doc is owed** and no consumer migration is
required.

### ⚠️ One thing to expect on upgrade, so it is not read as an alarm

`evidence_report` now reports a `conformance` tally, and on any design that predates this release it
will say **every realizing link is `unchecked`** — reflow2's own says 223 of 223. That is not a
finding about your code. `unchecked` means *nobody has recorded that they checked it*, which was
true before this release too and simply could not be said. Nothing is required of you; the number
exists so the silence is countable.

### Added

- **The loop can be asked what THIS session added.** `loop_status` gains `since_export`, answering
  what the committed record does not yet hold, so a mature graph's standing debt stops drowning the
  debt just created. Measured beforehand: across an entire working session `loop_status` returned
  the *identical* 80 unsurfaced gaps and 16 structural defects from the first call to the last,
  through four merged PRs and two releases. The failure is behavioural rather than arithmetical —
  **a number that never moves stops being read** — and it arrived on the one tool whose job is to
  say what is owed. Asked for independently three times: twice by one consumer and once by another.

  The baseline is the last export, which is the only session boundary a design with no clock and no
  session identity can compute. An **empty baseline says it cannot tell** rather than reporting
  everything as new: "you added 205 things" and "I have no record to compare against" are opposite
  answers. The reply always states which boundary it used. Off by default, because the scan reads
  and parses the committed export and an ordinary orientation call has no reason to pay for it.

- **A fact may carry its date, and it is read back with its age.** `TemporalFact` declares
  `valid_from` and `valid_to` as date-string properties, and `search_design` returns `as_of`,
  `age_days` and `expired` for any dated node — so a six-week-old observation is no longer narrated
  as current.

  This was **measured before it was designed**: 112 of 205 TemporalFacts carried a `valid_from` the
  schema did not declare, 76 carried the declared `VALID_FROM` *edge* to a DesignEpoch, and **zero
  carried both**. A clean split with nothing caught between two conventions is what a *missing word*
  looks like rather than what carelessness looks like — the edge needs an epoch, and a writer
  holding a plain date has nothing to point it at.

  The properties are not a second way to say one thing: the **edges** name a milestone and are what
  a roadmap orders by `sequence`; the **properties** carry a calendar position. A roadmap wants the
  first, an audit trail the second.

  The scope grew on evidence and it is worth knowing why. The only reader of `VALID_FROM` filters to
  `basis: forecast`, of which this design has **none** — so the 76 edge-bearing facts were as unread
  in practice as the 112 property-bearing ones, and there was no reader to teach. Declaring the
  property alone would have changed the schema and left the corpus exactly as inert, so the reader
  ships with it.

  Three refusals are deliberate: a **future** date reports a *negative* age rather than clamping to
  zero, because clamping renders a forecast as "current"; a claim expiring **today** is not yet
  expired; and an **unreadable** `valid_to` never expires anything, because guessing would silently
  retire live facts. No new dependency — the calendar arithmetic is fifteen lines.

- **A realizing link says whether anyone checked it.** `REALIZES` gains
  `conformance: unchecked | reviewed | verified`, defaulting to `unchecked`, and `evidence_report`
  counts the buckets and names up to ten unchecked links.

  The evidence came from outside the project. A Requirement said the calendar day is the person's
  **civil** date; the code used UTC; the Capability said `realized`; the checksum matched, so
  `reconcile_artifacts` was silent. **Every signal in the graph was green and a user found the bug
  in the product.** A checksum says the file has not *moved*; nothing said whether it still *does
  what the target requires*, and a file checked against the rule was indistinguishable from a file
  merely hashed.

  reflow2 does not and cannot compute this — it reads a design, never a running system. The property
  **records** what a person knows, and the count is the deliverable: *"223 of 223 realizing links
  were never checked against their requirement"* is a sentence the design could not previously
  produce.

  Deliberately **not** in `loop_status`: every link starts `unchecked`, so a large figure that never
  moves would be precisely the failure the first item in this release exists to end.

### Fixed

- **A swept file stops being called unobserved because its parent also claims it.** Reported by
  music_graph. `coverage_report` matched each observed path against the *first* claim covering it, so
  a design registering both `archive/` and `archive/reco.py` got the individual files back in
  `unobserved_locations` — files the sweep had just handed over.

  Graded "harmless" by the reporter, and the grading is the argument for fixing it: the field's only
  job is answering *did you forget to sweep something*, so a false entry is an alarm on correct
  modelling, and one is enough to make the field untrustworthy. The count stays one per observed
  path; the cheapest wrong fix — emptying the field — is blocked by three cases, because silence
  reads as "nothing was missed".


## [0.32.1] — 2026-08-16

A patch cut, made so music_graph gets **F24** without waiting for the next increment: v0.32.0 fixed F23 and shipped an hour before F24's fix landed, which left a consumer able to restore successfully and then walk straight into the second half of the same failure.

### Why this is a patch and not a minor

Stated rather than left implicit, because it is a judgement call. `reflow2_start_design` gains a refusal branch with new fields in its reply, and an output-shape change is normally minor. It is bucketed as a patch on this file's own wording — *"a behavior change is a **patch** when it only turns a swallowed failure into a loud one (a fix, not a new contract)"* — which is exactly what it does: starting a second design over an existing one used to succeed silently and now refuses. **The input surface did not move: all 149 toolsnaps match**, and every pre-existing reply shape is unchanged. Nothing here touches the schema, so the stamp is still 29 node types / 61 edge types / schema_version 1 and **no upgrade doc is owed**.

### Fixed

- **The latent surface notices a design arrived.** music_graph **F24**, the sibling of F23 — both fire on the run-book's own §0b restore walk. The server starts against an empty directory, says so truthfully, and then `--import` builds a full store underneath it seconds later. Nothing re-probes, so every design tool stays absent for the rest of the session and the export cannot be refreshed from the session that just did the restore. The sharp edge is the combination: the graph exists, the pre-commit hook is blocking commits, and **the only tool on offer is the one that starts a design** — against a directory that now has one.

  `reflow2_start_design` now re-probes before creating anything, using a read that opens no store and takes no lock, so it cannot mint an identity by looking. If a design is present it **refuses**: nothing is created, the design is named, and the reply gives the step that actually attaches the surface — a full client restart, not merely a reconnect. It also says plainly that the restore did **not** fail and must not be re-run, because the wrong recovery was the expensive one on offer. The handshake blurb now admits its own claim has a shelf life and names `--import` as how a design appears beneath it.

  **The reporter's preferred fix is not buildable and that is recorded rather than quietly substituted**: promoting the latent surface to the full one in place cannot work, because the tool router is fixed per service and clients cache the tool list. This makes the dead end *say what it is*; it does not remove it — after the refusal, the design tools remain absent until the client restarts.

- **The launch wrapper sees everything compiled in.** Development tooling only — **this script is not shipped in the kit, so consumers are unaffected**. Its content hash covered `crates/**/src/*.rs` and the manifests, but not the two trees compiled in from outside `src/`: `schema/*.yaml` (via `include_str!`) and `getting-started/skills/**/SKILL.md` (via the build script). A schema-only or skill-only edit therefore left the hash identical, the wrapper reported *"binary current, skipping build"*, and the server went on serving the previous vocabulary or the previous skill text.

  The framing that matters is sharper than "the hash was incomplete": **this hash is a gate in front of cargo's own change detection**, so what it misses, cargo is never asked about. The skills case is the clearest instance — `build.rs` already declared `cargo:rerun-if-changed` correctly, and the wrapper was defeating a mechanism that was right. Tests stay deliberately out: they are separate targets and cannot change the binary being served.

### Added

- Three findings and one open question recorded in the design graph: a PR export going stale behind a green, mergeable badge (measured on three of this repo's own PRs, each 56–63 nodes behind main); a property naming a node id being unguarded while edge endpoints are guarded across sixteen helpers; and an open question on how one finding should say it corroborates another, recorded as brainstorming rather than as a proposal.


## [0.32.0] — 2026-08-16

Fifteen merged changes, and the shape of the increment is that **every defect in it was found by
running reflow2 against a real project or by a first-time user — none by reading code, none by a
gate.** Four consumer projects reported in two days; verification against the running build changed
the answer often enough that "recorded as filed" stopped being acceptable practice.

### Fixed

- **A restored design no longer gains two properties nobody chose.** `Artifact.granularity` and
  `Artifact.volatility` declared `default:` in the schema, and `default:` is not documentation —
  dynograph-core executes it at **write** time, so importing an export materialised an intent the
  document never carried. Measured on music_graph's real committed export: restoring a design built
  before v0.24.0 made **all 35 of their Artifacts come back different**, moving the content hash and
  blocking the first commit on a machine where nobody had designed anything. The honest fix (refresh
  the export) was indistinguishable from the dishonest one (`--no-verify`) without diffing property
  by property first.

  **The fix is two YAML tokens and no code, because the safe reading already lived in the reader** —
  `coverage.rs` falls back to `atomic` and `drift.rs` to `stable`, and those are the only two
  readers. So the schema default changed no report's verdict and bought nothing. Measured both
  directions on their file: 35 nodes changed as shipped, **0** with the defaults removed.

  This reverses a deliberate BL-198 keep. That reasoning was right — `stable` *is* the safe reading —
  but storing it converted a fallback into an assertion. **Forward-only**: nothing already written
  moves, so existing exports do not churn.

- **A Question not created by `gap_to_prompt` was permanently unanswerable.** The lookup derived the
  Question id from the gap id by string formatting and did a single fetch, so a hand-authored
  Question could not be reached — while `open_questions` published its `question_id` and
  `answer_question` refused to accept one. The loop then reported *"follow it up rather than asking
  again"* about something it structurally could not close, and the next seat re-asks the user what
  they already ruled on. `answer_question` and `withdraw_question` now take **either** identifier,
  the derived id still wins when both could match, and the refusal names the ids that do exist.

- **`reflow2_check.py` stopped crashing on a directory Artifact** and stopped misfiling faults as
  gate failures — reflow2's own graph has zero directory artifacts, so its CI had never reached that
  line.

- **A retirement carried out stops being reported as drift** — `DriftKind::ExpectedAbsence`, so
  deliberate deletion is finally expressible rather than nagging forever.

- **The release proves the image starts before publishing it.** A flag removed in v0.27.0 was still
  being passed, so five releases shipped an image that never started.

### Added

- **The gate checks that a committed export can be read back.** `reflow2_check.py` gains a ROUND
  TRIP check — export → import → re-export, compared structurally. It is the only probe that sees an
  unrestorable design: `content_hash`, the lineage chain and `sync_status` all compare the export to
  *itself* and never run the importer. dev_storyflow's export had been unrestorable for four days
  with every one of those green. The `ci-gate` skill documents it, including the case where the
  cause is reflow2's rather than the reader's and re-exporting is **not** the fix.

- **An in-step record says how much of this graph it does not hold.** Every `sync` entry now carries
  `live_nodes` beside `export_nodes`. `sync` answers one question — has the shared record moved ahead
  of me — and answers it correctly; but nothing covered the *other* direction, and `in_step`, in a
  field called `sync`, was read as covering both. Reproduced on reflow2's own graph: `in_step` while
  two facts sat live and absent from the export. No state and no verdict changes — unexported work is
  still not `behind`, because that would tell a session to import over its own unsaved work.

- **`export_graph` says whether it wrote anything** — `wrote: created | changed | unchanged`. The
  hashes cannot answer it: an export that changed the file and one that changed nothing return the
  same `content_hash` *and* the same `prev_content_hash`, so a no-op was indistinguishable from a
  save.

- **`VERIFIES` admits `Constraint`** — a check may now say whether a limit holds. `Project` stays
  out, deliberately.

- **A refusal names what would have worked.** `authored_by` and `owned_by` distinguish a wrong
  *type* from a missing node and name `add_contributor`; the `adopt` skill says why the document
  wins.

- **A typed constructor that lands on an existing node says what it replaced** — a new
  `revision` block on `add_requirement`, `add_capability`, `add_component`, `add_interface`,
  `add_constraint` and `add_decision`. It carries the properties the call overwrote **with their
  prior values in full**, the properties it added, a `changed` flag, and a `prior_content_hash`.

  Constructors merge (BL-183) and `search_first` deliberately goes quiet on a revision, because a
  node's resemblance to itself is noise. **Nothing filled that silence**: a merge onto an existing
  id and a fresh create returned the same shape, with no signal that anything was replaced and no
  prior value. Reported **four times, by three agents, across two versions and three projects** —
  `add_constraint` twice on one id overwrote a multi-paragraph statement (*"could not honestly
  reconstruct it"*); a `record_change` snapshot taken after a sibling merge stored the NEW
  statement as the prior one (*"the timeline for that revision is a lie"*); an `accepted` Decision
  was widened from a debugging hypothesis and the user had to walk it back; and a malformed
  payload replaced a Decision's text while replying exactly like a create.

  **The prior value is echoed in full, not hashed.** A hash says something was lost; only the
  value puts it back, and being unrecoverable was the whole complaint. The `note` names the
  ordering that makes `record_change` honest — snapshot BEFORE the merge, because afterwards it
  files the replacement as the prior state.

  **It never refuses and never rolls back.** Re-calling a constructor to sharpen a node is what
  `revise-design` tells you to do; the merge is not the defect, the silence was. Reports, and the
  caller decides — `dec:three-party-checks`, the same posture as `search_first` beside it.

  Emitted on a revision even when **nothing changed** (`changed: false`), because "my merge was a
  no-op" and "my merge replaced a paragraph" are otherwise the same reply — the same ambiguity
  reported against `export_graph` in the same week.

  **The served surface is unchanged: all 149 toolsnaps match.** Toolsnaps pin input schemas, and
  this adds only a reply field, so nothing across the seam had to move. `sha2` is now a declared
  dependency of `reflow2-mcp` rather than an implied one (already in the lock via `reflow2-core`);
  zero new crates compile. Mutation-checked: reading the prior value from the node AFTER the write
  — precisely the reported defect — fails 4 of the 6 new checks.

### Changed

- **`change_type` stops answering two questions at once.** `ChangeSubject {system, record}` is added
  as an optional axis, and `defect_fix` joins the enum — "the design was right and the code was
  wrong" previously had no value, and five sessions across three projects each picked a *different*
  least-wrong one.

- **The no-documents rule gains its reasoning and its exception**, so the rule can be argued with
  rather than merely obeyed.

## [0.31.0] — 2026-08-15

### Added

- **`reflow2 update` — the second half of "install is one command, update is one word"**
  (`req:frictionless-update`, accepted 2026-07-20, its update half unbuilt until now).

  Anthony, 2026-08-14: *"do we have a 'reflow2 update' cli command yet? ... is there one to update
  a project graph? like cd to a reflow2 project repo, then run something ... to bring it up to
  current version installed on machine."*

  **Measured before building: the mechanism already existed and was unreachable by name.**
  `reflow2 init <dir>` has always refreshed a project's kit — reads the receipt, rewrites what
  moved, keeps your edits. Run against `dynograph-foundation` it reported *"installed from reflow2
  0.16.0 / now at reflow2 0.30.0"* and named 16 changes. **Nobody types `init` at a project that is
  already initialised**, so the capability sat behind a word that says the opposite.

  ```bash
  cd my-project
  reflow2 update --check     # what would change; writes nothing
  reflow2 update             # bring this project forward
  ```

  **IT REFUSES RATHER THAN QUIETLY DOING A FIRST INSTALL**, and the case that forced the guard is
  the ordinary one: a project can hold a *design* and no kit — that is exactly
  `dec:install-once-per-machine`, reflow2 registered machine-wide with the in-repo half never run.
  Updating there would perform a first install under a word promising to bring something forward,
  so it exits 1 and names `reflow2 init`. **Absence of a kit is not staleness.** The install
  receipt (`.reflow2/kit-version.json`) is the test, because it is the one honest signal that
  there is a prior install to carry.

  ⚠️ **TWO SURFACES, AND THIS COMMAND OWNS ONE.** `reflow2 update` is purely local and **never
  downloads anything** — it brings a project up to the reflow2 already on the machine. Updating
  reflow2 *itself* still means re-running the installer. The command *reports* when the binary is
  behind and refuses to do it for you; the help text and
  [UPDATING.md](getting-started/UPDATING.md) both say so, because conflating the two is the
  confusion `fact:updating-reflow2-does-not-update-a-project-already-set-up` records.

  4 tests in `tools/test_init.py` (100 in the file), pinning the **distinction** rather than the
  refresh — the refresh is `install` and every other case covers it; what would rot first is the
  two words meaning different things. Mutation-checked three ways: accepting any directory kills
  two, dropping the design-without-kit branch kills the one that names it, and dispatching
  `update` as a plain `init` kills the wrapper case.

- **The vocabulary now says where abandonment actually lives** — two descriptions, no schema move
  (the discoverability half of `dec:idea-does-a-capability-need-a-cancelled-state`; the mechanism
  itself was already decided at `dec:idea-discontinued-is-a-first-class-state`, 2026-08-11).

  Alex's fleet reported that agents *"stuff DROPPED into description text"* because `Capability`
  has no `cancelled` status. **The mechanism to record it already existed and worked** — an accepted
  Decision drawing `OBSOLETES` at the capability computes `discontinued: true`, which three gap
  detectors and delivery arithmetic already read. What did not exist was any way to **find** it.

  **Measured via `describe_schema`, which the served routing table tells an agent to use for exactly
  this lookup:** on `Capability`, `provenance`, `capability_type`, `inputs`, `outputs`,
  `is_entry_point` and `is_exit_point` all carried teaching descriptions and **`status` carried
  none** — four bare values and no fifth. The other half was as bare: `OBSOLETES`'s entire hint read
  *"Source makes target redundant or deprecated"*, naming neither the retirement job, nor the
  Decision as source, nor `discontinued`. **So an agent did everything right and still had nowhere
  to put the fact except prose.**

  `Capability.status` now says it records what was BUILT and only moves forward, that `cancelled`
  and `dropped` are absent **on purpose**, and where abandonment goes instead. `OBSOLETES` now says
  it is *the* retirement mechanism, that the **Decision** is the source end (a retirement edge
  normally presumes a successor; a discontinued thing has none, but always has a decision), that
  only an **accepted** decision discontinues anything, and that the target's own `status` must not
  be edited to say it — that would create the second source of truth this design avoids.

  **Not a schema change in the stamp's terms:** no new node type, edge type, property or enum value.
  `node_types` 29, `edge_types` 61, `schema_version` 1, all unmoved — so it reaches every consumer
  with no upgrade doc.

  **Pinned, not trusted.** 2 tests in `tests/discontinued_is_read.rs` (8 in the file) assert both
  descriptions carry the specific words a reader needs. Mutation-checked both ways: reverting
  `Capability.status` to the bare enum kills the first and nothing else; reverting `OBSOLETES` to its
  one-line hint kills the second and nothing else. A description nothing reads back is one edit from
  silently disappearing — the same reason the standing rule is pinned in every skill.

  ⚠️ **This fixes the abandoned case only.** A capability **nobody ever agreed to** has no decision
  to hang the edge from; that residual stays with `dec:exploratory-staging`, and
  `dec:idea-does-a-capability-need-a-cancelled-state` stays `proposed`.

- **Every served skill now has a slash command, and the coverage is pinned by a test**
  (`dec:idea-does-every-served-skill-get-a-command`, accepted 2026-08-14 on Anthony's word,
  option A; qualifies `dec:commands-are-the-exception`).

  **Found the way this project keeps finding things:** he typed `/capture-session` on a real work
  project and got `Unknown command`. Measured — 20 skills served, 11 command files, of which two
  front a tool (`debt.md` → `loop_status`, `decisions.md`), so **nine skills were fronted and
  eleven were reachable only if an agent thought to call `get_skill`.**

  ⚠️ **It was invisible from this repo, and that is the durable lesson.** `.claude/skills/` holds
  all twenty as the compile sources, and Claude Code loads any of them as `/<name>` — so every
  skill resolves *here* and eleven resolved nowhere on a consumer install. The question that
  authorised shipping commands asked for "the same ergonomics reflow2's own repo has", but the
  repo's ergonomics come from the **sources**, not the commands. The checkout does not merely fail
  to reproduce the defect; **it teaches the wrong affordance** — he learned the skill's name in the
  one place typing it works.

  **NEW COMMANDS (11):** `/capture-session` `/ci-gate` `/impact-check` `/ingest-corpus`
  `/link-artifacts` `/link-projects` `/parallel-work` `/plan-increments` `/report-friction`
  `/retire-from-design` `/revise-design`. Named after their skills, because that is what a person
  who knows the skill exists actually types. The existing short names (`/gaps` → detect-and-ask,
  `/where` → where-am-i) are unchanged and are what people type today — measured 2026-08-14,
  **37 `/where` invocations against 4 `get_skill` calls.**

  **THE DURABLE HALF IS THE CHECK.** `skill_lint` already failed a command naming a skill that does
  not exist; **nothing failed a skill that no command names**, and nothing pinned the count — which
  is how the records went on saying "eight" while eleven shipped, and how nine of twenty sat
  uncovered without one gate going red. `every served skill is named by at least one command` now
  closes that direction. Coverage means *named by* a command, not *same name as* one, so the short
  aliases still count. Mutation-checked: removing one command turns it red and names the skill.

  **WHY THE COST CHANGED.** The case against widening was Alex's *"inserted several things into my
  project"*. That was answered the same day by `init` asking which harness and writing
  `.claude/commands` for Claude Code **only** — an OpenCode or VS Code project receives none of
  them, so the projects that complained no longer pay for this.

  ⚠️ **KNOWN CONSEQUENCE FOR THE INSTRUMENT:** `surface_usage.py` counts skill use as `get_skill`
  calls, and a command never touches `get_skill`. Widening coverage will make apparent skill use
  *fall*. That is a measurement artefact, not a regression — see
  `fact:skill-use-measured-two-channels-2026-08-14`, which measures both channels (56% vs 87%).

### Changed

- **`REFLOW2.md` refreshed from the kit source**, which had drifted: the served file gained a
  section on the committed export (`docs/design/<project>.json` is the record teammates read,
  `.reflow2/` is machine state) and this repo's copy never received it. Surfaced by
  `check_kit_manifest.py`, which was reporting two findings on `main` before this change.

- **`decomposition_coverage` — the question the roll-up never asks**
  (`req:decomposition-covers-its-parent` → `cap:decomposition-coverage-is-asked`, accepted on
  Anthony's word; the check `dec:idea-a-top-level-graph-holds-what-the-component-graphs-share`
  named as the prerequisite for reopening the tier question).

  reflow2 rolls delivery **up** a decomposition and never checks that the children **cover** the
  parent. `report.rs` treats a parent as delivered exactly when every child is, so a requirement
  split into two children addressing a tenth of it reports `delivered` the moment both close —
  inside `req:completion-computed`, the number this project trusts *because* it is computed from
  the golden thread rather than asserted.

  The mechanism is general: **a decomposition by SUBJECT drops what belongs to no single
  subject**, because cross-cutting content has no natural child to land in. The instance is not
  hypothetical — reflow's monolithic `01-systems_engineering` was split into 01a–01f, and
  `context_management` and `self_improvement`, present in all six monolithic workflows, are absent
  from all seven children. Nothing noticed for months, because a roll-up only ever asks whether
  each child is done.

  **It asks and never answers.** No refusal, no LLM ruling on sufficiency, and no guess at what is
  missing — reflow2 can see *that* the question is unanswered and cannot know *what* fell between
  the children, and a plausible wrong guess is worse than the question because it gets recorded as
  the answer (`cap:no-fabricated-repair`). Held by a test, not by intent.

  Severity 0.50, rising to **0.70 once the parent already reports delivered**, where the risk has
  stopped being hypothetical. Per-parent rather than one project rollup, because "what did *this*
  parent hold that none of its children hold?" is answerable only about one parent. Keyed on the
  parent **and** its children, so changing the split re-asks — the recorded answer was about
  *those* children. Decomposition only, never derivation: a DERIVED requirement adds new technical
  necessity and is not expected to cover anything (`req:requirement-lineage`), and keying on the
  `DECOMPOSES` edge gets that for free.

  7 tests in `tests/detect.rs`, **5 mutations killed** — dropping the registration, flipping
  `is_aggregate` to `true`, anchoring only the parent, removing the delivered severity bump, and
  making the finding suggest what to add. Each died to exactly the test that names it.

  ⚠️ **The self-host cannot exercise this rule.** reflow2's own design has **zero** `DECOMPOSES`
  edges — all 164 requirements are lineage `original` — so this detector is silent here and will
  stay silent until reflow2 splits a requirement. `rule:the-self-host-always-trails-what-it-teaches`,
  and worth stating rather than discovering later from a green run.

  Served: `detect-and-ask` now names the gap in the list it tells an agent to expect (all three
  skill trees), and [docs/gap-surfacing.md](docs/gap-surfacing.md) carries the taxonomy row.

- **A collaborator's design work now survives clone → write → push → pull**
  (`req:the-git-exchange-path-is-defended-not-only-documented`; options **B and D** of
  `dec:idea-feedback-arrives-by-git-push-and-pull`, accepted on Anthony's word).

  He asked whether his brother could clone the repo, write feedback straight into the graph, push,
  and have it arrive on the next pull. **The answer was yes — that is the intended path**
  (`dec:multi-writer-architecture`: no server, the file is the transport). But four steps have to
  happen and **three fail silently**: import before writing, export before pushing, a per-clone
  merge driver, and import after pulling.

  - **The installer registers the merge driver** (`cap:the-design-record-merges-per-node-in-every-clone`).
    Git splits this deliberately: `.gitattributes` names the file and **travels**; the driver is
    `git config` and **cannot**, because git refuses to let a repository configure an executable.
    reflow2's own `.gitattributes` has carried `merge=reflow2` for months and **nothing ever set the
    driver** — so any fresh clone fell back to git's *line* merge on a multi-megabyte JSON graph,
    where two people who edited entirely different parts of the design still collide. Both halves
    are written now, and a driver somebody else set is **left alone and reported**, the same rule
    `write_mcp_config` and `ensure_hooks` already follow.

  - **The record having moved reaches an ordinary read** (`cap:the-record-moved-reaches-an-ordinary-read`).
    `loop_status` already computed `record_moved` correctly; `read_loop_hint` built its hint from the
    core `LoopStatus`, which has no sync knowledge — so the one **loud** step of the four was loud
    only inside a call nobody makes on the way past. Either debt alone now fires, because a design
    whose loop is otherwise clean can still have a record that moved under it.

  🛑 **Neither acts.** The hint names `import_graph` and stops; import is an upsert and an unasked
  one would silently overwrite live session work (`dec:ask-not-repair`). **Option C —
  import-on-first-use — was deliberately NOT taken**: every step here is silent-when-skipped and the
  instinct is to automate all of them, but automating an act because forgetting it is expensive is
  how a tool starts making design decisions on somebody's behalf.

  📌 And worth recording: **step 2 is the defect Alex reported the same morning.** His feedback route
  depended on the committable record existing, and before that fix his push would have succeeded and
  carried nothing.

## [0.30.0] — 2026-08-13

**Minor because of the new skill and the new gap source, not because of the fixes** — the rule is
the highest bucket present, and *"new capabilities or skills"* is minor. The schema stamp did
**not** move (29 node types, 61 edge types, schema 1), so no upgrade note is owed.

⭐ **The increment's lesson: every defect below was found by a person meeting the output for the
first time, or by running two real projects at each other — none by reading code and none by a
gate.** reflow2's own published boundary sat half-specified and no detector could say so; the
installer's one export landed inside its own ignore rule; and a gate that checks skills could not
see four files that serve tools. The three that a second user found were invisible to everyone who
already knew the system.

### Added

- **`link-projects` — the twentieth served skill, and the first for linking two SEPARATE projects**
  (`req:linking-two-projects-is-a-served-process`; **new skill → minor**).

  Anthony, after reflow2 and flo2 had been linked twice: *"are we tracking notes for a process that
  we can reuse when a user wants to link 2+ reflow2 projects"*. The honest answer was **no** — what
  was tracked was *findings*, not *process*. Nineteen served skills and none covered it:
  `link-artifacts` is files-to-capabilities inside one design, `parallel-work` is several people on
  **one** design. A user saying "link projectA and projectB" got an agent improvising, twice.

  `req:a-discipline-is-delivered-at-the-tool-not-in-a-catalogue` settles the form rather than
  straining against it: "link A and B" is a **task boundary the user states aloud**, which is the
  kind the served catalogue reaches well — unlike a discipline that applies mid-flow.

  Every step is evidenced by running it, not reasoned out:
  - **The user asserts linkability.** A house design and an accounting service do not link; that is
    a mistake in the asking, not a tool failure. The correspondence is an **input**.
  - **`pair_designs` is orientation only, and its correspondences are to be distrusted** — measured,
    it ranked a wrong match at 81 above the true seam at 64, and what prevented a false pair was the
    attribute key rather than the names.
  - **Assert the pair, then `seam_report`.** `unstated` is the punch list, and never agreement.
  - **Fill your own side with facts; leave an axis unstated rather than guess it; never write the
    other project's side.**
  - **Expect the seam to get *worse* before better.** Measured: filling one side moved it from
    3 agreed / 0 incompatible / 5 unstated to 4 / 1 / 3. The new incompatibility was true all along.

  It states what it cannot see (the types that cross a boundary, whether the projects should be
  linked at all, a segment mismatch versus a real incompatibility) and the honest limit that **no
  gap fires on an under-specified published boundary** — which is why it is worth running when
  nothing looks broken.

### Fixed

- **A fresh install had no shareable design record, and nothing said how to make one**
  (`fact:second-user-first-run-report-2026-08-13`).

  Alex, first run on a real work project: *"That .gitignore ignores .reflow2 folder. Where does it
  store the JSON file that can be committed for the graph? I can't find the artifact that can be
  shared by other project members."*

  **He was right, and the reason is narrower and worse than "it was undocumented".** The convention
  *was* written down — `getting-started/UPDATING.md` names `docs/design/<name>.json` as "the
  committed export — the durable record", and the `ci-gate` and `parallel-work` skills both use it.
  Two things defeated that:

  - **Nothing INSTALLED INTO THE PROJECT said it.** Reproduced on a scratch project: `AGENTS.md`,
    `CLAUDE.md` and the closing summary contain zero mentions of `export` or `docs/design`. The only
    trace was a parenthetical inside the `.gitignore` — *"the durable record is an export"* — which
    parses only if you already know what an export is. The real documentation lives in a file about
    *updating* and in skills that must be deliberately loaded; a first run reaches neither.
  - 🛑 **The one export init did produce went somewhere git cannot see.** `backup_graph` writes
    `.reflow2/backups/design-<stamp>.json` — and `.reflow2/` is the very directory the same
    installer ignores. **A user who went looking found an export that was invisible by
    construction.** That is the shape of the whole defect: the machinery was all there and its
    output landed inside the ignore rule.

  The knock-on: `ci-gate` was unreachable for a new project, because `reflow2_check.py` runs against
  a committed export that nothing had produced.

  Now three places say it, in the words of whoever is reading:
  - the `.gitignore` comment names `docs/design/<project>.json` and `export_graph` outright;
  - the installer's closing summary has a **"The record you SHARE"** block naming the path;
  - `POINTER.md` — the file the *agent* reads, which is what actually makes the export happen —
    gains a section on the record, including that one commit per PR should write it.

  **And `ensure_design_record` now WRITES THE FILE — always, including on a brand-new project.**
  Alex again, when the first version of this fix was described to him: *"it should just make the
  file and it should not be .gitignored. For some reason I thought it already created that file."*
  Two users independently expected it to exist, which is the strongest evidence available about
  where somebody looks.

  ⚠️ That first version only wrote the record when a graph already existed — silent on exactly the
  fresh project he was standing in. The reasoning was that exporting an empty graph mints an
  identity nobody asked for, the thing `describe_designs` refuses to do. **The analogy does not
  hold**: `describe_designs` inspects, and looking must not create; this is the installer, and
  running it is the moment the project adopts reflow2. It already writes `.reflow2/`, the MCP
  configs and an instruction file — minting the id is the smallest of those commitments, and an
  empty export is a real document (stamp, `graph_id`, `content_hash`, zero nodes) whose value is
  that the first real export **chains from it**.

  It never overwrites an existing record, and it is **never fatal**: a missing or unrunnable binary
  produces a line you can act on rather than a traceback over an otherwise-successful install.

- **The installer stopped reformatting `.claude/settings.local.json`** (same report).

  Alex: *"the init should not write over the .claude/settings.local.json if it already exists."*
  Measured: it never overwrote — `ensure_hooks` merges, preserves a `permissions` block byte-for-byte
  and keeps a hook you repointed. **But it re-serialised the whole document at a fixed 2-space
  indent**, so a compact 10-line file came back as 58 expanded lines and every line showed in the
  diff. **A merge you cannot distinguish from an overwrite is an overwrite as far as trust goes** —
  the standard `ensure_hooks`' own docstring sets for itself.

  It now keeps the indent the file already used (the whitespace itself, so a tab-indented file comes
  back tab-indented), and the summary says outright that the file was **merged and nothing removed**.
  Residual churn is documented rather than claimed away: `json.dumps` still normalises array layout,
  and format-preserving JSON editing needs a parser the stdlib does not have.

  ⚠️ **Not ruled out**: that Alex is on an older kit whose behaviour genuinely did lose content. His
  version was not established, and a consumer kit can sit releases behind — `ver:kit-manifest-agrees`
  exists because reflow2's own manifest was four releases stale and nothing noticed.

  8 regression tests added (`tools/test_init.py`, 62 → 70). One of them caught a gap in its own fix:
  the first `detected_indent` read only spaces, so a tab-indented file would still have been
  normalised.

- **`skill_lint` could not see four files that serve `#[tool]` methods** — found by a skill
  referencing `describe_designs` and being told the tool does not exist.

  It scanned `service.rs` plus `tools/*.rs`, while `latent.rs` (`describe_designs`, `reflow`),
  `degraded.rs`, `skills.rs` (`get_skill`, `list_skills`) and `main.rs` also declare tools. **The
  effect was backwards pressure on the allowlist**: a real tool looked unresolvable, so the fix
  would have been to add it to `NON_TOOL_TERMS`, which exists for terms that are *not* tools. The
  whole of `crates/reflow2-mcp/src/` is read now, so a tool added in a new module cannot go
  invisible by living in the wrong file. **Served tool count seen by the lint: 139 → 150.**

### Changed

- **The MCP surface is two boundaries, not one wrong `medium`** (`dec:the-mcp-surface-is-two-boundaries-not-one-medium`,
  accepted on Anthony's word; design record only, **no code, no schema move → patch**).

  `ifc:mcp-tools` had claimed `medium: REST` since before `unspecified` became the default
  (`req:seam-incompatibility`, 2026-07-28). The correction was blocked on a real vocabulary
  question — MCP is JSON-RPC 2.0 over **either** a stdio pipe **or** streamable HTTP, and no enum
  value names that — which was raised for him rather than guessed.

  ⭐ **The question dissolved rather than being answered.** The node's own `endpoint` field already
  said it: *"a stdio pipe to a launched process, **or** the address given to `--http`"*. One
  Interface was describing two boundaries, and they differ in more than medium — the HTTP one can be
  reached remotely, sit behind a gateway and terminate TLS; the stdio one can do none of those.

  - `ifc:mcp-tools` → **MCP tool surface (stdio transport)**, `medium: cli` — simply correct for a
    launched process spoken to on its stdin/stdout.
  - `ifc:mcp-tools-http` → **new**, `published`, provided by `cmp:service`, `medium: unspecified`.
    **Not an oversight**: the enum has no word for MCP over streamable HTTP, and `REST` is wrong —
    a single endpoint carrying JSON-RPC, not a resource tree.

  **No schema change**, which matters more than it looks: enum values are **not counted by the
  version stamp** (it counts node and edge *types*), so adding `json_rpc` would have been a
  compatibility event nothing can detect — the stamp-blindness pattern already recorded four times.

  🛑 **It quarantines the problem rather than solving it, and that was said before the choice.**
  `medium` is a pairing-key axis where `unspecified` reads as UNKNOWN and never as agreement, so the
  HTTP boundary cannot pair with anything until the vocabulary question is settled. `json_rpc`
  remains open and untaken.

  **Measured immediately after, and it vindicates the cut** — `seam_report` against flo2's
  `ifc:reflow2-design-api`, on both halves:

  | ours | agreed | incompatible | unstated |
  |---|---|---|---|
  | `ifc:mcp-tools-http` | 3 | **1** — `transport_security` none/tls | 4 |
  | `ifc:mcp-tools` (stdio) | 3 | **2** — also `medium` cli/REST | 3 |

  The stdio boundary now says loudly that it is **not** what flo2 means. And it sharpens the open
  TLS question: on a stdio pipe there is no transport to secure, so "segment artefact" was arguable;
  on plain HTTP it is not, which makes **a real assumption gap** the likelier reading. Resolving it
  needs flo2's side, deliberately not written to.

- **reflow2's own published MCP boundary is specified — and a second project had to ask** (design
  record + docs only; **no code, no schema move → patch**).

  `ifc:mcp-tools` is reflow2's `published` contract and carried `auth: unspecified` and
  `transport_security: unspecified`, plus three empty free-text axes. **Nothing inside reflow2 was
  ever going to say so**: `unprovided_interface` and `unconsumed_interface` read edges, not spec
  completeness, and no detector reports an under-specified published boundary. It took running
  `seam_report` against flo2 on an asserted pair — 3 agreed, 0 incompatible, **5 stated by nobody**,
  with flo2 having answered on the two that matter and reflow2 not. The report's own wording:
  *"we assume `unstated`, they require `none` — the side that assumed less finds out in production."*

  Every filled value is a fact about what is built, not an intention: `auth: none` (no
  authentication anywhere in `crates/`; `ifc:authenticating-gateway` is a *required* boundary whose
  name says so, and `--http-allow-host` is a Host allowlist, not auth), `transport_security: none`
  (no TLS anywhere; stdio pipe or plain HTTP), plus `endpoint`, `operations`, `error_model` and
  `payload_schema` read off the served surface.

  🛑 **Specifying honestly produced an incompatibility, and that is the point.** Re-running the seam:
  agreed 3→4, unstated 5→3, incompatible **0→1** — `transport_security`, ours `none` against flo2's
  `tls`. Two readings survive and the graph cannot choose: flo2 describing its own TLS-terminating
  edge (an artefact of asserting the pair across two segments), or flo2 expecting TLS *from* reflow2's
  HTTP surface (a real assumption gap). Either way it was invisible while reflow2 stayed silent.
  Resolving it needs flo2's side, which was deliberately not written to.

  `medium` is **left alone**: the node already records that `REST` is known-wrong and that the enum
  has no right value for MCP (JSON-RPC over stdio *or* streamable HTTP). That is a vocabulary
  decision, raised rather than guessed.

- **`interfaceless_dependency` is marked superseded in the gap taxonomy.** It was a planned gap
  source, never implemented under that key, describing the rule that shipped in v0.29.0+ as
  `undeclared_seam` — so the doc briefly carried two rows for one rule. The shipped rule is the
  stricter reading (an `Interface` must carry **both** a `PROVIDES` and a `CONSUMES`). The row is
  kept rather than deleted so anyone who read the taxonomy earlier finds where the rule went.

### Added

- **`undeclared_seam` — the coupling with no contract is now NAMED, in any project**
  (`req:an-undeclared-coupling-is-named-not-just-counted`; **new gap source → minor**).

  Anthony's reframing is the whole point of this landing as code rather than as labelling work:
  *"the step of adding seams for reflow2 needs to be something reflow2 does for ANY project."*
  The prior plan was to hand-label reflow2's own 73 couplings, which fixes one design and teaches
  reflow2 nothing. Every consumer arrives in the same state — parts that depend on each other with
  no contract recorded between them — and the tool should meet them there.

  **The mechanism already existed and was being discarded.** `maturity_report`'s `seams` band has
  always computed two sets — `couplings` (Component pairs joined by `DEPENDS_ON`) and `declared`
  (pairs joined by an `Interface` carrying **both** a `PROVIDES` and a `CONSUMES`, because one-sided
  is exactly the unrecorded contract the capture skill warns about) — divided one by the other, and
  dropped the difference on the floor. `couplings - declared` **is** the answer, computed on every
  maturity run, for every design, and never named. On reflow2's own graph that set has 73 members
  and the band reads 0%.

  Nothing else covered it: `unprovided_interface` and `unconsumed_interface` run the **opposite**
  direction and both require an `Interface` to exist already, so a design that has never declared
  one was invisible to both.

  - **It names the pair and asks. It never drafts the Interface.** reflow2 can see *that* two
    components are coupled and cannot know *what* the contract is — the medium, the payload, the
    auth, the direction. Proposing one would be `req:a-repair-suggestion-never-proposes-fabrication`
    exactly. A test asserts the finding stays interrogative and contains no invented contract
    vocabulary.
  - **One aggregate question, keyed on the rule**, not 73 nags — the BL-73 lesson, and the same
    flood `unexpected_coupling` was retired for. The acknowledgement therefore survives a new
    coupling (`req:set-scoped-acknowledgement-keys-on-its-rule`) while the count in the title moves.
  - **Silent when there is nothing to declare.** A design where no two Components depend on each
    other reports nothing, matching the band's own wording: *"an absence, not a deficiency."*
  - The computation is extracted to `DesignGraph::seam_sets`, shared by the band and the detector,
    so a detector and a band can never disagree about what a contract is.

## [0.29.0] — 2026-08-12

Two projects were pointed at each other for the first time, and everything below except the last
item was found by **running** that — not by reading code. A mirror could not be refreshed, a
published surface silently orphaned what it kept, and the export tool was inventing property
values nobody ever chose.

**No upgrade doc is owed.** The stamp is unmoved at 29 node types / 61 edge types,
`schema_version: 1` — update in place, no migration. The schema *did* gain one optional property
(`mirror_nodes` on Project); existing graphs are unaffected, and the fact that the stamp cannot
see a property change is a known blind spot, now on its fourth instance.

### Added

- **`capture-session`** — a skill you type, at any natural break or before a long session ends,
  to write down the reasoning that exists *only* in the conversation: what was tried and
  abandoned, what got measured, why the losing option lost. It does **not** ask an agent to rate
  the importance of its own session — that is the self-report this project distrusts everywhere
  else. Instead it gives six concrete tests under one question: *would a session six weeks from
  now redo this work, or repeat this mistake, because nobody wrote it down?* Served set is now 19.
- **`mirror_surface` can refresh** a design you already hold, rather than treating the second
  mirror as a stranger. `MirrorReport` gains `refreshed` and `withdrawn`. A refresh that would
  withdraw a boundary **you consume** is REFUSED and names both the node and your own edge into
  it — a partner retiring a contract you depend on is a conversation, not a cleanup.
- **`severed_containment` on `export_surface`** — a published surface already reported what it
  withheld; it now reports what the withholding did to what it **kept**. A `subsystem` component
  exposed for its published interface, whose parent was filtered out, arrives at the recipient
  looking like an orphan. This reports it rather than repairing it: carrying the ancestry would
  leak the internals the surface exists to withhold, and re-parenting to the Project would assert
  a `CONTAINS` nobody drew.
- **A registry that resolves `graph_id` → store path** (`reflow2-mcp`, internal). Groundwork for
  `cap:select-graph-by-id`: an id the registry does not hold is refused, and a filesystem path is
  refused as an unknown id rather than resolved — there is deliberately no path route in. **Not
  reachable yet**; no transport is wired to it and the single-`--graph-path` server is untouched.

### Fixed

- **`mirror_surface` collided with its own previous mirror.** Re-mirroring a design after the far
  side moved returned 12 of 13 ids as *collisions*, as if a stranger were squatting the
  namespace — one node updated out of thirteen. Worse than a plain failure: the stale nodes kept
  their old values, the host's edges still pointed at them, and `mirrors` still reported the
  **first** content hash, so a failed refresh left the staleness register reading *fresh*. The
  refusal was correct for a genuine cross-design clash and could not tell that case from this
  one. **Live since v0.12.0** — anyone who mirrored the same design twice hit it.
- **The export round trip injected 182 property values nobody chose.** `tools/export_via_binary.py`
  imported into a temp graph and exported back out, and the round trip *materialised* schema
  defaults — 91 artifacts silently gained `granularity` and `volatility`. The result is a record
  asserting intent that was never stated, and it shipped in a release. The tool now diffs what it
  wrote against its source and names every `(node_type, property)` pair the round trip invented,
  exiting non-zero.
- **A published surface's note** now says when something was orphaned, and names it, instead of
  leaving the fact in a field nobody prints.

### Changed

- **The unit of immutability is the thread, not the requirement** — recorded as design intent, no
  code yet. A satisfied requirement's text can currently move without a trace; what should be
  guarded is the whole SATISFIES → ALLOCATED_TO → REALIZES → VERIFIES chain, once it is confirmed
  operational.
- **The registry root is the tenant boundary** — a design that genuinely exists elsewhere on the
  machine is refused identically to one that was made up. Knowing a real `graph_id` is not a way
  in.
- **Fourteen delivered requirements got the owner's word**, moving off `proposed` on the record
  rather than by inference.
- **The installer stopped saying something alarming.** It announced *"Installing reflow2 for every
  project on this machine"*, which reads as though it is about to reach into every project
  directory and change it. It never was: nothing outside `~/.local` and `~/.claude` is touched,
  and a project only gains reflow2 when someone points it there. It now says **"Making reflow2
  globally available to projects on this machine"**, and `SETUP.md` matches. Reported by a user
  reading the output for the first time — the wording had been there since the machine-wide
  install shipped and nobody who already knew what it did had reason to notice.

## [0.28.0] — 2026-08-12

The surplus half of DETECT arrives, search-before-you-add stops depending on anyone reading a
skill, and a seat learns the shared record moved *when it orients* rather than when it writes.

**Derived from `changelog_view` between `rel:v0270` and `rel:v0280`** (57 drafted entries,
`unmapped: []`), then curated — Keep a Changelog is for humans. Every one of the increment's 26
ChangeEvents was pinned to its epoch first, because an unpinned event is silently dropped from
the derivation.

**No schema change.** The stamp is unmoved at 29 node types / 61 edge types, `schema_version: 1`,
so **no upgrade doc is owed and no migration is needed** — update in place.

### Added

- **`consumption_report`** — the surplus half of DETECT. Which built capabilities does the design
  record *no consumer* for? Reported as `nothing in this design consumes X`, never "unused":
  reflow2 reads a design and never a running system, so a feature real users call daily whose
  consumer nobody modelled is indistinguishable from a dead one. **Absence is only informative
  when presence is the habit** — below `MIN_MODELLED_RATIO` the list is withheld and the ratio
  itself is the finding. (Measured on reflow2's own design, the raw signal named 100 of 110.)
- **`sync_status`**, plus `record_moved` on `loop_status` — has the shared record moved since this
  seat last looked? `import_graph` already caught a graph up in one call; what was missing was
  anything telling you it was due. Silent unless somebody *else* has been there, so ordinary
  unexported work never fires it.
- **`what_next`** — which decisions to settle next, in four bands: `marked` (your own approver
  edge; no score reorders it), `ranked`, one deliberately `unexplored`, and `shaping` (the settled
  decisions a newcomer needs to read the rest). Scores are coarse on purpose and say so.
- **Capture registers the document it captured from**, so the backward half of the golden thread
  exists by construction rather than by anyone remembering.
- **The first outward-facing doc rendered from the graph** — `docs/impact-propagation.md`.

### Changed

- **`add_requirement` / `add_capability` / `add_component` / `add_decision` / `add_constraint` now
  REFUSE a near-duplicate** instead of quietly creating one. ⚠️ **The most likely thing to surprise
  an existing consumer.** The refusal names what it found and both routes out: call with the
  existing id to *sharpen* it, or pass `distinct_from` to create anyway. Short statements are never
  judged — there is no signal to judge on, and a young design is all short statements.
- **HEAL no longer proposes fabrication.** `disconnected_community` and `dead_end` stop offering
  `generate_bridge`, `orphan_node` stops offering `generate_owner`. `suggested_fix_type` is now
  optional, and where absent `repair_is_a_judgement` carries the sentence instead. Repairs that
  reorganise or restore what exists are untouched.
- **A refused enum names its legal values** rather than only rejecting.
- **An accepted Decision that `OBSOLETES` something now silences it** — a discontinued capability
  stops raising gaps, its requirement drops back to unsatisfied and is asked about again, and
  delivery reports `satisfied_only_by_discontinued` rather than shrinking quietly.
- **Ten schema properties stopped injecting defaults**, so absence now means nobody said. Existing
  nodes keep what they hold; the change is forward-only.
- `loop_status`'s verification roll is a digest — the full list stays on `graph_report`.

### Fixed

- **A replaced binary no longer strands the shared server.** The daemon spawn resolved its own
  executable through a path marked `(deleted)` after a rebuild, and failed with no usable message.
- **The stale-seat guard trusted the content hash a document states about itself.** A record edited
  by anything other than `export_graph` — a merge, a hand-fix — kept its old stamp, both fast paths
  read `Clear`, and the refusal went quiet in exactly the case it exists to catch: on the export
  path that silently deleted the other person's work. Now computed from content, with the
  discrepancy reported as `stamp_disagrees`.

### Build

- `[profile.dev] debug = "line-tables-only"` and `split-debuginfo = "unpacked"`. Measured: 313 MB →
  233 MB per test binary. Recorded honestly because the real cause of a 288 GB `target/` was that
  cargo has no GC — 1,824 artifacts under 104 names — and sweeping stale duplicates recovered
  225 GiB against these settings' 25%.

## [0.27.0] — 2026-08-09

Ownership becomes the third "who" axis, the content store is withdrawn, and the question of
whether governance belongs in reflow2 at all is answered: it does.

**Drafted from the measured surface** — `tools/toolsnaps/` goldens and the served-tool signatures
diffed against `v0.26.1` — rather than from `changelog_view`, because `[Unreleased]` was empty
across ten merged PRs and the toolsnap goldens are the ground truth for what a consumer can call.

⚠️ **UPGRADE DOC OWED AND WRITTEN: the schema stamp moved** — 61 edge types, up from 60, with
`OWNED_BY` added. Nothing in your graph is reinterpreted, but a new edge type means a new
capability your existing designs do not yet use. See
[docs/upgrading-to-v0.27.0.md](docs/upgrading-to-v0.27.0.md).

⚠️ **Four served tools are GONE.** If you call `content_put`, `content_get`, `content_exists` or
`content_manifest`, those calls now fail. See Removed.

### Added

- **`owned_by` — whose AREA a node is, durable and never released.** The third "who" axis, and the
  one that was missing. `AUTHORED_BY` says who *wrote* this (past tense, historical);
  `CLAIMS` says who is *in* it right now (transient, advisory, released at checkout); `OWNED_BY`
  says whose ground it is, and it survives every session. Deliberately **not** a traceability
  edge, so ownership never propagates a blast radius and never turns a Contributor into a hub.
  `loop_status(contributor_id)` reads it as `gaps_on_owned_ground`.
- **Most nodes legitimately have no owner, and that is not reported as a gap.** Whether unowned
  ground should be detected at all is left open rather than assumed.

### Changed

- **`loop_status` lists the decisions assigned to a named approver** instead of only counting
  them — you no longer have to read the export to find out which two they were.
- **reflow2 declares the interfaces it *requires*, not only the ones it provides**, so `seam_report`
  ran for the first time and `dead_surface` fell from 15 to 1. `unprovided_interface` no longer
  fires on a `required` interface, which is that interface's definition rather than a gap.
- **`get_node` refuses an unknown node type** rather than answering with a confident nothing.
- **`set_verification_status` no longer destroys `last_run_at` when the optional parameter is
  omitted** — consumer-reported, and the direction of the bug was the dangerous one.
- **The `revise-design` skill no longer claims `create_node` replaces.** It **merges** — the served
  tool upserts — and the old warning was scaring agents away from a safe call. Two served surfaces
  disagreed about whether a write destroys data; the tool description was the correct one.

### Removed

- **The content store: `content_put`, `content_get`, `content_exists`, `content_manifest`.**
  Built, shipped, correct, and used **zero times** across three projects in the retained sample.
  Git is already content-addressed and the repo already holds what the design produced, so the
  store was a second answer to a question that had one. Two accepted requirements are now
  deliberately **unsatisfied** rather than quietly re-scoped — the design says so out loud.
  `ingest_step` and `ingest_corpus_step` shared that source file and are **unaffected**; they moved
  to `ingest_tools.rs` and their schemas are byte-identical.

### Fixed

- **Three consumer-reported defects where a served surface destroyed or withheld data**, landed
  from PRs a bad merge had stranded.
- **`loop_status` no longer reports a zero that cannot be told from an absence** in two more places.

Dependency bump only: **rmcp 3.0.1 → 3.1.2** (and `rmcp-macros` with it). Cut as its own tag
rather than folded into v0.26.0, so that if the protocol layer misbehaves in the field there is
exactly one candidate cause.

**What you do: nothing.** No schema change, no stamp movement, and the served tool surface is
byte-identical — all 149 tool schemas match their goldens.

### Changed

- **rmcp 3.0.1 → 3.1.2.** Verified against the instrument that exists because the suite is not
  enough here: `stateless_seat_probe` reports one client → one seat on **all three** transports
  (stdio, legacy HTTP 2025-06-18, stateless HTTP 2026-07-28), with a seatless claim refused
  exactly where the session cannot supply one. That probe exists because the workspace suite
  stayed green through the rmcp v2→v3 upgrade *while seat identity was already broken*.
- The risk worth naming, and it did not materialise: `ProtocolVersion::LATEST` is still
  `2025-11-25`. Had it crossed the 2026-07-28 threshold, the sessionless path would have become
  the DEFAULT and `mint_seat` would have stopped being advisory for every claiming client — the
  tripwire test in `service.rs` pins this rather than leaving it to inspection.
- `darling` 0.24.0 joins the build graph via `rmcp-macros`, alongside the 0.23.0 that already
  arrived through `tantivy`. Both are **proc-macro** dependencies: build-time only, nothing added
  to the shipped binary.

## [0.26.0] — 2026-08-09

Governance becomes a question the graph can be asked, and the server stops lying about its own
currency. Drafted from `changelog_view` between `rel:v0250` and `rel:v0260` (60 entries,
`unmapped: []`) and curated by hand — the graph holds what moved, never what it costs you.

**No upgrade doc: the schema stamp did not move** (29 node types, 60 edge types,
`schema_version: 1`, unchanged from v0.25.0). Nothing in your graph is reinterpreted and no
migration is needed. One default changed, though, and it is worth thirty seconds of your time —
see the first Changed entry.

### Added

- **`served_by.stale` — the server now tells you whether it is still the code it was started
  from.** Derived from `/proc/self/exe`, which the kernel marks `(deleted)` when a running
  binary is replaced: one syscall, no version comparison, and it works when two builds share a
  version. Three-valued — `true` / `false` / `null` for *unknown*, and **unknown never reads as
  current**. `stale_note` carries the remedy, including the part that is easy to get wrong: a
  session restart alone changes nothing under `--shared`, because the client re-attaches to the
  same daemon. **What you do:** read `stale` before trusting any rollup from a long-running
  server; if it is `true`, the numbers came from code that is no longer on disk (your graph
  writes are unaffected).
- **`governance-proposal` skill and the `/rules` command** — the capture half of governance.
  It notices a rule you state in passing and asks what breaking it should COST, rather than
  deciding for you. **What you do:** nothing, unless you record design rules; if you do, invoke
  it (or `/rules`) and answer one question per rule.
- **Two governance gaps.** `unverified_enforced_rule` (0.60) asks what detects a violation of a
  rule that claims to be gate-blocking; `unstated_rule_enforcement` (0.40) asks an unstated rule
  which it is. **What you do:** expect new gaps if you have `DesignRule`s — see below for why
  they may be answerable with one word.
- **`build_without_governance` (0.45)** — a project with real artifacts and no recorded
  conventions is asked once, and it is acknowledgeable if the honest answer is "none worth
  stating". Keyed on artifacts, not components, so it never fires on a design still on paper.

### Changed

- **⭐ `DesignRule.enforced` no longer defaults to `true`. It is now three-state: `true`,
  `false`, and ABSENT meaning nobody has said.** Previously a convention recorded in passing
  asserted it could fail your build — and then owed a detector nobody agreed to. **What you do:
  nothing is reinterpreted; every stored value stays exactly as it is.** But rules that reached
  `enforced: true` by silence will now raise `unverified_enforced_rule`, and the honest fix is
  usually one call setting `enforced: false` on the ones you meant as guidance. reflow2 did this
  to its own four rules and three of them were advisory.
- **`VERIFIES` and `CONSTRAINS` enumerate their endpoints** instead of accepting anything through
  a `*` wildcard. `VERIFIES` now models `DesignRule`, which is what made "which of my rules have
  no detector?" askable at all; `CONSTRAINS` names its source as `Constraint`/`DesignRule`, so a
  rule binding a component is a modelled fit rather than a tolerated one. **What you do:**
  nothing — both lists were derived from live edges, so no existing edge is invalidated.
  `GOVERNED_BY` was deliberately NOT narrowed, because another project may legitimately use pairs
  this one does not; only its hint was corrected.
- **`graph_report`'s description** stops telling you to compare `served_by` against your repo by
  hand, and stops prescribing a session restart.

### Fixed

- **⭐ Seat liveness now answers about the session for the seat you actually carry.** v0.25.0's
  fix covered the seat the service leases — which is used only when you OMIT `seat`, and
  `dec:stateless-seat-handle` refuses that on the sessionless transport. A seat from `mint_seat`
  was never registered, so `gone` stayed unreachable and every claim read `live` forever. It now
  reads `unknown` on a server that cannot observe your session, and `mint_seat` stopped promising
  liveness it cannot compute. Reported by dev_storyflow (w-aa0607ff). **What you do:** if you
  rely on `claim_report` to tell you a colleague has left, note that `unknown` means *cannot
  see* — `release_claim` is what clears a claim on a shared server.

## [0.25.0] — 2026-08-08

### Fixed

- **⭐⭐ A merge proposal now says what it would DESTROY, before anyone can apply it — and a
  proposal that deletes a node stops reporting itself as needing no review.** `HealProposal` gains
  `would_destroy`: one entry per merge, naming the doomed node, the properties that die with it,
  and — when both nodes carry the same provenance — that the survivor was picked by the **alphabet**
  rather than by anything in the design. `requires_human_review` now also fires on any proposal
  containing a merge.

  Two defects, and the second read as a feature. `requires_human_review` was
  `!generated_content.is_empty()`, so a proposal whose *entire content* was irreversible node
  deletions reported `false` at confidence `0.9` — the machinery behind the served `check-health`
  skill calling merges the safe mechanical half. And `discarded`, which has always said what a
  merge let go, lived on the *report*: the receipt of an irreversible act, when the person deciding
  reads the proposal. `cap:heal` SATISFIES `req:no-silent-fallback` and this failed it.

  Reported by dev_storyflow, whose fleet stood itself down from the entire HEAL surface. Two of
  this repo's own tests had asserted the defect as intended behaviour — at unit level
  (*"a structural-only proposal needs no human review"*) and end-to-end (*"the only remaining
  operation is a content-free merge"*). A merge **is** content-free; it generates nothing. It also
  deletes a node.

- **⭐ A duplicate finding says when its node is a HUB, so five findings stop reading as five
  judgements.** `HealIssue` gains `hubs`: for each node also appearing in other findings of the
  same category, the node and how many. dev_storyflow's scoped `detect_defects` returned
  `in_scope: 5`, all duplicates — one Decision was in three of the pairs and one Requirement in the
  other two. **Five findings were two nodes.** Mid-stand-down, the count read as independent
  corroboration. Scoped per category so a well-connected node does not become a permanent warning.

- **A refusal names the call that fixes it.** `claim_region` on an unknown Contributor was a bare
  `NodeNotFound` — true and unactionable. A dev_storyflow worker hit it, concluded the tool was
  broken, and wrote *"there is NO `mint_seat` tool in the served surface"* into fleet onboarding;
  that false correction travelled **five hops**. It now names `add_contributor` and echoes the id
  you passed. It still refuses — naming the fix does not become doing it.

- **`add_contributor` accepts `contributor_id`**, the name `claim_region` uses for the same handle
  one step later. `deny_unknown_fields` is unchanged, so a genuine typo is still refused, and the
  alias stays out of the advertised schema.

- **The installer says when git is already tracking your graph store.** `.gitignore` never untracks,
  so a `.reflow2/` committed before the ignore rule stays committed — and the installer's warning
  for exactly this skipped every entry ending in `/`, which is only ever `.reflow2/`. Both the
  install path and **`--check`** were silent. The remedy it prints now also runs (`git rm -r
  --cached` for a directory). Reported by an early adopter whose project was installed at 0.11.0.

- **A schema hint stops offering values its own enum rejects.** `CAUSES.validation_status`
  described *"(observational / intervention / mechanism / temporal)"* against an enum of
  `unvalidated / hypothesis / validated / refuted` — no overlap. `describe_schema` serves that prose
  verbatim to every agent, so it was a wrong instruction at scale.


### Added

- **⭐ `getting-started/UPDATING.md` — how to update reflow2 without losing your design.** Ships in
  the consumer kit, not this repo's `docs/`, because the people who need it will never read this
  repository.

  Covers both deployment shapes: a locally-installed binary (replace it, restart the session, the
  store survives and the version stamp tells you what it found) and a container (replacing the
  image touches nothing, because none of the design is in the image — tested, not asserted).

  ⚠️ **It leads with the mistake that looks like data loss:** mounting `.../graph` instead of its
  parent leaves the identity sidecar behind, and a store opened without the identity it was created
  with **presents as an empty design while reporting no error** — the data still on disk beside it.

  It also states plainly that **reflow2 does not back your design up and will not**
  (`dec:backup-belongs-to-the-consumer`, proposed): backup is a property of *where the data lives*
  — your volume, your retention, your compliance — and reflow2 knows none of it. What it gives you
  instead is named: `export_graph` as a complete deterministic content-hashed snapshot, `--import`
  as a restore that preserves `graph_id`, and — for the repo-file model — git already being an
  off-host backup for free. The hosted case is the one that needs real work, and it belongs to
  whoever runs the server.

  Known gap, stated in the doc rather than hidden: **downgrading is not checked.** An older reflow2
  opening a store a newer one wrote has no verdict and no warning today.

- **⭐ A dependency declaration can name the dependency's own design.** `declare_dependency` gains
  an optional `graph_id`, carried through `reflow2.toml`. **Minor** — a tool-surface shape change.

  Two facts already sat side by side in that file and never touched: *"my build pins v0.12.0 of
  this"* and *"this is also a design I can compose with"*. Linking them makes a composition target
  **derivable from a committed, version-pinned manifest** instead of configured per machine — and
  it keeps the **direction** the dependency edge already carries, which a flat list of graph ids
  cannot express. Raised by @ajs: reflow2 declares dynograph-foundation as an external dependency
  *and* dynograph-foundation is itself a reflow2 project, and nothing joined the two.

  ⚠️ **Optional, and absent means "nobody has said" — never "there is no design".** Most
  dependencies never will have one (serde, tokio, rocksdb), so an unlinked dependency is the
  ordinary case and must not read as a defect. Stored only when stated, emitted only when stored,
  and a blank string is treated as unstated rather than as a design whose id happens to be blank.
  `ver:dependency-names-its-design`, passing.

  Not yet applied to reflow2's own declaration: `declare_dependency` is the right tool for that and
  the running server predates this build, so it lands after a restart rather than by writing the
  raw property behind the tool's back.

### Fixed

- **The dependency check was inert on Python 3.10 — most CI in the world.** `reflow2_check.py`
  read pins with `tomllib`, which arrives in 3.11. Its very first CI run on reflow2's own repo
  reported *"could not read any build file to check them"*, because the runner is `ubuntu-22.04`.
  Correct locally, doing nothing where it mattered.

  Now falls back to a narrow line reader when `tomllib` is absent — **byte-identical results to
  `tomllib`** on reflow2's own three Cargo manifests, and it **names every shape it cannot read**
  (`'x' spans lines, not read`, `[dependencies.y] sub-table form not read`) rather than returning a
  short list that would pass for a complete one. No `tomli`, no pip: the kit installs into other
  people's projects and a runtime dependency is a cost they did not agree to.

  Worth noting what worked: the honest-silence path built in the previous change is what *reported*
  this on the first run instead of passing quietly. `ver:toml-fallback-reader`, passing.

### Added

- **⭐ New skill: `plan-increments`** — planning delivery in increments and steps. Sixteen skills
  covered capture, detection, revision, health and collaboration; **none covered the temporal
  axis**, which has one of the richest tool surfaces in the product (`plan_epoch`, `add_epoch`,
  `set_epoch_status`, `schedule_for`, `arrival_delta`, `forecast_readiness`, `add_release`,
  `release_includes`, `precedes`, `gate_on`, `readiness_report`, …). Every consumer planning
  increments was reconstructing the practice from tool descriptions.

  **It carries the four conventions nobody guesses:** `plan_epoch` (has not happened) vs
  `add_epoch` (has); `SCHEDULED_FOR` = *due at* vs `AT_EPOCH` = *belongs to*, separate because one
  edge for both would be indistinguishable to every detector; `modality: expected` (a plan) vs
  `required` (an obligation whose miss is a computed violation — a KPP with a deadline); and the
  **deliberate absence of `achieved`**, because delivery is computed by `arrival_delta` and never
  asserted — *a plan recording its own success is the plan lying about itself*.

  **It also names a failure the other skills do not: a delivery plan kept outside the graph** — a
  numbered list in a conversation, a roadmap in a README, "next up" in a commit message. Raised by
  @ajs pointing at the assistant's own conversational queue. That is the third instance of the
  shadow-list class in one session, and the first where the agent was the one keeping the list; a
  list in a session is worse than a stale document, because it vanishes at session end and the next
  agent rebuilds it, losing whatever ordering the user had already reasoned through.

  **Its first exercise was reflow2's own plan, and the plan did not survive it**
  (`ver:plan-increments`, passing): 24 Releases and 72 DesignEpochs against **eleven**
  `SCHEDULED_FOR` edges, all pointing at retired releases, and nothing scheduled into any of the
  five planned future increments. `epoch:v0240-planned` was still `planned` while `rel:v0240` was
  `deployed` — now closed. And `arrival_delta` on it answered `items: []`, `required_count: 0`,
  `ready_to_cut: false` **for an increment that had already shipped**, because that plan lived in
  the epoch's *name* as prose and never as structure.

- **⭐ The shipped build gate now checks your dependency pins, not just your files.**
  `reflow2_check.py` — the gate that goes to every consumer in the kit — reconciles **declared
  dependencies** against what the build actually resolves, alongside the artifact reconcile it
  already did.

  **Why this mattered:** `req:design-dependencies-declared` has been accepted, its capability
  built, and `ver:design-dependencies-declared` passing — and **nothing a consumer ever ran
  invoked any of it.** A declaration nobody verifies is a promise, not a check. So in any project
  using reflow2, a dependency the build took that nothing declared ("the reliance nobody agreed
  to") and a declaration the build had moved past (a stale promise) were both unreported, forever.

  **Observations are keyed by SOURCE, not by name.** A design declares one dependency
  (`dynograph-foundation`) while the build names five crates from it. Name-matching would report
  four false `undeclared` findings and one backwards `unobserved`; grouping by the git URL both
  sides share reports the one true comparison.

  **Honest silence where it cannot look.** Only Cargo is read today. A project pinning in
  `package.json`, `pyproject.toml`, `go.mod` or `versions.env` is told *"N declared, and this gate
  could not read any build file to check them — NOTHING VERIFIED THE PINS"*, rather than passing
  quietly. It deliberately does **not** reconcile against an empty observation set, which would
  fire `unobserved` on every declaration and fail projects for having declared anything at all —
  punishing the correct behaviour is worse than the silence it replaces.

  Verified positively and negatively (`ver:check-reconciles-dependencies`, passing): the real pin
  reports as agreeing, and a deliberately-wrong tag failed the build with
  `version_mismatch — declared v0.12.0 but the build resolves v0.99.0`.

- **⭐ reflow2 ships as a container image.** A `Dockerfile`, a `container` job in `release.yml`
  publishing a version-tagged image to the registry, and `docker/build.sh` for a local build.
  Closes `req:reflow2-consumable-as-an-image`; a consumer runs reflow2 with no Rust toolchain and
  pins a version rather than chasing `latest`.

  **The binary is copied in, not rebuilt.** The `container` job wraps the artifact the `binaries`
  job already produced. That avoids a second ~14-minute RocksDB compile, and — the part that
  matters — guarantees **the image ships the exact binary the release ships**, rather than a
  separately-compiled twin that could differ with nothing reporting it.

  **State lives on `/data`, never in the image** (`req:hosted-state-outlives-the-image`):
  `/data/graphs/<design>/graph` for stores, sidecars **beside** them, `/data/content` for blobs.
  `--content-path` is set explicitly because its default points inside a consumer's *repo*, and a
  hosted server has none. Runs as uid 1000 so `chown -R 1000:1000 <volume>` is a complete
  instruction.

  **Confirmed in code rather than inherited:** the 120-minute `--idle-timeout` expiry is armed
  only inside `serve_http`'s `Some(SharedServer{..})` branch. The image runs plain `--http`, which
  passes `None`, so **it cannot exit on idle** — a property of the code, not of a flag someone
  must remember. Likewise the graph is opened *before* the socket binds, so the healthcheck's port
  probe cannot report ready during a cold full-text index build.

  **Verified by running it** (`ver:container-image`, passing): builds on both bases, reaches
  `Up (healthy)`, runs as uid 1000, produces the designed volume layout, and — the acceptance test
  from the requirement — **a design survives replacing the container**, `graph_id` byte-identical
  with zero re-mint warnings. Two defects were found this way and fixed: a binary built on a newer
  host dies at container start with `GLIBC_2.38 not found` (the base is now an `ARG`, and
  `docker/build.sh` pre-flights the binary against it rather than shipping an image that builds
  green and dies), and `useradd --uid 1000` exits 4 on `ubuntu:24.04`, which already ships a user
  there (now created only if absent).

  ⚠️ **No authentication.** `--http-allow-host` is DNS-rebinding protection, not auth. The image
  must sit on a private network behind a gateway that authenticates; this is stated in the image's
  own labels and in the Dockerfile.

  Not yet exercised: the publish path itself (registry login, push, pull-back verification) needs
  a version tag to run, and `cmp:packaging` correctly reports as *built but shipping in nothing*
  until a release includes it.

- **⭐ `loop_status` reports an open decision somebody was ASKED to settle.** New counter
  `unsettled_assigned_decisions`: a `Decision` left `proposed` while carrying an `AUTHORED_BY`
  edge with `role=approver`. It also names itself in `next` and in the read-side loop hint, so it
  surfaces on an ordinary orientation read rather than only when someone thinks to ask.

  **The approver edge is the discriminator, and that is the whole design.** A `proposed` Decision
  with *no* approver is somebody thinking out loud — the **brainstorm** skill records musings
  exactly that way — and it stays silent, so thinking out loud still costs nothing. Recording
  *whose* idea it is (`role=author`) does not make it owed either; only being asked does. On
  reflow2's own design that means **49 proposed decisions and exactly one reported**.

  Closes `req:an-assigned-open-decision-is-reported` and is the first cut of
  `cap:owed-to-a-contributor` (now `in_progress` — the contributor *scope* on `loop_status`,
  the other half of `req:the-loop-says-what-is-owed-to-a-person`, is not built).

  **Why it was worth building first:** the defect had three independent witnesses. flo2 measured
  it on its own design (eight open decisions, `detect_gaps` returning nothing about any of them,
  `loop_status` with no field that could); it was then reproduced deliberately here while
  capturing flo2's proposal (11 gaps, **zero** referencing the assigned decision); and
  [BL-215]'s hxm_program field report hit the same shape from a different project — *"every
  captured decision was ALSO hand-copied into markdown for teammates"*. A design brain that holds
  the open decisions but cannot report them pushes its users into keeping a shadow list, which is
  the precise failure reflow2 exists to prevent.

  `undecided_decision_point` does not cover this and should not be stretched to: it reasonably
  wants two or more **registered alternatives**, each with a design export behind it, and a
  decision whose options are prose has none — making it fire would mean inventing file paths that
  do not exist.

- **⭐ `describe_designs` — say what design lives at a path, without opening it.** The discovery
  half of `req:a-session-chooses-its-design` (accepted), built from a field report: a session
  opened at a repo root was told *"this directory has no design yet"* and started a **third**
  design while two populated ones sat one and two directories below. Nothing could say what they
  were.
  - **You walk the tree; reflow2 says what each one is.** Finding `.reflow2` directories is file
    navigation and belongs to the agent (`dec:agent-navigates-content`); naming the design at a
    path is the half only reflow2 can answer. Takes a **list** of paths — the caller is building
    a menu, and one round trip per candidate is the wrong shape for that.
  - **Served on the LATENT surface too**, deliberately. A session with no design is the exact
    moment someone is about to create one; a discovery tool reachable only *after* a design
    exists would arrive too late to prevent anything.
  - **⭐ It never opens the store, and that is the design rather than an optimisation.**
    `open_rocksdb_with_provenance` writes `<path>.meta.json` and **mints `<path>.id.json` when the
    store has none** — so a describe that opened the store would *name a design by the act of
    inspecting it*, and would fail on any design another session is holding. Everything reported
    comes from the two sidecar files beside the store, so it is lock-free, side-effect-free, and
    works fine against a design being written right now.
  - **Two deviations from the accepted decision, both forced by the code and both better than what
    was asked for.** *No node counts* — counting means opening, and the decision's "node counts by
    type" is not worth minting an identity to get. *No `busy` state* — the decision required
    distinguishing busy from absent because a held store is exclusively locked; reading sidecars
    removes the condition entirely rather than handling it.
  - **Something-unnamed never reads as nothing-here.** Four states — `design`, `unnamed`,
    `opted_in`, `absent` — because *"no design here"* is the sentence that starts an unwanted one.
    Every row carries a plain-language `reading`, since a bare enum makes the caller invent the
    meaning.

### Changed

- **`reflow2_start_design` now tells you to look before you start.** Its description previously
  ended *"it is safe to call when unsure"* with no mention of checking nearby — the sentence
  behind the accident above. It now requires a sweep of neighbouring `.reflow2` directories first
  and says why, and its payload tells a caller who skipped the check to **say so immediately**,
  while the recovery is still a deletion rather than a merge. Shipping the tool without the
  instruction that makes anyone call it would have been this project's fifth
  skill-disagrees-with-surface defect (BL-152, BL-178, BL-197, BL-204).

## [0.24.0] — 2026-08-05

**The document corpus can actually be ingested — and pointing it at someone else's codebase
immediately found a silent data-loss bug eleven releases of green self-host could not see.**

This increment set out to unblock a user who stopped at 26 of ~756 documents. It ends with the
folder driver built, trialled on a real corpus, and the defect that trial exposed fixed. The
sequence is the point: **the corpus ingest was green on every gate before the trial ran.**

**Minor, not patch** — `CorpusReport` changed shape and the tool surface grew by one. **No schema
change: the stamp is unmoved at 29 node types / 60 edge types, so no upgrade doc is owed.**

### Fixed

- **⭐ Sibling components are no longer silently merged away** (BL-213) — found by the FIRST
  REAL TRIAL of `cap:corpus-ingest`, on six dev_storyflow architecture documents. **This is a
  silent data-loss fix and it predates the corpus layer**: `ingest` has done it to every
  single-document run since fuzzy dedup landed.
  - **What went wrong.** A similarity score says two names are *alike*; it does not say they
    are *the same thing*. For identifier-shaped names the two come apart, and measured
    against `dynograph_resolution::token_sort_ratio` they come apart **inverted**:

    ```
    95  dynograph-vector  vs dynograph-core          merged, WRONG
    94  dynograph-storage vs dynograph-core          merged, WRONG
    84  Auth Service      vs Authentication Service  not merged, WRONG
    ```

    One document declaring **nine** crates produced **five** nodes — 44% of an architecture
    gone — and the survivors asserted something *false* rather than merely being incomplete:
    `cmp:dynograph-core` carried the name `dynograph-storage`.
  - **Why no threshold fixes it.** 95 was a sibling pair and 84 a true duplicate, so the
    ordering itself is wrong and no cutoff separates them. `docs/scope-corpus-ingest.md`
    asserted *"90 on a token-sorted ratio is near-identity"*; measured, it is not.
  - **The fix is a discriminator, not a number.** Before auto-merging, the names are
    tokenised on every non-alphanumeric and compared: a merge is refused when either side
    carries a whole word the other does not *abbreviate*. `core` and `storage` are not
    spellings of each other; `auth` and `authentication` are. An unpaired extra word is
    distinguishing too, so `Auth Service` and `Auth Service v2` stay apart — the case the
    scope doc warned collapsing would lose. Reordered tokens still converge, so BL-186's
    ordering fix is untouched.
  - **A refusal now says which word did it** — `distinguished_by` on the merge candidate,
    e.g. *"storage has no counterpart in dynograph-core"*. "These were not merged" is not
    actionable; naming the word is.
  - **Why it stayed invisible:** reflow2's own design uses prose names, not prefixed
    identifiers. Only a codebase corpus produces `foo-bar` siblings in bulk, so self-host was
    structurally incapable of surfacing it — **BL-199's shape, a second time**.

- **The corpus report says WHAT merged, not how many times** (BL-213). `CorpusReport.fuzzy_merges`
  was a `usize` for one day. A merge is the one thing an ingest does *without asking* and
  cannot undo by re-running — the losing node never exists — and the single-document report
  has always carried the full list, calling it *"auditable, never silent"*. The aggregate
  threw that away at exactly the scale where it matters most: the trial reported
  `fuzzy_merges=5` and finding that four had destroyed distinct crates took a hand-dump of
  the graph. Each entry now carries the document that caused it, which the single-document
  report cannot know.

### Added

- **A whole folder of documents becomes one design** (BL-186, `cap:corpus-ingest` — the last
  build row of this increment). New tool **`ingest_corpus_step`**, the corpus sibling of
  `ingest_step`, plus a served **`ingest-corpus`** skill for the half reflow2 cannot do: walking
  the directory. **No schema change; purely additive to the tool surface** (148 toolsnaps, 1
  added, 0 changed, 0 removed).
  - **One epoch for the whole run.** Left alone, `ingest` opens `epoch:{fragment_id}` per
    document, so 500 files landed as 500 unrelated events on the time axis instead of one ingest.
  - **Identity converges across documents.** The same component named in forty specs resolves to
    one node while each document keeps its own provenance `Fragment` — which is the difference
    between one design and forty disconnected ones. The report says so as `fuzzy_merges`, and
    zero across a large corpus is a red flag rather than a clean run.
  - **The ambiguous band is asked ONCE.** Near-matches are gathered across the whole corpus and
    deduplicated, because `dec:ask-not-repair` at corpus scale means the asking must be batched
    or the feature is unusable — the same pair surfacing in six documents is one question.
  - **⭐ The handshake batches, so a corpus costs the rounds a document does.** Prompts for every
    document are gathered into one round: ~3 rounds for a hundred documents instead of ~300.
    This needed **no new mechanism** — a prompt's id was already a hash of its semantic content,
    so one shared answer pool cannot cross-feed documents and two identical documents are
    answered once. *The document text still crosses the agent's context once per prompt, and
    that cost is not solved here.*
  - **Re-running is safe and is the resume path.** A document whose `fragment_id` already exists
    comes back `skipped`, **not** `failed`, so pointing it at a grown folder ingests only what is
    new. Resume is *derived from the graph*, never bookmarked — there is no cursor and no
    progress file. It depends entirely on the caller deriving `fragment_id` from the path, which
    is why the skill says so in the strongest terms it has.
  - **Every document that could not be read is NAMED, with why**, and one bad document never
    cancels its siblings. A run that cannot say what it did not understand manufactures
    confidence, which is worse than no run.
  - **The limit, stated rather than discovered:** convergence is lexical. `token_sort_ratio`
    catches `Auth Service` ~ `Authentication Service` and **cannot see `Read Cache` ~ `Local
    Store` at all**. The check's evidence scope records the same limit mechanically — `corpus_size`
    and `vocabulary_overlap` are **pinned**, so a passing check proves three documents that share
    words, not 1,124 that do not. *(This is also the first VERIFIES edge in this repo to carry an
    evidence scope: the vocabulary shipped in v0.21.0 and 116 of 116 edges had never used it.)*

- **An Artifact can say what it stands for and how its content behaves** — two new optional
  properties, one schema touch, both aimed at the same failure: *the graph could not tell two
  opposite states apart, and reported the wrong one confidently.* **Schema change, so this is a
  minor bump — but the stamp does not move (29 node types / 60 edge types unchanged), so no
  upgrade doc is owed.** Both default to the pre-existing behaviour, so an older graph reads
  exactly as it did.
  - **`granularity`** (BL-188) — `atomic` (default) / `opaque` / `pending_expansion`. A directory
    Artifact claims its whole subtree, which is the **adopt** skill's own rule, so a registration
    check reported *"every live doc is registered"* — truthfully — across **359 individually
    unreferenceable files**, and `coverage_report` counted the directory as covering everything
    beneath it. Nothing distinguished *deliberately opaque* (a settled archive) from *nobody has
    got to it yet*, which are opposite states with identical readings. `coverage_report` now
    returns `pending_expansion` and `opaque_claims` separately, so *"53 artifacts, of which 3 stand
    in for the rest"* is producible from the graph — a sentence that could not be formed at all
    before. **Deliberately carries no count of what a placeholder stands for:** reflow2 does no
    file I/O, so a stored number would be a caller-supplied figure nothing can recompute, which is
    the staleness BL-187 exists to name.
  - **`volatility`** (BL-191) — `stable` (default) / `append_only` / `living`. Five coordination
    buses modelled with checksums, exactly per the adopt skill's *"a checksum is what makes later
    drift detectable"*, produced **five `checksum_change` divergences within minutes — all correct,
    all meaningless**, because those files are appended to by design; and that disposition was owed
    again on every reconcile forever. A content change on a volatile artifact is now reported as
    **`expected_change`** and **not written to the drift ledger**. **It is reported, not
    suppressed** — silence would trade a false positive for a false negative, letting a wholesale
    replacement pass unmentioned, which is the strictly worse bug and the trap BL-176 avoided.
    **Absence still fires at full severity whatever the volatility.** **Shrink detection is
    deliberately NOT included**: a shrink is the genuinely alarming event for an append-only file
    and a checksum cannot express it — it needs a size baseline nothing records, and the reporting
    team's own compaction shrinks those files on purpose, so the heuristic needs the design's
    consent rather than a bare rule.
  - **`set_artifact_intent`** is the new tool that writes them. A dedicated setter rather than
    arguments on `add_artifact`, for the reason BL-183 made expensive: a constructor taking a
    partial property set and writing the whole node erases what the caller did not name. Omitted
    fields are left alone. **Its enum rejections name the legal values** (BL-192's cheapest fix) —
    otherwise these would have been two more properties reachable from no tool, which is exactly
    the defect BL-202 records.

### Added

- **A near-match now becomes a standing question HEAL can collect** (BL-186, toward
  `cap:corpus-ingest`). `dec:ask-not-repair` requires a suspected duplicate to be *asked*, never
  silently merged, and `cap:corpus-ingest` names the consequence: *"at corpus scale the asking must
  be batched or the feature is unusable."* A `MergeCandidate` cannot be batched — it lives in one
  document's `IngestReport` and is gone the moment the caller opens the next file, so four hundred
  documents produce four hundred separate asks to an agent that has forgotten the last one. Ingest
  now persists the suspicion as a **`DUPLICATES` edge**, which needed no new vocabulary because the
  batching machinery already existed and ingest simply never wrote into it: HEAL's `duplicate`
  detector fires on that edge, `propose_heal` turns it into a merge with the survivor rules, and
  `apply_heal` refuses anything no detector asked for. **Drawn only in the ask band** — at or above
  `auto_merge_threshold` the nodes are merged and nothing is left to ask. `confidence` carries the
  measured score and is omitted for a structural token-subset match, since `0.0` would read as
  "certainly unrelated". Never cascade-fails: a refused edge is a warning, because losing one
  suspicion must not cost the document that carried it. `IngestReport` gains `duplicates_recorded`.

### Fixed

- **The order two documents arrive in no longer decides the canonical name** (BL-186, toward
  `cap:corpus-ingest`). `req:corpus-ingest` calls this its load-bearing clause — *"which file
  happened to be read first must not determine the canonical name of anything"* — and it was
  unsatisfied. On a fuzzy auto-merge the extracted property map overwrites `name` on the survivor,
  so of two specs calling one thing `Read Path Cache` and `Cache Read Path`, **whichever was read
  last named it** — and for a corpus that is the iteration order of a folder nobody chose. Measured
  before the fix, same two documents, same design: `A then B → "Cache Read Path"`,
  `B then A → "Read Path Cache"`. The merge was never wrong (one node, one recorded merge); only the
  name followed arrival order. The survivor's name is now chosen from the two strings alone —
  **longer wins, ties lexicographic** — which is the reading `token_subset_match` already applies
  when it suggests the more specific side as survivor; the tiebreak exists only to make the rule
  total, since equal-length names would otherwise fall back to arrival order and rebuild the bug.
  **The losing name is reported, not discarded**: `FuzzyMerge` gains `canonical_name` and
  `alias_name`, because a merge that silently drops one of two human-chosen names destroys the only
  evidence a person ever chose it, and `dec:ask-not-repair` governs this capability. Agreeing
  documents record no alias. *Deliberately not applied to the direct-id path*: re-ingesting the same
  id with a new name is **matched-evolved**, where the newer document updating its own name is the
  correct reading.

- **The export-lineage guard had never run in CI** (BL-208). `check_export_chain` compares HEAD's
  export with HEAD~1's, and skips — silently — when that question cannot be answered.
  `actions/checkout@v5` defaults to `fetch-depth: 1`, so `HEAD~1` has never existed in a CI
  checkout and the guard has been inert since BL-107 added it. Found because `main` went green on a
  real lineage break that the identical check fails on locally; **proved with two clones of the same
  commit — `--depth 1` says `OK — design and build agree`, `--depth 2` reports the break.** Both
  jobs now pin `fetch-depth: 2`. The skip is right for a laptop outside a git tree and exactly wrong
  for the environment that is supposed to be authoritative: *a guard whose no-op path is silent will
  eventually run nowhere and say so to nobody.*

- **The CI design-coherence gate swept 124 of 144 artifacts and reported OK over the rest**
  (BL-206). `tools/reflow2_check.py` asked `scan_nodes` for every Artifact and iterated what came
  back. `scan_nodes` answers with as many nodes as **fit** and states what it withheld — here
  `total: 144`, `returned: 124`, `omitted: 20`, `capped_by: "size"` — but the gate's JSON-RPC
  client unwraps the `{count, items}` envelope to the items and drops those fields, so a capped
  page was indistinguishable from a complete set. The gate then passed `exhaustive: true` to
  `reconcile_artifacts`, asserting a sweep that was twenty artifacts short. **The blind spot was
  already hiding real drift**: `art:tools-built` changed in `d6631e5` and the gate reported OK over
  it twice. `reconcile_artifacts` was never wrong — asked about one of the missed artifacts
  directly, it named the drift at once. The sweep now pages on `next_offset` and fails loudly if
  the collected count misses `total`. *Its regression test was verified in both directions: with
  the single-page sweep restored, the gate exits **green** with unaccepted drift present.*

### Changed

- **`loop_status` is cheap again — it digests the verification roll instead of serving it**
  (BL-205). `cap:loop-status` promises *one cheap call*, the SessionStart hook and the server
  instructions both push you to run it, and the Stop hook nudges when graph writes finish without
  one — while on this repo's own graph it answered in **74,239 bytes**, over the response limit,
  so the call the loop depends on could not be read at all and had to be recovered from a spill
  file with a script. **99.6% of that was `verifications`**, a full per-check roll beside eight
  integers and a to-do list. It now returns a digest: counts `by_status`, `never_run` (a `passing`
  check with no `last_run_at` is an *assertion*, not a measurement, which a status tally alone
  cannot show), and every **not-passing** check in full, since those are the ones a reader acts
  on. The passing remainder is counted in `omitted` and named in `full_list`, never dropped in
  silence — the same contract `scan_nodes` states for a capped page. **74,239 → 1,123 bytes here,
  with the 3 checks worth reading still present.** *Result-shape change, so minor by this file's
  own rule.* No `brief`-style flag, deliberately: `graph_report` already serves the full roll from
  the same `verification_recency` computation, so a flag would add a third surface onto one list
  rather than a cheaper first one. Digested at the MCP boundary and not in core, so the two
  surfaces still cannot disagree about what the checks say.
- **reflow2's own design record now follows the `link-artifacts` skill it serves** (BL-199), and
  **CI enforces that it keeps doing so** (`tools/self_host_uses_documents.py`). Seven document
  artifacts asserted `REALIZES` against something they do not implement — three claimed a markdown
  file *implements a Decision*, four claimed a capability that code elsewhere implements
  (`docs/collaborating.md` did not implement the merge driver; `merge.rs` does) — and are now
  attached with `DOCUMENTS`. Nine more legitimately realize `cmp:docs`, a component constituted by
  its documents, and **gained** the describing edge they were missing rather than losing the true
  one. **The point is not tidiness: seven artifacts now stand on a describing edge alone, so
  reverting the BL-176 fix produces seven false positives in this repo. Until today it produced
  none — which is exactly why eleven releases of `0 gaps, 0 defects, loop clean` never surfaced it.**
  The new CI guard checks the *record*, not the code, and is mutation-checked against three seeded
  regressions. Deliberately **not** changed: the 11 `schema/*.yaml` files, where `SPECIFIES` versus
  `REALIZES` is a real judgement (the skill calls a machine-readable contract `SPECIFIES`; the yamls
  genuinely *are* the vocabulary, embedded with `include_str!`) — left for a human to settle.
  `delivered` held at 83/101 across the change; `impact-check` confirmed beforehand that every
  affected Capability keeps a real code realizer.

### Fixed

- **`orphan_node` no longer calls a correctly-filed document an orphan** (BL-176). The rule counted
  outgoing `REALIZES` and nothing else, so an Artifact linked the way the served **link-artifacts**
  skill prescribes — a design doc, ADR, README, runbook or agent-instruction file with `DOCUMENTS`;
  an OpenAPI/IDL contract with `SPECIFIES` — reported as *"realizes nothing"*. The message was true
  and the **category** was false: an orphan is a node attached to nothing, and those are attached.
  **Measured in the field before the fix: registering 26 ADR/architecture documents took structural
  defects from 13 to 39 — +26, exactly the batch size — and the false-positive rate from 46% to 82%,
  with ~730 documents still to come.** The reporter stopped work rather than continue, and refused
  the available workaround (asserting a bogus `REALIZES`) because it would be a lie at 756× scale.
  BL-114 had already witnessed the same thing twice in an unrelated repo.
  **The list is now the EXCLUSIONS, and that is the load-bearing change.** Naming the edges that
  *do* attach is what broke: an inclusion list must be extended every time the vocabulary grows, and
  until someone remembers, correct work reads as a defect — BL-170's hidden inclusion list is the
  same shape a second time. An Artifact is now an orphan only when **every** edge it carries, in
  either direction, is bookkeeping (`INCLUDES`, `CHANGED`, `YIELDED`, `AT_EPOCH`), so a new *design*
  edge counts as attachment the day it is added and only a new *bookkeeping* edge needs a line.
  **Deliberately not the degree-zero rule the Decision arm uses:** almost every artifact in a mature
  graph carries a Release `INCLUDES`, so counting it would silence the detector everywhere.
  **An Artifact attached by nothing still fires, and so does a document that documents nothing** —
  that distinction is the whole value of the rule and is pinned by three of the six new tests
  (`crates/reflow2-core/tests/orphan_attachment.rs`).
  **Honest limit, and it is BL-199: this repo cannot demonstrate its own fix.** All 139 of reflow2's
  artifacts carry `REALIZES` — including 32 of the 35 documents and specs, against the skill's own
  instruction — so `detect_defects` here goes 0 → 0 across the change. The evidence is the new tests
  and the field measurement, never this graph.

## [0.23.0] — 2026-08-03

**The design brain learned to talk about its own structure.** Four new readings, none of which
existed at v0.22.1: reflow2 can now certify that a restructuring preserved function, notice on its
own where its build stopped following its design, say where a design sits on the arc from function
to structure, and connect the evidence it already computes to the quality axis that evidence
informs. **No schema change — the stamp is unmoved at 29 node types / 60 edge types.**


### Added

- **`ility_report` — what the graph can actually say about the quality axes** (BL-184).
  `DimensionAssessment.score` is only ever *asserted*, and the one computation over the
  `dimension` enum fits a slope to those assertions — so reflow2 computed the trend of an opinion
  and never the ility. Meanwhile modularity, articulation points, dependency cycles, misplaced
  capabilities, decomposition mismatches, surprising couplings, build granularity and the
  trajectory bands were all being computed and connected to **no axis at all**. In reflow2's own
  design the enum has **never been written to at all** — nine distinctions, zero instances.

  This connects them and **computes nothing new**. It **never derives a score** and never writes
  to the graph: collapsing three cycles into `maintainability: 0.62` asserts a precision nobody
  has, which is why TRL was kept out of that same float.

  **Adverse is inherited, never re-judged** — a finding counts against an axis only where the
  computation that produced it already calls it a defect. A ratio, a trajectory position and a
  granularity observation are reported as *context*, because relabelling them would overrule
  modules that deliberately refused to grade.

  **The answer is not blanket:** `performance`, `security`, `scalability` and `observability`
  report *not informed*, with the reason, rather than reading clean. The output worth reading is
  `worth_weighing` — targets carrying an asserted good score on an axis whose detectors found
  something against them. A disagreement between two records; reflow2 rules on neither.

  No schema change; the stamp does not move.


- **`maturity_report` — where a design sits on the trajectory from function to structure**
  (BL-179). Designs normally get function right first and structure right later, iteratively and
  organically, so a well-developed function layer with no declared seams is a **normal position,
  not debt**. Seven bands — intent, function, allocation, seams, realization, assurance,
  operation — each a count over a population carrying the question it answers, with the
  lowest-scoring measurable band named as the **frontier**.

  **The frontier is relative, so the reading contains no threshold at all** — nothing to default,
  argue with, or quietly tune. reflow2 states where a design *is* and refuses to state where it
  *should* be, the same rule that stops it defaulting a TRL gate: a demonstrator may sit at
  function-first forever and be right; a fielded increment may not.

  Bands scoring **above** the frontier are reported as normal rather than as work done out of
  order, because real designs run ahead of themselves. A band with nothing to measure reads as
  *unmeasured*, never as zero. **No stage name is emitted** — breadboard/EVT/production is how
  people talk about the profile, and a label no computation reads would not earn its keep
  (`dec:edge-orthogonality`).

  **No schema change; the stamp does not move.** Nothing new is declared — this computes over
  edges already in the graph. Surfaced in `graph_report_markdown` as well as on demand.

### Changed

- **The MCP tool surface is carved into the systems the design already named** (BL-181).
  `service.rs` had grown to **6,356 lines and 139 tools in one file** — the design distinguished
  the systems those tools serve and the build separated none of them, which is exactly what
  `granularity_report` reported. 141 items moved **verbatim** into eleven per-domain modules
  under `tools/`, each declaring its own `tool_router`, summed in `ReflowService::new`.
  `service.rs` is now 2,658 lines.

  The carving follows `dec:bl83a-functional-decomposition` — *"reflow2's systems are functional,
  not its file tree"* — because a file tree that disagrees with the design's own decomposition is
  what this existed to fix. Router composition was already proven in-repo by `skills.rs`, so this
  was addition rather than invention.

  **Nothing about the served surface changed, and that is checked rather than claimed:** all 144
  toolsnaps match, and `certify_preservation` returns **`preserved`** — 0 function changes, 0
  unclassified, 36 structural. The first real restructuring that check has seen.

  Re-running `granularity_report` afterwards shows `art:service` **gone from the report**, and
  surfaces three artifacts that were masked behind it — `art:temporal` had been below the cutoff
  at z=1.94 and reads z=4.54 once service.rs stops inflating the spread. That is the z-masking the
  reading already warned about in `not_observed_about`, behaving as documented.

### Fixed

- **Re-calling a constructor no longer erases what the design already knew** (BL-183). Every
  public `add_*` helper named a *subset* of its node type's properties and wrote
  create-or-**replace**, so calling one with an existing id — which is exactly how
  **revise-design** says to change a node's text — silently reset every unnamed property to its
  schema default. **16 of 18 constructors did this.** `add_interface` lost 13 properties
  including `designation`, which is how the design says which contracts are the published
  boundaries (`req:key-interfaces`). `add_requirement` lost `status`, which *is* certainty
  (`dec:certainty-derived`), so rewording a requirement un-confirmed it. `add_capability`
  un-built a `verified` capability. `add_artifact` was BL-166 still reachable one call away —
  that fix had landed on `link_artifact` alone.

  It survived BL-46 and BL-166 because **it is invisible until a property has been moved off its
  default**, and it violated `req:no-silent-fallback` (accepted, priority *critical*).

  **The served revise-design skill was also wrong, and worse:** it told you to revise with the
  generic `create_node` and asserted that an existing id *merges*. It does not — it replaces. So
  the instruction caused the defect. The skill and both mirrors now say to use the typed
  constructor, and warn off `create_node` for revision.

  `create_node` and `import_graph` still replace, deliberately: an import document is a
  **complete** statement of a node, and merging there would resurrect properties it meant to
  omit.

### Added

- **`granularity_report` — reflow2 can now see where its own build stopped following its
  design** (BL-182). An artifact realizing N capabilities the design distinguishes is the build
  holding as *one* thing what the design holds as *N*. It reports that fact and **refuses a
  verdict**: no severity, no suggested fix, and none of the words that turn a fact into an
  accusation — which side is wrong (the file should be N files? the design over-decomposed?
  it is right for this phase?) is not reflow2's to say (`dec:report-dont-judge`).

  **There is no size threshold, by construction.** Artifacts are compared against *this
  design's own distribution*, so a breadboard-phase design where everything lives in one file
  has no outlier and is told nothing — a uniformly coarse design is not a broken one. It speaks
  only once a design has decomposed elsewhere and left one place behind, which is a position on
  the trajectory rather than a score (`dec:maturity-restructuring-delta`). Both cutoffs travel
  with the answer so they can be argued with, and `not_observed_about` names what it cannot
  see: unregistered artifacts, size of any kind, and outliers that mask one another.

  Surfaced in `graph_report_markdown` as well as on demand, because a reading nobody calls is
  still invisible. Pure arithmetic over `REALIZES` edges — no file I/O, no LLM.

### Changed

- **`req:restructuring-is-certified` widened to the function layer that was actually built.** As
  first written it named only Capability/Requirement addition-or-removal and a changed `SATISFIES`
  link, while `certify_preservation` also holds Flow, Actor, Constraint and Project, and treats
  the capability `DEPENDS_ON` DAG, `PART_OF_FLOW` and `INTERACTS_WITH` as function-bearing. **A
  requirement narrower than its implementation is a live hazard**: coverage reads satisfied while
  the requirement fails to pin the behaviour that matters, so a later "simplification" down to the
  letter of the requirement would break nothing any test of intent could catch. The statement now
  also records the rule that makes the check work at all — *a link is judged function-bearing by
  its endpoints, never by its type alone.* Old wording preserved in
  `snap:epoch:requirements-confirmed:req:restructuring-is-certified`.


- **`certify_preservation` — a restructuring is now certified, not asserted** (BL-180,
  `dec:maturity-restructuring-delta`, `req:restructuring-is-certified`). A *maturity
  restructuring* holds the function set invariant and moves everything else: allocation,
  packaging, which functions live in which component, which seams are declared. It is safe
  exactly when function is provably preserved — and that is computable, so this returns a
  **verdict** (`preserved` / `not_preserved` / `indeterminate`) where `compare_designs`
  returns a listing. It is the move `dec:passing-is-verified` makes for tests, applied to
  structure: the difference between a refactor someone hopes is safe and one the graph
  checked.

  **Edges are classified from their endpoints, not their type.** `DEPENDS_ON` is the
  functional DAG between two Capabilities *and* ordinary coupling between two Components —
  one edge type, two meanings. Reading the type alone would file all 51 of this design's own
  cross-system dependencies as function changes and make the check worthless on the first
  design anyone pointed it at.

  **Nothing is waved through.** A node type, an edge endpoint or a property edit the rules
  cannot place lands in `unclassified` and forces `indeterminate` — never a silent pass,
  because a classifier that has not been taught part of the vocabulary must not certify a
  design it never examined (BL-170's fourth quadrant). A reworded capability is undecidable
  by construction — a rename and a scope change are the same bytes — so it comes back with
  both values for a human, per `dec:three-party-checks`. A *known* function change outranks
  an unknown: more information cannot un-move a capability.

  Every certificate, including a clean one, carries `not_certified_about`: this reads two
  design records and no code, says nothing about whether the new structure is *better*, and
  is explicit that a changed Interface can break a consumer without touching a Capability.

## [0.22.1] — 2026-08-02

### Fixed

- **Restoring a design no longer renames it** (BL-169). `import_graph` loaded a document into the
  receiving graph under *that graph's* name, so replaying an export through a temp store returned a
  design called something else. `graph_id` namespaces every stored key and sits inside the export's
  content hash, so the rename was invisible to every other check: the lineage chain linked across
  it, the content hash matched its own content, `reflow2_check` passed and **both CI jobs were
  green** on a design that had stopped being called what it was called. The only signal anywhere
  was a `provenance_note` string in `compare_designs` that nothing gates on.

  Importing into an **empty** store now adopts the document's identity and reports it as
  `adopted_identity`; a store that already holds a design keeps its own name, because layering an
  export onto a live design is an upsert, not a restore. **The rule was not copied — it moved.** It
  had lived in the CLI's `--import` path, which is precisely why the command and the tool
  disagreed about what restoring a design means; it now lives in the operation, so every caller
  gets it.

  `reflow2_check.py` gains an **`IDENTITY`** check that refuses a silent rename the way it already
  refuses a severed chain, sharing the lineage check's pair-resolution rather than reimplementing
  it.

- **reflow2's own kit install is current again** (BL-175). This repository was carrying a
  `REFLOW2.md` from reflow2 **0.15.0** — seven releases behind — while `CLAUDE.md` directed every
  agent here to read it. It is now the 42-line pointer that `req:thin-install` intends, whose
  closing line is the guarantee that stops this recurring: *"Upgrading reflow2 should never
  produce a diff in this repository."*

  **The cause was not neglect, it was hand-editing a file the installer owns.** `place_kit_file()`
  refreshes a kit file only while its content still matches the manifest hash — the guard that
  stops an installer destroying local edits. Three commits edited `REFLOW2.md` in place instead of
  editing `getting-started/` and re-running, so the hash diverged and the file froze: every later
  install correctly refused to touch it. **The protection that keeps your edits safe is the same
  mechanism that keeps a stale file stale**, silently, until someone reads `--check`.

### Added

- **The served instructions now say what to do when reflow2 does *not* get in your way** (BL-174).
  `getting-started/AGENTS.md` — the text every consumer project receives through
  `get_instructions` — told an agent to report what *obstructed* it, and said nothing about a tool
  that answers cheerfully and is wrong. It now carries the counterweight: **a successful tool
  response is a claim, not a result; `0 gaps` means nothing was detected, never nothing is wrong**
  — plus four habits (read the result back; diff two things that ought to agree; ask why odd
  output is odd before filtering it; ask what the check could not have seen). Framed as the other
  half of using reflow2 well, not an argument for using it less. The consumer-facing twin of
  `docs/sharpening.md` §2b.

  The file itself was also, until now, **not a registered Artifact** — while `POINTER.md` and
  `SETUP.md` beside it both were. It is the single highest-consequence text in the kit, compiled
  into the binary, and the design could not see it. Registered as `art:kit-instructions`.

- **Coverage scope is derived from version control, and the adopt skill now says so** (BL-172,
  `dec:coverage-scope-is-declared` — accepted on the maintainer's word). Take everything version
  control tracks and remove what you can name a rule for; never assemble a list of the places
  worth looking, because a hand-picked scope makes a region nobody thought of *invisible* rather
  than *unclaimed*. Phase 4 of `adopt` told you to sweep and never said how to decide the scope,
  which is the hole BL-165 fell through.

  **This holds whether or not the subject is software**, and the reasoning is worth carrying. A
  non-code project puts **two** things under version control — the reflow2 design graph and the
  design artifacts; a code project puts **three** — graph, artifacts, and the implemented code.
  The first two are the constant; the implemented system is the only term that varies. A
  satellite or a fleet cannot live in a repository; its drawings, specs and analyses can, so a
  derived scope is if anything *more* clearly right there, because the tracked artifacts are the
  whole of what the question could be about. A codebase is the special case — special by holding
  *more* than the norm, being the one kind of subject an agent can inspect directly as well as
  through its design. Of the three, the design graph is excluded from the sweep: a design cannot
  be its own subject. Artifacts genuinely outside version control (a PLM
  system, a wiki) are still swept and handed over by the agent: derivation is the default, not a
  limit on what may be observed.

- **The self-model's sweep derives its scope instead of naming it** (BL-170). It swept two hardcoded
  globs, so `schema/` was not *excluded* from coverage — it was never *considered*, and that is the
  general form of BL-165. It now sweeps everything `git ls-files` tracks, minus four named
  exclusions echoed back with the rule that excluded them, and reports through `coverage_report` —
  a capability built in v0.11.0 that had no caller here. A region nobody thought of is in scope by
  default, which is the only way the case that hid BL-165 can surface.

  **The hole this closes, stated as the quadrant it lives in.** `coverage_report` compares what the
  caller swept against what the design claims, so it answers three cases — swept-and-claimed,
  swept-but-unclaimed (`unclaimed_regions`), claimed-but-unswept (`unobserved_locations`) — and is
  structurally blind to the fourth. *Neither swept nor claimed* is mentioned by neither input, so
  nothing can name it; `unobserved_locations` looks like the field that would catch it and cannot,
  because it only knows regions the design already claims. The general fix needs a third input —
  what the design *expects* to be swept — and that crosses the vocabulary, so it is recorded as
  `dec:coverage-scope-is-declared` (**proposed**) with three shapes and their costs rather than
  chosen here.

### Fixed

- **Re-registering a file no longer erases what the design already knew about it** (BL-166).
  `link_artifact` built its properties from the four fields it takes and wrote them with the
  create-or-**replace** form, so every one of `Artifact`'s other five was silently re-defaulted.
  The casualty that matters is `last_confirmed_at` — the dated evidence that someone actually
  checked the file against reality — which made a swept artifact indistinguishable from one nobody
  ever looked at, the exact distinction `reconcile_artifacts(record_events: true)` exists to draw.
  **`status` was the quieter half:** it only ever *looked* safe because its default (`realized`)
  happened to equal the stored value, so an Artifact at `verified` was being silently downgraded
  every time it was re-linked. A re-link still moves the properties it is given; it no longer
  drops the rest.

  **The evidence was in the committed design the whole time:** of the 34 artifacts
  `tools/build_design_graph.py` re-links on every run, *zero* carried a `last_confirmed_at`, while
  the only two in the entire design that did were the two registered by hand the day before and
  never re-linked since. This is BL-46 — a partial edit silently resetting a verified capability
  to `planned` — reappearing at a second call site; `upsert_node` was written for that incident
  and documents this precise hazard, and `set_artifact_checksum` twenty lines below hand-rolls the
  same merge rather than calling it.

### Changed

- **The eleven `schema/*.yaml` domains are registered artifacts** (BL-165), so the vocabulary is
  in release manifests, `reconcile_artifacts` can see it drift, and a ChangeEvent about a schema
  edit has somewhere correct to point. Ten of the eleven had never been registered, through eleven
  releases — which is how a v0.22.0 ledger entry came to claim `src/temporal.rs` had changed when
  the edit was to `schema/temporal.yaml`: with nowhere correct to point, the nearest-looking node
  gets named. **They are derived from the directory, not listed**, so a twelfth domain registers
  itself. No manifest is back-filled: the ten enter from the next release, per the same reasoning
  that left documentation out of the nine releases before v0.10.1 (`dec:intent-preserved`).

  **The root cause was not ten missing calls.** `coverage_report` names all ten on demand — the
  detector was never missing. What was missing is that the filesystem sweep in
  `build_design_graph.py`, written after the 2026-07-20 self-adopt found 15 of 33 source files
  unregistered, had a hardcoded scope of the two `src/` trees — so the one probe built to catch
  unregistered files could not see the directory AGENTS.md calls *"the foundation everything
  builds on"*. It sweeps `schema/*.yaml` now, and says in its own words when a swept file has no
  Artifact at all instead of leaving the reader to decode a synthetic id. That sentence found two
  more on its first run (BL-167).

## [0.22.0] — 2026-08-02

### Fixed

- **`import_graph` now describes itself, reports every fault at once, and stops asking you for its
  own identity** (BL-117, BL-118, BL-138 — all three from real `/adopt` passes by people who are
  not us, following the skill's central instruction *"build one export document and `import_graph`
  it once"*).
  **BL-138:** a document of `{nodes, edges}` — literally what the skill says to build — used to
  fail on `missing field 'graph_id'`. That field is now optional, and the reason it could be is the
  finding: **`import_graph` never read it.** An import loads into the receiving graph, whose id the
  server already knows, so the caller was being asked to restate the receiver's own identity and
  then have the answer ignored. `edges` may be omitted entirely too. **The counterweight is why
  this is not just deleting a requirement:** `mirror_surface` *does* read it and still refuses an
  unidentified document by name, because mirroring records where a surface came from and guards
  against mirroring a design into itself — neither answerable without the id. One rule was right
  for a round-tripped export and wrong for a hand-authored one; the code now distinguishes them.
  **BL-118:** validation stopped at the first violation, so a hand-authored 9,000-line document
  cost four full edit-retry cycles to learn four faults. Every fault is now reported in one
  response with its position (`nodes[1]`, `edges[0]`). **Atomicity is untouched and pinned
  separately** — a rejected import still writes nothing at all, including the items that were
  valid — and it still returns an *error* rather than an ok-with-failures report, so a rejected
  import can never read as success. This is `dec:bulk-is-all-or-nothing-with-per-item-findings`
  reused rather than reinvented.
  **BL-117:** the document shape now rides `import_graph`'s own description — envelope, what is
  optional, that endpoint types are recovered rather than stored — because an export of an empty
  graph teaches none of it and the reporter had to burn a scratch graph to learn it.
  6 new cases (18 in the suite), mutation-checked six ways.
- **BL-119 closed without a build: it was already fixed** by `chg:bl87`, which made the import
  stamp optional and reported. Confirmed in source rather than taken on report — and the same check
  then found half of BL-138 stale for the identical reason. Two of five rows in one cluster had
  been overtaken between filing and triage.

### Added

- **The epoch an increment delivers on is computed, not declared** (BL-68 — the last unbuilt part
  of the board's most ambitious item). **SCHEMA CHANGE — the stamp moves BOTH ways, 28 → 29 node
  types and 58 → 60 edge types**, the first release since v0.4.0 to move both at once. See
  [docs/upgrading-to-v0.22.0.md](docs/upgrading-to-v0.22.0.md); everything is additive and nothing
  is backfilled.
  Three pieces, and the split between them is the design. `add_readiness` records an **observation**
  — a TRL or MRL level 1–9 about an enabling technology, an input fact in the same family as a
  checksum. `gate_on` states a **judgement** — "this increment needs that technology at TRL 7" —
  and it rides an **edge**, so one increment can demand TRL 7 of one technology and TRL 4 of
  another, and a demonstrator and a fielded increment can demand different levels of the *same*
  technology (the row's own worked example). `forecast_readiness` records a **projection** as a
  `TemporalFact` marked `basis: forecast`, because `observed_at` says *observed* and nobody observed
  anything in 2035. `readiness_report` then derives the answer — the earliest epoch by which every
  gating technology clears the level demanded of it — and names the one that decided it:
  *"cannot deliver before 2035, because cmp:conversion is TRL 3 today, projected TRL 7 at 2035, and
  this increment needs TRL 7."*
  **Two refusals are the point, not the rough edges.** An increment with no stated threshold reports
  `ungated`, **never "ready"** — silence about a gate is not evidence there is none. A gate whose
  technology has no level and no clearing forecast makes the whole answer `indeterminate` rather
  than a date computed from the gates that happen to have evidence; dropping the inconvenient one
  would return an optimistic date built by ignoring half the record. Forecast confidence is likewise
  **stated by the author and never derived from horizon** — a decay curve is a judgement about risk
  appetite. The precedent throughout is `Interface.medium`, which once defaulted to `REST` and made
  two silent boundaries "agree" on a value neither had chosen.
  `GATED_ON` is a **traceability edge**, so a technology whose readiness slips reaches the blast
  radius of every increment gated on it — asked before the code was written rather than after a
  detector complained, which is the second time out of four that a new edge type has reached that
  table on purpose. 15 cases, mutation-checked nine ways, plus seven checks driven over real stdio.
  Built to `dec:readiness-is-an-observation-the-threshold-is-the-judgement` and
  `dec:readiness-forecast-is-a-temporal-fact`.

### Fixed

- **The loop nudge's impact-check trigger measured bookkeeping where it meant order** (BL-163).
  It fired on `edits > 0 and changes == 0` — **only when a session recorded zero ChangeEvents** —
  so a session that edited code and then wrote its ChangeEvents up *afterwards* had `changes > 0`
  and was met with silence, while every one of those events was bookkeeping-after. The hook's own
  message says *"Bookkeeping is not the loop"*; the trigger shipped beside it could not tell the
  two orders apart. **The root cause is one line:** `CHANGE_OPS` held `record_change` and
  `add_change_event`, both *recording* ops, and no set counted `propagate_change` or
  `propagate_from` at all — the hook could not separate recording from looking because it never
  counted looking. Now `PROPAGATE_OPS` exists and a session that edited code, recorded a change
  and never propagated is nudged to run impact-check.
  **This adds the one interruption `cap:skill-triggers` deliberately never added**, and it had to:
  the session it catches has no unchecked writes and *has* touched reflow2, so both older branches
  read it as clean and there was no nudge for a shape to refine. The counterweight is the
  conjunction — `edits > 0` (a pure design session has no blast radius to compute), `changes > 0`
  (this session engaged the design brain, which is what stops it becoming a second thresholdless
  bypass nudge), `propagates == 0` — and each clause is pinned by its own test. A session that
  propagated gets nothing; `propagate_from` counts, since the impact-check skill sends speculative
  questions straight to it. Tunable with `REFLOW2_LOOP_NUDGE_PROPAGATE_THRESHOLD`.
  11 new cases (47 total), mutation-checked seven ways.
  **Two things worth keeping.** Dropping the `changes > 0` clause fails seven tests including
  BL-90's *entire* bypass family — the measurement that proves that clause is what keeps this
  branch from swallowing the older one. And a **fourth defect surfaced only because the new tests
  failed**: `update_state` re-serialises the tally from an explicit key whitelist — a third
  hand-kept copy of the state's key set, beside `blank_state` and `parse_state` — so the new
  counter incremented in memory and was silently dropped on every write. That is BL-159's
  two-records-of-one-contract shape a third time, inside a single file.

### Added

- **`orphan_node` now reports a Decision that nothing links to** (BL-162). Found by running the
  `check-health` and `detect-and-ask` skills on reflow2's own design, getting a clean bill from
  every detector, and then counting zero-degree nodes by hand: `dec:sanitize-spof-accepted`, an
  **accepted** single-point-of-failure disposition, had no edges at all —
  `disconnected_community` cannot see it, because it only fires on clusters of ≥2 and a node
  joined to nothing is never a cluster. It matters beyond tidiness: such a Decision is unreachable
  by propagation so it never enters an impact analysis, and a disposition specifically **can never
  expire**, because expiry is computed from the affected set — a conditional judgement quietly
  becomes permanent. Graded by status: **Warning** when `accepted`, **Info** otherwise, since a
  parked decision point is a legitimate state. `decision:ack:` review records are excluded, matching
  the design network's existing rule that they describe a judgement *about* the design rather than
  its structure. **The rule keys on degree zero rather than on a missing `GOVERNED_BY`, and that was
  settled by measurement**: the edge-named form fires on six of reflow2's own decisions, five of
  them already connected — BL-42's shape, where this detector once became 20 of 31 defects and had
  to be cut back. Degree-zero fires on one, and any edge at all silences it. 6 cases,
  mutation-checked three ways.

- **The build now refuses to let its own gate list drift** (BL-159). AGENTS.md's *"A change is
  done when all of these are clean"* block and `.github/workflows/ci.yml` were two hand-kept
  records of one contract, and following the documented one exactly still produced a red build.
  `skill_lint.py` now cross-checks them four ways: **coverage** (every `cargo`/`python3` gate CI
  runs is either in the block or named in the blockquote as deliberately omitted), **fidelity** (a
  listed gate is spelled exactly as CI runs it, *flags included*), and both rot directions — a
  documented gate CI does not run, and an omitted name CI has stopped running. Fidelity is the
  load-bearing one: the defect that filed BL-159 was a flags difference on a gate that *was*
  listed, which coverage alone cannot see. The lint **observes**, the document **judges** — whether
  a `ci.yml` line is a gate at all is mechanical and lives in code; whether a gate belongs in the
  everyday local subset is judgement and stays in AGENTS.md, read from the prose a person already
  reads rather than a parallel machine-readable list. It found two real holes immediately
  (`cargo test -p reflow2-core --no-default-features` and `test_check_doc_versions`, in `ci.yml`
  and in neither list) and one in itself. New hermetic suite `tools/test_skill_lint.py`, 14 cases;
  mutation-checked seven ways.

### Fixed

- **A checksum's LENGTH is a dialect too, and the compensation lived in the wrong layer**
  (BL-160). Designs register digests at mixed lengths — reflow2's own `build_design_graph.py`
  writes `hexdigest()[:16]` — while an honest caller running `sha256sum` supplies all 64, and
  `reconcile_artifacts` compared **strings**. A full sweep of a provably clean tree reported
  **51 phantom drifts** in the same minute `reflow2_check.py` said *"OK — design and build
  agree"*: the gate was right for the wrong reason, because it carried a Python truncation
  workaround no other consumer had. Every consumer that was not the gate — an agent driving
  `reconcile_artifacts` over MCP, another project's CI, the coding agent the tool's own
  description tells to *"compute the hashes yourself"* — hit the bug the gate was immune to.
  This is BL-125 in a second form and takes the same verdict: *a false red on a gate whose whole
  job is to be believed is worse than no gate*. `artifact::checksums_agree` now answers it in the
  core, on **both** the drift comparison and `set_artifact_checksum`'s would-move-the-baseline
  guard — the second is not a gate problem at all but a BL-157 bulk sweep being refused on every
  short-registered artifact for a change that never happened. The Python workaround is **deleted**
  rather than duplicated by the next consumer. What is required is a real **prefix** relationship,
  never truncate-both-to-N, and it applies to the `sha256:` dialect only: two full digests sharing
  sixteen characters are still drift, `blake3:zz` and `blake3:zzzz` stay different, and an empty
  digest agrees with nothing. When the two dialects agree the longer digest stays on the record,
  which makes the accept idempotent across dialects. No minimum prefix length is imposed — a short
  baseline is a weak baseline, but its strength is decided when it is registered, and a read side
  refusing to honour what the write side accepted would be the same write/read disagreement again.
  Measured on reflow2's own design: 109 artifacts, 51 truncated baselines, all 51 unchanged.
  9 new cases (14 total), mutation-checked seven ways.

- **The loop nudge corrupted the record it judges from** (BL-161). A session that consulted the
  design graph constantly was told at Stop that it never had — three times in one session, and the
  second independent reproduction. `write_state` used `Path.write_text` (truncate, then write —
  **not atomic**) while `read_state` swallowed a parse failure into an **all-zero, `touched: False`**
  tally. PostToolUse hooks run as separate concurrent processes and parallel tool batches are
  ordinary, so one hook reading while another wrote got a truncated file, the failure was swallowed
  into zeros, and that process **wrote the zeros back** — wiping `touched`, `artifacts`, `captures`
  and `gap_pass` for the rest of the session.

  This is AGENTS.md rule 4 and engineering principle 2 violated inside the tool that enforces them
  (*"no catch-returns-default… a swallowed failure makes broken code report success"*), and the
  right pattern already existed here — `ver:content-store` pins *"an interrupted write leaves no
  partial file, proven by writing to a temp path and renaming"*. **Reproduced rather than inferred:**
  seeding `{touched: true, artifacts: 4, writes: 4}` and firing 150 concurrent edit hooks returned
  `{touched: false, artifacts: 0, writes: 0, edits: 6}` — 144 of 150 increments lost with every
  sticky field. A gentler 40-pair run loses one update, which is why it read as intermittent.

  Fixed three ways: the read-modify-write takes an exclusive `flock` (degrading to the previous
  behaviour where `fcntl` is absent, because a hook must never break a session); the write goes to a
  temp file and `os.replace`; and an unreadable tally still **restarts** the count — an existing
  test required that and was right — but the restart is now **marked**, and the Stop backstop drops
  its one *negative* claim (*"the graph was never consulted"*) when the flag is set. A tally rebuilt
  from nothing cannot prove nothing happened. The *positive* claim (*"N writes went unchecked"*)
  survives a restart honestly, which is what keeps the flag from being an off switch.

- **The nudge now keeps its own "fires once" promise** (BL-111). Every nudge ends *"this nudge fires
  once; stopping again proceeds"*, and that rested entirely on the harness's `stop_hook_active` — a
  flag covering a single stop *cycle*, never persisted. So the rule implemented was *once per stop
  cycle* while the rule advertised was *once per session*, and the gap bit hardest exactly where the
  nudge could not be satisfied: a session whose server is unreachable was nudged at every stop with
  no action available that would stop it, which is when someone disables the hook.

  `claim_nudge()` is an atomic test-and-set — the first caller prints, everyone after stays silent —
  and it had to be a *claim* rather than a flag set after printing, which is where BL-161's lock
  earns its keep. **The hook can legitimately be registered more than once**: reflow2 installs
  machine-wide *and* a project can carry its own registration, and the two command spellings do not
  dedupe, so two processes run the Stop hook concurrently. A plain read-check-write would let both
  read `nudged: false` and both print — the doubled message this was filed from. The counterweight
  is that the claim is spent by *nudging*, not by *stopping*: a session with nothing owed stops
  silently and keeps its one nudge.

- **The nudge's op sets never learned the bulk forms** (BL-161, second half). `ARTIFACT_OPS` held
  `set_artifact_checksum` but not `set_artifact_checksums`; `CAPTURE_OPS` had no `create_nodes`; the
  gap-pass reckoning had no `gaps_to_prompts`. A session doing everything right *through the tools
  BL-153 shipped* tallied as having done none of it — BL-152's shape landing on the trigger that
  judges whether the loop ran, and it worsens exactly as the bulk forms succeed.

### Added

- **A word for "nothing moved" — the artifact ledger's missing third answer** (BL-157, BL-158).
  Two findings, one hole, both found by hitting them rather than by reasoning about them.

  **`baseline_established`, a third drift disposition** (BL-157). `set_artifact_checksum` required
  a disposition and both available answers presupposed a movement: `design_holds` means *the code
  moved and carried no design meaning*, `design_updated` means *behaviour moved and the design
  moved with it*. An artifact registered with **no** checksum getting its first one is neither.
  Closing `art:detect`'s missing baseline therefore recorded a `refactor` of a file that session
  never touched — a change that never happened, written into the ledger that exists to keep the
  design free of exactly that. The new disposition takes no `change_type` and records
  `ChangeEvent.change_type = baseline_established`: the record moved, the code did not.

  **Which disposition is available is now a fact, not a preference, and the wrong one is refused.**
  An accept against an artifact with no checksum is refused naming `baseline_established`; a
  `baseline_established` that would **move** an existing baseline is refused naming the other two.
  That second guard is what keeps this a fix rather than an off switch — without it the new
  disposition would be a way to accept real drift without answering what the change meant, which
  is the silent accept `dec:two-sided-accept` exists to forbid. Re-establishing the *same*
  baseline stays idempotent, so re-running a sweep is safe. `baseline_established` is also
  **refused by `add_change_event` and `record_change`**, so the label cannot be applied by hand to
  an ordinary change and the ledger's count of first baselines still measures something.

- **A clean reconcile records what it confirmed** (BL-158). `record_events` only ever recorded a
  *divergence*, so a pass that checked everything and found everything correct wrote nothing —
  and `loop_status`, which computes `unexamined` from recorded claims, went on saying nobody had
  ever looked. Reproduced first-hand on reflow2's own design: **107 artifacts, 106 unchanged, zero
  drift, and the number moved by zero.** The operator who checks everything and the operator who
  checks nothing produced identical graphs. A recording pass now stamps `Artifact.last_confirmed_at`
  on every artifact observed to still match, `reconcile_artifacts` returns them in `confirmed`, and
  `confirmation_ledger` reports `confirmations` / `last_confirmed_at` and counts them toward
  *examined*. Supersedes BL-134, which had the same finding by inference.

  **A property rather than an event, deliberately**: a confirmation is high-frequency and says
  nothing changed, so a node per artifact per pass would bury axis Z — the log of what actually
  *moved* — under non-events. It is the shape `Verification.last_run_at` already uses to answer the
  same question about a check. **A confirmation records only what was observed**: a partial sweep
  confirms exactly the artifacts it looked at, a drifted artifact is never confirmed, and an
  undated pass writes none and returns them in `unconfirmed_undated` rather than dropping them
  silently.

  **Proven on the real design, which is BL-158's own measurement replayed.** The freshly-built
  binary, driven over stdio JSON-RPC against a throwaway copy of reflow2's own 1218-node design,
  swept all 109 registered artifacts off the actual working tree: 109 unchanged, zero drift, 109
  confirmed, and `loop_status` went from *"1 built capability never checked against reality"* to
  entirely clean. The capability that cleared was `cap:skill-triggers` via `art:nudge-detect` — the
  exact claim the loop had been asking about, pass after clean pass. The row's original measurement
  was 107 artifacts, 106 unchanged, zero drift, and the number moving by **zero**.

  Both are schema changes that **do not move the version stamp** — the stamp counts node and edge
  *types*, and this adds one `change_type` enum value and one `Artifact` property. No older reflow2
  is locked out and no upgrade doc is owed. One caveat worth stating: the stamp cannot see an enum
  widening, so an older binary will open a design containing a `baseline_established` ChangeEvent
  and only refuse if it tries to re-write that node.

- **Bulk forms for the five tools the surface measurement caught calling themselves** (BL-153 fix
  shapes (1) and (3), `cap:bulk-forms`, `dec:bulk-is-all-or-nothing-with-per-item-findings`,
  `dec:bulk-keeps-the-judgement-per-item`). `create_nodes`, `create_edges`,
  `set_artifact_checksums`, `acknowledge_gaps` and `gaps_to_prompts`. Together with
  `release_includes_all` these answer **every** self-loop BL-153 named: `set_artifact_checksum`
  244, `create_node` 112, `contains` 109, `acknowledge_gap` 90, `gap_to_prompt` 83,
  `contain_component` 77, `satisfies` 74.

  **`create_edges` is one tool, not six.** `contains`, `contain_component`, `satisfies`,
  `allocate` and `realizes` are thin wrappers that only fill in the endpoint types, so a bulk
  `create_edge` is the bulk form of all of them — and BL-155 found 40 of 132 served tools never
  called, which makes six near-identical tools a cost rather than a convenience.

  **All of it or none of it, *with* per-item findings.** BL-153 posed the refusal semantics as a
  choice — "all-or-nothing, or per-item findings?" — and it is neither/both: every item is
  attempted so you learn every failure in one round trip, and if anything failed the batch is
  discarded and nothing is written. The store already had the atomic batch HEAL's apply step and
  `import_graph` use. Collecting all failures is also the defect BL-118 files against
  `import_graph` ("validation is fail-fast, one error per attempt"), which a bulk form must not
  inherit — surfacing one error per round trip would replace N writes with N retries.

  **The judgement stays per item.** `set_artifact_checksums` carries a disposition *per artifact*
  and `acknowledge_gaps` a reason *per gap*, never hoisted to a call-level argument. BL-153 named
  this as the trap that would make a bulk form worse than the loop it replaces, and
  `dec:two-sided-accept` is what it would break. 244 accepts now cost one call and still 244
  decisions. `gaps_to_prompts` groups answers per gap for the same reason plus a mechanical one:
  it is what stops two gaps' prompt ids colliding, so no gap is ever replayed against another's
  answers. A half-answered ask batch is refused rather than half-served.

  A rejected bulk write returns an **error** carrying every failure in its `data`, not a payload
  with `applied: false` — a tool result reads as success, and "nothing was written" dressed as a
  result is the silent-failure shape this project forbids.

- **`release_includes_all` — a release's manifest is derived from the design instead of typed
  out** (BL-153, `cap:derived-release-manifest`, `dec:manifest-derived-is-not-manifest-accepted`).
  One call turns every Artifact and Component the design holds into an `INCLUDES` edge, freezing
  each artifact's current checksum as shipped. `release_includes` was the single largest line item
  in reflow2's entire recorded usage — 1008 calls across 7 sessions, 988 of them consecutive, about
  144 per release cut — all of it typing out something the graph already knew, and the rule
  AGENTS.md already states: a release "must list every component that goes out, not a
  hand-maintained roll-call". Measured on reflow2's own design: **160 edges in one call**.

  Four guards, because a bulk write is where a design erodes quietly:
  - **Nothing is written unless `apply: true`** (default false), matching `reconcile_artifacts`'
    `record_events` — a call that packages a release is the one you most want to read first.
  - **Re-running never rewrites a frozen `as_checksum`.** An entry already in the manifest comes
    back `already_present` and is left alone. A derivation that recomputed every entry would
    rewrite the manifest of a shipped release each time the live drift baseline moved.
  - **An `exclude` id naming nothing is refused**, whole call, before anything is written — a
    caller who believes they excluded something they did not would ship it and never be told.
  - **`without_checksum` names the artifacts whose entry cannot say *what* shipped**, rather than
    leaving a `null` to be discovered when someone asks what a past release contained.

  This is a derivation, not an accept: `dec:two-sided-accept` and `dec:ask-not-repair` bound bulk
  *dispositions*, and no disposition is taken here — the graph is asked what the project contains
  and answers.

### Fixed

- **AGENTS.md's documented gate list disagreed with the gates CI runs** (BL-159) — 8 commands
  against ~24, and one of the 8 carried the wrong flags. The missing
  `cargo clippy -p reflow2-mcp --all-targets -- -D warnings` is what turned two `redundant_closure`
  warnings into a red build while every documented gate was green; measuring the rest of the
  divergence then found the same bug latent one line above, where the `-p reflow2-core` clippy
  lacked `-D warnings`. Both flags corrected, `reflow2_check` added, and the block now states
  that it is a subset with `ci.yml` authoritative — green here is not green there.

- **`art:detect` had no drift baseline**, so `crates/reflow2-core/src/detect.rs` — the file
  realizing `cap:detect`, `cap:kpp`, `cap:aggregate-gap-keying`, `cap:release-pinned-to-time` and
  `cmp:detect` — could never report drift, and `reconcile_artifacts` returned `no_baseline` rather
  than clean. Found twice the same hour by two independent routes: an exhaustive reconcile sweep
  (106 of 107 unchanged, this one uncheckable) and the new derived manifest, which named it as the
  sole entry in `without_checksum`.

### Changed

- **A dependency cycle that runs only through file and library contracts is now a `Warning`, not a
  `Critical`** (BL-141(b), `dec:foundation-cycle-is-a-warning`). `Critical` means *must fix*, and a
  loop that exists only because two parts read and write the same file formats has nothing to fix:
  a renderer that reads MIDI and writes WAV, against a transcriber doing the reverse, has no runtime
  dependency in either direction. Four such loops were reported `critical` in one adopt pass and
  none was real.

  **Downgraded, not silenced** — the finding keeps its place, its affected set, its suggested fix
  and its explanation, and loses only the claim that it is an emergency. Suppressing the case
  outright was considered and rejected: shared-data coupling is sometimes genuine, since two
  services over one table are truly entangled when a schema change in one breaks the other.

  Two guards keep it honest. **One real `DEPENDS_ON` edge anywhere in the loop keeps the whole cycle
  `Critical`**, because a genuine dependency does not stop being one by sharing a loop with a data
  contract. And `Interface.medium` defaults to `unspecified`, which is *not* a foundation medium —
  so **silence about the medium keeps the louder answer**, and a design that never classified its
  boundaries is not quietly excused.
### Added

- **The four agent-facing capabilities that v0.21.0 was planned for, delivered late** (BL-68's
  siblings; `cap:tool-carries-convention`, `cap:gap-carries-a-reading`, `cap:skill-triggers`,
  `cap:session-artifacts`). `arrival_delta` on `rel:v0210` now reports all four **delivered** —
  they had been `outstanding`, the honest fifth outcome for work nobody had said would slip or drop.

  - **The tool carries what an agent would never guess.** Three served descriptions gained their
    missing convention: `set_verification_status` (*a check left at `planned` is not confirmation*),
    `record_change` (*record the change BEFORE you make it*), and `export_graph` (*export once
    between commits — the lineage link is built from whatever file is already at that path*).
    `skill_lint` now holds a **named register** of tools whose convention must survive rewording,
    plus a description budget. **BL-154 is the evidence:** measured over 46 sessions, skills are read
    once per 380 tool calls and four are never read at all, while the description arrives with every
    call.

  - **A question arrives as options the user can pick.** `detect-and-ask` now states all six
    obligations — offer a reading, carry what would change it, make the options selectable, put the
    recommendation first and mark it, give every option its consequence including the ones not
    recommended, and answer in the user's language. Checked by `skill_lint`, with two negative
    checks that are the load-bearing half: **no served skill may hardcode a particular answer
    language**, and none may write the user's word — a status, a decision, an acknowledgement — in
    the same breath as a recommendation.

  - **The moment tells the agent which skill it needs.** The loop nudge stops merely counting writes
    and matches four situations: an edit with no ChangeEvent names `impact-check`; a recorded change
    with no artifact link names `link-artifacts`; captured intent with no gap pass names
    `detect-and-ask`; a rendering written with nothing stored names `session-artifacts`.
    **It adds no new interruptions** — a shape only refines a nudge the hook had already decided to
    send, so the count is unchanged and only the sentence improves. A session that did the right
    thing is met with silence, which is the case that matters most.

## [0.21.0] — 2026-08-01

**The release that made the QUALITY of evidence a fact the graph holds.** Until now reflow2
recorded that a check *exists* and that it *passes*, and nothing else — so a green tick looked
identical whether the check ran this morning across the whole input space against an independent
source, or a year ago at one fixed seed against the very data the thing under test was fitted to.
Three axes close that, and a fourth finding stops a detector claiming more than it checked.

**This release moves the schema stamp (57 → 58 edge types), so it is not optional if you share a
design** — see [docs/upgrading-to-v0.21.0.md](docs/upgrading-to-v0.21.0.md).

### Changed

- **A `circular_dependency` finding now says which edge kinds it actually walked** (BL-141(a),
  `cap:cycle-names-its-basis`, `ver:cycle-basis`). Every cycle names the Interfaces its hops were
  collapsed out of, says whether any hop is a real `DEPENDS_ON` edge, and says when every Interface
  involved is a `library`/`data` medium — something read or linked against rather than called
  across at run time. **Detection is unchanged**: the same cycles, the same `critical`. Only the
  sentence grew.

  Why it matters: `dependency_pairs` collapses `c CONSUMES i` + `p PROVIDES i` into a direct pair,
  which is right for detection and threw away the one datum a reader needs. An adopt pass over an
  ~11k-LOC research repo produced **four `critical` cycles and none were real** — the message
  `A → B → A` reads identically whether the code is tangled or one Interface node is standing for
  two contracts. The same class as BL-114: a finding claiming more than the detector checked.

  **The medium is the load-bearing part, and that was found by measurement rather than reasoning.**
  The first build discriminated on interface *count*, following the report's own diagnosis that
  each phantom was "one Interface standing for two contracts". Reproducing their cycle on their
  real design showed it runs through **two** interfaces and is structurally identical to a genuine
  service cycle — a renderer reading MIDI and writing WAV against a transcriber doing the reverse.
  Only `medium` tells them apart, so that is what gets reported, with a counterweight test pinning
  that the same shape over `REST` makes no such claim.

### Added

- **The evidence-quality family — a check's TIME, INPUT and INDEPENDENCE become facts the graph
  holds** (`cap:verification-freshness`, `cap:evidence-scope`, `cap:independent-evidence`;
  `ver:evidence-quality`, 18 cases, mutation-checked on all three axes). BL-106, BL-126 and
  BL-136 are one hole seen three ways: reflow2 has always recorded that a check **exists** and
  that it **passes**, and never what its evidence **covers**.

  - **TIME** (BL-106) — the confirmation ledger gains `last_verified_at` and
    `verification_freshness` per claim: is the newest passing check older than the newest
    accepted change to what it covers? `Verification.last_run_at` has been written on every
    status set since the beginning and read by nothing, the same shape as the temporal axis
    before BL-70. A **fact, never a gap** (`dec:verification-freshness-not-a-gap`) — it would
    fire on every legitimate refactor, and a list that can never reach zero gets skimmed.
    **On reflow2's own design this reports 9 genuinely stale claims**, `cap:store` worst at nine
    days between its last check and the last accepted change beneath it.
  - **INPUT** (BL-126) — `set_evidence_scope` records what a check **pinned** and what it
    **swept**, on the `VERIFIES` edge rather than the `Verification`, because scope is a fact
    about the *claim*: one suite can cover one capability across the whole space and touch
    another at a single point (`dec:evidence-scope-on-the-verifies-edge`). `evidence_report` then
    names the parameters every passing check pinned and none swept. A check stating no scope is
    counted **unscoped**, never read as broad — and on reflow2's own design that is **87 of 87
    passing checks**, which is the silence the axis exists to make visible.
  - **INDEPENDENCE** (BL-136) — `calibrated_against` records what a value was **fitted** to, and
    any passing check that *is*, or *produced*, that evidence is reported **consumed — a fit, not
    a test** and excluded from independent evidence. Structural by construction, not analytic:
    the project this came from built four independent internal diagnostics and *none* could have
    found its circular fit, because no check inside a design can establish its own independence.

  **Found by dogfooding before it shipped**, and it is the family's own failure mode one level
  up: comparing dates as whole strings called a check dated `2026-07-28` stale against an accept
  at `2026-07-28T14:52:00-04:00` — an ordering nobody recorded, asserted by the very report that
  exists to stop exactly that. Same calendar day is now `Unknown`, with the counterweight that an
  earlier *day* is still stale.

### Changed

- **Schema: a new `CALIBRATED_AGAINST` edge type (57 → 58), which MOVES THE GRAPH STAMP.** A
  graph written by this build is refused by v0.20.0 and earlier; see
  [docs/upgrading-to-v0.21.0.md](docs/upgrading-to-v0.21.0.md). It is a **traceability** edge by
  deliberate decision (`dec:calibration-propagates`), so correcting an anchor puts every value
  fitted to it in the blast radius. That question was asked *before* the code was written rather
  than after a detector complained — `INCLUDES` and `SCHEDULED_FOR` each reached the impact table
  only once `disconnected_community` fired on an island they had failed to join, and the table's
  own comment says nothing checks the question is asked.

## [0.20.0] — 2026-08-01

### Added

- **A changelog is a derivable view of the graph's own delta** (`cap:changelog-view`,
  `ver:changelog-view`; PR #9). `changelog_view` renders the difference between two moments of
  this design in the format the industry already reads. Buckets (Added/Changed/Deprecated/
  Removed/Fixed) are **mapped** from vocabulary the graph already records and every entry names
  the rule that placed it; anything no rule covers comes back in `unmapped` rather than being
  guessed or dropped. Omit both ends for `[Unreleased]` — everything after the last **deployed**
  release — which makes *"what would this increment's changelog say?"* answerable before cutting
  it. **The output is a DRAFT and says so**: no entry claims what a consumer should do, because
  the graph holds what moved and never what it costs downstream; `needs_a_human` names that
  obligation instead of inventing it. Nothing is stored — a stored changelog would be a second
  source of truth able to disagree with the graph.

  *This entry was itself written by hand, three commits late, which is the argument for the
  capability: see BL-137, found by running the tool on this very cut.*

- **A Release that names no moment is reported** (BL-122; `cap:release-pinned-to-time`,
  `ver:release-without-epoch`; PR #11). A release cut is a point in time, and a `Release` with no
  `AT_EPOCH` edge cannot be placed on axis Z — so a changelog window cannot be computed from it
  and the design cannot say what was true when it shipped. The detector found three on this
  project's own graph, and **v0.17.0 is still missing its edge to this day** despite v0.18.0's
  commit message boasting that it would not repeat the omission. Two things the backlog item did
  not name and the build did: `planned` releases are exempt, because an epoch is minted at cut
  time and demanding one earlier is an alarm on correct work; and `Release.status` **defaults to
  `planned`**, so a shipped release whose status was never set inherits that exemption — checked
  against the real graph and recorded as a passing test rather than a comment.

- **`set_interface_spec` accepts `medium`, so the foundation exemption is reachable from the
  tools** (BL-129; `ver:interface-spec`). `Interface.medium` and its honest `unspecified` default
  already existed, the seam checker already compared it, and the structural detectors already
  exempted a `library`/`data` foundation from `single_point_of_failure` — *"a library linked into
  its callers cannot fail on its own"*, as AGENTS.md warns. **Only the door was missing.**
  `add_interface` takes id and name; `set_interface_spec` filled in eight properties and not this
  one, so the sole route was `create_node`. A user following the obvious path left every boundary
  at `unspecified` and collected false single-point-of-failure warnings for shared libraries,
  having done exactly what the tools invited — the punishing-correct-work shape of BL-23.

  Put on `set_interface_spec` rather than `add_interface` deliberately: `medium` is part of what
  a consumer must **agree with**, which is that tool's subject, and every other contract property
  already lives there. `add_interface` stays minimal, so there is one way to do this rather than
  two. Omitting it still leaves the stored value alone, like every other field on that tool.

  A **minor** by the versioning table — the tool surface gained a parameter — and no schema change
  (the property was always there), so no stamp move and no upgrade doc. Toolsnap regenerated
  deliberately: one tool, one field.

### Fixed

- **An acknowledgement no longer counts as design structure, and `disconnected_community` can
  finally be closed** (BL-124; `ver:acknowledgement-not-structure`). `acknowledge_defect` wires
  `GOVERNED_BY` from every affected node to the review Decision, deliberately, so the review
  stays reachable from the design — and `disconnected_community` hashes its id from the affected
  set. For that one category the two behaviours collide: the review **joined the island it
  acknowledged**, enlarged it by one, minted an id nobody had accepted, and the defect returned
  one node larger every time. An entire category was permanently unclosable, which is exactly
  the *"a list that can never reach zero gets skimmed"* failure the acknowledge tools exist to
  prevent. Reproduced in the field across four sessions of a real project, growing 8 → 9 → 10.

  Fixed in `design_network()` rather than in the defect id, because that network has **three**
  consumers and the other two were wrong silently. **Measured on reflow2's own graph** (125
  review records, 610 edges): four of the eight most central nodes were acknowledgements and are
  now none, with real nodes rising into their place (`rel:v0170` +75%, `cmp:detect` +41%); and
  `surprising_connections` went **16 → 32** — the bookkeeping edges were *suppressing* half the
  real surprises by tying communities together, which is the opposite of the pollution that was
  predicted. Reproduce with `tools/bl124_instrument.py`.

  A review is still recorded, still carries its reason, and is still reachable by `GOVERNED_BY`
  from what it acknowledges — it is excluded from the *network*, not from the *graph*, and it
  still appears in a blast radius, because a review genuinely is affected when what it reviewed
  changes. Three counterweights are pinned: an **ordinary** Decision still counts as structure,
  a genuinely isolated cluster still fires, and withdrawal still reopens the *same* defect.

- **A bare content hash no longer reports drift on a file nobody touched** (BL-125;
  `ver:checksum-dialect`). `canonical_checksum` turns a bare hex digest into `sha256:<hex>`,
  and since 2026-07-25 it ran on the two **write** paths only — `drift.rs` compared literally.
  So a caller who passed a bare hash to `link_artifact` and the same bare hash to
  `reconcile_artifacts` was told **every artifact of an untouched tree had drifted**, which is
  precisely the false red that function was written to stop: *"a false red on a gate whose whole
  job is to be believed is worse than no gate."* Both sides now go through the canonicaliser.
  The observed value is canonicalised too, not merely compared canonically, because it is part
  of a `checksum_change` event's identity — leaving the raw form filed one divergence under two
  ids depending on the dialect supplied.

  It failed as a **false positive**, never an error: well-formed output, correct `realizes`
  edges, correct `propagation_seeds`, entirely wrong conclusion — and the natural response
  (re-register everything) overwrites the baselines and hides it for another cycle. Measured at
  the real MCP surface both ways: before, `unchanged: 0` with a `checksum_change` and a blast
  radius seeded from a file nobody edited; after, `unchanged: 1`, no findings, no seeds.
  **Mutation-checked by construction** — the suite was written first, and the three bug cases
  failed while both counterweights passed, so a "fix" that made every comparison equal would
  have passed the bug cases and destroyed the detector. Found by an external review of a project
  designed end to end through reflow2.

## [0.19.0] — 2026-07-31

### Added

- **The design can hold what it points at: a content-addressed store, committed to the
  repo** (`dec:where-content-lives`, `dec:content-store-implementation`,
  `dec:what-lives-where`; `cap:content-store`, `cap:content-manifest`,
  `req:the-store-is-reachable-from-a-session`).

  **`content_put`, `content_get`, `content_exists`, `content_manifest`** — the store is
  reachable from a session, which is the half that was missing: `cap:content-store` was
  `realized` and passing its check while *nothing could call it*. The repo holds what the
  design PRODUCED; the store holds what INFORMED it — the documents, diagrams and captures
  a Decision points at and would otherwise lose at session end.

  Hand-rolled and synchronous in `reflow2-core`, **zero new crates compile** (`base64` was
  already in the lock and is now a declared dependency rather than an implied one, per
  `dec:design-dependencies-declared`). `object_store` is the documented upgrade path, not the
  implementation.

  **`--content-path` is its own flag, deliberately not derived from `--graph-path`.** The
  graph lives under `.reflow2/`, which is gitignored, and blobs are COMMITTED — deriving
  would have put a consumer's diagrams somewhere git ignores, quietly contradicting the
  decision that they travel with the design. A server with no store configured **refuses by
  name** rather than inventing a directory; a default chosen at call time is
  `req:no-silent-fallback`'s failure wearing a friendly face. `text` or `base64`, exactly
  one: passing both is refused rather than resolved by silently dropping a payload.

  **The manifest is DERIVED from the graph, never stored** (`dec:content-manifest`). A
  manifest kept as its own record would be a second source of truth about what the design
  references, and would drift the first time someone updated one and not the other;
  rendering it to a committed file is a projection, the same as every other view. The
  readable name is the Fragment's own `title` — the graph already requires one, so there is
  no second place for names to live. `missing` names content the graph references and this
  checkout lacks (the case someone handed the export alone hits, where a diagram that will
  not open becomes a named finding rather than a silent absence); `orphaned` is the reverse,
  bytes referenced by nothing, which is how a store grows without anyone deciding to.

  **`.gitattributes` marks the blob directory `binary`** — the point is not the diff, it is
  that line-ending conversion would otherwise silently corrupt a PNG on a CRLF checkout:
  data loss on someone else's machine that nothing in the history would explain.

  Toolsnaps 124 → 129.

- **What bounds the store is WHAT gets stored, not how big it is**
  (`dec:content-growth-is-bounded-by-what-not-by-size`).

  Measured on reflow2's own material before deciding: the entire design prose is 3.5 MB
  across 64 markdown files, the export 1.7 MB, the whole `.git` history 81 MB — and **29
  session transcripts come to 115.8 MB, mean 4.0 MB each.** Transcripts alone are 1.4× the
  entire repository history, accumulated in a couple of weeks. That inverts what
  `dec:content-manifest` assumed ("raster images are the real risk"), and the correction is
  recorded rather than edited into accepted text.

  **So a size cap is the wrong lever, which is the whole point.** A 4 MB file passes any
  sane threshold; what ends a repository is 4 MB × every session, permanently, unprunable.
  The control is *what* gets stored — transcripts by exception, not by default.
  `content_put` refuses past **100 MB** with `accept_large` as a recorded override, and the
  threshold is anchored to GitHub's hard block rather than invented to feel safe
  (`req:defaults-do-not-assert`). The refusal says in its own text that it is *not* what
  keeps the store small, so it cannot be mistaken for the answer; one test asserts exactly
  that, and another asserts a transcript-sized file passes — the case that makes the cap
  insufficient.

  **The manifest reports total bytes and largest entries** — report, never judge: no
  threshold, no warning. This is the piece that would have surfaced the finding without
  anyone running `du` on a hunch.

- **The cut trigger stops being vacuously true**
  (`dec:release-trigger-needs-a-required-item`). `missed_obligations.is_empty()` is
  vacuously true when nothing is required, so an increment promising *nothing* read as ready
  — the empty-release failure `dec:release-trigger` was chosen to prevent, arriving through
  its own back door. `ready_to_cut` now requires an empty miss list **and** at least one
  obligation, and `required_count` is reported beside it because an empty miss list is
  otherwise ambiguous: everything landed and nothing was promised look identical. An
  increment with nothing required gets a note saying it has not been scoped, rather than a
  bare no. Mutation-checked — drop the second clause and the empty-increment test fails.

  Found by asking the machinery rather than by reasoning about it: `rel:v0200` reported
  READY while holding one unbuilt capability. The same query surfaced that **75 of 79 built
  capabilities are scheduled against nothing**, so `arrival_delta` today answers "did the
  plan hold?" accurately and cannot answer "what actually shipped?" at all — the
  `added_after_baseline` blind spot `dec:arrival-delta` already names under "deliberately
  not built".

- **The time axis runs forward: epochs can be PLANNED, and work can be SCHEDULED against
  them** (`req:epochs-can-be-planned`; `cap:planned-epochs`, `cap:satisfaction-schedule`).

  **`DesignEpoch.status`** is `planned` or `arrived`, with **`plan_epoch`** to create a point
  that has not happened and **`set_epoch_status`** to move between them. Arrival is the
  interesting direction: the moment a claim about the future becomes a point in the past, and
  the moment a planned-versus-delivered delta becomes computable.

  Status is its own property rather than a value in `epoch_type`, because **kind and tense are
  orthogonal** — folding `planned` into the type enum would make a planned MILESTONE and a
  planned RELEASE CUT unsayable, and those are the two a roadmap is made of. The default is
  `arrived`, which is a *record* rather than a choice: `add_epoch` has only ever meant "record
  the point I am at", so every epoch written before this property existed did arrive.

  **`record_change` now REFUSES a planned epoch.** A snapshot captures the present, so it
  cannot belong to a point that has not happened. This is the half that makes `status` a
  property the system *reads* rather than one more declared-and-unconsulted field.

  **`schedule_for`** adds the satisfaction schedule — the `SCHEDULED_FOR` edge from a
  Requirement or Capability to a DesignEpoch (time axis) or a Release (capability-increment
  axis), carrying `modality`: `expected` (a plan) or `required` (an obligation whose miss at
  arrival is a computed violation — the scheduling face of a KPP). One edge serves both views
  because they are two views of one architecture. There is deliberately **no `achieved`
  modality**: delivery is computed from the golden thread and never asserted, so a schedule
  that recorded its own success would be a second source of truth able to disagree with the
  first.

  **SCHEMA CHANGE — the stamp moves, 56 → 57 edge types.** A graph written by this version is
  refused by an older reflow2, which is deliberate and loud (BL-19/BL-94). `SCHEDULED_FOR` is
  additive; nothing was retired. It is kept separate from `AT_EPOCH` on purpose — that edge
  means *belongs to* and is declared over a wildcard source, so one type carrying both meanings
  would be indistinguishable to every detector.

  Three new tools (`plan_epoch`, `set_epoch_status`, `schedule_for`); `add_epoch`'s description
  now says it records a point that HAS happened. Toolsnaps 122 → 124.

- **`arrival_delta` — what was PLANNED against what actually arrived** (`dec:arrival-delta`,
  delivering obligation 2 of `req:plans-move-honestly`; `cap:arrival-delta`,
  `cap:plan-movement-recorded`). Anthony's question, in his words: *"what didn't we achieve
  that we were supposed to in increment 10?"*

  Every scheduled item comes back as **delivered**, **deferred** (and where to),
  **discontinued**, or **outstanding** — a fifth outcome beside the four originally sketched.
  The four assume every undelivered item was consciously moved or dropped; the commonest case
  is that nobody touched it and it did not happen. Calling that *discontinued* would put a
  withdrawal on the record nobody made, and *deferred* would invent a date nobody chose, so it
  is reported as itself and put to the user — the one question `req:plans-move-honestly` says
  must be asked and never defaulted. Work scheduled after the baseline is reported separately,
  because a delta measured only against the plan cannot see the work that was not in it.
  `required` claims that did not land come back as **computed violations** rather than slips.

  **The baseline is the target's FIRST snapshot**, with every later one returned as the
  movement trail. The last would have measured only the most recent revision: two replans leave
  epoch 3 holding `{A,B,C}` then `{A,C}`, so reading the last says the plan was always `{A,C}`
  and the slip vanishes from the very report meant to show it.

  **Nothing about the outcome is stored.** The plan lives in the epoch's snapshots and delivery
  is computed from the golden thread, so writing the result down would create a second source
  of truth able to disagree with the first — the same argument that keeps `achieved` out of
  `modality`.

- **A lossy schedule edit is now REFUSED while the plan is unrecorded** — removing a
  `SCHEDULED_FOR`, re-pointing it, or rewriting its modality, through either `delete_edge` or
  `delete_node`. Re-pointing B's edge from epoch 3 to epoch 4 without a recorded change leaves
  the graph saying epoch 3 was only ever about A and C: the plan silently rewriting its own
  history, which `req:intent-preserved` forbids. The refusal names the `record_change` that
  unblocks it. **Adding** to a plan destroys no earlier claim and is deliberately free.

### Fixed

- **A snapshot no longer drops a commitment on the floor** (`dec:commitment-edges-survive-snapshots`).
  `snapshot_node` excluded every edge whose other endpoint was a bookkeeping node type, and
  `DesignEpoch` is one — a proxy that was exact when written, because every edge to an epoch was
  then audit trail. `SCHEDULED_FOR` broke it: an edge to an epoch is now a *commitment*. So
  `record_change` on a scheduled requirement — the obvious way to record a slip — captured a
  snapshot with the schedule edge silently dropped, destroying the due date it was called to
  preserve **and reporting success**. The exclusion is now by the edge's ROLE; `AT_EPOCH` on the
  same node stays out, and a test pins that. Nothing could have detected this: a snapshot that
  drops an edge is indistinguishable from one whose node never had it.

- **`pair_designs` — the seam between two designs is now COMPUTED, not hand-wired**
  (`req:complementary-pairing`, `cap:complementary-pairing`). This was the last
  open gap in the design.

  **The missing half was the subscribe side.** `Interface.designation` could say
  `published` or `internal` — a design could state what it OFFERED but not what
  it NEEDED in any form another design could be matched against. It now carries
  `published` / `required` / `both` / `internal`, and the role lives on the
  Interface rather than the node, because a component both publishes and
  subscribes so a per-node role collapses to `both` and pairs with everything
  (`dec:pairing-role-placement`).

  Pairing matches **complements** — `published`/`both` against `required`/`both`
  — never like with like, the way a base pairs with its complement and not a
  copy of itself. Two boundaries pair when their names match fuzzily (reusing
  ingest's two-band resolution rather than a second matcher) **and** they agree
  on `medium`, `transport_security` and `auth`.

  **All three axes, because two of them were learned the hard way.** The first
  draft keyed on role plus medium, and the dynograph-foundation trial refuted it
  from the provider's side within an hour: their design carries three
  `medium: REST` boundaries, one of which is public and unauthenticated *by
  design* because an orchestrator's liveness probe cannot hold a credential.
  Under medium alone, "I require REST" pairs against it — not a near miss but
  the rule confidently producing a wrong, security-relevant answer. There is a
  test for exactly that case, and a mutation confirms it fails if the key is
  narrowed back.

  Five outcomes, all useful: **paired**; **conflicts** — names match, axes refuse
  — reported with *every* refusing axis rather than the first, so nobody fixes
  `transport_security`, redeploys, and only then discovers `auth` also refuses;
  **unmet needs** (we require it, nobody publishes it — the loudest signal);
  **dead surface**; and **duplicate providers**, since two publishers of one need
  is a conflict rather than a match. Uncertain name matches are **candidates to
  ask about**, never actions (`dec:ask-not-repair`).

  Boundaries carrying no role are **counted and named**. `internal` is the
  default, so it cannot distinguish "deliberately internal" from "nobody
  classified this", and without saying so a design that never did the labelling
  would pair with nothing and report a clean seam.

  `seam_report` is unchanged and complementary: pairing says *which* boundaries
  correspond, `seam_report` says whether the full contracts agree once they do.
  Its doc comment has said since July that pairing would one day supply those
  pairs instead of a person; it now does.

### Removed

- **Nine proposed requirements retired as "considered, but not accepted"**
  (`dec:proposed-requirements-pruned`). Twenty-six sat at `proposed`; nine are
  now `dropped` — kept on the record with their full statements and a snapshot
  of their final state, not deleted, because the captures were real and several
  were found by measurement. `dropped` also stops them raising
  `unsatisfied_requirement`, so the open list means what it says again.

  Seven went as not value-adding: two that record a *doubt* rather than a need
  and say so in their own names (`drift-rolls-up-to-a-score`,
  `framework-is-chosen-not-defaulted`); two that are backlog hygiene dressed as
  product requirements (`friction-has-a-baseline`, `friction-has-a-severity-bar`);
  one whose acute half `cap:bounded-reads` already answers
  (`context-is-a-modelled-quantity`); one purely speculative
  (`blocking-is-partial`); and one redundant with the already-delivered
  `req:coverage-visible` (`adopt-says-how-much-it-got`).

  Two were folded into `req:defaults-do-not-assert`, which states the rule they
  restate — *the schema must not declare what nothing reads or checks*:
  `edge-defaults-do-not-assert` and `functional-vocabulary-computes`.

  **A correction worth reading, made mid-execution.**
  `req:supporting-is-not-conflict` was proposed as a third fold on the grounds
  that it had exactly one edge and was "an orphan in all but name". That edge
  was an incoming `SATISFIES` from `cap:supporting-is-not-conflict`, which is
  **realized** — the requirement was *delivered*, and its status had simply never
  moved off `proposed`. It is now `accepted`. Low edge count was read as low
  value; for a requirement that has been built, edge count is exactly backwards.

  Structural isolation turned out to be evidence about **wiring**, never about
  worth, in both directions: it over-accused four parked captures that are good
  ideas, and under-accused one that was already shipped. Disconnected islands
  fell from 6 to 3 as a side effect; the three that remain are one coherent
  planning cluster that wants wiring, not retiring.

### Added

- **`set_project_mode`, and reflow2's own project is now `rigid`**
  (`req:mode-is-chosen-and-changeable`, `cap:governance-mode`). A project's
  governance mode decides whether `apply_heal` **applies** structural repairs
  (`flexible`) or **proposes them and stops** so a human decides (`rigid`).
  Until now it could be set only at `genesis`, so every design ever made
  carried the `flexible` default and could never move off it — a governance
  choice nobody made and nobody could revisit. There is now a setter; an
  unknown mode is refused by schema validation and leaves the previous choice
  intact, and the project's other properties survive the write.

  **reflow2's own design has been moved to `rigid`.** `apply_heal` merges and
  deletes nodes, and this repo has already been bitten once by an auto-apply
  corrupting a graph — the chained-duplicate guard exists because two
  individually-sanctioned merges wrote to a node the first had deleted while
  the report still said `verified`. Structural edits to the design brain get a
  human in front of them. The cost: every HEAL repair here is now two steps.

### Changed

- **`Project.mode`'s schema description now says what the mode actually does.**
  It read *"flexible = design evolves with the build; rigid = design is the
  source of truth"* — which promises a breadth the code does not implement.
  `mode` gates exactly one thing, `apply_heal`, and the description says so.
  The prose is the discovery surface agents read, so prose that over-promises
  is the defect `req:schema-prose-is-checked` is about; this is one instance
  fixed at the source rather than left as an example.

### Fixed

- **A standing judgement about the whole design stops expiring every time the
  design grows** (`req:set-scoped-acknowledgement-keys-on-its-rule`,
  `dec:aggregate-gap-keyed-on-rule`). `gap_id` hashes a gap's affected nodes, so
  a gap whose subject moved gets a fresh judgement — right for a gap about
  specific nodes, and wrong for an AGGREGATE whose affected set *is* the whole
  population the rule ranges over. There the set changes on every addition, so
  the acknowledgement could never carry: `unvalidated_capability` had been
  re-acknowledged about **twenty times**, at 33, 34, 35 … 65, 67 and 68
  capabilities, always with the same disposition, and about twenty of those
  reasons said in their own text that the churn was a finding.

  An aggregate gap is now keyed on its rule alone. The discriminator is an
  explicit `GapSource::is_aggregate()`, written as an exhaustive match so a
  future aggregate detector must come and decide rather than silently inherit
  per-node keying. **It is deliberately not keyed on `GapScope::Project`**,
  which is the obvious-looking answer and is wrong: `unsatisfied_requirement`
  and `status_contradiction` are project-scoped but carry one requirement each,
  so keying on scope would collapse every unsatisfied requirement in a design
  into one gap sharing one judgement — accept one and the rest go quiet. A test
  pins that trap specifically, and both it and the fix are mutation-checked.

  The trade-off is real and accepted: a capability added later is covered by the
  earlier judgement without a fresh look, which is what a *standing* disposition
  means. The growth stays visible without the churn, because a review names the
  count it was made at while the live gap's title carries the count now.

  **One-time migration**: the rollup's id moves from `gap:80f8bc457bfe9e16` to
  the stable `gap:0a77650b58242054`, so it needs one fresh acknowledgement — the
  last it should ever need. The twenty historical ones are left as they are
  rather than withdrawn, because withdrawal marks a Decision `superseded` and
  would claim the judgement was revoked; `reviewed_gaps` reports them as
  `retired`, which is what actually happened.

### Added

- **`mint_seat`, and `claim_region` now refuses rather than guessing an owner**
  (`req:seat-identity-survives-stateless-mcp`, `cap:seat-handle`,
  `dec:stateless-seat-handle` — Anthony chose option (a), mint-and-carry with a
  loud refusal). `mint_seat` returns a durable name for a session; pass it as
  `seat` on `claim_region` and reuse it for the whole session.

  **Nothing changes for existing callers.** On stdio and on Streamable HTTP
  below MCP 2026-07-28 the session already gives the server an identity to hang
  a claim on, so `seat` stays optional and omitting it behaves exactly as
  before. `claim_region`'s schema shape is unchanged.

  **On the sessionless transport (2026-07-28 and later) omitting it is
  refused**, by name, saying to call `mint_seat` — because there rmcp builds a
  handler per *request*, so a seat minted on your behalf would be a different
  string on your next call and the claim's owner would change under you. The
  refusal is the load-bearing half, not a convenience: minting silently
  *succeeds* while recording an owner that drifts, and `claim_report` would
  report one session as several owners while liveness stopped meaning anything.
  Serving a wrong answer quietly is what `req:no-silent-fallback` exists to
  forbid.

  Verified on the thing itself: `tools/stateless_seat_probe.py` now drives all
  three transports and checks **both** halves — one client keeps one seat
  everywhere, and a claim with no seat is refused exactly where the session
  cannot supply one, with the refusal required to name the remedy. It exits
  **zero** for the first time, so `ver:stateless-seat` moves to `passing` on its
  own evidence and the acknowledgement that held it acceptable while failing has
  been **withdrawn** rather than left standing. Seven Rust cases pin both
  answers of the transport question, plus the version threshold itself — one of
  them fails deliberately if a future rmcp moves `ProtocolVersion::LATEST` past
  2026-07-28, which is the day the sessionless path becomes the default.

### Changed

- **Upgraded to rmcp 3.0.1** (`dec:rmcp-v3-upgrade`), the release that implements
  the MCP 2026-07-28 revision. The whole code change is one generic bound:
  `StreamableHttpService::new` narrowed from `S: Service<RoleServer>` to
  `S: ServerHandler`, because the sessionless transport builds a handler per
  *request* and has to ask it for `get_info` and the tool list with no session
  having cached them. Both surfaces that come through that door already
  implement it via `#[tool_handler]`.

  Of the eight breaking-change areas in the v3.0.0 notes, exactly one reached
  this code. The `#[tool]`/`#[tool_router]`/`#[tool_handler]` macros absorbed
  the MRTR response-enum change on `call_tool`/`get_prompt`/`read_resource`
  entirely; MSRV 1.88 is under this workspace's 1.94; and the OAuth, tasks,
  subscription and split-metadata surfaces are ones reflow2 never touched.
  **All 118 toolsnaps match — the served tool surface did not move**, so no
  consumer has anything to change.

### Added

- **`tools/stateless_seat_probe.py`** — one client, two claims, count the
  distinct seats, per transport and per protocol version. It exists because
  every gate stayed green through the upgrade and that green was misleading:
  every one of reflow2's own test clients negotiates `2025-06-18`,
  `2025-11-25` or `2024-11-05`, and `ProtocolVersion::LATEST` in rmcp 3.0.1 is
  still `V_2025_11_25`, so **nothing in the suite speaks `2026-07-28`** and
  nothing exercised the sessionless path the requirement is about. The same
  shape as coverage row 3AX-3 the day before: a tick only as wide as the case
  its evidence exercises.

  What it measures — stdio: **one seat**. HTTP at 2025-06-18: **one seat**.
  HTTP at 2026-07-28: **a different seat on every request**, because rmcp
  builds a handler per request and `ReflowService::share` mints a seat per
  service. That is `req:seat-per-client` gone on that transport:
  `claim_report` would report one session as N owners, and a stale-seat
  refusal would fire against your own previous write.

  **Not an outage, and the probe is what says so**: Claude Code and grok build
  both connect over stdio, which is unaffected, and no client reaches the
  sessionless path by default. Nor does reflow2's own shared mode — `proxy.rs`
  pins `2025-06-18` on both of its handshakes, so a session process proxying to
  a daemon still gets a session and a seat of its own. The broken path is
  reachable only by an external client dialling `--http` and choosing
  2026-07-28 itself. It is a deadline — the 2026-07-28 revision's 12-month
  lifecycle window — not a breakage. Exits non-zero today and is
  deliberately **not** a CI gate; it is a baseline failing on purpose in the
  sense of `docs/sharpening.md`, and worth promoting to a gate when the fix
  lands.

  No configuration avoids it: `legacy_session_mode` applies only below
  2026-07-28, and requests negotiating that version are served statelessly
  regardless. The client chooses, so reflow2 cannot decline on its behalf.
  `dec:stateless-seat-handle` records the four options at `proposed` and
  **awaits Anthony's word** — his direction covered the upgrade, not the shape
  of the seat fix.

### Fixed

- **A node revised twice in one epoch no longer loses its first snapshot**
  (`req:snapshot-per-revision-not-per-epoch`, `dec:snapshot-id-per-revision`).
  The snapshot id was `snap:{epoch}:{node}` and nothing else, while
  `create_node` merges on an existing id — so the second `record_change`
  against the same node in the same epoch silently overwrote the first
  snapshot **and reported success both times**. That contradicted
  `req:intent-preserved` ("the past is never overwritten") and falsified the
  revise-design skill's closing promise that a reader can answer *"what did
  this say before"* without git archaeology. Found 2026-07-28 by following the
  documented procedure exactly — amending one requirement twice in a single
  epoch — and the pre-amendment text survived only in a previously committed
  export.

  The **first** capture in an epoch keeps the unsuffixed id, because existing
  graphs and committed exports carry those ids; only a genuine second revision
  appends `:r2`, `:r3`, so `HAS_SNAPSHOT` becomes one-to-many exactly when
  history requires it. An **identical** re-capture returns the existing
  snapshot rather than minting a duplicate — snapshotting a node that has not
  moved is a no-op, not a new version, and treating it as one would make the
  history claim edits that never happened. That comparison is against the
  **tail** of the chain and nothing earlier: a node edited A → B → A inside one
  epoch has three genuine revisions, and matching any earlier snapshot would
  hand back the A-capture for the third and record two — hiding an edit that
  did happen, the same loss as the overwrite, just quieter. A ceiling of 64 distinct snapshots
  per (epoch, node) errors rather than growing history quietly: an epoch is
  meant to bound a round of *work*, and a node revised that many times inside
  one means the epoch has stopped meaning anything.

  A patch, not a minor: no id that exists today moves, and nothing in the tool
  surface changes shape.

- **`art:graph`'s drift baseline caught up with `da50ae8`** — the
  dangling-edge refusal changed `graph.rs` without the two-sided accept, so the
  design gate had failed on main for three commits. Accepted `design_holds`:
  `req:no-silent-fallback` already said this and the code was brought to meet
  it, so there is no design meaning to record.

## [0.18.0] — 2026-07-28

### Added

- **reflow2 installs once per machine — starting a project is no longer a thing
  you do** (`req:no-setup-per-project`, `dec:install-once-per-machine`). The
  release installer now registers reflow2 with your agent for *every* project:
  the MCP server at user scope, the ten slash commands in `~/.claude/commands/`,
  the coherence-loop hooks in `~/.claude/settings.json`, and a `reflow2` command
  on `PATH` (`install` / `init` / `check`). Starting a design is then
  `cd anywhere && claude` and `/genesis` — no per-project installer, no config,
  no restart. `reflow2_init.py` keeps its job for a repo you SHARE, where a
  teammate's agent must be told reflow2 governs the code, and `tools/reflow2_install.py`
  has `--check` and `--uninstall`.
- **A directory with no design costs nothing** (`--only-if-present`,
  `cap:latent-surface`). What makes the machine-wide registration safe: the
  store is created if absent, so without this a user-scope server would drop a
  RocksDB store into every directory a session was ever opened in. Where no
  design has been started, reflow2 now serves the LATENT surface — the handshake
  says it is installed and available and that this directory has no design,
  exactly one tool is served (`reflow2_start_design`), and nothing is created.
  Deliberately distinct from the degraded surface, which means a graph exists
  and could not be opened. `loop_nudge.py` is gated by the same test, so
  machine-wide hooks are silent wherever `.reflow2/` does not exist.
- **`/genesis` and `/adopt` ship as slash commands.** The eight that shipped
  before were all mid-loop ones, so a brand-new project had no discoverable way
  in — found by setting one up and typing `/genesis` to no effect. Each names
  the other for the wrong-door case.

- **Say when two linked designs disagree at a boundary** (`seam_report`,
  `req:seam-incompatibility`). Compares paired boundaries across the eight axes
  an interface spec carries — medium, paradigm, payload format, auth, transport
  security, operations, error model, payload schema — and classifies each as
  **agreed**, **incompatible**, **differs**, or **unstated**.

  Built to a specification that came from a *measurement*, not an opinion: with a
  seam hand-drawn between two real designs, `compose_and_analyse` plus every
  ordinary detector produced **zero** findings. They reason about structure, and a
  contract mismatch is a comparison of properties **across a pair** — which
  nothing did. The silence was not the absence of problems; it was the absence of
  anyone looking.

  Three rules it will not bend on. **`unspecified` is never agreement** — an axis
  nobody stated reports as unstated and is counted separately, so "0
  incompatibilities" can never be read as "compatible". **Free text is never
  called incompatible** — a machine cannot tell a real mismatch from two people
  wording the same contract differently, so `operations`, `error_model` and
  `payload_schema` report as *differs, a person must read this*. And **the report
  always names what it did not examine**: the types that *cross* a boundary are
  part of the contract and invisible to it, so even a clean seam says so.

  Pairing is supplied rather than computed, because the subscribe side is not
  declarable until `req:complementary-pairing` lands.

### Fixed

- **The session-start line no longer asserts a design exists.** It claimed "this
  project has a design graph" and sent the agent to **where-am-i** — in a project
  created minutes earlier, a constant stating something nobody measured, and the
  skill's own text says to use **genesis** when the graph is empty. It now names
  both doors and lets one cheap call decide which.
- **`reflow2_init.py --check` crashed on an empty project directory** — the first
  thing a new user points it at. It read a pointer target without checking it
  exists, while `pointer_targets` deliberately returns files that do not exist
  yet for a project owning no instruction file. The check path now reports the
  create the write path was already making.
- **`reflow2_init.py`'s pre-update backup was skipped whenever the design was
  shared.** It used a plain `--export`, which opens the store — and since sharing
  became the default the shared server holds the lock, so the backup silently did
  not happen exactly when it was worth having. It now falls back to
  `--export-snapshot`.

- **`Interface.medium` no longer defaults to `REST`.** Every interface created
  without a stated medium *claimed to be REST* — so two boundaries that had each
  said nothing came back "agreed" on a value neither had chosen. Found because
  three of the seam tests failed, correctly.

  It is not cosmetic: `medium` is a pairing-key axis, so a library boundary
  silently reading as REST would pair against a REST provider and the rule would
  confidently produce a wrong answer. The default is now `unspecified`, the same
  principle `designation` already follows — publishing is a commitment, and so is
  naming a protocol.

  **reflow2's own three interfaces were all wrong** and two are corrected
  (`ifc:core-api` → `library`, `ifc:graph-export` → `data`). Enum values are not
  counted by the version stamp, so this locks nobody out — but **defaults apply
  on create, never retroactively**, so every interface written before today keeps
  what it has. An existing design may still claim a medium nobody chose.

- **`seam_report`'s `design` parameter declares its type** (BL-28). schemars
  renders `serde_json::Value` as an "any" schema with no `type`, which the smoke
  test's own check refuses — every advertised parameter must say what it
  accepts. It now advertises `object`, the export document it always required;
  a stringified export is rejected at the schema instead of deeper in. This is
  what held main red between the v0.17.0 cut and this one.

## [0.17.0] — 2026-07-28

**Upgrading:** [docs/upgrading-to-v0.17.0.md](docs/upgrading-to-v0.17.0.md). **Nobody is locked
out** — this release adds no node or edge types, so the version stamp does not move and an older
reflow2 still opens a design written by it. The one cost is a slow first build: the
dynograph-foundation pin moved v0.11.0 → v0.12.0, forcing a `librocksdb-sys` rebuild.

**The release that made two repos check each other.** Almost everything here was found by running
reflow2 against a second real project rather than by reviewing it — a provider published its
surface, this consumer composed against it, and each side found defects the other could not see.
Three of the fixes are in reflow2 itself.

### Changed

- **The dynograph-foundation pin moves to v0.12.0**, and not as housekeeping. It closes a type leak
  this project was structurally exposed to — `search_fulltext` returned `dynograph_text::TextHit`,
  a type from a crate reflow2 never names and reaches only through an optional feature — and it
  makes the `rocksdb` and `fulltext` feature names, which reflow2 forwards to *by name*, a
  committed contract upstream rather than an internal this build silently rested on. Verified safe
  before the tag existed, by building and testing this consumer against the provider's unreleased
  tree. **No storage format changed**: `keys.rs` and `backend.rs` are untouched between the tags,
  so a graph written by the previous foundation reads identically.

### Added

- **A design can declare which version of another design it depends on**
  (`declare_dependency`, `reconcile_dependencies`, `reflow2.toml`,
  `req:design-dependencies-declared`).

  The cross-repo trial made this load-bearing rather than convenient: a seam
  analysis compares your design against a dependency's published surface, both
  sides move, and **without a recorded pin there is nothing to take a surface
  *as of***. Proven rather than supposed — reflow2 pins dynograph-foundation at
  `v0.11.0` while storyflow pins `v0.9.4`, two minors apart, and the provider
  could not produce an as-of-tag surface at all. An offer from `main` described
  **neither** consumer's real contract.

  Two facts are kept apart on purpose: **what you mean to depend on** (the
  declaration — durable, committed, and the thing a provider can acknowledge)
  and **what your build actually resolves** (the observation — read fresh every
  time, because that is what ships). Storing only the first gives a document
  that drifts; storing only the second gives a fact nothing can contradict.
  Comparing them is what makes *"am I relying on something I never declared?"*
  answerable — the state the trial named as the dangerous one, because it breaks
  with nobody at fault.

  `reflow2.toml` is **generated**, and carries which reflow2 wrote it, for the
  same reason the export carries a version stamp. Declaring nothing reads as
  "nobody has said", never as "depends on nothing".

  **Core does not parse `Cargo.toml`.** The caller supplies the observation, as
  `reconcile_artifacts` and `coverage_report` already do — because storyflow
  pins *one* dependency across a `Cargo.toml`, a `docker-compose.yml` and a
  `versions.env`, and a Cargo-only core would model a third of that seam and
  report the rest as absent.

- **`Resource` gains `version`, `components`, `features` and `declared_in`** for
  the `design-dependency` case. Declared on `Resource` rather than as a new node
  type deliberately: the version stamp counts node and edge *types*, so a new
  type would lock out every older reflow2 for a feature that does not need it.
  The fit is imperfect and the imperfection is the price of not breaking every
  existing install.

### Fixed

- **An install can no longer report success while leaving the kit invisible**
  (`req:kit-reaches-the-agent`). When a project owns **no instruction file at
  all**, `reflow2_init.py` now **creates** the primary-harness convention
  (`CLAUDE.md`) carrying a pointer to `AGENTS.md`, instead of only appending to
  files that already exist.

  Found in use, not in review: installing into a repo that had neither
  `AGENTS.md` nor `CLAUDE.md` wrote a fresh `AGENTS.md`, printed a success
  report, and the next session saw no reflow2 anything — because Claude Code
  reads `CLAUDE.md` first and never opened it. This is the **same defect class**
  the installer already documents from storyflow, in the opposite direction: that
  fix protected an *existing* `CLAUDE.md`, and did not cover a project with no
  instruction file at all — which is the ordinary state of a repo that has never
  been agent-worked, and therefore of an adopt target. The rule is not "protect
  what exists" but **reach what reads**.

  Deliberately narrow. Creation happens *only* when the project owns no
  instruction convention whatsoever, so a repo that already has one does not get
  another invented, and `GEMINI.md`/`.cursorrules`/the rest are never written
  into a project that asked for none of them. The created file stays a pointer
  rather than becoming a second home for instructions.

  **And the eight slash commands now ship with the kit** (`/gaps`, `/health`,
  `/where`, `/req`, `/decisions`, `/debt`, `/brainstorm`, `/kpp`) — the one
  narrow exception to `dec:skills-served`, recorded as
  `dec:commands-are-the-exception`. Skills stay served, because a stale skill is
  silently wrong. A command is four lines naming a skill, with no version-coupled
  content, so a stale one is still correct — the skill behind it is fetched fresh.
  The single way a command *can* rot is by naming a skill that no longer exists,
  and `skill_lint` now fails on exactly that, so it is caught here rather than in
  someone else's repo. Without them a consumer install was experienced as
  **broken rather than thin**: the skills were reachable and nothing said so.

### Added

- **A published surface can carry a behavioural promise** (`set_requirement_designation`,
  `req:publishable-promise`). `Requirement` gains a `designation` (`internal` |
  `published`), and `export_surface` carries the published ones alongside the
  boundaries.

  Found by a real cross-repo trial rather than by review: a provider design
  published its surface to a consumer design and **could not express the one
  commitment the consumer most needed** — that a missing on-disk backend fails
  loud rather than silently falling back to memory. `export_surface` withheld
  every `Requirement` as internal, behavioural commitments *live* in
  Requirements, so the document said what the boundaries **are** and nothing
  about what any of them undertakes to **do**. The promise survived only as a
  comment in the *consumer's* build file — on the wrong side of the seam, where
  the provider would never see it change.

  Opt-in per requirement and `internal` by default, for the same reason
  `Interface.designation` is: publishing is a commitment, and defaulting to it
  would assert one nobody made. Undesignated intent is still withheld and still
  counted. A surface with no promises now **says so** — "none stated" must never
  read as "none exist", which is the same false-green rule the trial turned up
  elsewhere.

  Property addition only, so the version stamp does not move and no older
  reflow2 is locked out.
- **Sharing one design between sessions is now the DEFAULT, and needs no setup**
  (`--shared`; `req:sessions-share-a-graph` completed). Point every session at
  the same graph and they all read and write it, concurrently, with nobody
  starting a server or choosing a port:

  ```jsonc
  // what reflow2_init.py now writes
  {"command": "…/reflow2-mcp", "args": ["--graph-path", ".reflow2/graph", "--shared"]}
  ```

  A `--shared` session looks for the server holding that graph, starts a
  **detached** one if there is none, and speaks to it on the session's behalf.
  **No session owns the server** — it runs in its own process group, so the
  session that happened to start it can end without taking anyone else's design
  brain with it. An idle server expires (`--idle-timeout`, default 120 minutes)
  so the store's write lock is not held against the CLI forever, and an attached
  session recovers from that by itself.

  **This is not a new capability — it is the missing half of one that shipped in
  v0.14.0.** `--http` already let several sessions share a design; what it never
  had was a way for a session to *find* that server, so using it meant a human
  starting a daemon, picking a port, and editing every client's config. Nobody
  did, because the installed default did the opposite.

  **What that cost, measured rather than imagined.** A StoryFlow fleet of three
  lead sessions and a worker pool ran for five days believing the design graph
  was single-holder *by nature*. They built a HOLD/RELEASE convention around it,
  voted 3/3 on whether to give each session its own graph, read the design
  through best-effort store copies, and wrote *"workers do NOT run reflow2"* into
  their standing protocol — while the binary they were running had `--http` the
  whole time. The lesson is not that they missed a flag; it is that **a
  capability you have to reconfigure your way into is one most users never
  reach**, and reflow2 shipped the configuration that made concurrent sessions
  fail. Sharing is now what you get, and working alone is the special case (it
  costs nothing: one session starts one server and is its only client).

  Proven on the default path, not on a hand-built server: four sessions started
  simultaneously against no server elect **exactly one** — the store's own write
  lock is the arbiter, so there is no check-then-start race — and all four then
  share one design. Killing a session leaves its peers writing; `SIGKILL`ing the
  server leaves a deliberately stale rendezvous behind and the next tool call on
  an attached session still succeeds, with the pre-crash design intact.

- **`--stop-shared`** — stop the shared server holding a graph and release the
  write lock, without hunting a pid. A stale record left by a killed server is
  cleared rather than reported as a running one.

- **Two designs can be analysed together without either being written to**
  (`compose_and_analyse`, `req:composed-analysis`). The user's framing, and it is
  the better one: to check whether a project and its dependency line up, import
  one design into the other and run reflow2's **ordinary** checks over the whole,
  so seam problems surface as the gaps they already are instead of needing a
  bespoke comparator.

  It cannot be `import_graph`, and the reason is worth stating: `import_graph`
  writes every node under its **original** id with upsert semantics — point it at
  a different design and the dependency's `cmp:store` silently overwrites yours.
  So the other design's ids are namespaced as `{namespace}::{id}`, the combined
  graph is built **in memory and thrown away**, and every finding is attributed
  **ours**, **theirs**, or **seam**. Your export is byte-identical afterwards and
  never starts shipping the dependency's internals. An empty namespace is
  refused rather than allowed to collide by omission.

  This is a third composition mechanism, not a replacement: `mirror_surface`
  imports another design's published surface and keeps it foreign;
  `merge_designs` reconciles two versions of the *same* design; this one analyses
  two *different* designs and persists nothing.

- **An interface can publish its whole contract** (`set_interface_spec`,
  `req:interface-spec-complete`). `Interface` gains `paradigm`,
  `payload_format`, `payload_schema`, `endpoint`, `operations`, `auth`,
  `transport_security` and `error_model` — the things two systems actually have
  to agree on, in a form a computation can compare rather than a free-text blob
  a human has to read. Two designs cannot be checked for incompatibility at a
  seam unless the seam is described in comparable terms.

  A field nobody has recorded reads as **unspecified**, never as **none** — the
  flattering default would tell a consumer that an unrecorded contract is an open
  one. Filling one field leaves the rest and the name alone, so a spec completed
  by several people over time loses nothing. Rate limits, concurrency caps and
  timeouts stay as `Constraint`s bound by `CONSTRAINS`, which already carries
  `quantity`/`limit`/`direction` — duplicating them as interface properties would
  give the same fact two homes.

  These are **property** additions, not new node or edge types, so they do not
  move the version stamp and do not lock out an older reflow2.

## [0.16.0] — 2026-07-27

**Upgrading:** [docs/upgrading-to-v0.16.0.md](docs/upgrading-to-v0.16.0.md), and this one is **not
optional if you share a design**. A new edge type moves the version stamp, so a reflow2 older than
this cannot open a design written by it — loudly refused, never silently half-read. Upgrade every
machine and session that touches a shared graph, together. Working alone on one machine: update and
carry on.

**The release that makes a pile of documents into a design.** `ingest` had existed for months and
was unreachable from a session; now your own agent drives it, and it recovers rationale and test
evidence rather than requirements alone. Around that: near-matches are asked about instead of
guessed, reflow2 can finally say what it has never been told about, and a check can say whether it
was run against a model or against reality.


### Fixed

- **A severed design-history chain is no longer silent** (BL-107). Each committed
  export records the `content_hash` of the one it replaced, giving the design a
  lineage independent of git. `export_graph` builds that link from **whatever
  file is already at the target path** — so exporting somewhere else and copying
  the result into place severs it, which is what happened for six consecutive
  commits here while the gate reported 0 notes every time.

  `reflow2_check.py` now compares an export against the one it replaced and
  fails loud on a break. Two contexts, one rule: before a commit the working
  file's predecessor is HEAD's version; in CI the working file *is* HEAD's
  version, so the pair checked is HEAD against HEAD~1.

  Both ways of being wrong about this are avoided and tested. Unchanged content
  is **not** a break — the chain is not meant to advance — and a first export has
  no predecessor. Outside a git working tree the question is skipped rather than
  guessed, so a project without git can still run the gate.

### Added

- **A check can say WHERE it was run, so a simulation stops looking like reality**
  (`PERFORMED_IN`, `evidence_report`, `req:design-the-simulator`). The argument
  for testing in simulation first is that issues are cheap to fix there and
  expensive in the field — and that only holds if you can still tell the two
  apart afterwards. reflow2 could not: a check run on a rig and the same check
  run in production were both simply `passing`.

  `Environment.env_type` gains `simulation`, a check points at the environment it
  was performed in, and `evidence_report` says which environments proved each
  capability and flags the ones **proven only in simulation**.

  **It reports and never ranks.** It will not claim lab beats staging beats
  field: which of those is "more real" is domain-specific, and an ordering that
  is wrong somewhere gets worked around rather than corrected. And a passing
  check that names no environment is counted as **unplaced**, never assumed
  real — silence is not evidence of the field.

  ⚠️ **This adds an edge type (55 → 56), and that is a harder change than the
  enum growth above.** The version stamp counts node and edge *types*, so an
  older reflow2 will **refuse** to open a design written by this one — loudly,
  not silently, which is the point, but it means every machine and session
  sharing a design must upgrade together. The next release needs its own upgrade
  note saying so; `demonstration` / `observation` / `simulation` are property
  values and do not do this.

- **You can now drive INGEST yourself — no LLM provider involved** (`ingest_step`,
  SP-3b/BL-7). The multi-pass extraction pipeline has existed for months and was
  **unreachable from a session**: it needs an `LlmBackend`, reflow2 ships none,
  and the calling agent cannot be reached mid-op because it is the outer caller.
  So provenance Fragments, time-aware resolution, the resolution bands and the
  structural subset pass all sat behind a door with no handle.

  Call it with no answers; it replies with prompts; answer them in context and
  call again with everything gathered so far, until it reports `done`. Usually
  three or four rounds — later passes are gated on the discovery classifier and
  threaded with the ids earlier ones produced, so they genuinely cannot be asked
  up front.

  **Nothing is written until the last round.** The earlier rounds replay the
  whole pipeline against a throwaway graph, which is safe because every prompt is
  issued before the integrate phase begins — so an abandoned handshake leaves no
  half-design behind, and a test pins it. There is also **no server-side session
  state**: each call is self-contained, so it survives a restart, works across
  seats sharing one server, and cannot leak an abandoned run.

  Prefer it over calling `add_*` yourself for anything document-shaped. That is
  what buys you provenance back to the source text, snapshot-before-overwrite
  when a re-ingest changes something, and the resolution work above.

- **reflow2 can say what it has never been told about** (`coverage_report`,
  BL-95). Every other check reasons about nodes *already in the graph*, so a
  design covering a third of a system reported the same `0 open gaps` as one
  covering all of it — and the unmodelled part is largest exactly where the
  system is largest. In this repository, `merge.rs` and `alternatives.rs` (1,886
  lines, shipped in v0.10.0) sat unmodelled for two days with nothing firing.

  You sweep the tree and supply what you saw — reflow2 does no file I/O, so its
  answer is only ever as wide as your sweep. It replies with the regions no node
  claims, rolled up to the shallowest wholly-unclaimed directory and ranked by
  mass, so the biggest silence sorts first and a vendored tree arrives as one
  finding rather than 900.

  **It is not a score and there is nothing to pass.** An artifact whose location
  is a directory claims everything beneath it, so modelling a vendored mass as
  one opaque unit is *correct* — a file-count ratio would have scored that as
  1-of-901 covered and called the right answer a failure. That trap has its own
  test. Exclusions come back named with the rule that excluded them, because "we
  ignored it" and "it is covered" must never look alike.

  The `adopt` skill now ends by asking, so a thin pass is measured rather than
  felt. What is **not** built, and recorded rather than half-done: the sweep is
  not persisted, so `detect_gaps` cannot yet raise coverage from graph state.

- **Ingest recovers test evidence too** (`[pass:verifications]`) — the last of
  the three things a body of documents was asked for, after requirements and
  rationale. Checks come back with the source's own account of what was done and
  what it found, and a `method` from the eight schema values.

  **They land `planned`, never `passing`.** A document saying "the load test
  passed" is a *claim about a result*, not reflow2 watching it pass — and
  recording it as passing would let prose promote a capability to verified, which
  is precisely the "green while nothing was checked" failure found in this
  project's own code the day before. The claim survives in `description` where a
  person can read it and decide.

### Fixed

- **A check that has not passed no longer counts as proof** (`unverified_capability`).
  The gap skipped a capability on *any* incoming `VERIFIES`, so attaching a
  `planned` Verification silenced the question — which is exactly what the
  detect-and-ask skill already warned against ("a check left at planned does not
  count as confirmation"). The skill said it; the detector did not enforce it,
  and that gap is where a design goes quiet without getting better. It now
  requires a **passing** check, and its evidence line distinguishes "no checks at
  all" from "checks that have not passed".

  Measured before changing: **zero** capabilities on reflow2's own graph were
  riding a non-passing check, so no existing verdict moved. Two test fixtures did
  change, and both were encoding the old behaviour — each asserted "a complete
  thread has nothing to flag" while its checks had never run.

  This is also what makes extracting test evidence safe: without it, ingesting a
  document that mentions testing would have quietly answered reflow2's own
  question about whether anything was proven.

- **Ingest recovers the rationale layer — *why* it was built that way**
  (`[pass:decisions]`). The pass that makes an old body of documents worth
  ingesting at all: reasoning is what a codebase cannot be re-read to recover,
  and it is what leaves when the people do.

  `ingest` extracted none of it before. The discovery gate has classified
  `decisions` all along and nothing consumed the flag — so a document saying
  *"we chose cache-aside because write-through amplified writes"* produced a
  capability and no record of the choice.

  Extracted choices carry their `rationale` in the source's own terms, and each
  `governs_ids` becomes a `GOVERNED_BY` edge from the governed node. An id whose
  type cannot be read from its prefix is **reported and dropped**, never written
  against a guessed type.

  **They land `proposed`, never `accepted`** — pinned by a test, because it is
  the kind of thing a later reader "fixes". An extraction is an agent's reading
  of somebody's document, not the user's signature, and an accepted Decision is
  what `where-am-i` reads back as "what you decided", what the fork layer treats
  as binding, and what the KPP contradiction check reads as a trade already made.
  Requirements from ingest land `proposed` for the same reason.

- **A near-match that is not certain is now a question, not a silent duplicate**
  (`IngestReport.merge_candidates`). `ingest` reads the two thresholds the schema
  has always declared — `fuzzy_threshold` and `auto_merge_threshold` — instead of
  one hardcoded constant, and the band between them finally does something.

  Below `fuzzy_threshold`: a new node, as before. At or above
  `auto_merge_threshold`: merged, as before. **Between them: created *and*
  reported**, so the ambiguous case is put to a person rather than settled by
  arithmetic (`dec:ask-not-repair`).

  The fault was not where it first looked. The foundation's *default*
  auto-merge threshold is 90 — exactly the constant reflow2 had hardcoded — so
  the merging half was accidentally right, and a test pins that reading the
  schema changes nothing about what merges. What was missing was the band below
  it. Measured: **"Auth Service" vs "Authentication Service" scores 84**, so the
  single most common corpus case sat in the invisible band and quietly became two
  components.

  The model comes from storyflow, which has fought this for years
  ([docs/storyflow-resolution-nuggets.md](docs/storyflow-resolution-nuggets.md)) —
  the first finding to travel between the two projects since they diverged.

- **…and the near-matches scoring cannot reach are found structurally**
  (`MatchKind::TokenSubset`). A similarity ratio falls as the length difference
  grows, so `Gateway` vs `API Gateway` scores **74** — below every threshold
  reflow2 declares — while being one of the commonest things a folder of
  documents contains. No amount of tuning reaches it; it needs a different
  question, so when scoring finds nothing reflow2 now asks whether one name's
  words are a strict subset of another's.

  Reported, **never merged on its own**: `Auth Service` is a strict subset of
  `Legacy Auth Service`, and those are plainly two services. The report names the
  longer, more specific side as `suggested_survivor` — storyflow's rule, and the
  non-obvious half, since the naive "keep whichever node has more edges" collapses
  the specific into the vague.

  Names are normalised first (lowercase, punctuation trimmed, grammar words
  dropped). The stopword list is deliberately **grammar only** — extending it to
  `service`, `system` or `module` would collapse `Billing Service` and `Auth
  Service` into the same two tokens.

  Every merge and every candidate now carries a `match_kind`, so when one turns
  out wrong it is clear whether to fix a threshold or a rule.


## [0.15.0] — 2026-07-26

**Upgrading:** [docs/upgrading-to-v0.15.0.md](docs/upgrading-to-v0.15.0.md). A minor bump because
the schema moved; nothing breaks, nothing in your repository changes, and there is nothing for you
to do. An older reflow2 can still *read* a graph written by this one — tested, not assumed.

**Take this one if you run the shared server.** v0.14.0 shipped with a hole in exactly that path:
when the graph was already held and `--http` was given, the explanation went to stdio and nothing
listened on the port, so every session pointed at that URL saw a refused connection —
indistinguishable from reflow2 never having been configured. Found and fixed the same day.

### Added

- **`demonstration` and `observation` are verification methods**
  (`Verification.method`, schema). Anthony's taxonomy, 2026-07-26. Test,
  analysis, inspection and **demonstration** are the four canonical verification
  methods in DoD and INCOSE practice, and reflow2 carried only three of them —
  so "we showed it working", which is how a great deal of acceptance actually
  gets closed, had to be miscoded as `test`. `observation` — watching a system
  run in the field without changing it — is the as-fielded method, distinct from
  inspecting an artifact and from running a contrived example, and had no value
  at all.

  `review` and `simulation` are kept: they are the document and modelled
  sub-cases people already use, and removing enum values would strand existing
  nodes that carry them.

  **This is additive and your graphs are safe, which was proven rather than
  assumed.** A binary built with the previous value set reads a graph containing
  `demonstration` and reports it faithfully; validation runs on write, and the
  version stamp counts node and edge *types* (unchanged at 28 and 55). An older
  reflow2 can therefore still read — it simply cannot write the new values. The
  same call, and the same reasoning, as `DriftEvent.drift_type`'s earlier growth.

  It is still a schema change, so the next release is at least a **minor** bump.

### Fixed

- **The degraded surface now comes out of the door you asked for**
  (`ver:degraded-follows-transport`, BL-105). Shipped broken in v0.14.0 and
  found the same day, by hand, while setting up the shared-server recipe on a
  real machine — no detector caught it.

  With the graph already held by another process **and** `--http` given,
  reflow2 served its one-tool explanation over **stdio** and left nothing
  listening on the port: `main.rs`'s failure arm called `serve(stdio())` and
  never read the flag. So every session pointed at that URL got a refused
  connection — indistinguishable from reflow2 never having been configured,
  which is the precise outage `req:never-silently-absent` exists to end, and it
  had been reintroduced on the transport added two commits later. An operator
  running it by hand fared no better: it died as `failed to start the degraded
  MCP server: connection closed: initialize request`, naming neither the lock
  nor the remedy.

  Both arms now hand rmcp a service factory, so each answers on the transport
  that was requested, and the startup line says which surface it is carrying —
  a degraded server looks like a working one from outside, and "serving over
  HTTP" alone would let an operator walk away satisfied.

  The existing check stayed green throughout because it only ever drove stdio;
  `tools/test_degraded_server.py` contained no occurrence of `http` at all. It
  has four new cases against a real held lock, and they were **mutation-checked
  rather than assumed** — reverted against the v0.14.0 behaviour all four fail
  with *"nothing ever listened on the port the caller asked for"*.


## [0.14.0] — 2026-07-26

**Upgrading:** nothing breaks, nothing in your repository changes, and no schema moved. This is a
minor bump because it adds capabilities, not because it asks anything of you — update and carry on.
If you only ever run one session per project, it changes nothing at all.

**The release that lets several sessions share one design — including across machines.** All three
cases are now covered and, importantly, they are not equally hard: different projects on one machine
never needed anything; the same project on one machine needs a server, because the store is
single-writer *per process*; the same project from **another** machine additionally needs you to
name the host it will be dialled at, because the only thing guarding an unauthenticated server is
the transport's Host allowlist.

Live sharing has a centre — the machine running the server — so it does not replace the git route
for two people working independently, and [docs/collaborating.md](docs/collaborating.md) now says
which to reach for and why. They compose: share live while you are both at it, commit and push the
export when you are done.

### Added

- **Several sessions can share one design, live** (`cap:shared-sessions`,
  `req:sessions-share-a-graph`, `dec:central-host` **accepted**). Anthony,
  2026-07-26: *"I want to have multiple sessions running and being able to use
  the same reflow2 graph."*

  ```bash
  reflow2-mcp --graph-path ./.reflow2/graph --http 127.0.0.1:8787
  ```

  Point every session's MCP config at that address instead of spawning its own
  process. A requirement one session captures is visible to the others
  immediately — no export, no merge, no pull. **Different projects never needed
  this**: each has its own graph directory, so each session runs its own server
  and they never meet.

  The reason it needs a server rather than six processes: the store is
  single-writer **per process**. Six processes cannot each open the directory;
  one process holding it with six sessions attached still has exactly one
  writer, so the constraint is satisfied rather than worked around.

  Two changes underneath. The graph moved from a `Mutex` to an `RwLock`, so
  concurrent reads no longer queue behind each other — and the compiler audited
  the read/write split on the way through: all 32 read sites genuinely need only
  `&DesignGraph`. And **seats are now minted per session rather than per
  process** (`req:seat-per-client`) — without that, a shared server would have
  reported every client as the same owner, and `claim_report` would have told
  six sessions they were each other. That one would have been silent.

  **No authentication.** Bind loopback or a private network; anything that can
  reach the port can write the design.

- **…including sessions on another machine** (`cap:remote-sessions`,
  `req:sessions-across-machines`). Anthony, 2026-07-26, on the third of the
  three cases: *"I'd like to use on my other machine."*

  ```bash
  reflow2-mcp --graph-path ./.reflow2/graph \
              --http 0.0.0.0:8787 \
              --http-allow-host my-desktop.tail1234.ts.net
  ```

  Binding a reachable address was not enough, and the reason is worth knowing:
  the transport answers only requests whose `Host` header is on an allowlist —
  `localhost`, `127.0.0.1` and `::1` by default. That is DNS-rebinding
  protection, and with no authentication on reflow2 it is the only thing between
  a web page you visit and your design. So reaching the server from elsewhere is
  a **deliberate act**: name the host those sessions will dial.

  `--http-allow-host` is repeatable, takes `host` or `host:port`, and **extends**
  the default list rather than replacing it — naming a remote machine can never
  lock out the local sessions already using that server.

  And the failure it prevents is announced rather than discovered: binding a
  non-loopback address without naming a host previously refused every remote
  session with a bare `403` and nothing saying why. The server now warns at
  startup and names the flag that would have worked.

  Proven against real servers, with the `Host` header a remote session would
  actually send: an unnamed host refused (for *that* reason, not merely with
  that status), a named host completing a whole session, a remote seat and a
  loopback seat sharing one design, and the advisory firing on a wildcard bind
  but staying quiet on a loopback one — 5 cases, in CI.

## [0.13.0] — 2026-07-25

**Upgrading:** [docs/upgrading-to-v0.13.0.md](docs/upgrading-to-v0.13.0.md). Nothing breaks and
nothing in your repository changes; there is one new file beside your graph, and re-running the
installer registers the session-end nudge you did not have.

Also in this release: importing a whole design into an **empty** store now takes that design's
name — a restore is the same design in a new store, and without this the export round trip stopped
coming back byte-identical (`graph_id` is inside the content hash). A store that already holds a
design keeps its own name, which is what makes absorbing the shared record safe. Caught by the
smoke test the hour identity landed.

### Added

- **The loop's own safety net is checked, and its absence is announced**
  (`cap:nudge-path-proven`, `req:nudge-path-proven`). The Stop hook is the only
  trigger that fires when an agent has stopped calling anything, which makes it
  the one that matters most — and nothing verified it. `test_loop_nudge.py`
  covered the script's logic given its inputs and passed happily the whole time
  nobody had checked the hook was registered.

  `tools/test_nudge_path.py` now reads `.claude/settings.json`, takes the command
  the harness would run, and runs **that** with the JSON a real Stop hook
  receives — asserting the `{"decision":"block"}` the harness actually consumes,
  that the reason names what happened, that a session which ran the loop check is
  left alone, and that it fires **once**.

  And the backstop, which matters more than the proof: the server reports
  `installed` / `absent` / `broken` (registered but the script is missing — the
  dangerous middle case, because the settings file *looks* right) / `unknown`
  (never reported as absent — claiming a net is missing when we only failed to
  look is the same lie in the other direction). When it is missing, the advisory
  rides the **handshake instructions**, the one channel every session reads
  unasked, and `loop_status` carries it as a field.

- **The installer registers the nudge**, closing the finding that check turned
  up: until now `reflow2_init.py` wired no hooks, so every consumer project ran
  with no session-end backstop. It goes in **`.claude/settings.local.json`**,
  not the shared `settings.json` — the command carries an absolute path to *your*
  kit, and a collaborator inheriting it gets a hook that fails silently, which
  is the `broken` state above and the worst of them. It points at the **kit's**
  script rather than a copy in the project, so it updates with the package and
  nothing in your repo goes stale. Merged, never clobbered: your own hooks and
  settings survive, a nudge you repointed is left alone and reported, and a
  second run does not stack a duplicate.

- **A claim names the session that made it, and a claim nobody is working says
  so** (`cap:claim-liveness`, `req:claims-have-owners`). Claims record a **seat**
  — `machine:pid:mint`, minted once per process with zero coordination — and
  `claim_report` **computes** liveness by asking the operating system whether
  that process still exists. Nothing writes "I am alive", so nothing can be
  stale about it.

  A claim whose session has exited is reported `gone`, listed in `stale` with
  its note intact, and **kept out of `overlaps`**: a collision with nobody is
  not a collision, and reporting it as one is how an advisory report starts
  lying — people wait for somebody who left. A claim from another machine, or
  from before seats were recorded, is `unknown` and *still counts* as a possible
  collision, because reading it as free would invite someone to take work that
  is actively being done.

  Schema: `CLAIMS.seat`, additive. `claim_region` takes an optional `seat` for
  callers with a durable session handle (a fleet worker name); it is a name,
  never a lock.

- **A design knows its own name** (`cap:design-identity`, `req:design-identity`,
  `dec:identity-out-of-band`). Every reflow2 graph used to answer to one
  hardcoded id, so **no design could tell another design from itself** —
  `mirror_surface` refuses a surface whose source is the importing graph, and
  with a single constant that guard could never pass for any pair of real
  designs. Composition between designs was impossible on disk.

  An id is now established on first open and read on every one after, from
  `<graph-path>.id.json` — a sibling of the store, because **the id namespaces
  every stored key**: it has to be known before the design can be read. It is
  minted with zero coordination (creation nanosecond, process, absolute path)
  and no new dependency; the friendly label is a changeable layer on top, and
  `design_identity` reads it or renames it. An unreadable identity is **refused,
  never defaulted** — defaulting opens a *different* design at the same path and
  reports nothing wrong.

  **Existing graphs keep the name their data is stored under.** A store that
  already holds a design under the old shared id adopts it, forever. Minting for
  those would have left the design on disk and opened a new empty one beside it;
  it is also what keeps every existing export valid, since `graph_id` is inside
  the export's content hash.

### Changed

- **The MCP configs are gitignored, and the graph path in them is relative.**
  Both are machine state — every config carries an absolute path to *this*
  machine's binary — and reflow2's own repo has ignored them from the start,
  while a consumer project did not. Committed, they reach a collaborator
  pointing at a binary that does not exist there, and the installer then
  correctly refuses to repoint an entry somebody may have customised, so they
  get a loud line they have to notice and act on.

  The relative graph path (`.reflow2/graph`) fixes the case people actually hit:
  **several sessions on one machine**. An absolute path copied into a second git
  worktree points both sessions at the same store, so the second loses the
  single-writer lock and gets the degraded server; relative, each worktree opens
  its own. The binary path stays absolute — there is no PATH to rely on.

  A config git already tracks is **reported with the fix** (`git rm --cached
  .mcp.json`), because ignoring a tracked file changes nothing until it is
  untracked, and saying "ignored" without saying that would be a half-truth.

## [0.12.0] — 2026-07-25

**Upgrading:** read [docs/upgrading-to-v0.12.0.md](docs/upgrading-to-v0.12.0.md) first. This is the
first release that *removes* files from a consumer project, and the last one that needs to touch a
consumer project at all.

### Changed

- **The skills are served by the server, not copied into your project**
  (`cap:skills-served`, `dec:skills-served`) — **minor**, and the one release that
  *removes* files from a consumer repo. See
  [docs/upgrading-to-v0.12.0.md](docs/upgrading-to-v0.12.0.md).

  Alex's feedback: setup should be a paragraph in the instructions file plus an
  MCP entry, after which *"you wouldn't need to change anything in your repo
  again and updates would be confined to the reflow package."* He was describing
  a defect the installer's own docstring already conceded — the kit *"is copied
  into your project, so it otherwise freezes at install time while reflow2 keeps
  moving"* — and which had already bitten in the least visible place: reflow2's
  installed manifest read 0.8.0 with twelve skills while the project was at
  0.11.0 with fifteen, four releases running, unnoticed.

  Skills **and the ~20 KB working-instructions document** are now compiled into
  the binary (`build.rs` embeds `getting-started/`), served by **`list_skills`**,
  **`get_skill`** and **`get_instructions`**, and
  advertised by a catalogue — plus a call-`get_instructions`-first line — in the
  handshake instructions, the one channel a client puts in the agent's context
  unasked. What an install now leaves in a project is a **2.4 KB pointer file**
  naming those three tools, the MCP configs and `.reflow2/`: nothing a later
  release rewrites. `reflow2_init.py` copies no skills,
  and on the first run after upgrading it removes the copies an older kit left:
  untouched files deleted *with the reason*, edited files kept and reported as
  **shadowing** the served skill, because a harness does auto-load those.

  The trade, stated rather than buried: a harness-native skill is auto-matched
  from its description without the agent asking, and a served skill is not. That
  is the price of never being stale, and it was accepted deliberately.

### Fixed

- **An export that would delete the other seat's work is refused**
  (`cap:stale-seat-refusal`, `req:stale-seat-knows`). The hazard git answers with
  a non-fast-forward refusal, one level down — and worse here, for one reason:
  **a stale export is not a conflicting export, it is a complete one.** A
  session's graph is a long-lived copy of the committed design; export from a
  graph that never caught up and the document you write is internally perfect
  and simply older. The merge driver finds no conflict (there is none), and the
  other person's requirements are gone with nothing in the diff that looks like
  an error.

  Before replacing an export, reflow2 now asks whether the write would **drop**
  anything the file holds. The file where you left it is written silently — one
  hash comparison against a marker in `<graph-path>.sync.json`. A file that moved
  but loses nothing is written, with the movement reported. A write that would
  remove nodes or edges is **refused**, naming the ids, the three-step remedy,
  and `accept_divergence=true` for discarding that work on purpose. The check is
  deliberately narrow: a check that fired on every ordinary export would be
  passed by habit within a day and would then protect nobody.

  `import_graph` gained a **`path`** argument in the same change, so the remedy
  the refusal names is one the tool can actually perform — and importing a file
  records the sync, which is what clears the refusal.

- **A session that cannot open the graph now says so instead of vanishing**
  (`cap:degraded-surface`, `crates/reflow2-mcp/src/degraded.rs`). Reported from a
  six-session StoryFlow fleet on 2026-07-25: the store is single-writer, so the
  first session won the lock and the rest **died at startup, before any tool
  existed**. What they saw was not an error but *nothing* — zero `reflow2__*`
  tools — and, in one boss's words, *"nothing distinguished this from 'reflow2 was
  never configured for this project'"*. reflow2's own good diagnosis went to stderr
  and died with the process; one session went on to report the project as having no
  design brain.

  Any open failure now serves a handshake with the translated reason **in the
  server instructions** (which the client puts in the agent's context) plus exactly
  one tool, `reflow2_unavailable`, carrying the reason, the remedies and an explicit
  *do not conclude reflow2 is missing*. Same fix covers the other cause of the same
  silence: a graph refused for schema-version skew. Nine cases in
  `tools/test_degraded_server.py`, measured from both sides of a **real** held lock.

- **`--export-snapshot` reads a graph another session is holding.** The second
  field blocker: a locked-out seat could not so much as export the design, and
  export is where the whole per-seat merge workflow starts. Copies the store's flat
  files to scratch (skipping `LOCK`), exports the copy, removes it **and its
  provenance sidecar**. Loudly labelled best-effort — it is a live-database read,
  not a backup — and a graph that is *not* locked gets an ordinary export and is
  told so.

- **A bare hex checksum no longer reads as total drift** (`canonical_checksum`).
  Drift is a *string* comparison and the gate observes `sha256:<hex>`, so an
  artifact registered from raw `sha256sum` output was identical on disk and 100%
  drifted at the same time. Found by reflow2's own coherence gate reporting four
  false reds in one session. A bare digest is now stored canonically as
  `sha256:<hex>`; anything carrying another algorithm, or not hex at all, is stored
  verbatim — this normalises a known dialect, it does not police the field.

### Added

- **Designs compose by mirroring, and the mirror carries a coordinate**
  (`cap:mirror-surface`, `mirror_surface` / `mirrors`). `dec:nested-graphs` is
  **decided**: option (c) — *a graph per ownership boundary, levels inside each*. A
  design is its own graph when something is separately **owned**, **released** or
  **shared**; the hierarchy does not decide, authority does.

  An edge cannot cross a store (the schema validates both endpoints), so linking
  designs is not an edge problem: the other side's published surface is mirrored in
  as **local nodes marked `imported`**, and the mirrored Project carries
  `mirror_of`, `mirror_content_hash` and `mirrored_at`. Your components then
  provide/consume the mirrored Interface with **ordinary local edges** — so the
  golden thread, propagate and every detector keep working, and foreignness is a
  property of the *node*, never of the link. Crossing the seam even reports as a
  published-boundary crossing, which is exactly what changing your side of someone
  else's contract is.

  **Collisions are refused, not merged.** `import_graph` is an upsert, so mirroring
  a surface whose ids collide would silently overwrite your design with someone
  else's nodes. A collision leaves your node untouched and is named; an edge
  touching a collided id is dropped rather than rewired, because pointing their
  `PROVIDES` at your same-named component would fabricate a relationship neither
  design asserted. Mirroring a design into itself is refused outright.

  Schema: `Project` gains the three mirror properties (additive, no new types).

### Found while building

- **Every reflow2 graph carried the same hardcoded `graph_id`**, so no design could
  tell another from itself — `rule:no-foreclosure` item 5 arriving as a concrete
  blocker rather than a hypothetical. `DesignGraph::open_in_memory_as` lets a
  design name itself, which is enough for library and test use. The durable case is
  filed as `req:design-identity` and deliberately **not** half-built: the id
  namespaces every stored key, so a graph reopened under the wrong name would
  present an empty design.

- **`export_surface` — publish just the boundary** (`cap:publish-surface`). The
  contracts others are entitled to rely on, and nothing internal: every Interface
  designated `published`, the artifacts that specify or realize it (the real ICD),
  the components on each side, and the project. Requirements, capabilities,
  decisions, verifications and history stay home.

  **It counts and names what it withheld.** A recipient cannot tell a small design
  from a heavily filtered one, so the note says which they are holding — and says
  what the document is *not* ("not a backup"). A design with no designated boundary
  gets an `EMPTY SURFACE` warning rather than a quietly empty file, because that
  case is indistinguishable from having nothing to share and someone could publish
  it believing otherwise. Not refused, though: *"prove I publish nothing"* is a
  legitimate question.

  Deliberately **not part of the export hash chain** — a derived view must not read
  as an ancestor of the full design. That constraint came from running impact-check
  *before* writing any code: `dec:export-hash-chain` was in the direct ring.

  This is the first piece of `req:design-composes`, chosen because every answer to
  the open federation question needs it: whatever composes, it composes through a
  published boundary rather than by reaching into another system's internals.

- **reflow2's own boundaries are now declared**: `ifc:mcp-tools` and
  `ifc:graph-export` are `published`; `ifc:core-api` stays internal, because it is
  the seam between reflow2's own modules and no consumer touches it. Impact
  analysis now reports crossings against them.

## [0.11.0] — 2026-07-25

**A minor bump: two schema changes, and the day reflow2 was used hard enough on
itself to find its own friction.** Two behaviour changes want reading before you
upgrade — see [docs/upgrading-to-v0.11.0.md](docs/upgrading-to-v0.11.0.md).

### Changed

- **A new Decision lands `proposed`, not `accepted`** — a behaviour change to a
  shipped tool, on Anthony's call, from friction found using reflow2 on itself.
  Recording a choice is not the same act as settling it: with the old default,
  every open question landed as *settled and reasoned*, which is the forgery
  `dec:certainty-derived` forbids for requirement status, with more consequence —
  an accepted Decision is what **where-am-i** reads back as "what you decided",
  what the fork layer treats as binding, and what the KPP contradiction check
  reads as a trade already made. Six corrections in one session, and the
  brainstorm skill had to carry the workaround in prose. Two of reflow2's own
  tests failed on the flip and both were right to — each had leaned on the default
  instead of stating its intent. Existing graphs are unaffected (defaults apply at
  write time). See [docs/upgrading-to-v0.11.0.md](docs/upgrading-to-v0.11.0.md).

### Added

- **Published boundaries, and severability computed rather than asserted**
  (`cap:key-interfaces`, `req:key-interfaces`, `req:modularity-computed`).
  `Interface.designation` is new — `internal` (default) or `published` — plus
  `set_interface_designation`. `published` marks a contract others are entitled to
  rely on: what an ICD publishes, and what MOSA calls a modular system interface.
  Default internal on purpose, because publishing is a commitment nobody should
  have asserted for them.

  **It is read, not just stored.** `propagate_from` now reports
  `boundary_crossings` — the published Interfaces a change passes through, *named*
  rather than counted, in both the full radius and the summary, with
  `crosses_published_boundary` per impacted node. So "is this part severable" is
  computed: a change either stays behind the design's published boundaries, or the
  report says which contract carried it and therefore whom to talk to.

  Two independent routes had asked for exactly this property — MOSA's designation
  discipline, and BL-45's system-of-systems thread five days earlier ("nothing
  marks an Interface external-facing").

  A test caught a real bug: the first implementation counted a boundary *one hop
  past the depth bound* as crossed, so an internal change reported crossing a
  contract the walk never reached. Also pinned: seeding a change **on** a
  published interface is not a crossing (you are changing the promise, not passing
  through it), and withdrawing a designation removes the crossing, because the
  computation follows the design rather than remembering it.

- **Four friction findings filed as graph elements** rather than as backlog prose,
  from the first real run of the **report-friction** skill:
  `req:decision-status-not-asserted` (fixed above), `req:reviewed-defects` (the
  open defect list should mean still-needs-attention, the way `reviewed_gaps`
  already does), and two `planned` Verifications for checks that do not exist —
  that a served tool rejects an argument it does not know, and that the installed
  kit's manifest agrees with the kit it claims to be. That last one found
  reflow2's own `.reflow2/kit-version.json` four releases stale with stale
  per-file hashes, unnoticed.

- **Four requirements promoted out of two brainstorms** (Anthony, 2026-07-25).
  The nested-graphs and MOSA ideas stop being musings and become intent, while the
  two decisions stay open on *how* — because promoting an idea and choosing an
  architecture are different acts.

  - **`req:analyse-at-any-level`** — any level of the design can be analysed on its
    own, and a narrowed answer names what it withheld. **Already delivered** by
    `cap:scoped-analysis`, so this requirement arrives satisfied: it names the
    intent behind the feature built an hour earlier.
  - **`req:design-composes`** — a system's design can be a unit of its own that
    composes with the designs around it, linked by interface specifications rather
    than by everyone reading everything. Accepted and **blocked** on
    `dec:nested-graphs`: the three options need materially different machinery, and
    guessing would build for the road not taken.
  - **`req:key-interfaces`** — the design says which interfaces are published
    boundaries versus internal plumbing, *and computations read the distinction*.
    Wanted independently by MOSA's central discipline and by BL-45's
    system-of-systems thread; open work, and the next rung.
  - **`req:modularity-computed`** — severability and cohesion are computed from the
    graph rather than asserted by the architecture diagram: if the blast radius of
    a change inside a part escapes that part's published boundaries, the part is
    not modular whatever the diagram says. Sequenced behind key interfaces, which
    is what tells the computation where a boundary is.

  All 39 requirements are now user-confirmed, and the gate carries two honest notes
  — the two pieces of open work above.

- **Scoped detection — a team can ask about its own part of the design**
  (`cap:scoped-analysis`). `detect_gaps` and `detect_defects` now take `scope` (a
  node id) and `depth`. From Anthony's satellite case: a program with space,
  ground and control segments, where his team owns inter-satellite laser comms and
  needs *its* gaps day to day, not the program's. An unscoped detector on a
  program-sized graph is the unbounded-read failure one level up — complete, and
  so unusable that people stop looking.

  **A scoped answer always says what it left out.** Every finding lands in exactly
  one of four buckets that sum to the total: `in_scope`, `out_of_scope`,
  `unanchored` (findings that belong to no part in particular — the lifecycle-phase
  gaps), and `project_level` counted within in-scope. Project-wide rollups still
  reach a team when they touch that team's work, carrying their own `scope` so the
  reader can see whose finding it is; hiding them would be the tool deciding what a
  team may worry about.

  Two findings, both caught by tests rather than review. **A scope is not a blast
  radius**: the first implementation reused the propagation radius, and the test
  that scoped to a Project and expected the whole design failed, because `CONTAINS`
  is deliberately excluded from the traceability rules. That exclusion is right for
  impact (a change to a segment does not implicate every screw in it) and wrong for
  ownership (a segment lead owns what is inside it), so a scope is now containment
  closure — unbounded, since ownership does not attenuate with distance — followed
  by the traceability radius. And the first filter **silently dropped the
  unanchored gaps**, which is the exact silent drop the feature exists to prevent;
  hence the fourth bucket.

  Recorded, not fixed: `claim_region` still uses the radius alone, so claiming a
  segment does not claim the subsystems inside it. That may be a defect in the
  claims layer, but changing what a claim covers changes what two people believe
  they hold.

- **A `brainstorm` skill — think an idea through without committing it** (15 skills
  now, plus a `/brainstorm` command). Anthony's brother's original "rubber-ducking"
  ask, reframed by Anthony into what it actually is: not a staging gate but a *kind
  of record*. Ideas enter the graph immediately as `proposed` Decisions named as
  open questions, with the options in the user's own words and the honest
  counter-argument beside each. Nothing waits in a buffer where it could be
  forgotten, and nothing is claimed as intent.

  **The mechanism turned out to already exist.** `detect_gaps` raises
  `undecided_decision_point` only on a proposed Decision holding **two or more
  registered alternatives** — a fork with a real design behind each branch. So a
  Decision whose options live in prose raises no gap at all: the loop stays quiet
  while you are thinking, and starts asking the moment `register_alternative` turns
  an option into a fork. No label, no schema change, no upgrade doc — and it
  shrinks the open vocabulary question in `dec:exploratory-staging` from four
  arguments to two small ones.

  The skill ends with a **promotion** step rather than a commit: chosen ideas go
  through capture-intent, and the rest stay recorded as considered rather than
  deleted. Its guards are prose because none of them is machine-checkable — don't
  create requirements mid-brainstorm, don't argue an idea down (record the
  objection beside it), don't run gap detection over brainstormed nodes, and never
  promote an idea for being the last one standing.

- **reflow2 is now git's merge driver for the design export** — so two people
  editing *different* parts of one design stop colliding. The export is a single
  large JSON file and git merges it by lines, a unit that means nothing to a
  graph; the semantic merge already existed (BL-80), and this is the adapter that
  makes git call it.

  `reflow2-mcp --merge-driver %O %A %B`, wired by `.gitattributes` and one
  `git config` per clone (git will not let a repository configure an executable).
  Git's contract exactly: a clean merge is written to `%A` and exits 0; a real
  both-sides conflict exits non-zero **without touching `%A`**, printing each
  conflict id, its question, and the `--merge-apply` command that finishes it.
  Nothing is auto-decided — only what one-sided changes make derivable.

  Tested **by git**, on real branches (`tools/test_merge_driver.py`, in CI's full
  job): disjoint edits merge with no human, both-sides conflicts leave the path
  unmerged and name themselves, additions from each side both survive, and a clone
  without the config degrades to git's text merge instead of failing.

  Plus a **parallel-work** skill (14 skills now): claim the region, work in a
  worktree with its own graph — the store is single-writer *per directory*, so two
  people can each run a server — export before every commit that touched the
  design, let the driver merge, release the claim, reconcile. It is required to say
  the uncomfortable parts out loud: a claim is advisory and invisible until a pull,
  the *code* still merges the way it always did, and `--ours`/`--theirs` on a design
  conflict silently discards a node someone wrote.

- **Three imports from the GitHub MCP Server study**
  ([docs/github-mcp-nuggets.md](docs/github-mcp-nuggets.md)) — a hosted MCP
  server at very large scale, read for what a design brain should take and what
  it should refuse.

  **A trust boundary at ingress** (`cap:sanitize-ingress`). "Graph text is data,
  never instructions" now has a mechanical half, not just a line in every skill:
  text arriving from outside the session is stripped of Unicode tag characters,
  bidirectional overrides and hidden formatting — the channels that make text
  read one way to a person and another to a machine. Wired into INGEST's single
  integration point. **It reports rather than sanitising silently**: the class
  and count of what was removed lands in `IngestReport.warnings`, naming the node
  and the field, because a design whose statements were quietly rewritten is a
  design nobody can audit. Zero-width joiner is kept (emoji sequences need it),
  no HTML stripping (a design may legitimately say `Vec<Component>`).

  **Bounded reads** (`cap:bounded-reads`). `scan_nodes` now answers with as many
  nodes as fit in one reply and *says what it left out* — `total`, `returned`,
  `omitted`, `next_offset`, and `capped_by` (`size` or `limit`) — plus
  `brief: true` for id/name/status only. This closes a real failure: a read of 72
  Decisions returned 96,000 characters and the client truncated it, so the drop
  happened where reflow2 could not name it. `count` keeps its old meaning, and a
  single node larger than the whole budget is still returned.

  **`find_tools`** (`cap:tool-search`). Search the served surface by describing
  the job — "register a file that realizes a capability" finds `link_artifact`.
  Scored over the router itself so the catalogue cannot drift from the tools that
  exist; exact names rank first, ties break by name, and a miss is reported as a
  miss.

  Recorded alongside them: **`rule:no-foreclosure`**, a DesignRule holding the
  six shortcuts *not* to take if reflow2 is to grow into a hosted multi-user
  service — identity in a global, inventing authority, treating the single-writer
  lock as a contract, per-request config that widens rather than narrows,
  assuming one graph, and trusting text by its location. It is `enforced: false`
  until Anthony says whether it should be gate-blocking.

- **The capture half of KPPs — the agent notices, the user decides** (BL-96,
  `cap:kpp-proposal`). A new **kpp-proposal** skill: when something you said
  carries a threshold, a "shall", or a consequence you described as fatal, the
  agent asks whether it is inviolable — and asks it as a question you can answer
  without the vocabulary ("if it came in at 450 instead of 500, would that sink
  the project?") rather than "is this a KPP?". It never promotes on its own,
  because criticality is a claim about *consequence* — mission, contract, money —
  which is not in the graph and not visible to an agent. A `/kpp` command drives
  the same skill when you want to state one outright.

  `add_constraint` now takes **`objective`** and its description names the `kpp`
  category. That was not cosmetic: before this, `objective` had no writer on the
  MCP surface at all and the category was undocumented, so the only way to record
  a confirmed KPP over MCP was the generic `create_node` — which is exactly how
  `tests/kpp.rs` had to build its fixtures. A capture skill without it would have
  pointed the agent at a door that was not there. `objective` is never defaulted;
  three new tests pin that, and that the objective is never mistaken for the
  threshold (missing it is disappointing, missing the threshold is fatal — only
  the second is a breach).

  Also new: **[SKILLS.md](SKILLS.md)**, the catalogue of which skill and which
  slash command to reach for, linked from the README and the consumer kit. It
  says plainly what is *not* there yet, including that the slash commands are not
  installed into a consumer project.

- **Advisory claims — see who already has a region of the design in hand**
  (BL-44). `claim_region` / `release_claim` / `claim_report`, plus a `CLAIMS`
  edge (Contributor → any node) carrying depth, timestamp and a note.

  **A claim is not a lock and cannot be.** The design lives as a file in each
  checkout with no shared server (`dec:multi-writer-architecture`), so there is
  nowhere for a lock to live. Nothing refuses a write, nothing consults a claim
  before allowing one, and a second writer who ignores a claim gets a correct
  three-way merge exactly as if it did not exist — pinned by a test, because a
  claims layer that *reads* like locking is worse than none.

  The region is **computed** from a seed and a depth, never stored as a node
  list, so it follows the design as the design changes. Overlaps are **reported,
  never prevented**: two people may claim the same ground, both claims stand, and
  `claim_report` names the shared nodes ranked by size. Two claims by the same
  person are not a collision. The advisory limit ships in the payload, not only
  the docs — whoever reads it over the wire never sees the docs.

  Schema: edge types 54 → 55, so a graph written by this build is refused by
  older binaries with "update reflow2".

- **Drift now says which WAY the design and the build diverged**, not just that
  they did. `reconcile_artifacts` accepts an optional `realizes` per observation
  — what the caller observed a file actually implementing — and compares it
  against the recorded `REALIZES` edges. More than recorded is **understated**,
  less is **overstated**, each-having-what-the-other-lacks is **diverged**. The
  finding carries `unrecorded` and `unbuilt`, naming the specific design nodes,
  which is the answer to "what does my design now claim wrongly?" that a
  checksum could never give.

  Motivated by a field observation: reflow2 running on a large project reported
  that its docs "consistently understate what's built". A file that grew a whole
  subsystem and a file with a typo fixed were the same `checksum_change`, so
  understatement was invisible exactly where it was largest.

  Direction is judged **independently of the checksum** — a design can be wrong
  from the day it was written, and long-lived untouched files are where
  understatement accumulates. Overstatement ranks above understatement and above
  plain change: understatement is a record that is behind, overstatement is a
  record that is *wrong*, and someone will plan against it. Omitting `realizes`
  means "not assessed" and is never read as agreement, so existing callers are
  unaffected. No schema change — understated records as `undocumented_addition`
  (which is what it is, one level down) and overstated as `spec_mismatch`.

- **`graph_report` now says how much intent is actually DELIVERED**, derived from
  the golden thread instead of read from `Requirement.status` (BL-104). A
  requirement counts as delivered when something satisfies it, that capability is
  realized, and it currently carries a passing check — at its own or component
  granularity. Two properties make it a derivation rather than a slower
  assertion, and both are pinned by tests: it goes **backwards** when a check
  starts failing, with nobody editing anything; and requirements whose own
  provenance is `inferred` are excluded and counted separately, because a
  requirement read back out of the code implementing it is satisfied by
  construction — without that guard a brownfield adopt would report itself fully
  delivered on arrival. Dropped requirements leave the denominator: abandoning a
  need is not failing to deliver it.

### Added

- `--merge-apply` CLI mode (BL-80): the file-pure half of three-way merge. `--merge base ours
  theirs` prints the conflicts and their ids; a JSON decisions file maps each id to
  `base`/`ours`/`theirs`; then `--merge-apply base ours theirs --resolutions FILE` runs
  `resolve_merge` and prints the merged export document to stdout — opening no graph (so it runs
  while a server holds the lock) and refusing (non-zero exit, no output) until every conflict is
  decided. The document-in/document-out sibling of the `apply_merge` tool (which commits into the
  live graph), completing git's merge workflow over export files.

### Design (no code change)

- The **fork layer designed** (BL-70) and recorded in the design graph: three decisions —
  `dec:fork-point-address` (a fork point is the coordinate Decision → epoch → export
  `content_hash`, resolved against git, rather than a native ref/branch layer),
  `dec:reopen-supersedes` (re-opening a settled choice mints a new Decision that obsoletes the
  original, which is never un-accepted), and `dec:temporal-backfill-from-releases` (the epoch chain
  is backfilled only from real shipped release tags).
- The **epoch chain backfilled and anchored**: 12 epochs from genesis to the current work, chained
  with `PRECEDES`, carrying the export `content_hash` for the three releases whose exports embed one;
  all 34 Decisions and all 9 Releases pinned to the epoch the git evidence puts them in. Found while
  doing it: the temporal axis was nearly unused — no Decision was anchored to any epoch, and there
  were no Snapshots at all, because `add_change_event` had been used throughout where `record_change`
  was meant.
- `merge.rs` and `alternatives.rs` **modelled at last** as `cmp:merge` / `cmp:alternatives` under
  Time & History, with their capabilities moved off the `cmp:compare` stand-in and both files
  registered as checksummed artifacts. This dissolved the long-standing disconnected-community
  defect; the remaining five are the accepted single points of failure.

## [0.10.1] — 2026-07-24

### Changed

- **The schema stamp records *which* types it carried, not just how many** (BL-86, the real fix —
  **patch**; the `.meta.json` stamp gains two additive, backward-compatible fields). The upgrade
  check refused a graph whose type *count* exceeded the binary's — but a count can't tell "uses a
  type I **retired** → migrate the graph" from "uses a type I've **never heard of** → you're
  behind." So retiring `VALIDATES`/`ENABLES` (55 → 53 edge types) made a pre-removal graph get
  refused with advice to *update the binary it was already on*. Now `GraphStamp` records the sorted
  type-name sets, and the refusal names the exact offending types and gives the right path for each
  (retired → migrate the graph; unknown → update reflow2), via a small retired-types registry.
  Legacy count-only stamps still parse (`serde(default)`) and get a sharpened message — an excess
  the retired types fully explain now leads with migration instead of hedging. Closes the open half
  of BL-86 (the message half shipped earlier).

### Fixed

- **The design-coherence CI gate now actually runs** (**patch** — a silent CI failure made loud).
  `reflow2_check.py` spawns the `reflow2-mcp` binary, but the gate step had been placed in the
  no-RocksDB `core` job, which never builds it — so every push hit `FileNotFoundError` on that step
  (job red) and the gate never ran in CI, while everyone read "gate green" from the *local* run.
  Moved to the `full` job, right after the binary is built. Found while cutting v0.10.0.

## [0.10.0] — 2026-07-24

### Changed

- **Reads now surface coherence-loop debt at the moment of attention** (BL-91; **minor** — the
  orientation reads' result shape gains an optional `loop_hint` field, plus a new capability and
  graph nodes). The write tools have carried a static `loop_hint` since BL-74; reads carried
  nothing, so the only mid-session reminder was the agent's own discipline. Now `graph_report`,
  `graph_report_markdown`, `scan_nodes`, `search_design` and `get_node` attach a `loop_hint`
  **only when `loop_status` reports real debt** (never static-every-read — the boilerplate
  anti-pattern BL-90 rejected) and **only when the owed-set has changed since it was last surfaced**
  (fire-on-change). It is the mid-session trigger between SessionStart (fires once) and the Stop
  nudge (fires at the end), landing on the agent's most frequent call. `dec:read-hint-shape` option C.
  - **Cost is bounded structurally:** the owed-set changes only on a write, so a service
    write-generation counter gates the recompute — within one generation the first orientation read
    computes `loop_status` once and later reads add nothing. Debt is always read from current state,
    never remembered (`dec:loop-status-state-not-history`); only the *presentation* is throttled.
  - Modeled: `cap:read-loop-hint` SATISFIES `req:read-surfaces-debt`, ALLOCATED_TO `cmp:service`,
    REALIZED by `art:service`, VERIFIED by `ver:read-loop-hint` (new `tools.rs` cases). Closes the
    last open gap (`req:read-surfaces-debt` unsatisfied) and dissolves the read-hint
    disconnected-community defect. `chg:bl91`.
  - **This caught a latent bug in reflow2's own tooling:** `tools/reflow2_check.py`,
    `reflow2_cli.py` and `smoke_mcp.py` unwrapped the `{count, items}` list envelope by *exact* key
    set, so the additive `loop_hint` broke the unwrap and crashed the gate. Now they match by
    presence — the documented envelope convention the `jl!` test macro already used. `art:check`
    reconciled (design_holds).

- **reflow2's own CI gate and view renderer now have hermetic regression suites — and writing them
  caught a real gate bug** (BL-88; **patch** — the fix turns a silent miss loud). `tools/reflow2_check.py`
  (the consumer coherence gate, BL-66) and `tools/render_views.py` (the viewpoint renderer) had no
  tests and reflow2's own gate was the one thing with no net under it.
  - **Bug the new suite caught:** the gate is documented to fail when a registered artifact "changed
    **or vanished** with no two-sided accept," but it only matched the reconcile kind `"missing"` — and
    reconcile emits `missing_artifact` (severity *high*). So a registered file that *vanished* was
    silently downgraded to a note, never turning the build red. Fixed to match `missing_artifact`; the
    gate now fails on a vanished artifact as it always claimed to.
  - **`tools/test_reflow2_check.py`** (in CI's `full` job) drives the real binary to build tiny designs
    and pins the gate's whole contract — its exit code — across the trio it was hand-verified against:
    coherent-passes (0), missing-export-cannot-run (2), tampered-fails-integrity (1), plus both drift
    shapes (changed file, vanished file → 1) and no_baseline-is-a-note (0).
  - **`tools/test_render_views.py`** (in CI's `core` job, pure-Python file form — no binary) pins the
    *projection* doctrine: the renderer emits only what the graph states and **confesses** what a
    viewpoint needs but the graph lacks (an unsatisfied requirement is confessed, a satisfied one is
    not; no Project is confessed; a decision's rationale is projected verbatim).
  - `render_views.py` is now modeled (`art:render-views` realizing `cap:report`, governed by
    `dec:views-are-projections`); both suites are registered as passing `Verification`s.

- **Three adopt-scale ergonomic tweaks from the BL-83b dogfood** (BL-89; **minor** — one new
  optional tool param, no schema change):
  - **`describe_schema` gains `required_only`** — with `node_type`, returns just the properties a
    `create_node` must supply and omits the (large) edge lists, so an adopter reading many types at
    scale isn't pushed back to `schema/*.yaml` for "what's required."
  - **`unreleased_component` follows containment** — a Release that `INCLUDES` an assembly now covers
    its `CONTAINS`-children, so shipping a subsystem no longer needs an explicit `INCLUDES` per leaf
    (the operate layer stopped being an 11-gap flood in the dogfood). Same "an assembly speaks
    through its children" rule `dead_end` and the community detector already carry; a built component
    *outside* any shipped assembly still fires.
  - **Adopt-skill granularity guidance now keys off contracts/capabilities, not LOC** — node count
    tracks how many distinct things a system does and exposes, not its size (reflow2 is ~34k LOC yet
    ~100 nodes; a 110k-LOC system was ~78), so the skill says to size the model by counting contracts
    and capabilities rather than an LOC ratio.

- **`import_graph` accepts an unstamped document instead of refusing it** (BL-87; **minor** —
  `GraphExport.stamp` becomes optional and `ImportReport` gains a `provenance_note` field). A
  hand-authored or third-party document with no `stamp` used to fail deserialization with a bare
  `missing field \`stamp\`` and no hint about the envelope (the BL-83b adopt dogfood hit this and
  recovered only by exporting an empty graph first). But `import_graph` never *gated* on the stamp —
  it was pure friction. The stamp is now the sibling of `content_hash`: absence is a first-class,
  **reported** state, never a refusal. A stampless import proceeds and the `ImportReport` carries a
  `provenance_note` saying the document was unstamped and the upgrade-direction check couldn't be run
  — loud, not silent (`req:no-silent-fallback`). Every export reflow2 writes still carries a stamp,
  and `compare_designs` / `merge_designs` read `reflow2_version()` (`"unstamped"` when absent) so
  their provenance notes never hide a missing stamp. The `import_graph` input schema is unchanged
  (the document is still a free object); this is the leniency-plus-report half of the fix, chosen
  over publishing the envelope shape and keeping the stamp mandatory.

- **The critical `detect↔verify` circular dependency is broken by relocating the id hash to its
  true home** (`dec:fnv1a-foundational`; **patch** — an internal refactor, no surface/schema
  change). The self-model's one *critical* structural defect was a genuine but spurious cycle:
  `detect → verify` is real (gap detection reads a capability's verification state), but
  `verify → detect` existed **only** because `verify` borrowed `fnv1a` — the FNV-1a deterministic-id
  hash that happened to live in `detect.rs` since gap-id hashing first needed it. Eight modules
  reached through `crate::detect::fnv1a`, so the graph asserted a dependency on the *detect domain*
  the code didn't really have. `fnv1a` moves to `nodes.rs` (the vocabulary/identity layer, a
  dependency leaf minting a derived node's id is an identity concern), which breaks the cycle and
  removes six fnv1a-only false couplings on `cmp:detect` (agent, artifact, drift, fielded, heal,
  verify); `report` keeps its real `GapCandidate` dependency. The build script derives `DEPENDS_ON`
  from source, so a rebuild reproduces exactly this shape. Verified on the real self-model:
  `detect_defects` now reports **zero critical** defects (7 warnings — 5 accepted SPOFs, 2
  genuinely-disconnected intent clusters). Also reconciles the artifact drift that the BL-84
  detector fix (below) had left on `structure.rs`/`heal.rs`.

- **Structural detectors no longer cry wolf on pure-decomposition scaffolds or library/data
  foundations** (BL-84; **patch** — turns two false positives quiet, no surface/schema change;
  BL-5/BL-69 family). Two selectivity lessons the community and SPOF detectors were missing:
  - **`disconnected_community` skips a decomposition scaffold.** The design network excludes
    `CONTAINS` on purpose (decomposition is not traceability), so a functional-subsystem grouping —
    several subsystems tied to each other through the Decision that governs them, reaching their
    modules only downward through containment — islanded by construction. It was the false positive
    BL-83a's own self-model surfaced (an 8-node "subsystem island"). An island now reachable from
    the main body through `CONTAINS` is recognized as a grouping, not an orphan — the cluster-level
    twin of `dead_end`'s existing "an assembly speaks through its children" exemption. A genuinely
    disconnected cluster (no containment crossing its boundary to the body) still fires.
  - **`single_point_of_failure` treats a `data` foundation like a `library`, and skips an Interface
    that is itself one.** `couples_only_as_a_library` already spared a component coupled only by a
    `library` contract (F6); it now also spares `data` (a store everything reads), and a new twin
    spares an `Interface` node whose own `medium` is `library`/`data` — the shared-foundation
    contract two subsystems meet at, which is no more a runtime failure point than the library
    component is. Silence is still earned by an explicit `library`/`data`; every run-time medium
    (REST and friends) stays a candidate. Surfaced by the BL-83b adopt dogfood.

- **Edge-vocabulary orthogonality: retired `VALIDATES` and `ENABLES`; added `Verification.kind` and
  the `unvalidated_capability` gap** (`dec:edge-orthogonality`; **schema change → minor**, 55 → 53
  edge types — a graph using the retired edges won't open on the new binary, but none did, and none
  exist in any committed graph; BL-19). The standing rule now on the record: an edge distinction
  earns its keep only if a *computation* reads the two sides differently — otherwise it costs
  extraction consistency (an LLM picks between near-synonyms inconsistently) for no gain.
  - **`VALIDATES` retired** — it was orthogonal-in-name-only with `VERIFIES` (both `Verification →
    Capability`, the canonical V&V confusion) *and* orphan (no code read it). The verify-vs-validate
    distinction is real, so it moves to a **`Verification.kind`** property (`verification` = built
    right / meets spec; `validation` = right thing / meets intent) — a property of the check, not a
    rival relationship, which removes the edge-choice ambiguity. Set it with the new
    **`set_verification_kind`** tool. It earns its keep via a new **`unvalidated_capability`** DETECT
    gap: capabilities with a passing verification-kind check but no validation-kind check ("built
    right, but the right thing?"), reported as one project-level rollup, not N alarms (BL-73).
  - **`ENABLES` folded into `CAUSES`** — same causal axis, differ only by degree, neither read by any
    computation; `CAUSES`'s hint now covers the enabling case.
  - **`TRIGGERS` kept** — it is *not* a causal-degree variant: it carries a `role` property and drives
    the Flow/process-feedback model, so a computation reads it.

### Added

- **The loop nudge now covers the total-bypass session** (BL-90; **patch** — turns a silent gap in
  the trigger into a loud one, no tool-surface or schema change; closes `req:nudge-covers-bypass`).
  `tools/loop_nudge.py` armed only on reflow2 *writes*, so a session that edited code while making
  **zero** reflow2 calls — the agent that ignores the design brain entirely — reached Stop
  silently. A second `PostToolUse` matcher (`Edit|Write|MultiEdit|NotebookEdit` in
  `.claude/settings.json`) now counts file edits, and the Stop hook blocks **once** when a session
  edited files and never touched reflow2 at all: "N file(s) edited and the design graph was never
  consulted — start with `loop_status`; impact-check before further edits, link-artifacts after."
  Blunt by design (the hook can't read the graph to know which files are design-relevant), so it is
  bounded by a count threshold — `REFLOW2_LOOP_NUDGE_EDIT_THRESHOLD` (default 3) — and the
  once-only rule; any single reflow2 call, even a read, disarms it. Stays a *nudge that names what
  is owed*, never a wall. This is the bypass one step upstream of the one BL-74 was built from.

- **`undecided_decision_point` DETECT gap — an open fork surfaces as a question** (BL-70, the last
  of the "missing teeth"; **minor** — a new gap type). A *proposed* Decision holding ≥2 registered
  alternatives is now surfaced by `detect_gaps` as an open decision the design hasn't made — "which
  do you choose? compare them, then collapse." Anchored on the Decision and its alternatives, so an
  acknowledgement survives only while that exact fork stands; it clears the moment the decision is
  collapsed. This gives a proposed Decision teeth: without it, a held-open analysis of alternatives
  would sit undecided forever with nothing to nudge it (`detect.rs`, `tests/alternatives.rs`).
- **Analysis of alternatives — compare parallel design branches on the same measures — the
  `analyze_alternatives` tool** (BL-70 v1, branch-by-file; **minor**). Given the paths to two or
  more alternative design exports (the first is the baseline), it loads each into its own throwaway
  graph, runs the same rollup, and lays the decision-relevant measures **side by side** — design
  nodes, open gaps, structural defects, allocation modularity, capabilities verified — plus every
  non-baseline branch's structural divergence from the baseline (`compare_designs`). Alternatives
  become comparable **on measures, not advocacy** (`dec:parallel-alternatives`). Alternatives are
  design *space* (sibling roads that CONTRADICT, held under a proposed Decision), distinct from
  epochs (*time*); collapsing the winner reuses `merge_designs`/`apply_merge`, retiring the losers
  reuses retire-from-design — so almost no new machinery, and no detector learns about "worlds"
  (`crates/reflow2-core/src/alternatives.rs`, `tests/alternatives.rs`).
- **The decision point — hold and collapse forkable alternatives — `set_decision_status`,
  `register_alternative`, `alternatives_for`, `collapse_decision` tools** (BL-70 rung 2; **minor**).
  A *proposed* Decision is now a decision point with teeth: `register_alternative` hangs a
  lightweight Artifact pointer (naming its export, branch-by-file) under it, `GOVERNED_BY` the
  Decision and `CONTRADICTS` its siblings — refusing unless the Decision is proposed (you fork an
  open choice, not a settled one). `alternatives_for` lists them (feed the locations to
  `analyze_alternatives`). `collapse_decision` chooses a winner: the Decision moves to `accepted`,
  the losers are superseded (`OBSOLETES` — retired on the record, **not deleted**), and the outcome
  and rationale are written into the Decision's own `alternatives` field — the ADR "losers'
  obituary" the fork upgrades from prose into live, forkable structure. The winner's design content
  is merged separately with `apply_merge`. Alternatives are design *space* (CONTRADICTS), distinct
  from epochs (*time*) — `dec:parallel-alternatives`. `tests/alternatives.rs` — 8 cases total.
- **Three-way merge of two divergent designs — `merge_designs` + `apply_merge` tools, `--merge`
  CLI** (BL-80, propose + apply; **minor** — new tools on the surface). Compare's write-side
  sibling: given a common ancestor (base) and two divergent records (ours, theirs), it runs git's
  trivial-merge case table per node and per property over typed values — one-sided changes are
  taken, agreed changes are taken, both-sides changes become **conflicts surfaced as questions**
  with deterministic ids, and a node one side deleted while the other changed it is **retained and
  asked** (deletion must be re-justified; `dec:merge-conflict-semantics`). Edges get the identical
  rule. `merge_designs` **proposes — it writes nothing** (`dec:merge-three-way`,
  `dec:report-dont-judge`); the base comes from git (`git merge-base` + the committed export at
  that commit), so reflow2 builds no commit DAG of its own. **`apply_merge` is the explicit commit**:
  it takes the human's per-conflict decisions (`base`/`ours`/`theirs`) and makes the live design
  equal the merged result, atomically — **refusing, and writing nothing, until every conflict is
  decided** (and on any resolution that names no conflict). Pure/deterministic core
  (`crates/reflow2-core/src/merge.rs` — `merge_designs`, `resolve_merge`, `apply_merge`;
  `tests/merge.rs` — 25 cases). Specifies **and closes** the core of BL-12's multi-writer merge.
- **Merge rerere — reuse a recorded conflict resolution — `recall_resolutions` tool +
  `apply_merge use_recorded`** (BL-80 #5; **minor**). Each merge conflict now carries a
  `resolution_key`: a content fingerprint over the disputed values and property, deliberately
  **node-independent**, so the identical conflict anywhere keys the same (git's model). `apply_merge`
  records every applied property/edge-property resolution — as an answered `Question` whose id *is*
  the key, so it travels in the export and reuses the answer machinery (no schema change). A later
  `recall_resolutions` returns the recorded decision for matching keys, and `apply_merge use_recorded`
  fills undecided conflicts from them — **advisory**: the human still opts in and confirms
  (`dec:merge-rerere`, `dec:report-dont-judge`), never an auto-decision. Resolve the shape once,
  apply it across all N near-identical conflicts — the `BL-73` field pain, answered. v1 covers
  property/edge-property conflicts (node-type/delete-modify keys deferred).
- **Design-authorship identity — the `Contributor` keystone, authorship seed** (BL-79, user-chosen
  direction; **schema change → minor, and a graph written now cannot be opened by a
  pre-`Contributor` binary** — refused loudly by the count-based provenance check, per BL-19). The
  schema gains a `Contributor` node type (kind: person / automated_agent / organization) and an
  `AUTHORED_BY` edge, giving the design a structured *who* — who authors and decides the design
  itself — kept deliberately distinct from the existing `Actor` (who the designed system *serves*):
  two different lifecycles, not one overloaded type. `AUTHORED_BY` is **not** a traceability edge
  (absent from the impact table on purpose), so authorship never enlarges a blast radius; the smoke
  test asserts exactly that. Two typed tools land it — `add_contributor` and `authored_by` — and
  the capture-intent skill now records who is driving once per session and attributes captured
  nodes *when they are captured*, not at session end. This is the seed of the identity thread the
  backlog kept pointing at: the same node will carry claims (BL-44), alternative-authorship (BL-70),
  and the mechanical half of requirement-certainty (BL-41). Schema now: 28 node types, 55 edge
  types. Deferred, recorded: the `ACTS_FOR` rung (agent-acts-for-person, the git author/committer
  split), and any "unauthored node" detector (left out to avoid an N-alarm on existing graphs).

- **Tool-surface hardening: read-only classification + toolsnaps** (BL-76, from the
  github-mcp-server comparison; minor: every served tool gains an `annotations.readOnlyHint`
  field — no schema change, no call shape change). Every one of the ~80 MCP tools now declares
  the standard MCP `readOnlyHint` annotation, so a client can tell a query from a mutation
  (approval prompts, dry-run affordances) without guessing from the name. The classification is
  derived from the graph borrow itself — a read-only tool takes the shared lock, a writer takes
  `let mut g` — so it cannot silently disagree with what the tool does; the non-obvious writers
  (`gap_to_prompt`, which records the question it phrased, and the `reconcile_*` family, which
  records DriftEvents) are correctly not read-only. Two mechanical tripwires keep it honest:
  `smoke_mcp.py` fails if any served tool omits the hint (a new tool cannot ship unclassified —
  the explicitness gate), and **toolsnaps** (`tools/toolsnap.py`) freeze each tool's served
  schema as a committed golden JSON, CI-diffed, so a surface change — a lost param type, a
  reshaped result, a stale binary (the BL-28/BL-32/BL-48 bug family) — becomes a reviewed diff
  named tool by tool rather than a silent drift.

- **`req:frictionless-update` confirmed** — the "install is one command, update is one word"
  requirement moved `proposed → accepted` on the user's word (2026-07-22); all 18 requirements
  are now user-confirmed. Records the intent behind BL-51's one-liner install / one-word update
  direction as a settled requirement, not an assertion awaiting review.

- **Requirement certainty, derived and rendered** (BL-75, closing the field-trial trio;
  minor: `graph_report` gains a field and a snapshot line — no schema change). A
  requirement's certainty is computed from the two axes that already span the space, never
  stored as a third: `accepted`/`met` → **user-confirmed**, `proposed` + `inferred` →
  **recovered from the artifact, awaiting the user**, `proposed` + `authored` → **asserted,
  awaiting the user**, `deferred`/`dropped` → settled out. The snapshot now carries a
  "Requirement certainty" line so no session reconstructs it in prose — the caveat where-am-i
  had to hand-write every time. The load-bearing doctrine is now stated everywhere it
  matters (`dec:certainty-derived`, the `set_requirement_status` tool description, the
  capture-intent and adopt skills): an agent captures at `proposed`, and **every move off
  `proposed` records the user's word** — promoting a status yourself forges their signature.
  Second item this week where the vocabulary was already sufficient and only the read side
  was blind.

- **Component-granularity verification — the third state** (BL-73, from the field trial;
  minor: `verification_coverage` gains a field, `detect_gaps` a gap kind). A capability's
  verification is now three-valued: `verified` (a passing check of its own),
  `component_verified` (its allocated component carries a passing check — computed at read
  time, never written), and unchecked. The coverage line reports it ("12/20 verified, 8 more
  at component granularity"), and the N per-capability `unverified_capability` alarms on a
  component-tested system collapse into ONE `component_granularity_verification` gap per
  carrying component at 0.35 — "is component granularity enough for these?", acknowledgeable
  once. `status_contradiction` accepts component-granularity proof; `loop_status` counts it
  as proven; a failing component suite carries nothing (passing-is-verified holds at every
  granularity). The write side needed nothing — `VERIFIES` always accepted a Component
  target; the adopt skill now teaches registering each real suite where it lives. The trial
  that raised this read a tested system as "0/20 verified" and paid 21 acknowledges to
  record the truth; that shape is now a handful of one-time questions.

### Fixed

- **A feature-off on-disk open no longer writes a provenance stamp before failing** (**patch** —
  a silent side-effect made loud/correct). `open_rocksdb` stamped `<path>.meta.json` *before* the
  `rocksdb`-feature gate, so a build without the feature left a stray stamp behind — and across a
  schema change a stale higher-count stamp then pre-empted the "fail loud, name the feature" error
  with a "knows more of the schema" refusal (it also made the feature-off test non-hermetic on a
  machine that had run it under an older binary). The store is now opened *first* (still content-
  agnostic, so the real "knows more" refusal for an actual on-disk graph is unchanged), and the
  test pins that a failed open writes no stamp. Surfaced by the BL-83b adopt dogfood.
- **The "knows more of the schema" refusal now names both recovery paths** (**patch** — a
  misleading message made correct; BL-86). The stamp is count-based, so a graph refused for a
  higher count could mean *either* a stale binary *or* a graph that predates a schema **removal**
  (like this release's 55 → 53 edge-orthogonality) — the count can't tell them apart. The old
  message assumed only the first and said `cargo build`, useless for the removal case and for a
  `curl | sh` consumer with no checkout. It now presents both: update reflow2, **or** migrate the
  graph (import a committed export into a fresh graph, or export-with-the-writer → import-here;
  retired types are dropped and named on import). The set-based-stamp fix that would remove the
  ambiguity entirely is BL-86.

## [0.9.0] — 2026-07-22

A minor release, and the one the field trial should pick up: the design record now **proves
itself**, and the coherence loop now **fires on a trigger instead of on memory**. The tool
surface gains `loop_status`; the export document gains `content_hash` + `prev_content_hash`
(both optional and backward-compatible — old exports still import, absence is reported not
errored); the kit gains `loop_nudge.py`. No graph-schema change, so no upgrade doc: existing
graphs open unchanged and consumers update blindly, gaining the tool, the tamper-evident
export, and the loop hook. Headline threads since 0.8.0: the AT-Protocol-inspired export
hash-chain (the committed design is now tamper-evident in CI, verified cross-language), and
the close-out of the adoption-critical BL-74 — `loop_status`, the write-tool `loop_hint`s, and
the event-fired loop-nudge hook, from the first extensive field trial.

### Added

- **`loop_status` — the coherence loop's outstanding debt, cheaply** (BL-74 rungs c+b, from
  the first extensive field trial; minor: new tool + new `loop_hint` field on write
  results). The field lesson: under operational load, adding nodes *feels* like using reflow2
  while the capture→detect→ask→decide loop silently stops. One call now returns the debt as a
  to-do list — anchored gaps never put to the user, questions waiting or
  answered-but-unwritten, structural defects, capabilities claiming realized/verified with no
  passing check, drift awaiting a disposition, claims never examined — computed from graph
  state alone, never run history (looking is not writing; phase nudges are guidance, not
  debt). The write tools (`add_requirement`, `add_capability`, `add_component`,
  `add_interface`, `link_artifact`) now carry a static `loop_hint` pointing at the next loop
  step in the result the agent already reads. The capture-intent and detect-and-ask skills
  teach the call; rung a (a kit hook recipe firing `loop_status` on client events) stays open
  on BL-74.
- **The loop-nudge hook — the trigger itself** (BL-74 rung a, closing BL-74). The kit ships
  `tools/loop_nudge.py` (stdlib, beside `reflow2_check.py`): one script wired to three harness
  events — SessionStart prints the orient-first reminder, PostToolUse counts reflow2 graph
  writes per session (a `loop_status`/`detect_gaps`/`detect_defects` call resets), and Stop
  blocks **once** when a session tries to finish with unchecked writes, saying exactly what to
  run. Never blocks twice, never reads the graph (the session's server holds the single-writer
  lock — the hook counts events; the graph answers what is owed), never breaks a session (any
  failure warns and exits 0). Claude Code settings snippet in the kit AGENTS.md step 0a;
  `REFLOW2_LOOP_NUDGE_THRESHOLD` tunes it. Its own hermetic suite runs in CI.
- `build_design_graph.py` writes the committed export **through the export tool's file seam**,
  so the self-model export now carries the lineage chain instead of silently dropping it —
  found because the first hashed rebuild came out chain-rootless.

- **The export proves itself: content hash + lineage chain** (`dec:export-hash-chain`, from
  the AT Protocol comparison; minor: the export document and several results gain fields).
  Every export now carries `content_hash` — sha256 over the canonical sorted JSON of the
  design content only, excluding the stamp, so the same design fingerprints identically
  whichever build wrote it — and `prev_content_hash`, recorded when an export replaces an
  existing export file with changed content (unchanged content keeps the old chain, so
  unchanged designs still write byte-identical files). `compare_designs` gains `ancestry`
  (other_succeeds_base / base_succeeds_other / siblings_of_common_parent / unknown) — the
  one-generation answer to "was this divergence made from the base, or did the two fork?" —
  and calls out a side whose hash doesn't match its own content. `import_graph` reports the
  same mismatch loudly (import proceeds; seeing it is not optional), and `reflow2_check.py`
  fails the build on a committed export that doesn't match its own hash — the committed
  record is now tamper-evident in CI, verified cross-language (the stdlib-Python
  recomputation is pinned against the Rust one in the smoke test). Pre-hashing documents
  stay importable and comparable everywhere; absence is reported, never an error.
- Backlog: the AT Protocol design notes land under BL-12 (identity-decoupled hosting,
  labels-as-overlay, per-writer-repos-plus-merge as a candidate shape), and BL-72 raises
  namespaced schema packs (Lexicon-style domain vocabularies that compose without forking).

## [0.8.0] — 2026-07-21

A minor release: the tool surface gains `compare_designs` (and the binary the `--diff` flag) —
a new tool, so minor per the versioning policy. No schema change, so no upgrade doc: existing
graphs open unchanged and nothing needs migrating; consumers update blindly and gain the tool.

### Added

- **`compare_designs` — the design-vs-design diff** (BL-71 rung c; minor: new tool on the
  surface). The reconcile family compares design against *reality*; nothing compared two
  as-designed records until the curated rebuild clobbered the accumulated live layer and only
  a node count noticed. The new core op diffs two export documents — or the live graph against
  one — into `added` / `removed` / `changed` (property-level, with absent-vs-present
  distinguished) **relative to a named base**, banded into design content vs the supporting
  layer, reporting divergence and never judging which side is right ("drift" stays reserved
  for design-vs-reality; `dec:design-diff-vocabulary`). Reachable three ways: the
  `compare_designs` MCP tool (`base_path` alone = live vs committed record, `other_path` too =
  file vs file), and `reflow2-mcp --diff BASE [OTHER]` — the two-file form never opens the
  graph, so it runs even while a server holds the lock (CI, branch comparison). The where-am-i
  skill now opens with it when a committed export exists. Also the read side BL-70's
  branch-by-file comparison and BL-12's two-writer merge will build on.
- **Release manifests are honest about late-born files**: `build_design_graph.py` now records
  a module absent at a release's tag as *not in that release's manifest* (said out loud per
  release) instead of refusing the whole rebuild — absence from an old manifest is the truth,
  and the checksum refusal still guards files *claimed* for a release that never carried them.

## [0.7.0] — 2026-07-21

A minor release: the schema gains one optional property (`Snapshot.edges`), which is what makes
this 0.7.0 rather than 0.6.2 — see [docs/upgrading-to-v0.7.0.md](docs/upgrading-to-v0.7.0.md)
(short version: existing graphs open unchanged, old snapshots stay readable, no action needed).
The theme is the coherence loop closing its own gaps: history that survives edge moves, a SPOF
detector that measures the right graph, and the whole loop made continuous for consumers via a
CI gate.

### Added

- **A consumer CI coherence gate** (BL-66). `tools/reflow2_check.py` — stdlib-only, ships in
  the kit tarball — reads the *committed* design export (never the live `.reflow2/` store),
  rehashes every registered artifact from the working tree, reconciles, and runs the gap
  detectors. Exit 1 on unaccepted drift (an accepted drift updates the export, so red means the
  two-sided accept was skipped) or an open anchored gap at/above `--gap-threshold` (default
  0.8) — `acknowledge_gap` is the honest way to go green without fixing. Exit 2 when it cannot
  run; never a silent pass, and deliberately no flag to skip the drift check. New **ci-gate**
  skill carries the copy-paste CI step and the red-to-green playbook.

- **Snapshots capture edges, so an edge move keeps its history** (BL-63). `snapshot_node` (and
  therefore `record_change`) now stores the node's design edges — direction, edge type, the
  other endpoint and the edge's properties, sorted for byte-stable exports — in a new optional
  `Snapshot.edges` property beside `state`. A large class of design change is an edge move, not
  a property edit: a re-allocation deletes `ALLOCATED_TO` one component and draws it to
  another, and before this the only durable record of the old owner was a hand-authored
  Decision — a lazy reallocation left no trace. Edges touching bookkeeping nodes (snapshots,
  change events, epochs, drift, provenance, questions) are excluded: a snapshot captures design
  structure, not the audit trail, and would otherwise grow with its own history. New
  `parse_snapshot_edges` / `SnapshotEdge` in the core API; a snapshot taken before this change
  has no `edges` property and reads as an empty capture, not an error. The revise-design and
  retire-from-design skills now say edges are captured, and revise-design's links guidance
  drops its pre-BL-63 workaround ("leave a formerly-true edge") for the honest sequence:
  record first, then delete. **Schema note for the next cut**: `Snapshot.edges` is a new
  *optional* property — existing graphs open unchanged and old snapshots stay readable — but a
  schema change makes the next release minor (0.7.0), and its upgrade doc should say exactly
  this.

### Fixed

- **`single_point_of_failure` measures connectivity on the as-built operational network**
  (BL-69). It used to measure removal-splits on the full design network, where intent edges are
  wrong in both directions at once: a leaf component whose capability/artifact/verification hang
  off it fired (the severed "subsystem" was made of sentences), while a genuine operational cut
  vertex stayed silent because the parts it severs remained "connected" through a SATISFIES
  chain — a path that carries nothing at run time. Connectivity (and candidate enumeration) now
  runs on Components/Interfaces/Resources/Environments plus the Artifacts realizing them; all
  prior selectivity lessons (baseline-relative islands, non-trivial subsystems, intent-node and
  library exclusions) are unchanged. On reflow2's own design: `cmp:flow` (false) stops firing;
  `cmp:export`, `ifc:graph-export` and `cmp:graph` (true, previously hidden) now report alongside
  the already-accepted `cmp:service`. A defect list can grow when the detector stops lying —
  that is the fix working, not a regression.

## [0.6.1] — 2026-07-21

A patch release: correctness and doctrine fixes only, no tool-surface or schema shape change,
so it updates in place and an existing design opens unchanged. The headline is the **core
silent-failure batch** (BL-58) — a dozen places where a failure could be swallowed or a value
silently reset are now loud or correct.

### Fixed (BL-58 · core silent-failure batch)

- **A re-ingest no longer resets properties it did not mention** — matched-evolved integration
  merges (`upsert_node`) instead of replacing, so a status or provenance set separately
  survives (the BL-46 failure, on the ingest path).
- **`propagate_change` on a missing/typo'd ChangeEvent errors** instead of returning an empty
  blast radius indistinguishable from "impacts nothing."
- **`apply_heal` applies all operations in one atomic batch** — a mid-proposal failure rolls
  the whole apply back instead of committing earlier merges (which have no undo) while
  reporting nothing happened.
- **Snapshots serialize with sorted keys**, so two exports of identical history are
  byte-identical (they were process-random before).
- **Swallowed edge-creation errors now surface** — a failed `GOVERNED_BY` / `ASKS_ABOUT` /
  provenance / drift-seed edge is reported, not silently dropped.
- **Budgets refuse a non-finite contribution** at the write seam (a NaN used to panic the
  worst-path scan) and report a **provable** over/under-run instead of hiding it behind
  `Incomplete` when unstated contributors cannot change the outcome.
- **Large integers are not lossily widened to floats** (the `i64::MAX` rounding edge now fails
  loud); `truncated_beyond_depth` is documented honestly as a one-hop-frontier lower bound; a
  drift on an undocumented file no longer writes a dangling edge; a `CONTAINS` and a
  `DEPENDS_ON` missing-intermediate over the same pair get distinct gap ids; a reused ingest
  `fragment_id` is refused; node-type scans are deterministically ordered.

## [0.6.0] — 2026-07-21

The first release cut from the public repo, and the one to actually reach a downstream user:
v0.5.0 was tagged but its binaries never published (a stuck CI runner), and the whole
2026-07-21 deep-review batch has landed since — including fixes for a HEAL bug that could
delete a node, an installer that could clobber a user's edits, and an `install.sh` that could
die silently. Several agent-facing tool shapes changed (`get_node`, `delete_*`, `propagate`'s
default, new params on `add_change_event`/`export_graph`), which is why this is a minor bump,
not a patch. No graph-model or schema change, so an existing design opens unchanged.

### Changed

- **The tool boundary now reports whose fault an error is** (BL-57): a caller's mistake — a
  typo'd id, an unknown type, a bad enum value — returns `invalid_params`, not
  `internal_error`. Fixed at the one choke point (`dyno_err`), so ~60 tools stop blaming the
  server for the caller's typo.
- **A typo'd optional parameter is now rejected, not silently swallowed** (BL-57): every tool
  request declares `deny_unknown_fields`, so the published schema carries
  `additionalProperties: false`. `full` misspelled as `ful`, or `detected_at` as `at`, is
  refused at the boundary instead of quietly doing nothing. (This immediately caught a latent
  bug where the smoke suite had been passing an ignored `at` to `reconcile_artifacts`.)
- **`export_graph` refuses to overwrite an existing file** unless `overwrite: true` is passed,
  and reports the resolved absolute path — a stray or injected `path` can no longer silently
  clobber a file.
- **`get_node` returns one shape both ways** (BL-57): `{node: {…}}` when present, `{node:
  null}` when absent (was a bare object vs `{value: null}`).
- **The everyday two-session lock collision reads plainly** (BL-57): starting a second server
  on the same graph now gets the single-writer explanation, like `--export`/`--import` already
  did, not a raw RocksDB error.

### Testing

- **The skill lint now checks single-word tool names** (BL-61): an underscore-only filter had
  exempted `allocate`, `satisfies`, `genesis`, and 8 other served tools from the "does this
  tool still exist?" check — a rename would have left the skills' prose pointing at a dead
  tool with the lint still green. Filter dropped; allowlist extended to the legitimate
  single-word non-tool terms; a renamed single-word tool now fails.
- **The 14 tools that had no test coverage now have some** (BL-62): the temporal (epochs,
  precedes, pin, record_change), resource, realization, allocation-analysis, dimension-drift,
  and delete families are exercised in `tests/tools.rs`, and a new `smoke_mcp.py` section
  drives `create_node`/`scan_nodes`/`search_design`/`delete_node`/`get_node` over the real
  stdio boundary — the blind spot the smoke test exists to close.

### Docs

- **The instruction files now describe v0.5.0, not the pre-surface era** (BL-60). AGENTS.md's
  "Current state" section told readers to assume no MCP surface, service, or LLM wiring
  existed — while 78 tools ship; it, the README, and the coverage matrix are corrected
  (surface shipped and decided; two crates; foundation pin v0.10.0; 27 node / 54 edge types;
  the full module list; INCLUDES in the traceability set). surface-plan.md and
  interaction-surfaces.md carry "superseded / decision made" banners; SETUP.md drops the
  stale "repo is private" note and the commit-the-graph contradiction (commit an export);
  three skills whose steps contradicted current tool behavior are fixed.

### Fixed (from the 2026-07-21 deep review)

- **A self-loop `DUPLICATES` edge no longer drives HEAL to delete the node** (BL-53,
  critical): `x DUPLICATES x` built a sanctioned merge whose re-pointing skipped every edge
  and whose delete then removed the survivor itself — with no undo, reported as success. It
  is now refused at derivation, covering propose and apply alike.
- **The installer proves ownership before refreshing a file** (BL-54): a per-file hash
  manifest in the install stamp. Your edits to an installed AGENTS.md or skill are kept and
  reported (`LEFT ALONE`), never overwritten; files the kit no longer ships are pruned only
  when untouched; a malformed MCP config reports left-alone instead of crashing mid-install;
  `--check` and the real run now agree.
- **`install.sh` cannot die silently anymore** (BL-55): a release without `checksums.txt`
  reaches the honest "NOT verified" note instead of a message-less exit, and a binary that
  cannot execute on your platform fails loudly with the build-from-source recipe instead of
  printing success.
- **A partial release upload can no longer become `releases/latest`** (BL-55): release.yml
  drafts first, uploads, asserts every expected asset is attached, then publishes.
- **`smoke_mcp.py --graph-path` refuses to delete an existing directory** unless `--wipe` is
  passed (BL-56) — pointing it at a live design graph used to destroy it before any prompt.

### Added

- **`add_change_event` can declare what it changed** (BL-50): an optional `affected` list
  draws the CHANGED edges in the same call — validated whole before anything is written, so a
  bad entry refuses the event instead of leaving a partial record. Previously the one edge
  type that models "this event changed that node" had to be drawn one generic `create_edge`
  at a time.
- **A SessionStart hook recipe in the consumer kit** (BL-50): the "orient with where-am-i at
  session start" ritual can now be wired into harnesses that support hooks, so it stops
  depending on the agent recalling the instruction file.

### Changed

- **A Release is part of the design network** — INCLUDES joined the propagate/structure
  traceability table (same shape as REALIZES: the contents are the source of truth, the
  release a downstream packaging). A changed artifact now reaches the releases that ship it
  in a blast radius, and a Release + Environment pair is no longer a disconnected island by
  construction — found modelling v0.4.0, where the graph's own HEAL reported `{env:dev,
  rel:v040}` as a 2-node disconnected community.
- **Integer literals are accepted for float-typed properties** (BL-50): `confidence: 1` now
  widens losslessly to `1.0` at the core write seam instead of being refused with "expected
  Float, got int". JSON has one number type; every client writes the bare integer. Range
  checks still apply after widening, and a non-exact integer still fails loud.
- **`describe_schema` from/to counts half-exact matches** (BL-50): an edge type that names one
  endpoint and is open on the other by design (CHANGED, SATISFIES) is now reported as the
  modelled fit for its pair — `half_exact_matches` in the payload, honest wording in the note —
  instead of being lumped with both-sides wildcards.
- **`delete_node` / `delete_edge` return `{deleted}`** instead of a bare boolean — a scalar in
  `structuredContent` is the same malformed envelope as BL-48, caught by the new choke-point
  wrap the day it landed.

- **`propagate_change` / `propagate_from` answer with a summary by default** (BL-49, from the
  self-adopt live session): counts by distance, the distance-1 ring with the edge that reached
  each node, risk crossings at any distance, and the usual `unknown_seeds` /
  `truncated_beyond_depth` partial fields. The full per-node dump with `via` hop chains is
  behind `full: true`. On the self-model a blast radius came back as 70k characters nobody
  could read inside a session — a blast radius that doesn't get read doesn't get acted on.
- **`export_graph` writes to a file on request** (BL-49): pass `path` and it writes the
  document as deterministic sorted-key JSON (byte-identical for an unchanged graph, diffable
  under git) and returns a small `{path, bytes, nodes, edges, stamp}` receipt instead of the
  ~90k-char payload.

### Fixed

- **`graph_report_markdown` is reachable again from spec-compliant clients** (BL-48). It put
  its Markdown into `structuredContent` as a bare string, where the MCP contract wants an
  object — the same response-side shape as the v0.2-era array bug, and it made the report a
  session reads first fail outright from Claude Code. Prose now travels as text content only,
  `ok_json` wraps any remaining scalar so no tool can leak one, and `smoke_mcp.py` asserts the
  result envelope on every call it makes.

- **`create_node` on an existing id now merges instead of replacing** (BL-46, from the
  self-adopt live session). The props you pass overwrite; every stored property you omit
  survives. Previously the supplied object replaced everything and schema defaults
  re-materialized over the rest — a partial "edit one property" call silently reset a
  verified capability's status to `planned`. The tool description now states the contract
  the revise-design skill always promised. Creation and validation are unchanged: a new id
  still creates, unknown types and missing required properties still fail loud.
- **The merge survivor rule no longer lets a vintage node tie with an explicit `authored`
  one** (BL-47, same session). A node without a `provenance` property — possible only for
  nodes written before the property existed — now ranks just below explicit `authored` and
  above everything else. Before, it counted as `authored`, the tie fell to the id
  tiebreak, and the alphabet nearly deleted an authored, verified capability in favour of
  its genesis stub. Pre-provenance graphs (all nodes vintage) behave exactly as before.
- **A merge now keeps the survivor's edge when the removed node has the same edge** (BL-47's
  second finding). Previously the removed node's edge properties landed on top of the
  survivor's via the create_edge upsert — reported, but still clobbered; report-then-clobber
  was the wrong half of two-sided accept. The drop is still reported in `discarded`.

## [0.5.0] — 2026-07-20

The tool surface changed shape again (`documents`, the 78th tool), which is what moves the
minor pre-1.0. The schema did **not** change (still 27 node types / 54 edge types): no stamp
moves, older binaries still open a graph this version wrote, upgrading is a rebuild — or, new
with this release, downloading the prebuilt binary, because **this is the first version with
published release binaries.**

### Added

- **reflow2 without a checkout: published release binaries and a one-line installer** (BL-15's
  last open half). Every version tag now builds `reflow2-mcp` for Linux x86_64 and macOS
  arm64/x86_64 and attaches the binaries, the consumer kit tarball, and sha256 checksums to
  the GitHub release. `tools/install.sh` (`curl … | sh`) detects the platform, downloads via
  `gh` while the repo is private (plain curl the day it isn't), verifies checksums, installs
  to `~/.local/bin` and `~/.local/share/reflow2/kit`, and prints the exact next command;
  re-running it updates binary and kit together, never touching design graphs.
  `reflow2_init.py` now works from the installed kit: `--binary`/PATH resolution,
  `KIT_VERSION.json` in place of git metadata, and update advice that names the installer
  instead of `git pull` + `cargo build`. SETUP.md leads with the no-build path.

- **A file that *describes* the design can finally say so: the `documents` tool** (BL-26's
  write side; the recurring lesson's ninth instance closed). `DOCUMENTS` was declared in the
  schema from the start — design docs, ADRs, READMEs, diagrams, instruction files — with no
  constructor and no tool, which is why two instruction files disagreeing about the build
  command went uncatchable: neither file was in any graph. `documents(artifact, target_type,
  target_id, doc_kind?)` closes that, failing loud when either endpoint is missing (the
  storage engine accepts dangling edges, so this check is the only one there is). The
  link-artifacts skill now states the criterion — record a file when something would be
  *wrong* if it drifted out of step with the design; keep generated files out — and the
  boundary against `REALIZES` (implementation) and `SPECIFIES` (machine-readable contract).
  Whether PROPAGATE should traverse `DOCUMENTS` — blast radius reaching every doc that
  mentions a node — stays an open decision on BL-26, deliberately.

### Changed

- **A merge's survivor is now chosen by provenance, with id as the tiebreak** (the BL-29
  survivor decision, taken by the user). A merge keeps only the survivor's properties, so the
  choice decides whose words are kept — and the old lexicographic-id rule could let an
  `inferred` stub delete an `authored` node's text. The rank follows how directly a human
  stands behind the text: `authored` > `planned` > `imported` > `reconciled` > `inferred` >
  `healed`; equal rank falls back to the smaller id, so the choice stays fully deterministic
  and graphs without the property (the schema default is `authored`) behave exactly as before.

### Fixed

- **A chained duplicate (a↔b, b↔c) can no longer corrupt the graph through `apply_heal`**
  (BL-29's last reproducible hazard, now reproduced and closed). Both merges are individually
  sanctioned — each `DUPLICATES` edge is real — but applying them in one proposal writes to a
  node the earlier merge deleted; the storage layer accepts the dangling edge, so the graph
  corrupted silently while the report claimed `verified: true`. (`propose_heal`'s own output
  only avoided this by luck of issue-id hash ordering.) Three changes, each pinned by a test:
  `propose_heal` emits one merge per chain and defers the rest with the reason stated
  (`skipped_operations`, never silent); `apply_heal` refuses any proposal — including a
  hand-built one — whose merges share a node, before a single write; and a merge now
  re-points a `DUPLICATES` edge to a *third* node onto the survivor, so the chain's
  still-unresolved claim (b↔c) survives as a↔c and the propose/apply loop converges — one
  round per link — instead of the user's assertion vanishing with the merged node.
- **A real edge joining the two nodes being merged is reported, not silently dropped.** It
  cannot be re-pointed (it would become a self-loop), so it dies with the merge — that loss
  now appears in `HealReport.discarded` like every other. The pair's own `DUPLICATES` edge
  stays silent: resolving it is the merge's purpose.

## [0.4.0] — 2026-07-20

The tool surface changed shape (`delete_edge`), which is what moves the minor pre-1.0. The
schema did **not** change (still 27 node types / 54 edge types), so no stamp moves and a
v0.3.0 binary still opens a graph this version wrote — upgrading is `git pull` and a rebuild,
nothing else. The v0.3.0 tag sits at the commit that prepared it (36adb2e, 2026-07-19);
everything after rides here.

### Added

- **The design is searchable: `search_design`, BM25 over every `fulltext` property.** The
  schema declared `fulltext:` on `name`/`statement`/`description` from the day it was written,
  and the foundation implements the index (`dynograph-text`, Tantivy, mirrored automatically
  on every node write) — but reflow2 never enabled the feature and nothing served it:
  recurring-lesson instance #17, one level deeper than usual, because this time even the
  *schema annotations* were shipped capability nothing could reach. Until now the only
  retrieval was `get_node` (know the id) and `scan_nodes` (read a whole type), which made
  finding-by-content the LLM's job — the seat-swap partnership.md forbids: finding and
  counting belong to the graph.

  The `fulltext` cargo feature follows the `rocksdb` pattern exactly: off on the sub-second
  core path, enabled by `reflow2-mcp` on the dependency edge, failing loud (never silently
  empty) when absent. `search_design(query, node_type?, limit?)` returns ranked hits hydrated
  with each node's name, echoes the limit that bounded it (hits == limit means there may be
  more), and reports index-drift hits as `stale` rather than dropping them; the server
  reindexes once at open, so a graph written by an older, index-less binary becomes
  searchable instead of silently absent. Skills now lean on it: capture-intent searches
  before adding (a near-duplicate found is a revision, not a new node), and
  revise/retire-design map the user's words to real ids instead of guessing or scanning
  whole types into context.

- **The loop can now change its mind on the record: `revise-design` and `retire-from-design`
  skills, and a `delete_edge` tool.** The kit's skills covered create (genesis,
  capture-intent, link-artifacts) and read (where-am-i, check-health, detect-and-ask), and
  impact-check covered the moment *before* an update — but no skill walked the update itself,
  and nothing at all covered removal. The primitives existed and were undocumented: an
  existing id passed to `create_node` **merges** (revised props overwrite, the rest survive),
  which is how revision is expressed — established by probe this session, written down
  nowhere until now.

  - **revise-design** — impact first, then `record_change` BEFORE the edit (the snapshot must
    capture the node still saying the old thing), then the edit via create-as-merge / the
    typed status setters / edge tools, then re-detect for the second-order rot a reasonable
    edit leaves behind.
  - **retire-from-design** — forces the fork that matters: design history (was real, now
    over) is *retired* — `record_change` with `deprecation`, `status: dropped`, an
    `OBSOLETES` from the successor — while a modelling mistake (never should have existed)
    is *deleted* with no ceremony. Confusing the two either erases the past or embalms a typo.
  - **`delete_edge`** (MCP tool) — retract one mis-drawn assertion; both endpoints survive.
    Until now the only way to remove a wrong edge over MCP was to delete one of its endpoint
    nodes — instance #16 of "the core can, the surface can't" (`DesignGraph::delete_edge`
    existed all along). A link that WAS true and stopped being true is history, not an error;
    the tool description says so.

  Found because the kit's mirror copies in this repo were themselves stale (missing F6's
  `medium` paragraph) — refreshed, and docs/skills/README.md now says eleven skills.

### Changed

- **The self-model now derives structure from source and reconciles against the filesystem**
  (the 2026-07-20 self-adopt run). Turning the `adopt` skill on reflow2 itself found that 15 of
  the committed model's 16 gaps pointed at the *model*: five shipped, MCP-exposed, tested
  capabilities (`reconcile-verified`, `reconcile-deployed`, `model-process`, `freshness`,
  `adopt`) still said `planned`, 15 of 33 source files carried no Component or Artifact, and
  the graph held **zero DEPENDS_ON edges** — so `circular_dependencies` was structurally blind:
  a detector cannot walk edges nobody drew. Ruled per sharpening.md §2 (model wrong, not
  system) and fixed in `tools/build_design_graph.py` as standing probes rather than one-off
  edits:

  - **DEPENDS_ON is derived from imports and calls, never from prose.** Two signals: `use
    crate::` paths, and `self.method()` calls resolved against which module's
    `impl DesignGraph` block defines the method — Rust needs no `use` for inherent methods,
    and it is exactly these that carry cycles rustc never flags. Comments are stripped first
    (a rustdoc intra-doc link in `detect.rs` otherwise fabricates a detect↔heal cycle that
    does not exist), and a method name defined in more than one module is skipped loudly,
    never guessed. 74 evidence-based edges; with them in place **reflow2 reports its own
    `cmp:propagate ↔ cmp:structure` cycle as a critical defect** — the first structural truth
    about itself it has ever surfaced unprompted.
  - **The build now ends by reconciling the model against the filesystem** — a full sweep of
    both crates' src trees plus the installer through `reconcile_artifacts` (`exhaustive`,
    unswept-file entries included), so an unmodelled source file or a stale checksum is a
    printed drift finding on every rebuild, not a discovery someone has to re-make.
  - The release manifest moved to `rel:v030` (v0.2.0 never contained `flow.rs` or `budget.rs`;
    freezing today's checksums under that tag would assert files into a release that never
    carried them) and now `INCLUDES` the skills tree, which closed a true
    `unreleased_component` complaint. `cap:adopt` is allocated to `cmp:skills` and realized by
    `adopt/SKILL.md` — a capability whose implementation is a skill, stated as such.

  The graph is now 173 nodes / 324 edges (was 125/175), the export stays byte-identical across
  rebuilds, and the gap list is down to three — `cap:kit`, `cap:freshness`, `cap:adopt`, each
  genuinely unverified — every one a thing to build, none a modelling error. Gaps fell 16 → 3
  because the model was corrected, not because any probe was loosened.

### Fixed

- **A flow's cycle now reports every step caught in it, not just one walk through it** (F7, the
  storyflow trial). `flow_report`'s `cycles` carries `members` — the full strongly-connected
  cluster — alongside `path`, the representative closed walk, because they are different claims.
  The walk can be shorter than the cluster, and on storyflow it omitted `p-prompt`: the hand-off
  to the human, and the entire reason that process is a loop rather than a line. reflow2's own
  loop model is worse still — the cluster is six phases and the walk is three — and
  `model_the_loop.py` now prints which members the walk leaves out, so the probe demonstrates
  the failure it was built from. The behaviour was always correct; only the report was wrong,
  which is the no-silent-truncation rule reaching a field nobody thought of as truncated.

- **`single_point_of_failure` no longer flags shared libraries** (F6, the storyflow trial —
  7 of 15 components → **5**, and the two that went were the only impossible ones). A library
  imported by every service is a *perfect* articulation point, and the suggested repair,
  `add_redundancy`, is incoherent for it: you cannot run a second copy of a library to survive
  its failure.

  BL-5's second pass scoped candidates to node *types* that operate — only things that operate
  can fail. This is the same lesson one level down: `Component` covers both a running service
  and a linked library, and topology cannot tell them apart because a library API and a service
  API are the same shape in the graph. The discriminator has to be stated, and the schema
  already had it — `Interface.medium`, whose values include `library`. A component whose
  contracts are *all* carried by a library is coupled at build time, not run time, so it is not
  a runtime failure unit. A mix still counts: anything carried at run time makes it a thing that
  can fail at run time.

  **The default is `REST`, so a design that says nothing is unchanged** — silence has to be
  earned by an explicit `library`, which is the right direction for a detector that must never
  go quiet by default. The `adopt` skill and the consumer AGENTS.md now both say to state
  `medium`, because a fix nobody writes the signal for is not a fix.

- **The installer now meets projects as they actually are** (BL-27, F1/F2 from the storyflow
  trial). The pointer line goes into **every** instruction-file convention a project already has
  — `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, `.github/copilot-instructions.md`, `.cursorrules`,
  `.windsurfrules` — pointing at wherever reflow2's own instructions landed, never at itself.
  The previous fix protected `AGENTS.md` alone, so storyflow (which carries `CLAUDE.md` and no
  `AGENTS.md`, the commonest shape in the wild) got a fresh `AGENTS.md` and the file its agent
  reads first never mentioned reflow2 — the whole kit invisible on the primary path.

  And the closing next-steps message now branches on **the project** — a bounded source-file
  count, with the evidence stated — rather than on whether reflow2 happened to write a sidecar.
  A repo with code is pointed at `adopt`; an empty directory still gets `genesis`; and an
  *update* whose graph is still empty gets the adopt hint too, which is the case that would
  otherwise repeat the failure for anyone who installed before the skill existed. Before this,
  a 2,643-file system was told to describe, in a paragraph, what it wanted to build.

  Verified against four shapes rather than the one the earlier fix assumed — `CLAUDE.md` only,
  foreign `AGENTS.md` plus `CLAUDE.md`, empty directory, and a re-run for idempotency — plus the
  real storyflow repo, where `--check` named the single missing change and the run applied it.

- **The adopt pass's noise floor** (BL-42), both halves, measured on the same 122-node storyflow
  graph that found them: **gaps 51 → 38, defects 31 → 19, total output 82 → 57 — with every true
  finding preserved**, including the `generation_plus ↔ media_service` cycle.

  `unrealized_capability` now reads a claim the modeller already made instead of guessing from
  topology: a component marked `realized` **asserts that it exists**, so a missing artifact there
  describes how far the artifact layer reaches, not a hole in the design. A `planned` or
  `in_progress` component still gets the forward-looking question, so designing forwards is
  unchanged. The count survives as `graph_report.realization` — the same bargain BL-23 struck:
  drop the question, keep the number. There is deliberately no threshold or proportion; BL-5's
  lesson was that a loud detector needs a different *question*, not a tuned number.

  HEAL's `orphan_node` no longer covers Requirements or Capabilities. DETECT already asks both
  (`unsatisfied_requirement`, `unallocated_capability`), they were never repairable — each mapped
  to a `generate_owner` stub `apply_heal` can never apply — and the docs' own division puts
  meaning in gap-surfacing and structure in HEAL. Four independent trials complained about this
  double-count (ophyd 15, 3dtictactoe 10, the self-host run, and storyflow where it was **20 of
  31 defects**). The Artifact orphan stays: DETECT has no counterpart for a file that realizes
  nothing. Closing the gap also required teaching `unallocated_capability` that a `Flow` is
  structure (BL-37), or a loose capability on a process-only graph would have gone silent
  entirely. On reflow2's own design graph, defects fell 14 → 9.

- **`graph_report` counted only the node types it chose to itemise** (BL-43). The storyflow import
  wrote 122 nodes and the report said 109; the 13 missing were exactly the Fragments — the whole
  provenance ledger, invisible to the surface an agent reads first. `total_nodes` is now every
  node in the graph, counted from the **schema** rather than a second hardcoded list, so a node
  type added later cannot silently drop out the way `Fragment` did. `design_nodes` keeps the
  lifecycle-ordered itemisation, and a new `other_counts` names everything outside it — in the
  payload and in the Markdown. Rule 6 (no silent caps) applied to reporting.

### Added

- **The `adopt` skill** (BL-27) — genesis's sibling, pointed backwards: bring a system that
  already exists under design control. The ninth kit skill, structured as the accepted
  reverse-engineering lifecycle (gather → scan → analyze static+dynamic → recover → validate),
  with every trial-earned discipline encoded: intent never inferred from the implementation
  that satisfies it; structure from imports and calls, never prose; breadth-first coarse
  granularity over the whole repo (one Interface per contract, a vendored mass as one opaque
  Component) with one atomic `import_graph`; found documents weighed before trusted; the test
  suite actually *run* and fed to `reconcile_verification`; recovered rationale as
  provenance-marked Decisions, found limits as budget Constraints, found processes as Flows;
  and a closing validation pass holding every finding to "true of the system, or an error in
  the model". The installer's brownfield next-steps message and the consumer AGENTS.md now
  point at it. Deliberately not yet included: incremental deepening, which stays behind the
  frontier-marking work on BL-27.

- **The P4 reconcile — the last feedback loop closes, and the phase trial goes fully green**
  (BL-30's M half). `reconcile_verification` completes the family: `reconcile_artifacts` asks
  about the code, this asks about the *outcomes*, `reconcile_deployment` about what runs. The
  caller supplies what a real run reported per check (`passed`/`failed`/`skipped` — anything
  else is rejected by name and the batch survives); the graph names every divergence from what
  it believed. "Recorded passing, run reported failed" — believed proven, actually broken, the
  reflow1 failure in miniature — sorts first and records at severity high. Divergences are
  persistent `unresolved_drift` gaps with P4-appropriate advice, auto-resolved when a later run
  agrees; event identity is the (declared, observed) pair, so a check that flaps leaves its
  history visible per axis Z. A partial run is never read as absence; `exhaustive` names the
  passing/failing claims the run did not cover. The phase trial's P4 probe now injects the
  divergence, and the instrument reached **13/13 — fully green and exits 0 for the first
  time**: the standing measurement for the failure that sank the original reflow now passes,
  and works as a regression gate. This is also adoption's dynamic-analysis receptor (BL-27's
  RE-lifecycle mapping).

- **Converting an existing project actually works end to end** (BL-27, the conversion step —
  probed against a brownfield-shaped repo before and after). Three installer fixes in
  `reflow2_init.py`: the project's own `AGENTS.md` gains **one marked pointer line** to
  `REFLOW2.md` (append and report, never overwrite — same rule as the merged MCP configs;
  without it the agent read the one file that never mentions reflow2 and the whole kit was
  invisible, BL-22's lesson again); **`.reflow2/` is gitignored** (appended or created,
  idempotent — the installer previously had no `.gitignore` handling, so converted repos
  started tracking a RocksDB directory); and the closing **next-steps text branches** —
  brief → genesis for a fresh directory, record-what-exists for an adopted one, instead of
  pointing every brownfield user down the greenfield path. `--check` previews all of it.

- **`design_without_intent`** (BL-27) — the fifth phase-coverage nudge, for the pure brownfield
  starting state: capabilities and components seeded from code with zero requirements
  previously reported *nothing at all*, because `unmotivated_capability` is deliberately gated
  on requirements existing. One project-level nudge at 0.72 (the top of the nudge band — on an
  adopted system the first question is *what is this for*, not *how is it structured*), gone
  the moment one requirement is stated, with wording that directs intent to sources outside
  the implementation — a requirement inferred from the code it describes is satisfied by
  construction and can never contradict anything.

- **The as-fielded reconcile** (BL-9). `reconcile_deployment` is the P5 sibling of
  `reconcile_artifacts`, one phase later: not *does the code match the design?* but *does what is
  **running** match what the design declares?* The caller supplies per-environment observations
  (an empty `running` list is a positive statement); the graph compares them against
  `DEPLOYED_TO` and reports `deployment_missing` (declared active, not running),
  `deployment_undeclared` (running, never declared) and `deployment_contradicted` (running while
  declared planned/rolled back). Unknown ids are reported, a partial observation is never read
  as absence, and `exhaustive` names the declarations the observation could not see. Only
  Releases run and only Environments host, so the original reflow's library-plugin false
  positive — every component expected to appear as a running thing — is impossible by
  construction. Recorded divergences are persistent `unresolved_drift` gaps (with
  deployment-appropriate advice) that a later agreeing observation resolves automatically; the
  design-side answer is `deploy_to` with the true status. The phase-coverage trial's P5 probe
  now injects a real divergence instead of checking the tool exists — **P5 2/2, phase trial
  12/13**; the one remaining miss is BL-30's `reconcile_verification`, the last of the three
  feedback loops. New `DriftEvent.drift_type` values are additive enum growth (validation runs
  on write; the stamp is unchanged). The **as-fielded viewpoint** joins the catalogue.

- **Budgets — the path-cumulative quantity rollup** (BL-11). The vocabulary was waiting: a
  `Constraint` (which had **no write side** — the fourteenth recurring-lesson instance) now
  carries `quantity` (unit-bearing name: `mass_kg`, `latency_ms`), `limit` and `direction`, and
  each `CONSTRAINS` edge carries the target's `contribution` and its `basis`
  (estimated/evidence/measured — the coupling-weight rigor ladder). `add_constraint` and
  `constrains` are the write side; `budget_report` rolls it up: the stated total against the
  limit, basis coverage, the worst dependency path among contributors (contracts collapsed —
  end-to-end latency, mass down a chain), and an honest verdict. The discipline is
  graph-analysis's: an unstated contribution is **never zero** — it makes the verdict
  `incomplete` and is listed by name, because a partial sum passed off as a total is how budgets
  lie. No limit → `ungated`, not passing; a cycle among contributors refuses the path claim by
  name. The **measures viewpoint** (≈ SV-7) joins the catalogue, closing its last ⬜ row — all
  ten viewpoints now render.

- **Evolution and provenance viewpoints** (BL-40, second increment — the catalogue's last two
  projectable rows). **Evolution** (≈ SV-8 proper, axis Z): the epoch chain drawn from what is
  stated — solid arrows for `PRECEDES`, dotted arrows labelled `sequence` when only the property
  orders them — with what happened at each epoch via `AT_EPOCH`/`OCCURS_DURING`. The two stated
  orderings are cross-checked: a disagreement is confessed, a `PRECEDES` cycle is confessed as
  the chain contradicting itself, an epoch neither chained nor sequenced is confessed as
  unplaceable, and a ChangeEvent pinned to no epoch is confessed as the axis-Z discipline
  broken. **Provenance** (≈ AV-2-ish): authored-vs-inferred per node type with `inferred` nodes
  listed by name (the trust-relevant set), and the Fragment ledger — each source with what it
  `YIELDED` and the action taken; an unstated origin, a mute Fragment, and a dangling YIELDED
  edge are all confessed. Every new confession class is exercised by a torture graph during
  development; the committed design graph still projects with the same 2 true confessions.

- **The viewpoint catalogue doubled, and got a home** (BL-40, first increment). Three views join
  functional/structural/traceability in `tools/render_views.py`, all pure projections:
  **operational flow** (≈ OV-5b/OV-6 — steps in stated order, transitions labelled with their
  `role`, cycles rendered as clusters of mutually reachable steps, reported never judged; the
  seed's standing confession "no flow view is expressible" is now answerable because BL-37 made
  it so), **as-released** (≈ SV-8 — what each Release shipped with checksums frozen at cut, the
  built-but-not-shipped diff, deployments), and **decisions** (the record of *why*: rationale,
  standing, and what each decision governs). `--graph-path` projects a live graph directory via
  `reflow2-mcp --export`, so views no longer require a hand-managed export file — with the
  single-writer rule surfaced honestly when a session holds the graph.
  [docs/viewpoints.md](docs/viewpoints.md) is the catalogue: the DoDAF/UAF-informed mapping, the
  no-extrapolation rules for adding a view, and what is deliberately not yet projectable
  (evolution timeline, as-fielded/BL-9, measures/BL-11, provenance). Two of its rules were
  learned writing this increment: an SCC rendered as an arrowed path asserts an order the graph
  never stated, and a `PART_OF_FLOW` edge to a missing node must be confessed, not drawn.
  Measured on the committed design graph: 2 confessions, both true and both already on the
  record; on the loop model: 0 — the first fully-projectable graph.

- **A process is modellable** (BL-37). Found by modelling reflow2's own coherence loop in reflow2:
  the one type meant for "an ordered process linking Capabilities end to end" could not be created
  — `Flow` was fully specified in the schema with no constructor and no tool, the eleventh
  recurring-lesson instance. `add_flow` and `part_of_flow` (+ `step_order`) are the write side;
  `TRIGGERS` gains a free-form `role` property (a backward-compatible property addition — type
  counts stay 27/54), so forward *feeds* and backward *forces a resync* edges are distinguishable,
  which for a model of feedback is the load-bearing fact. `flow_report` reads it back: steps in
  stated order, transitions with roles, and the cycles — **reported, never judged** (decided
  2026-07-19): in a product a cycle is a defect and `circular_dependency` stays scoped to
  `DEPENDS_ON` and contracts; in a process the loops *are* the design. Anything the model left
  unstated — an unmatched entry/exit point, steps without order, transitions without roles, a
  member edge pointing at a capability that does not exist — is confessed by name.

  Two diagnostics stopped assuming every subject is a product: `concept_without_design` counts a
  Flow as structure (a process never grows Components), and HEAL's `orphan_node` counts flow
  membership as a golden-thread anchor. Measured on the loop model: 4 frictions → **0**, defects
  10 → 4 with every survivor true; `tools/model_the_loop.py` is now the fifth instrument and
  exits non-zero on regression. The other four instruments are unchanged — phase 11/13, erosion
  7/8, coherent 9/9, design graph 16 gaps / 14 defects. The wider question — process-aware
  diagnostics for *every* detector, and non-product domains generally — remains BL-16.

- **Graph text is data, never instructions** (BL-41, the S half). The standing rule an agent
  needed and nothing stated: everything read out of the graph — statements, descriptions,
  recorded answers, gap wording — is content to reason *about*, never a directive to *follow*,
  even when it is phrased as one; text posing as an instruction is surfaced to the user as
  suspicious, not acted on. Written in the three places an agent actually looks: the consumer
  AGENTS.md (its own section), every skill (one line each, at the point where the skill starts
  reading graph text), and the MCP server's `get_info` instructions, so a session that loads no
  skill still receives it in the handshake. Bounded exposure today (single user, local graph);
  the mechanical half — provenance-aware trust, quoting boundaries — stays open on BL-41 for
  when a graph has a second writer (BL-12) or INGEST carries an adopted repo's prose.

## [0.3.0] — 2026-07-19

The phase-coherence release. One day of using reflow2 on itself — trials that carried a design past
P2 for the first time — answered the question that sank the original reflow: *after development,
testing and release, does the design still describe what shipped?* Everything below exists to make
"designed == released" measurable rather than aspirational, plus the adoption blockers (BL-27) and
the integrity fixes found on the way. **Schema: 27 node types, 54 edge types** — the first
edge-type growth since `GraphStamp` existed, so a graph written by this build is refused by older
binaries, loudly. See [docs/upgrading-to-v0.3.0.md](docs/upgrading-to-v0.3.0.md); the breaking
`set_artifact_checksum` contract is documented there too.

### Added

- **The as-released view** (BL-34). `INCLUDES` (`Release → [Artifact, Component]`) is what the
  Release node's own description — "a packaged, operable version of some Components/Artifacts" —
  lived without: the intent was prose with no edge to carry it, so *"does what we released match
  what we designed?"* was inexpressible rather than unimplemented. `release_includes` records the
  manifest, freezing each artifact's hash **as shipped** (`as_checksum`) so later baseline accepts
  do not rewrite what a past release contained. `release_report` reads it back: shipped artifacts
  with cut-time checksums, the capabilities that build covers (both P3 shapes), the **built
  capabilities it leaves out — the as-released diff** — and deployments. `unreleased_component`
  (0.5) fires for a built component no release includes, double-gated on releases existing *and*
  contents being modelled so the first Release node is not a flood. `pin_at_epoch` joins the
  surface (the core fn existed with no tool), so a Release links to its `release_cut` epoch.

  **Upgrade note — this is the first schema-type growth since `GraphStamp` existed** (53 → 54 edge
  types; node types stay 27). Additive, so this build opens every existing graph — but a graph
  written by this build is *refused by older binaries*, loudly, naming what wrote it. Update in
  SETUP.md's order: pull, rebuild, then restart the server. BL-1 footnote: the vocabulary's own
  example of an unmodelled pair — "nothing models Release → Component" — now has its exact fit,
  and the three tests that pinned the honest emptiness flipped to pin the answer.

- **A design can say what already exists, and what it inferred** (BL-27, two of five blockers on
  adopting a system that already exists).

  `add_capability` takes an optional `status`, and `set_capability_status` moves one afterwards —
  the sibling of `set_requirement_status` and `set_verification_status`, for the same reason: a
  capability's standing changes far more often than its description, and re-stating the
  description to move it invites drift. Nothing hardcoded `planned`; the constructor simply never
  set the property, so every capability took the schema default. On the greenfield path that
  default is right and stays untouched — a new capability really is planned. On the brownfield
  path it is unusable: ophyd's 15 shipped, under-test capabilities all landed `planned`, so the
  graph asserted that a production system was entirely unbuilt. Settable **at creation** because
  correcting it afterwards is two writes per node, which is what an adoption pass does least well.

  `provenance` is now a property on `Requirement`, `Capability`, `Component` and `Interface` —
  the four types an adoption pass reads back out of a running system — reusing
  `Fragment.provenance`'s exact vocabulary (`authored` default / `planned` / `inferred` / `healed`
  / `reconciled` / `imported`) so there is one word for one idea. `set_provenance` writes it, and
  `import_graph` carries it at create time, which is the path an adopt pass should actually use.
  `inferred` is the value that earns the property: a Requirement backed out of the code that
  implements it is satisfied by construction, so it can never contradict anything and a graph full
  of them says nothing — but only if a reader can tell. Ophyd had nowhere to put that and wrote
  `[EXTERNAL — …]` into the statement text, which is not queryable.

  Adding properties leaves the node and edge type counts at 27/53, so `GraphStamp` does not move
  and existing graphs still open — the backward-compatibility argument BL-19 sets out, now
  exercised. Existing nodes read `provenance` as absent rather than `authored`, since defaults
  apply on create and are not backfilled; an export/import round trip resolves that, and there is
  a test pinning that provenance survives one.

- **`possible_duplicate` — duplicate detection that computes something** (BL-27, the last of five
  blockers). HEAL has had a `duplicate` category all along, and it fired on a `DUPLICATES` *edge* —
  reporting a conclusion somebody had already reached and recorded. It computed nothing, so it could
  never fire on a duplicate nobody had found, which is every duplicate an adoption pass exists to
  discover. 3dtictactoe modelled two components holding an identical set of three capabilities, one
  of them dead code with a subtly wrong victory check, and `detect_defects` returned eight defects
  with no `duplicate` among them. That is `gap-surfacing.md`'s first discipline exactly — *detectors
  read computed signals, not raw edge-name filters* — the trap it records as storyflow's biggest.

  The computed rule is structural: two Components sharing at least two allocated Capabilities and at
  least 80% of their sets by Jaccard overlap. Both thresholds are guards against the ordinary case —
  two components providing the one capability they share is normal design, and a large component
  containing a small one's whole set is not a duplicate of it.

  **It asks rather than repairs, and that is the load-bearing decision.** `HealCategory::Duplicate`
  maps to an applicable merge that `apply_heal` executes — deleting a node and re-pointing its
  edges, with no snapshot and no undo. Merge is safe only because a human asserted the endpoints;
  driving it from a heuristic would let the machine delete a component it merely suspects. A HEAL
  issue also cannot be dismissed, where a gap can be acknowledged — and `unexpected_coupling` is the
  cautionary tale of a detector firing on correct architecture with no way to stop it. So the two
  compose: DETECT asks, the user confirms by drawing the `DUPLICATES` edge, HEAL merges. A pair
  already carrying that edge is skipped, so nothing is reported twice.

  This complements rather than replaces the semantic rule `heal-process.md` plans on
  `resolution: fuzzy_then_vector`, which needs the deferred `EmbeddingBackend` and finds things
  *described* alike where this finds things *wired* alike.

- **`unmotivated_capability` — the direction DETECT was blind in** (BL-27, the fourth of five
  blockers). `detect_gaps` walked Requirement→Capability only, so a Capability satisfying no
  Requirement was never reported. Both brownfield trials ran the probe deliberately — ophyd seeded
  `cap:qserver-auth` with no `SATISFIES` and got 13 `unsatisfied_requirement` gaps and silence
  about the orphan; 3dtictactoe did the same with `cap:draw-detection` and got four gaps, none
  about it.

  It matters because the two directions are not equally likely on the two paths. Capabilities are
  normally created *from* requirements, so in greenfield an orphan is a half-finished thought.
  Reading a system backwards inverts that: the capability is the thing that indisputably exists,
  and one nothing justifies is either a requirement nobody wrote down or dead code.

  Severity reads `Capability.provenance` rather than being fixed — 0.55 authored, 0.70 `inferred`.
  Ophyd asked for this to outrank `unsatisfied_requirement` "on a brownfield graph", and no fixed
  number can honour that qualifier; provenance is what tells the two readings apart, so the gap
  leads the list exactly where the trial wanted it to and sits below the requirement gaps
  otherwise. This is the first thing to consume the property added above.

  HEAL was deliberately not given the symmetric check, and a graph with capabilities but zero
  requirements still reports nothing — both are recorded in the backlog with the reasoning rather
  than left to be rediscovered.

### Added

- **`reflow2-mcp --import` — a design can be loaded without speaking MCP** (BL-39). `--export` has
  existed since BL-20, so a design could be read out of a graph by a script and never written back.
  Combined with the store being single-writer, that sealed a session: a committed export, a backup,
  or a design built on another machine could only enter through the `import_graph` *tool*, as one
  inline argument — 42 KB for reflow2's own design. The practical effect was that the consumer skills,
  which run against the live graph, could only ever see a design the session itself built. Backwards,
  for a tool whose selling point is that a design outlives the session.

  `--import <file>` is the sibling, and takes `-` for stdin so `--export` on one machine pipes into
  `--import` on another. Upsert, matching the tool. It reports what landed **and what did not** — an
  import that quietly skipped half a design would be the worst kind of success, so any edge whose
  endpoints were missing is printed by name rather than dropped.

  The lock stays — single-writer is the storage model, not an oversight — but it is no longer a
  mystery. RocksDB's *"IO error: While lock file… Resource temporarily unavailable"* named neither the
  cause nor the fix; it now reads *"another process already has the design graph open… stop that
  server and run this again."*

### Added

- **The confirmation ledger — when was each claim last checked against reality, and what was the
  answer** (BL-35, the keystone of the phase-coherence thread). The erosion trials' founding
  observation was that an eroded design and a genuinely coherent one both reported *quiet*:
  structural completeness was all that was measured, and it is true in both graphs. `confirmation_ledger`
  (core + MCP + a `graph_report` rollup) gives every capability with built artifacts one of three
  states that used to be indistinguishable: **drifting** (an observed divergence is unanswered — and
  a persistent `unresolved_drift` gap at 0.75, so the open question survives the session that found
  it), **confirmed** (examined, with the claim history visible: design_holds vs design_updated
  counts, design edits on the record, `last_claim_at` from dated claims), and **unexamined** (nobody
  has ever looked — *not* the same as confirmed, which was the entire point).

  Two schema facts made it clean: `DriftEvent.resolved` — declared with `default: false` and written
  by nothing, the twelfth "unreachable capability" instance — is now flipped by the accept that
  answers the drift; and an accept's `CHANGED` edge carries `accepted_baseline: true`, so a
  disposition claim is distinguishable from ordinary change history. Deliberately not built: lie
  detection — five `design_holds` claims with zero design edits is the erosion signature and the
  ledger makes it legible, but judging a specific claim false is semantic, and a deterministic
  detector would fire on every stable design with cosmetic churn. The ledger reports; the human
  judges. Measured: erosion 4/8 → 5/8, coherent-erosion 5/9 → 6/9.

### Changed

- **Accepting drift is a two-sided decision** (BL-33). `set_artifact_checksum` — "an accepted change
  is the new baseline" — updated the code-side baseline and asked nothing about the design. That is
  the erosion mechanism verified by trial: run *test fails → fix → accept* N times, every step
  locally reasonable, and the design is fiction while reporting zero gaps. The third option —
  *accept the file, leave the design alone, say nothing* — no longer exists.

  `disposition` is required. `design_holds` records a dated `ChangeEvent` claiming the change
  carried no design meaning (idempotent per artifact+checksum; the claim can be wrong but not
  silent). `design_updated` names the `record_change` event from the design-side edit and links it
  to the artifact — one change, both sides, and the first `ChangeEvent` in the codebase that
  originates from the build rather than the design. A phantom event reference is refused before the
  baseline moves; the refusal caught the coherent-erosion trial itself accepting in the wrong order.
  Measured: erosion 3/7 → 4/8, coherent-erosion 4/9 → 5/9. The `link-artifacts` skill and consumer
  AGENTS.md teach the new contract, including: when in doubt, the honest answer is `design_updated`
  — ask the user what the fix changed.

### Fixed

- **A status is a claim the structure must back** (BL-31). `status_contradiction` (0.70) fires on a
  Capability `verified` that no passing check verifies, and on a Requirement `met` that nothing
  satisfies — the latter previously invisible to everything, because `met` silences
  `unsatisfied_requirement` by design. Its first catch was this repo's own design graph: `cap:kit`
  claimed `verified` and nothing automated checks the installer; the status was ruled wrong and
  downgraded on the record.

- **The epoch chain is drawable** (BL-36). The `precedes` tool orders one `DesignEpoch` after
  another — the core fn existed with no tool, on the axis whose whole job is making history
  legible. The coherent-erosion instrument draws the chain per fix cycle, walks it back out of the
  export, and with it reached 9/9 — the first instrument fully green.

- **The server says who it is** (BL-32). `graph_report` gains `served_by` — the reflow2 version the
  binary was built from, and the binary's mtime — because an MCP session started before a rebuild
  keeps serving the old surface with nothing to say so; that state is now visible from inside the
  session, and the upgrade doc makes checking it the post-restart step. The consistency check
  (handshake version must equal report version) caught a bug as old as the surface itself:
  `Implementation::from_build_env()` reports the **rmcp library's** version, so every initialize
  handshake had introduced this server as "2.2.0". It now introduces itself as `reflow2-mcp` at its
  own version.

- **A new drift is a new `DriftEvent`** (BL-33, the S sub-piece). The event id carried no notion of
  which state the artifact had drifted *to*, so a second drift hashed to the first one's id and was
  silently skipped — five fix cycles left one event, and "drifted once" was indistinguishable from
  "drifted five times, capability never revisited", erasing exactly the accumulation that reveals
  erosion. The observed checksum is now part of a `checksum_change` event's identity ("the artifact
  became X while the design believed Y"), so re-observing the same X dedups — the property the old id
  existed for, kept — while a drift to X′ is a new event. State-shaped kinds stay keyed without it:
  "still missing" re-observed is the same unresolved divergence. Axis Z's *never overwrite the past*
  now holds on the as-built side, and `DriftFinding` reports the observed checksum. The erosion
  trial retains 5 events for 5 drifts, with its probe tightened from `> 0` to an exact count.

- **A failing check is a gap, not a satisfaction** (BL-30, the S half). The erosion trial's headline:
  `build_without_verification` asks *"how will you confirm this works?"* and was closed by a test
  proving it does not — with `detect_gaps`, `detect_defects` and `graph_report` byte-identical
  between the passing and failing cases. The later phases counted test nodes and ignored test
  results, which is the reflow1 failure in miniature.

  A `Verification` with `status: failing` now raises **`failing_verification`** at severity 0.8 —
  above every absence-shaped gap, because a requirement nothing satisfies is work not started while a
  failing check is work *proven broken* — anchored to both the check and what it checks, clearing
  when the check goes green. The phase nudge still closes when a check exists; the difference is the
  silence is filled with the right signal. And `verification_coverage` now counts a check that
  **passes**, not one that exists: `planned`, `failing`, `skipped` and `blocked` all mean "not
  currently confirmed". Measured: `phase_trial` P4 1/4 → 2/4, `erosion_trial` 2/7 → 3/7. The M half —
  `reconcile_verification`, feeding real test results in — stays open.

- **`single_point_of_failure` only names things that can fail** (BL-5, second pass). The first fix
  asked whether removal increases the count of non-trivial subsystems — the right question about
  topology, measured at fixture scale. On the first real 96-node design it named 22 nodes, most of
  them Requirements and Capabilities that are load-bearing *because* they are cross-cutting: a golden
  thread converges on intent by design, so in a tree most internal nodes pass any purely topological
  test. The missing filter was not a threshold but a category: the suggested fix is `add_redundancy`,
  and redundancy is only coherent for things that operate. Candidates are now scoped to `Component`,
  `Interface`, `Resource` and `Environment`. Measured: 22 → 4, the survivors being exactly the
  plausible ones (`cmp:service`, `cmp:init`, `cmp:export`, `ifc:graph-export`) — and with it the
  design-graph instrument reached zero known-false output.

- **`unrealized_capability` accepts both shapes the schema allows at P3** (BL-38). `REALIZES` is
  declared `from: Artifact, to: "*"`, so "this file realizes the capability" and "this file realizes
  the module" are both valid, and `link_artifact` invites either — but the detector walked only the
  first, silently mandating one of two equal modellings and flooding anyone who picked the other:
  11 of 33 gaps on reflow2's own design were "Nothing builds capability X" for capabilities shipping
  in the binary that reported them. A capability now also counts as realized when an artifact
  realizes a Component it is allocated to (`art -REALIZES-> cmp <-ALLOCATED_TO- cap` — the path that
  was present in every false positive and never walked). Measured: the design graph went from 33
  gaps to 16, and every survivor is a genuinely unbuilt capability.

- **`dead_end` no longer fires on a pure container** (BL-38). The design network excludes `CONTAINS`
  on purpose — decomposition is not traceability — which made an assembly whose one job is holding
  modules read as "not connected to anything". Assemblies are now exempt: they speak through their
  children, which are flagged individually if disconnected. A contained leaf hosting nothing is the
  true case and still fires.

- **The installer no longer destroys a project's own `AGENTS.md`.** `reflow2_init.py` copied the kit's
  `AGENTS.md` over whatever was there and reported it as an ordinary `AGENTS.md` line in the install
  summary — no warning, no backup, no refusal. Verified on a scratch repo: a project's build
  instructions were replaced and the run reported success. That is every brownfield target, and it is
  the file a project actually runs on.

  A destination the kit did not author is now left alone, and the kit content goes to `REFLOW2.md`
  beside it; both `--check` and the install say so, and the kit's own header tells the reader where to
  find it. Ownership is decided by the kit file's first heading rather than a marker, so kits
  installed before this check are still recognised as ours and refresh in place. The greenfield path
  is unchanged and repeat installs stay idempotent.

  The BL-27 entry describing this understated it — it read "cannot install into a repo that already
  has its own `AGENTS.md`", when in fact it did not refuse, it overwrote. Corrected there too.

- **The repo's `AGENTS.md` now routes by audience.** Developing reflow2 and using reflow2 are
  different jobs with different files, and nothing said so at the top of the one an agent lands on
  first. It now opens with a two-row table: build reflow2 → this file plus `docs/sharpening.md`; design
  your own project → the consumer kit, installed by `reflow2_init.py`, and the build commands here are
  not for you.

- **`apply_heal` checks the proposal instead of trusting it** (BL-29). It used to execute whatever
  it was handed. Verified before the fix: a hand-written proposal carrying a made-up issue id and a
  `Merge` naming two capabilities that no detector had called duplicates was applied, and deleted
  one of them — `applied=true, operations_applied=1`. `ApplyHealReq` deserializes caller JSON
  straight off the MCP surface, so any client could do it, and a merge has no snapshot and no undo.

  Propose-then-apply is described as the whole point — a proposal can be reviewed, capped and
  audited before anything changes — but nothing bound the applied proposal to one HEAL actually
  made. Now every operation must match one HEAL derives from the graph **as it stands**, and
  anything else is refused before a single write, so a rejected proposal leaves the graph untouched.
  A stale proposal fails the same way: resolve the defect by hand between propose and apply and the
  merge no longer runs. The issue→operation mapping is shared by both sides rather than written
  twice, so they cannot drift apart.

  Worth knowing: `requires_human_review` is computed per *proposal* and `apply_heal` has never
  consulted it. It reports that generative stubs are present; it was never a gate on applying the
  structural half, and the check above is what actually guards that path.

- **A merge says what it could not carry** (BL-29). `HealReport` gains `discarded`. A merge keeps
  the survivor's own properties and re-points the removed node's edges, so three things were being
  let go in silence — the removed node's properties (its name, description and status went with
  it), an edge whose other endpoint was not a known node, and an edge triple both nodes already had,
  where `create_edge` is an upsert so the removed node's edge properties overwrite the survivor's.
  Each is now reported with the reason. That is rule 4: the loss is often the right call, but it may
  not be silent.

- **A cross-type merge is refused rather than half-applied** (BL-29). `DUPLICATES` is declared
  `from: "*" to: "*"`, so `Requirement DUPLICATES Component` is schema-valid. Merging across types
  re-points one type's edges onto another and gets rejected part-way through — after earlier
  operations in the same proposal have already committed, since atomicity is per-operation. It is
  now refused at proposal time and lands in `skipped_operations` with the reason.

- **A gap that names nodes now outranks a phase nudge** (BL-27, the third of five blockers).
  `detect_gaps` ordered purely on severity, which compared two numbers that are not on the same
  scale: the phase-coverage nudges carry fixed literals (`concept_without_design` 0.70,
  `build_without_verification` 0.65) while `unsatisfied_requirement` computes `0.5 + priority_bump`
  — 0.60 for the default `medium`, and until BL-28 no client on one major harness could write
  `priority` at all, so the losing number was a default nobody chose. Three brownfield trials
  watched the consequence independently at a 20× size difference: the top gap was an artifact of
  GENESIS's own seeding order, the actionable one sat below it, and an agent working the list
  top-down did the useless thing first.

  The sort now bands on anchoring before severity. A gap that names nodes describes something wrong
  **now**; a project-level phase nudge describes what comes **next**, and `gap-surfacing.md`
  already drew that line — discipline 8 puts phase-coverage in the *proactive* group, discipline 3
  says concrete beats abstract.

  The phase detectors themselves are unchanged, deliberately. Their inference is correct about the
  graph, and the aidrone trial recorded the greenfield behaviour as worth not regressing — GENESIS
  seeds P0/P1 and stops, the nudge fires, "the skill and the detector agree." It is demoted, never
  suppressed: with nothing anchored to report it is still the first thing the user sees. Both
  directions are pinned by tests, and the ordering is asserted over the real MCP path.

- **Every tool parameter declares a type** (BL-28). Six parameters — `gap_to_prompt.gap`,
  `apply_heal.proposal`, `import_graph.document`, `create_node.props`, `create_edge.props` and
  `reconcile_artifacts.observed[]` — were declared `serde_json::Value`, whose generated schema
  says nothing about the type. A client with nothing to marshal against is free to guess, and the
  clients guessed differently: grok build sent a JSON object, **Claude Code sent the object
  serialized as a string**, and the string was rejected. From Claude Code that removed the ask
  half of DETECT, the apply half of HEAL, graph restore/migration, and all property-setting on
  generic CRUD — four of the six are named in skills the consumer kit installs.

  The parameters are now declared as JSON objects, so the contract states what to send. The server
  still rejects a stringified object rather than accepting both shapes: taking either would be the
  silent fallback rule 4 forbids, and would hide the next client that marshals wrongly.

  Found by running `/genesis` on reflow2 itself from Claude Code
  ([trial](docs/trials/2026-07-18-selfhost-genesis.md)). Every existing layer was green throughout:
  `tools/smoke_mcp.py` passed all six because it sends Python dicts, and the Rust integration tests
  never cross the JSON boundary at all — the fourth and fifth instances of "a client we wrote"
  agreeing with itself and being wrong. The guard added here asserts the *published schema* instead
  (no advertised property without a type), which is the only layer that could have caught it.

## [0.2.0] — 2026-07-18

Fourteen backlog items, all of them findings from putting reflow2 in front of people and agents
who had not seen it. Two upgrade documents ship with this release:
[docs/upgrading-to-v0.2.0.md](docs/upgrading-to-v0.2.0.md) and
[docs/v0.2.0-what-we-dont-know.md](docs/v0.2.0-what-we-dont-know.md) — the second is the more
important of the two.

### Added

- **The design exports to a portable document, and back** (BL-20). `export_graph` /
  `import_graph`, in the core and on the tool surface. One mechanism doing three jobs: migration
  across an upgrade (export with the old build, import with the new), backup, and moving a design
  between machines.

  Deterministic on purpose — node types, ids, edges and property keys are all sorted, which is why
  the exported types use `BTreeMap` rather than the store's `HashMap`. Two exports of an unchanged
  graph are byte-identical, so a backup directory under version control shows what changed *in the
  design* rather than a fresh blob every run.

  Import is upsert and atomic: ids already present are overwritten, anything absent from the
  document is left alone, and a document that fails validation leaves the graph untouched rather
  than half-loaded. An edge whose endpoints are missing is named in the report, never dropped
  quietly. The document carries a `GraphStamp` saying which reflow2 wrote it.

- **The installer backs the design up before it changes anything** (BL-19). `reflow2_init.py`
  exports to `.reflow2/backups/design-<utc>.json` — beside the graph, never `/tmp`, which
  systemd-tmpfiles clears. A failed export is reported and does not abort the update, since the
  update may be exactly what fixes the binary that could not read the graph. `reflow2-mcp --export`
  prints the document to stdout so a script can back up without speaking MCP.

  **Backfill needed no new code:** importing applies the current schema's defaults, so a document
  written before a property existed comes back carrying it. Export with the old build, import with
  the new, and mixed-vintage nodes resolve themselves.

- **A graph records which reflow2 wrote it** (BL-19). `<graph>.meta.json` sits beside the store —
  never inside the directory RocksDB owns — holding the reflow2 version, schema version, and node
  and edge type counts. `open_rocksdb` reads it, compares, refreshes it, and the server reports any
  difference on stderr and in the log. Until now nothing was written to the graph directory at all,
  and validation runs on write and never on read, so a graph opened by a different reflow2 just
  behaved differently with no error and no marker.

  **One difference is fatal, and only one:** a graph written by a reflow2 whose schema knew *more*
  than the running one. That graph can hold nodes this binary has no vocabulary for, so opening it
  would silently show less of the design than it holds. Everything else opens and is reported —
  schema growth is additive, so refusing an older graph would lock someone out of their own design
  over a change that cannot hurt them.

  The type counts are the signal, not the declared schema version: that is `1` in every domain and
  has never been bumped.

- **The agent can report friction with reflow2 itself** (BL-21). A `report-friction` skill, with
  the trigger in the consumer `AGENTS.md` because a skill alone is not reliably discovered
  (BL-22). Everything reflow2 knows about its own weak points came from staged trials; ordinary
  use produces better evidence and was losing all of it.

  Redaction is the load-bearing part. A friction report naturally quotes the graph, and the graph
  is the user's design — so the skill reports reflow2-shaped facts (which tool, argument *shapes*,
  node *types*, counts, errors with ids masked) and asks before including anything of theirs. It
  never files without asking, searches for duplicates first, and falls back to writing a local
  file when `gh` is unavailable or the repository is unreachable — which is the normal case, since
  the repo is private.

- **`reflow2_init.py` refuses to install a skill that would silently fail to load.** A malformed
  `name`, one that does not match its directory, or a missing `description` makes a harness ignore
  the skill with no error anywhere. The installer now names the problem instead.

- **An answered question stays visible while its gap is open** (BL-25). `open_questions` returns
  `asked` (still waiting) and `answered`-but-the-gap-is-still-open, the latter carrying the reply.
  Answering settles nothing on its own: either the answer gets written into the design and the gap
  closes, or the gap is acknowledged. Until one happens, something is outstanding and the list
  says so.

  Found by re-running the self-host probe minutes after BL-4 shipped. Answering *"it is a library
  you build from source; no deploy layer is intended"* left the gap open and the question quiet,
  so a third session saw a bare open gap with no sign it had been asked — and would have asked
  again. BL-4's problem displaced one step.

- **Questions outlive the session** (BL-4). `gap_to_prompt` phrased a question, returned it, and
  forgot — it was the only tool on the surface that never touched the graph. So the next session
  re-derived the same gap and asked the same thing again, which the blind trial called *"the
  stateless-agent problem reflow2 is supposed to solve"*; it worked around it by copying questions
  into a Markdown file by hand.

  The serve pass now records a `Question` node at a derived id, `ASKS_ABOUT` the nodes the gap
  concerned, keeping the wording the user actually saw. New tools: `open_questions` (still
  awaiting an answer), `answer_question`, `withdraw_question`. The **where-am-i** skill reads them
  before anything else and repeats the original wording — being asked the same question twice,
  worded differently, is how someone learns the tool is not listening.

  Re-asking updates the phrasing but cannot reopen an answered question, so a later session cannot
  erase what an earlier one learned.

  This adds the first new node type since the schema was written: **27 node types, 53 edge
  types**. Purely additive — validation runs on write and no existing node carries the label — so
  existing graphs are unaffected (BL-19).

- **The assembly hierarchy is reachable** (BL-2). `contain_component` nests one Component inside
  another, and `add_component` takes an optional `level`. Both were needed: `hierarchy_issues`
  had shipped as a read tool with no writer to feed it, returning `[]` for want of input rather
  than because a design was healthy. Exposing the containment alone would have been worse than
  nothing — every component defaults to `component`, so each nesting would have reported a false
  `level_mismatch`.

- **`set_requirement_status`** (BL-3) — `proposed` / `accepted` / `deferred` / `dropped` / `met`.
  The field was in the schema and read by DETECT, but nothing could write it, so a blind trial
  put the word "ASSUMED" in the statement text instead.

### Changed

- **Per-file verification coverage is counted, not asked** (BL-23). An `Artifact` with no
  `VERIFIES` edge of its own no longer raises a gap; `graph_report` gains a *Verification
  coverage* line instead (`7/7 capability(ies) verified; 0/22 artifact(s) carry a check of their
  own`). Capabilities are unchanged — nothing proving a behaviour works is still a real gap.

  The rule was not wrong, it was loud. Modelling reflow2's own design put it at 22 of 25 gaps, on
  a crate whose capabilities are all tested, and a list that cannot reach zero teaches you to skim
  it. On that same 119-node graph the change takes **25 gaps to 3**.

- **A cross-community coupling is a signal, not a gap** (BL-6b). It no longer appears in
  `detect_gaps`; `graph_report` lists it under "Surprising couplings" as it already did, and
  `surprising_connections` returns it whole. Nothing was lost — it stopped demanding an answer.

  It fired on correct architecture. An `Interface` joins two clusters by construction, so
  modelling every contract as AGENTS.md instructs made the detector penalise each one: ten of
  thirteen gaps in one blind trial, and the other's verdict was *"that coupling **is** the
  product"*. Two earlier rounds of tightening had not fixed it. It was also never in the gap
  taxonomy — `docs/gap-surfacing.md` lists `orphan_node`, `dead_end`, `disconnected_cluster` and
  `single_point_of_failure` — so this restores the spec rather than departing from it.

- **`reviewed_gaps` reports acknowledgements that outlived their detector.** A trial had already
  acknowledged a coupling, and retiring the gap would have made that judgement vanish from the
  reviewed list while the `Decision` sat unreferenced in the graph. Such reviews are now listed
  with `retired` set and no candidate, because a list that shrinks for reasons the user cannot
  see is the dishonesty the open/reviewed split exists to prevent. `ReviewedGap` gains `gap_id`
  and `retired`; `gap` is now optional.

- **Artifact verification gaps read as being about files** (BL-6). `unverified_capability`
  reported Capabilities *and* Artifacts, titling the latter "Nothing verifies reading.py" —
  semantically right, legibly wrong, and independently noted by both blind trials. Artifacts now
  report under `unverified_artifact` with wording of their own. Detection is unchanged: proving a
  capability works still does not prove *this file* is what delivers it.

  The `unverified_capability` key is deliberately untouched. Gap ids hash the source string and
  acknowledgements are stored under the resulting id, so renaming it would have silently expired
  every capability acknowledgement with nothing to tell the user why. A test now pins both keys.

- **HEAL respects a dropped requirement.** DETECT skipped `dropped`/`met` requirements; HEAL's
  orphan scan did not. Marking one dropped therefore silenced half the system and left the other
  half nagging about the same node. Found while making `status` writable — the field was
  unreachable, so the inconsistency had never been reachable either.

- **`describe_schema`** — the design vocabulary is now discoverable instead of guessable. Ask
  with no arguments for every node and edge type, with `node_type` for one type's properties and
  the edges it can carry, or with `from` + `to` for the question an agent actually has: *what may
  connect a Release to a Component?* A blind trial brute-forced fourteen edge types against
  `create_edge` to answer that, then settled on `DEPENDS_ON` "because it was the one that
  validated".

  Matches distinguish an endpoint that **names** a type from one that accepts it through the `*`
  wildcard, and say so in words. Without that distinction the tool would have handed back
  `DEPENDS_ON` and reproduced the original mistake with better ergonomics — validating is not the
  same as meaning what you intended.

- **Rejected writes name the alternatives.** The trial's sharper complaint was that
  `Unknown edge type: PACKAGES` "tells me I'm wrong without telling me what's right" — and a
  discovery tool only helps an agent that already knows to call it. A failed `create_edge` now
  lists the edge types that accept those endpoints, each with its schema hint; a failed
  `create_node` lists the type's properties, or the known node types when the type itself is
  unknown. Still fails loud: the rejection is better, not softer.

- **`tools/reflow2_init.py`** — set up or update reflow2 in a project with one command. Installs
  the design environment only: agent instructions, skills, an MCP config with the binary path
  already resolved, and the graph directory. Creates no `src/`, build file or language choice —
  what kind of project it is comes out of the design, not a scaffold. Re-running updates in
  place, reports what changed, and never touches the design graph, your files, or a customised
  `.mcp.json`.

- **`AGENTS.md` is now the primary instruction file**, per the [agents.md](https://agents.md)
  convention; `CLAUDE.md` is a pointer. The build commands previously lived only in `CLAUDE.md`,
  which non-Claude agents never read.
- `COORD.md` claim board, `.gitattributes` union merge for the shared records, and pull-first in
  every entry point.

### Fixed

- **`single_point_of_failure` is measured against the baseline** (BL-5). The test asked whether ≥2
  non-trivial subsystems remained *after* removing a node, which quietly assumed the design was
  connected to begin with. One unrelated island of two nodes already satisfies that, so **every**
  articulation point elsewhere in the graph reported as a single point of failure while nothing
  about its fragility was different. It now asks whether removal *increases* the count.

  This is the blind trial's *"all 15 defects vanished at once when I added two bookkeeping edges;
  nothing about actual fragility changed"* seen from the other side — those edges attached an
  island. On reflow2's own design: 8 structural defects → 2, and both survivors are correct.

- **A Component the Project contains is no longer reported as floating** (BL-24). `orphan_level`
  checked only for a *Component* parent, but a Project carries no `Component.level` — it sits
  above all of them — so a Project holding a few subsystems raised one false gap per subsystem,
  which is the shape `contains` produces. Containment by the Project now counts as a parent. A
  component nothing contains at all is still an orphan.

- **Every tool returns an object.** MCP defines `structuredContent` as an object, so seven
  list-returning tools — including `detect_gaps` — were malformed and rejected outright by
  spec-compliant clients. Lists now arrive as `{"count": n, "items": [...]}`. Found by a Grok
  trial; three home-grown test layers missed it because each was a client we wrote.

- **The kit's skills reach every agent, not just one** (BL-22). Skills were installed to
  `.grok/skills/` alone — the narrowest-reach of the four harnesses — so a project bootstrapped
  by `reflow2_init.py` and opened in Claude Code had an AGENTS.md naming seven skills the agent
  could not load. They now install to `.claude/skills/` (read by Claude Code, OpenCode **and**
  Copilot/VS Code) as well as `.grok/skills/`.

  This also explains a finding from the Grok trial that had looked like a subtle registration
  problem: opencode searches `.opencode/`, `.claude/` and `.agents/`, and the kit had written
  `.grok/`. The directory was never on the search path.

- **MCP config for every agent, merged rather than overwritten.** `reflow2_init.py` now writes
  `.mcp.json`, `opencode.json` and `.vscode/mcp.json` from one generator, since only Grok reads
  another tool's format. All three are merged into: `opencode.json` is that tool's *entire*
  config, and any project may already run other MCP servers — both must survive.

  Merging fixes a silent failure in the process. The installer previously bailed out whenever
  `.mcp.json` existed without a `reflow2` entry, so **any project already using one MCP server
  never got reflow2 installed at all** — while the run still reported success.

## [0.1.0] — 2026-07-18

The first release the design loop runs end to end on: a real project was designed and built
through it by an agent that had never seen the source, and by a second user on macOS via grok
build.

### Added

- **Interface layer** — `Interface` nodes with `PROVIDES`/`CONSUMES`, typed constructors, LLM
  extraction, MCP tools, and detection of contracts with a missing side
  (`unprovided_interface` / `unconsumed_interface`). Closes the failure the original Reflow never
  solved: a change made on one side of a service boundary leaving the other side stale.
  Pairing is keyed on node identity, so a shared name cannot mask a break.
- **Circular-dependency detection** — over a *directed* dependency view (`DEPENDS_ON` plus
  contracts collapsed through their `Interface`), reported per strongly-connected cluster rather
  than per elementary cycle. Critical, and propose-only: which edge to invert is a design
  decision.
- **As-built drift** (SP-6b) — an `Artifact.checksum` baseline and `reconcile_artifacts`, which
  compares caller-supplied observations and reports `missing_artifact` / `checksum_change` /
  `undocumented_addition` / `no_baseline`. Because `REALIZES` reads as Upstream, drift walks
  *back up* the golden thread to the Capability and Requirement behind the code. The core
  performs no I/O by design.
- **Write side for the types DETECT asks about** (WS-1..3) — `Verification` (+ `VERIFIES`,
  status), `Release`/`Environment`/`Resource` (+ `DEPLOYED_TO`, `REQUIRES_RESOURCE`), and
  `Decision` (+ `GOVERNED_BY`). Previously the system raised gaps demanding exactly these types
  and offered no typed way to answer them.
- **Gap review** — `acknowledge_gap` moves a judged gap into `reviewed_gaps` with the reason,
  stored as a real `Decision` so it outlives the session; `withdraw_gap_acknowledgement` puts it
  back. Reviews expire on their own when the situation changes, because a gap's id hashes its
  affected nodes.
- **`tools/reflow2_cli.py`** — one-shot command-line access to a graph, for shells, scripts and
  agents without an MCP connection.
- **`tools/smoke_mcp.py`** — end-to-end test of the shipped binary over stdio: the whole loop,
  plus persistence and cross-process determinism.
- **`docs/reflow-audit.md`** — every workflow and tool of the original Reflow, with an
  adopt / obsoleted / do-not-port verdict.
- **`where-am-i` skill** — read the graph back to the user in their own words. Added because a
  real user could not tell what the system had concluded.
- **`check-health` skill** — the HEAL step had MCP tools and no skill to invoke them, so eight
  defect categories were unreachable in practice.

### Fixed

- **Gap detection was not reproducible across processes.** `build_network` iterated a `HashSet`,
  whose hasher Rust seeds per process, so node insertion order — and with it Leiden's tie-breaks
  and every gap derived from community structure — differed between runs. Five runs on one
  unchanged graph gave 11, 12, 13 and 11 gaps. This silently undermined gap review: an accepted
  gap could return under a different id.
- **`unexpected_coupling` fired on every correctly-modelled contract.** An `Interface` joins its
  provider to its consumers and little else, so Leiden gave it a community of its own and each
  `PROVIDES` edge read as a "sole bridge" — the modelling discipline penalising itself.
  Contracts are now collapsed to the components they couple.
- **Community fragments were treated as parts of the design.** Both endpoints of a bridge must
  now sit in a community of ≥3 — the same non-trivial test `single_point_of_failure` already
  used.
- **`Fragment` and `DriftEvent` sat inside the topology** they were never part of, shifting
  communities and, for `DriftEvent`, eligible to be reported as a coupling in its own right.
- **`link_artifact` guidance was misleading** — it told the agent to confirm the
  `unrealized_capability` gap had closed, when the first `link_artifact` *switches that detector
  on* for every other capability, so the total rises. Correct behaviour, wrong instruction.

### Changed

- `detect_gaps` now returns **open** gaps only; reviewed ones move to `reviewed_gaps`. The open
  list is meant to mean *still needs attention* — a list that can never reach zero gets skimmed.
- The MCP surface grew from 34 to 52 tools.
- `getting-started/SETUP.md` gained a kickoff line and a stop/resume section, and states the
  one-agent-at-a-time constraint with the exact error text.

### Known limits

Recorded honestly rather than omitted; see [docs/backlog.md](docs/backlog.md) for the full list.

- **No schema discovery.** An agent needing an edge type has to guess; the blind trial
  brute-forced fourteen before settling on one *because it validated*.
- **`ingest` is not reachable over MCP** (SP-3b), so the multi-pass extraction pipeline — and
  with it provenance, fuzzy dedup and time-aware resolution — does not run in agent-native use.
- **`gap_to_prompt` output is not persisted**, so a question asked in one session is re-derived
  and re-asked in the next.
- **Component hierarchy cannot be built from the surface** — `contain_component` exists in core
  and is not an MCP tool.
- **`single_point_of_failure` responds to graph shape more than to risk** — the blind trial saw
  15 defects fall to 0 after adding two bookkeeping edges.
- Multi-project graph selection, concurrent multi-agent access, `EnvironmentRule`/`QualityGate`,
  and generative HEAL content all remain deliberate deferrals.

## [0.0.1] — before 2026-07-18

Initial core: the schema (26 node types / 52 edge types), `DesignGraph`, the coherence loop
(CHANGE / PROPAGATE / DETECT / HEAL), the temporal axis, INGEST, GENESIS, artifact linking, the
graph-analysis modules, and the `reflow2-mcp` server. See
[docs/requirements-coverage.md](docs/requirements-coverage.md) for what that covered.
