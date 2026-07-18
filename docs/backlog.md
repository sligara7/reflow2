# Backlog — what's open, and why

> Part of the **Reflow 2.0** design docs — see **[overview.md](overview.md)** for the map.

Three records, three questions:

| Record | Answers |
|---|---|
| [requirements-coverage.md](requirements-coverage.md) | *Are we meeting the docs?* — requirement → module → test |
| [../CHANGELOG.md](../CHANGELOG.md) | *What changed, and when?* |
| **this file** | *What should we do next, and on what evidence?* |

Items point at their source rather than restating it. Sizes are rough: **S** ≈ an afternoon,
**M** ≈ a day or two, **L** ≈ a real project.

Each item has a stable id (**BL-n**). Claim one on the board in
[../COORD.md](../COORD.md) before starting, so two people don't build the same thing.

## Evidence base

Four independent sources, which is why several items appear on more than one list:

- **Blind trial, 2026-07-18** — an agent with no knowledge of reflow2's source designed and
  built a weather station through the consumer kit. Its friction log is the single richest
  source of findings we have; quotes below are its words.
- **Grok via opencode, 2026-07-18** — a second blind trial, different model *and* harness. Found
  the `structuredContent` array bug that three home-grown test layers all missed, because every
  one of them was a client we wrote. Notes: [trials/](trials/).
- **macOS / grok build, 2026-07-18** — first real external user. Reached the design loop and
  asked for things the trial agent could not (it had no continuity across sessions to miss).
- **Self-host probe, 2026-07-18** — reflow2's own design (119 nodes) pushed into a reflow2 graph
  and interrogated. The first test above fixture scale, and the only one where we know the right
  answer. Notes: [trials/2026-07-18-selfhost-probe.md](trials/2026-07-18-selfhost-probe.md).
- **[reflow-audit.md](reflow-audit.md)** — the original Reflow's workflows and tools, with
  adopt/obsolete verdicts.

## Next up

| ID | Item | Why | Size |
|---|---|---|---|

## Closed

Kept as a short pointer so a stable id never dangles; the detail is in the CHANGELOG.

- **BL-22 · Skills are not reliably discoverable** — done. The kit installed `.grok/skills/`
  alone, the narrowest-reach of four harnesses, so a project opened in Claude Code had an
  AGENTS.md naming seven skills the agent could not load. `reflow2_init.py` now installs to
  `.claude/skills/` (read by Claude Code, OpenCode and Copilot) and `.grok/skills/`, and writes
  `.mcp.json`, `opencode.json` and `.vscode/mcp.json` from one generator. Configs are merged, not
  overwritten — which also fixed a silent failure where a project that already had any MCP server
  never got reflow2 installed while the run reported success. Tables and the reasoning:
  [skills/README.md](skills/README.md).
- **BL-21 · The agent can report its own friction** — done. A `report-friction` skill plus the
  trigger in the consumer AGENTS.md, since a skill alone is not reliably found (BL-22). Redaction
  is the load-bearing part: a friction report naturally quotes the graph, and the graph is the
  user's design, so the skill reports reflow2-shaped facts — tool, argument *shapes*, node
  *types*, counts, masked errors — and asks before including anything of theirs. It never files
  without asking, searches for duplicates first, and degrades to a local file when `gh` is absent
  or the repo is unreachable, **which is the normal case: the repo is private**. Also folded skill
  frontmatter validation into `reflow2_init.py`, because a malformed `name` makes a skill fail to
  load with no error anywhere.
- **BL-25 · An answered question stays visible while its gap is open** — done. `open_questions`
  now returns two kinds: `asked` (still waiting) and `answered` **whose gap is still open**, with
  the reply attached. Answering settles nothing by itself — either the answer gets written into
  the design and the gap closes, or the gap is acknowledged; until one happens there is something
  outstanding and the list says so. A question whose gap has closed or been acknowledged drops out
  of the list but stays in the graph. Verified on reflow2's own design: the third session now sees
  the question and the reply, and acknowledging takes it to **0 gaps, 0 outstanding, 1 reviewed**.
- **BL-4 · Asked questions outlive the session** — done. `gap_to_prompt` was the only tool that
  never touched the graph: it phrased a question, returned it, and forgot, so the next session
  re-derived the same gap and asked again. Its serve pass now records a `Question` node at a
  derived id (`question:{gap hash}`), `ASKS_ABOUT` the nodes the gap concerned, with the wording
  the user actually saw. `open_questions` / `answer_question` / `withdraw_question` are on the
  surface, and `where-am-i` reads them first. **New node type** — 27 node types, 53 edge types —
  purely additive, so per BL-19 it is safe for existing graphs. Re-asking updates the wording but
  cannot reopen an answered question; there is a test for that.
