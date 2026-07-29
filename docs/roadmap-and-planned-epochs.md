# The roadmap: planning epochs forward

Working design for `req:epochs-can-be-planned` and `req:plans-move-honestly`, both currently
**proposed**. Written 2026-07-29 from the committed export; nothing here is in the graph yet. The
purpose is to get the design settled enough that acceptance is one deliberate act rather than a
drift — which is what [BL-100](backlog.md) argues for and what `dec:epoch-roadmap-storage` was
raised to prevent.

Recommendations below are marked as such. The decisions are Anthony's; this document does not make
them, it makes them cheap to make.

## Where this stands

| Node | Kind | Status |
| --- | --- | --- |
| `req:epochs-can-be-planned` | Requirement | proposed |
| `req:plans-move-honestly` | Requirement | proposed |
| `req:graph-indexes-snapshots` | Requirement | proposed |
| `dec:epoch-roadmap-storage` | Decision | proposed — OPEN |
| `dec:plan-time-axis` | Decision | proposed — OPEN |

The foundations it builds on are already settled: `req:inviolable-intent` (accepted), `cap:kpp`
(verified), `req:requirement-lineage` (accepted), `req:what-next` (accepted).

## The atom is a satisfaction schedule

Anthony's distillation, 2026-07-28, is the load-bearing sentence: the roadmap is *a mapping of
requirements to the epoch or increment where they are satisfied*. "In epoch 1, requirements A and B
are satisfied; in increment 3, requirements C, D and E are expected to be satisfied; in epoch 27,
KPP 1 is required to be satisfied."

The three verbs in that sentence are three different modalities, and the distinction is the design:

- **ARE satisfied** — recorded history. The backward direction the graph already does.
- **EXPECTED to be satisfied** — a plan. Carries confidence; confidence is worth less further out.
- **REQUIRED to be satisfied by** — an obligation. A miss at arrival is a computed violation, not a
  question. This is the *scheduling face* of a KPP: `req:inviolable-intent` gains a deadline
  dimension without weakening its content dimension.

That shrinks what a planned epoch has to *be*. It is not an expected snapshot of the whole graph. It
is a set of dated pins, each carrying a modality and a confidence.

## What is stored, and what is derived

**Recommendation.** Store two modalities, derive the third.

`EXPECTED` and `REQUIRED` are claims about the future that exist nowhere else, so they must be
stored. `ARE satisfied` is already computable from what the graph holds — a requirement's
satisfaction is `SATISFIES` from a capability, and *when* it happened is the `ChangeEvent` that
moved its status, already pinned `AT_EPOCH`. Storing it a third time would create a second truth
that can disagree with the first.

This is the same economy `dec:epoch-roadmap-storage`'s third amendment reached for snapshots: the
expected-graph-at-epoch-N is a **derived view**, rendered from the pins that hold at N, not a stored
document. Nothing stores the same truth twice.

*Claim to verify before accepting:* that a requirement's historical satisfaction epoch really is
computable today for the general case. `AT_EPOCH` is currently `ChangeEvent → Epoch` (156 uses) and
never `Requirement → Epoch`, so pinning a requirement to an epoch is a **new edge shape**, and the
backward query has to route through ChangeEvents. If that routing turns out to be lossy, the
derive-don't-store recommendation is what changes.

## Two views, one mechanism

The requirement asks for two paired views of the same architecture: increments in **time** (epochs —
what the architecture is expected to be at epoch_1..n) and increments in **capability** (what each
small, frequent release adds — the ~2-minors-a-week practice).

**Recommendation.** One node type with a series discriminator, not two node types.

Both views are an ordered series of pins. The requirement asks that *any* two be comparable — "any
two epochs, any two increments, or either against the present." If time-increments and
capability-increments are different node types, that is three comparison operations and three sets
of detector logic. If they are one type distinguished by a property, it is one operation, and the
delta machinery that already exists works on both without learning anything new.

## Horizon, confidence, and what reflow2 must not claim

`req:plans-move-honestly` records the volatility insight: *"the further out you plan your epochs,
the more volatile they are, almost like what the Black-Scholes model was created for."*

