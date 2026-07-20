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

- Brownfield trial on ophyd-service — @ajs — since 2026-07-18 — docs/trials/2026-07-18-brownfield-ophyd-service.md (findings log; no code yet)
- Greenfield trial on aidrone — @ajs — since 2026-07-18 — docs/trials/2026-07-18-greenfield-aidrone.md (running findings log; design lives in ~/projects/aidrone)






## Blocked / waiting

- **On-restart batch test** — blocked on restarting the editor session (BL-32: the live MCP server
  predates every fix this session shipped, and holds the `.reflow2/graph` lock; the operator is
  remote and cannot restart). Everything below is *already verified* by fresh-binary gates — this is
  about exercising the **live surface**, first session that can:
  1. `./target/debug/reflow2-mcp --graph-path .reflow2/graph --import docs/design/reflow2.json` —
     load the real 96-node design over the genesis stub (BL-39's path, first real use).
  2. Confirm the session's tool list carries this session's additions: `set_capability_status`,
     `set_provenance`, and the BL-38/BL-27 detector behaviour.
  3. Run **where-am-i** against the real design — it should narrate reflow2's actual state, not an
     18-node stub — including "what's settled": the design graph now carries 8 Decision nodes
     distilled from the 2026-07-19 session (each rationale links the session transcript).
  4. Run **check-health** and **detect-and-ask** — live counts should match
     `build_design_graph.py --analyse-only` (16 gaps, 34 defects at time of writing; see
     sharpening.md for current baselines).
  5. Verify `graph_report.served_by.reflow2_version` says **0.3.0** — the new skew check, and the
     proof the restart actually picked up the new binary.
  6. Anything that diverges between the live surface and the instruments is a finding — record it in
     [docs/trials/](docs/trials/).
  7. **Then cut the release:** `git tag v0.3.0 && git push origin v0.3.0`. The bump, CHANGELOG
     section and upgrade doc are already on main — the tag is deliberately the last step, after the
     live surface has been exercised.

## Recently finished

Trimmed periodically; the durable history is [CHANGELOG.md](CHANGELOG.md) and `git log`.

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
