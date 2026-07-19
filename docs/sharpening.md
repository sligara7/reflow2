# Sharpening reflow2 — how this project finds its own gaps

> Part of the **Reflow 2.0** design docs — see **[overview.md](overview.md)** for the map.

**This is the standing method for evolving reflow2, not a record of one exercise.** If you are an
agent improving this project, read it before you pick up a backlog item: it tells you where findings
come from, how much to trust one, and how to avoid the specific way this kind of work goes wrong.

The founding problem is the one that sank the original reflow. Its early phases went well and its
later ones proceeded as if the design did not exist — and *nobody noticed at the time*, because
nothing measured it. A tool for keeping designs coherent that cannot tell whether its own design is
coherent will fail the same way. So the question is not "is reflow2 good?" but **"what would tell us
it isn't, and are we running that?"**

---

## 1. What using reflow2 on itself can and cannot tell you

The license for the whole method is one fact, best stated the way the project's author put it:
**reflow2 is the one subject where we possess the expected value.** We know what it should do, and
we can see what it actually does — so a mismatch is a defect, not a debate. On a user's design,
"is this really a duplicate?" is a judgment call; here the right answer is known before the tool
speaks. Everything else in this document is machinery around that one asymmetry.

Self-hosting also looks circular, and in one specific respect it is: **if a capability is missing,
its absence is exactly what the missing capability would have detected.** Not infinite regress — a
hole that cannot see itself.

That is not a guess. On 2026-07-19 twelve items were raised in one session. How each was actually
found:

| How found | Items |
|---|---|
| Tried to do something and hit a wall | BL-36, BL-37, BL-39 |
| Injected a probe built around a hypothesis | BL-30, BL-31, BL-33 |
| Code or schema reading | BL-29, BL-34 |
| Compared two runs against each other | BL-35 |
| Noticed by accident | BL-32 |
| **reflow2 emitted something and a human knew it was wrong** | BL-38, BL-5 |

reflow2's own output contributed to **two of twelve**, and both required already knowing the answer:
it reported 11 capabilities unbuilt and 22 nodes as single points of failure, and those became
findings only because the code demonstrably ships and the topology is demonstrably fine.
**Not once did reflow2 correctly report a problem with itself.** Do not expect it to.

*Dated follow-up, later the same day.* After BL-38 and BL-5's second pass cleared the known-false
output, the design-graph instrument reached **zero known-false**: all 16 gaps and 14 defects are
true, including one — the `Release`/`Environment` island — that is reflow2 independently reporting
BL-34's consequence, and two that found real omissions in the committed model. The claim above stays
as the baseline it was; the lesson is that reflow2's self-reports become usable *exactly to the
degree the noise has been paid down*, and the paydown is measurable.

**So what is self-hosting actually for?** Two things, both real:

- **It generates load nothing else does.** Three of today's items exist purely because someone tried
  to use reflow2 on reflow2 and hit a wall. A wall is observable from outside the system even when
  the blind spot is not. You cannot see your own blind spot; you can notice you keep bumping into
  the furniture.
- **It is the only subject where the expected value exists** — the asymmetry this section opened
  with. Noise is unambiguous rather than debatable, so a false positive is immediately legible as a
  defect.

And one thing it is *not* for: **reflow2's silence is never evidence that reflow2 is healthy.** It can
say what is missing. It cannot certify what is there.

## 2. The failure mode to actually fear

Not circular *detection* — circular **accommodation**: shaping the model until the tool goes quiet,
then reporting that the tool is fine.

It nearly happened the first time. Modelling reflow2's design, artifacts were linked to *Components*
(a file realizes a module — how code is actually organised), which produced 11 false
`unrealized_capability` gaps. The tempting move is to "fix the model" to `Artifact → Capability`, at
which point the graph reports clean and **BL-38 never exists**. Same tool, same repo, opposite
conclusion, and the version that looks better is the one that learned nothing.

> **Rule.** When self-hosting produces noise, write down the modelling decision *and* the output
> **before** deciding which of the two is wrong. Otherwise the design quietly bends toward whatever
> keeps the tool happy.

