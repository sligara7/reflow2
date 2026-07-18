---
name: report-friction
description: Use when reflow2 itself gets in your way while you are designing — a tool that fails without saying why, a gap that fires on correct work, something you cannot record, a rejection you cannot act on. Writes a report the maintainer can act on, redacted of the user's design content, and offers to file it. Not for problems with the project you are designing; only for reflow2 itself.
---

# Report friction with reflow2

Everything reflow2 knows about its own weak points came from someone writing down what fought
them. Those were staged trials; ordinary use produces better evidence and currently loses all of
it. If reflow2 obstructed you, that is worth ten minutes of the maintainer's time — and you are
the only one who saw it.

## When to use this

Reach for it when **reflow2** is the problem:

- a tool fails and the error does not tell you what would work
- a gap fires on something you did correctly, or the same gap keeps returning
- you cannot record something the design clearly contains
- a tool exists but nothing told you it did, or it does not do what its description says
- you had to work around reflow2 to get on with the design

**Not** for problems with the thing you are designing. A missing requirement is a gap; a
detector that cannot express your requirement is friction.

Do not stop the user's work to do this. Note it, keep going, and raise it at a natural break.

## Redact first — this is the part that matters

**A friction report naturally quotes the graph, and the graph is the user's design.** Requirement
text, component names, the brief, sometimes a commercial or restricted project. That must not
leave their machine because you found a bug.

Report **reflow2-shaped facts only**:

| Include | Instead of |
|---|---|
| which tool, and the argument *shapes* | the argument values |
| the node **types** involved | node ids, names, or statements |
| what you expected and what happened | the design that produced it |
| counts and structure — "22 artifacts under one capability" | what those artifacts are |
| the error text, with ids masked | the raw error |

Rewrite ids as placeholders — `cap:X`, `req:Y`. If you cannot describe the problem without the
user's content, **ask them** before including it, and say exactly what you would be sending.

If a minimal reproduction needs a graph, build one from invented nodes. A bug that only shows up
on their real design is worth saying so about — but describe the *shape*, not the content.

## Write the report

Keep it to what a maintainer can act on:

```markdown
## What I was doing
One or two sentences. The design step, not the design.

## What I expected
## What happened
Exact error text, ids masked.

## Minimal shape that reproduces it
Node types and counts, or invented nodes. No user content.

## Why it matters
What it cost — a workaround, a wrong edge, twenty minutes of guessing.

## Environment
reflow2 <version> (<commit>) from .reflow2/kit-version.json
<agent/harness>, <OS>
```

Say if you are unsure it is a bug. "I could not work out how to do X" is a **documentation** gap
and worth reporting as one — label it that way rather than filing it as a defect.

## Then ask, and file only if they say yes

**Never file without asking.** An issue is a public action taken under the user's identity, in a
repository they do not control. Show them the text you would send, and let them read it.

Search first — `gh issue list --repo sligara7/reflow2 --search "<keywords>"`. Several people
hitting one problem should thicken one report, not open five. If it exists, add what is new about
your case and nothing else.

Then, if they agree:

```bash
gh issue create --repo sligara7/reflow2 --title "<one line>" --body-file <report>
```

**If that fails — no `gh`, not authenticated, or no access to the repository — that is expected,
not an error.** reflow2's repo is not public. Save the report in the project as
`reflow2-friction-<date>.md`, tell the user where it is and that they can send it on, and carry
on with their work. A written-down finding on their disk is still worth more than one nobody
recorded.

## What not to do

- Do not file automatically, or in bulk, or for something you have not actually hit.
- Do not send the user's design content to make a point.
- Do not report the same friction twice in one session because it happened twice.
- Do not editorialise about the maintainer's choices. Say what happened, what it cost, and what
  you expected. They will draw their own conclusions — the trial reports that shaped this project
  worked precisely because they described friction rather than prescribing fixes.
