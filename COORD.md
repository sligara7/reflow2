# COORD.md — who's working on what

The claim board for reflow2. Two people, two agents (Claude Code and grok build), one repo,
coordinating through git.

Its only job is **avoiding collisions**: knowing what someone else already has in hand, so two
people don't build the same thing or edit the same module at once. It is not a plan, a spec, or
a status report —

| Question | Where |
|---|---|
| Who has what right now | **this file** |
| What the work *is*, and why | [docs/backlog.md](docs/backlog.md) |
| Is the code meeting the docs | [docs/requirements-coverage.md](docs/requirements-coverage.md) |
| What changed and when | [CHANGELOG.md](CHANGELOG.md) |

## For agents reading this

**Before anything else: `git pull --rebase`.** A claim board you haven't pulled is a claim board
from the past — you'll claim something that's already taken and find out at merge time. Pull,
then read.

**Before starting work:** read the In-progress list. If someone holds the item, or holds
something that touches the same files, pick something else or say so — don't quietly work in
parallel.

**When you start:** add one line under **In progress** with the handle, the date, and roughly
what you're touching. Commit that line *before* the work, not after — a claim nobody can see is
not a claim.

**When you finish:** move the line to **Recently finished** with the commit, and push.

**Keep it one line per item.** Two agents editing this file will eventually collide in git; a
line-per-item makes the conflict trivial to resolve, a table or a paragraph does not.

**Don't restate the backlog here.** Reference the id (`BL-4`) and let
[docs/backlog.md](docs/backlog.md) carry the description, so there's one source of truth for what
the work actually is.

## Handles

| Handle | Who | Agent | Usually working on |
|---|---|---|---|
| `@ajs` | Anthony | Claude Code | core, schema, detectors |
| `@bro` | (brother) | grok build | using it for real; consumer kit feedback |

Add yourself if you're new here.

## In progress

*Format: `- BL-n or short title — @handle — since YYYY-MM-DD — files/areas touched`*

- Brownfield trial on ophyd-service — @ajs — since 2026-07-18 — docs/trials-private/2026-07-18-brownfield-ophyd-service.md (private) (findings log; no code yet)
- Greenfield trial on aidrone — @ajs — since 2026-07-18 — docs/trials-private/2026-07-18-greenfield-aidrone.md (private) (running findings log; design lives in ~/projects/aidrone)






## Blocked / waiting

- **~~Release follow-up (v0.5.0 re-dispatch)~~ SUPERSEDED 2026-07-21** — v0.5.0's release run
  (29785834848) sat stuck in GitHub's macos-x86_64 queue for 11h and never published any
  binaries, so v0.5.0 never actually reached a user. Rather than re-dispatch a stale, buggy
  build (main was 27 commits ahead with the deep-review criticals — HEAL node-deletion,
  installer clobber, install.sh silent death), **v0.6.0 was cut from current main** instead
  (user-approved). The stuck v0.5.0 run was cancelled. BL-15 verification rides on the v0.6.0
  release run under the new draft-then-publish release.yml.

- **Standalone-repo conversation** — was gated on the v0.5.0 Release node (done 2026-07-20);
  counterpart Decision with reopening conditions, CRUD skills, and search are all in place
  for it. Wants the user.

## Recently finished

Trimmed periodically; the durable history is [CHANGELOG.md](CHANGELOG.md) and `git log`.

- BL-66 done: consumer CI coherence gate. tools/reflow2_check.py (stdlib, self-contained, in the kit tarball via release.yml) reads the COMMITTED export — CI can't open gitignored .reflow2/, and the export is what the team reviewed — rehashes registered artifacts (sha256 truncated to each registration's own length), reconciles, runs detect_gaps. Exit 1 on unaccepted checksum_change/missing or open anchored gap ≥ 0.8 (--gap-threshold); acknowledge_gap = the sanctioned green, no drift-skip flag exists. Exit 2 loud on can't-run. Three-way verified on reflow2 itself (pass / doctored-file fail / missing-export refuse). ci-gate skill (CI snippet + red-to-green playbook naming the two launderings) + SETUP.md pointer; skill_lint NON_TOOL_TERMS += "path". Graph: art:check REALIZES cap:reconcile-built, accepted two-sided (chg:bl66-ci-gate). Export 247n/486e — @ajs — 2026-07-21 — (this commit)

