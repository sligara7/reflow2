#!/usr/bin/env python3
"""Reflow2's own command surface lives in several places. Check they agree.

WHY THIS EXISTS
---------------
Measured 2026-09-01 (`fact:the-command-surface-has-five-copies-and-nothing-
reconciles-them`): the surface a user meets is spread across skill directories,
command directories and an alias table in Rust, and nothing compared them. Two
things had silently drifted — four commands existed only in this repo, and the
alias table was missing three of eleven entries, which reproduced the exact
2026-08-19 failure the table was written to fix.

The deeper reason is a split guarantee. Skills are SERVED from the binary
(`dec:skills-served`) precisely so they cannot drift from the running version.
Commands are COPIED FILES, so they can — and the copied half is the one a user
in another project actually meets.

WHAT IT CHECKS, and each is a FAIL because each has already happened or is one
edit away:

  1. The three skill directories are byte-identical.
  2. The command source and its in-repo copy are byte-identical.
  3. Every served skill is reachable by some command.
  4. Every command either invokes a skill that exists, or is one of the
     tool-backed commands that deliberately has none.
  5. Every command whose name differs from the skill it invokes appears in
     COMMAND_ALIASES — the case that broke, twice.

WHAT IT ONLY REPORTS
--------------------
User-scope commands (`~/.claude/commands/`) live OUTSIDE the repository, so CI
cannot see them and a green build says nothing about them. They are reported as
a note when this runs on a machine that has them, and never fail the build —
claiming to have checked something invisible to CI would be worse than silence.
"""

from __future__ import annotations

import filecmp
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SKILL_DIRS = [
    ROOT / "getting-started" / "skills",
    ROOT / ".claude" / "skills",
    ROOT / ".grok" / "skills",
]
COMMAND_SRC = ROOT / "getting-started" / "commands"
COMMAND_COPY = ROOT / ".claude" / "commands"
SKILLS_RS = ROOT / "crates" / "reflow2-mcp" / "src" / "skills.rs"
USER_COMMANDS = pathlib.Path.home() / ".claude" / "commands"

# A command whose prose names no skill because it calls a tool directly. Kept
# here rather than inferred: "names no skill" and "forgot to name its skill"
# are different facts, and only one of them is fine.
TOOL_BACKED = {"debt", "decisions", "next"}

findings: list[str] = []
notes: list[str] = []


def names(d: pathlib.Path) -> set[str]:
    if not d.is_dir():
        return set()
    if d.name == "skills":
        return {p.name for p in d.iterdir() if p.is_dir()}
    return {p.stem for p in d.glob("*.md")}


def same_tree(a: pathlib.Path, b: pathlib.Path) -> list[str]:
    """Differing entries between two directories, recursively."""
    cmp = filecmp.dircmp(str(a), str(b))
    out = list(cmp.left_only) + list(cmp.right_only) + list(cmp.diff_files)
    for sub in cmp.common_dirs:
        out += [f"{sub}/{x}" for x in same_tree(a / sub, b / sub)]
    return out


def skill_invoked_by(cmd: pathlib.Path) -> str | None:
    """The skill a command file says to use, from `the **name** skill`."""
    m = re.search(r"\*\*([a-z0-9-]+)\*\*\s+skill", cmd.read_text(encoding="utf-8"))
    return m.group(1) if m else None


def aliases_from_rust() -> dict[str, str]:
    """The `("alias", Ok("skill"))` pairs, read from the source of truth."""
    text = SKILLS_RS.read_text(encoding="utf-8")
    return dict(re.findall(r'\(\s*"([a-z0-9-]+)"\s*,\s*Ok\("([a-z0-9-]+)"\)\s*\)', text))


# 1 — the skill directories
source = SKILL_DIRS[0]
for other in SKILL_DIRS[1:]:
    if not other.is_dir():
        findings.append(f"skill directory missing entirely: {other.relative_to(ROOT)}")
        continue
    if diff := same_tree(source, other):
        findings.append(
            f"{other.relative_to(ROOT)} has drifted from the served source: {sorted(diff)}"
        )

# 2 — the command directories
if cmd_diff := same_tree(COMMAND_SRC, COMMAND_COPY):
    findings.append(
        f"{COMMAND_COPY.relative_to(ROOT)} has drifted from "
        f"{COMMAND_SRC.relative_to(ROOT)}: {sorted(cmd_diff)}"
    )

skills = names(source)
commands = names(COMMAND_SRC)
aliases = aliases_from_rust()

# 3 and 5 — every skill reachable, and every rename declared
reached: dict[str, str] = {}
for name in sorted(commands):
    invoked = skill_invoked_by(COMMAND_SRC / f"{name}.md")
    if invoked is None:
        if name not in TOOL_BACKED:
            findings.append(
                f"/{name} names no skill and is not one of the tool-backed commands "
                f"({', '.join(sorted(TOOL_BACKED))}) — either it lost its skill or it "
                f"belongs in TOOL_BACKED"
            )
        continue
    # 4 — it must name a skill that exists
    if invoked not in skills:
        findings.append(f"/{name} says it uses the '{invoked}' skill, which does not exist")
        continue
    reached.setdefault(invoked, name)
    if invoked != name and aliases.get(name) != invoked:
        findings.append(
            f"/{name} is served as the '{invoked}' skill but is NOT in COMMAND_ALIASES — "
            f"get_skill('{name}') will refuse with a bare list of names, which is the "
            f"2026-08-19 failure that table exists to prevent"
        )

for skill in sorted(skills - set(reached)):
    findings.append(f"the '{skill}' skill has no command — nobody can type their way to it")

# The note CI cannot make into a check.
if USER_COMMANDS.is_dir():
    user = names(USER_COMMANDS)
    if missing := sorted(commands - user):
        notes.append(
            f"{len(missing)} command(s) are in this repo but NOT at user scope, so every OTHER "
            f"project on this machine is without them: {missing}. Outside the repo, so this is "
            f"a note and never a failure."
        )
    if extra := sorted(user - commands):
        notes.append(f"user scope carries command(s) this repo does not: {extra}")
else:
    notes.append("no user-scope command directory on this machine; nothing to compare.")

for n in notes:
    print(f"  note  {n}")
for f in findings:
    print(f"  FAIL  {f}")

print(
    f"\ncommand surface: {'FAILED' if findings else 'OK'} — "
    f"{len(skills)} skill(s), {len(commands)} command(s), {len(aliases)} alias(es), "
    f"{len(findings)} finding(s), {len(notes)} note(s)."
)
sys.exit(1 if findings else 0)
