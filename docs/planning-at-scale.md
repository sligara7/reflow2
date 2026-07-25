# A thousand planners, one plan: reflow2 at coordination scale

> Part of the **Reflow 2.0** design docs — see **[overview.md](overview.md)** for the full map and
> reading order.

*Distant-future exploration, recorded because the near-term rule is not to foreclose it. Nothing
here is scheduled work. The open questions live in the graph as proposed Decisions —
`dec:live-tier`, `dec:who-needs-to-know`, `dec:plan-time-axis` — and this document is their
reasoning.*

## The case

Anthony's, 2026-07-25, from work he has actually done: **State Funerals for US Presidents** — he
worked President Bush's. Hundreds to thousands of people plan and coordinate simultaneously, and the
plan is **echeloned**: NORTHCOM holds the OPLAN, JTF-NCR holds a more specific plan under it, each
service holds its service plan, and each unit holds its own beneath that. Events and news arrive
during execution and the plan has to move in response, in minutes, at every level that the change
touches.

His framing of the reflow2 question is exact: *"while not a 'design', it is a plan and I feel like
reflow2 should be able to capture this type of effort."*

Two questions, then, and they are independent:

1. **Can the vocabulary carry a plan?** (Mostly yes, and the gap is specific.)
2. **Can the architecture carry a thousand simultaneous writers reacting in near real time?**
   (Not today, and not by scaling what exists — but the shape it would take is already decided.)

## 1 · The vocabulary carries a plan better than it has any right to

`req:design-anything` kept the vocabulary domain-neutral on purpose. The payoff shows up here — a
staff planning process maps onto it almost without strain, and the mapping is not a metaphor:

| Planning artefact | reflow2 element | Note |
|---|---|---|
| Specified & implied tasks; commander's intent | `Requirement` | Intent that everything below traces to |
| Functions to be performed | `Capability` | The WHAT, separate from who does it |
| Units and organisations | `Component` | The WHERE |
| **Troop-to-task / task organisation** | `ALLOCATED_TO` | Capability → the unit that will perform it |
| **Coordination points, handoffs, transfer of authority** | `Interface` + `PROVIDES`/`CONSUMES` | Both sides recorded, or the seam is invisible |
| Phases, sequence of events | `Flow` + `PRECEDES` / `TRIGGERS` | Sequence — but not clock time; see §3 |
| **COA development, comparison, decision** | `Decision` + `register_alternative` + `analyze_alternatives` + `collapse_decision` | Options held open with a design behind each, then one chosen, with the analysis kept |
| Branches and sequels | Registered alternatives that stay open | The road not taken is part of the plan's memory |
| Constraints and restraints | `Constraint` | Limits, with breach **computed** rather than remembered |
| The two or three that cannot slip | `Constraint category=kpp` | Threshold + objective; a breach is derived from the design |
| **Rehearsals, confirmation briefs, backbriefs** | `Verification` + `VERIFIES` | "Verify by execution" is a rehearsal |
| **A FRAGO** | `ChangeEvent` + `propagate_change` | A change *with its blast radius computed* |
| Staff gap analysis | `detect_gaps` | Unallocated tasks, unsourced requirements, a handoff recorded on one side only |
| Who the plan serves | `Actor` | The family, dignitaries, the public |
| Forces, vehicles, aircraft, billeting | `Resource` + `require_resource` | With a budget roll-up |
| Who is working which part | `Contributor` + `claim_region` | Advisory, computed, never blocking |
| Plan versions | `DesignEpoch` / `Snapshot` / content-hash chain | Which version was in force, provably |

Two of those rows are the reason this is worth taking seriously rather than filing as a stretch.

**A FRAGO is a change event with a computed blast radius.** "The motorcade slips thirty minutes" is
one edit, and the question every staff asks next — *who does that break?* — is the question
`propagate_from` already answers, including where it crosses a published boundary into somebody
else's echelon. That is not a feature reflow2 would need to grow for this use case. It is the thing
reflow2 already is.

**The seams are where plans fail, and reflow2's detectors are pointed at seams.** A handoff recorded
by the receiving unit and not the sending one is exactly `detect_gaps`' one-sided-interface finding.
An echelon that has not allocated a task to anybody is an unallocated capability. Both are checkable
before the rehearsal rather than discovered during it.

### What the vocabulary is genuinely missing

Being honest about the gaps matters more than the mapping:

- **Scheduled time.** reflow2's temporal axis is *decision* time — epochs, snapshots, `valid_from`,
  ChangeEvents: "when did we settle this, and what was in force then". A plan needs *execution*
  time: H-hour and offsets from it, durations, windows, and a **critical path computed from
  dependencies**. `Flow`/`PRECEDES` gives ordering, not schedule. This is the one place the
  vocabulary would have to grow, and it is the highest-value thing on this page after federation
  (`dec:plan-time-axis`).
