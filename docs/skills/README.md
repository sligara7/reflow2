# Where the kit has to put things, and why

> Part of the **Reflow 2.0** design docs — see **[overview.md](../overview.md)** for the map.

reflow2 ships a consumer kit: agent instructions, eleven skills, and an MCP config, installed by
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

## What the kit installs, and why (BL-22, done)

`reflow2_init.py` now writes skills to **every** directory some harness searches, and an MCP
config in **every** format:

| Harness | Skills | MCP |
|---|---|---|
| Claude Code | `.claude/skills/` | `.mcp.json` |
| OpenCode | `.claude/skills/` | `opencode.json` |
| Copilot / VS Code | `.claude/skills/` | `.vscode/mcp.json` |
| Grok CLI | `.grok/skills/` | `.mcp.json` (compat) |

The kit's source of truth is `getting-started/skills/`, harness-neutral, copied to each
destination. Adding a harness is one line in `TREES` or `MCP_CONFIGS`.

**Before this, the kit installed `.grok/skills/` alone** — the narrowest-reach option — so a
project opened in Claude Code had an AGENTS.md naming seven skills the agent could not load. It
also explains the Grok trial's *"Skills are files, not auto-injected"*: opencode searches
`.opencode/`, `.claude/` and `.agents/`, and the kit had written `.grok/`. Not a registration
problem — the directory was never on the search path.

**The configs are merged, never overwritten.** `opencode.json` is that tool's entire config
(theme, model, permissions), and any project may already run other MCP servers; both must
survive. Merging also fixed a silent failure: the installer used to bail whenever `.mcp.json`
existed without a `reflow2` entry, so a project already using one MCP server never got reflow2
at all — while the run reported success.

A config whose `reflow2` entry points somewhere else is left alone and said so; `--force-mcp`
repoints it. Malformed JSON is reported, never rewritten.

The skills themselves were always spec-compliant — valid `name` matching the directory, a real
`description` — which is why this was purely an install-path bug. Keep them that way: a bad
`name` (slashes, colons, dots, or a namespace prefix) makes a skill **silently fail to load**,
with no error anywhere.

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
