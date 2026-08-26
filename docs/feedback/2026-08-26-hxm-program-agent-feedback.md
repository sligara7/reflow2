# reflow2 feedback — from using it as a *hub* over many repos

Running log, kept by AJ's agent sessions on `hxm_program`. Dated entries; newest first within each heading.
Design content is redacted the way the `report-friction` skill asks (node *types* and *shapes*, not names or
statements) except where the point is unintelligible without it. Nothing here has been filed as an issue —
AJ decides what to send.

**Environment (2026-08-26):** reflow2 0.40.0 (commit `d68bb88`, release-tarball, built 2026-08-25). The server
was 0.39.0 for most of the 2026-08-25 session and reconnected as 0.40.0 on 2026-08-26. Harness: Claude Code
(Fable 5), Linux. The hub graph: 187 → 251 nodes / 479 edges over the 2026-08-25 session; minted on 0.23.0.

**The use case, in one paragraph.** `hxm_program` is not a product; it is a *hub* — a living, agent-maintained
source of truth for a program of beamlines whose actual code lives in ~10 other repos (profile collections,
tools libraries, a simulated beamline, upstream community libraries). The graph is used less as "the design of
a system" and more as the program's **memory and coherence engine**: decisions, brainstorms, review records of
*other people's PRs in other repos*, craft rules learned from a colleague, verification state of work that
happens elsewhere, and the connective tissue between all of it. Only one person (AJ) runs reflow2; everyone
else reads the repo's markdown. That asymmetry and the multi-repo sprawl are where most of the feedback comes
from. On this machine there are eight `.reflow2` stores: the hub, two sim-scoped sub-designs (one of them
now living in a *different* repo, `hex-ob`), a "digital beamline" vision design, three unrelated projects,
and reflow2's own repo (opted-in, empty). None of the seven siblings has a committed export or a published
surface.

---

## What works well

### 2026-08-25/26 — a full day: three brainstorms, a 16-question gap pass, two external-PR reviews, two PRs delivered

- **The duplicate guard is the single most valuable behaviour.** `add_requirement` / `add_capability` /
  `add_decision` refusing a near-duplicate and naming the existing node — with the explicit choice *sharpen
  it* or *`distinct_from`* — caught real overlaps every time (a capability vs its requirement; a brainstorm
  decision vs the requirement promoted from it). It also makes the agent search before writing, which is the
  habit the whole tool depends on.
- **`brainstorm` as a first-class skill is the right shape for how design conversations actually go.** Seven
  ideas across three brainstorms in one day, each with counter-arguments *beside* the option, none promoted
  until the user said "keep (g)+(b)". Writing the "road taken" back onto the brainstorm decision means a later
  reader meets reasoning and outcome in one place. `review_relations` with the *"nothing honestly related —
  here is what I searched"* note is genuinely good design: it separates "nobody looked" from "somebody looked
  and found it new".
- **`loop_status` is the right size.** One cheap call that says exactly what is owed, in the order to do it.
  It kept a long session honest at every hand-off, and the `sync` block (committed export in step / behind)
  answered "did I forget to export" without a separate tool.
- **`gaps_to_prompts` → answers → `acknowledge_gaps` with a per-gap reason** turned 20 detector findings into
  a 16-question walkthrough the user answered in one message ("agree all"). Requiring a reason per gap is
  right: the acknowledgements read well weeks later.
- **`unstated_rule_enforcement` earned its keep**: it forced "is this a gate or advice?" on 12 craft rules
  that had been recorded without anyone deciding. Three became gate-blocking; that had consequences (they
  now must live in a file people can read).
- **`export_graph` is deterministic, diffable, lineage-chained, and `loop_status` knows whether the committed
  file is in step.** "Commit the graph into the repo" became a five-minute change with a check behind it.
- **`describe_designs` reading only sidecars** — safe to point at every `.reflow2` on the machine without
  minting anything; found eight designs (one empty) in one call, with the version each was minted on.
- **`coverage_report` on the hub repo (2026-08-26):** 72 of 78 swept files claimed by artifacts, 6 small
  unclaimed files named (a watch script, three harness-pointer files, `index.html`, `.gitignore`), two
  subtrees correctly reported as *opaque claims*, and — the useful part for a hub — every artifact whose
  location is in **another repo or a PR** listed under `unobserved_locations` rather than silently counted as
  covered. That is exactly the honesty a multi-repo design needs.
- **Error messages that teach.** `add_change_event` refusing `description` and saying *use `summary` and
  `rationale`*; `create_node` reporting `undeclared` properties but still writing; `revision.replaced`
  handing back the prior value on every overwrite. The tool tells you what it did, not just that it did it.
- **`where-am-i` + `what_next`'s four bands** produced a one-minute orientation that matched what the user
  cared about; the "unexplored" band's *"this is a sample, not the least important"* caveat is honest.
- **Skills served from the binary** meant the instructions never drifted from tool behaviour across the
  0.39→0.40 reconnect mid-session.

---

## Issues (report-friction shape)