- BL-63 done: snapshots capture design edges (new optional Snapshot.edges beside state; sorted; bookkeeping neighbours excluded so a snapshot never accumulates its own history) — a lazy reallocation now leaves "A once owned Z" recoverable from the snapshot alone, no hand-authored Decision needed. parse_snapshot_edges/SnapshotEdge in core; pre-BL-63 snapshots read as empty capture, not error. Deviation from the entry's lean, reasoned in backlog: full-capture-with-exclusions, not changed-edges scope. revise-design's "leave a formerly-true edge" workaround replaced with record-first-then-delete; both CRUD skills updated (3 mirrors, kit refreshed to 0.6.1). Schema +1 optional prop → next cut is 0.7.0. Gates: workspace green, clippy/fmt/schema/skill-lint/test_init clean, all instruments at baselines incl. coherent 9/9 (its snapshot reader unaffected — state shape unchanged). Drift accepted two-sided (chg:bl63-snapshot-edges). NOTE: a session started before this build snapshots without edges — restart before relying on it live — @ajs — 2026-07-21 — (this commit)

- BL-69 done: SPOF connectivity moved to the as-built operational network (ops types + realizing artifacts) — the design network's intent edges donated mass (cmp:flow fired on stranding its own capability/artifact/verification cluster) and phantom connectivity (cmp:export, a true cut vertex, hid behind a SATISFIES chain). Candidates enumerate from the same network; all prior selectivity lessons kept (baseline-relative, non-trivial, operational-candidates, library filter). Self-graph measured before→after: {flow, service} → {graph, export, ifc:graph-export, service} — the false positive out, three true findings in (service already accepted via dec:service-spof-accepted; other three await disposition). 2 regression tests + island fixture rebuilt operational; workspace + all instruments at baselines; drift accepted two-sided (chg:bl69-spof-cut-vertices; plus art:artifact's missed BL-26 accept recorded late via chg:bl26-documents-write-side). Export 242n/480e. NOTE: a session started before this build serves the old detector — restart before relying on it live — @ajs — 2026-07-21 — (this commit)

- Design-analysis capture BL-64..68 (concept items, no code): from a session comparing reflow2 against UAF's full acquisition lifecycle and modern commercial practice. BL-64 disposal/retirement (missing P6), BL-65 risk/security as a lifecycle-spanning concern + DevSecOps continuous governance, BL-66 design-coherence as a consumer CI gate (actionable S-M), BL-67 SLO/telemetry reconciliation (as-operating fidelity), BL-68 readiness-driven roadmapping (KEYSTONE — TRL/MRL/-ilities gate achievability; derive the delivery timeline from readiness × the thread; "the roadmap is a risk-burndown schedule"; unifies 64/65/67). All reuse propagate + detect-and-ask + the reconcile seam; the new part in each is vocabulary, a user design decision. Nothing built — @ajs — 2026-07-21 — (this commit)

- BL-58 DONE (all 12 items, for 0.6.1) + versioning policy written into CHANGELOG. Fixes: ingest upsert (BL-46 on ingest path), snapshot BTreeMap determinism, propagate_change existence check, apply_heal one atomic batch (merge_nodes made batch-free — it was the only caller), swallowed edge errors surfaced (detect/ingest/fielded), budget non-finite rejection + provable-overrun + total_cmp, i64::MAX widen guard, truncated_beyond_depth honest doc, drift dangling-edge skip, per-edge gap ids for missing-intermediate, ingest fragment_id reuse refused, node_type_index sorted. 10 registered artifacts drifted → accepted two-sided (chg:bl58-silent-failure-batch); export 229n/466e, gaps 0. Gates: 39 workspace suites, clippy clean, smoke + phase_trial + model_the_loop + coherent_erosion + build_design_graph all green. Versioning policy: patch=fixes/silent-failure-loud, minor=surface/schema shape change; this batch is all patch → next cut is 0.6.1. Remaining review: BL-59 (perf), BL-56b (harness leaks) — @ajs — 2026-07-21 — (this commit)