The sibling failure this repo has already committed **five times** is *"a client we wrote agreed with
itself."* Home-grown test layers concur and are all wrong, because each was built on the same
assumption — the `structuredContent` array bug, `delete_node`, `graph_report_markdown`, BL-28's five
untyped parameters, and BL-32's stale server, which `smoke_mcp.py` cannot catch by construction
because it spawns a fresh binary every run. **Asserting the published contract is a different check
from asserting behaviour through your own client, and it is the one that catches this class.**

## 3. The method

Five practices, each earned:

1. **Inject probes; do not wait for divergence.** The highest-value findings in the whole record —
   ophyd's unmotivated capability, 3dtictactoe's orphan, the failing-test probe — came from seeding
   a defect *on purpose* and checking whether the tool noticed. Observation inside a short trial
   finds almost nothing.
2. **Pre-register what you expect, then score it.** A live golden thread makes specific claims that
   are sometimes wrong; a decorative one makes claims that are vacuous. Writing the prediction down
   before running is what tells them apart. The phase-detector diagnosis was predicted from
   *"there is exactly one reconciler"* before the trial ran, and the trial confirmed the shape.
3. **Attempt the operation you would otherwise route around.** Scripts route around friction; skills
   and real usage hit it. Every wall is a finding. BL-37 and BL-39 exist for no other reason.
4. **Ask the second question.** "Did a file change?" is nearly worthless — the answer is always *yes,
   I fixed a bug*. "Does the design still describe what shipped?" is the one that finds things.
5. **Render, and confess.** The graph is the single source of truth and a view is a *projection*
   of it — DoDAF/UAF-style: many viewpoints, one model. So a renderer may only draw what the graph
   states, and **everything it needs but cannot find is a finding**: a modelling gap, a reflow2
   gap, or a true design gap — never something for the agent to improvise past. If producing a
   description of the design requires extrapolation, that is almost always a defect signal
   pointing back at the graph. `tools/render_views.py` is the standing form: its first run
   confessed exactly BL-37 (no flow view is expressible) and a committed-model gap (no capability
   dependencies), and its own first draft had to be tightened when it produced zero confessions by
   only asking questions the graph could already answer — the instrument-accommodation trap from
   §4, live.
6. **Grade your own evidence.** Mark what you verified by execution versus what you read in the
   code, in the same entry. Several BL-29 hazards are recorded as code-read and unreproduced, on
   purpose. A backlog that cannot tell the two apart rots.

## 4. The instruments, and what "better" means

Today's exercise left four runnable scripts. **They are the fitness function** — the numbers that say
whether reflow2 is getting better at the thing it claims to do. All are currently failing on purpose
and record a baseline to move:

| Instrument | Asks | Baseline (2026-07-19) | Blocked on |
|---|---|---|---|
| `tools/phase_trial.py` | Does the design carry weight after P2? | **9/13** — P3 4/4, P4 2/4, P5 0/2, thread 3/3 (was 8/13; BL-30's S half caught the failing-verification probe) | BL-31, BL-30 (M), BL-9 |
| `tools/erosion_trial.py` | After N fix cycles, does the design describe what shipped? | **5/8** (was 2/7 at baseline; the erosion signature — N claims, zero design edits — is now legible in the confirmation ledger), remaining misses are BL-34 territory | BL-33, BL-34 |
| `tools/coherent_erosion_trial.py` | Is `designed == released` reachable, and does anything drive it? | **6/9** (was 4/9) — the accept poses the second question (BL-33) and the ledger tells which fix moved the design (BL-35) | BL-34, BL-36 |
| `tools/build_design_graph.py` | What does reflow2 say about reflow2's own design? | 96 nodes, 16 gaps, 14 defects — **every output true** (was 33/36 with 13 false gaps and 24 false defects; BL-38 and BL-5's second pass cleared the noise) | — |

`tools/smoke_mcp.py` stays the gate for the shipped surface; these four are the gate for whether the
*loop* works. They exit non-zero by design and should not be wired into CI as pass/fail until the
items above land.

