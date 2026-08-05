# Scope: pointing reflow2 at a folder of documents

> Part of the **Reflow 2.0** design docs — see **[overview.md](overview.md)** for the full map and
> reading order.

> **BUILT 2026-08-05 — [BL-186]. `cap:corpus-ingest` is `verified`.** Every step of the sequence
> below now exists: coverage first, the Decision and Verification passes, per-type thresholds, the
> SP-3b handshake (`ingest_step`), and the folder driver itself as **`ingest_corpus_step`** over
> `crates/reflow2-core/src/corpus.rs`, with the walk in the served **`ingest-corpus`** skill.
>
> **Open decision 1 resolved to (b), as this doc recommended** — the handshake, not a skill doing
> the disciplines by hand. Two things this doc did not anticipate, both recorded on the capability:
> **the handshake batches ACROSS documents**, so a corpus costs the ~3 rounds one document costs
> rather than 3N (no new mechanism — a prompt id was already a content hash); and **re-running is
> the resume path**, because a document whose `fragment_id` already exists comes back `skipped`
> rather than failed. Finding 1's quadratic `fuzzy_match` is **still not addressed** and is still
> the right thing to measure — but the dominant cost turned out to be agent round trips, which is
> what the batching attacks.

*Scoping only when written — the findings below are preserved as they stood, and the sections that
have since been overtaken carry their own BUILT notes. `cap:corpus-ingest` was `planned` ([BL-97](backlog.md)).
Written 2026-07-26 by reading `ingest.rs`, `graph.rs` and the schema, not from memory. The reading
changed the shape of the work in both directions: one hard part is already solved, and a different
one was not on the list at all.*

## What we are trying to buy

Someone has a large body of markdown — specifications, notes, test records, decisions — accumulated
over years. They want to point reflow2 at the folder and get back one coherent design, with every
claim traceable to the document it came from.

The ask, in the requester's own words: *requirements, test results, designer intent — basically
whatever is in the documents.* Hold onto that list; **Finding 2 is that reflow2 currently extracts
the first of those three and neither of the others.**

## Finding 1 — cross-document identity is already built

BL-97 called this "the hard part" and estimated the folder driver as wiring on top of it. That was
half right, and backwards.

