# AGENTS.md — `reflow2-mcp`

The agent-native surface: an MCP stdio server exposing the coherence-loop ops as tools. Thin by
intent — every tool locks the graph, calls one deterministic core op, and returns. Logic belongs
in `reflow2-core`, not here.

**Read the [root AGENTS.md](../../AGENTS.md) first** for orientation, architecture and the
project-wide rules. This file covers only what differs when you are editing *this* crate.

## Commands — read this before you start

**Building this crate compiles RocksDB: ~10 minutes cold, then cached.** It depends on
`reflow2-core` with `features = ["rocksdb"]`, because a design must survive across agent
sessions. That feature is enabled on the dependency edge, so `--no-default-features` cannot turn
it off — the core crate's fast path does not apply here and there is no way to make it.

Budget for it. Start the build before you need it, and do not kill it halfway: the C++ objects
are cached per-file, so a killed build resumes, but a fresh checkout pays full price.

```bash
cargo test -p reflow2-mcp                 # links RocksDB — slow first time
cargo check -p reflow2-mcp --all-targets  # type-check only, no link: much faster while iterating
cargo clippy -p reflow2-mcp --all-targets

# End-to-end over the real stdio JSON-RPC binary, against a real RocksDB graph.
# Covers what cargo test cannot: the shipped surface, the tool schemas, and the
# JSON an agent actually receives. Stdlib-only Python; non-zero exit on failure.
cargo build -p reflow2-mcp
python3 ../../tools/smoke_mcp.py
```

**Run `smoke_mcp.py` for any change to the tool surface.** The Grok trial found a bug that three
home-grown test layers missed — every one of them was a client we wrote, so all three agreed with
each other and with the server, and all three were wrong. The smoke test drives the real binary
over real stdio. It is the only layer that would have caught it.

## Adding a tool

There is **no central registration list**. `#[tool_router]` derives the tool set from every
`#[tool]`-annotated method in the single `impl ReflowService` block, and `schemars` generates the
input schema from the request struct. So a new tool is:

1. A `#[derive(Debug, Deserialize, JsonSchema)] struct FooReq` — one doc comment per field; the
   agent reads them in `tools/list`, so they are user-facing text, not internal notes.
2. A `#[tool(description = "…")]` method on `ReflowService`. Describe the *use case*, not the
   mechanism — an agent picks tools by matching its problem, not by reading signatures.
3. Optional args go through `#[serde(default)]`. A half-specified argument set should fail loud
   rather than silently defaulting to "return everything".
4. **Classify it: `annotations(read_only_hint = …)` on the `#[tool]` attribute.** `true` if the
   method takes the shared lock (`let g = self.graph.lock()`), `false` if it takes `let mut g`
   or can otherwise mutate the graph — the borrow *is* the classification, so read it off the
   body rather than the name (`gap_to_prompt` and the `reconcile_*` family read like queries but
   record, so they are `false`). `smoke_mcp.py` fails if any served tool omits the hint, so a
   tool cannot ship unclassified (BL-76).
5. **Regenerate the toolsnap.** `cargo build -p reflow2-mcp && python3 ../../tools/toolsnap.py
   --update`, then commit the new `tools/toolsnaps/<name>.json`. CI diffs the served surface
   against these goldens; a surface change is a reviewed diff, never a silent one (BL-76).

## Conventions that are easy to break

- **Return through `ok_json`.** It wraps bare arrays as `{count, items}`, because MCP defines
  `structuredContent` as an object and a spec-compliant client rejects an array outright. That is
  the bug the Grok trial found. Wrapping lives at the one choke point precisely so a new list tool
  cannot reintroduce it by forgetting.
- **Never hold the lock across an `await`.** `let g = self.graph.lock().await;` then call the sync
  core op and let the guard drop.
- **Distinguish caller errors from server faults.** `params_err` / `McpError::invalid_params` for
  a bad argument; `dyno_err` (internal error) for a genuine failure. A typo in a type name is not
  a server fault.
- **A rejection should say what would have worked.** `create_node` / `create_edge` list the valid
  alternatives on failure — a blind trial burned twenty minutes because `Unknown edge type:
  PACKAGES` "tells me I'm wrong without telling me what's right". Keep that property when you
  touch the write path, and keep it failing loud: the rejection is better, not softer.
- **`service.rs` is large and the macro forces one `impl` block** — but request structs, DTOs and
  error helpers do not have to live in it. Factor along that seam rather than growing the file.
