# Upgrading to v0.11.0

> Part of the **Reflow 2.0** design docs — see **[overview.md](overview.md)** for the map.

Two schema changes, one of them a **behaviour change to a shipped tool**. Both were friction found
by using reflow2 on itself (2026-07-25).

## 1 · A new Decision is `proposed`, not `accepted`

`Decision.status` now defaults to **`proposed`**. It used to default to `accepted`.

**Why.** Recording a choice is not the same act as settling it. With the old default, every open
question — a decision point held for later, a brainstormed option, an architecture nobody had
picked — landed as *settled and reasoned*. That is the forgery `dec:certainty-derived` forbids for
requirement status, with more consequence: an accepted Decision is what **where-am-i** reads back
to the user as "what you decided", what the fork layer treats as binding, and what the KPP
contradiction check reads as a trade already made. Six open questions recorded in one session each
needed correcting immediately afterwards, and the brainstorm skill had to carry the workaround in
prose. Recorded as `req:decision-status-not-asserted`.

**What breaks.** Anything that called `add_decision` and relied on the result being settled:

| If you were doing this | Do this instead |
|---|---|
| `add_decision(...)` and expecting `accepted` | `add_decision(...)` then `set_decision_status(id, "accepted")` |
| Recording an already-made decision | Same — two calls, the second one deliberate |
| Recording an open question | Nothing. It is now correct by default. |

**Existing graphs are unaffected.** A default applies at write time, so Decisions already stored
carry their status explicitly. Nothing is rewritten and no migration is needed.

**Two of reflow2's own tests failed on this**, and both were right to: each had leaned on the old
default instead of saying what it meant. A test about a *settled* decision now settles it.

## 2 · An Interface can be designated a published boundary

`Interface.designation` is new: **`internal`** (the default) or **`published`**.

`published` marks a contract others are entitled to rely on — what a systems-engineering ICD
publishes, and what MOSA calls a modular system interface (10 U.S.C. 4401). `internal` is plumbing
its owner may change freely. New tool: `set_interface_designation`.

**Default `internal` on purpose.** Publishing is a commitment; defaulting to it would assert one
nobody made — the same reasoning as the Decision change above.

**It is read, not just stored.** `propagate_from` now reports **`boundary_crossings`**: the
published Interfaces a change passes through, named rather than counted, in both the full radius
and the summary. Each impacted node also carries `crosses_published_boundary`.

That makes severability computable instead of asserted: if a change inside a part crosses none of
the design's published boundaries, the part is contained; if it does, the report says which
contract carried it, so you know whom to talk to. `req:key-interfaces`, `req:modularity-computed`.

**Nothing to migrate.** Existing Interfaces read as `internal`, which is the honest reading — none
of them had declared a boundary. Designate the ones that are real ICDs; the crossings appear the
moment you do, and disappear if you withdraw the designation, because the computation follows the
design rather than remembering it.

**What it is NOT.** A designation is not a claim that the boundary has held. Whether a published
contract actually stayed stable is its drift history — evidence, not a property anyone sets.

## Result shape changes

| Result | New field |
|---|---|
| `propagate_from` (full) | `boundary_crossings: [interface_id]`, and `crosses_published_boundary` per impacted node |
| `propagate_from` (summary) | `boundary_crossings: [interface_id]` |

Additive: nothing was removed or renamed.