`ingest`'s `fuzzy_match` (`ingest.rs:916`) resolves an extracted node against **every existing node
of that type in the graph** — `self.scan_nodes(node_type)`, not a per-document buffer. So ingesting
forty specs one after another *already* converges: document two's "Auth Service" resolves onto
document one's node rather than duplicating it. The convergence machinery is not missing. It is
present, tested, and it reports what it did (`IngestReport.fuzzy_merges`, "auditable, never
silent").

Time-aware resolution comes with it and matters more at corpus scale than at single-document scale:
a node that already exists and has *changed* is snapshotted and given a `ChangeEvent` before the new
content lands — never a silent overwrite. Re-running a corpus after the source folder moves on
therefore produces a diffable history rather than a clobber.

**What is actually missing is cost, not capability.** `scan_nodes` is a full scan of a type, and
`fuzzy_match` runs it **once per extracted node**. For a handful of nodes that is free. For a corpus
producing thousands, it is quadratic: every new requirement re-reads every requirement already
stored. This is the batch-scale problem BL-97 gestured at, and it is an indexing problem with a
known shape, not a research problem.

## Finding 2 — the passes cover about a third of the ask

This is the finding that should drive the plan, and it was not in BL-97 at all.

`ingest` creates exactly eight node types: **Project, Requirement, Constraint, Capability,
Component, Interface**, plus the provenance `Fragment` and a `DesignEpoch`.

| The requester asked for | reflow2 extracts it today |
|---|---|
| Requirements | **yes** |
| Test results | **no** — `Verification` has no extraction pass |
| Designer intent / rationale | **no** — `Decision` has no extraction pass |
| Also absent | `Artifact`, `Actor`, `Flow`, `Resource`, `DimensionAssessment` |

The module doc is honest about this — it says the increment "implements the spine" and lists
"flows, actors, decisions, artifacts, resources, inference, dimensions, changes" as deferred. But
the consequence for this request is concrete: **a corpus run today would silently return a design
with no test evidence and no recorded rationale**, from documents that contain both. The user would
see a plausible-looking result and have no way to tell that two thirds of what they pointed at was
never looked for.

Two of those passes are worth far more than the others for this use case. `Decision` is the one that
makes an old corpus worth ingesting at all — *why* something was built the way it was is exactly
what is lost when the people leave. `Verification` closes the loop reflow2 already reasons about
(`unverified_capability`, `build_without_verification`) and is the difference between a design that
claims things and one that can show them.

## Finding 3 — the real blocker is that `ingest` is unreachable

`ingest` takes `&dyn LlmBackend`. reflow2 deliberately ships **no provider backend**
(`docs/interaction-surfaces.md`; the agent-native decision says the ambient coding agent *is* the
LLM). In practice `ingest` runs only against `MockLlmBackend`, and **there is no ingest tool on the
MCP surface** — none of the 108 served tools is an ingest. So today the only way a user's documents
become a design is the `adopt` skill, where the *agent* reads the files and writes nodes with the
ordinary `add_*` / `create_*` tools.

That leaves a real fork, and it is the one decision that changes everything downstream:

**(a) Agent-driven corpus ingest.** A skill walks the folder; for each document the ambient agent
extracts and calls the existing write tools. No provider dependency, no change to the deferral,
consistent with every other reflow2 skill. The extraction quality is the agent's, which is the same
bargain the whole product already makes. The cost: the disciplines `ingest.rs` enforces in code —
never cascade-fail, no phantom edges, provenance on everything, time-aware resolution — become
*instructions* the agent is asked to follow, and instructions are weaker than a type system. This is
the failure `docs/sharpening.md` names: shaping the model until the tool goes quiet.

**(b) Make core `ingest` reachable.** Put it on the MCP surface with the *agent* as the backend —
an MCP handshake where reflow2 returns the extraction prompts and the agent returns the JSON
(this is SP-3b, already specified and deferred). Keeps every discipline in Rust where it is
enforced, adds no provider dependency, and is the only option that makes the corpus path
*mechanically* honest rather than behaviourally honest.

(b) is more work and is the one I would build. It is also the option that makes Finding 2's missing
passes worth writing, because a pass written in Rust is reusable by every future ingest; a pass
written into a skill is prose.

## Finding 4 — the schema's resolution declaration is read by nothing

BL-97 says the convergence primitive "already exists — every node type declares its own resolution
strategy and threshold in the schema (`resolution: { strategy: fuzzy_then_vector, fuzzy_threshold:
80–85 }`)". Those declarations exist. **Nothing reads them.** `ingest` uses a hardcoded
`FUZZY_MATCH_THRESHOLD: u32 = 90` and `token_sort_ratio` only; the `fuzzy_then_vector` strategy's
vector half is deferred, and the per-type threshold is ignored in favour of one global number.

That is the **fifth** instance of this project's recurring shape — a field declared, written into
the schema, and consulted by no computation. The temporal axis before BL-70, the inviolable-intent
vocabulary before BL-96, `DriftEvent.resolved` before BL-35, `Verification.last_run_at` today
([BL-106](backlog.md)). It is worth fixing here rather than noting again: a corpus is exactly where
a per-type threshold earns its keep, because `Requirement` and `Component` do not deserve the same
similarity bar.

> **BUILT 2026-07-26.** `dynograph-core` — the version reflow2 is *already pinned to* — parses a
> `ResolutionConfig` onto every node type carrying **two** thresholds, `fuzzy_threshold` and
> `auto_merge_threshold`, and `DesignGraph::schema()` is already public: the reader existed, only
> the call site was missing. `ingest` now reads both.
>
> **The fault was not where this section first said it was.** The foundation's *default*
> `auto_merge_threshold` is **90** — exactly the constant reflow2 had hardcoded. So the merging half
> was accidentally correct all along, and re-reading it from the schema changes nothing about what
> merges (pinned by a test). What was missing was the **band below it**: a near-match scoring
> between a type's `fuzzy_threshold` (80–88) and 90 was silently created as a second node, with
> nothing reported. Measured: **"Auth Service" vs "Authentication Service" scores 84** — the
> canonical corpus case sat in that invisible band. It is now reported as a `merge_candidate` and
> still created, so nothing is decided by arithmetic and nothing is lost.

## Finding 5 — auto-merge at 90 versus `dec:ask-not-repair`

`dec:ask-not-repair` says suspected duplicates are **asked, never silently merged**. `ingest`
merges at `token_sort_ratio ≥ 90` and records it. At single-document scale that is defensible: 90 on
a token-sorted ratio is near-identity, and the merge is reported. At corpus scale the same rule
quietly decides thousands of times, and the failure is directional — *"Auth Service"* and *"Auth
Service v2"* score high, and collapsing them loses a distinction the corpus was trying to record.

The counter-pressure is real too: BL-42's lesson is that a detector which punishes correct bulk work
needs a different question, not a tuned threshold, and asking about every near-match across 500
documents is unusable.

**This needs a decision, not a default.** A defensible shape: keep auto-merge for the very high
band, route the ambiguous band into a *batched* question (one prompt listing N candidate pairs,
answered once), and never merge across a band boundary without a record. That is a design question
about where the bands sit, and it is the user's call.

> **Answered in shape, 2026-07-26, by
> [storyflow-resolution-nuggets.md](storyflow-resolution-nuggets.md).** storyflow has fought this in
> production for years and its answer is exactly the two-band model above, declared per node type —
> below `fuzzy_threshold` ignore, between the two **ask**, at/above `auto_merge_threshold` act. That
> satisfies `dec:ask-not-repair` precisely: the rule governs the middle band, and the top band is not
> a suspicion. It narrows this open question from *what shape* to *which numbers*.
>
> The study also adds a mechanism this scope missed entirely: ratio scoring is **length-sensitive**,
> so `Auth` versus `Authentication Service` — the single most common corpus case — scores below any
> threshold you choose. No tuning fixes it; it needs a structural **token-subset** test, with the
> longer, more specific name surviving. Without that pass the bands alone will not do the job.

## Provenance and epochs at corpus scale

Both already behave correctly and are worth stating so nobody re-solves them:

- **One `Fragment` per document, enforced.** Reusing a `fragment_id` is refused up front, before any
  write, because it would overwrite the prior run's Fragment and reopen its epoch (BL-58). A folder
  driver must therefore mint a Fragment per document — which is exactly the provenance the user
  wants: every claim's `YIELDED` edge points back at the file it came from.
- **Epochs need a deliberate choice.** If `epoch_id` is unset and any node evolves, ingest opens
  `epoch:{fragment_id}`. Across 500 documents that is up to 500 epochs. A corpus run should pass one
  epoch for the whole run, so the history reads as *"the corpus was ingested"* rather than as five
  hundred unrelated events.

## What this buys, and what it does not

**Buys:** a folder becomes a design, with per-document provenance, cross-document identity, and a
re-runnable pass that records what changed rather than clobbering it.

**Does not buy:** any guarantee that the design is *complete* with respect to the folder. That is
[BL-95](backlog.md), and it is the necessary companion here rather than a nice-to-have — without a
coverage measure, a run that understood 30% of the corpus reports the same "0 gaps" as one that
understood all of it, and the user's confidence would be unearned. **A corpus ingest that cannot say
what it did not understand should not ship.**

## Open decisions

1. **(a) agent-driven skill or (b) the SP-3b handshake?** Recommend (b). Everything else depends on
   this.
2. **Which extraction passes, in what order?** Recommend `Decision` then `Verification` — they are
   the two the requester named and the two reflow2 already reasons about.
3. **Where do the auto-merge bands sit** (Finding 5), and is the ambiguous band batched?
4. **Does coverage (BL-95) ship with it or before it?** Recommend with it, as one deliverable.

## Sequence

1. **Coverage first or alongside** — [BL-95](backlog.md)'s `coverage_report`, so a thin pass is
   measured rather than felt.
2. **The `Decision` and `Verification` extraction passes**, in Rust, behind the existing pass
   machinery. Independently useful even for single documents.
3. **Per-type resolution thresholds** read from the schema instead of one constant (Finding 4), plus
   an index so `fuzzy_match` stops re-scanning (Finding 1).
4. **The SP-3b ingest handshake** on the MCP surface, if decision 1 goes to (b).
5. **The folder driver** — the smallest piece, and deliberately last: it is a loop over 2–4 with one
   epoch, one Fragment per file, and a report that names every document it could not parse.

## What I could not verify from here

- **Extraction quality on a real corpus.** Every claim above is about mechanism. Whether an agent
  reading 500 heterogeneous markdown files produces a design a human recognises is a trial, not an
  argument, and it should be run on a real folder before anyone promises this to a user.
- **Where the cost actually bites.** The quadratic scan is visible in the code; at what corpus size
  it stops being acceptable is a measurement nobody has taken.
