# AGENTS.md — `reflow2-core`

The deterministic, LLM-free engine: schema, graph handle, and one module per coherence-loop step.
Surface-neutral by design — nothing here knows about MCP, a CLI, or an LLM provider.

**Read the [root AGENTS.md](../../AGENTS.md) first** for orientation, architecture and the
project-wide rules. This file covers only what differs when you are editing *this* crate.

## Commands

Always scope with `-p reflow2-core` **and** `--no-default-features`. That combination uses
dynograph-storage's in-memory backend and skips the RocksDB C++ compile; the suite runs in well
under a second.

```bash
cargo test -p reflow2-core --no-default-features
cargo test -p reflow2-core --no-default-features --lib            # unit + doctests
cargo test -p reflow2-core --no-default-features --test heal      # one integration file
cargo test -p reflow2-core --no-default-features golden_thread    # by name (substring)

# The `fulltext` feature (Tantivy — pure Rust, no C++, but a real dependency
# tree, so it stays off the fast path above). tests/search.rs has two arms,
# like persistence.rs: the default build proves absence fails loud; this runs
# the real BM25 round trip.
cargo test -p reflow2-core --no-default-features --features fulltext --test search

cargo clippy -p reflow2-core --no-default-features --all-targets
cargo fmt

python3 ../../tools/validate_schema.py   # must print OK after any schema/*.yaml edit
```

**The `-p` is load-bearing, not tidiness.** Without it the workspace also builds `reflow2-mcp`,
which depends on this crate with `features = ["rocksdb"]` — an explicitly-enabled feature on a
dependency edge, which `--no-default-features` cannot switch off. Drop the `-p` and you get the
~10-minute C++ build you were trying to avoid.

**A green core is not a green repo.** These commands cannot see `reflow2-mcp`, where the tool
surface lives. Run `cargo test --workspace` before pushing.

## Layout

Each loop step is a set of methods on `DesignGraph` in its own module — Rust lets `impl
DesignGraph` span files, so add a new capability as a new module rather than by growing
`graph.rs`.

| Step | Module |
|---|---|
| CHANGE (axis Z) | `temporal.rs` |
| PROPAGATE | `propagate.rs` |
| DETECT | `detect.rs` |
| HEAL | `heal.rs` + `structure.rs` |
| LLM seam | `llm.rs` |
| Schema discovery | `vocabulary.rs` |

`schema.rs` embeds the ten `schema/*.yaml` domains via `include_str!` and merges them — the same
files `validate_schema.py` checks, so there is one source of truth. `nodes.rs` holds the
`node::`/`edge::` name constants; it is a **convenience subset, not a mirror** of the schema
(fewer constants than there are types), so read the schema for ground truth, never that file.

## Invariants specific to this crate

The project-wide invariants are in the root file. These are the ones you will trip over here:

- **Fail loud on the write path.** CRUD rejects unknown types and missing required properties.
  Never widen a signature to make a bad write succeed.
- **Deterministic output.** Ids are a stable FNV-1a hash (not `std` `DefaultHasher`); anything
  built by iterating the schema's `HashMap`s must be sorted before it is returned, or repeated
  calls differ and tests flake.
- **`LlmBackend` is sync and object-safe.** The core holds `&dyn LlmBackend` and never names a
  provider. Do not add an async runtime or a provider dependency here.
- **PROPAGATE and structure exclude `CONTAINS`.** Decomposition is not traceability.
- **Test against `MockLlmBackend`**, never a live provider.
