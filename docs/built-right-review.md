# Did we build this right? — a census of the design against itself

> Part of the **Reflow 2.0** design docs — see **[overview.md](overview.md)** for the map.
> Method context: **[sharpening.md](sharpening.md)**. Actionable items are filed as backlog rows.

**Run 2026-08-04, against `main` at `61aa6b1` (v0.23.0), on a graph of 1,348 nodes and 5,111 edges.**

Anthony's question was open-ended: *"had I known what I know of reflow2, would I have built it
differently or better?"* — schema, performance, structure, anything. This document is the answer,
and the discipline it holds itself to is the project's own: **measured, not asserted.** Every number
below is reproducible from two files this project maintains separately — `schema/*.yaml` and
`docs/design/reflow2.json` — with the commands given at the end.

**The one-line answer: the vocabulary is roughly twice the size of what anything writes, and the
gap is not evenly spread — it falls almost entirely on the half of the product that reflow2 does
not use on itself.** Nothing here is a crisis; most of it is the ordinary residue of a design that
grew faster than its own usage. But three findings are load-bearing, and one of them explains an
open blocker that self-hosting had failed to surface for eleven releases.

---

## 0. The generator, because it is reusable

Everything in this document came from one move, and it is worth naming because it is cheap and it
worked: **diff two artifacts the project maintains separately and expects to agree.**

- `schema/*.yaml` says what the design vocabulary *is* — 29 node types, 60 edge types, 208 properties.
- `docs/design/reflow2.json` says what the design vocabulary *is used for* — every node and edge
  reflow2 has ever written about itself.
- `crates/reflow2-mcp/src/**` says what an agent can *reach* — 142 tools, 125 request structs.

None of these three asks reflow2 a question, so none of them can be answered by the tool going
quiet. That matters: `sharpening.md` §1 warns that **reflow2's silence is never evidence reflow2 is
healthy**, and §5 concludes that genuine surprise only comes from external subjects. This census is
a partial counter-example worth recording — **it is a self-host generator that surprised**, because
it never consults the instrument under test. It compares two of its outputs.

---

## 1. Nine node types and thirty-five edge types have never been instantiated

```
declared:   29 node types · 60 edge types · 208 properties · 11 schema domains
never used:  9 node types (31%) · 35 edge types (58%)
```

Node types with zero instances: `Actor`, `Anchor`, `DimensionAssessment`, `DimensionObservation`,
`EnvironmentRule`, `Flow`, `QualityGate`, `ReadinessAssessment`, `TemporalFact`.

Read domain by domain, the shape is sharper than the percentage: **`dimensions` is entirely inert
(both node types, zero instances), and so are `environment`'s rule half, `readiness`'s assessment,
`temporal`'s `TemporalFact`, `verify`'s `QualityGate`, and `functional`'s `Flow`.** Six of eleven
domains have a dead centre.

**This is not new, it is [BL-184] generalised.** That row found `DimensionAssessment` count = ZERO
and, with it, that `dimension_drifts` — a slope computed over those assessments — had been reading
an empty set silently for eleven releases. The census says that was not a one-off: **it is the
expected condition of roughly a third of the vocabulary.** Any computation written over an unused
type inherits the same defect, and nothing in the build will say so.

**Would I have built it differently: yes, and this is the clearest case.** The eleven domains were
declared up front, as a vocabulary. Six of them have never been exercised by any real design —
reflow2's own or any trial's. A vocabulary declared ahead of a second real subject is a set of
promises the tool advertises through `describe_schema` and cannot keep. **The alternative is not a
smaller schema — it is the same schema grown on demand**, one domain per real design that needs it,
so that no type reaches `describe_schema` without at least one graph that uses it.

The honest counterweight, stated because it is real: **an unused type is not automatically a
mistake.** `EnvironmentRule` exists for regulated/deployment-heavy designs; `Flow` for behavioural
ones; reflow2 is neither. A design tool whose vocabulary covered only its own shape would be
useless. **The defect is not that they are unused — it is that nothing distinguishes "unused
because inapplicable here" from "unused because unreachable", and no computation over them is ever
challenged.**

---

## 2. Twenty-eight properties carry exactly one value — twenty-two of them the schema default

Across every populated node type:

```
28 properties have a single value on every instance
22 of those values are the schema default
```

The full list is reproducible; the ones that matter:

