# What a working 15-session fleet knows that reflow2 doesn't

> Part of the **Reflow 2.0** design docs — see **[overview.md](overview.md)** for the full map and
> reading order.

**Source:** the StoryFlow fleet's own protocol files — `P.md` (worker pool), `COORD.md` (boss
charter), `S.md` (boss cold-start), `B.md`/`Q.md`/`A.md`/`SB.md` (the buses) — plus the field
`feedback.md` written by three of its sessions on 2026-07-25. Anthony's deployment, developed and
tuned over months of real concurrent work, offered to reflow2 with the instruction *not* to copy it
verbatim: **take what transfers.**

This document is that extraction. It is deliberately short on admiration and long on the one
question that matters: *what does reflow2 do differently on Monday?*

Read it alongside [`docs/github-mcp-nuggets.md`](github-mcp-nuggets.md), which asked the same
question of a service with millions of users. The two answers converge more than they diverge, which
is itself worth noticing: the fleet arrived by iteration at several things GitHub arrived at by
scale.

## The situation the fleet solves and reflow2 does not

Fifteen-plus concurrent agent sessions, three co-equal supervisors, one repository, one Docker host,
one production deploy — and no central server, no lock service, no database. Coordination is
**files in git plus a notification path**. It works reliably enough that its owner runs six sessions
against it as a matter of routine.

reflow2's design (`dec:multi-writer-architecture`, `dec:repo-file-embedded`) reaches for exactly this
substrate: a repo file, merged on pull, no hosted service. So the fleet is not an analogy. It is a
working instance of reflow2's own chosen architecture, several years of iteration ahead of it — and
on 2026-07-25 that instance is where reflow2 was measured and found wanting.

---

## 1. Identity is assigned out of band, never derived

> *"Your handle = your SESSION UID. Run `echo $CLAUDE_CODE_SESSION_ID` — the harness already gave you
> a globally-unique UUID. No counter, no reading the roster to pick a number, no 4-slot census — your
> name is unique **by construction** whether the pool holds 1 worker or 15, so duplicates/mint-races
> are impossible."*
>
> *"~~MINT-RACE GUARD~~ **RETIRED**. The mint-race it guarded against was an artifact of *deriving*
> the handle by reading shared state; UID naming removes the derivation, so it removes the race."*
> — P.md, setup steps 2 and 4b

**This is the single most transferable idea in the fleet, and it is the deepest.** The fleet used to
name workers by slot number, which meant reading the shared roster to find a free one, which meant
two simultaneous joiners could pick the same name, which meant a guard: re-read after posting and
re-suffix if you collided. All of that machinery — and the class of bug it patched — existed *only
because the name was derived from shared state*. Take an identifier that was assigned with zero
coordination and the guard becomes deletable. Not fixed: **retired.**

reflow2 has this bug in two places right now.

- **A design does not know its own name.** `req:design-identity` is open and the coherence gate's
  one standing note. Every graph opened in memory answered `DEFAULT_GRAPH_ID`, which is why
  `mirror_surface` needed `open_in_memory_as` before its tests could tell two designs apart. A
  mirror between two designs that both claim to be "reflow2" is meaningless.
- **A seat does not know its own name.** Claims (`claim_region`) record a *cluster* and a *label*,
  not a durable owner, so `claim_report` cannot distinguish a live seat from one that closed three
  days ago.

The fix is the fleet's, twice: mint a UUID at genesis, store it in the graph, and let the friendly
name be a *label* on top of it. Identify a seat by the harness session id it was given. Neither
identifier is negotiated with anybody, so neither can race, at 1 seat or 15.

**Filed:** the mechanism half of `req:design-identity`; `req:claims-have-owners`.

## 2. Coordination is announcement plus provable ordering — never a lock

> *"**CLAIM ASSERTIVELY — grab-and-go, DON'T wait for permission.** If you need a worker and one is
> `🟢 AVAILABLE`, CLAIM it and START — don't ask the user, don't defer to peers, don't hesitate…
> Timidity wastes the pool."*
>
> *"**AFTER EVERY CLAIM → POST IT.** Claiming happens here (grab-and-go); then announce the claim +
> the lane on `B.md` — **every time, not only when contested.**"*
>
> *"⚖️ CLAIM-CROSSING RESOLUTION (**first-on-disk, both provable**) — w-72c75734 = 🐞 ·
> w-83f29559 = 🔌; 👑's later claims VOID."* — P.md
>
> *"First claim wins until the Boss says otherwise."* — COORD.md §2.2

reflow2 already got the hard half right, and by the same reasoning: `cap:cluster-claims` never
blocks anybody, it reports overlaps (`ver:cluster-claims` pins exactly that). What the fleet adds is
the two halves reflow2 left out.

