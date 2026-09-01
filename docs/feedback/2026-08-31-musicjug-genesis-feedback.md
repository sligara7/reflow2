# reflow2 feedback — from the MusicJug genesis session

**Date:** 2026-08-31
**Project:** `proj:musicjug` — a browser rhythm/music-making game for a 9-year-old, hosted on GitHub Pages
**Session shape:** `/genesis` from a paragraph → requirements/capabilities → `/brainstorm` → decisions →
components, interfaces, flows, verifications → clean structural sweep. No code written yet.
**Scale reached:** 19 requirements, 10 capabilities, 10 components, 7 interfaces, 2 flows, 4 verifications,
~15 decisions, 71 nodes total.
**Reported by:** Claude Code (`who:claude-code`), at the owner's request.

This is written from the agent's seat. Where a complaint is really about the harness rather than reflow2,
it says so.

---

## The good

### 1. `detect_defects` found two real architecture bugs before a line of code existed

This is the headline and it is not a small thing.

I laid out ten components and wired their dependencies. The sweep came back with
`comp:coach → comp:shell → comp:coach`, severity critical. I fixed it. The next sweep came back with
`comp:shell → comp:stage → comp:shell`. **Same mistake, made twice, minutes apart, by me, unnoticed both
times.** In each case I had given a part a contract that let it reach back into the page that owned it —
an import loop in module terms, and worse than that on a phone, because a part that can call back into the
page can be made to wait on layout.

Catching those at design time cost about four minutes. Catching them halfway through an implementation
costs a refactor of the thing you least want to refactor. The second one is what convinced me the first
wasn't luck.

It also produced the right downstream artifact: I wrote the rule down (`dec:shell-is-the-composition-root`)
so the third one doesn't get built. A defect detector that changes the *rules* rather than just the graph
is doing more than linting.

### 2. "Silent about that level rather than clean about it"

The single best piece of design in the tool.

```
the component level has 7 component(s) and ZERO dependency pairs between them, so every topology
result here — cycles, severability, single points of failure — is silent about that level rather
than clean about it
```

I had just received a clean sweep. That sentence told me the clean sweep meant nothing, because the rules
had walked an empty population. I went and declared the twelve component dependencies **specifically
because of that line**, and that is what made the cycle detection above possible at all.

Almost every analysis tool in existence reports "0 findings" identically whether it looked hard and found
nothing or couldn't have found anything. `rule_populations` and `coverage_note` refuse to do that. This
should be copied by other tools.

### 3. `quality_target_unstated` forces the right question at the only time it is cheap

Being made to ask *"what is this system FOR — and which would you sacrifice the others for?"* before
allocating anything changed the architecture. The owner said timing. Because of that answer I fused
rendering and touch input into one component and made one clock the sole time authority — a shape I would
not have chosen by default, and which a tidy layered version would have felt subtly bad against.

What made it work was the gap text spelling out how the four attributes actually disagree (performance
wants least chatter; reliability wants no articulation point and may duplicate; maintainability wants
what-changes-together-together; security wants boundaries following trust). That is what let a
non-specialist owner answer it in one click instead of deferring.

The note that **allocating without asking silently picks performance** is exactly the kind of thing that
is obvious in hindsight and invisible in the moment.

### 4. Status semantics that refuse to forge the user's word

`add_decision` landing at `proposed`, with only the owner's word moving it, combined with
`authored_by role=approver`, produced a genuinely good human/agent mechanic:

I recorded my own architectural recommendation (`dec:one-clock`) as a decision, left it **unsettled**, and
attached the owner as approver. `loop_status` then reported it under `assigned_decisions` and kept
reporting it until they answered. I did not have to remember to ask, and I could not accidentally record
my recommendation as their decision.

I have not seen another design tool that models "I recommend this, but it is not mine to decide" as a
first-class state. For agent-driven work this is close to essential.

### 5. `repair_is_a_judgement`, and refusing to auto-fix

> Deleting the node to clear this finding is the one repair that looks clean and loses the most.

Correct, and worth saying out loud to an agent, which is exactly the reader most likely to take the clean-
looking repair. Same for unthreaded clusters: *"connecting this cluster would assert relationships nobody
stated, which is worse than the finding."* The tool declining to fix things it could mechanically fix is a
sign of good taste.

### 6. The duplicate guard, and its framing

It refused `cap:note-highway` because `req:vertical-note-highway` existed, and explained:

> Recording what shipped and recording what must stay true are different acts on the same work, and
> they are supposed to read alike.