| Property | Value on every instance | Why it matters |
|---|---|---|
| `Requirement.concern` | `core` on 110/110 | An **11-value ility axis** — safety, security, cost, usability, environmental… — **read by no code in the core.** |
| `Requirement.provenance`, `Capability.*`, `Component.*`, `Fragment.*`, `Interface.*` | `authored` on **402/402** | The brownfield axis. See §3. |
| `Requirement.designation` | `internal` on 110/110 | `export_surface` withholds `internal` and carries `published` — so **`export_surface` has never had anything to carry.** |
| `Requirement.lineage` | `original` on 110/110 | `decomposed`/`derived` behave differently under delivery roll-up; neither has ever existed here. |
| `Verification.kind` | `verification` on 106/106 | **Zero validations, ever.** See §6. |
| `DriftEvent.drift_type` | `checksum_change` on 37/37 | **Seven of eight drift kinds have never fired.** |
| `Verification.status` | `passing` 103, `planned` 3 | **No `failing`, `skipped` or `blocked` verification exists in the record.** |

Two distinct problems are tangled here and they want different fixes.

**(a) A materialized default is indistinguishable from a choice.** These values are written into
every node and every export. `Capability.tier = operational` on all 112 is not a statement anybody
made; it is the default, serialized. **This quietly defeats at property level exactly the doctrine
the project enforces at requirement level** — `graph_report`'s "Requirement certainty" line exists
precisely to separate what a user confirmed from what an agent asserted, and `dec:certainty-derived`
argues that the distinction must be derivable rather than stored. At property level it is neither
derivable nor stored: the export cannot tell you whether anyone ever chose `core`. **Would I build
it differently: yes — do not serialize a value equal to the default.** An absent property would
then honestly mean "nobody said", which is what it means today in every case that matters.

**(b) `Requirement.concern` is an ility axis nobody wired up.** v0.23.0 shipped `ility_report`
([BL-184]), which computes quality axes by mapping existing findings onto nine dimensions. The
schema has carried a cross-cutting-concern enum on `Requirement` since the beginning — indexed,
eleven values, aligned to the ILS concerns an SME actually raises — and **no code reads it.** The
ility axis was designed twice, and the first design was never connected to anything. That is worth
saying plainly because it is the single cheapest correction available: `ility_report` currently has
nothing to say about `usability` or `safety` because nothing computes over them, while the field
that would let a user *declare* them has been there all along.

---

## 3. The brownfield half of the product has never been dogfooded

**`provenance` is `authored` on all 402 nodes that carry it — every Capability, Component,
Fragment, Interface and Requirement in the graph.** Not one `inferred`, `reconciled`, `healed`, or
`imported` node has ever existed here.

This is not cosmetic. `provenance` is load-bearing in at least five places:

- `report.rs:70` — `certainty_of()` returns `Recovered` only for `inferred`/`reconciled`/`healed`.
  **So the `recovered` bucket in the "Requirement certainty" line is structurally unreachable in
  this graph**, and the reading that exists to keep a summary honest has never demonstrated its
  honest case here.
- `report.rs:763` — a complete golden thread is counted as `inferred_only` rather than `delivered`
  when the requirement was recovered from the code that satisfies it. Never taken.
- `detect.rs:1195`, `heal.rs:408`, `preserve.rs:226` — three more branches, none exercised by the
  highest-volume real graph this project has.

**Per Anthony's own calibration, greenfield and brownfield are both first-class, and in his working
life brownfield is closer to the norm** — several organisations with competing, partially-built
designs, evolved on the fly. The **adopt** skill exists for exactly that, and adopt is the path
that writes `inferred`. **reflow2 dogfoods genesis mode every single day and adopt mode never.**

**Would I have built it differently: I would have made the self-host subject a brownfield one.**
reflow2's own graph was authored alongside its code, so it is a genesis design by construction —
which means the trial that runs thousands of times exercises the half of the product that was
already working, and the half aimed at the harder, more common case is exercised only by tests.
That is the same standing bias `sharpening.md` §5 already records about trials stopping at P2, one
level deeper: **not "the front half of the lifecycle", but "the front half of the product".**

The cheap correction is not a rewrite. It is to **keep one adopted graph in rotation as a standing
subject** — any repo nobody here wrote, run through `adopt`, re-run each release. Every branch above
would then have a live case.

---

