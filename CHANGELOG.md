# Changelog

Notable changes to Reflow 2.0. Format follows [Keep a Changelog](https://keepachangelog.com);
versions follow [semver](https://semver.org) — pre-1.0, so the minor number moves when the
graph model or the tool surface changes shape.

Two companion records, deliberately kept separate:

- **[docs/requirements-coverage.md](docs/requirements-coverage.md)** — *are we meeting the docs?*
  Every requirement → module → test, with an honest Met/Partial/Deferred status.
- **[docs/backlog.md](docs/backlog.md)** — *what should we do next?* Open work with its evidence
  and rough size.

This file is the third view: *what changed, and when*.

## [Unreleased]

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
