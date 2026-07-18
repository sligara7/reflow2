# Setup — build `reflow2-mcp` and connect your agent

reflow2 runs as a local MCP server your agent talks to. One-time build, then it's just a binary.

## 1. Install the build toolchain

`reflow2-mcp` embeds RocksDB (via `librocksdb-sys`), which compiles C++ — so you need a C++
toolchain plus `clang`/`cmake`. Also install Rust (`https://rustup.rs`) if you don't have it.

### macOS

```bash
xcode-select --install                      # C/C++ toolchain
brew install cmake llvm pkg-config
# If the RocksDB build can't find libclang, point it at Homebrew's llvm:
export LIBCLANG_PATH="$(brew --prefix llvm)/lib"
```

### Debian / Ubuntu / Lubuntu

```bash
sudo apt install -y clang cmake libclang-dev pkg-config
```

## 2. Build the server

```bash
git clone https://github.com/sligara7/reflow2.git
cd reflow2
cargo build -p reflow2-mcp --release        # first build compiles RocksDB (~10 min, then cached)
```

The binary lands at `reflow2/target/release/reflow2-mcp`. Note that path (or copy it onto your
`PATH`, e.g. `cp target/release/reflow2-mcp ~/.local/bin/`).

## 3. Register the MCP server in your project

reflow2's design graph lives in your **project** repo (so it travels with the code). In your
project root, create `.mcp.json` — **grok build and claude code both read this format**:

```json
{
  "mcpServers": {
    "reflow2": {
      "command": "/absolute/path/to/reflow2-mcp",
      "args": ["--graph-path", "./.reflow2/graph"]
    }
  }
}
```

(If `reflow2-mcp` is on your `PATH`, `"command": "reflow2-mcp"` is enough.)

Alternatives for grok build: `grok mcp add`, or the in-session `/mcps` modal, or an entry in
`~/.grok/config.toml` — all read the same server definition.

## 4. Verify

Start your agent in the project repo and ask it to list its tools, or run reflow2's own smoke
check from the reflow2 repo:

```bash
printf '%s\n' \
 '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"smoke","version":"0"}}}' \
 '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
 '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' \
 | ./target/release/reflow2-mcp --graph-path /tmp/reflow2-check | tail -1
```

You should see a JSON line listing ~30 tools (`detect_gaps`, `add_requirement`, …).

## Notes

- The graph directory (`./.reflow2/graph`) is created on first use. Commit it (or an export)
  so the design syncs between people/agents via git.
- Logs go to stderr; stdout is the JSON-RPC channel — don't redirect stdout into logs.
- Cross-platform: RocksDB builds on Windows too (MSVC + `cmake`), but only macOS and Linux are
  exercised today.
