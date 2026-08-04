# The increment plan — what we do about everything now on the board

> Part of the **Reflow 2.0** design docs — see **[overview.md](overview.md)** for the map.
> Sources: the `dev_storyflow` field report (BL-186–BL-197) and the self-census
> ([built-right-review.md](built-right-review.md), BL-198–BL-205).

**Written 2026-08-04, at Anthony's ask: map the findings onto increments/versions.** Twenty-one
open rows arrived within a day of each other from two independent directions — a real user hitting
a real corpus, and reflow2 measured against itself. This is the plan for spending them.

**Nothing here is accepted.** The epochs are recorded as **planned** (`plan_epoch`), which is a
claim about the future, not a commitment. Promoting any row to a Requirement or Capability is
Anthony's signature, per AGENTS.md. Recommendations are marked as such.

---

## The one rule that shaped the grouping

**A schema change is never cheap and never solo.** It pulls the minor bump, the upgrade doc, and
the foundation-migration checklist — so two schema changes in one increment cost barely more than
one, and two schema changes in *different* increments cost double. Every grouping below is
schema-clustered first and thematically second.

The second rule: **an increment should be a sentence a user recognises**, not a bucket of tickets.

---

## v0.24.0 — "The document corpus can actually be ingested"

**The unblock. A real user has stopped work; this is the increment that lets them start again.**

