# reflow2 consumer template

Drop-in files that teach a **coding agent** (grok build, claude code) to use reflow2 as the
persistent design brain for *your* project (surface-plan.md, SP-4). This is **not** for
developing reflow2 itself — it's for a project that reflow2 keeps coherent (e.g. the softball
Unity game).

## What's here

| File | Purpose | Copy to |
|---|---|---|
| `AGENTS.md` | Teaches the agent the reflow2 coherence loop | your project repo root |
| `SETUP.md` | Build `reflow2-mcp` + register it (macOS / Linux) | keep as reference |
| `.grok/skills/*/SKILL.md` | Modular loop-step workflows the agent auto-triggers | your project repo root |
| `mcp.json` | The MCP server registration (Claude-style; Grok reads it too) | your project as `.mcp.json` |

## Use it

1. Follow `SETUP.md` to build the `reflow2-mcp` binary and install the RocksDB toolchain.
2. Copy `AGENTS.md`, `.grok/`, and `mcp.json` (→ `.mcp.json`) into your project repo. From this
   directory, into your project at `$PROJECT`:
   ```bash
   cp AGENTS.md "$PROJECT/AGENTS.md"
   cp -r .grok  "$PROJECT/.grok"        # the skills — a hidden dir; `cp *` would miss it
   cp mcp.json  "$PROJECT/.mcp.json"    # then set "command" to your reflow2-mcp path
   ```
3. Start your agent (grok build / claude code) in that repo. It reads `AGENTS.md`, connects to
   the `reflow2` MCP server, and drives the loop. The design graph persists in
   `./.reflow2/graph` and travels with the repo (git-synced).

Both agents read the same `.mcp.json` and the same `./.reflow2/graph`, so two people on two
agents work one shared design.