- BL-57 done (all 7): dyno_err variant-aware (caller mistakes → invalid_params, ~60 tools, one choke point, Schema stays internal since it's open-time); deny_unknown_fields on all 65 request structs + smoke additionalProperties:false guard — IMMEDIATELY caught a real latent bug (smoke passed `at` to reconcile_artifacts, silently ignored; field is detected_at); export_graph overwrite guard + resolved-path + invalid_params for unwritable; serve path gets explain_open_failure; get_node one named {node} shape both ways (rippled 17 consumers, 0 skills — strengthened previously-always-true smoke checks); withdraw_gap_ack `was_reviewed`→`withdrawn`; answer_question keeps erroring (doctrine: no silent drop) + documented. 4 new tests (taxonomy code-assert, overwrite guard, +the BL-62 ones adjusted). Gates all green; drift accepted (chg:bl57-tool-boundary-honesty); export 218n/445e. Remaining review: BL-56b, BL-58, BL-59 — @ajs — 2026-07-21 — (this commit)

- BL-61 + BL-62 done: skill_lint's `_`-filter dropped so single-word tool names (allocate/satisfies/genesis/…) are checked — 11 tools were exempt; allowlist +58 single-word non-tool terms, unused-guard keeps it exact, negative-tested (renamed single-word tool → exit 1). BL-62: all 14 uncovered tools now tested — 2 tools.rs tests (temporal/resource/realization/analysis/delete walk + ask→withdraw) → tools.rs 31→33, and smoke §9c drives create_node/scan_nodes/search_design/delete_node/get_node over REAL stdio (the blind spot smoke exists for). Pure tests+lint, no registered-artifact drift. Gates: workspace 39 green, fmt/skill_lint clean, smoke ALL PASS. Remaining review items: BL-56b, BL-57, BL-58, BL-59 — @ajs — 2026-07-21 — (this commit)

- BL-60 docs truth pass DONE: the primary instruction files no longer describe the pre-surface era. AGENTS.md "Current state" rewritten to v0.5.0 (surface shipped + decided, full 30-module list, GENESIS/INGEST built, two crates, v0.10.0 pin, 54 edges, INCLUDES traceable); README (27 types + Question, real layout tree, path fix); requirements-coverage (IS-5/6/7 → ✅, preamble/deferral/numerals); surface-plan + interaction-surfaces superseded banners; overview routing + private-repo delinking; SETUP public-repo + commit-an-export; getting-started/README all 11 skills; 3 skill contradictions fixed (link-artifacts full:true, detect-and-ask→retire, check-health apply gate — verified against heal.rs:849 "requires_human_review is not consulted"). skill_lint allowlist += blocked_by_mode (the lint caught my own new field ref). Pure docs — no graph writes, no artifact drift. Gates: skill_lint/test_init/validate_schema green — @ajs — 2026-07-21 — (this commit)

- Deep review (4 parallel reviewers, criticals verified in source) → BL-53..62 raised; tier-1 FIXED same day: BL-53 HEAL self-loop merge-deletes-node (guard in merge_op_for, covers propose+apply), BL-54 installer hash-manifest ownership (edits LEFT ALONE, obsolete files pruned when untouched, non-dict config no longer crashes, --check agrees; 12 new test_init cases → 21), BL-55 install.sh silent death + false success fixed and release.yml drafts-then-publishes with asset assertion, BL-56a smoke --graph-path needs --wipe. Open tiers: BL-56b harness leaks, BL-57 boundary honesty, BL-58 core silent-failure batch, BL-59 adopt-scale perf, BL-60 docs truth pass (AGENTS.md "Current state" is fiction — critical for new readers), BL-61 skill-lint single-word blind spot, BL-62 coverage gaps. Gates all green; drift accepted (chg:deep-review-tier1); export 215n/440e — @ajs — 2026-07-21 — (this commit)

- BL-52 done — first CI + skill lint: ci.yml (core job: fast gates incl. clippy -D warnings both crates, schema, test_init, skill lint; full job: workspace + smoke_mcp + phase_trial + model_the_loop + coherent_erosion against the real binary, rust-cache'd). tools/skill_lint.py checks the skills' CONTRACT (tool refs resolve vs the #[tool] set with a both-ways allowlist, mirrors byte-identical, frontmatter, BL-41 rule) — deliberately no LLM evals, trials stay the semantic evidence. All green locally + negative-tested; ver:skill-lint added (passing) VERIFIES cmp:skills; export 212n/435e. Watch the first live run — @ajs — 2026-07-20 — (this commit)

- v0.5.0 Release node done: rel:v050 with all 34 artifacts frozen at tag v0.5.0 + cmp:skills, deployed active on env:dev; release_report answers released==designed with 26/26 capabilities covered and an empty not-covered diff; rel:v040 retired, its deployment withdrawn (rolled_back — the sanctioned "declaration withdrawn" vocabulary per reconcile_deployment's own correction path). ROOT-CAUSE FIX: INCLUDES joined the traceability table (nodes.rs) — the {env:dev, rel:v040} island existed because every release was outside the design network by construction; pinned by structural + propagate tests. build_design_graph.py now freezes release checksums from git tags, never the working tree (v0.4.0 manifest verified 33/33 against its tag first). Axis-Z: epoch:v050-cut + epoch:v050-hardening added, five floating ChangeEvents pinned (render_views confession cleared). ALSO from user mid-session: req:frictionless-update captured (Claude-Code-style one-liner install / one-word update / frequent minor cadence) → BL-51. Export 211n/434e, gaps 0 — @ajs — 2026-07-20 — (this commit)

- BL-50 done (all three): int literals widen to schema floats at the core write seam (range check kept, foundation untouched); add_change_event takes `affected` and draws CHANGED atomically — refuse-first, write-whole; describe_schema counts half-exact matches so CHANGED reads as the modelled fit; delete_node/delete_edge return {deleted} (bare-bool envelope, caught by BL-48's choke-point wrap); SessionStart hook recipe documented in the kit's step 0a. Workspace 39 suites green, smoke ALL PASS incl. 9 new checks; drift on art:graph/art:vocabulary/art:service accepted two-sided (chg:bl50-tool-boundary), export 205n/386e, gaps 0. Same restart caveat: a session predating this build serves the old surface — @ajs — 2026-07-20 — (this commit)

- BL-48 + BL-49 done: graph_report_markdown returns prose as text (ok_json wraps any scalar — the string twin of the array envelope bug), propagate defaults to a core-computed summary (full dump behind full=true), export_graph writes a deterministic file on request; smoke_mcp asserts the result envelope on every call + drives all three over real stdio; impact-check skill teaches summary-first; drift on art:propagate/art:service accepted two-sided (chg:bl48-bl49-tool-surface), export 201n/380e, gaps 0. Side effect: reflow2_init.py rerun for the mirror refresh did the full self-host conversion — REFLOW2.md + pointer lines committed, per-machine MCP configs (opencode.json, .vscode/mcp.json) gitignored. NOTE: a session started before this build serves the old shapes — restart before relying on summary/path live — @ajs — 2026-07-20 — (this commit)

- Proposed-requirement sweep + capability verification done (user-directed): all 6 proposed requirements evidence-checked against the repo and accepted (deterministic-core, invocation, persistence, driving-agent, human-decides, as-built-honest — statements match reality verbatim, provenance now explicit `authored`); cap:dimensions + cap:ingest realized→verified on live runs of their suites (6/6, 16/16), VERIFIES edges already in place; gaps stay 0; export refreshed (198n/376e). Requirement statuses now: 17 accepted, 0 proposed — @ajs — 2026-07-20 — (this commit)

- BL-47 + BL-46 done: merge survivor ranks unset provenance below explicit authored (the near-deletion of cap:kit can't recur), colliding edges keep the survivor's properties, create_node on an existing id merges per the revise-design contract (`upsert_node`); workspace green, smoke_mcp green, drift accepted two-sided (chg:merge-integrity-bl47-bl46). NOTE: this session's running server predates the rebuild — restart before relying on the new semantics live — @ajs — 2026-07-20 — (this commit)

- **Stub-survivor reconciliation done — first live self-adopt session, 0 gaps**: where-am-i →
  detect-and-ask with the user (all 4 decisions theirs: 3 merges, cap:store wired, req:platform
  satisfied by cap:kit, storyflow trial recorded as cap:adopt's proof), HEAL merges under the
  survivor rule, gaps 11→0, defects 6→4, dec:merge-survivor-provenance landed, export refreshed
  (197n/370e, stamp 0.5.0). 5 findings about reflow2 itself → BL-46..BL-50;
  trial: docs/trials/2026-07-20-self-adopt-live.md — @ajs — 2026-07-20 — (this commit)

- **HISTORY REWRITTEN 2026-07-20 (@bro: action needed)** — repo went public (Apache-2.0); five real-system trial records (storyflow x2, ophyd-service, aidrone, 3dtictactoe) were scrubbed from ALL history before anyone cloned. Every SHA changed; tags moved. Update your clone with: `git fetch origin && git checkout main && git reset --hard origin/main` (stash local work first), or re-clone. Private trial records now live in gitignored docs/trials-private/ (ask @ajs for copies); public trials (self-host, erosion, phase-coverage, weather-station) stay — @ajs — 2026-07-20 — (this commit)

- v0.5.0 cut (user approved): surface changed shape (`documents`), CHANGELOG section moved out of Unreleased, no schema change so no upgrade doc; the tag push is the first live run of release.yml — binaries + kit for the no-checkout path. NOTE for next session: served_by will say 0.5.0 now, not 0.4.0 — @ajs — 2026-07-20 — (this commit)

- BL-15 published binaries built: release.yml (3 platforms + kit tarball + checksums, version-less asset names), install.sh (gh-first for the private repo, checksum-verified, binary+kit replaced together), reflow2_init.py checkout-independent (--binary/PATH, KIT_VERSION.json, installer update advice); SETUP.md leads with the no-build path. Verified from a simulated tarball end to end; the real release run against v0.4.0 is the remaining verification. Embed-kit-in-binary stays as the S-M follow-up — @ajs — 2026-07-20 — (this commit)

- BL-26 S half done: `documents` core fn + MCP tool (78th), endpoints fail-loud (storage accepts dangling edges — the check is the only one there is), doc_kind carried; link-artifacts skill states the record-this-file criterion, mirrors refreshed. The traversal decision (M half) stays open and wants the user — @ajs — 2026-07-20 — (this commit)

- BL-29 survivor rule done, closing the item: user decided option 2 — provenance wins (authored > planned > imported > reconciled > inferred > healed), id breaks ties; pinned in three directions, pre-provenance graphs unchanged. Decision node for the design graph queued for the first live-server session — @ajs — 2026-07-20 — (this commit)

- BL-29 chained-merge hazard reproduced and fixed: sanctioned merges sharing a node corrupted the graph silently with `verified=true` (dangling edges accepted by storage); propose now defers chain links, apply refuses shared-node proposals pre-write, third-party DUPLICATES claims survive a merge; pair-edge drops reported. BL-29 done to a decision (survivor rule). Also: fmt fixes to search.rs/service.rs that slipped past the previous session's gate — @ajs — 2026-07-20 — (this commit)

- Restart batch test live half done: real design imported over the stub (CLI path, 180 nodes/332 edges), skew check PASS via raw-stdio probe (served_by 0.4.0, mtime match, delete_edge+search_design present), deltas verified stub-survivor-shaped, **v0.4.0 tagged and pushed** — @ajs — 2026-07-20 — (this commit)

- Full-text search done: `fulltext` feature enabled (schema carried the flags all along — recurring-lesson #17, this time even the annotations were unreachable capability), search.rs core op + search_design MCP tool + reindex-at-open, search-before-add in capture-intent and find-the-node in revise/retire skills; gates green incl. smoke_mcp against the fresh binary. NOTE: the feature flip re-fingerprints librocksdb-sys — budget ~14 min for the one-time rebuild — @ajs — 2026-07-20 — (this commit)
- CRUD skill closure done: revise-design + retire-from-design skills (update/delete were the kit's missing verbs), delete_edge MCP tool (+ tools.rs test — a wrong edge no longer costs an endpoint node), stale skill mirrors refreshed, create_node merge semantics finally written down — @ajs — 2026-07-20 — (this commit)
- Cycle break + true-gap closure done: propagate↔structure cycle broken (shared vocabulary moved to nodes.rs/graph.rs, verified gone by the probe that found it AND by the rebuilt self-model); confirm.rs and reflow2_init.py each got their first test suite; self-model now 175 nodes, 1 gap (cap:adopt, deliberate), 4 warnings, 0 critical — @ajs — 2026-07-20 — (this commit)
- Self-model standing probe done: build_design_graph.py derives DEPENDS_ON from source and reconciles vs disk; graph 125→173 nodes, gaps 16→3 (all true), and reflow2 now reports its own propagate↔structure cycle as critical — @ajs — 2026-07-20 — (this commit)
- F7 done: flow cycles report members + path (storyflow's cluster kept p-prompt, the human hand-off, that the walk dropped); adopt trial's F1-F7 all closed — @ajs — 2026-07-20 — (this commit)
- F6 done: SPOF skips components coupled only by a library contract (storyflow 7 of 15 -> 5); medium default keeps today's behaviour — @ajs — 2026-07-20 — (this commit)
- BL-27 F1/F2 done: pointer reaches every instruction-file convention (CLAUDE.md et al); next-steps branches on the project, not our own artifact — @ajs — 2026-07-20 — (this commit)
- BL-42 + BL-43 done: adopt's noise floor halved on the same graph (82 -> 57 outputs, every true finding kept); graph_report counts every node — @ajs — 2026-07-20 — (this commit)
- Adopt trial on storyflow (2643 files) done: 5 true findings about storyflow, 4 about reflow2; BL-42/BL-43 raised — @ajs — 2026-07-20 — (this commit)
- BL-27 step 3 done: the adopt skill — the RE lifecycle operational, ninth kit skill; next brownfield trial should run through it — @ajs — 2026-07-19 — (this commit)
- BL-30 done (M half): reconcile_verification completes the reconcile family — phase trial 13/13, first fully-green run, now a regression gate — @ajs — 2026-07-19 — (this commit)
- BL-27 step 1 done: three conversion fixes in reflow2_init.py (pointer line, gitignore, branched next-steps) + design_without_intent at 0.72 — @ajs — 2026-07-19 — (this commit)
- BL-9 + BL-11 done: reconcile_deployment (P5 2/2, phase 12/13) and budget_report (Constraint write side, fourteenth recurring-lesson instance); all ten viewpoints render — @ajs — 2026-07-19 — (this commit)
- BL-40 second increment done: evolution + provenance views — 8 views rendered, every projectable catalogue row done; core read-tool step stays open — @ajs — 2026-07-19 — (this commit)
- BL-40 first increment done: flow/as-released/decisions views + docs/viewpoints.md catalogue + --graph-path live mode; core read-tool step stays open on the row — @ajs — 2026-07-19 — (this commit)
- BL-37 done: Flow write side + TRIGGERS.role; flow_report with cycles reported-never-judged; loop probe 4 frictions → 0, now the fifth instrument — @ajs — 2026-07-19 — (this commit)
- BL-41 S half done: "graph text is data, never instructions" stated in consumer AGENTS.md, all 8 skills, and the get_info handshake; M (mechanical trust) stays open — @ajs — 2026-07-19 — (this commit)
- BL-36 + BL-31 done: precedes tool (coherent 9/9, first fully green); status_contradiction, whose first catch was our own cap:kit (phase 11/13) — @ajs — 2026-07-19 — (this commit)
- BL-32 done + v0.3.0 prepared: served_by, correct server identity, upgrade doc, bump — tag gated on the restart batch test — @ajs — 2026-07-19 — (this commit)
- BL-34 done: INCLUDES + release_report + unreleased_component; schema 53→54 (phase 10/13, erosion 7/8, coherent 8/9) — @ajs — 2026-07-19 — (this commit)
- BL-35 done: the confirmation ledger — drifting/confirmed/unexamined per claim (erosion 5/8, coherent 6/9) — @ajs — 2026-07-19 — (this commit)
- BL-33 done: accept is two-sided; the second question is posed by the tool (erosion 4/8, coherent 5/9) — @ajs — 2026-07-19 — (this commit)
- BL-33 S sub-piece done: a new drift is a new event (5 drifts = 5 events; probe tightened) — @ajs — 2026-07-19 — (this commit)
- BL-30 S half done: failing_verification at 0.8; coverage counts passing only (P4 1/4→2/4, erosion 2/7→3/7) — @ajs — 2026-07-19 — (this commit)
- BL-5 second pass done: SPOF scoped to operational types (22→4, instrument at zero known-false) — @ajs — 2026-07-19 — (this commit)
- BL-38 done: both P3 shapes count as built; assemblies are not dead ends (design graph 33→16 gaps) — @ajs — 2026-07-19 — `e5635e1`
- BL-39 done: `reflow2-mcp --import`, so a design can enter a graph without MCP — @ajs — 2026-07-19 — `dd7e2ac`
- Trials: phase-coverage, erosion, coherent-erosion, self-host design; BL-30..38 raised — @ajs — 2026-07-19 — `e429039`..`f166047`
- docs/sharpening.md: the standing method for improving reflow2 — @ajs — 2026-07-19 — `5617503`
- BL-29: apply_heal checks the proposal; merge reports what it drops — @ajs — 2026-07-19 — `be6e18d`
- BL-27: possible_duplicate — all 5 adopt blockers now done; BL-29 raised — @ajs — 2026-07-19 — `3410117`
- BL-27: unmotivated_capability, the direction DETECT was blind in — @ajs — 2026-07-18 — `d470860`
- BL-27: 3 of 5 adopt blockers (capability status, provenance, gap ranking) — @ajs — 2026-07-18 — `83da659`, `3c157ab`
- BL-28 typed tool params (fixes the ask half of DETECT from Claude Code) — @ajs — 2026-07-18 — `6dfdd61`
- Self-host GENESIS trial; BL-28 raised, BL-27 widened — @ajs — 2026-07-18 — `2ea0db1` + revision
- Brownfield trial on 3dtictactoe; BL-27 raised from it and ophyd — @ajs — 2026-07-18 — `84aacb6`
- Self-host: consumer kit installed into reflow2 itself — @ajs — 2026-07-18 — `98baf40`
- **v0.2.0 released and tagged** — @ajs — 2026-07-18 — `b69dc28`; frozen for real-use feedback
- BL-20 export/import; BL-19 backup+backfill; BL-18 staleness check — @ajs — 2026-07-18 — `79f5d42`, `3a618e3`, `21e9e11`
- BL-19 (stamp+check half): a graph records which reflow2 wrote it — @ajs — 2026-07-18 — `2764eec`
- BL-21: report-friction skill; installer validates skill frontmatter — @ajs — 2026-07-18 — `31064f6`
- BL-25: an answered question stays visible while its gap is open — @ajs — 2026-07-18 — `20b7943`
- BL-4: asked questions outlive the session (new Question node type) — @ajs — 2026-07-18 — `be510b4`
- BL-5: SPOF measured against the baseline (self-host: 8 defects -> 2) — @ajs — 2026-07-18 — `44e3e23`
- BL-24: Project containment anchors a subsystem (self-host: 3 gaps -> 1) — @ajs — 2026-07-18 — `0bf157d`
- BL-23: per-file coverage counted not asked (self-host probe: 25 gaps -> 3) — @ajs — 2026-07-18 — `7c5b702`
- BL-6b: coupling demoted to a signal; retired acks stay visible — @ajs — 2026-07-18 — `f340fcb`
- BL-22: kit reaches every agent; MCP configs merged not overwritten — @ajs — 2026-07-18 — `9e9e765`
- BL-2/BL-3/BL-6: assembly hierarchy, requirement status, artifact gap split — @ajs — 2026-07-18 — `9ab3da3`
- BL-1 schema discovery + evidence-backed rejections; per-crate AGENTS.md — @ajs — 2026-07-18 — `9440929`
- Answer the first external user (where-am-i skill, pause/resume, setup kickoff) — @ajs — 2026-07-18 — `ed818ae`
- Records: CHANGELOG, backlog, trials — @ajs — 2026-07-18 — `ed818ae`
- Gap review + cross-process determinism fix — @ajs — 2026-07-18 — `8a66afb`, `565c418`
- Selective `unexpected_coupling`; provenance out of topology — @ajs — 2026-07-18 — `824a6cc`
- Write side for the types DETECT asks about (WS-1..3) — @ajs — 2026-07-18 — `e722766`
- Reflow audit: all workflows and tools, with verdicts — @ajs — 2026-07-18 — `7179218`
- Interface layer, cycle detection, as-built drift — @ajs — 2026-07-18 — `7f168a5`

## Stale claims

If an item has been claimed for **more than a week with no commits against it**, anyone may take
it — leave a note on the line saying you did, rather than deleting the original claim.

## When git conflicts

It will happen — two people, one repo, and the shared records are the files you both touch most.

**The rule that matters: never resolve a conflict by discarding the other side.** For an agent,
"resolve the conflict" usually means picking a side — and here that silently deletes a
teammate's claim, changelog entry or finding. Both sides are almost always correct; the job is
to keep both, not to choose.

| File | What to do |
|---|---|
| `COORD.md`, `CHANGELOG.md` | Resolved automatically — `.gitattributes` marks them `merge=union`, so both sides' lines are kept. If you *still* see a conflict, you both edited the same line: keep both meanings and tidy the wording. |
| `docs/backlog.md`, `docs/requirements-coverage.md` | Usually you each touched different rows — keep both. If it's the **same** row, someone's status is newer than yours; check `git log -p` on that file and ask rather than overwriting. |
| `docs/trials/*` | Append-only evidence. Keep both; never edit someone else's trial record. |
| Source and tests | A real conflict here means the claims didn't work — two people edited the same module. Reconcile the *intent*, not just the text, and re-run the full gates before pushing. |
| `Cargo.lock` | Regenerate rather than hand-merge: take either side, then `cargo build` and commit the result. |

**If you cannot resolve it confidently, stop and say so.** Pushing a guess is how one person's
afternoon gets quietly deleted. An unresolved conflict sitting in the working tree is a
recoverable state; a bad merge on `main` is not.

## Conventions

- **Branches:** `feat/<short-name>` off `main`, one per claimed item where practical.
- **A change is done** when `cargo test --no-default-features`, `cargo clippy
  --no-default-features --all-targets` and `cargo fmt --check` are clean, and
  `python3 tools/validate_schema.py` prints OK after any schema edit — see
  [AGENTS.md](AGENTS.md).
- **Update the records in the same change**, not afterwards: coverage matrix when a status moves,
  CHANGELOG when a user would notice, backlog when an item is finished or discovered.
- **Findings from real use** (a trial, a session that went wrong) go in
  [docs/trials/](docs/trials/) verbatim, and get an item in the backlog if they need work.
