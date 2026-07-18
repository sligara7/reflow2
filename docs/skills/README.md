# Where the kit has to put things, and why

> Part of the **Reflow 2.0** design docs — see **[overview.md](../overview.md)** for the map.

reflow2 ships a consumer kit: agent instructions, seven skills, and an MCP config, installed by
`tools/reflow2_init.py`. Whether an agent ever sees them depends entirely on **which directory
they land in**, and every harness looks in a different place.

This file is the distilled answer. The full vendor documentation it was taken from is beside it:
[vendor-claude-code.md](vendor-claude-code.md) ·
[vendor-grok-cli.md](vendor-grok-cli.md) ·
[vendor-vscode-copilot.md](vendor-vscode-copilot.md)
(sources: [code.claude.com/docs/en/skills](https://code.claude.com/docs/en/skills),
[docs.x.ai/build/features](https://docs.x.ai/build/features/skills-plugins-marketplaces#skills),
[code.visualstudio.com/docs/agent-customization/agent-skills](https://code.visualstudio.com/docs/agent-customization/agent-skills)).

## The table that matters

**Project skills, by harness:**

| Harness | Reads |
|---|---|
| Claude Code | `.claude/skills/` (also every parent dir up to the repo root, and nested dirs on demand) |
| GitHub Copilot / VS Code | `.github/skills/`, `.claude/skills/`, `.agents/skills/` |
| Grok CLI | `.grok/skills/` (walked up to the repo root) |

**MCP server config, by harness:**

| Harness | Reads |
|---|---|
| Claude Code | `.mcp.json` (project scope) |
| Grok CLI | `.grok/config.toml`, **and** `.mcp.json` / `.cursor/mcp.json` / `~/.claude.json` as compatibility, merged below its own config |

## What this means for reflow2 today

**MCP is fine.** The kit writes `.mcp.json`, and Grok explicitly loads `.mcp.json` as a
compatibility source. One file serves both. Nothing to do.

**Skills are not.** The kit installs **only** `.grok/skills/`, which is the *least* portable of
the three locations — no harness but Grok CLI reads it.

| Harness | Sees the kit's seven skills? |
|---|---|
| Grok CLI | yes |
| Claude Code | **no** |
| Copilot / VS Code | **no** |

So a project bootstrapped with `reflow2_init.py` and opened in Claude Code has AGENTS.md
referring by name to skills the agent cannot load. This is not theoretical — it is the current
behaviour, and it is a live instance of the recurring lesson in
[backlog.md](../backlog.md): the capability exists and the surface that should advertise it does
not. Tracked as **BL-22**.

**`.claude/skills/` is the single highest-value target**, because Claude Code *and*
Copilot/VS Code both read it. Installing there plus the existing `.grok/skills/` covers all
three harnesses.

The kit's skills are otherwise spec-compliant — valid `name` matching the directory, a real
`description`. Only the location is wrong. Worth knowing because a bad `name` (slashes, colons,
dots, or a namespace prefix) makes a skill **silently fail to load**, with no error.

## SKILL.md, the part that is actually a shared standard

The file format is common across Claude Code and Copilot, so one skill body works everywhere.

```markdown
---
name: detect-and-ask          # required · lowercase, digits, hyphens · MUST match the directory
                              # name · ≤64 chars · no slashes/colons/dots/prefixes
description: Use before …     # required · ≤1024 chars · says WHAT it does AND WHEN to use it,
                              # because that is what the agent matches on to load it
---

Step-by-step instructions here.
```

Optional fields, both harnesses: `argument-hint`, `user-invocable` (default true),
`disable-model-invocation` (default false — set true for things only a human should trigger),
`context: fork` (run in a subagent so intermediate reasoning stays out of the main context).

Two rules worth repeating because they shape how reflow2's skills should be written:

- **The `description` is the load-bearing field.** It is how an agent decides whether to pull the
  skill in at all. Say when to use it, not just what it does. The kit's existing descriptions do
  this well and are worth copying from.
- **Keep the body short.** Once loaded, a skill's content stays in context across turns, so every
  line is a recurring token cost. Supporting files are referenced with relative Markdown links
  and loaded only when needed.

## Instruction files, for completeness

Grok reads `AGENTS.md`, `CLAUDE.md`, and `.claude/rules/` / `.cursor/rules/` for compatibility,
walking from the repo root down. This is why the
[agents.md](https://agents.md) convention is load-bearing for reflow2 and why AGENTS.md — not a
skill — is the reliable place to put anything an agent *must* see. Skills are depth; AGENTS.md is
the thing that is actually read.