- **Command relationships.** `ALLOCATED_TO` is capability→component; OPCON / TACON / supported /
  supporting are relationships *between components*, and they change by phase. Modellable, not
  modelled.
- **Geography.** Locations, routes, control measures. Probably a property and a view rather than new
  vocabulary, but it is currently absent.
- **A running log.** "Post messages, react to news" needs a channel. That question is already open as
  `dec:coord-board-in-graph`, raised for two people; a thousand raises it far more sharply, and the
  answer in §2 is that the *channel* should be derived rather than subscribed to.

**Note what must not happen:** none of this adds military words to the schema. `req:design-anything`
holds — a view renders OPLAN vocabulary over neutral elements, the same way the MOSA question was
settled (`dec:mosa-conformance`: the requirements stay domain-neutral, the acquisition words live in
a projection). A schema that learned "OPORD" would be a schema that has to learn every domain's
nouns forever.

## 2 · A thousand writers is not one graph, and never was

The instinct is that a thousand simultaneous users means one enormous shared store, which means a
central service, which means re-architecting everything reflow2 decided. That instinct is wrong, and
the reason is a decision already made: **`dec:nested-graphs` option (c) — a design is its own graph
when something is separately owned, released or shared. Hierarchy does not decide; authority does.**

The funeral hierarchy is that rule, written by somebody else first:

- NORTHCOM's OPLAN is one graph. NORTHCOM's staff writes it.
- JTF-NCR's plan is its own graph, which **mirrors** NORTHCOM's published surface — the tasks,
  timings and control measures it is given — and holds its own internals.
- Each service plan is its own graph, mirroring JTF-NCR's surface.
- Each unit's plan is its own graph beneath that.

So a thousand planners are not a thousand writers on one store. They are perhaps a hundred to two
hundred graphs with **five to twenty writers each**, plus a publication layer between them. That
decomposition is the whole ballgame: it turns an impossible problem into an ordinary one, and it is
also how the actual organisation works. Echelons own their plans. Nobody edits NORTHCOM's OPLAN
because they are in a battalion; they read what NORTHCOM published and write their own.

It also means the authorization model needs no new concept: **the ownership boundary is the
authorization boundary.** You write your echelon's graph; you read your superior's published
surface. `Interface.designation` (internal | published) is already the read-authz primitive, and
`export_surface` already refuses to leak internals — it counts and names what it withheld.

### What actually breaks, concretely

Federation makes the problem tractable. It does not make today's implementation sufficient. Six
things break, in rough order of how soon:

1. **Single-writer per graph.** Five writers on one echelon's graph is five sessions on one
   RocksDB lock — the outage we fixed at *six* seats on 2026-07-25, where the fix was to explain the
   failure, not to remove it. Multi-**reader** is `req:read-while-held` (blocked on
   `dynograph-storage` exposing `open_as_secondary`). Multi-**writer** per graph is a real store
   question, unanswered.
2. **The whole-document write path.** Every write today ends in serialising the entire design and
   three-way merging the entire document. reflow2's own 606-node graph is 769 KB; a funeral
   federation is 10⁵–10⁶ nodes across its graphs, changing continuously during execution. The
   export must remain the **durable archival derivative** — never become the write path. (It is not
   today: the store is the write path. Keeping it that way is a non-foreclosure rule, not a feature.)
3. **Latency.** A git round trip is minutes. "React to events near real time" is seconds. Different
   regime, and no amount of merge-driver work closes it.
4. **Nothing pushes.** reflow2 nudges an agent that is already running. A planner whose task just
   moved is not running anything.
5. **No identity, no authorization.** A thousand users needs per-user identity and per-boundary
   permission. `Contributor` exists; enforcement does not.
6. **Staleness becomes the dominant failure.** `req:stale-seat-knows` — a seat writing over a record
   that moved underneath it — is one seat's bad afternoon at two writers and a systemic property at
   a thousand.

### What the live tier would look like

The shape follows from the constraints above rather than from taste (`dec:live-tier`):

- **One process per graph, many clients.** Already identified as the cheapest first cut in
  `dec:central-host`: a process per project, its own graph path, its own token, project selection by
  which endpoint you point at — no multi-tenancy inside the process. At federation scale that is
  *also the natural sharding*: one process per ownership boundary, which is one staff.
- **MCP over streamable HTTP.** `rmcp` has the transport; reflow2 serves stdio only. This is the
  one piece of plumbing that is purely work rather than a question.
