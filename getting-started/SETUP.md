# Setup — build `reflow2-mcp` and connect your agent

reflow2 runs as a local MCP server your agent talks to. One-time build, then it's just a binary.

## 0. The no-build path: install a prebuilt release

If a [GitHub release](https://github.com/sligara7/reflow2/releases) exists for your platform
(Linux x86_64, macOS arm64/x86_64), you need **no toolchain at all** — no Rust, no C++, no
~10-minute RocksDB compile:

```bash
curl -fsSL https://raw.githubusercontent.com/sligara7/reflow2/main/tools/install.sh | sh
```

It installs the `reflow2-mcp` binary to `~/.local/bin` and the consumer kit to
`~/.local/share/reflow2/kit`, verifies checksums, and prints the exact next command. The repo
is public, so plain `curl` works with no authentication — you only need the
[GitHub CLI](https://cli.github.com) (`gh auth login`) if you fork it privately. Then set up
any project with:

```bash
python3 ~/.local/share/reflow2/kit/tools/reflow2_init.py <your-project> --binary ~/.local/bin/reflow2-mcp
```

To update later, re-run the installer — it replaces the binary and kit **together** (the skew
a mismatched pair causes is exactly what `served_by` exists to catch) and never touches your
design graphs. Everything below is the from-source path: for contributors, unsupported
platforms, or running ahead of the latest release.

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

**Or skip all of it:** `python3 tools/reflow2_init.py /path/to/project` writes the config for
every agent it knows about — `.mcp.json` (claude code, and grok build reads it too),
`opencode.json`, and `.vscode/mcp.json` — with the binary path resolved, and installs the skills
into every directory those agents search. It merges rather than overwrites, so other MCP servers
and your own settings survive. Re-run it any time to update.

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

### Set up a project

From the reflow2 repo:

```bash
python3 tools/reflow2_init.py ~/projects/my-thing
```

That installs everything the agent needs and points the MCP config at the binary you just built.
Re-run it any time to pick up reflow2 updates — it won't touch your design graph or your own
files. `--check` shows what would change without writing.

### Keeping up to date

reflow2 moves; your project's copy of the kit doesn't. To pick up changes, from the reflow2 repo:

```bash
git pull                                    # 1. get the new reflow2
cargo build -p reflow2-mcp --release        # 2. rebuild the server
python3 tools/reflow2_init.py ~/projects/my-thing   # 3. refresh the project
```

**The order matters.** Doing 1 and 3 without 2 leaves your project with current instructions
driving an old server — same tool names, different behaviour, and nothing obviously wrong until
something misbehaves. `reflow2_init.py` checks for exactly that and warns you; run it with
`--check` first if you'd rather look before touching anything.

Your design graph and your own files are never touched by an update.

### Starting a design

Open your agent in the project folder and prompt it with **a short overview of what you're
trying to design or build** — a paragraph is plenty, in your own words. That's the whole
kickoff: reflow2 bootstraps from it and starts asking you about the parts you left out.

You don't need to know systems engineering, and you don't need the brief to be complete. The
gaps are the point — it will find them and ask.
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

## Stopping and picking it up again

**Just stop.** The design lives in `./.reflow2/graph` on disk, not in the agent's head, so
closing the session loses nothing. There is no save step and nothing to flush.

When you come back, open your agent in the same folder and ask it something like:

> *"Where are we with this? Give me the overview, then let's carry on."*

It will read the graph and tell you what the design says, what's been decided, and what's still
open. Ask that any time you lose the thread mid-session too — you don't have to be resuming.

If the agent asks *"shall we start building or keep filling in gaps?"* and you'd rather stop for
the day, stopping is a perfectly good answer. Everything decided so far is already recorded.

**One agent at a time** — see the note below.

## Notes

- The graph directory (`./.reflow2/graph`) is a machine-local RocksDB store, created on first
  use — the installer gitignores `.reflow2/`, so **don't commit the directory**. To share a
  design via git, commit an **export**: `reflow2-mcp --graph-path .reflow2/graph --export >
  design.json` produces a deterministic, diffable JSON your teammate loads with `--import`.
  The export is the durable record; the RocksDB dir is a local cache of it.
- **Gate CI on the committed export.** `tools/reflow2_check.py` (in the kit) rehashes every
  registered artifact against the working tree and runs the gap detectors, exiting non-zero on
  unaccepted drift or a serious open gap — so the design is checked on every commit, not once a
  session. The **ci-gate** skill has the copy-paste CI step and the honest ways to turn a red
  build green.
- **One agent at a time.** The graph is a RocksDB store and only one `reflow2-mcp` process can
  hold it. If a second agent starts against the same `--graph-path`, it exits immediately with
  `While lock file: .../LOCK: Resource temporarily unavailable`. That is the lock doing its job,
  not a broken build — close the other agent (or point this one at a different graph) and retry.
  Sharing a design *sequentially* works fine, including across machines via git; several agents
  working the same graph at once is a future effort.
- Logs go to stderr; stdout is the JSON-RPC channel — don't redirect stdout into logs.
- Cross-platform: RocksDB builds on Windows too (MSVC + `cmake`), but only macOS and Linux are
  exercised today.
