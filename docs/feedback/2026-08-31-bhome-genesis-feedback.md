# reflow2 feedback

Running notes on using reflow2 itself — good, bad, and ideas. Kept separate from the
design (which lives in `.reflow2/graph`). Newest entries at the bottom of each section.

---

## Session 1 — 2026-08-31 — genesis of `proj:bhome`

Context: brand-new empty repo, bootstrapping a design from a paragraph-long brief about a
self-sustaining transportable pod dwelling. ~25 tool calls, 22 nodes, 44 edges.

### What worked well

**The empty-directory bootstrap is handled unusually carefully.** A directory with no design
serves exactly one tool, and the server instructions state outright "reflow2 IS INSTALLED AND
AVAILABLE HERE... Nothing has failed." Without that, the obvious read of "only one tool is
served" is that the integration is broken. The instructions even flag their own shelf life
("THIS SENTENCE HAS A SHELF LIFE... nothing re-checks it"), which is the right way to write a
cached claim.

**`describe_designs` exists to stop a specific real failure**, and its description says which
one: a session at a repo root started a third design while two populated ones sat below it.
Tool descriptions that name the incident that motivated them are far easier to obey than ones
that just state a rule.

**Duplicate detection caught two genuine near-collisions** and, more importantly, told me
exactly how to proceed (`distinct_from: [...]`) *and* told me to reconsider first. It fired on
`cap:satellite-link` vs `req:satellite-comms-optional`, and on the Latimer-metaphor Decision vs
`req:self-sustaining`. Both were correct to flag and correct to override.

**Revision safety on `add_decision` is the standout.** Re-calling with an existing id to fix one
field returned:

> "This call REPLACED 1 property... AND NO SNAPSHOT HOLDS THE PRIOR VALUE OF `rationale` —
> checked, not assumed. For that field, the value in `replaced` above is the only copy in
> existence and this reply is the only place it appears."

It then handed back the prior value inline and gave the exact undo sequence. That turns a
silently-lossy edit into a recoverable one, and it explicitly distinguishes *checked* from
*assumed*. More tools should do this.

**`get_instructions` reports `total_bytes` / `returned_bytes`** so a client-side truncation is
detectable rather than silent, and offers per-section fetch as the fallback. This is a real
problem (a capped read drops the tail, which is where the important handshake lives) solved
without ceremony.

**`provenance: authored | inferred`** mapped exactly onto what the user asked for — "mark what
came from me and what you inferred." It was a first-class field, not something I had to encode
in prose. Good fit between what users actually ask and what the schema holds.

