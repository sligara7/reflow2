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
