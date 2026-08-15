# reflow2 feedback (agent session notes)

Session notes from agents using this project's graph. Local only — not a filed
issue. Product/design content is summarized by type and count, not quoted.

---

## 2026-08-14 — implement a designed increment, then iterate a rule (Grok 4.6)

**Environment:**
- Live MCP: **reflow2 0.31.0** (`reflow2-mcp --version`; `graph_report.served_by.reflow2_version` = `0.31.0`). `served_by.stale` is `null` (non-Linux; unknown is not current).
- On-disk `.reflow2/kit-version.json` still says `0.11.0` / `0ec473e` (2026-07-25 tarball). Kit sidecar and the process you are talking to are not the same generation.

**What I was doing:** User asked where the MVP stood, then to *build* an
already-designed increment (persist + send + inbox; not the later transport).
After ship: three product-rule corrections on the same gate (first calendar
day; civil-date vs UTC date of `createdAt`; whether the join day itself is
owed if it is “yesterday”), then an as-built cache miss (save on one surface
did not refresh a derived flag on another). Graph writes throughout
(ChangeEvents, snapshots, Decision/Requirement merges, Artifact checksums).

### Strengths

- **The graph held enough settled Decisions** that the first implementation
  pass did not invent the verb, the daily cap, viewed-on-scroll, or topic
  list. That is the point of capture-before-build and it worked.
- **`compare_designs` vs the committed export** at session start was honest:
  live graph was already ahead (new Capabilities/Decisions/Artifacts). Better
  than guessing from counts.
- **`record_change` snapshots** survived the gate being rewritten three
  times. A later reader can see the swing without git archaeology — when the
  snapshot was taken *before* the merge.
- **`add_change_event` refusal named legal `change_type` values** when I
  passed an unknown one. Actionable error.
- **`link_artifact` + checksum** closed `unrealized_capability` on the new
  files. Confirming “this Capability is no longer in that gap’s
  `affected_ids`” is the right check (total gap count went up, as the skill
  warns).
- **`search_design`** still the right way to find the gate Requirement /
  Decision when the user describes a bug in their words.

### Issues / friction (new or confirmed while *building*)

1. **`add_change_event` has no `change_type` for an as-built product bug.**
   I sent `bugfix`. Refused. Legal set is
   `requirement_creep | new_feature | test_failure_fix | performance_optimization | refactor | scope_change | constraint_change | environment_change | deprecation | resync | baseline_established`.
   A client cache that leaves a derived flag stale is not a test failure and
   not scope change. I used `test_failure_fix`. The enum forces a lie.
   **Expected:** a type for “implementation defect against existing intent”
   (or document that `test_failure_fix` is that bucket).

2. **“Record before you edit” loses to parallel tool calls.**
   `revise-design` is correct: snapshot while the node still says the old
   thing. In one batch I `record_change`’d a Decision *after* a sibling
   `add_decision` on the same id had already merged. The Snapshot stored the
   *new* statement. No warning that the prior state was already gone.
   **Cost:** the timeline for that revision is a lie; easy for any agent
   that parallelizes writes.

3. **Revision ceremony is too heavy for a three-line rule correction.**
   Tightening “does this gate apply when yesterday == signup civil date?”
   cost: epoch (already existed), two `record_change`, two typed merges,
   tests, two checksum accepts. The user was iterating a live bug. The loop
   is right for a first capture; it is slow for “you implemented the wrong
   comparison.” Agents will skip snapshots under that pressure — I almost
   did.

4. **An accepted Decision is too easy to overwrite with the agent’s
   diagnosis.**
   The UTC/civil-date bug and the “just joined” waiver are different
   claims. I merged a *generous* waiver into an accepted Decision while
   debugging the UTC bound. Graph-text-is-data correctly did not stop me.
   Nothing in revise-design asks “are you changing the rule or fixing
   as-built to match the rule you already have?” The user had to walk it
   back.
   **Expected:** when merging an `accepted` Decision, require an explicit
   “this is a new rule” vs “restore prior accepted text / as-built was
   wrong.” Or refuse to widen an accepted gate without `set_decision_status
   proposed` first.

5. **As-built semantic drift is invisible.**
   A Requirement already said the calendar day is the person’s civil date,
   not UTC midnight. The realizing check-in still used `createdAt`’s ISO
   date (UTC). `reconcile_artifacts` can only see checksums. Status said
   realized. The user found it in the product.
   **Expected:** not magic, but a documented hole: checksum honesty ≠
   “this file still implements the Requirement’s date rule.” Maybe
   `evidence_report` / a note on REALIZES that the claim was never checked
   against that property.

