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

Three independent sources, which is why several items appear on more than one list:

- **Blind trial, 2026-07-18** — an agent with no knowledge of reflow2's source designed and
  built a weather station through the consumer kit. Its friction log is the single richest
  source of findings we have; quotes below are its words.
- **Grok via opencode, 2026-07-18** — a second blind trial, different model *and* harness. Found
  the `structuredContent` array bug that three home-grown test layers all missed, because every
  one of them was a client we wrote. Notes: [trials/](trials/).
- **macOS / grok build, 2026-07-18** — first real external user. Reached the design loop and
  asked for things the trial agent could not (it had no continuity across sessions to miss).
- **[reflow-audit.md](reflow-audit.md)** — the original Reflow's workflows and tools, with
  adopt/obsolete verdicts.

## Next up

| ID | Item | Why | Size |
|---|---|---|---|
| **BL-1** | **Schema discovery tool** | Trial agent brute-forced 14 edge-type guesses, then used `DEPENDS_ON` *"because it was the one that validated, which is precisely the kind of silent accommodation this project says it's against."* The error *"tells me I'm wrong without telling me what's right."* | S |
| **BL-2** | **Expose `contain_component`** | Exists in core, not on the surface, so an assembly hierarchy can't be modelled — and `hierarchy_issues` returns `[]` because it never has a hierarchy to check. | S |
| **BL-3** | **`Requirement.status` reachable** | Already in the schema, defaulting to `proposed`. The trial agent wanted to mark requirements provisional and wrote "ASSUMED" into the statement text instead. | S |
| **BL-4** | **Persist asked questions** | `gap_to_prompt` output evaporates, so the next session re-derives and re-asks. The trial agent's own framing: *"the stateless-agent problem reflow2 is supposed to solve."* Same gap the external user hit as "how do I pause and resume". | M |
| **BL-5** | **Re-examine `single_point_of_failure`** | *"All 15 defects vanished at once when I added two bookkeeping edges. Nothing about actual fragility changed."* Use that case as the test. | M |
| **BL-6b** | **Demote `unexpected_coupling` from a gap to a report-only signal** | Three independent reports now — both blind trials and the coupling work itself. Grok: *"that coupling **is** the product"* on the one real interface in a 3-component design. Community structure is not meaningful at this scale, so it should inform rather than demand attention. | S |
| **BL-6** | **Rename/split `unverified_capability`** | Fires per-Artifact despite the name, producing gaps titled "Nothing verifies reading.py". Semantically right, legibly wrong. | S |

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
single-writer concurrency and this. Size **M–L**, and worth deciding the fork first.



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
| **BL-12** | **Concurrent multi-agent / team access** | Deliberate future effort. RocksDB is single-writer; this is the strongest argument for the service shape. | L |
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

Seven times in one session a capability existed in core and was unreachable or unadvertised on
the surface: `Interface`, HEAL's skill, the `Verification`/operate write side,
`contain_component`, `graph_id`, `Requirement.status`, and `graph_report` as an answer to
"where am I". Items 1–3 above are all the same shape.

Before building something new, the higher-yield question is usually: **what does the core
already do that nothing can reach?**
