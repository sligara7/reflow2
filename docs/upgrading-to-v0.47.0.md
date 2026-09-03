# Upgrading to v0.47.0

**One `loop_status` field was renamed, and the rename fails SILENTLY. Everything
else in this release is additive.**

## The rename

| Before | After |
| --- | --- |
| `loop_status.unwritten_answers` | **`loop_status.answered_with_open_gap`** |

**What breaks, and why it is worse than an error:** a script reading the old key
gets `None`/`undefined` back. No exception, no warning — the count simply
disappears from whatever you compute. A dashboard shows zero. A nudge stops
mentioning a debt class. **A missing key and a genuinely clean design look
identical**, which is the failure this project spends most of its effort trying
to prevent, so it is called out here rather than left to be discovered.

**What to do:** rename the key wherever you read it. If you use reflow2 only
through an agent and have never read that field by name, **you have nothing to
do** — the agent reads the surface at connect time.

`tools/graph_probe.py` and `tools/loop_nudge.py` ship updated in this release; if
you have copies of either, take the new ones.

## Why the name moved

The old name asserted something the code could not know. `loop_status` counts
questions whose status is `answered` and whose gap is still open — that is all it
computes. It never looks for the answer among the design's nodes. But the field
was called `unwritten_answers` and the message read *"N answered question(s)
never reached the design"*, which is a claim about design nodes that nothing
checks.

The commonest way to land in that count is a **deferral the genesis skill itself
prescribes**: the user says "not sure", you record it as an open Decision, and
the gap stays honestly open. Doing exactly the right thing was reported as debt,
and both remedies the message suggested were wrong for that case.

Three separate projects diagnosed this from scratch before it was fixed.

## What is new, and needs nothing from you

**The stamp moved: 64 → 65 edge types.** A consumer pinning a build can observe
that, which is why this note exists at all.

- **`ANSWERS` (`* → Question`)** — a design record can name the Question it
  answered. `loop_status` reports `answered_naming_their_record` alongside the
  count above.
  🛑 **Its absence means "nobody said", never "not written in."** Every design
  written before this release has answered Questions and no `ANSWERS` edges — 52
  in reflow2's own — so no report treats a missing edge as debt. **You are not
  expected to backfill them**, and nothing will nag you to.

- **`deliberately_open` on edge types** — an optional schema *property*, so it
  does not move the stamp. `describe_schema` now ranks an edge that DECLARES it
  is for your pair above ones that merely accept it, and `modelled_open_matches`
  plus a rewritten `note` let you tell *"nothing here is for this pair, ask for a
  new edge type"* from *"the answer is below, declared"*. Those two used to
  render identically.

- **`swept.expected_at_this_phase`** on `detect_defects` — at genesis, with no
  Components declared, `unthreaded_cluster` findings are listed here instead of
  being filed as defects. **Counted and visible, never silenced.** If you run
  `detect_defects` on a brand-new design you should see fewer defects and a
  populated bucket; that is the fix, not a regression.

## One behaviour change worth knowing

`gap_to_prompt` / `gaps_to_prompts` now resolve the replayed gap **by `id`
alone** — the text you send back is an echo and is not read. Mangling or
trimming it can no longer silently re-key your answers, which two projects hit
ten days apart.

**The consequence:** a gap that is no longer open — including one you have
**acknowledged** — is now REFUSED rather than served from your stale copy. That
is deliberate: `acknowledge_gap` exists so a settled question is not asked again,
and phrasing one from a copy the server no longer recognises is exactly that
re-ask. The refusal says which of the two happened.

## Nothing else

No node type was added or removed. No property became required. No existing edge
changed what it accepts. An existing graph opens and reads exactly as before.