**Recommendation.** Take the analogy for what it asserts and not further. It says volatility is real
and grows with horizon. It does not say reflow2 should price it.

Concretely: **confidence is authored, horizon is computed.** A human states how confident they are;
the graph computes how far out the pin sits and surfaces the two together. A detector may then ask a
question — *"this pin is 14 epochs out and carries confidence 0.95; is that right?"* — which is a
question, not a defect. What reflow2 must not do is compute a decay curve and assert a confidence
nobody stated. That would be inventing certainty, which is the failure this project exists to
prevent.

**Horizon is sequence distance by default.** `epoch_n − epoch_current` is always available and needs
no calendar. An epoch *may* carry an optional target date, and where it does, a calendar horizon is
also computable. This matters for the next section.

## This does not force `dec:plan-time-axis`

`dec:plan-time-axis` asks whether reflow2 models *scheduled* time — H-hour and offsets, durations,
windows, a critical path computed from dependencies. [planning-at-scale.md](planning-at-scale.md)
calls that the highest-value gap in the vocabulary after federation.

**Recommendation: keep them decoupled, and say so on the record.** The roadmap needs *ordering* and
*arrival*, which `PRECEDES` and the epoch sequence already give. It does not need durations or a
critical path. Deciding the roadmap should not silently decide the schedule question, and a roadmap
that shipped having quietly answered `dec:plan-time-axis` by drift would be exactly the mistake
BL-100 names.

## Arrival semantics

Arriving at an epoch is an operation, and most of it is already specified in
`req:plans-move-honestly`. Stated as a sequence:

1. **Compute the delta.** Planned-versus-delivered for every pin at this epoch, as a matter of
   course rather than on request.
2. **Requirement met.** Record it. The pin's modality moves from expected to recorded.
3. **Requirement missed entirely.** Its fate is asked, never defaulted (see 5).
4. **Requirement partially met.** Decompose it — `req:requirement-lineage`, so the split is on the
   record and both children know their parent. The satisfied child closes at this epoch. The
   remainder goes to 5.
5. **The one question that must be asked.** Transfer or discontinue. *Transferred* re-pins to a
   later epoch and is a recorded slip; *discontinued* is dropped on the user's word, with the
   reason, under `retire-from-design`'s discipline. The status vocabulary already carries both —
   `deferred` and `dropped`. Defaulting either way would decide by silence.
6. **A `REQUIRED` pin that was missed is a violation**, not a question — computed and reported, the
   same way `cap:kpp` reports a content violation today.
7. **Replanning is a dated event.** When a planned epoch's content moves, the plan's own change goes
   on the record, so *"what did we believe at the time?"* stays answerable. Never a silent re-pin.

Anthony expects further nuances here; this is an open list, not a closed one.

## KPP firmness has two dimensions, and they move independently

From `req:plans-move-honestly`, worth stating separately because it is the subtlest rule in the set:

- **Content is inviolable.** A KPP cannot be weakened to make a plan work. This is `cap:kpp` today.
- **Time may slip.** A KPP may move to a later epoch, and the slip is a recorded dated event.

The two are not the same firmness and must not be collapsed. A missed `REQUIRED` pin *with* a
recorded slip is a plan that moved honestly. The same miss *without* one is a violation. That
distinction is the whole point of recording the slip.

## Resolving `dec:epoch-roadmap-storage`

The decision has three candidates and has been amended three times in one day. The third amendment
does most of the work already.

**Recommendation: close it as (c-refined).** The graph owns the pins and the index; git stores any
rendered snapshot a human wants to hold; the expected-graph-at-epoch-N is derived, not stored. The
`rel:v0180 ↔ tag v0.18.0` pattern is this exact division of labour, already shipping, pointed
backward — the roadmap points it forward.

**The residual question answers itself.** The decision asks *"whether (b)'s in-graph planned content
is required for detectors to reason about the roadmap, or whether the index alone suffices."* Under
the satisfaction-schedule atom the question dissolves: the plan's content **is** the pins, and pins
are edges in the graph. There is no separate body of planned content for the index to stand in for.
`detect_gaps` and `propagate` reason over the pins natively. The index is the smaller thing it
always was — a map from epoch to git ref, for rendered snapshots only.

