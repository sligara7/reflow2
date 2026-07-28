# Getting started — use reflow2 on your own project

**This folder is the instructions.** It sets up reflow2 as the persistent, coherent *design
brain* for a project you build with a coding agent (grok build or claude code) — e.g. a Unity
game. You write the code; reflow2 remembers the whole design, surfaces the decisions, and tells
the agent what a change breaks.

## Do this, in order

1. **Install reflow2 — once, for this machine — [SETUP.md](SETUP.md).**
   ```bash
   curl -fsSL https://raw.githubusercontent.com/sligara7/reflow2/main/tools/install.sh | sh
   ```
   That installs the server and registers it with your agent for **every** project: the MCP
   server at user scope, the slash commands, and the coherence-loop hooks. SETUP.md also has the
   from-source path (macOS and Debian/Ubuntu copy-paste blocks, plus a 3-check PASS/FAIL
   verification) for contributors and unsupported platforms.
2. **Start a design, in any directory:**
   ```bash
   mkdir my-thing && cd my-thing && claude
   ```
   then `/genesis` with a paragraph about what you want to build, or `/adopt` for code that
   already exists.

That's it — **there is no step 3, and no per-project setup**. The design graph is created in
`$PROJECT/.reflow2/graph` when you start a design; directories where you never start one are
left completely alone.

Nothing is scaffolded for you: **no `src/`, no build file, no language choice**. What kind of
project this is comes out of the design, not out of a template — that's the whole idea.

**Optional, for a repo other people work on:** `reflow2 init .` adds a short pointer file and a
project-scope MCP config, so a teammate's agent is told reflow2 governs this code. Solo work
needs none of it.

## What each file is

| File | What it does |
|---|---|
| **SETUP.md** | **Start here** — install once, start a design, verify it works |
| `commands/…` | The ten slash commands, installed into `~/.claude/commands/` by the machine-wide install. `/genesis` and `/adopt` are the two front doors; `/where`, `/gaps`, `/health`, `/debt`, `/decisions`, `/req`, `/kpp`, `/brainstorm` are the loop. Each is a pointer at a served skill, so it carries nothing that can go stale |
| `skills/…` | The 15 workflows: genesis, adopt, brainstorm, capture-intent, kpp-proposal, parallel-work, where-am-i, detect-and-ask, impact-check, check-health, link-artifacts, revise-design, retire-from-design, ci-gate, report-friction. `adopt` is the one for a system that already exists; [SKILLS.md](../SKILLS.md) says which to reach for when. **Served by the server, not installed** — `list_skills` names them, `get_skill` fetches one, and they always match the running binary |
| `AGENTS.md` | Teaches the agent the reflow2 loop. Only needed in a repo you share — `reflow2 init .` puts the pointer there |
| `mcp.json` | A reference copy of the server registration. The installers write the real ones — user scope for the machine-wide install, `.mcp.json` / `opencode.json` / `.vscode/mcp.json` for `reflow2 init` |

> The rest of this repository (the `crates/`, `docs/`, `schema/` folders) is reflow2's own
> source and design docs — you don't need any of it to *use* reflow2. Just SETUP.md.