**Gap text is written to be acted on, not just reported.** The `quality_target_unstated` gap
explains the four-way trade-off, warns that not choosing silently picks performance, and
explicitly forbids the wrong escape hatch ("never `acknowledge_gap`, which silences this
permanently and for every capability added afterwards"). That last clause stopped me from doing
the convenient thing.

**`create_edges` all-or-nothing batching** wrote 38 edges in one call with per-item echo. The
"every item is attempted so you learn every failure at once, and if anything failed nothing is
written" semantics are the right ones.

**`review_relations` has an honest escape hatch.** `note` ("nothing was genuinely related, here
is what I looked at") is presented as a full answer rather than a weaker one, with an explicit
"NEVER invent a relation to get past this." That is the difference between a field that improves
the graph and one that fills it with noise.

### Friction

**Cross-type duplicate detection fires predictably during genesis.** Both refusals I hit were
`Requirement` vs `Capability`/`Decision` pairs that are *supposed* to coexist — the requirement
"it may have satellite internet" and the capability that provides it are the normal shape of the
golden thread, not a duplicate. The error text anticipates this ("USUALLY THIS IS THE LOOP
WORKING"), which softens it, but during genesis specifically every capability is created moments
after the requirement it satisfies, so this will fire often and each hit costs a round trip.

**Node creation and linking are separate calls, and the linking dominates.** Seeding 13
requirements + 9 capabilities took 22 calls; wiring them took 44 edges, of which 22 were pure
`CONTAINS proj:bhome -> child` boilerplate that could not have been anything else. See ideas
below.

**`related_to` on an exploratory Decision is only discoverable by triggering it.** The
requirement (and the `RelationLinkReq` shape) is documented inside a long tool description; in
practice you learn it from the refusal. The refusal is clear, so this is minor.

**Minor, and mine not reflow2's:** I passed a JSON-escaped `\"` inside a string field and it was
stored literally as `\"`. Nothing validates prose fields for that, which is reasonable — but a
graph whose content is meant to be read back to humans might benefit from noticing it.

### Ideas

1. **Let `add_requirement` / `add_capability` take `project_id` inline.** It would have removed
   22 of 44 edge writes in this session with no loss of meaning — the containment is never in
   doubt at creation time.
2. **Let `add_capability` take `satisfies: [req_id, ...]` inline.** This kills a round trip
   *and* would let duplicate detection skip the cross-type warning: if the capability declares in
   the same call that it satisfies the requirement it resembles, the "is this a duplicate?"
   question is already answered.
3. **A genesis bulk-seed call.** `import_graph` exists but is aimed at adoption. Genesis has a
   recognisable shape — N requirements, M capabilities, the satisfies matrix between them, all
   under one project — and a single call taking that would make the "seed the brief" step atomic
   rather than 60+ writes that can half-land.
4. **Consider suppressing the cross-type near-match when the two nodes are about to be joined by
   `SATISFIES`**, or at least ranking same-type matches above cross-type ones in the refusal.
5. The `gap_to_prompt` two-pass handshake is good design (reflow2 phrases, the model fills), but
   it is 2 calls per gap. A batch form taking several gaps at once would help at genesis, where
   you typically want to ask 3-5 questions in one breath.

### Still unproven this session

Nothing has been built yet, so `link_artifact`, `reconcile_artifacts`, verification and the
change-propagation path are all untested here. The gap detector currently reports
`build_without_verification` on a project with no artifacts, which is technically correct but
arrives earlier than it is useful.

### Addendum — same session, after the first round of answers

Wrote four user answers back into the design (a quality target, two accepted decisions, a
prohibition constraint, two new requirements, 21 edges).

**More good:**

- **The instrument moves when you fix things.** Gaps went 5 → 4 → 3 as I wired up the aquaponics
  reasoning and then recorded the quality-attribute answer, and each drop was the gap I had just
  addressed. That responsiveness is what makes the count trustworthy; a detector that stayed at 5
  would be indistinguishable from one that had stopped looking.
- **`SATISFIES` carries `coverage: partial`.** Being able to say "this capability half-delivers
  this requirement, and here is which half" is much more honest than a binary satisfied/not, and
  it let me record that growing food only partly answers "months between resupply."
- **Constraints are genuinely not budgets-only.** A pure prohibition — no blackwater in the fish
  water, ever, including under backflow — is a first-class Constraint with no numbers involved.
  The tool description calls this out explicitly and it was right to.
- **The revision-safety note fired consistently** on all four revise calls, not just the first.
  Consistency matters more than cleverness for a safety net.

**More friction:**

- **Creating an accepted Decision with a quality target takes three calls** (`add_decision`,
  `set_decision_status`, `set_quality_target`). The status default is deliberately `proposed` and
  that reasoning is sound and documented — but `quality_target` has no such argument against it
  and could be a parameter on `add_decision`.
- **`constrains` stamped `basis: "estimated"` on a prohibition.** There is no contribution and no
  estimate involved in "these two streams must never connect", so a basis field on that edge is
  meaningless at best and slightly misleading at worst. Suggest omitting `basis` when no
  `contribution` is given.
- **The revision-safety note is slightly loud for a node created seconds earlier in the same
  session.** The warning's whole force is "no snapshot holds the prior value" — but when the prior
  value was written by this same session and is still in its transcript, the risk is much lower.
  A quieter form for same-session revisions would keep the loud one meaningful.

---

## Session 1, part 2 — the `brainstorm` skill

Two half-formed ideas (mushrooms as a decomposer loop; insects as fish feed and human protein)
recorded as exploratory Decisions rather than requirements.

### Good

- **The skill's central instruction is the right one and I would not have arrived at it alone:**
  *"Do not argue an idea down. If there is a real counter-argument, record it beside the idea —
  an idea killed in conversation loses its reasoning; an idea recorded with its objection keeps
  both."* That produced a materially better record than a conversation would have. The cockroach
  allergen objection and the "five living systems vs. the field-repairability quality target"
  objection are now attached to the ideas permanently instead of being said once and forgotten.
- **`kind: "exploratory"` and `related_to` must be passed in the SAME call, and the skill explains
  why** — a follow-up setter is two order-dependent calls, which is exactly the hazard a harness
  emitting parallel tool batches hits. That is a real failure mode designed around rather than
  documented around.
- **The relation vocabulary carried genuine engineering judgment.** `MITIGATES` let me record that
  the insect idea is not an addition to the design but a candidate *repair to a defect already
  written down* (the fish-feed closure hole recorded an hour earlier). `RISKS` let me record that
  both ideas spend the quality target the project chose. An idea that links to the defect it
  repairs is the design brain actually earning its keep.
- **The skill documents its own drift.** Step 3 says the instruction "used to read *this call is
  not optional: add_decision defaults to accepted*", which was true until 2026-07-25 and then
  silently became an instruction to make a redundant write — caught by two sessions, filed
  independently, and now rewritten as *"confirm the node came back proposed"*. A skill that names
  a tool's default has taken a dependency on that default; checking the actual return stays true.
  Very few instruction sets are honest about this.
- **The duplicate-guard message improved.** Compared with the earlier cross-type refusal, this one
  led with *"This is not a duplicate accusation. Saying the same thing twice in different words is
  sometimes real signal, which is why the second route exists and why nothing was merged for you."*
  That framing is much easier to act on correctly.
- **The honest-limits section is unusual and worth keeping.** It admits the graph has no
  `brainstorm` kind, that a proposed Decision is "close to an idea but not the same thing", and
  that a brainstormed idea can be quoted back as though settled — naming the mitigation (the
  "recorded as brainstorming" line) rather than pretending the risk away.

### Friction

- **The duplicate guard fought the skill's own rule.** The skill says plainly: *"Several unrelated
  ideas mean several Decisions. One node holding two unrelated questions can never be answered,
  only half-answered."* I followed that — and the guard then refused the second Decision as too
  close to the first, because both were phrased as "a decomposer organism as a waste path and a
  food source." The two questions have different answers, different costs and different risks, and
  merging them would have been wrong. **Suggestion: when one user message produces several
  exploratory Decisions in a single batch, don't cross-check them against each other** — the skill
  has already asserted they are separate questions, and the guard is re-litigating that.
- **There is no honest "reinforces" relation.** `CONTRADICTS` carries `alignment: supporting` for
  corroboration, but an edge that renders as `CONTRADICTS` when it means "supports" reads wrong to
  any human skimming edge types later. I dropped a true positive link (the mushroom loop reinforces
  the self-sustaining requirement by eating a waste stream nothing else eats) rather than write a
  misleading-looking edge. A first-class `SUPPORTS` / `REINFORCES` would have captured it.
- **Minor:** the skill says "do not run detect-and-ask over brainstormed nodes" but also "before
  moving on: `loop_status`". Both are right, but the near-adjacency of "don't detect" and "do check
  the loop" takes a re-read to separate.

### Addendum — promoting a brainstormed idea to intent

- **`EVOLVES_INTO` turned out to be exactly the right edge for a promotion**, and no instruction
  pointed at it. The brainstorm skill says "the brainstorm Decision stays, now with the road that
  was taken recorded on it" but doesn't say *how*; the obvious reading is to rewrite the decision's
  prose, which means re-sending a 2.5 KB field to append one sentence and risking transcription
  error. `EVOLVES_INTO` ("source is an earlier form that becomes target over time") records the
  same fact structurally, costs nothing, and is queryable. **Suggestion: name it in the skill's
  step 5** — "draw `EVOLVES_INTO` from the idea to what it became."
- **Related friction:** revising one field of a long node means re-sending that whole field. For a
  Decision whose `decision` text is several paragraphs, appending a line is disproportionately
  expensive and is the one operation where the revision-safety warning is most likely to be earned.
  An append mode, or field-level patch, would help.
- **`coverage: partial` earned its keep again.** Mushrooms genuinely do satisfy "produce food"
  while deliberately not counting toward the calorie budget, and being able to say *partial, and
  here is why* kept that honest rather than forcing a yes/no.
- **`provenance: planned`** was the right home for a requirement I derived rather than one Anthony
  stated (sealed, separately vented chambers for spores and allergen). `authored` would have been a
  forgery and `inferred` means "read back out of an existing system", which this isn't. Three of
  the six values are genuinely distinct and useful; worth knowing the middle one exists.

### Addendum — a silent failure worth reporting

**A malformed write succeeded and produced a wrong node with no complaint.** I fumbled an
`add_decision` call and put literal `</decision><parameter name="rationale">...` markup *inside*
the `decision` string. The tool accepted it: it created the node with a several-kilobyte
`decision` field containing that markup as prose, **no `rationale` property at all, and none of
the three relations drawn** — and returned a normal success payload. Nothing in the reply
suggested anything was wrong.

This is my error, not reflow2's, and I caught it by reading the response back — which is exactly
the habit the instructions argue for under *"a successful tool response is a claim, not a
result."* That section earned its place here.

But it is also a cheap thing to catch. **Suggestion: warn (do not reject) when a prose field
contains what looks like tool-call markup** — `</fieldname>`, `<parameter name=`, stray XML tags
matching the tool's own parameter names. A design node whose `rationale` is missing because it got
swallowed into `decision` reads as a node somebody chose not to justify, and no detector can tell
those apart. `unreviewed_ideas` would not have fired either, because the relations were lost in
the same stroke and the node genuinely had none.

Related, and the reason this is worth more than a shrug: the revision-safety note that fires on
every overwrite is excellent at protecting a field you *meant* to replace. It has nothing to say
about a field you never managed to write in the first place.

---

## Session 1, part 3 — change propagation, and a repeat of the silent failure

Anthony reopened an already-accepted decision (human waste to composting toilet) because a new
idea, biogas, competed for the same resource.

### The best moment reflow2 has had so far

**`propagate_change` paid back the whole session's link discipline, two turns later.** Reopening
one accepted decision returned **36 reachable nodes, of which only 4 were at distance 1** — the
biogas idea, the waste capability, the food capability, and the two-nutrient-paths requirement.
That was the actual work list, and it was correct: those four needed revising and the other 32
did not.

What makes this the strongest demonstration of the premise is *where the signal came from*. The
propagation was informative because of `RISKS` and `CONTRADICTS` edges I had drawn during
**brainstorming**, when they felt like bookkeeping. The path
`biogas CONTRADICTS humanure-decision → RISKS insects-feed-the-fish → cap:breed-insects` surfaced a
consequence — that a digester starves the insect colony the closed-loop claim now rests on — which
I would not have reliably re-derived by reading. `crosses_risk_edge` on each hop is what separates
"reachable" from "worth worrying about", and the summary-vs-`full` split kept a 36-node answer
readable.

Also good: **`set_decision_status` back to `proposed` is the reopen mechanism**, and it reads
exactly right — a settled choice becomes an open fork again without losing its history or its
original rationale.

### The silent malformed-write happened again — same class, different tool

I made the same mistake on `add_change_event` that I had made on `add_decision`: literal
`</summary><parameter name="rationale">` markup inside a string parameter. **Accepted silently
again**, producing an event whose `summary` contained the markup as prose and which had no
`rationale` at all.

**Two occurrences, two different tools, both silent, in one session.** That moves this from "my
slip" to "a cheap guard is missing." A one-line check for `</fieldname>` or `<parameter name=`
inside any prose field, warning rather than rejecting, would have caught both instantly. The cost
of not having it is a node that reads as deliberately unjustified.

### Friction, now with a repeat count

- **The cross-type duplicate refusal fired twice more** (`cap:manage-salt` vs
  `req:salt-does-not-accumulate`; `cap:cycle-air-to-the-beds` vs `req:air-loop-...`). **That is
  four occurrences in one session, all the same shape**: a requirement and the capability created
  moments later to deliver it. That pairing is the golden thread working exactly as the
  instructions describe, so the guard is firing hardest on the most canonical pattern reflow2 has.
  Ranking same-type near-matches above cross-type ones, or suppressing the warning when a
  `SATISFIES` edge between the pair follows in the same batch, would fix it without weakening the
  real duplicate check.
- **`register_alternative` requires a `location` naming a per-branch design export.** I had a
  genuine two-way fork with neither branch designed yet, and no honest path to point at. Inventing
  one would have put a file location in the graph that resolves for nobody — which `capture-intent`
  explicitly warns against. So I recorded the fork in prose instead and said so on the node.
  **Suggestion: allow an alternative with no location** (or an explicit "not yet designed" marker),
  because the moment a fork is worth registering is usually *before* either branch exists.

### And a detector that behaved well on something I deliberately left blank

I omitted `subject` on the ChangeEvent because neither `system` nor `record` obviously fit.
`change_axis_unstated` fired, explained why the distinction matters ("nothing can tell them apart
afterwards: only the person making the change knew"), and offered acknowledgement as a legitimate
exit. That is the right shape for a detector: it separates *nobody said* from *we decided it does
not matter here*, and it made me actually think it through rather than guess — the answer is
`system`, because Anthony's intent genuinely changed rather than our record of it catching up.

---

## Session 1, part 4 — the stop hook, and `detect_defects`

A session-end hook fired: *"2 graph write(s) this session and no loop check... Bookkeeping is not
the loop."* It was right. I had named the outstanding structural defects three times across the
session and never actually read them — exactly the failure the AGENTS.md loop section predicts
("a busy session that only ever adds nodes... *feels* like using reflow2 the whole time").
**The nudge worked, and it worked on the specific failure it was written for.**

### `detect_defects` is the best-instrumented call in the tool

Not because it found a lot, but because of how carefully it says **what it could not have found**:

- `swept.coverage_note` — "3 rule(s) had NOTHING TO EXAMINE and their silence means only that",
  naming them.
- `swept.rule_populations` — the denominator per rule. `circular_dependency` examined **1**
  dependency pair. A clean cycle check over one pair is not evidence of an acyclic design, and
  this is the only tool I have seen that volunteers that.
- `design_network_nodes: 52` against `nodes: 55`, with the gap explained rather than hidden — the
  topology rules walk a narrower graph that drops provenance types and `CONTAINS`.
- The `unthreaded_cluster` message says outright that "cut off here" is not "unreachable in the
  graph", because it does not follow every edge type.

This is the concrete form of the instructions' own warning that *"a successful tool response is a
claim, not a result."* Most tools state findings; this one states its own blind spots in the same
breath, unprompted.

**`suppressed_by_parked_idea: {contradiction: 2}`** is a genuinely subtle correct call. I drew
three `CONTRADICTS` edges this session. Two were *between brainstormed ideas* (biogas vs the
toilet decision, biodiesel vs biogas) and were correctly suppressed — a contradiction between two
things nobody has chosen is not a defect. The third involves a real requirement and was correctly
surfaced. Nothing told me that distinction existed; the tool just got it right.

**`repair_is_a_judgement` keyed by category and sent once**, with a note explaining that it used
to be over half the reply (3 paragraphs across 50 rows) and that nothing is withheld — read it as
`row.repair_is_a_judgement ?? repair_is_a_judgement[row.category]`. Good token discipline that
explains itself rather than silently truncating.

And it **refuses to auto-repair anything but duplicates**, saying why: "Connecting this cluster
would assert relationships nobody stated, which is worse than the finding." Correct, and it stopped
me reaching for the tidy fix.

### The findings were real

Both `unthreaded_cluster` warnings were **true edges I had simply failed to draw** — the satellite
link needs electricity (undeniable, and Starlink is a real continuous load on the power budget),
and transportability relates to the core-plus-site-kits split (the kits ship separately, which is
the whole point). Neither was a false positive. A rule that finds "these two nodes talk to nothing"
found two subsystems I had genuinely left floating.

### Friction

- **"8 further node(s) sit alone in this walk and are not reported by this rule"** is buried
  mid-sentence inside a prose `message`, repeated identically on both cluster findings. That is a
  *bigger* fact than either finding it is attached to — eight isolated nodes versus two clusters of
  two — and it deserves to be a field in `swept`, not a clause. As prose it is easy to skim past,
  and it is precisely the "what could this not see" information the rest of the reply is so good at
  surfacing structurally.
- **Two of the six defects are `Question` nodes flagged as orphans**, including one still awaiting
  the user's answer. Questions are created by `gap_to_prompt` as a side effect of the intended
  workflow, so following the documented path reliably manufactures `orphan_node` findings. They are
  `info` severity, which is right — but they dilute a six-item list down to four real items, and
  the fix ("park it against an accepted Decision") is disproportionate ceremony for a record the
  tool created itself.
- Similarly `epoch:genesis` is flagged as an orphan — created by the `genesis` tool, never attached
  to anything by any documented step. Three of six findings are artifacts of reflow2's own scaffolding.

---

## Session 1, part 5 — retiring something, and a fifth duplicate-guard hit

Anthony settled two decisions, and one of them knocked out a requirement accepted an hour
earlier. So this was the first time anything got *removed* from the design.

### Retirement is modelled properly, and it stopped me doing the tidy thing

`OBSOLETES` is the mechanism, and three details in its description are each load-bearing:

- **It is drawn from the accepted Decision that withdrew the thing, not from a successor.** The
  reasoning given: a retirement edge normally presumes a successor at the source end, and something
  discontinued with no replacement has nothing to put there — but it always has a decision behind
  it. That is a real modelling insight, not a convention.
- **Only an ACCEPTED Decision discontinues anything.** A `proposed` withdrawal has withdrawn
  nothing. This forced me to create the ruling, get it accepted, *then* draw the edge — three steps
  where deleting the node would have been one.
- **The target's `status` deliberately does not move**, "because that field records what was BUILT,
  and this edge records that it is gone."

And it worked end to end without my managing it: after the retirement, gap count stayed at 3. The
discontinued capability's `unrealized_capability` and `unmotivated_capability` findings fell silent
on their own, and `set_requirement_status: dropped` stopped the requirement raising
`unsatisfied_requirement`. **Nothing was deleted and nothing started nagging** — the rejected idea
keeps its full case, the dropped requirement keeps the problem statement that killed it, and the
withdrawn capability is still readable.

The alternative I would have reached for unaided — `delete_node` — is precisely what the
`orphan_node` repair note calls out: *"Deleting the node to clear this finding is the one repair
that looks clean and loses the most."*

### Fifth duplicate-guard hit, and the pattern is now unmistakable

Creating the withdrawal Decision was refused as too close to **the ChangeEvent and the Requirement
I had created moments earlier as part of the same operation.**

That is now five refusals in one session, and this one is the most instructive: recording a change
and recording the decision that authorises it is *the documented workflow*, and it necessarily
produces two nodes describing the same event in similar words. The guard's own text concedes this
("recording what shipped and recording what must stay true are different acts on the same work, and
they are supposed to read alike") — but it still costs a round trip every time.

**The five hits break into two shapes, both canonical reflow2 workflows:**
1. Requirement + the Capability created to satisfy it (4 times) — the golden thread.
2. ChangeEvent + the Decision authorising it (1 time) — the change loop.

Neither is a duplicate. A guard that fires hardest on the two patterns the instructions most want
you to follow is mis-tuned, not wrong. Suppressing it when the two nodes are joined in the same
batch by `SATISFIES` or `CHANGED`/`OBSOLETES` would keep the real check and remove every false
positive seen here.

### Change propagation, second run

Settling two decisions at once returned 46 impacted nodes across 4 distance bands, with an 8-node
direct ring — and the summary form (`counts_by_distance` + `direct_ring` + `risk_crossings`) was
the right default. I used `full: true` on the first propagation and did not need it on the second;
the risk-crossing list alone told me which of the 46 mattered.

---

## Session 1, part 6 — a good refusal, and a note on bulk-write atomicity

Two more brainstormed ideas (green materials; commodity materials).

- **`related_to` rejected an unknown field loudly**: I passed `basis` on a `CAUSES` relation —
  legitimate on the edge type itself, but not exposed through `related_to` — and got
  *"unknown field `basis`, expected one of `relation`, `other_type`, `other_id`, `evidence`,
  `incoming`"*. **This is the behaviour the malformed-markup case should have had**: refused, named,
  and told me the legal set. Same session, two extra fields in a prose payload, two opposite
  outcomes — strict deserialisation caught one, and free-text fields swallowed the other. The
  inconsistency is the finding, and it argues the fix belongs at the prose-field boundary.
- **`create_edges` all-or-nothing behaved exactly as advertised** when the failed decision meant two
  of four edges referenced a node that did not exist: *"nothing was written — 2 of the items failed
  and a bulk write is all or nothing. Every failure is listed so you can fix them together."* No
  partial graph, both failures named at once. Worth noting because the alternative — writing the two
  valid edges and reporting two errors — would have left the design half-linked in a way that reads
  as complete.
- **Cost of the strictness, and it is real**: the refusal meant re-sending a ~4 KB decision body
  verbatim, because nothing was created. Field-level validation before the write, or a dry-run,
  would make long-prose nodes cheaper to get right. This is the same underlying gap as the
  append-vs-replace friction noted earlier: reflow2 has no way to say "change this one thing" on a
  large node.
- **`unreviewed_ideas` staying silent was the confirmation** that both decisions' relations landed —
  a detector's *absence* used as a positive check, which only works because the detector is known to
  fire on exactly that condition. Worth recording as an argument for detectors having sharp,
  predictable trigger conditions rather than fuzzy ones.

### Promotion, part 2 — and Constraint finally earning its documentation

Three ideas promoted. All three landed as **Constraints, not Requirements**, and that routing was
non-obvious enough to be worth recording.

`capture-intent`'s table says a prohibition with no number in it — *"we must never…", "it is not
allowed to…"* — is a **Constraint, NOT a Requirement**, and warns that reading the type as
budgets-only once cost a design eleven constitutional prohibitions left as Requirements that
"report unsatisfied forever." That warning did real work here. My instinct was Requirement for all
three; on the table's advice they became:

- "no proprietary or single-vendor parts" — a prohibition
- "no high-emission materials in the sealed envelope" — a prohibition
- "prefer the lower-impact material, all else equal" — a tiebreaker rule, also not a goal

Had they been Requirements, all three would have raised `unsatisfied_requirement` permanently,
because nothing *satisfies* a prohibition — you either comply or you don't. Gap count stayed at 3
after all three landed, which is the confirmation the routing was right.

**The measured-failure framing is what made the advice usable.** "Constraint is not only for
budgets" as a bare statement would have washed over me. "Eleven prohibitions in a real design now
report unsatisfied forever, and it never produced an error — it produced a confident wrong
conclusion" is a specific enough consequence to change behaviour.

**Sixth duplicate-guard hit**, and it completes the taxonomy: the settled Decision was flagged
against the open idea Decision it resolves. That is a *third* canonical workflow shape, after
requirement-plus-capability and change-plus-decision — an open question and its answer are supposed
to coexist, and `EVOLVES_INTO` between them is precisely how the brainstorm skill says to record a
promotion. **All six false positives are pairs the tool's own workflows create by design.**

---

## Session 1, part 7 — a scope narrowing, and edges as living records

Anthony narrowed the climate range from desert-to-arctic to lower-48 city climates, and chose to
buy the container shell rather than fabricate one.

### The best single moment of the session

`propagate_change` on the narrowing returned 53 impacted nodes with an 8-node direct ring — and
**five of those eight were `RISKS` edges pointing at the changed requirement.** Narrowing the
climate range did not merely alter one node; it weakened or retired five separately-recorded
objections at once, and the propagation showed me exactly which five without my remembering any of
them.

Those five objections were written across three earlier conversations, hours apart, each as an
aside during a brainstorm. **No human and no unaided model would have reliably recalled that
"commodity materials are specified for ordinary climates" was an objection now answered by a change
to the climate.** That is the design brain doing the thing it exists for.

### Edge properties are a record that goes stale, and nothing says so

This is the most substantive new finding.

Those five `RISKS` edges carried `evidence` text written when the range was desert-to-arctic —
"in the arctic, a tank of water at fish temperature is a large continuous heat load", and so on.
After the narrowing, that evidence is **wrong**, and a reader following the edge would be misled by
prose that reads as current.

Nothing in reflow2 flags this. The node-level story is excellent — `add_*` warns loudly on every
overwrite, `reconcile_artifacts` catches drift between design and files, `change_axis_unstated`
catches an unstated axis. But **edge `evidence` and `note` fields are prose written at a moment in
time, they are exactly as perishable as node text, and no detector reads them.** I updated four of
the five by hand only because the propagation happened to list them and I happened to remember what
they said.

Two suggestions, in order of cheapness:
1. **When `propagate_change`'s direct ring includes an edge whose `evidence` mentions terms that no
   longer appear in the changed node, say so.** Even a crude string check would have caught
   "arctic" surviving in four edges after "arctic" left the requirement.
2. More generally, a `stale_edge_evidence` detector, or simply a `written_at` stamp on edge prose so
   a reader can see the evidence predates the node it describes.

**Re-writing an edge with `create_edges` upserts its properties cleanly**, which made the repair
easy once identified — the problem is purely one of noticing.

### Eighth duplicate-guard hit

Same change-event-versus-decision shape as before. The count is now eight, across three canonical
workflow pairs. I have stopped treating each as noteworthy and started passing `distinct_from`
pre-emptively, which is itself the concerning outcome: **a guard that is routinely pre-empted has
stopped being a check.**

---

## Session 1, part 8 — eleven decisions at once, and a budget that says no

Anthony accepted every recommendation from a walkthrough of the open items. Roughly 60 writes:
7 new decisions, 2 new requirements, 3 new constraints, 1 new capability, 8 revisions, 40 edges.

### `budget_report` is the sharpest tool in the set

The floor-area constraint was set up as a real numeric budget — `quantity: floor_area_sqft`,
`limit: 250`, `direction: maximum` — with each capability's footprint attached as a `contribution`
on its `CONSTRAINS` edge. The report came back:

```
total: 297, limit: 250, verdict: "exceeded", unstated: [], basis_coverage: {estimated: 10}
```

**A one-word verdict on whether the design physically fits.** Before this, "it's going to be tight"
was a sentence in a conversation that would have evaporated. Now it is a computed fact attached to
the ten subsystems that cause it, each with a note saying what its number covers, and it will
re-compute every time any of them changes.

Three details that make it trustworthy rather than merely tidy:
- **`unstated: []`** — it lists contributors with no number rather than treating them as zero, and
  it says the verdict is `incomplete` when any are missing. An empty list here is what licenses
  reading `297` as a real total.
- **`basis_coverage: {estimated: 10}`** — every number is flagged as my estimate, not measurement.
  Nobody reading this later can mistake it for survey data.
- **`path_note`** — it volunteered that the contributors form no dependency chain, so the simple
  total is the only meaningful rollup. It explained why it did not compute the other thing it can
  compute.

This is the first tool in the session that produced a *conclusion Anthony has to act on* rather
than a record of a conclusion he reached. Worth noting how cheap it was: ten edges with a number on
each.

### The design is now large enough that the loop is doing real work

Two questions asked hours ago were still open and `open_questions` had them verbatim, with the
exact wording Anthony saw — so answering them was a lookup rather than a reconstruction. Both
closed cleanly with `answer_question`.

### One reversal, handled the way the tooling wanted

The insect species recommendation reversed a decision accepted earlier the same day. Recording it
as a `ChangeEvent` first, with the reasoning chain that had gone wrong, then a new accepted
`Decision`, then revisions to the four nodes that carried the old assumption — that sequence is
what the AGENTS.md loop prescribes and it worked without friction. The design now contains both the
reasoning that led to black soldier fly and the reasoning that overturned it, which is the thing a
conversation would have lost.

---

## Session 1, part 9 — `register_alternative`, and taking back an earlier complaint

Three permitting options recorded as real alternatives under the open decision, plus an
egress requirement.

### I was wrong about `register_alternative`, and the reason is worth recording

Earlier I complained that `register_alternative` demands a `location` naming a per-branch design
export, and I declined to use it for the composting-toilet-versus-digester fork on the grounds that
neither branch had been designed and inventing a path would put a file location in the graph that
resolves for nobody.

That complaint stands as written but the conclusion was too quick. **The correct move was to
actually write the branch documents.** Three short memos — one per permitting option, a page each —
took a single Bash call, and then `register_alternative` worked exactly as designed: three
Artifacts, each `GOVERNED_BY` the Decision, each `CONTRADICTS` its siblings, all pointing at files
that genuinely exist and that Anthony can read.

The requirement I called friction is doing real work: **it refuses to let a "fork" exist as three
bullet points in a prose field.** If the options are not worth writing down separately, they are
not yet alternatives — they are a sentence. Forcing the document into existence is the point, and
the design gained its first three artifacts as a side effect.

**Revised suggestion, much narrower than the original:** say this in the tool description. "If no
branch document exists yet, write one — that is the work this call is asking for" would have got me
there two hours earlier. The current wording reads as a schema requirement rather than as a
deliberate forcing function.

### The tension the design surfaced on its own

Recording the egress requirement produced two `RISKS` edges I did not go looking for. An escape
opening is a large cut in the corrugated wall requiring welded reinforcement — which collides with
the minimise-cuts constraint AND with the buy-the-shell-build-the-inside decision, whose entire
purpose was to keep Anthony off welding. **It is the one place the bought-shell line has to be
crossed**, and neither Anthony nor I had noticed until the requirement was written down next to
the constraint.

Worth naming what did the work here: nothing detected this. Writing the requirement in the same
vocabulary as the constraint, and then asking what it touches, is what surfaced it. The graph made
the question askable; it did not answer it.