- **BL-5 · `single_point_of_failure` measured against the baseline** — done. Not the cause the
  self-host probe guessed (it blamed the `≥2` threshold, by analogy with `surprises.rs`).
  Reproducing the shape showed the real one: the test asked whether ≥2 non-trivial components
  exist *after* removal, which assumes a connected design. One unrelated island already satisfies
  that, so every articulation point elsewhere reported — and attaching the island cleared them all
  at once, which is exactly the trial's *"15 defects vanished when I added two bookkeeping
  edges."* It now asks whether removal **increases** the count. reflow2's own design: 8 defects → 2,
  both true.
- **BL-24 · A Component the Project contains is not floating** — done. `orphan_level` only
  recognised a *Component* parent, and the Project carries no `Component.level` because it sits
  above all of them — so the shape the tools lead you to (a Project holding a few subsystems)
  reported one false gap per subsystem. The Project now counts as a parent; a component nothing
  contains is still an orphan, and there is a test for each direction. Together with BL-23 this
  took reflow2's own design from **25 gaps to 1**, and that one is true.
- **BL-23 · Per-file verification coverage is counted, not asked** — done. One `VERIFIES` edge
  per source file was 22 of 25 gaps on reflow2's own design, on a crate whose capabilities are all
  tested. The rule was not wrong, it was loud, and volume is what makes a list get skimmed.
  `graph_report` now carries a `Verification coverage` line and the gap is gone. Measured on the
  same 119-node graph: **25 gaps → 3**, of which one is true and two are BL-24.
- **BL-6b · `unexpected_coupling` demoted to a signal** — done. The decisive fact was not the
  trials but the spec: [gap-surfacing.md](gap-surfacing.md) names `orphan_node`, `dead_end`,
  `disconnected_cluster` and `single_point_of_failure` as the structural gaps — this was never
  among them, having been volunteered by the graph-analysis work. It is now reported by
  `graph_report` under its own heading, which already existed, so no information was lost. Two
  earlier rounds of tightening had not stopped it firing on correct architecture; an `Interface`
  bridges two clusters by construction, so modelling contracts as instructed made the detector
  penalise every one. `reviewed_gaps` now reports acknowledgements whose detector has been
  retired rather than dropping them, since a trial had already accepted one.
