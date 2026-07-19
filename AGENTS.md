# AGENTS.md — orientation for AI agents working on Reflow 2.0

> **This is the primary instruction file for this repo** — it follows the
> [agents.md](https://agents.md) convention, so every agent reads the same thing.
>
> **Before you start:** run **`git pull --rebase`**, then read **[COORD.md](COORD.md)** and claim
> what you take. It is the shared board between the people and agents working this repo; a board
> you haven't pulled is out of date, and it covers resolving merge conflicts without discarding
> anyone's work. **[docs/backlog.md](docs/backlog.md)** has what is open and why.

Read this first. It tells you what this project is, how it's organized, and the rules to
follow so your changes stay coherent with the design.

**Per-crate files exist and the closest one wins**, per the convention. If you are editing inside
a crate, read its file too — the build commands genuinely differ, and getting that wrong costs
ten minutes of C++ compile:

| Editing | Also read | Why it differs |
|---|---|---|
| `crates/reflow2-core/**` | [its AGENTS.md](crates/reflow2-core/AGENTS.md) | sub-second test path; core-only invariants |
| `crates/reflow2-mcp/**` | [its AGENTS.md](crates/reflow2-mcp/AGENTS.md) | pays the RocksDB build; needs the smoke test |

## Commands

Everything runs from the repo root. The core crate is `crates/reflow2-core`.

```bash
# Build / test — for dev iteration ALWAYS scope to the core with -p AND pass
# --no-default-features: that combination uses dynograph-storage's in-memory
# backend and skips the RocksDB C++ compile (~10 min). Runs in well under a second.
#
# -p reflow2-core is load-bearing, not tidiness. Without it the workspace also
# builds reflow2-mcp, which depends on reflow2-core with `features = ["rocksdb"]`
# — an explicitly-enabled feature on a dependency edge, which --no-default-features
# cannot switch off. Drop the -p and you get the C++ build you were avoiding.
cargo test -p reflow2-core --no-default-features

cargo test -p reflow2-core --no-default-features --test heal          # one integration-test file
cargo test -p reflow2-core --no-default-features golden_thread_round_trips  # one test by name
cargo test -p reflow2-core --no-default-features --lib                # unit + doctests only

cargo clippy -p reflow2-core --no-default-features --all-targets      # keep clippy-clean
cargo fmt                                               # and fmt-clean (cargo fmt --check in CI)

# The full workspace, including the MCP surface. Pays the RocksDB compile once,
# then it is cached. Run before pushing — the core-only gate cannot see
# reflow2-mcp, where the tool surface and its tests live.
cargo test --workspace

# Schema validation (Python, no Rust toolchain needed). Must print "OK" after any
# schema/*.yaml edit. Needs PyYAML; use whatever python3 has it.
python3 tools/validate_schema.py

# reflow2's own functional design, as a reflow2 graph (96 nodes). The export at
# docs/design/reflow2.json is the durable record — .reflow2/ is gitignored, so
# the JSON is what gets reviewed and diffed. Rebuild it after a design change;
# --analyse-only re-imports the committed export and re-runs the analysis.
python3 tools/build_design_graph.py
python3 tools/build_design_graph.py --analyse-only

# Phase-coverage trial — does the design still carry weight after P2? Seeds a
# realistic graph, injects the divergences P3/P4/P5 are each supposed to catch,
# and scores whether the graph noticed. NOT a gate yet: it exits non-zero today
# because 5 probes are genuinely missed (BL-30, BL-9). It is the standing
# measurement for the failure that sank the original reflow — the early phases
# going well and the later ones proceeding as if they hadn't.
python3 tools/phase_trial.py

# Erosion trial — the sharper question. Not "did a file change?" but: after N
# rounds of test-fails/fix-code/accept, does the design still describe what
# shipped? Currently 2/7, and it reports ZERO gaps on a design that has lost
# touch with its code (BL-33, BL-34). Also non-zero exit by design.
python3 tools/erosion_trial.py

# The same cycle done right — the constructive counterpart. Proves designed ==
# released is reachable today with axis-Z discipline (original intent survives in
# a Snapshot), and that reflow2 gives the SAME verdict for the coherent graph and
# the eroded one. That gap is BL-35.
python3 tools/coherent_erosion_trial.py

# End-to-end smoke test of the MCP *binary* (stdio JSON-RPC, real RocksDB graph).
# Covers what cargo test can't: the shipped surface, tool schemas, and the JSON an
# agent actually receives. Needs `cargo build -p reflow2-mcp` first (RocksDB, ~10 min
# cold). Stdlib-only Python; exits non-zero on any failed check.
python3 tools/smoke_mcp.py

# Install or update reflow2 in a consumer project (the design environment only —
# never a src/ layout or build file; project type is a design output, not an input).
python3 tools/reflow2_init.py /path/to/project           # set up, or update in place
python3 tools/reflow2_init.py /path/to/project --check   # what would change
```

Tests live beside the code as `crates/reflow2-core/tests/*.rs` (one file per module/concern) plus
unit tests in `src/schema.rs` and doctests in `src/lib.rs`/`src/nodes.rs`.

## Working on this repo

**Order of operations.** `git pull --rebase`, then claim your item on [COORD.md](COORD.md) and
commit that line *before* the work — a claim nobody can see is not a claim. Then read
[docs/backlog.md](docs/backlog.md) for what is open and why. COORD.md also covers resolving merge
conflicts on the shared records without discarding anyone's work; read that before you hit one.

**Branches.** `feat/<short-name>` off `main`, one per claimed item where practical.

**A change is done when all of these are clean:**

```bash
cargo test --workspace                                   # both crates
cargo clippy -p reflow2-core --no-default-features --all-targets
cargo fmt --check
python3 tools/validate_schema.py                         # after any schema/*.yaml edit
python3 tools/smoke_mcp.py                               # after any tool-surface change
```

Compiling is not the finish line, and neither is a green unit test. Drive the thing you changed:
the surface a user actually touches is the MCP binary, and three home-grown test layers once
agreed with each other and were all wrong because each was a client we wrote.

**Update the records in the same change, not afterwards** — this is the rule most often skipped,
and the records are the project's memory:

| Record | Update when |
|---|---|
| [CHANGELOG.md](CHANGELOG.md) | a user would notice |
| [docs/requirements-coverage.md](docs/requirements-coverage.md) | a status moves |
| [docs/backlog.md](docs/backlog.md) | an item is finished or discovered |
| [docs/trials/](docs/trials/) | a real session went wrong — verbatim, append-only |
| [COORD.md](COORD.md) | you start, and again when you finish |

**Claims made in the records must be evidence-backed.** If a backlog entry asserts a consequence,
it should be traceable to something someone observed — not inferred while writing the entry. When
you cannot source it, say so or strike it.

## Architecture

Reflow 2.0 is a graph-backed engine that keeps a design coherent across its lifecycle. The
one runtime crate, `reflow2-core`, is the **deterministic, LLM-free core** — build-order
steps 1–2 of [docs/interaction-surfaces.md](docs/interaction-surfaces.md). It is neutral to
the interaction surface (MCP / CLI / hosted) and to any LLM provider; those plug in last.

### The store and schema (the foundation)

- The graph store is **[dynograph-foundation](https://github.com/sligara7/dynograph-foundation)**,
  consumed as library crates **by git tag** (`v0.9.4` in the workspace `Cargo.toml`):
  `dynograph-core` (schema + `Value`), `dynograph-storage` (`default-features = false` so
  RocksDB is opt-in; the core runs on the in-memory backend), `dynograph-graph` (pure
  graph-theory algorithms). To iterate against an unreleased foundation locally, uncomment
  the `[patch]` block in the root `Cargo.toml` — do not commit it uncommented.
- The **schema is the vocabulary** (27 node types, 53 edge types across 10 `schema/*.yaml`
  domains): the node/edge names are load-bearing. `src/schema.rs` embeds all ten YAML files
  via `include_str!` and merges them with `Schema::from_multiple_yamls` — the same files
  `tools/validate_schema.py` checks, so there is one source of truth. Terminology in code
  must match the schema; `src/nodes.rs` holds the `node::`/`edge::` name constants.

### The design graph handle

`src/graph.rs` — `DesignGraph` wraps a `dynograph_storage::StorageEngine` scoped to one
logical graph id. It is the single handle everything else hangs off: generic
schema-validated CRUD (`create_node`/`get_node`/`create_edge`/`outgoing`/`incoming`/
`scan_nodes`/`delete_*`), typed golden-thread constructors (`add_project`,
`add_requirement`, `add_capability`, `add_component`, `satisfies`, `allocate`, `contains`),
and `pub(crate)` batch controls for atomic apply. Each coherence-loop step is a set of
methods on `DesignGraph` implemented in its own module (Rust lets `impl DesignGraph` span
files):

| Loop step | Module | Entry points |
|---|---|---|
| **CHANGE** (axis Z — never overwrite the past) | `src/temporal.rs` | `add_epoch`, `snapshot_node`, `add_change_event`, `record_change` |
| **PROPAGATE** (blast radius along the golden thread) | `src/propagate.rs` | `propagate_change` (reactive), `propagate_from` (speculative) |
| **DETECT** (find gaps to ask the human) | `src/detect.rs` | `detect_gaps` → `GapCandidate`s; `GapCandidate::to_prompt` (PROMPT half, via `LlmBackend`) |
| **HEAL** (fix structure the machine can) | `src/heal.rs` (+ `src/structure.rs`) | `detect_defects`, `propose_heal`, `apply_heal` |
| **LLM seam** | `src/llm.rs` | `LlmBackend` trait, `MockLlmBackend`, `complete_json` |

`src/structure.rs` builds a `dynograph-graph` view (the "design network" — design nodes
joined by *traceability* edges) for HEAL's topology detectors.

### Load-bearing invariants (do not regress these)

- **No silent fallbacks / no silent drops** (AGENTS.md rule 4). This is enforced concretely:
  CRUD fails loud on unknown types / missing required props; PROPAGATE bounds depth but
  *reports* `truncated_beyond_depth`; HEAL moves un-appliable ops to `skipped_operations`
  with a reason; DETECT surfaces unknown seeds; the LLM PROMPT step degrades to raw wording
  with `rephrase_degraded = true`. New code must keep this bar; tests assert it explicitly.
- **HEAL is propose-then-apply.** `propose_heal` never mutates; `apply_heal` mutates
  atomically, is mode-aware (`rigid` project mode = propose-only), gates generated content
  behind `requires_human_review`, and does post-repair verification. Keep detection,
  proposal, and mutation separate.
- **PROPAGATE / structure exclude `CONTAINS`.** Decomposition is not traceability; including
  it makes the Project a hub that short-circuits distances. Impact and topology traverse the
  shared traceability set (`propagate::is_traceability_edge`).
- **Structural topology detectors are selective.** A design's golden thread is tree-shaped,
  where every internal node is a naive articulation point — so `single_point_of_failure`
  only fires when a node separates ≥2 real subsystems (see `structure.rs`).
- **Do not bump the dynograph-foundation pin as housekeeping.** The five foundation crates are
  pinned by git tag in the workspace `Cargo.toml`. Moving that tag forces a full
  `librocksdb-sys` C++ rebuild (~10 min) on **every** machine that pulls — yours, your
  collaborators', and every consumer project. Bump it only when a reflow2 change actually needs
  something the new tag provides, and say which capability in the commit message. "Latest is
  probably better" is not a reason; a routine reflow2 update should cost a consumer nothing but a
  text refresh.
- **A foundation bump is a data-migration question, not just a version change.** Nothing is
  stamped on the graph directory — not a schema version, not a foundation tag — and validation
  runs on write, never on read. So a storage-format change (`keys.rs`, value serialization) could
  misread an existing store with nothing to detect it, and an additive schema change leaves
  mixed-vintage nodes rather than backfilling (the foundation's own `engine/tests.rs:1325` pins
  that behaviour: defaults apply on create, not retroactively). Before any bump, ask what happens
  to a graph written by the previous version. See **BL-19**.
- **Deterministic ids.** Gap/heal issue ids are a stable FNV-1a hash of
  `source + sorted affected ids` (not `std` `DefaultHasher`) so they're reproducible for
  dedup/caching.
- **`LlmBackend` is sync and object-safe.** The core holds `&dyn LlmBackend` and never names
  a provider. Typed JSON parsing is the free function `complete_json`, kept off the trait to
  preserve object safety. Build and test new LLM-reasoning ops against `MockLlmBackend`; do
  not add an async runtime or a provider dependency to the core.

### What's deliberately not here yet

Anything gated on the two deferred decisions — real LLM provider backends and the
interaction surface — plus the LLM-reasoning processes (INGEST/extraction, SME, GENESIS,
generative HEAL content) and a few schema-present-but-no-code areas (dimensions/depth,
`Component.level` matryoshka). See the coverage matrix for the exact deferral list; don't
assume a service, API, or running system exists.

---

## What Reflow 2.0 is

A graph-backed system that partners with an LLM agent to **design and build anything** —
software, hardware, a document, a full acquisition program. It captures the **entire
lifecycle of a design (concept → operations) in one knowledge graph**, tied together by
the systems-engineering *golden thread* (traceability from every artifact back to the
intent it serves).

The payoff: when **anything changes in any phase**, the ripple effects are automatically
found, surfaced to the user as plain questions, and healed back to coherence — so concept
through operations always stays in agreement. **The user never needs to know systems
engineering; the graph does.**

This is a clean-room rebuild ([github.com/sligara7/reflow2](https://github.com/sligara7/reflow2))
of ideas from the author's earlier projects (all under
[github.com/sligara7](https://github.com/sligara7)): `reflow`, `storyflow`,
`chain_reflow`, and the graph engine `dynograph-foundation`.

## The one mental model to hold: the coherence loop

```
CHANGE → PROPAGATE → DETECT → SURFACE → RESOLVE/HEAL → COHERENCE
```

- **CHANGE** — any edit becomes a `ChangeEvent` at a `DesignEpoch` (the old state is snapshotted, never overwritten).
- **PROPAGATE** — walk the traceability edges to compute the blast radius.
- **DETECT** — re-diagnose the touched region for new gaps/contradictions.
- **SURFACE** — turn those into constructive, plain-language questions for the user.
- **RESOLVE/HEAL** — the user answers (re-ingested) or HEAL proposes structural fixes.

Three complementary lenses on the graph: **phases** (P0–P5 lifecycle), **three axes**
(X = network, Y = decomposition, Z = change-over-time), and this **loop** (behavior).

## Current state (important)

This project is **early implementation**. The docs + schema remain the source of truth;
runtime code has begun with the **deterministic, LLM-free core** (build-order steps 1–2 of
[docs/interaction-surfaces.md](docs/interaction-surfaces.md)). Do not assume any surface,
service, or LLM wiring exists yet — none does.

- `crates/reflow2-core/` — Rust crate implementing the deterministic coherence-loop
  spine so far. Modules: `schema` (loads the 10 domains into one merged dynograph
  `Schema`), `graph` (`DesignGraph` over `dynograph-storage`, in-memory backend, schema-
  validated CRUD + typed golden-thread constructors), `temporal` (axis Z — `DesignEpoch` /
  `ChangeEvent` / `Snapshot` and `record_change`, the **CHANGE** step: snapshot the past,
  never overwrite), `propagate` (**PROPAGATE** — direction-classified bounded BFS over the
  golden thread → an explained `BlastRadius`; reactive from a `ChangeEvent` or speculative
  from seeds), `detect` (**DETECT** — deterministic gap detectors → ranked `GapCandidate`s;
  traceability + phase-coverage groups, gated on type-population counts), `heal` (**HEAL** —
  detect structural defects → a `HealProposal` (propose, never mutate) → atomic `apply_heal`
  with post-repair verification; mode-aware (rigid = propose-only), strategy-filtered,
  human-review-gated for generative fixes; content-free duplicate-merge is the applied
  repair), `structure` (a `dynograph-graph` view of the design network powering HEAL's
  graph-topology defects: `disconnected_community`, *selective* `single_point_of_failure`,
  `dead_end` — selective because a golden thread is tree-shaped, where every internal node
  is a naive articulation point), `llm` (the pluggable `LlmBackend` seam — object-safe, sync
  — + `MockLlmBackend` + `complete_json`; the LLM boundary the core holds as `&dyn`), `ingest`
  (**INGEST** — freeform text → graph via the `LlmBackend`: phase-gated extraction passes,
  provenance Fragment, time-aware resolution with matched-evolved snapshots, fuzzy cross-id
  dedup), `allocate` (**graph-analysis** — `evaluate_allocation` scores the current
  function→service allocation; `propose_allocation` clusters the weighted coupling graph with
  Leiden). Consumes `dynograph-foundation` by git tag (`v0.10.0`): `dynograph-core`,
  `dynograph-storage` (`default-features = false` so the RocksDB C++ build stays opt-in),
  `dynograph-graph` + `dynograph-resolution` (pure, no features) — mirrors the predecessor
  `ir2`. Fast dev/test build: `cargo test --no-default-features`. Keep it green, clippy-clean,
  and `cargo fmt`-ed. Not yet built: real `LlmBackend` provider backends + the interaction
  surface (deferred decision), the optional embedding seam (semantic dedup/retrieval), HEAL's
  generative healer content, SME, GENESIS.

**Open decision (deliberately deferred):** the *interaction surface* — MCP/skills for a
coding agent, a hosted web app, a CLI, or a library — is not yet chosen. It plugs in last
and determines whether an external LLM provider is needed (agent-native = no; hosted =
yes). The core is built to be neutral to this; see
[docs/interaction-surfaces.md](docs/interaction-surfaces.md). Don't hard-wire a surface or
an LLM provider into the core.

- `schema/*.yaml` — 10 composable [dynograph-foundation](https://github.com/sligara7/dynograph-foundation)
  schema domains (27 node types, 53 edge types). This is the foundation everything builds on.
- `docs/*.md` — the vision, design, and process specifications.
- `tools/validate_schema.py` — validates the schema against dynograph-core's rules.

## Where to look

**Always start with [docs/overview.md](docs/overview.md)** — it maps every document and the
reading order (Vision → Design → Process → Heritage). Then:

| You want to… | Read |
|---|---|
| understand the "why" | [docs/vision.md](docs/vision.md) |
| understand the graph structure | [docs/three-axes.md](docs/three-axes.md), `schema/` |
| know how content gets into the graph | [docs/extraction-plan.md](docs/extraction-plan.md), [docs/sme-augmentation.md](docs/sme-augmentation.md), [docs/artifact-linking.md](docs/artifact-linking.md) |
| know how change is handled | [docs/impact-propagation.md](docs/impact-propagation.md), [docs/gap-surfacing.md](docs/gap-surfacing.md), [docs/heal-process.md](docs/heal-process.md) |
| understand the operating environment/ruleset | [docs/operating-environment.md](docs/operating-environment.md) |
| know how a human drives it (and the LLM-sourcing tradeoff) | [docs/interaction-surfaces.md](docs/interaction-surfaces.md) |
| confirm the build meets the docs (traceability) | [docs/requirements-coverage.md](docs/requirements-coverage.md) |
| use the graph to *drive* design decisions (allocation, weights, analysis crates) | [docs/graph-analysis.md](docs/graph-analysis.md) |
| make reflow2 drivable by a coding agent (the next build phase) | [docs/surface-plan.md](docs/surface-plan.md) |
| see where ideas came from | [docs/reflow-v3-nuggets.md](docs/reflow-v3-nuggets.md), [docs/chain-reflow-nuggets.md](docs/chain-reflow-nuggets.md) |

## Rules for changing this project

1. **Schema-first.** The node/edge vocabulary is load-bearing. After any `schema/*.yaml`
   edit, run `python3 tools/validate_schema.py` (needs PyYAML — on this machine use
   `~/miniconda3/bin/python`). It must print "OK".
2. **Keep docs cohesive.** Every doc carries a breadcrumb to `overview.md`; new docs must
   too, and must be added to the overview's document map and reading order.
3. **Terminology matches the schema.** Use the real node/edge names (e.g. `Capability`,
   `Component`, `Artifact`, `Verification`, `Environment`, `EnvironmentRule`,
   `ChangeEvent`). Do not reintroduce retired names from old Reflow (e.g. `PhaseEvent`,
   `ContractedFunction`, `APIEndpoint`).
4. **Honor the disciplines** the process docs call "non-negotiable" — most importantly
   **no silent fallbacks / no silent drops**: surface failures and skipped items loudly;
   never let data loss or an unstated assumption pass as success.
5. **References are the author's own** under `github.com/sligara7`. The only third-party
   pieces are dependencies (dynograph-foundation's RocksDB/Tantivy/HNSW/serde; LLM
   providers like OpenRouter) — never conceptual content.
6. **Don't touch the sibling source repos** (`../../storyflow`, etc.) — mine them for
   ideas, but all new work lands here.

## Engineering principles (adapted from storyflow's `PROTOCOL.md ⭐`)

These are the author's hard-won code-quality principles, carried over and adapted to
reflow2 (a single Rust core, no fleet). They **override speed — timing bends to correctness**.

1. **Right long-term fix — no patches/stopgaps.** Find the *root cause* first (reproduce,
   trace, prove the mechanism), then fix it at the root, not the symptom. If you can't name
   the root cause, say so and keep digging. If the correct fix needs something that isn't
   there yet, **stop and report the gap** — a reported gap is honest; a papered-over one
   re-breaks later.
2. **No silent fallbacks / no silent drops — an integrity line, not a style preference.**
   Never swallow an error into a "looks fine" state (no catch-returns-default, no atomic op
   that drops the bad part, no empty-on-failure). A swallowed failure makes broken code
   report success — it *lies* to the user. Fail loud, or don't write it. This is rule 4
   above; it is the project's first principle. (Enforced concretely across the core — see
   the "load-bearing invariants" under Architecture above.)
3. **Record every deferral — no silent stubs.** When you defer work, write it down as
   Deferred in [docs/requirements-coverage.md](docs/requirements-coverage.md) **in the same
   change**, and annotate the code site (an unused field, a stubbed branch) with a pointer
   back. A deferral nobody wrote down is a silent stub — the same integrity breach as a
   silent drop. "Partial and recorded" is fine; "looks done but quietly isn't" is not.
4. **Verify your own claims before stating them.** Run the real check yourself (foreground
   `cargo test --no-default-features`, clippy, fmt) and confirm any symbol/field/API you
   reference actually exists. "Tests pass" means you watched them pass; a green that only
   passed because an error was swallowed is a false report.
5. **Real-path tests.** A test must exercise the path callers actually use, end to end — not
   an unchanged inner helper. "Done" = the real behavior is observable and tested, not "it
   compiles."
6. **No silent caps/truncation.** If you bound coverage (top-N, a subset of passes, a depth
   cap), say so loudly in the code and the report — silent truncation reads as "covered
   everything" when it didn't. (This is why PROPAGATE reports `truncated_beyond_depth` and
   INGEST records `dropped_edges`.)
7. **Modular, composable code — no monoliths.** Keep files focused and single-responsibility;
   split along natural seams (each coherence-loop step is its own module). Prefer small
   composable pieces and dependency injection (e.g. `&dyn LlmBackend` passed in) over
   sprawling files or deep inheritance.

## Provenance of the ideas (so you can trace any decision)

- **storyflow** → the extraction pipeline, the six universal processes, the operating-
  environment ruleset (its "cosmology"), SME/supplementary analysis, the note layer, the three axes.
- **chain_reflow** → matryoshka/missing-intermediate detection, correlation-vs-causation
  rigor, creative linking, system-of-systems.
- **reflow (v3)** → the phase spine, as-designed/as-built/as-fielded fidelity views,
  framework packs, root-cause change classification.
- **dynograph-foundation** → the schema-driven graph store (RocksDB + HNSW + BM25 +
  fuzzy/vector resolution) reflow2 targets.