| Row | What | Size | Schema? |
|---|---|---|---|
| **BL-176** | `orphan_node` counts attachment, not `REALIZES` | S | no | ✅ **DONE** (#68) |
| **BL-199** | re-attach reflow2's own 32 document artifacts with `DOCUMENTS` | S | no |
| **BL-188** | artifact granularity intent — `opaque` vs `pending_expansion` | M | **yes** |
| **BL-191** | artifact volatility — `stable` vs `append_only`/`living` | S | **yes** |
| **BL-186** | build `cap:corpus-ingest` (accepted requirement, accepted mechanism, unbuilt) | M | no |
| **BL-196** | `genesis` asks where design artifacts will live | M | no (skill) |

**Why these together.** BL-188 and BL-191 are both new **`Artifact` properties** — one schema
touch, one bump, one upgrade doc, instead of two. And they are precisely what an ingest needs:
BL-188 is how you know where a resumed ingest stopped, BL-191 is how a living document stops
firing `checksum_change` forever.

**The order inside the increment is forced, not preferred:**
`BL-176` (ingest at all) → `BL-199` (re-attach, which is *also* what makes BL-176 regression-testable
here) → `BL-188` (know where you stopped) → `BL-186` (the build).

**BL-196 rides along** because it is the same subject — custody — and is a skill question with no
code: `genesis` already asks about deployment and platform, and should ask where artifacts will
live while it is still cheap to answer.

---

## v0.25.0 — "The loop tells the truth about what it is owed"

**No schema. All message and reporting quality. The highest friction removed per unit of effort,
and the increment most likely to be felt on day one.**

| Row | What | Size |
|---|---|---|
| **BL-205** | `loop_status` returns 72,886 chars and cannot be read — 99.6% is `verifications` | S |
| **BL-194** | `open_questions` returning 0 reads as an all-clear; name the other counts | S |
| **BL-192** | enum rejections list the legal values; field errors name the spelling for that position | S |
| **BL-195** | `disconnected_community` proposes the bridge a ruling forbids | S |
| **BL-201** | `Requirement.concern` — wire the ility axis in, or retire it | S |
| **BL-203** | make an unexercised type visible instead of silently reading an empty set | M |

**Why these together.** Every one is *"the tool said something true and unhelpful"*. Five are S.
BL-203 is the generalisation of BL-184 and belongs here rather than in a schema increment, because
the actionable half is a **report**, not a vocabulary change.

**BL-201 is the cheapest strategic win on the whole board:** an eleven-value quality axis has been
sitting on `Requirement` since the beginning, indexed, read by nothing, while `ility_report` was
built to compute quality axes from scratch.

---

## v0.26.0 — "Status accounting: when was this true?"

**The biggest conceptual gap either source found, and EIA-649 names it. Schema increment.**

| Row | What | Size | Schema? |
|---|---|---|---|
| **BL-187** | external anchor — `anchor_kind` + `anchor_ref` on `TemporalFact`/`Verification`/`Artifact` | M | **yes** |
| **BL-185** | a test written from the requirement vs one written to match the code | M | **yes** |
| **BL-189** | lifecycle retirement — 95 self-corrections against 7 retirements | M | maybe |

**Why these together.** BL-187 and BL-185 both add properties to **`Verification`** — one schema
touch. And they are the same question from two sides: BL-187 asks *when* a check was true, BL-185
asks *what it was evidence of*. BL-189 is the retirement half of the same hole — a record that has
outlived its subject and nothing says so.

**The measurement that justifies the increment:** half the commits one evidence corpus pins to are
**no longer ancestors of main** (squash-merge ate them; median pin 117 commits behind), and 11 of
this project's own verifications sat green on a single two-day-old batch run. Both corpora record
*when* something was true and nothing computes whether it still is.

**Carry the caution from BL-185 into the build:** record the fact, report the signal, **never grade
it** — the `adopt` path necessarily produces tests written after the code, and scoring that as
failure would repeat BL-179's punish-correct-work trap.

---

## v0.27.0 — "Seams" — the frontier the instruments already name

| Row | What | Size | Schema? |
|---|---|---|---|
| **BL-193** | an Interface CONSUMED across a boundary with no spec fires no gap | M | no |
| **BL-190** | stewardship/accountability edge (role → node), NOT the decomposition spine | M | **yes** (edge) |

**Why now and not earlier.** `maturity_report` puts **seams at 0% — 0 of 73 component couplings
declared through a contract, across 23 releases** — and names it the frontier. The census found the
same fact from a third side: `Interface` has 3 instances and the 5 properties that make a seam
checkable (`operations`, `payload_schema`, `error_model`, `endpoint`, `description`) are set on
**none** of them.

**BL-190 joins it because it is the other structural-vocabulary gap** and a new edge type is a
schema change; pairing it with the seam work keeps that to one bump. The fleet's ownership matrix
is currently inexpressible, and `CONTAINS` — the only modelled Component→Component fit — is the
decomposition spine, so using it would corrupt `hierarchy_issues`.

---

## v0.28.0 — "One honest write surface"

**The largest and least urgent. Its own increment because BL-202 is L and moves the toolsnaps.**

| Row | What | Size |
|---|---|---|
| **BL-202** | typed constructors reach 44% of declared properties — collapse the two write surfaces | L |
| **BL-198** | stop serializing schema defaults, so a default and a choice are distinguishable | M |
| **BL-197** | two served descriptions describe a world that changed | S |
| **BL-204** | a skill asserts tool behaviour that changed, and nothing checks that | S |

**Why last.** Nothing here blocks anyone, and BL-202 is the one item on the board that would move
the served surface. **BL-198 needs care before it is built, not after:** `compare_designs` and the
`merge=reflow2` driver both compare property maps, so suppressing defaults changes what *changed*
means to both. That cost should be measured, not assumed.

**BL-204 pairs with BL-197** — both are "the words we ship stopped matching the code we ship", and
the valuable half of BL-204 is not the sentence but the check that would have caught it.

---

## Not an increment, and deliberately so

**BL-200 — keep one adopted, brownfield graph in standing rotation.** `provenance` is `authored` on
**402 of 402** nodes, so five code branches and the entire `adopt` path are exercised only by tests.
This is a **standing practice**, not a build: any repo nobody here wrote, run through `adopt`,
re-run each release. It belongs in `sharpening.md`'s regimen, not in a version.

It is also the highest-leverage item on this page. **Every increment above would be tested against
one kind of graph; this is what makes them tested against two.**

---

## What this plan does not answer

- **Whether the retrospective exercise becomes a skill** — `dec:idea-retrospective-review-skill` and
  `dec:idea-retrospective-asks-or-computes`, both `proposed`, both unranked. Anthony has not chosen.
- **Whether the schema should have been grown on demand** rather than declared across eleven domains
  up front (BL-203's other half). That is a standing judgement about how the vocabulary evolves.
- **Any release date.** The epochs are `planned`, which per `req:plans-move-honestly` is a claim
  carrying confidence, not a commitment — and confidence is worth less further out.

## A note on this page's own method

The vocabulary this plan uses — planned epochs — was **built and unreached**: `plan_epoch` has
existed since `req:epochs-can-be-planned` and the census found `DesignEpoch` carried 49 instances
with **not one of them planned**. Recording these five as planned epochs is the first use. That is
deliberate: a plan written only in Markdown is exactly the "index that must be read in full"
failure BL-196 measured, and this project should not build the fix and then not use it.