## The vocabulary is built and unreached

`ANTICIPATES` — the forward-looking edge whose `confidence` property this design needs — is used
**once** in 3,304 edges, and that single use carries empty properties. `AT_EPOCH` is used 156 times
and points backward in every one.

This is the project's recurring shape again: a designed feature with no computation reaching it.
Worth naming in the acceptance, because it means the schema work here is smaller than it looks and
the *computation* work is where the effort actually sits.

## Edges to add

The cluster is attached to the project and its author, but not woven to the design it depends on.
Proposed, in the graph's existing vocabulary — no new edge types:

| From | Edge | To | Why |
| --- | --- | --- | --- |
| `req:epochs-can-be-planned` | `GOVERNED_BY` | `dec:epoch-roadmap-storage` | The decision governs nothing today; it has only `AUTHORED_BY`. |
| `req:plans-move-honestly` | `DEPENDS_ON` | `req:epochs-can-be-planned` | They are a stated pair and currently unlinked. |
| `req:plans-move-honestly` | `DEPENDS_ON` | `req:inviolable-intent` | The KPP two-dimensional firmness rule rests on it. |
| `req:plans-move-honestly` | `DEPENDS_ON` | `req:requirement-lineage` | Partial satisfaction is decomposition; without it, step 4 has no mechanism. |
| *(new)* `cap:roadmap` | `SATISFIES` | `req:epochs-can-be-planned` | Nothing currently claims to satisfy it except a changelog view. |

**One existing edge to question.** `cap:changelog-view SATISFIES req:epochs-can-be-planned`. A
changelog is a derivable view of the graph's *delta* — backward-looking by construction. It is hard
to see how it satisfies a requirement about planning forward. Likely mis-targeted at capture time,
and worth confirming before acceptance rather than inheriting.

## What a later session must write

Ordered, once an agent is running with the graph actually served:

1. Confirm or drop the `cap:changelog-view → req:epochs-can-be-planned` edge.
2. Verify the derive-don't-store claim — that historical satisfaction epochs are computable through
   `ChangeEvent`/`AT_EPOCH` for the general case.
3. Close `dec:epoch-roadmap-storage` as (c-refined), with the residual question answered.
4. Record that the roadmap does **not** decide `dec:plan-time-axis`, so the coupling is refused
   explicitly rather than left ambiguous.
5. Accept `req:epochs-can-be-planned` and `req:plans-move-honestly`.
6. Add the edges above; create `cap:roadmap`.
7. **Register this document itself.** It was written in a session where the MCP server was bound to
   the wrong directory, so `link-artifacts` never ran and the loop nudge fired at the end with
   nothing able to answer it. Its closest sibling `docs/planning-at-scale.md` is registered as
   `art:planning-at-scale` (`artifact_type: document`, sha256 checksum, `status: realized`) and
   `REALIZES dec:live-tier`; this file should follow that shape and realize
   `dec:epoch-roadmap-storage`. Until then, as-built does not know it exists.
8. Run `detect_gaps` and `propagate` — the point of doing this in-graph rather than on paper.

## Still genuinely open

- Whether `Epoch` gains a series discriminator or increments become their own node type.
- Whether an epoch's optional target date is worth having before `dec:plan-time-axis` is settled.
- The remaining arrival nuances Anthony expects and has not yet named.
- How this meets **BL-68** — *"the roadmap is a risk-burndown schedule"*, roadmaps derived from
  readiness rather than declared. This document designs the *schedule*; BL-68 argues the schedule
  should be **computed** from where risk clears. Those must be reconciled, and BL-68 is the more
  ambitious claim.

## Domain neutrality

`req:design-anything` holds, and `dec:mosa-conformance` settled the pattern: the schema stays
domain-neutral and the domain words live in a projection. Nothing here adds "sprint", "release
train", "PI" or "increment review" to the vocabulary. A roadmap *view* may render any of those over
neutral epochs and pins. A schema that learned one planning dialect would have to learn them all.
