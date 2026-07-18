# Artifact Linking — connecting real files to graph entities

> Part of the **Reflow 2.0** design docs — see **[overview.md](overview.md)** for the full map and reading order.

> **Status (SP-6):** the **write side is built, link-only** — `DesignGraph::{add_artifact,
> realizes, link_artifact}` (`src/artifact.rs`) + the `add_artifact` / `realizes` / `link_artifact`
> MCP tools + the `link-artifacts` consumer skill materialize `Artifact` nodes and `REALIZES`
> edges with provenance (recorded on a `Fragment` that `YIELDED` the Artifact). As-built **drift
> detection** (the reconcile pass + `DriftEvent`) described below is implemented in `drift.rs` (SP-6b),
> with observations supplied by the caller rather than read from disk by the core;
> `SPECIFIES`/`DOCUMENTS`/`PRODUCES` edges are reachable via the generic `create_edge` tool.

How reflow2 ties the *actual work products* — code, design docs, dataflow charts,
OpenAPI specs, test results — to the design entities they serve. This is the software
analogue of storyflow's fragment→node linking, adapted for a developer audience.

## How storyflow does it

A writer creates a **`Fragment`** — a chunk of prose stored *inline* (`content`,
fulltext-indexed for search). Extraction turns the prose into entities, and integration
draws **`MENTIONS`** edges (Fragment → Character / Location / Event / …, each with a
`salience` 0–1) plus **`EXPLORES`** (Fragment → Concept). Result: every entity traces back
to the fragment(s) that introduced it, and the fragment is both the content store and the
provenance anchor. "The bridge from prose to the graph."

## reflow2's two building blocks

reflow2 keeps the same idea but separates *the unit of work* from *the durable file*,
because a developer's artifacts (a 2,000-line module, a binary, a CAD model) shouldn't be
inlined into a graph node:

| | `Fragment` | `Artifact` |
|---|---|---|
| **is** | a unit of *ingested work* — a code snippet, a doc section, a review note, a chat turn | a durable, addressable *deliverable* that lives outside the graph |
| **content** | small snippets inline; larger via `content_ref` (path/hash) + indexed `summary` | never inlined — `location` (path / URI / content-hash) + indexed `description` |
| **role** | the extraction/provenance anchor | the thing the design actually produces |
| **links via** | `YIELDED` → every node it created/updated (reflow2's `MENTIONS`) | `REALIZES` / `SPECIFIES` / `DOCUMENTS` / `PRODUCES` → the entity it serves |

A typical chain: a developer pastes a Python script → a `Fragment` is ingested →
it `YIELDED` an `Artifact{artifact_type: code, location: "src/auth/token.py"}` →
that Artifact `REALIZES` the `Capability` *validate_token*.

## Linking each artifact type to its entity

| Real thing | `Artifact.artifact_type` | Edge → entity |
|---|---|---|
| Python / bash / source file | `code` | **`REALIZES`** → `Capability` / `Component` |
| OpenAPI / protobuf / JSON-schema | `spec` | **`SPECIFIES`** → `Interface` (its authoritative contract; `format: openapi`) |
| Design doc / ADR / README | `document` | **`DOCUMENTS`** → `Component` / `Decision` / `Project` |
| Dataflow / sequence / arch diagram | `diagram` | **`DOCUMENTS`** → `Flow` / `Component` (`doc_kind: dataflow`) |
| Test report / coverage / sim output | `test_result` | **`PRODUCES`** ← `Verification` (`outcome: pass/fail`) |
| CAD model / drawing | `model` / `drawing` | **`REALIZES`** → `Component` / `Artifact` |
| Config / IaC | `config` | **`REALIZES`** → `Component` / `Release` |

Three verbs keep the intent unambiguous:
- **`REALIZES`** — the artifact *is* the implementation (executable/buildable).
- **`SPECIFIES`** — the artifact *defines the contract* (machine-readable, authoritative).
- **`DOCUMENTS`** — the artifact *explains* (human-facing, non-authoritative).
- **`PRODUCES`** — a `Verification` *emitted* the artifact (evidence of a run).

## How the links get made (two paths)

1. **Extraction-driven (forward).** Ingest the file/snippet as a `Fragment`; the extraction
   + graph-informed resolution identifies which existing `Capability` / `Component` /
   `Interface` it serves (fuzzy + vector match) and creates the `Artifact` + the right edge.
   New target entities are created if none match. Provenance `authored`.
2. **Reverse-engineering / as-built (backward).** AST-scan a repo (or parse an OpenAPI file,
   a CAD BOM, a JUnit report) → discover functions/endpoints/parts → materialize `Artifact`
   nodes → match them to the as-designed entities → `REALIZES`/`SPECIFIES` edges. This
   produces the **as-built** fidelity view and powers drift detection: an as-designed
   `Capability` with no `REALIZES`-ing Artifact is a build gap; an Artifact realizing
   nothing is an undocumented addition; a mismatch is a `DriftEvent`.

`Artifact.location` is the pointer of record; a reconcile pass confirms the file still
exists and matches, emitting a `DriftEvent` when the code and the graph diverge.

## Why this matters

For a software team, this is what makes the graph *live*: the OpenAPI spec, the module that
implements it, the sequence diagram that documents the flow, and the test report that
verifies it are all one connected subgraph around the `Interface`/`Capability` they share.
Change any one and impact-propagation walks the rest — the coherence loop, at the artifact
level.

## The note layer — intent, pseudocode, review (author/director notes)

storyflow lets an author attach **author notes** and **director notes** *side-by-side* with
a narrative fragment (its `SceneDirection` node, `note_kind ∈ {author, director}`, linked by
`DIRECTS`). The coding analogue is rich:

- the **code fragment** (the implementation) sits beside its **intent** — the author note
  explaining *why*;
- **pseudocode** is an author note written *before* the implementation;
- **review / feedback notes** are a reviewer's commentary on the code.

As you put it: *both the actual code and the author notes are Fragments, but serve two
purposes.* So reflow2 keeps both as `Fragment`s, split into two layers:

| Layer | `fragment_type` | Role |
|---|---|---|
| **Primary content** | `implementation` / `test` / `spec` / `config` | the work product; `YIELDED`s graph entities |
| **Note layer** | `note` (intent) / `pseudocode` / `review` / `feedback` | commentary that `ANNOTATES` the primary fragment / Artifact / entity |

The **`ANNOTATES`** edge (Fragment → Fragment / Artifact / any entity) is the reflow2 analog
of storyflow's `DIRECTS` — it hangs the note beside the thing it's about, carrying
`note_kind` (`author` / `reviewer` / `director`) and a `resolved` flag for review comments.
Three linking verbs now cover the three provenances of commentary:

- **`ANNOTATES`** — a *human* note (intent, pseudocode, review). `provenance: authored`.
- **`SUPPLEMENTS`** — a *machine* SME analysis. `provenance: inferred`.
- **`YIELDED`** — extraction *output* (entities the fragment produced).

Two quick notes on overlap: a fragment's *own* one-line intent can also live inline on
`Fragment.intent`; the `note`/`pseudocode` fragment + `ANNOTATES` is for standalone or
richer commentary (and for a *reviewer's* note on someone else's code). And review comments
gain a `resolved` flag so gap-surfacing can ask about open feedback.

