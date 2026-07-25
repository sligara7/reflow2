# Upgrading to v0.12.0 — the skills *and the instructions* leave your repository

> Part of the **Reflow 2.0** design docs — see **[overview.md](overview.md)** for the full map and
> reading order.

**Read this before updating.** It is the first release that *removes* files from your project, and
it is the last one that will need to touch your project at all.

## What changes

Until now, installing or updating reflow2 copied fifteen skills into `.claude/skills/` and
`.grok/skills/`, plus an instructions file. Every reflow2 release therefore rewrote thirty-odd
files in a repository that has nothing to do with reflow2's release cycle — and an installed kit
silently froze at install time while reflow2 moved on. reflow2's own installed manifest read
**0.8.0 with twelve skills** while the project was at 0.11.0 with fifteen. Four releases stale, and
nothing anywhere noticed.

From v0.12.0 both the skills **and the ~20 KB working-instructions document** are **compiled into
the reflow2 binary and served over MCP** (`dec:skills-served`, `req:thin-install`, on Alex's
proposal). The instruction file was the last thing that still churned: it changed with almost every
release, so an upgrade produced a diff in a repository that has nothing to do with reflow2's release
cycle. What lands in your project now is a **2.4 KB pointer** that names where to look and is
deliberately stable.

- The **catalogue** — one trigger line per skill — arrives in the server's handshake instructions,
  which your client puts into the agent's context automatically.
- **`list_skills`** returns the full descriptions.
- **`get_skill`** returns one skill in full, to be read *before* the work it covers.
- **`get_instructions`** returns the full working instructions — the loop, the standing rules, what
  to do first on an existing design. The handshake tells your agent to call it before its first
  design action.

## What happens when you update

Run `reflow2_init.py <project>` once more (or `--check` first — it lists every change before
anything moves). It will:

1. **Delete the skill files it previously installed**, naming the reason: *"now served by the MCP
   server — call list_skills / get_skill"*.
1b. **Replace the installed `AGENTS.md` with the pointer** (unless you edited it, in which case it
   is left alone and reported). The content is not lost — it is what `get_instructions` returns.
2. **Keep any skill file you edited**, and say so loudly — because your harness *does* auto-load a
   file in `.claude/skills/`, and a served skill is never offered, so **your edited copy silently
   wins over every future release of that skill**. That is the one case where doing nothing has a
   cost: delete the file to follow the served skill, or keep it deliberately.
3. Leave everything else alone.

After that, upgrading reflow2 is `git pull && cargo build` (or the installer's one-liner) and
**your project does not change**. There is a test that says so:
`test_a_new_release_touches_nothing_in_the_project`.

## What stays in your repository

- `AGENTS.md` (or `REFLOW2.md` if you already had one), plus a pointer line in whatever instruction
  files you use.
- `.mcp.json` / `opencode.json` / `.vscode/mcp.json`.
- `.reflow2/` — your design graph, machine-local and gitignored.

## The trade, stated plainly

A skill in `.claude/skills/` is **auto-matched by your harness** from its description: the agent
never asks for it, the harness offers it. A served skill has no such magic — the agent has to know
it exists, which is why the catalogue rides the handshake instructions.

So this trades a little discovery magic for never being stale. Anthony's call, 2026-07-25:
*"we need to get rid of the issue of a stale release. If going with the catalogue-in-instruction is
the cost, I think it is worth that."*

If a particular skill matters enough to you that you want harness-native discovery, keep a copy in
your own `.claude/skills/` deliberately — the installer will never touch a file you wrote, and a
local skill takes precedence over the served one of the same name.

## New tools

| Tool | Purpose |
|---|---|
| `list_skills` | The catalogue: name + the full description an agent matches on |
| `get_skill` | One skill in full, by name |
| `get_instructions` | The full working instructions for driving a project with reflow2 |

No other tool changed shape; 107 toolsnaps, the other 104 unmoved.

## What "no diff on upgrade" is worth, and how it is checked

`tools/test_init.py::test_a_new_release_touches_nothing_in_the_project` simulates a *newer* reflow2
against a project set up by an older one and asserts that no skill file and no instruction content
moves. The earlier version of that test only re-ran the same release, which proved almost nothing —
worth saying, because the claim in this document is exactly the one that test now backs.