**Announce unconditionally.** A claim nobody can see is not a claim. In a repo-file architecture,
"announce" has a precise meaning: **commit and push the claim before you start the work**, not with
it. reflow2's claims currently live in the graph, and the graph reaches other seats only when
somebody exports and commits — so today a claim is typically published *after* the work it was
supposed to deconflict.

**Resolve by provable order.** When two claims cross, the fleet does not ask a server who won: it
looks at which landed on disk first, and both parties can verify the answer independently. reflow2
has strictly better material for this than a markdown timestamp — a content-hash chain and git
history. The doctrine is worth stating out loud, because the alternative people reach for by reflex
is a lock, and a lock is what took five of six sessions offline.

**Filed:** `dec:advisory-concurrency` (proposed) — the claim is announced by being pushed, and
crossings resolve by what git can prove, never by exclusion.

## 3. Before you write to something shared, re-read the tail

> *"**Multi-writer** (bosses claim, workers check-in/recycle): commit your OWN posts, `git pull
> --rebase` + re-read the tail **immediately before writing**."*
> — P.md header, citing memory `feedback_boss_compaction_clobbers_concurrent_post`

Note where that memory's name points: the damage came from a *maintenance* action (compaction)
overwriting a concurrent post. Not two people typing in the same paragraph — one person rewriting a
file from a copy that had gone stale in their hands.

**reflow2 has this hole and it is worse than the fleet's, because reflow2's staleness is invisible.**
The per-seat pattern reflow2 now recommends is: your own RocksDB graph, the committed
`docs/design/reflow2.json` as the shared record, the merge driver reconciling them on pull. But a
seat's live graph is a *long-lived cache* of that file, and nothing in reflow2 notices when the file
moves underneath it. A session that opened its graph on Tuesday, pulled on Wednesday and exported on
Thursday writes a document derived from Tuesday's design — and the merge driver will merge it
cleanly, because a stale export is not a conflicting export. It is a *complete* one.

The fleet's rule ports directly, and reflow2 can do better than a rule: it already has
`compare_designs`, which answers "how do these two documents differ" node by node. A seat should be
told, before it writes, that the committed record has moved since it last synced — the same way
`git push` refuses a non-fast-forward instead of silently winning.

**Filed:** `req:stale-seat-knows` — the highest-value single item in this document, because it is a
*silent data-loss* path in the architecture reflow2 has already chosen.

## 4. A notification path must be proven to wake, and must have a backstop

> *"**Run the recipe under the harness's session-waking `Monitor` facility — a bare background-bash
> loop writes output but never wakes your session and is NOT a monitor (proven deaf ×2).** BANNED:
> `comm` AND `grep -Fxvf` — both fail silently (zero wakes)."*
>
> *"Also arm a **~10-min `ScheduleWakeup` heartbeat** so a missed wake self-heals."*
>
> *"**the claim reached me as a monitor wake, not a manual re-read**, so wake-proof (1) of the
> handshake is genuine."* — P.md setup step 3; A.md
>
> *"Re-run the comms handshake with every worker claiming to be live — **a prior session's ACK doesn't
> survive a boss swap**."* — S.md

Three separate rules, one principle: **a mechanism you rely on to interrupt you is worthless until
you have observed it interrupt you.** The fleet banned two plausible implementations by name after
measuring zero wakes, added a heartbeat so a missed wake self-heals rather than hanging forever, and
refuses to count a peer as live on an acknowledgement from a session that has since been replaced.

reflow2's equivalent machinery is `loop_nudge` (the Stop hook) and the read-side `loop_hint`. It has
the same failure mode and, right now, the same blind spot: `tools/test_loop_nudge.py` tests the
*script*, given its inputs. Nothing tests that the hook is installed, fires in a live session, and
reaches the agent — and if it is missing, nothing self-heals, because the read-side hint only fires
when a read happens. The whole coherence loop rests on a path nobody has proven end to end.

The liveness rule bites too: a claim by a session that has closed should be *reported as stale*, not
listed as if somebody were working. With owners attached (§1) that becomes computable, which is the
reflow2-shaped version — computed, not remembered.

**Filed:** `req:nudge-path-proven`; the staleness half of `req:claims-have-owners`.

## 5. Standing grants are named; everything else expires with the session

> *"**Standing PR-merge authority.** Every Boss may merge worker PRs on its own approval — a
> **standing grant that carries across every new Boss session**; the user does not re-grant it. …
> Scope: merges to `main` **only**. It does **not** extend to prod deploys, which remain JOINT boss
> actions with explicit user authorization — **merging is not deploying.**"* — COORD.md §1
>
> *"prior-session GOs — merge authority, deploy windows — do **not** carry into your session. Get
> fresh ones."* — S.md

The fleet does not have a vague sense of what it may do unasked. It has a written list of exactly
which permissions survive a context reset, and an explicit statement that the highest-consequence
one does not. That distinction is drawn along a line worth stealing verbatim: **the reversible class
is standing, the outward-facing class is per-session.**

reflow2 has accumulated standing grants without ever writing them down as such. Accepting checksum
drift under a standing policy from 2026-07-23 is one. Cutting and pushing a release is emphatically
the other kind — it is this project's deploy — and so is filing an issue in a repository the user
does not control (which the `report-friction` skill already treats correctly: never file without
asking).

This one costs nothing but honesty: a short section in AGENTS.md listing what an agent may do here
without asking again, and what it must ask for every session. Absent that list, both errors are
available — asking permission for the routine, and assuming it for the irreversible.

**Filed:** as an AGENTS.md doctrine note, not a graph node. It governs agents, not the design.

## 6. Shared records get compacted in a quiet window, and something watches their size

> *"Compaction standard"* (COORD.md §2) · *"Register for the compaction hook: `echo
> "$CLAUDE_CODE_SESSION_ID" > .claude/.boss_q` — gitignored, per-session; the Stop-hook then nudges
> you when YOUR bus is over the compaction threshold."* — S.md
>
> *"CURRENT STATE + everything after the last compaction (archives in `docs/archive/` if you need
> history)"* — S.md cold-start

The fleet's buses are append-only logs that would grow without bound, so: a threshold, a hook that
notices, compaction into an archive rather than deletion, and a **top-of-file current-state block**
that a cold-starting session reads instead of the whole log. Note the registration trick — a
per-session gitignored file naming who to nudge — which is how a shared hook finds the right
session in a multi-session repo.

reflow2's `COORD.md` and `docs/backlog.md` have precisely this shape and no such discipline; the
backlog is 100+ items deep and the retirement of its prose into graph elements is already an open
thread. reflow2's own Stop hook is the natural place for the nudge. Low priority, cheap, and it
protects the one document every session must read.

**Filed:** nothing yet — noted here so the next COORD.md rewrite has the pattern to hand.

## 7. Dissent is recorded and then waited on

> *"If a worker believes a Boss ruling is wrong, it writes a dissent under its status entry and
> **waits**; it does not proceed unilaterally. The user overrides everyone."* — COORD.md §1

A disagreement becomes a durable artifact instead of either a silent capitulation or a unilateral
act. reflow2 has the vocabulary for this already — a `proposed` Decision, `CONTRADICTS`, and the
rule that every move off `proposed` records the *user's* word — but no skill tells an agent to reach
for it when it thinks the recorded design is wrong. Right now an agent that disagrees with the graph
either follows it or quietly doesn't.

**Filed:** nothing new; a line for the `revise-design` skill.

---

## What was already right, confirmed by a working fleet

Worth stating, because a lessons document that only lists deficits misleads:

- **Claims that report rather than block** (`cap:cluster-claims`). The fleet reached the same
  answer — advisory claims, contention reported and resolved by evidence — and runs 15 seats on it.
- **Fail loud with what to do next** (rule 4). The fleet's ⭐#2 "no silent failure" is the same rule,
  arrived at independently, and its worst outage of the month was a `/health` endpoint that returned
  a hardcoded `"healthy"` for seven days.
- **Report, don't judge** (`dec:report-dont-judge`); **ask, don't repair** (`dec:ask-not-repair`).
  The fleet's STOP+REPORT walls — *"no patching / no guessing"* — are the same doctrine with a siren
  on it.
- **Verify by execution.** COORD.md §2.11 makes a boss re-run the evidence first-hand before
  accepting a worker's PR; reflow2's Verification nodes carry `method` and status for the same
  reason. The fleet's sharper version: *"a green suite the reviewer didn't run isn't evidence"* —
  and its practice of proving a test **red on main** before showing it green.

## The five things to build, in order

1. **`req:stale-seat-knows`** — a seat is told the committed design moved before it writes over it.
   Silent data loss in the architecture reflow2 has already chosen. (§3)
2. **`req:design-identity`, with the mechanism from §1** — UUID at genesis, name as label. Closes the
   gate's standing note and makes mirroring between two designs mean something.
3. **`req:claims-have-owners`** — a claim carries the session that made it; a claim with no live
   owner is *reported* stale, not listed as work in progress. (§1, §4)
4. **`req:nudge-path-proven`** — prove the loop nudge reaches a real session, and give it a backstop
   for when it doesn't. The coherence loop rests on it. (§4)
5. **`req:read-while-held`** — the fleet's actual blocker, and the one thing on this list reflow2
   cannot fix in its own source: reading a design must not require owning it. Needs
   `open_as_secondary` exposed in `dynograph-storage`. (from `feedback.md`)

Items 1–4 are reflow2-shaped: identity assigned rather than negotiated, staleness computed rather
than remembered, liveness derived rather than asserted. Item 5 is a dependency.
