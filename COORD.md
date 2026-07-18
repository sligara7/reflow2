# COORD.md — who's working on what

The claim board for reflow2. Two people, two agents (Claude Code and grok build), one repo,
coordinating through git.

Its only job is **avoiding collisions**: knowing what someone else already has in hand, so two
people don't build the same thing or edit the same module at once. It is not a plan, a spec, or
a status report —

| Question | Where |
|---|---|
| Who has what right now | **this file** |
| What the work *is*, and why | [docs/backlog.md](docs/backlog.md) |
| Is the code meeting the docs | [docs/requirements-coverage.md](docs/requirements-coverage.md) |
| What changed and when | [CHANGELOG.md](CHANGELOG.md) |

## For agents reading this

**Before starting work:** read the In-progress list. If someone holds the item, or holds
something that touches the same files, pick something else or say so — don't quietly work in
parallel.

**When you start:** add one line under **In progress** with the handle, the date, and roughly
what you're touching. Commit that line *before* the work, not after — a claim nobody can see is
not a claim.

**When you finish:** move the line to **Recently finished** with the commit, and push.

**Keep it one line per item.** Two agents editing this file will eventually collide in git; a
line-per-item makes the conflict trivial to resolve, a table or a paragraph does not.

**Don't restate the backlog here.** Reference the id (`BL-4`) and let
[docs/backlog.md](docs/backlog.md) carry the description, so there's one source of truth for what
the work actually is.

## Handles

| Handle | Who | Agent | Usually working on |
|---|---|---|---|
| `@ajs` | Anthony | Claude Code | core, schema, detectors |
| `@bro` | (brother) | grok build | using it for real; consumer kit feedback |

Add yourself if you're new here.

## In progress

*Format: `- BL-n or short title — @handle — since YYYY-MM-DD — files/areas touched`*

- _(nothing claimed)_

## Blocked / waiting

- _(nothing)_

## Recently finished

Trimmed periodically; the durable history is [CHANGELOG.md](CHANGELOG.md) and `git log`.

- Answer the first external user (where-am-i skill, pause/resume, setup kickoff) — @ajs — 2026-07-18 — `ed818ae`
- Records: CHANGELOG, backlog, trials — @ajs — 2026-07-18 — `ed818ae`
- Gap review + cross-process determinism fix — @ajs — 2026-07-18 — `8a66afb`, `565c418`
- Selective `unexpected_coupling`; provenance out of topology — @ajs — 2026-07-18 — `824a6cc`
- Write side for the types DETECT asks about (WS-1..3) — @ajs — 2026-07-18 — `e722766`
- Reflow audit: all workflows and tools, with verdicts — @ajs — 2026-07-18 — `7179218`
- Interface layer, cycle detection, as-built drift — @ajs — 2026-07-18 — `7f168a5`

## Stale claims

If an item has been claimed for **more than a week with no commits against it**, anyone may take
it — leave a note on the line saying you did, rather than deleting the original claim.

## Conventions

- **Branches:** `feat/<short-name>` off `main`, one per claimed item where practical.
- **A change is done** when `cargo test --no-default-features`, `cargo clippy
  --no-default-features --all-targets` and `cargo fmt --check` are clean, and
  `python3 tools/validate_schema.py` prints OK after any schema edit — see
  [CLAUDE.md](CLAUDE.md).
- **Update the records in the same change**, not afterwards: coverage matrix when a status moves,
  CHANGELOG when a user would notice, backlog when an item is finished or discovered.
- **Findings from real use** (a trial, a session that went wrong) go in
  [docs/trials/](docs/trials/) verbatim, and get an item in the backlog if they need work.
