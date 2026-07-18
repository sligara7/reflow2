# Getting started — use reflow2 on your own project

**This folder is the instructions.** It sets up reflow2 as the persistent, coherent *design
brain* for a project you build with a coding agent (grok build or claude code) — e.g. a Unity
game. You write the code; reflow2 remembers the whole design, surfaces the decisions, and tells
the agent what a change breaks.

## Do this, in order

1. **Build the server + install prerequisites — [SETUP.md](SETUP.md).** One-time. It has a
   copy-paste block for **macOS** (installs everything from scratch) and for Debian/Ubuntu, then
   builds the `reflow2-mcp` binary and runs a 3-check **PASS/FAIL** verification. Do this first.
2. **Copy this kit into your project** (from this folder, into your project at `$PROJECT`):
   ```bash
   cp AGENTS.md "$PROJECT/AGENTS.md"
   cp -r .grok  "$PROJECT/.grok"        # the skills — a hidden dir; `cp *` would miss it
   cp mcp.json  "$PROJECT/.mcp.json"    # then set "command" to the path SETUP.md printed
   ```
3. **Start your agent in your project** (grok build / claude code). It reads `AGENTS.md`, connects
   to the `reflow2` server, and drives the loop — starting by bootstrapping your idea (GENESIS).

That's it. The design graph lives in `$PROJECT/.reflow2/graph`, travels with the repo (git), and
is shared by anyone working on it.

## What each file is

| File | What it does |
|---|---|
| **SETUP.md** | **Start here** — build the server, connect your agent, verify it works |
| `AGENTS.md` | Teaches the agent the reflow2 loop (goes in your project root) |
| `.grok/skills/…` | Auto-triggering workflows (genesis, detect-and-ask, impact-check, …) |
| `mcp.json` | The MCP server registration (grok build and claude code both read it) |

> The rest of this repository (the `crates/`, `docs/`, `schema/` folders) is reflow2's own
> source and design docs — you don't need any of it to *use* reflow2. Just SETUP.md.
