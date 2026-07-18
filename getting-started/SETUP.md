# Setup — build `reflow2-mcp` and connect your agent

reflow2 runs as a local MCP server your agent talks to. One-time build, then it's just a binary.

## 1. Install the build toolchain

`reflow2-mcp` embeds RocksDB (via `librocksdb-sys`), which compiles C++ — so you need a C++
toolchain plus `clang`/`cmake`, and the Rust toolchain. All one-time.

### macOS (from scratch)

Copy-paste this whole block into Terminal. Safe to re-run — steps you already have are no-ops.

```bash
# 1. Homebrew — the macOS package manager (skip if `brew --version` already works):
if ! command -v brew >/dev/null; then
  /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
  eval "$(/opt/homebrew/bin/brew shellenv 2>/dev/null || /usr/local/bin/brew shellenv)"
fi

# 2. Xcode command-line tools (C/C++ compiler). If a dialog pops up, click Install and wait:
xcode-select --install 2>/dev/null || true

# 3. Build dependencies + Rust:
brew install cmake llvm pkg-config
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"

# 4. Let the RocksDB build find libclang (works on Apple Silicon and Intel):
export LIBCLANG_PATH="$(brew --prefix llvm)/lib"
```

Then continue to step 2 in the **same** Terminal window (so `LIBCLANG_PATH` is still set).

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

The binary lands at `reflow2/target/release/reflow2-mcp`. Print its absolute path — you'll paste
it into `.mcp.json` in the next step:

```bash
echo "$(pwd)/target/release/reflow2-mcp"
```

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

## 4. Verify the build works (before wiring up your agent)

Run this checklist from the `reflow2` repo. Each line prints **PASS** or **FAIL** — you should
see three PASSes. It uses a throwaway graph in `/tmp`, so it touches nothing else.

```bash
BIN="$(pwd)/target/release/reflow2-mcp"          # the binary you built
G="/tmp/reflow2-check"; rm -rf "$G"

# Check 1 — the binary runs.
"$BIN" --version >/dev/null 2>&1 && echo "PASS 1: binary runs" || echo "FAIL 1: binary won't run"

# Check 2 — the server starts and lists its tools.
printf '%s\n' \
 '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"check","version":"0"}}}' \
 '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
 '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' \
 | "$BIN" --graph-path "$G" 2>/dev/null | grep -q '"detect_gaps"' \
 && echo "PASS 2: server lists its tools" || echo "FAIL 2: no tools listed"

# Check 3 — a real write round-trips (bootstrap a project, read it back).
printf '%s\n' \
 '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"check","version":"0"}}}' \
 '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
 '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"genesis","arguments":{"project_id":"proj:check","name":"Check"}}}' \
 | "$BIN" --graph-path "$G" 2>/dev/null | grep -q '"created":true' \
 && echo "PASS 3: create + persist works" || echo "FAIL 3: write did not work"
```

**Three PASSes → you're ready** to register the server (step 3) and point your agent at the repo.
If **Check 1** fails, the build didn't finish — re-run step 2 and read the error. If **2 or 3**
fails after Check 1 passes, re-run and copy the full output (`2>&1`) for help.

### Optional: the full loop check

If the three checks pass but something later behaves oddly, run the deeper smoke test. It drives
the same binary through the whole loop — capture intent, detect gaps, register a built file, edit
it, catch the drift, follow it back to the requirement, find a dependency cycle, and reopen the
graph to prove it persisted. Needs only Python 3 (no extra packages), and cleans up after itself:

```bash
python3 tools/smoke_mcp.py --bin target/release/reflow2-mcp
```

It prints a PASS/FAIL line per check and ends with `ALL CHECKS PASSED`. Anything that fails names
the exact step, which is worth pasting if you ask for help.

## Notes

- The graph directory (`./.reflow2/graph`) is created on first use. Commit it (or an export)
  so the design syncs between people/agents via git.
- **One agent at a time.** The graph is a RocksDB store and only one `reflow2-mcp` process can
  hold it. If a second agent starts against the same `--graph-path`, it exits immediately with
  `While lock file: .../LOCK: Resource temporarily unavailable`. That is the lock doing its job,
  not a broken build — close the other agent (or point this one at a different graph) and retry.
  Sharing a design *sequentially* works fine, including across machines via git; several agents
  working the same graph at once is a future effort.
- Logs go to stderr; stdout is the JSON-RPC channel — don't redirect stdout into logs.
- Cross-platform: RocksDB builds on Windows too (MSVC + `cmake`), but only macOS and Linux are
  exercised today.