- **BL-2 · Expose `contain_component`** and **BL-3 · `Requirement.status` reachable** — done
  `9ab3da3`. Both needed more than the entry said. BL-2 also had to expose `Component.level`:
  shipping the containment alone would have flagged a false `level_mismatch` on every nesting,
  since everything defaults to `component` — worse than the silence it replaced. BL-3 also had to
  fix HEAL, which unlike DETECT ignored a `dropped` requirement, so marking one would have
  silenced half the system while the other half kept nagging. Recorded as **WS-7**/**WS-8**.
- **BL-6 · Split `unverified_capability`** — done `9ab3da3`. Artifacts now report as
  `unverified_artifact` with wording of their own; detection is unchanged, because proving a
  capability works still does not prove *this file* delivers it. The capability key is frozen
  deliberately: gap ids hash it and acknowledgements are stored under the resulting id, so a
  rename would silently expire every acknowledgement and orphan the Decision where neither
  `detect_gaps` nor `reviewed_gaps` looks. A test pins both keys.
- **BL-1 · Schema discovery tool** — done `9440929`, consumer kit `f00fac7`. `describe_schema`
  plus rejections that name the alternatives. The design turned on one detail worth remembering:
  `EdgeEndpoint::accepts()` returns true for the `*` wildcard, so the naive answer to the trial's
  question would have been `DEPENDS_ON` — the very edge it chose and distrusted. Matches are
  labelled exact vs wildcard for that reason. Recorded as **WS-6** in the coverage matrix.

## Bigger threads

**BL-15 · Project bootstrap and kit updates** — *from the external user, 2026-07-18.* "You should
be able to launch a project from reflow, which bootstraps everything into a new repo... And maybe
it adds a script for pulling in releases. You won't know up front what project type it is though."

Two problems, and his caveat is the design.

*Bootstrap.* Today the kit installs by three hand-run `cp`s, one of which needs the binary path
edited in, plus a hidden `.grok/` that `cp *` misses. That should be one command.

*The caveat is the product, not an obstacle.* You don't know the project type up front **because
that is a design decision the loop is supposed to make.** So bootstrap only what is type-neutral
— AGENTS.md, skills, MCP config, `.reflow2/`, `.gitignore`, a brief template — and deliberately
scaffold no `src/` layout, build file, or language choice. Those come *out* of the design, and
both blind trials produced exactly that: a structure that fitted what had been designed. A
scaffold that guessed would commit a design decision before the design existed.

*Updates are the sharper half, and currently absent.* The kit is copied, so a consumer's copy
freezes at install time — the first external user's copy is already stale by a day of skill
fixes and nothing tells him. Text (AGENTS.md + skills) is easy to refresh; the binary needs a
~10-minute RocksDB build, so it wants either published release binaries or a pinned-version
check. Bears directly on the embedded-vs-service fork: a service would make this disappear.

**Bootstrap and in-place updates: done** (`tools/reflow2_init.py`) — one command installs or
refreshes the design environment, resolves the binary path itself, records a kit version so
staleness is detectable, and leaves the graph, user files and a customised `.mcp.json` alone.
It installs no `src/`, build file or language choice, on purpose.

**Update-skew detection: done.** `reflow2_init.py` now reports whether the *binary* is older
than the source it was built from — the quiet failure where you pull, re-run init, and forget to
rebuild, leaving current instructions driving an old server. SETUP.md documents the three-step
update in the order that matters.

**Still open — published releases.** Everything above assumes a reflow2 checkout to run the
script from, which is true today because building the binary requires one. It stops being true
the moment someone wants reflow2 without cloning it: that needs published, per-platform binaries
(and macOS raises signing questions), or a fetch-from-git mode in the installer. It remains the
third piece of evidence for the service side of the embedded-vs-service fork, alongside
single-writer concurrency and this. Size **M–L**.

**The fork is decided** (2026-07-18, [surface-plan.md](surface-plan.md)): **repo-file, embedded**.
So this is no longer gated — build published per-platform binaries, which is the packaging answer
to a packaging problem. The service was weighed and set aside: its strongest argument
(concurrency) is hypothetical while there is one writer, it would put the user's design graph on a
machine they do not control, and it is permanent operational cost. The conditions that would
reopen it are written down; "published binaries proved insufficient" is one of them, so this item
is also the experiment that would justify revisiting.

> **Unblocked 2026-07-18.** BL-18, BL-19 and BL-20 were all waiting on the embedded-vs-service
> fork. It is decided — **repo-file, embedded** ([surface-plan.md](surface-plan.md)) — so build
> them for that shape. Export/import is now the migration story rather than a stopgap until a
> service centralises it.

**BL-26 · Which files does the design depend on, and is `DOCUMENTS` traversable?** — *user,
2026-07-18.* Prompted by the question "should every document in a repo be captured in the graph —
what is the purpose of each file?"

*Not every file.* [BL-23](#closed) is the caution: modelling 22 source files as Artifacts made
them 88% of the gap list. Capturing everything is how a list becomes something people skim. The
criterion is not "is it a file" but **"would something be wrong if this drifted out of step with
the design?"** That splits a repo four ways:

| Group | Example | Today |
|---|---|---|
| Produces the design | `crates/**/src/*.rs` | ✅ `Artifact` + `REALIZES` + checksum + `reconcile_artifacts` |
| Describes the design | `docs/*.md`, README | ⚠️ `DOCUMENTS` is in the schema; nothing can create it |
| Instructs agents | `AGENTS.md`, `COORD.md`, `.github/copilot-instructions.md` | ❌ nothing |
| No design meaning | `Cargo.lock`, `target/`, generated output | should stay out — this is where the noise would come from |

*The founding evidence is a failure reflow2 should have caught.* In one session on 2026-07-18:
AGENTS.md's build command was found wrong and fixed; hours later, by accident,
`.github/copilot-instructions.md` was found carrying **the same stale command**; and
`docs/backlog.md` grew a duplicated section that nothing noticed until someone went looking. Two
instruction files disagreeing about how to build the project is a coherence failure, and catching
coherence failures is the entire point — it was missed because neither file is in any graph.

*This is more than modelling more files.* Two things stand in the way:

1. **`DOCUMENTS` has no write side.** It is declared in `schema/build.yaml` and named in
   `nodes.rs`, with no constructor and no MCP tool — the recurring lesson below, for the ninth
   time.
2. **PROPAGATE does not traverse it.** `propagate.rs` lists `SPECIFIES`/`DOCUMENTS` as
   *"intentionally not traversed in this increment"*, so even fully wired a change would not
   ripple to the documents describing it. Making docs coherence-checked means **deciding
   `DOCUMENTS` is traversable**, and deciding what that implies for blast radius — a change to a
   Component reaching every doc that mentions it could be useful or could be the next flood.
   Weigh it against BL-23 before switching it on.

*The self-referential case is the best test available.* reflow2's own records — CHANGELOG,
backlog, requirements-coverage, COORD — are a hand-maintained golden thread, and four separate
lapses in one session went uncaught. The self-host probe already models
`requirements-coverage.md`'s **contents** as 72 Requirements but not the **file** as an Artifact
documenting them: the graph knows the requirements and not the document that is supposed to track
them. Extending the probe to the instruction and record files would test this before any of it
ships.

Size **M** for the write side plus a decision on traversal; **S** if it stops at recording which
files matter and leaves impact alone.

**BL-20 · Graph export / import, and versioned local backups** — *user, 2026-07-18.* Unblocks
BL-19's migration half and, through it, BL-18.

*Nothing exports today.* dynograph-foundation has no `export`/`dump`/`to_json` for graph data —
only `Schema::from_json` for the schema itself. But every primitive is present: `scan_nodes`
per type, `scan_outgoing_edges` / `scan_incoming_edges` per node, and `begin_batch` /
`commit_batch` for an atomic restore. Walk the schema's node types (the same introspection BL-1
added), scan, serialize; import replays under one batch. **Buildable entirely in reflow2 —
no foundation bump**, which the pin discipline in AGENTS.md wants.

*Why this is more than backup.* Export/import is the general migration path BL-19 lacks: dump
with the old binary, load with the new one. That covers both a storage-format change in the
foundation and an additive schema change, and it is far more robust than bespoke per-change
backfill code. It also gets design portability (move a graph between machines, hand one to
someone else) for free.

*The versioned-backup layer.* A design graph is small — hundreds to low thousands of nodes, so
tens of KB to a few MB as JSON. Cheap enough to snapshot on every session end or graph change and
keep all of them. `git init` on the backup directory and a commit per snapshot gives point-in-time
restore, a browsable history, and near-free packing, without a remote.

Three constraints the design must respect:

- **The export must be deterministic** — sorted keys, stable ordering. `HashMap` order is random,
  so a naive dump rewrites the whole file every time: every commit becomes a fresh blob, diffs
  become unreadable, and the main benefit of the git layer evaporates. Same discipline as
  `vocabulary.rs`.
- **Not `/tmp`.** systemd-tmpfiles clears it (on reboot, and by age), so backups there silently
  disappear — the opposite of the goal. Use `~/.local/share/reflow2/` or a directory beside the
  graph.
- **This is not the temporal axis.** `DesignEpoch` / `Snapshot` / `ChangeEvent` record *why* the
  design changed, semantically, inside the graph. This records the graph's bytes at a point in
  time. Neither substitutes for the other: the temporal axis cannot recover a corrupted store, and
  a snapshot cannot explain a requirement's history. Keep the two distinct in naming and docs so
  no one later mistakes one for the other.

Note `StoredNode` / `StoredEdge` do not derive `Serialize` — the orphan-rule workaround already
exists as `NodeDto` / `EdgeDto` in `reflow2-mcp/src/dto.rs`, and would need to move to core.
Size **M**.

**BL-19 · The graph must survive an upgrade** — *user, 2026-07-18.* **Blocks BL-18**: an
"you're out of date" nudge shipped before this exists drives users into an upgrade path with no
migration story.

*What is actually true today* (verified against dynograph-foundation v0.10.0). The schema lives
in the **binary** — reflow2 embeds the ten YAMLs via `include_str!` and re-merges them on every
open — while the RocksDB directory holds only nodes and edges. `new_rocksdb(schema, path)` takes
the schema from the caller and **stamps nothing on disk**: not a schema version, not a foundation
version. Validation runs on write, never on read.

So the reassuring half first: **upgrading reflow2 does not delete anyone's graph.** The feared
catastrophe is not the failure mode.

*The quieter hazard is real, though.* The foundation's own test (`engine/tests.rs:1325`) pins the
behaviour: add a required property with a default, and the default is applied **on create, not
backfilled**. Existing nodes keep the old shape. A schema change therefore leaves mixed-vintage
nodes with no error and no marker — detectors read `None` on old ones and a value on new ones.
That is a silent drop, which AGENTS.md rule 4 forbids everywhere else in this codebase.

*And the destructive case has no guard at all.* If dynograph-foundation changes its key encoding
(`keys.rs`) or value serialization, an existing store may be misread — and because nothing stamps
a version on the graph directory, there is no way to **detect** that a store predates the format,
let alone refuse to open it.

Wants, roughly in order of value: a version stamp written into the graph directory (schema
version + foundation tag + reflow2 commit); a fail-loud check on open when it does not match what
the binary expects — refuse rather than half-read; a backup-before-upgrade in
`reflow2_init.py`; and only then a migration/backfill path for additive schema changes. The first
two are **S** and buy the ability to say "your graph was written by an older reflow2" instead of
silently misbehaving. Backfill is **M**.

**BL-18 · Am I running the current reflow2?** — *user, 2026-07-18.* Extends the update half of
BL-15, whose local machinery is already built and whose remaining gap this names precisely.

`reflow2_init.py` stamps `.reflow2/kit-version.json` with `reflow2_version`, the short `commit`
and `committed_at`, and `binary_is_stale()` compares source mtime against binary mtime. Every one
of those checks is **local**: a consumer copy can tell that its binary predates its source, but
never that its source predates upstream. That is the one an installed copy actually needs —
the first external user's kit went stale in a day of skill fixes and nothing told him.

The check is cheap because the stamp already exists: `git ls-remote` for the remote HEAD, compare
against the stamped commit. No clone, no auth, one round-trip.

*Where it fires is the open question.* `reflow2_init.py --check` is the obvious home but only
helps someone who remembers to run it — the failure mode being fixed. Firing it from the MCP
server's startup instructions puts it in front of the agent unprompted, at the cost of a network
call per session; it must degrade silently when offline rather than blocking the loop, and must
not turn into a nag once someone has *decided* to stay on an older build.

*What it must not promise.* Unlike `claude update`, there is nothing to pull: the binary needs a
~10-minute RocksDB build, so the check can only report staleness, not resolve it. A real
`reflow2 update` needs published per-platform binaries — BL-15's still-open half, and a decision
that belongs with the embedded-vs-service fork. Keep the two apart: this item is **S** and
useful now; that one is **M–L** and gated on the fork.

**BL-16 · Domain-appropriate artifacts — the non-coding design problem** — *user, 2026-07-18.*

Coding is the *natural* domain here because agents are trained on it, so "design and build
anything" is quietly load-tested only on its easiest case. Ask for a rocket and the question
"what are the artifacts?" has no obvious answer. Ask for a 3D-printed object and one artifact
should probably be an `.stl` — but nothing in reflow2 knows that, and the agent may not either.

The gap is not the `Artifact` type, which is domain-neutral already. It is that **nothing helps
the agent decide what set of artifacts a given design concept actually calls for.** For software
it free-associates correctly from training; for a rocket it may need retrieval to find that the
answer involves things like a mass budget, a trajectory sim, drawings, a test plan — and for
hardware some artifacts are *physical*, which the as-built/drift machinery (`reconcile_artifacts`
checksums files) has no notion of.

Bears on P3 realization, on `unverified_capability` (what counts as verifying a weld?), and on
BL-9's as-fielded view. Likely wants a per-domain artifact-kind prompt or a retrieval step at
GENESIS, not a hardcoded taxonomy — the whole point is that the project type is a design output.
Size **M–L**, and it is the sharpest test of the "design anything" claim we have.

**BL-17 · Engineering principles as a separate, design-general file** — *user, 2026-07-18.*
Ported from `~/dev_storyflow/PROTOCOL.md`, whose "⭐ Engineering Principles" section is the
generalizable part (the fleet/bus/worker-pool/LEDGER/docker machinery around it is storyflow
infrastructure with no analogue here — reflow2 has COORD.md and two people).

Two of the seven are already reflow2 invariants: *no silent fallbacks* is AGENTS.md rule 4, and
*no silent caps/truncation* is implemented as `truncated_beyond_depth` / `skipped_operations`.
The four not yet written down here are **root-cause-before-fix** (name the mechanism, never
pattern-match a fix onto a symptom), **done = end-to-end** (merged ≠ done), **verify your own
claims by execution before reporting**, and **modular, no monoliths**.

Keep them in their own file rather than inlining into AGENTS.md, exactly as suggested: they need
tailoring away from coding. `PROTOCOL.md` phrases them for a web stack — "lens-exposed",
Playwright on the real surface, `npm run check`, unit-vs-live tests. For a rocket or a document,
"end-to-end" and "the real path" mean something else, and verification is a `Verification` node
rather than a test run. A separate file can generalize; a section buried in AGENTS.md will drift
back toward code. AGENTS.md then points at it. Size **S**.

| ID | Item | Why | Size |
|---|---|---|---|
| **BL-7** | **`ingest` over MCP** (SP-3b) | The multi-pass extraction pipeline is unreachable agent-native, so provenance, fuzzy dedup and time-aware resolution never run. Needs a transactional prepare pass. Closely tied to #4 and to session continuity. | L |
| **BL-8** | **Session state / multi-project** | Select a graph per project; give agents memory across sessions. Core already supports `graph_id`; nothing exposes it. See the memory note and [reflow-audit.md](reflow-audit.md). | L |
| **BL-9** | **As-fielded view** | Audit item 2, and it needs **no new schema** — `operate.yaml` is fully defined and now writable (WS-2). `reconcile_deployment` as a sibling of `reconcile_artifacts`. Guard against the library-plugin false positive the audit flags. | M |
| **BL-10** | **Root-cause classification of drift** | `drift.rs` detects divergence with no notion of *why*, so no notion of which side is wrong. Reflow's seven-category taxonomy ends in a decision rule. Needs a scalar coherence score to gate on. | M |
| **BL-11** | **Path-cumulative budget analysis** | Three independent reflow tools reached for it. PROPAGATE walks impact but never accumulates a quantity along source→sink paths — the classic SE budget rollup (latency, mass, power, cost). | M |
| **BL-12** | **Concurrent multi-agent / team access** | Deliberate future effort, and the trigger for revisiting the embedded/service fork — *decided 2026-07-18 as repo-file*. RocksDB is single-writer and fails loud, so agents take turns; that is only a real cost once a **second writer actually exists**, which it does not. Reach for RocksDB read-only secondaries before a service if the need turns out to be "let me look while you work". | L |
| **BL-13** | **Advanced testing tiers** | Comprehension (partly answered by the blind trial), scale (all fixtures are 3–10 nodes), messy input, longitudinal. | M |
| **BL-14** | **`tools/` sweep follow-ups** | The remaining adopt-list items in [reflow-audit.md](reflow-audit.md): typed gap resolution strategies, abstraction-gap → strategy, document round-trip, MCP resources/prompts. | M |

## Deliberate deferrals

Not gaps — decisions, recorded so they aren't rediscovered as bugs.

- **WS-4 `EnvironmentRule` / WS-5 `QualityGate`** — nothing reads or asks for either type, so a
  constructor would build the mirror image of the problem WS-1..3 fixed. Each lands with its
  detector.
- **Real LLM provider backends** — unnecessary agent-native; the ambient agent is the LLM.
- **`EmbeddingBackend`** — semantic dedup and retrieval. The audit has prior art on shape
  (local MiniLM/384-dim, normalized inner product, hash-gated rebuild) and one caution: retrieval
  thresholds are not identity thresholds.
- **Generative HEAL content** — proposals stay review-gated stubs.
- **Bayesian architecture optimization** — assessed and dropped; see the audit's do-not-port list.

## Recurring lesson

A capability exists in core and is unreachable or unadvertised on the surface. Nine instances so
far: `Interface`, HEAL's skill, the `Verification`/operate write side, `contain_component`,
`graph_id`, `Requirement.status`, `graph_report` as an answer to "where am I", the whole
`TemporalFact` / `ABOUT_ENTITY` / `VALID_FROM` / `VALID_TO` layer (schema-complete, zero Rust
API), and `DOCUMENTS` (declared, named in `nodes.rs`, no constructor and no tool — BL-26).

Before building something new, the higher-yield question is usually: **what does the core
already do that nothing can reach?**

The sibling lesson, learned the same way: a capability can also be unreachable because nothing
*points at it*. The consumer kit's skills were installed where three of four harnesses never look
(BL-22), and `describe_schema` would have been invisible to the people who needed it had the kit
not been updated in the same change (BL-1). Shipping the code is not shipping the capability.
