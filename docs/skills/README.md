# Where the kit has to put things, and why

> Part of the **Reflow 2.0** design docs — see **[overview.md](../overview.md)** for the map.

reflow2 ships a consumer kit: agent instructions, seven skills, and an MCP config, installed by
`tools/reflow2_init.py`. Whether an agent ever sees them depends entirely on **which directory
they land in**, and every harness looks in a different place.

This file is the distilled answer. The full vendor documentation it was taken from is beside it:
[vendor-claude-code.md](vendor-claude-code.md) ·
[vendor-grok-cli.md](vendor-grok-cli.md) ·
[vendor-opencode.md](vendor-opencode.md) ·
[vendor-vscode-copilot.md](vendor-vscode-copilot.md)
(sources: [code.claude.com/docs/en/skills](https://code.claude.com/docs/en/skills),
[docs.x.ai/build/features](https://docs.x.ai/build/features/skills-plugins-marketplaces#skills),
[opencode.ai/docs/skills](https://opencode.ai/docs/skills/),
[code.visualstudio.com/docs/agent-customization/agent-skills](https://code.visualstudio.com/docs/agent-customization/agent-skills)).

## The table that matters

**Project skills, by harness.** `.claude/skills/` has become the de-facto shared location —
three of the four read it, two of them describing it in so many words as "Claude-compatible".

| Harness | Reads | `.claude/skills/` |
|---|---|---|
| Claude Code | `.claude/skills/` (plus every parent up to the repo root, and nested dirs on demand) | ✅ native |
| OpenCode | `.opencode/skills/`, `.claude/skills/`, `.agents/skills/` | ✅ |
| Copilot / VS Code | `.github/skills/`, `.claude/skills/`, `.agents/skills/` | ✅ |
| Grok CLI | `.grok/skills/` (walked up to the repo root) | ❌ |

**MCP server config, by harness.** No such convergence — and unlike skills, only Grok offers a
compatibility shim.

| Harness | Reads |
|---|---|
| Claude Code | `.mcp.json` (project scope) |
| Grok CLI | `.grok/config.toml`, **and** `.mcp.json` / `.cursor/mcp.json` / `~/.claude.json` as compatibility, merged below its own config |
| OpenCode | `opencode.json` — **no `.mcp.json` compatibility** |
| Copilot / VS Code | `.vscode/mcp.json` |

## What this means for reflow2 today

The kit installs `.grok/skills/` and writes `.mcp.json`. Against the tables above:

| Harness | Sees the seven skills | Finds the MCP server |
|---|---|---|
| Grok CLI | yes | yes (via `.mcp.json` compat) |
| Claude Code | **no** | yes |
| OpenCode | **no** | **no** (needs `opencode.json`) |
| Copilot / VS Code | **no** | **no** (needs `.vscode/mcp.json`) |

**Skills land in the least portable directory of the four.** So a project bootstrapped with
`reflow2_init.py` and opened in Claude Code — the harness this repo is developed with — has an
AGENTS.md naming seven skills the agent cannot load.

**This is what the Grok trial hit.** Its finding, *"Skills are files, not auto-injected … the
available_skills list only listed `council` and `customize-opencode`"*, was not a subtle
registration problem: opencode searches `.opencode/`, `.claude/` and `.agents/`, and the kit had
written `.grok/`. The directory was simply not on the search path.

**`.claude/skills/` is the single highest-value change**, covering Claude Code, OpenCode and
Copilot at once. Adding it beside the existing `.grok/skills/` covers all four harnesses.
`.agents/skills/` is the vendor-neutral name but Claude Code does not read it, so it is strictly
worse as a single choice.

**MCP is a second, smaller gap.** `.mcp.json` serves Claude Code and Grok, but OpenCode and
VS Code each need their own file. Whoever ran the Grok trial must have wired `opencode.json` by
hand — friction the installer could remove.

Both are the recurring lesson in [backlog.md](../backlog.md): the capability exists and the
surface that should advertise it does not. Tracked as **BL-22**.

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

`name` must match `^[a-z0-9]+(-[a-z0-9]+)*$` — no leading, trailing or doubled hyphens — and must
equal its directory name. Names must also be unique *across all searched locations*, which
matters once the kit installs into two directories: the same skill in `.claude/skills/` and
`.grok/skills/` is fine (different harnesses read different ones), but two *different* skills
sharing a name are not.

Optional fields vary by harness and unknown ones are ignored, so extras are safe but not
portable. Claude Code and Copilot support `argument-hint`, `user-invocable` (default true),
`disable-model-invocation` (default false — set true for things only a human should trigger),
and `context: fork` (run in a subagent so intermediate reasoning stays out of the main context).
OpenCode recognises only `name`, `description`, `license`, `compatibility` and `metadata`.
**Keep reflow2's skills to `name` + `description`** so one file works everywhere.

When a skill does not appear: check `SKILL.md` is spelled in capitals, that the frontmatter has
both required fields, that the name matches the directory, and that the directory is one the
harness actually searches — that last one is what bit us.

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
