# Getting started — use reflow2 on your own project

**This folder is the instructions.** It sets up reflow2 as the persistent, coherent *design
brain* for a project you build with a coding agent (grok build or claude code) — e.g. a Unity
game. You write the code; reflow2 remembers the whole design, surfaces the decisions, and tells
the agent what a change breaks.

## Do this, in order

1. **Build the server + install prerequisites — [SETUP.md](SETUP.md).** One-time. It has a
   copy-paste block for **macOS** (installs everything from scratch) and for Debian/Ubuntu, then
   builds the `reflow2-mcp` binary and runs a 3-check **PASS/FAIL** verification. Do this first.
2. **Set up your project** — one command, from the reflow2 repo:
   ```bash
   python3 tools/reflow2_init.py ~/projects/my-thing
   ```
   It installs the agent instructions, the skills and an MCP config with the binary path already
   filled in, and creates the folder the design graph lives in. The project directory is created
   if it doesn't exist.

   It deliberately creates **no `src/`, no build file, no language choice**. What kind of project
   this is comes out of the design, not out of a scaffold — that's the whole idea.

   **Re-run the same command any time to update.** The kit is copied into your project, so it
   would otherwise freeze while reflow2 keeps moving. Re-running refreshes the instructions and
   skills, leaves your design graph and your own files alone, and prints what changed. Use
   `--check` first if you want to see it without writing anything.
3. **Start your agent in your project** (grok build / claude code). It reads `AGENTS.md`, connects
   to the `reflow2` server, and drives the loop — starting by bootstrapping your idea (GENESIS).

That's it. The design graph lives in `$PROJECT/.reflow2/graph`, travels with the repo (git), and
is shared by anyone working on it.

## What each file is

| File | What it does |
|---|---|
| **SETUP.md** | **Start here** — build the server, connect your agent, verify it works |
| `AGENTS.md` | Teaches the agent the reflow2 loop (goes in your project root) |
| `.grok/skills/…` | Auto-triggering workflows (genesis, where-am-i, capture-intent, detect-and-ask, impact-check, check-health, link-artifacts) |
| `mcp.json` | The MCP server registration (grok build and claude code both read it) |

> The rest of this repository (the `crates/`, `docs/`, `schema/` folders) is reflow2's own
> source and design docs — you don't need any of it to *use* reflow2. Just SETUP.md.
