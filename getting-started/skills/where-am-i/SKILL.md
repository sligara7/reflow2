---
name: where-am-i
description: Use when the user asks where things stand, what you've concluded, what's been decided, or wants to pick up an existing design after a break — and at the start of any session on a graph that already has a Project. Reads the design graph and tells them, in their own words, what the design now says and what's still open.
---

# Tell the user where the design stands

The user cannot see the graph. Everything you have recorded — every requirement, decision and
open question — is invisible to them unless you say it. When they ask *"what are your
conclusions?"* or *"where are we?"*, they are asking you to read the graph back to them.

Do this at the **start of any session** on an existing design, and any time they ask.

## Gather

- `graph_report_markdown` — snapshot, top gaps, allocation health.
- `scan_nodes` for `Decision` — what has actually been settled, and why. **This is the part they
  most want and the report does not include it.**
- `detect_gaps` — what still needs their input.
- `reviewed_gaps` — what was raised and consciously accepted.
- `scan_nodes` for `Requirement` / `Component` / `Interface` — the shape of the design.

## Tell them

Write it as prose for the person who described this project to you, not as a data dump. Aim for
something they could read in under a minute:

1. **What they're building** — one line, from the Project and its objective.
2. **What's settled** — the Decisions, in plain language, with the *reasoning*, not the ids.
   "You decided the outdoor unit sends cumulative totals rather than deltas, so a lost reading
   heals itself." This is the answer to "what are your conclusions".
3. **The shape so far** — how many requirements, what the main parts are and how they connect.
   Name the parts, don't list node ids.
4. **What you already asked them** — call `open_questions` first. These were put to them in an
   earlier session, in the wording they saw. Repeat that wording rather than inventing a new
   phrasing for the same thing: being asked the same question twice, worded differently, is how
   someone learns the tool is not listening.
   - `status: asked` — still waiting on them. Ask again *only* as a follow-up, not as if new.
   - `status: answered` — they already told you, and it is still open. Say what they said back to
     them and what it implies, rather than re-opening the question. Usually it means their answer
     never got written into the design, or the gap should be acknowledged.
5. **What's still open** — the *remaining* gaps that need them, phrased as the questions they are.
   Say how many there are and lead with the ones that actually block progress.
6. **Where to go next** — the one or two things worth doing now, and offer the choice rather than
   assuming: keep filling in the design, or start building.

## Keep it honest

- **Never paste raw ids at the user.** `cmp:reading-store` means nothing to them; "the reading
  store" does. Ids belong in your tool calls, not your prose.
- **Don't imply more certainty than the graph holds.** A Requirement recorded from an assumption
  you made is not the same as one the user confirmed — say which is which.
- **Don't hide the open questions to make the summary tidy.** The gaps are the value.
- **If nothing has changed since last time, say so plainly.** A short honest answer beats a
  padded one.

If the graph is empty or has no Project, this is a new design — use the **genesis** skill
instead, and ask the user for a short overview of what they want to build.
