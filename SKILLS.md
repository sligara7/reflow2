# The skills — which one, when

Fifteen skills and eight slash commands. A **skill** is a prose workflow an agent loads by itself
when your situation matches its description, so the loop happens without you naming tools; a
**slash command** is you asking for one on purpose.

**Skills are served, not installed** (`dec:skills-served`, 2026-07-25). The source is
[getting-started/skills/](getting-started/skills/); `build.rs` compiles it into the reflow2 binary,
which serves it through **`list_skills`** and **`get_skill`** with a one-line catalogue in the
handshake instructions. So a consumer project holds no skill files, the served set always matches
the running server, and upgrading reflow2 changes nothing in your repo
([docs/upgrading-to-v0.12.0.md](docs/upgrading-to-v0.12.0.md)).

The cost, since it is real: a skill file on disk is auto-matched by your harness from its
description, and a served one is not — the agent reads the catalogue and asks. A project that wants
harness-native discovery for a particular skill can keep its own copy in `.claude/skills/`, which
takes precedence over the served one.

This repository still keeps `.claude/skills/` and `.grok/skills/` copies for working on reflow2
itself; `tools/skill_lint.py` holds all three byte-identical, and
`crates/reflow2-mcp/tests/served_skills.rs` holds the served set identical to the source. The slash
commands live in `.claude/commands/` and are **not** installed into a consumer project — today they
are a convenience for working on reflow2 itself.

## The loop, in the order you hit it

| Skill | When |
|---|---|
| **genesis** | Very start, or the graph is empty. Bootstraps from your opening brief. |
| **brainstorm** | You are thinking out loud, not deciding. Records ideas *as* ideas — open questions, in your words, with the counter-arguments — and asks at the end which ones you want to keep as intent. |
| **capture-intent** | You share a new idea, feature or requirement. Turns your words into nodes and wires the golden thread. |
| **kpp-proposal** | Something you said sounds like it *must* hold no matter what. Asks you whether it is inviolable — a key performance parameter — rather than deciding for you, and records either answer. |
| **detect-and-ask** | Before building, and after capturing. Finds gaps and puts them to you as plain questions. |
| **impact-check** | Before changing or removing anything. Shows the blast radius so you edit only what is affected. |
| **link-artifacts** | Right after you create or change a real file. Registers it with a checksum so drift is detectable. |
| **check-health** | After structural changes. Cycles, single points of failure, duplicates, islands — how the design is *shaped*. |
| **parallel-work** | Two people on one design. Claims the region, isolates the work in a worktree with its own graph, and merges the design semantically instead of by lines. |
| **capture-session** | At any natural break, and before a long session ends. Writes down the reasoning that exists *only* in the conversation — what was tried and abandoned, what got measured, why the losing option lost — and routes each kind to the node type that already fits it. Not a transcript, and not for new intent. |

## Changing what is already there

| Skill | When |
|---|---|
| **revise-design** | You changed your mind about something in the design. Walks the change onto the record before making the edit. |
| **retire-from-design** | Something should leave. Forces the question that matters — *was it ever true?* — because history gets retired and mistakes get deleted. |

## Getting oriented

| Skill | When |
|---|---|
| **where-am-i** | Where things stand, what has been decided and why. Run it at the start of any session on an existing design. |
| **adopt** | Pointed at a system that already exists with little documentation. The sibling of genesis. |

## Setup and feedback

| Skill | When |
|---|---|
| **ci-gate** | Wire the design check into CI so unaccepted drift turns the build red. |
| **report-friction** | reflow2 itself got in your way. Not for problems with the project you are designing. |

## Slash commands

| Command | What it does |
|---|---|
| `/where` | Runs **where-am-i** — the design read back to you in plain language. |
| `/gaps` | Walks the open gaps and asks you about them. |
| `/health` | Structural health: cycles, single points of failure, islands. |
| `/decisions` | What has been decided, and the reasoning behind it. |
| `/debt` | What the coherence loop is owed right now. |
| `/req` | Captures a requirement in your own words. |
| `/kpp` | Records something as inviolable intent, after checking you meant it that way. |
| `/brainstorm` | Thinks an idea through with you and records it as an idea, not a commitment. |

## Two worth knowing better than the rest

**impact-check** is the one that is easy to skip and expensive to skip. It is the whole difference
between finding out what a change touches *before* you make it and after.

**report-friction** is the most underused. Friction with reflow2 tends to surface because someone
asked a good question in the moment, not because anything captured it — and a friction report is
how the next session does not need you to notice again.

## What is not here yet

- **"What should I work on next?"** — modelled as `cap:what-next` and unbuilt. The design can
  answer it; no skill asks it yet.
- **"I inherited this — where does a new feature go?"** — `cap:onboarding`, also unbuilt, and
  arguably reflow2's strongest demo when it lands.
- **The slash commands are not in the kit.** A consumer project gets the skills and no commands.
- **Live collaboration.** **parallel-work** is parallel work with good merges, not simultaneous
  editing: the graph store is single-writer, and a claim is advisory rather than a lock.
- **A `brainstorm` kind.** Ideas are recorded as open questions at `proposed`, which is silent and
  works, but a person scanning the decision list cannot tell a musing from a choice without
  reading it. Open in the design, not in this list.

This list is what exists today, not what is designed. The graph is the record of the difference.