### F-01 · `gaps_to_prompts` refuses a gap object unless `suggested_depth` is present — but a budgeted `detect_gaps` withholds it

- **Doing:** the two-pass detect→ask handshake on 16 gaps, passing each gap object back.
- **Expected:** a gap object as `detect_gaps` returned it to be accepted; the docs say *"REPLAY EACH GAP OBJECT
  UNCHANGED"*.
- **Happened:** `invalid GapCandidate: missing field 'suggested_depth'`. The budgeted reply
  (`detail: titles_only`) had withheld fields, so "unchanged" was impossible. The refusal names the field, so
  it cost one retry.
- **Minimal shape:** `detect_gaps(budget_chars=12000)` on a 20-gap design → titles-only items → pass one to
  `gaps_to_prompts`.
- **Why it matters:** the budget mechanism and the replay contract fight each other. Either the budgeted reply
  keeps the fields the handshake needs, or `gaps_to_prompts` accepts a gap `id` alone.

### F-02 · The duplicate guard fires between a Requirement and the Capability that satisfies it

- **Doing:** capture-intent — `add_requirement`, then `add_capability` for the thing that delivers it. The
  wording overlaps by construction.
- **Happened:** every capability creation after its requirement was refused and needed a second call with
  `distinct_from: [req:X]`. Six times in one session; the same for a brainstorm Decision vs the Requirement
  promoted from it.
- **Why it matters:** the guard is right to exist, but req↔cap and decision↔promoted-requirement are the two
  *expected* near-duplicates in the golden thread. An `add_capability(satisfies=req:X)` parameter that draws
  the SATISFIES edge would remove the retry *and* the separate edge call.

### F-03 · Brainstorming, done exactly as the skill says, raises structural defects

- **Doing:** `brainstorm` skill: record ideas as proposed Decisions; link with `CONTRADICTS` / `ANTICIPATES` /
  `DEPENDS_ON` and evidence, as instructed.
- **Happened:** `detect_defects` reports the CONTRADICTS edge as a `contradiction` warning, the linked ideas as
  an `unthreaded_cluster`, and each ANTICIPATES edge as `unresolved_setup` ("X anticipates Y but nothing
  follows through"). Defects went 2 → 7 over a day of doing the right thing, and `loop_status` repeats
  "7 structural defects outstanding" to the user at every check.
- **Why it matters:** the skill says *"do not run detect-and-ask over brainstormed nodes"*, but the defect
  detectors have no such exemption. A proposed Decision that is deliberately a brainstorm should be swept as
  `parked` (as `governed_by(ruling: parks)` does), or the detectors should skip relation edges on `proposed`
  decisions.

### F-04 · `DesignRule.description` is undeclared

- `create_node` on a `DesignRule` with `description` reports `undeclared: ["description"]` — yet every rule in
  this graph carries one (written through the same tool by an earlier session), and the working practice
  "thicken the rule's description with each new sighting" depends on it. Declare it, or say where a rule's
  supporting observations should go.

### F-05 · No home for operational know-how ("this is how we do X here", beamline quirks)

- The `capture-intent` skill itself says *"nothing fits cleanly … recorded as an open gap in reflow2's own
  design"*. This hub's most valuable content is exactly that: the hand-edited IOC template, the GUI only
  reachable via VDI, the script that ships in test mode. Today it lives in task-file prose and, since this
  session, in a dated "known quirks" section on a page — **outside the graph**. A node kind with *subject
  part, observed-when, still-true-as-of, source task* would let the graph hold what the hub is for.

### F-06 · A ChangeEvent lands undated unless the caller remembers `detected_at`

- Nine events written in one day, none dated in the field (dates are in the prose). The verification digest
  then cannot order changes against `last_run_at`. Suggest: report an undated ChangeEvent the way an undated
  sweep is reported — as undated, not silently.

### F-07 · "Never exported, single machine, git-ignored" is silent

- Until this session, 60+ decisions and every enforced rule existed only on one laptop; a collaborator was
  being reviewed against gate-blocking rules he could not read. `loop_status.sync` now covers "the export is
  behind" once an export exists; **"there is no export at all"** is still silent. A design written to for
  weeks with no export deserves a line. (Also: none of the seven sibling designs here has one — see O-02.)

### F-08 · Revising a Decision means re-sending the whole field

- Documented (*content fields optional to revise*) and made safe by the `revision.replaced` echo — but writing
  a brainstorm's outcome back onto its decision required re-sending the entire `decision` text five times in
  one day. An append-style call (`record_outcome` on a Decision) would match the brainstorm skill's own step 5.

### F-09 · Session-end nudge

- `nudge: absent` in every `loop_status`. The harness's Stop hook did fire once ("1 graph write and no loop
  check") and was correct. The advisory says the harness "may have one of its own, which reflow2 cannot
  see" — it could: the hook could write a marker file reflow2 reads.

### F-10 · Sizing defaults that undercut the prescribed workflow

- `detect_gaps`' default budget produced `titles_only` on a 20-gap design (see F-01); 28 000 was fine.
- `where-am-i` asks for `graph_report_markdown` on top of `graph_report`; on this graph the digest was enough
  and the markdown was too large to be worth reading.

### F-11 · `coverage_report` echoes `excluded: []` when exclusions were applied upstream

- I filtered `codegraphs/`, `docs-graph/` and the export out of the sweep *before* calling, and passed the same
  names as `exclusions`; the reply lists them under `unobserved_locations` (correct — artifacts claim them)
  but `excluded` comes back empty, so a reader cannot see that they were deliberately left out. Minor: the
  tool cannot know what the sweep never saw; a note that exclusions are matched against `observed` would do.

---

## Opportunities (the hub-over-many-repos angle, in priority order)

### O-01 · A *hub / parent* relationship between designs — distinct from peer seams and from merge

Today two designs can **link** (peer seams: `published`/`required` boundaries, `pair_designs`, `seam_report`,
`mirror_surface`) or **merge** (absorption). This program needs a third thing: *this design is the hub of
those* — ownership and roll-up without absorption. What the hub wants to say about the simulated-beamline
design that now lives in another repo:

- "Capability X here **is realised by** their design" (a cross-design REALIZES/SATISFIES), so the hub stops
  keeping shadow copies of the sim's capabilities — which it does today, and nothing flags the duplication.
- "When their committed export moves, tell me" — `sync_status` against *another* design's export.
- "Roll their verification state up" — the hub's `loop_status` showing the sim suites' pass/fail without the
  hub owning those verifications.
- `describe_designs` reporting each design's last export date, so a hub can see which children have gone dark.

The seam vocabulary (medium, auth, payload, error model) fits *interfaces*; the hub's relations are
dependency/ownership, and forcing them into `Interface` nodes would be modelling fiction. (Recorded in the hub
graph as an open brainstorm, 2026-08-26, with the link / merge / index / hybrid options and their costs.)

### O-02 · Linking needs every partner to export — make that a default, not a discipline

`link-projects` forbids opening a sibling's store (correct), so everything depends on partners committing
exports or publishing surfaces. Zero of seven siblings here have one — and they all belong to the same person,
so the missing piece is not coordination but a nudge (F-07) plus a recognised default path (`reflow2.json`
at repo root) that `describe_designs` and `sync_status` find without configuration.

### O-03 · A first-class "review record" for other people's PRs in other repos

The most common thing this hub does is review someone else's PR in a repo that has no design: measure (lint
counts, pass/fail/error), find, hand findings to a human to post one at a time, re-check on the next head,
track which findings each colleague commit closes. Today that is a ChangeEvent + a Verification (planned /
failing) + a planning file, all improvised. A shape — *review of artifact at version V: measurements,
findings with severity and disposition, re-check at V+1* — would make `evidence_report` meaningful for
external work and let `what_next` rank "post finding 3" the way it ranks decisions.