## 4. ⭐ reflow2's own graph does not follow the skill reflow2 serves — and that is why [BL-176] survived

This is the finding I would most want on the record.

```
document/spec artifacts in reflow2's own graph ..... 35
  attached by REALIZES ............................ 35
  attached by DOCUMENTS or SPECIFIES ...............  3
  carrying REALIZES and NOT DOCUMENTS/SPECIFIES ... 32
  DOCUMENTS edges in the whole graph ...............  4
  SPECIFIES edges in the whole graph ...............  0
  REALIZES edges in the whole graph ............... 183
```

The **link-artifacts** skill — which reflow2 serves to every agent that touches it — is explicit:

> *A design doc, ADR, README, runbook or agent-instruction file (AGENTS.md, CLAUDE.md) **describes**
> the design instead: register it with `add_artifact` (artifact_type `document`) and link it with
> `documents` (+ `doc_kind`), **not** `realizes`.*

**reflow2's own graph does the opposite on 32 of its 35 document and spec artifacts** — `art:adopt-skill`,
`art:brainstorm-skill`, `art:collaborating`, `art:fleet-lessons`, `art:kit-pointer` and 27 more all
carry `REALIZES` and nothing else. `SPECIFIES` has never been written at all.

Now put that next to the open blocker. **[BL-176]**: `orphan_node` reports *"Artifact 'x' realizes
nothing"* for artifacts attached by `DOCUMENTS`/`SPECIFIES`. The field hit it at 26 documents —
defects 13 → 39, false-positive rate 46% → 82% — and **stopped work**, explicitly refusing the
workaround of adding a bogus `REALIZES` because it would be a lie at 756× scale.

**reflow2's own graph is that workaround, already applied, 32 times over.** Not deliberately —
nobody was hiding a warning — but the effect is exact: **the highest-volume trial this project has
was structurally incapable of surfacing [BL-176], because it never once did the thing that triggers
it.** Eleven releases of `0 gaps, 0 defects, loop clean` were true and uninformative on this point.

**The general lesson, and it belongs in `sharpening.md`:** self-host is blind wherever the self-host
graph does not follow the practice the skills prescribe. The instruments cannot see it, because
both sides of the comparison were built by the same hand. **The generator that finds it is a diff
of prescribed practice against actual practice** — grep what the skills tell an agent to do, then
check whether the repo's own graph did it. This one took a single query and explained a hard
blocker that had been invisible for months.

The corollary for the immediate work: **fixing [BL-176] is necessary but not sufficient.** Those 32
edges are also wrong on their own terms, and re-attaching them with `DOCUMENTS` is what would make
this graph capable of regression-testing the fix.

---

## 5. Two write surfaces that disagree, and the documented one is narrower

```
node types with a typed constructor ....................... 13
their declared properties ................................. 94
properties the typed constructor exposes .................. 42  (44%)
properties named by NO request struct in the whole surface . 67 of 208 (32%)
```

Every skill prescribes the typed path — `add_requirement`, `add_capability`, `add_artifact`. That
path reaches under half of each type's declared properties. The rest need a dedicated setter
(`set_requirement_status`, `set_artifact_checksum`, …) or the generic `create_node`, which takes an
arbitrary schema-validated property map and which **no skill prescribes**.

So the effective schema — what agents actually write — is defined by constructor signatures, not by
`schema/*.yaml`. Two known rows are symptoms of this one cause:

- **[BL-129]** — `Interface.medium` unreachable from both tools that create and specify Interfaces.
- **[BL-183]** — 16 of 18 constructors silently reset every property they did not name, *and the
  served revise-design skill prescribed the call that did it*. That bug is only possible when a
  constructor's field list is a subset of the type's properties.

**Would I have built it differently: one write path per node type, taking the full property map,
validated by the schema — with ergonomic arguments as sugar over it, never as a second surface.**
The typed constructors were an ergonomics decision that hardened into a lossy second schema. The
generic `create_node` is the honest surface and it is the one nothing documents.

---

## 6. Where the frontier, the weakest-modelled type, and the field report all meet

`maturity_report` names **seams** the frontier: **0%, 0 of 73 component couplings declared through a
contract**, across 23 releases. That is not three facts, it is one fact seen from three sides:

- **The band** — seams 0/73.
- **The type** — `Interface` has 3 instances and **5 of its 14 properties (`operations`,
  `payload_schema`, `error_model`, `endpoint`, `description`) are set on none of them.** The fields
  that make a seam checkable exist and have never been filled.
