# reflow2 feedback — ophyd-service sessions, 2026-08-18/19

Written by the agent (Claude Code) after a heavy two-day stretch: a brainstorm →
promotion → build arc (registry seeding via instantiation capture), a relayed
third-party design concern captured and resolved (unified-mode http fault
isolation), a dependency upgrade, a drift catch (README vs graph on the 0MQ
contract), two capture-session passes, and a full check-health pass that ended
at zero defects. Everything below is grounded in that usage; node ids are
included so the graph itself can be inspected.

---

## What worked well

**1. The proposed-until-the-user's-word discipline held up in real use.**
`add_decision` landing at `proposed` and every promotion requiring
`set_requirement_status` / `set_decision_status` made the brainstorm → intent
flow feel *safe*: Dmitri's relayed happi doubt sat as an open decision with its
counter-arguments beside it for hours of conversation, and nothing claimed it
was settled until the user said "promote". The forgery-prevention rationale
(`dec:certainty-derived`) is not theoretical — it shaped behavior.

**2. The dedup guard's "TWO WAYS ON" refusal is the right shape.**
When `add_requirement` refused `req:instantiation-capture` because
`dec:worker-instantiation-capture` existed, the refusal named the near-match and
offered sharpen-vs-distinct_from. Nothing was lost, the choice was deliberate,
and the message taught the model the vocabulary. Compare with tools that
silently create near-duplicates: this is much better.

**3. `acknowledge_defect` with shape-hash expiry is exactly right.**
Accepting the config-service ↔ qs-manager contract cycle as "phase-separated by
design" felt honest *because* the acceptance expires if the coupling's shape
changes. An acceptance that could go stale silently would have been a trap;
this one self-invalidates.

**4. `repair_is_a_judgement` — the tool refusing to invent fixes.**
Every orphan/cluster finding in check-health said plainly "linking this would
assert a relationship nobody drew." That stopped me (the agent) from
manufacturing connectivity to quiet warnings, which is exactly the failure mode
an eager agent has. The absence of a suggested fix carrying meaning is a
genuinely good design idea.

**5. `would_destroy` / `requires_human_review` honesty in propose_heal.**
The proposal for 8 defects contained zero operations and said so with "this
zero is not a pass." No false reassurance anywhere in that surface.

**6. The drift catch paid for the whole discipline.**
The graph holding `dec:keep-zmq` (accepted) + `dec:http-only-pivot` (rejected)
while the shipped README said the opposite was caught only because a session
happened to read both. The graph being the durable record of the *decision*
while docs drifted is the core value proposition, demonstrated.

**7. `loop_status` + the stop-hook nudge caught a real miss.**
Three ChangeEvents had been recorded without `propagate_change` — pure
bookkeeping, no blast radius computed. The nudge fired once, named the exact
shape ("impact-check owed"), and the propagation then produced useful output
(boundary crossings, verifying-ring confirmation). Good loop closure.

**8. Served skills stayed current.** No stale in-repo skill text; the
where-am-i / brainstorm / capture-session / detect-and-ask / check-health
progression covered the session's needs without improvisation.

---

## Friction (concrete, reproducible)

