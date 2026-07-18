# reflow2-mcp

The **agent-native surface** for Reflow 2.0 (surface-plan.md, SP-3): an MCP stdio
server that exposes the deterministic coherence-loop operations as fine-grained
tools, backed by a durable on-disk (RocksDB) design graph that survives across
agent sessions.

The calling coding agent (grok build / claude code) **is** the LLM — there is no
external model provider. LLM-reasoning steps round-trip through the agent via the
collect-then-serve handshake (see `gap_to_prompt`).

## Build

Needs a C++ toolchain for RocksDB (`librocksdb-sys`): on Debian/Ubuntu
`sudo apt install -y clang cmake libclang-dev pkg-config`; on macOS
`xcode-select --install` then `brew install cmake llvm pkg-config`. Then:

```bash
cargo build -p reflow2-mcp --release
# binary at target/release/reflow2-mcp
```

## Connect (grok build / claude code)

Add to the consuming project's `.mcp.json`:

```json
{
  "mcpServers": {
    "reflow2": {
      "command": "reflow2-mcp",
      "args": ["--graph-path", "./.reflow2/graph"]
    }
  }
}
```

The graph directory is created on first use and lives in the project repo, so the
design travels with the code (git-synced). One design per server process.

## Tools

- **DETECT / analyze (read-only):** `detect_gaps`, `propagate_change`,
  `propagate_from`, `graph_report`, `graph_report_markdown`, `detect_defects`,
  `propose_heal`, `evaluate_allocation`, `propose_allocation`, `hierarchy_issues`,
  `surprising_connections`, `dimension_drifts`, `dimension_drift`.
- **Build (mutating):** `add_project`, `add_requirement`, `add_capability`,
  `add_component`, `satisfies`, `allocate`, `contains`, `create_node`,
  `create_edge`, `get_node`, `scan_nodes`, `delete_node`, `apply_heal`.
- **CHANGE (mutating):** `add_epoch`, `add_change_event`, `record_change`.
- **LLM handshake:** `gap_to_prompt` — call with empty `answers` to get
  `{status: "needs_llm", prompts}`, fill them in-context, call again with
  `answers` to get `{status: "ok", prompt}`.

Result convention (mirrors the predecessor `ir2` server): tools return their
payload as JSON directly (no envelope), and partial-success fields
(`unknown_seeds`, `skipped_operations`, `rephrase_degraded`, …) are always
present — no silent fallbacks.

## Deferred (SP-3b)

`ingest` (programmatic LLM extraction) needs a transactional prepare pass so its
mutating collect pass can roll back; it pairs with GENESIS (SP-5). Until then the
agent extracts intent in-context and writes the graph via the `add_*` / `create_*`
tools.