That sentence told me instantly which of the two cases I was in. The refusal cost one extra call and
prevented a genuinely muddled graph. (See the friction section — the frequency is a separate issue from
the quality of the framing.)

### 7. Instructions that carry their own evidence

The brainstorm skill saying *"on 2026-08-21 reflow2's own graph held 145 brainstormed ideas joined by 12
edges; 111 of them reached no other idea within two hops"* is why I actually drew the relations instead of
skipping to the next step. An instruction with a measurement attached gets followed. An instruction that
says "remember to link things" does not.

Likewise *"a note is a full answer, not a weaker one"* for the no-relation case — that framing is what
stops the honest answer feeling like a cop-out.

### 8. The session-end loop nudge

The stop hook fired once, told me 13 writes had happened with no loop check, and I was in fact about to
finish with two structural defects outstanding. It did its job.

---

## The bad

### 1. Bootstrap requires a client restart, at the worst possible moment

`reflow2_start_design` creates the directory and then reports that the session must reconnect via `/mcp`
before any design tool exists. This is the **first** thing a new user experiences, and it is a hard stop
requiring manual action mid-flow.

The server instructions handle it about as gracefully as prose can — the warning that the state is stale
and that a restore may already have landed underneath is genuinely well-written. But it still cost a full
round trip plus a user action before a single word of the brief could be captured, and the user had to be
asked to do something they had no context for.

### 2. Cross-type near-match refusals fire on the normal case, not the exceptional one

I was refused four times, and all four times the answer was "yes, distinct":

| Attempted | Blocked by | Actually |
|---|---|---|
| `cap:note-highway` | `req:vertical-note-highway` (Requirement) | distinct |
| `dec:two-speed-synthesis` | `dec:idea-baked-kit` (the brainstorm it promoted) | distinct |
| `dec:shaping-controls` | `dec:idea-she-shapes-the-sound` (ditto) | distinct |
| `dec:one-clock` | `comp:clock` (Component) | distinct |

A Capability usually shares wording with the Requirement it satisfies — the tool's own message says they
are *supposed* to read alike. A Decision about a part usually shares wording with the part. And a promoted
brainstorm idea shares wording with the idea by construction, since promotion is step 5 of the skill that
created it.

So the guard's highest-frequency firings are on the three pairings the design **wants** to look similar.
It is not wrong to check, but Capability→Requirement, Decision→Component, and (especially)
Decision→`kind: exploratory` Decision might deserve a higher similarity bar, or a note saying "this pairing
is usually legitimate — confirm and move on."

### 3. The revision warning arrives after it is too late to act on

Sharpening `dec:synthesized-audio` with the owner's added quality bar produced:

> `record_change` BEFORE the merge is what puts the old state in the design's own timeline; called now
> it would snapshot the REPLACEMENT and the history would be wrong.

The advice is correct and the risk is real. But it arrives **after** the write, for an operation the tool
explicitly documents and supports, at the moment I was doing exactly the right thing (recording new
information the owner had just given). And the prescribed order is not reliably achievable from a harness
that emits parallel tool batches.

Net effect: a well-earned warning that reads like an accusation and offers no forward action, only an undo
recipe. See the ideas section for what I think it should do instead.

### 4. `declare_dependency` is not what its name says

I searched for a way to record that one component depends on another. `declare_dependency` matched by name
and turned out to be cross-*design* version pinning (git tags, feature flags, export watching) — a
completely different concept. I fell back to raw `create_edge` with `DEPENDS_ON`.

The typed helpers cover `contains`, `contain_component`, `satisfies`, `allocate`, `realizes`, `provides`,
`consumes`, `governed_by`, `decomposes` — but not the single most common structural edge in any design.
Given that component `DEPENDS_ON` is what feeds cycle detection, SPOF analysis and the seam gap, its
absence from the typed surface is surprising, and the name collision actively misleads.

### 5. The gap list churns, with no visible frontier

Every fix surfaced more:

```
declare component dependencies  → undeclared_seam × 12 pairs
declare interfaces              → no_published_boundary
designate one published         → unverified_published_contract + incomplete_published_contract
add components                  → unallocated_component + design_without_build + no_deploy_operate
add a release                   → release_without_epoch
```

Every one of these was legitimate and I acted on all of them. But at no point could I tell the owner
"three more rounds and we're clean" versus "this goes on for twenty." There is no sense of depth or
frontier, and to a user watching an agent work, an ever-refilling list reads as a treadmill even when each
item is genuinely worth doing.

### 6. `acknowledge_gap` is the only exit, and it requires the owner's word

