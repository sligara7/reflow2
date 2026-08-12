# Setup — one command, once, for every project you will ever have

reflow2 runs as a local MCP server your agent talks to. **You install it once per machine.
There is no per-project setup.**

## Install

If a [GitHub release](https://github.com/sligara7/reflow2/releases) exists for your platform
(Linux x86_64, macOS arm64/x86_64), you need **no toolchain at all** — no Rust, no C++, no
~10-minute RocksDB compile:

```bash
curl -fsSL https://raw.githubusercontent.com/sligara7/reflow2/main/tools/install.sh | sh
```

That is the whole install. It puts the `reflow2-mcp` binary in `~/.local/bin` and the kit in
`~/.local/share/reflow2/kit`, verifies checksums, and then **makes reflow2 globally available to
your agent, in any project on this machine**: the MCP server at user scope, the slash commands in
`~/.claude/commands/`, and the coherence-loop hooks in `~/.claude/settings.json`. The repo is
public, so plain `curl` works with no authentication — you only need the
[GitHub CLI](https://cli.github.com) (`gh auth login`) if you fork it privately.

## Start a design

```bash
mkdir my-thing && cd my-thing
claude                     # or grok / opencode
```

then, in the agent:

```
/genesis I want to build ...      # a new project — a paragraph is plenty
/adopt                            # or: code that already exists
```

**That's it.** No installer to run in the project, no config to write, no restart. The design
graph is created in `my-thing/.reflow2/` the moment you start one.

**Directories you never design stay untouched.** reflow2 is registered with `--only-if-present`,
so a session opened in a folder that has no design gets a server which says exactly that and
serves one tool (`reflow2_start_design`) — and creates nothing. Most folders on your machine
should stay that way, and they will.

To update: re-run the installer. It replaces the binary and kit **together** (the skew a
mismatched pair causes is exactly what `served_by` exists to catch), re-registers everything at
the new path, and never touches your design graphs. To remove the registration:
`reflow2 install --uninstall`.

## Optional: a repo you share

The machine-wide install is for **you**. If a project has other people (or other machines)
working on it, add the in-repo half so *their* agent is told reflow2 governs this code:

```bash
cd my-thing && reflow2 init .
```

That writes a short pointer file, a project-scope MCP config and `.gitignore` lines — and
nothing else. Commit the pointer file and your design export; the graph directory stays local.

---

Everything below is the **from-source path**: for contributors, unsupported platforms, or
running ahead of the latest release.

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

The binary lands at `reflow2/target/release/reflow2-mcp`.

## 3. Make it globally available to projects on this machine

```bash
python3 tools/reflow2_install.py
```

Same thing the release installer does, pointed at the binary you just built: the MCP server at
user scope for Claude Code and OpenCode, the slash commands in `~/.claude/commands/`, the
coherence-loop hooks in `~/.claude/settings.json`, and a `reflow2` command on your `PATH`.
`--check` shows what it would change without writing; `--uninstall` takes it back out.

Then start a design in any directory — see **[Start a design](#start-a-design)** above. Nothing
to do per project.

<details>
<summary>Registering by hand instead</summary>

The server definition, if you would rather write it yourself. `--only-if-present` is what makes a
machine-wide registration safe: without it, every directory you open a session in gets a design
graph created in it.

```bash
claude mcp add -s user reflow2 -- /absolute/path/to/reflow2-mcp \
  --graph-path .reflow2/graph --shared --only-if-present
```

For a single project instead, `.mcp.json` in the project root takes the same shape (grok build
and claude code both read it), and there you can drop `--only-if-present` because the project
plainly has a design:

```json
{
  "mcpServers": {
    "reflow2": {
      "command": "/absolute/path/to/reflow2-mcp",
      "args": ["--graph-path", ".reflow2/graph", "--shared"]
    }
  }
}
```

Alternatives for grok build: `grok mcp add`, or the in-session `/mcps` modal, or an entry in
`~/.grok/config.toml` — all read the same server definition.
</details>

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

### Keeping up to date, from source

```bash
git pull                                     # 1. get the new reflow2
cargo build -p reflow2-mcp --release         # 2. rebuild the server
python3 tools/reflow2_install.py             # 3. re-point the registration at it
```

**The order matters.** Doing 1 and 3 without 2 leaves you with current instructions driving an
old server — same tool names, different behaviour, and nothing obviously wrong until something
misbehaves. Step 3 is only needed when the binary's *path* changes; if it doesn't, the new build
is picked up the next time an agent starts.

Your design graphs are never touched by an update, and nothing in any project needs refreshing —
the skills and working instructions are served by the server, not copied into your repos.

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

**Several agents at once is fine** — see the note below.

## Notes

- The graph directory (`./.reflow2/graph`) is a machine-local RocksDB store, created when you
  start a design. **Don't commit the directory** — add `.reflow2/` to the repo's `.gitignore`,
  which `reflow2 init .` does for you if you run it. To share a
  design via git, commit an **export**: `reflow2-mcp --graph-path .reflow2/graph --export >
  design.json` produces a deterministic, diffable JSON your teammate loads with `--import`.
  The export is the durable record; the RocksDB dir is a local cache of it. The simplest way to
  get one is to ask your agent to export the design — a shared server holds the write lock, so a
  CLI `--export` needs `--stop-shared` first, or `--export-snapshot` for a best-effort read of a
  graph somebody else is holding.
- **Gate CI on the committed export.** `tools/reflow2_check.py` (in the kit) rehashes every
  registered artifact against the working tree and runs the gap detectors, exiting non-zero on
  unaccepted drift or a serious open gap — so the design is checked on every commit, not once a
  session. The **ci-gate** skill has the copy-paste CI step and the honest ways to turn a red
  build green.
- **Several sessions can work one design at once**, and `reflow2_init.py` configures that by
  default (`--shared`). The store itself is still single-writer — only one process may hold the
  RocksDB directory — so the sharing works by there being exactly one *server*: the first session
  starts a detached one, every later session finds it through `.reflow2/graph.server.json` and
  attaches. No session owns it, so the one that happened to start it can close without taking
  anybody else's design brain with it. To release the write lock for maintenance (a CLI
  `--export`, a backup), stop it explicitly:

  ```bash
  reflow2-mcp --graph-path .reflow2/graph --stop-shared
  ```

  If you configured the server by hand rather than with `reflow2_init.py` and left `--shared`
  off, a second agent against the same `--graph-path` exits with `While lock file: .../LOCK:
  Resource temporarily unavailable`. That is the lock doing its job — add `--shared` to the
  config rather than closing the other agent.
- Logs go to stderr; stdout is the JSON-RPC channel — don't redirect stdout into logs.
- Cross-platform: RocksDB builds on Windows too (MSVC + `cmake`), but only macOS and Linux are
  exercised today.