> **A score can improve two ways, and only one is progress.** When a number moves, check whether the
> tool got better or the trial got easier. Loosening a probe to make a baseline go green is
> accommodation aimed at the instruments instead of the model, and it is harder to spot because it
> looks like progress.

## 5. Where the surprises come from

Every finding in this project's history that was *surprising* rather than *confirmatory* came from
outside:

- the `structuredContent` bug — a different model on a different harness;
- BL-27 — two codebases nobody here wrote;
- BL-28 — a harness difference, invisible to the client we built;
- BL-15, BL-18 — a real external user on a different OS.

Self-host confirms and generates load. **External subjects are the only source of genuine surprise**,
so the regimen degenerates without them. Two constraints follow:

- Keep at least one **non-reflow2 subject** in rotation, and prefer one nobody here wrote.
- Watch for the tell that self-host has gone stale: **the instruments all pass while real users still
  hit friction.** That means the probes have converged on what the tool already does.

There is also a standing bias to weigh. Until 2026-07-19 every trial stopped at or before **P2** — the
phases the original reflow was already good at — so an item citing "three independent trials" often
means three trials of the front half. Check what a source actually exercised before leaning on it.

## 6. The cycle

The naive statement of the loop is: *use reflow2 on itself; we know what it should do and can see
what it does; fix it until actual equals expected; move to the next thing.* That is the engine, and
it is worth writing down because each of its three steps fails in a specific, observed way. The
corrected cycle:

**1. Attempt — including the things you would route around.** The naive loop assumes there is
always an "actual" to observe. Often there is not: three of the twelve 2026-07-19 items (BL-36,
BL-37, BL-39) were not mismatches but operations that could not be performed at all — no tool for
`precedes`, no constructor for `Flow`, no way to import a design. Nothing to compare, and still a
finding. **A wall produces no observation; count it anyway.** This is why scripts under-find:
scripts route around friction, and the friction is the data.

**2. Compare — knowing the expectation often forms on contact.** The naive loop reads *know
expected → observe actual*. In practice it frequently runs *attempt → be surprised → then articulate
what should have happened*. Nobody had written down what reflow2 should say about a failing test
until it was watched saying nothing. "We know what it should do" is the step that quietly fails —
you think you know, and you find the assumption only when it breaks. And sometimes the defect is
invisible in any single run: the eroded graph and the coherent graph each returned a defensible
`[]`; the defect *was that they matched* (BL-35), findable only by comparing two runs to each other.

**3. Reconcile — deciding which side was wrong, on the record, before changing either.** "Fix
reflow2 so actual = expected" has two solutions, and one is cheating: you can also fix the *model*
until the tool goes quiet — §2's accommodation, which was nearly committed the first day this method
was used. The honest form records the modelling decision and the output first, then rules on them.
Sometimes the expectation is what was wrong; that is a finding too, and it goes in the trial record
like any other.

**4. Leave a probe behind, then move on.** The naive loop ends at "move to the next fix," and the
evidence against that is one day old: BL-5 was fixed, measured 8 → 2 on the graph it was fixed
against, and reopened the next day at 22 of 36 on a design with a different shape. It regressed
silently because the measurement was taken *once*. A fix without a standing probe is an anecdote;
the instruments in §4 exist so the next agent inherits the measurement.

Concretely, when you change reflow2:

1. Run the four instruments before and after. A number that moves is the claim; a number that moves
   the wrong way is the finding.
2. If you are adding a detector or a phase capability, add a **probe** for it to the relevant
   instrument, so the next agent inherits the measurement rather than the anecdote.
3. If the tool went quiet, ask whether the design got better or the model got accommodating.
4. Write findings into [trials/](trials/) verbatim and raise a **BL** item, marking verified-by-execution
   separately from code-read. That is what makes a finding survive the session that found it.

The point is not to be rigorous for its own sake. It is that reflow1's failure was invisible while it
was happening, and the only defence against repeating it is measuring the thing that failed —
continuously, with instruments that are allowed to say no.