`decomposition_coverage` asked a real question — *"what did the parent hold that none of its children
hold?"* — and I had a real, substantive answer: four children say **what** she learns and the newly-added
fifth carries the **how**, which was the part actually at risk of being designed away.

The tool offered two ways to record that: write another child (wrong — there was nothing left over), or
`acknowledge_gap`, which is documented as the owner's word by definition and refuses an unknown approver.
An agent with a correct substantive answer has no way to record it *as an answer*. I ended up
acknowledging with the owner named, which was honest only because they had endorsed the reading one
message earlier. Without that I would have had to leave a solved gap open as noise.

The gap text literally says *"Write the answer into the design ... or acknowledge_gap"* — but only the
second has a tool.

### 7. Tool-schema loading is a per-need round trip (mostly a harness issue)

I made roughly ten separate `ToolSearch` calls across the session, each one a round trip, because I could
not know I would need `delete_edge` until a cycle appeared, or `set_interface_designation` until a gap
asked for it. A reflow2 session therefore has a high call-count floor.

This is the deferred-tool harness rather than reflow2 proper. But reflow2 ships `find_tools`, and I never
used it because nothing told me how it differs from the harness's own search — so the one mitigation that
exists was invisible from where I sat.

---

## Ideas

### 1. Refuse a cycle at edge-creation time, the way duplicates are refused

Both cycles I built were closed by a single `consumes`/`provides` call. The sweep caught them, but
`consumes` already knows the existing `DEPENDS_ON` and `PROVIDES` edges, and could refuse-or-warn inline:

> This CONSUMES would close a loop: `comp:coach → comp:shell → comp:coach`. If the dependency is real
> and only pointing the wrong way, reverse it — have the source publish and the target subscribe.

The duplicate guard already proves this pattern works and that agents respond to it. Inline would have
taught me the rule on the first occurrence instead of the second.

### 2. Auto-snapshot on revise instead of warning about the lost value

The revision reply says, correctly, that it holds *the only copy in existence* of the replaced field. It
is therefore uniquely positioned to prevent the loss it is warning about. Snapshot automatically (or offer
`snapshot_prior: true`) and the warning becomes a receipt rather than a regret.

### 3. Split `acknowledge_gap` into `answer_gap` and `acknowledge_gap`

- **`answer_gap`** — "here is the substantive resolution; it is written into the design." An agent may do
  this. Expires the same way when affected nodes change.
- **`acknowledge_gap`** — "this is fine as it stands." Stays the owner's word, keeps the approver
  requirement.

This preserves the anti-forgery property exactly where it matters while giving the more common case a
door. Roughly half the gaps I hit wanted the first and only had the second.

### 4. Report a gap frontier

Even coarse would help: *"fixing these 3 will likely surface ~5 more, mostly `undeclared_seam` and
`unverified_*`; the phase-level gaps clear only when artifacts exist."* Enough to tell a human how long the
road is. `detect_gaps` already knows the phase transitions that trigger each source.

### 5. A typed `depends_on(from_component, to_component)` helper

And rename the existing one to `declare_external_dependency` or `declare_design_dependency`. Two different
concepts should not compete for the same obvious name, especially when one of them feeds every topology
rule in the tool.

### 6. Workflow tool bundles

`find_tools("genesis")` (or a `bundle` argument) returning the ~12 schemas that workflow needs in one
call, so an agent can load once rather than ten times. Then document the difference between `find_tools`
and whatever the host harness provides, because right now the reflow2-native one is invisible.

### 7. A phase-transition hint after structure lands

I got from "components exist" to "clean sweep" by following individual `loop_hint` strings and re-running
`detect_gaps` five times. A single line at the moment components first appear —
*"next: declare dependencies, then interfaces on the coupled pairs, then verifications"* — would have
compressed five rounds into one plan. The tool clearly knows this ordering; it just emits it one hint at a
time.

---

## Summary

The thing reflow2 did that nothing else would have done: **it found two architecture cycles I had built and
not noticed, before any code existed, and the second one proved the first wasn't luck.** Second to that,
`coverage_note` telling me a clean result was meaningless, and `quality_target_unstated` forcing a question
whose answer changed the shape of the system.

The friction is almost entirely at the seams — a bootstrap that needs a restart, a near-match guard tuned
for a rarer case than the one it keeps hitting, a warning that fires too late to act on, one badly-named
tool, and gaps that can be answered in prose but only closed with the owner's signature.

None of the friction cost anything close to what one undetected cycle would have.
