# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Read first

- **[AGENTS.md](AGENTS.md)** — what Reflow 2.0 is, the coherence-loop mental model, the
  module map, and the non-negotiable rules for changing the project. Start there for *why*.
- **[docs/overview.md](docs/overview.md)** — maps every design doc and the reading order.
- **[docs/requirements-coverage.md](docs/requirements-coverage.md)** — the living
  traceability matrix (doc requirement → module → test → Met/Partial/Deferred). Consult it
  to see what's built vs. deferred, and **update it in the same change that moves a status**.

This file covers the operational side: commands and code-level architecture.

## Commands

Everything runs from the repo root. The core crate is `crates/reflow2-core`.

```bash
# Build / test — ALWAYS pass --no-default-features for dev iteration:
# it uses dynograph-storage's in-memory backend and skips the RocksDB C++
# compile (~10 min). The whole suite runs in well under a second.
cargo test --no-default-features

cargo test --no-default-features --test heal            # one integration-test file
cargo test --no-default-features golden_thread_round_trips   # one test by name (substring)
cargo test --no-default-features --lib                  # unit + doctests only

cargo clippy --no-default-features --all-targets        # keep clippy-clean
cargo fmt                                               # and fmt-clean (cargo fmt --check in CI)

# Schema validation (Python, no Rust toolchain needed). Must print "OK" after any
# schema/*.yaml edit. Needs PyYAML; use whatever python3 has it.
python3 tools/validate_schema.py

# End-to-end smoke test of the MCP *binary* (stdio JSON-RPC, real RocksDB graph).
# Covers what cargo test can't: the shipped surface, tool schemas, and the JSON an
# agent actually receives. Needs `cargo build -p reflow2-mcp` first (RocksDB, ~10 min
# cold). Stdlib-only Python; exits non-zero on any failed check.
python3 tools/smoke_mcp.py
```

A change is "done" only when `cargo test --no-default-features`, `cargo clippy
--no-default-features --all-targets`, and `cargo fmt --check` are all clean. Tests live
beside the code as `crates/reflow2-core/tests/*.rs` (one file per module/concern) plus unit
tests in `src/schema.rs` and doctests in `src/lib.rs`/`src/nodes.rs`.

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
- The **schema is the vocabulary** (26 node types, 52 edge types across 10 `schema/*.yaml`
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