6. **`detect_gaps(scope: Capability)` explodes.**
   On the increment Capability, `depth` 1–3: `in_scope` ~47–51, almost
   every hit `unverified_capability` on unrelated planned work. Could not
   answer “is *this* Capability still in an `unrealized_capability` gap?”
   without grepping `affected_ids`.

7. **`propagate_change` from a Capability allocated to a shared API +
   shared client is not an edit list.**
   Distance-1 was the two Components plus the Requirement/Decision thread;
   distance-2 was every other Capability on those Components (~65 named,
   113 truncated). “Edit only what the radius names” is unusable. The
   useful signal (Interface `PROVIDES`/`CONSUMES`) was buried.

8. **`set_artifact_checksum` disposition is the wrong question mid-bugfix.**
   After aligning as-built to an *already accepted* Requirement, neither
   `design_holds` (“no design meaning”) nor `design_updated` (“design
   moved; pass the ChangeEvent”) fits cleanly if you *also* revised a
   Decision in the same hour. Agents spend a turn picking a side instead
   of shipping the comparison fix.

9. **`loop_status` is a coherence dashboard, not a build dashboard.**
   All session: ~62 never-asked gaps, 39 structural defects, 16 unproven
   claims, 10 unexamined artifacts. `next[0]` is always detect-and-ask.
   The work was implement → user-correct → implement. That noise is worse
   when the user is waiting on a running app.

10. **`add_decision` merge still hints `lands proposed`** on a write that
    left `status: accepted`. I did not accidentally demote this time; the
    hint still invites it.

11. **`link_artifact` accepted a Requirement as `target_type`.**
    Skill text says Capability or Component. The call succeeded. Either
    the tool is wider than the skill, or this is an accidental accept.
    Unclear whether REALIZES onto a Requirement is meaningful.

12. **Committed export stayed stale.**
    `compare_designs` at start already showed a large live-vs-export
    delta. This session wrote more and did not refresh
    `reflow2-export.json` (shared server holds the store; export wants it
    stopped). Agents will not `--stop-shared` in the middle of a
    user-facing debug loop.

13. **`get_instructions` truncates** (~22.6 KB, cut mid-file). Standing
    rules after the cut never arrived.

14. **Unit Verification + `status: realized` overclaimed.**
    I linked a passing unit Verification and marked the increment
    Capabilities realized. The user then found first-day, civil-vs-UTC,
    join-day-owed, and a cache miss. `realized` is a claim about the
    thread, not about the running product. Nothing in the loop said
    “realized at unit, unvalidated in the field.”

### Opportunities

- Add `change_type` for as-built defects (`defect_fix` / `as_built_fix`),
  or officially alias `test_failure_fix`.
- `record_change` should refuse (or warn) if the target’s current hash
  already differs from what the caller thinks they are snapshotting —
  or be a required first step inside the typed merge when `status` is
  `accepted`.
- Scoped `detect_gaps`: filter to gaps whose `affected_ids` include the
  seed (or its SATISFIES/REALIZES neighbors), and do not raise
  `unverified_capability` for `planned` + zero artifacts.
- `propagate_change` summary: separate “same Component dump” from
  “crossed an Interface.” Default `full` off is right; the summary
  still needs a “sibling Capabilities on the same Component, not
  actually impacted” band.
- Checksum accept: allow `as_built_aligned` when a Requirement did not
  change and the file now matches it.
- `loop_status` session mode: “building this Capability” hides
  unsurfaced project-wide gaps unless they touch the seed.
- Remaining-work / increment view: this session used an accepted
  Decision’s prose plus Capability `status` as the MVP list. `what_next`
  ranks open Decisions and does not answer “what is left to build.”

### Gaps (mine / process)

- I promoted two Capabilities to `realized` on unit tests before the
  user had clicked through. Should have left `in_progress` until the
  gate matched their account.
- I widened an accepted Decision from a debugging hypothesis instead of
  treating civil-date vs UTC as as-built only. That cost the user two
  extra turns.
- I did not export the graph. I did not run detect-and-ask on the 62
  unsurfaced gaps (correctly: they were not the work).
- I did not install the loop nudge; `loop_status` was called by hand.

### Not filed upstream

Local log only (per `report-friction`). Worth a redacted report if you
want them sent: items **1** (change_type enum), **2** (snapshot after
merge), **5** (semantic drift vs checksum), **6** (scoped detect_gaps).