- **The field** — [BL-193], filed today: an Interface CONSUMED across a component boundary with no
  spec fires no gap, so a release can change every signature behind it and the loop stays green.

And one more, which is the note I would end on. **`Verification.kind` is `verification` on 106 of
106 nodes. Not one `validation` has ever been recorded.** The schema draws the classical
distinction — *did we build it right* versus *did we build the right thing* — and this project has
only ever recorded the first. Anthony's question was the verification one, and it has a good
answer. **The validation question is the one the graph has no instance of.**

---

## 7. Performance is not the binding constraint, and I would not spend effort there yet

Stated plainly because "would we build it better" invites optimisation work that the evidence does
not support:

- The graph is **1,348 nodes / 5,111 edges**. The queued ~756-document corpus ingest takes it to
  roughly 2,100 nodes — **1.6×** — and touches only per-artifact linear paths.
- The one superlinear detector I found is `detect_possible_duplicates`, **O(C²) over Components
  only** — 54 components, 1,431 pairs — bounded by components, not by artifacts, so the corpus
  ingest does not touch it.
- **The ingest's real risk is [BL-176]'s noise (82% false-positive defect list), not speed.**
  Optimising runtime now would be optimising the wrong axis.

**Method caveat, stated because the project's standard demands it: this is read from the code's
complexity, not measured by timing.** Only a debug binary exists in the tree, and timing a debug
build would inflate constants by an order of magnitude and prove nothing about the shape. A real
timing rig against a release build, at 1×/4×/8× synthetic scale, is worth building **before** any
ingest much beyond the corpus — not before this one.

One structural note that is not performance but is architecture: **the `service.rs` split
([BL-181]) is deliberately half-done.** 142 tools now live in 12 domain modules; **all 125 request
structs stayed in `service.rs`.** A module owns its behaviour but not its wire types. That was a
recorded, reasoned deferral, not an oversight — recording it here because the census walks straight
into it, and because the deferred half is the part that makes the module boundary real.

---

## 8. What I would actually change, in order

1. **Stop serializing defaults** (§2a). Cheapest, and it makes "nobody said" expressible everywhere.
2. **Re-attach the 32 document artifacts with `DOCUMENTS`** (§4) — after [BL-176] lands, since today
   that is what firing the false positive looks like. This is what makes the fix testable here.
3. **Keep one adopted, brownfield graph in standing rotation** (§3). Five dead code branches get a
   live case, and the half of the product aimed at Anthony's actual working conditions gets a trial.
4. **Wire `Requirement.concern` into `ility_report`, or retire it** (§2b). Two designs of one axis,
   one of them connected to nothing.
5. **Collapse the two write surfaces** (§5) — one full-property path per type, ergonomics as sugar.
6. **Grow the schema on demand, not ahead of a subject** (§1) — and until then, make an unexercised
   type visible rather than silent, so no computation quietly reads an empty set again.

Items 1–5 are filed as backlog rows. Item 6 is a standing judgement about how the vocabulary should
evolve and belongs to Anthony, not to a row.

---

## Reproducing this

All figures come from the repo at `61aa6b1`; nothing here needs a running server.

```bash
# vocabulary declared vs instantiated (§1)
python3 - <<'PY'
import yaml, glob, json, collections
sn, se = {}, {}
for f in glob.glob('schema/*.yaml'):
    s = yaml.safe_load(open(f))['schema']
    sn.update({k: s['name'] for k in (s.get('node_types') or {})})
    se.update({k: s['name'] for k in (s.get('edge_types') or {})})
d = json.load(open('docs/design/reflow2.json'))
un = collections.Counter(n['node_type'] for n in d['nodes'])
ue = collections.Counter(e['edge_type'] for e in d['edges'])
print('node types never instantiated:', sorted(k for k in sn if k not in un))
print('edge types never used:', sorted(k for k in se if k not in ue))
PY

# single-valued properties, and whether the value is the default (§2)
# document/spec artifacts attached by REALIZES rather than DOCUMENTS (§4)
# typed-constructor property coverage (§5)
#   — the same three-way diff: schema/*.yaml × docs/design/reflow2.json × crates/reflow2-mcp/src/**
```

The three inputs are the point. Any two of them agreeing proves nothing; it is where they disagree
that a finding lives.