- **Writes to the store, exports derived.** The git file stays the durable record and the offline
  and small-team answer (`dec:repo-file-embedded`, `dec:multi-writer-architecture` — unchanged for
  the case they were decided for). This is a **both**, not an either: the same core, two deployment
  tiers.
- **Ordering without a single git history.** "First-on-disk, both provable"
  (`dec:advisory-concurrency`) needs a replacement when there is no shared repo. There is one, and
  it is the federation shape again: **a total order within each ownership boundary** (the store
  gives it; the content-hash chain proves it) and **dated coordinates across boundaries** (a mirror
  already carries whose design, which version, when). No global clock, no distributed consensus —
  which is precisely how echelons coordinate: each owns its own sequence, and cross-echelon
  agreements are pinned to a named version at a named time.

### The part that is actually novel: who needs to know is *computed*

This is the strongest argument that reflow2 belongs in this problem rather than a chat tool with a
plan attached, and it is worth stating plainly (`dec:who-needs-to-know`).

In every coordination system that exists, notification is **subscription**: you join a channel, or
somebody remembers to include you on the distribution. Both fail the same way — the person who most
needed to know was not on the list, and nobody finds out until it matters.

reflow2 does not need a subscription model, because it already computes the answer. A change lands;
`propagate_from` returns the blast radius, including which published boundaries it crosses; the
owners of the impacted regions are exactly who must be told. Nobody maintains a distribution list,
nobody is spammed with changes that cannot touch them, and the notification is *derived from the
plan's own structure* — so when the plan changes shape, the distribution changes with it, for free.

That is the reflow2-shaped answer, in the same family as every other one in this project: **computed,
not remembered.** It also inherits the honesty rules — a notification says what changed, what it
impacts, and what is now unresolved, rather than "the plan was updated".

## 3 · What must not be foreclosed now

Anthony's own rule, from the GitHub pass: *"don't do something now that would make it impossible for
reflow2 to scale and grow to what GitHub is."* Applied to this page — none of these are work, they
are things to keep true:

1. **The store stays the write path; the export stays derived.** The moment a feature requires the
   whole document to be rewritten to record one change, the live tier is dead.
2. **No tool takes a file path where a graph coordinate would do.** File-shaped APIs assume one repo.
3. **`graph_id` must stop being a constant** — already filed (`req:many-designs-one-service`
   accepted, `req:design-identity` and `dec:identity-out-of-band` open). Identity assigned out of
   band scales to a thousand graphs for the same reason it scales to fifteen seats: nothing is
   negotiated, so nothing races.
4. **Single-writer must stay an implementation fact, not an API promise.** Anything that leaks "you
   are the only writer" into tool semantics has to be unbuilt later.
5. **The vocabulary stays neutral.** Domain words go in views (`req:design-anything`).
6. **Claims stay advisory.** At a thousand writers, a blocking claim is a denial-of-service on your
   own organisation. Report and resolve by evidence (`dec:advisory-concurrency`), which is also what
   the fleet proved at fifteen seats and what a staff does socially.

## 4 · Reconciliation with `org-scale-vision.md`

[org-scale-vision.md](org-scale-vision.md) describes "one graph the whole company plans in". This
page supersedes that framing while keeping everything it wanted. The golden thread from top-level
objectives down to a unit's task is exactly right, and it does **not** require one store — it
requires a *chain of published surfaces*, one per echelon, each mirroring the one above with a dated
coordinate. That is strictly better than one graph: it survives an echelon being offline, it makes
authorization fall out of ownership, it shards naturally, and it matches how the organisation
already divides authority. `dec:nested-graphs` decided this on 2026-07-25; the one-graph reading is
a road not taken.

## 5 · The honest counter-argument

Recorded here so nobody has to rediscover it, and because `dec:central-host`'s rationale already
made a version of it: **reflow2's loop is ask-the-human-and-record-the-answer — deliberate,
attributable, and slow.** A live coordination floor optimises for simultaneity. During *execution*,
when a motorcade is moving, the right tool may well be a radio and a common operational picture, not
a design brain that wants a decision recorded with its rationale.

The distinction that resolves it: reflow2's claim is not to be the execution net. It is to hold the
**plan** — the intent, the task organisation, the coordination seams, the constraints, the decisions
with their alternatives, and the record of what was in force when — and to compute, when something
moves, who that breaks. Planning for a State Funeral runs for months before the day; the
near-real-time requirement is about *replanning* under events, which is a plan-side activity, not a
radio-side one.

Which is also the reason this page is not scheduled work. The near-term value is the non-foreclosure
list in §3 — every item of which is worth keeping true even if a thousand users never arrive.
