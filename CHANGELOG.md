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