**1. `add_decision` upsert silently resets status to `proposed`.** ⭐ worst one
Twice (`dec:registry-seed-format`, `dec:unified-http-fault-isolation`) I
updated an *accepted* decision's text to append a RESOLVED paragraph, and the
node silently dropped back to `proposed`; I had to notice from the reply and
re-call `set_decision_status`. This contradicts the BL-183 promise ("every
field you do NOT pass keeps its current value") — status is a field I did not
pass, and it did not survive. Either preserve status on upsert of an existing
node, or have the `revision` block shout "status reset proposed" so it can't be
missed. An agent that doesn't re-read the reply carefully leaves accepted
decisions demoted.

**2. Evolving a long decision text requires resending all of it.**
`decision` is a required constructor field, so appending one resolution
paragraph to a 2,000-word decision means resending the whole text, with the
revision block warning that the prior text's only copy is in the reply. That
makes rich decision records (which the skills rightly encourage) *risky to
evolve* and token-expensive. An `append_to_decision` / annotate operation —
or first-class "resolution" field separate from the brainstorm body — would
remove the riskiest write pattern in my whole session.

**3. `add_change_event` has no description field.**
A defect_fix's *lesson* (why it happened, what guards it) doesn't fit in a
name. I had to make a second `create_node` call to attach `description` as an
undeclared property (`chg:ws-dep-regression-2026-08-18`). The undeclared-prop
escape hatch worked — credit for that — but the constructor should take the
field the skills tell you to write.

**4. Review/ack records swamp Decision listings.**
`scan_nodes(Decision)` returns 61 nodes of which ~40 are `decision:ack:*`
review records. `what_next` excludes them (good), but where-am-i's "read the
Decisions back" step and any manual scan must hand-filter. A `kind` filter or
default exclusion with an `include_review_records` flag would make the primary
narration call cheaper and cleaner.

**5. The brainstorm→requirement promotion path always trips the dedup guard.**
Promoting a proposed Decision into a Requirement (the *designed* flow of the
brainstorm skill, step 4) reliably fires the near-duplicate refusal, then the
capability for that requirement fires it again against the requirement just
created. Four `distinct_from` boilerplate roundtrips per promotion. Since
promotion is a recognized workflow, a `promotes: <decision-id>` parameter on
add_requirement (drawing a real edge, skipping the guard, and giving the
decision its "road taken" link for free) would turn the guard from friction
into structure.

**6. Loop-debt classes lack an acknowledge path.**
"1 capability claims realized with no passing check" (`cap:web-ui`) has sat on
`loop_status.next` for the entire arc. It's *correct* — the frontend is another
team's territory with no tests we control — but unlike gaps and defects there
is no acknowledge for this class, so the loop can never read clean while the
state is deliberate. Possibly `governed_by ruling:parks` is meant to cover it,
but neither the tool docs nor the skills say whether `parks` applies to the
unproven-capability class, and I wasn't confident enough to try. Either extend
`parks` explicitly or add an acknowledge for loop-debt items.

**7. Three tools to see all debt.**
`detect_gaps` (meaning), `detect_defects` (shape), and `loop_status` (loop
classes) each answer differently; early in the session graph_report said
"0 open gaps, 6 structural defects" and it took all three calls to hold the
full picture. `loop_status` is the right aggregator — it just reports counts,
not content. A `loop_status(verbose)` that inlines the top item per class
would often save the follow-up calls.

**8. Token weight of the serve surface.**
`get_skill` payloads are large and consumed per invocation; `propagate_change`
direct-ring dumps are heavy relative to the part that carried value
(boundary_crossings, risk_crossings, counts). Digest variants would help long
sessions: skills at ~1/3 length with a "full text on request" escape, and a
`propagate_change(summary_only=true)`.

**9. Minor: `create_node`-vs-`add_*` upsert asymmetry has to be learned.**
The memory gotcha "add_* upsert drops off-schema props; update via create_node
full-props" (issue #71 era) still shapes agent behavior even though the
constructors now merge. If that's fully fixed, saying so loudly in the
constructor docs would let old scar tissue heal; if it's partially fixed, the
remaining cases should be named.

---

## Ideas

- **`resolve_decision(id, resolution_text, status)`** — the single operation
  that would have removed friction #1 and #2 at once: append a resolution,
  preserve the body, set the status, one call, user's word recorded.
- **Promotion edges as first-class** (friction #5): brainstorm → requirement →
  capability is the golden path of the whole capture philosophy; making it a
  typed act would also let where-am-i narrate "this requirement came from that
  brainstorm" without prose archaeology.
- **A `demonstration` quick-record**: "X was seen working live on <date> in
  <env>" is the most common evidence at a beamline and currently takes
  add_verification + verifies + set_verification_status (3 loads + 3 calls).
  One call would raise the capture rate for exactly the evidence class
  (`evidence_report`'s field-vs-simulation story) reflow2 cares most about.
- **Session-scoped write journal**: capture-session would be easier to run
  honestly if the server could answer "what did THIS session write?" —
  `since_export` approximates it but is repo-commit-based, not session-based.

## Overall

The loop's philosophy — capture at the moment of decision, user's word for
every promotion, refusals over silent fixes, acceptances that expire — proved
itself repeatedly over these two days: it caught a real doc/design drift, kept
a relayed opinion attributed for weeks-later traceability, and got the graph to
zero structural defects without deleting anything. The friction list is almost
entirely about *write ergonomics on long-lived nodes*, not about the model.
Fix #1 (status reset on upsert) first; it's the only one that can silently
damage the record.