### O-04 · Readers of the repo who will never run reflow2

Only one of ~6 collaborators runs it. This session's answer was a repo-level rule (commit the export; every
accepted decision / enforced rule must have a file-side home; a repo script checks it). Two things reflow2
could do instead: (a) `export_markdown` — accepted decisions, enforced rules, open questions rendered as the
graph's *published view*, regenerated with the export; (b) a detector — given the repo's files, which
accepted/enforced nodes are referenced by none of them — so the file-side-home rule needs no local script.

### O-05 · Verifications of work outside the design's own tree

`ver:` nodes here verify PRs on other repos, hardware sessions weeks away, CI on someone else's fork.
`evidence_report` lists all seven as *unplaced / unscoped* because no `Environment` was named. Cheap win: let
`add_verification` take `environment` and an `artifact_ref` (repo@sha or PR number) so a check can say where
it ran and on what, without modelling a Release/Environment tree the hub will never have.

### O-06 · Record *how* a check was done, not only what it found

The one thing that nearly went unrecorded this session was the *method* of the export's restricted-content
scan. `set_verification_status` has `findings`; a `method_used` (or `procedure`) field on the run would have
prompted it.

### O-07 · Ergonomics that would remove real retries

- `add_capability(satisfies=req:X, allocated_to=cmp:Y)` — the golden thread in one call (F-02).
- `record_outcome` on a Decision (F-08).
- `gaps_to_prompts` accepting gap ids (F-01).
- A `parked`/brainstorm sweep class for proposed decisions (F-03).
- `add_change_event` warning on a missing `detected_at` (F-06).

---

## Measurements to compare against next time

| Date | Graph | Session shape | Retries caused by reflow2 | Defects at session end |
|---|---|---|---|---|
| 2026-08-25 | 187 → 251 nodes, 479 edges | 3 brainstorms · 16-question gap pass · 2 external PR reviews · 2 PRs opened | 8 (6× duplicate guard req↔cap; 1× `suggested_depth`; 1× `description` on ChangeEvent) | 2 → 7, all from brainstorm edges (F-03) |
| 2026-08-26 | 251 → 252 | link-projects survey; coverage sweep; this file | 1 (Write-before-Read is the harness, not reflow2) | 7 (unchanged) |
