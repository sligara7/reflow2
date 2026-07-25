# Friction report — 2026-07-25

Four things fought me while building today (KPP capture, the github-mcp imports, the merge driver,
the brainstorm skill, scoped detection). Written with the **report-friction** skill, which I had
never actually run before today — the last item is what running it exposed.

No redaction was needed: this is reflow2 designing reflow2, so the "design content" and the
maintainer's content are the same. Worth noting as a small finding about the skill itself — its
redaction discipline is load-bearing for a consumer project and moot when self-hosting, and it
does not say so.

## 1 · A tool that gains its FIRST parameter silently ignores it — HIGH

**What I was doing.** Added a `scope` parameter to `detect_gaps`, then called it with `scope` set,
against the MCP server this session had been holding open since before the build.

**What I expected.** Either the new behaviour, or a loud failure — the surface was declared
`deny_unknown_fields`, so an unknown argument should be refused.

**What happened.** The call succeeded and returned the *unscoped* answer, with no warning. The
argument was dropped in silence.

**Why.** The old handler took no `Parameters` at all, so nothing deserialized the arguments object
and there was nothing to reject with. `deny_unknown_fields` only protects a tool that already has
a request struct. `served_by` reports version and binary mtime (BL-32), but a caller comparing
those has to already suspect the problem — and nothing in the *result* says an argument was ignored.

**Minimal shape.** A tool with no parameters, called with any argument, served by a binary
predating the parameter's addition. Every tool's first parameter has this window.

**Why it matters.** I nearly reported the feature as broken. Worse is the general case: the
first-parameter transition is exactly when a caller is most likely to be running a stale server,
because the parameter is new. A wrong answer that looks right beats a loud failure every time —
and this project's whole position is the opposite (`req:no-silent-fallback`).

**What would have helped.** Any of: a request struct on every tool from the start, even an empty
one; or the smoke/toolsnap layer asserting that a served tool rejects unknown arguments; or the
launch wrapper refusing to serve when the binary predates the working tree, which it half does
already (it content-hash rebuilds — it just cannot help a session that was already open).

## 2 · `add_decision` defaults to `accepted` — HIGH

**What I was doing.** Recording six open questions today as `proposed` Decisions (three of them
brainstorms, which are open by definition).

**What happened.** Every one landed as `accepted` and needed a `set_decision_status` correction
immediately afterward. Six times in one session.

**Why it matters.** For Requirements this project treats an unearned status as forgery —
`dec:certainty-derived`: "every move off `proposed` records the USER's word, never your own
judgment", and `set_requirement_status`'s own description says so. Decisions have no equivalent
guard and the opposite default: a Decision recorded as *settled* when it was open asserts that a
choice was made and reasoned. That is the same forgery with more consequence, because a settled
Decision is what `where-am-i` reads back to the user as "what you decided", and what
`kpp_contradicted` and the fork layer treat as binding.

Evidence that this is not just my slip: it is recorded as a standing GOTCHA in my own session
memory ("`add_decision` defaults to `accepted` — correct it"), meaning it has been noticed
repeatedly across sessions and never filed. And the **brainstorm** skill written today had to
carry the workaround in prose: *"`set_decision_status` to `proposed`. This call is not optional."*
A skill instructing a human-facing correction for a tool default is a defect with a bandage on it.

**What would have helped.** `proposed` as the default, with `accepted` an explicit act — matching
`add_requirement`. That is a behaviour change to a shipped tool, so it wants a minor bump and a
line in the upgrade doc; the alternative (documenting the default in the tool description) leaves
the forgery available.

## 3 · No way to see only NEW structural defects — MEDIUM

**What I was doing.** Checking structural health after each of five builds.

**What happened.** `detect_defects` reports the same six findings every time — five accepted single
points of failure, plus one accepted the moment it appeared. All six carry recorded Decisions
explaining why they stand. I had to diff the list by eye each time to see whether anything new had
arrived, and the count moving 5 → 6 was the only signal.

**Why it matters.** `reviewed_gaps` solved exactly this for gaps: an acknowledged gap moves out of
`detect_gaps` into a reviewed list, "not deleted, not hidden", so the open list means *still needs
attention*. Defects have no counterpart, so the honest six are permanent noise, `loop_status` can
never report the structural set clean (already accepted as BL-93), and a genuine seventh defect
would arrive into a list nobody reads carefully. The tool's own docstring makes the promise the
gap side keeps: "a list that can never reach zero gets skimmed."

**Minimal shape.** Any graph with an accepted architectural defect — one hub component is enough.

## 4 · reflow2's own installed kit is four releases stale, and nothing checks the manifest it wrote — MEDIUM

**What I was doing.** Reading `.reflow2/kit-version.json` for the version to put in this report.

**What happened.** It says `reflow2_version: 0.8.0`, commit `4f6e427`, installed 2026-07-21. The
repo is at 0.10.1. The manifest lists 12 skills with content hashes; the kit now has 15, and at
least three of the listed hashes no longer match the files on disk (I edited two skills today and
added three). Nothing anywhere noticed.

**Why it matters.** The manifest records per-file hashes, which is only worth doing if something
compares them — the same reasoning `link_artifact`'s checksum rests on. `skill_lint` checks that
the three *source* copies are byte-identical; the *installed* kit is outside its remit. So the one
place that records what a consumer actually has is unverified, and reflow2's own repo is the
first place it drifted. Anyone reading that file to answer "what version is this project running"
gets a confident wrong answer, which is the failure mode the doc-version drift check was built for
last week, in a file that check does not look at.

**What would have helped.** Either the installer refreshing the manifest when it refreshes skills
(it may already, and simply has not been re-run here since 0.8.0 — in which case the finding is
that nothing *reminds* anyone), or a check that compares manifest hashes to disk and reports drift,
which is `reconcile_artifacts` applied to the kit.

## Environment

- reflow2 0.10.1 (working tree at `3783591`); `.reflow2/kit-version.json` claims 0.8.0 / `4f6e427`
- Claude Code (Opus 5), Linux 6.17
- Self-hosted: reflow2's own design graph, 545 nodes / 1660 edges
