#!/usr/bin/env python3
"""Set up (or update) reflow2 in a project.

    python3 tools/reflow2_init.py ~/projects/my-thing
    python3 tools/reflow2_init.py ~/projects/my-thing --check    # what would change
    python3 tools/reflow2_init.py ~/projects/my-thing            # re-run to update

Installs the **design environment** and nothing else: a short pointer file, an
MCP config with the binary path already filled in, and the directory the design
graph lives in.

It deliberately creates no `src/` layout, no build file, no language choice, no
project scaffolding of any kind — because *you don't know the project type yet,
and neither should this script*. What kind of project this is, and therefore how
its code should be laid out, is a decision the design loop makes with you. A
scaffold that guessed would be committing a design decision before the design
exists, which is the thing reflow2 is for.

**Neither the skills nor the working instructions are installed** — both are
served by the MCP server (`dec:skills-served`, `req:thin-install`), so they
always match the reflow2 you are running and **upgrading reflow2 produces no
diff in your project**. What lands in your repo is a short, stable pointer file,
a pointer line in whatever instruction files you already have, the MCP configs,
and `.reflow2/`. The first run after this change also *removes* the skill copies
an older kit left behind — untouched ones deleted, edited ones kept and
reported, because your harness would go on loading them in preference to the
served ones.

Re-run it any time; it leaves your design graph and your own files alone and
tells you exactly what changed. `--check` first if you want the list before
anything moves.

Standard library only.
"""
from __future__ import annotations

import argparse
import datetime
import filecmp
import hashlib
import re
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
KIT = REPO / "getting-started"
STAMP = ".reflow2/kit-version.json"

# Everything installed, as (source in the kit, destination in the project).
# Text only — nothing here implies a project type.
#
# POINTER.md, not AGENTS.md, since 2026-07-25 (req:thin-install). The full
# working instructions are ~20 KB and change with almost every release, so
# installing them meant every reflow2 upgrade produced a diff in a repository
# that has nothing to do with reflow2 — the same defect as the copied skills,
# in the one file left. They are served by `get_instructions` now; what lands
# here is a short pointer that is deliberately stable and says where to look.
FILES = [
    (KIT / "POINTER.md", "AGENTS.md"),
]

# Where kit content goes when the project already owns that filename. AGENTS.md
# is the case that matters: every brownfield target has one, and it is the file
# the project actually runs on.
SIDECAR = {"AGENTS.md": "REFLOW2.md"}

# Every instruction-file convention an agent might read *first*. The pointer
# line goes into all of these that exist, not just the one whose name we
# happen to use ourselves.
#
# Found the hard way: storyflow carries CLAUDE.md and no AGENTS.md, so the
# installer saw nothing to protect, wrote a fresh AGENTS.md, and left the file
# Claude Code actually reads with no mention of reflow2 — the whole kit
# invisible on the primary path. The earlier fix protected precisely the wrong
# filename, because the fixture it was built from had an AGENTS.md, because
# that is what the backlog note had recorded. A fixture built from the recorded
# problem reproduces the recorded problem, not the real one
# (docs/trials/2026-07-20-adopt-storyflow.md, F1).
INSTRUCTION_FILES = [
    "AGENTS.md",
    "CLAUDE.md",
    "GEMINI.md",
    ".github/copilot-instructions.md",
    ".cursorrules",
    ".windsurfrules",
]


def foreign_owner(src: Path, dst: Path) -> str | None:
    """Why `dst` must not be overwritten, or None if it is ours to manage.

    A file we wrote (or an older version of it) is ours to refresh. A file the
    project wrote is not, and clobbering it is a silent destruction of the
    instructions the project runs on — reported, before this check existed, as an
    ordinary `AGENTS.md` line in the install summary.

    Identified by the kit's own first heading rather than a marker comment, so
    kits installed before this check are still recognised as ours.
    """
    if not dst.exists():
        return None
    try:
        head = src.read_text(encoding="utf-8").lstrip().splitlines()[0].strip()
        existing = dst.read_text(encoding="utf-8").lstrip().splitlines()
    except (OSError, UnicodeDecodeError, IndexError):
        return "unreadable, so not safely replaceable"
    if not existing:
        return None
    return None if existing[0].strip() == head else "it is not a reflow2 kit file"

# NOTHING IS COPIED ANY MORE — the skills are served by the MCP server
# (`dec:skills-served`, accepted 2026-07-25).
#
# This list held `.claude/skills` and `.grok/skills`, because no two harnesses
# search the same directory and a skill outside the one yours reads is a file
# nobody loads. That was correct, and it was also the whole problem: fifteen
# skills copied into two trees meant every reflow2 release rewrote thirty-odd
# files in a project that had nothing to do with it — and an installed kit
# silently froze while reflow2 moved on. reflow2's own manifest sat FOUR
# releases stale (0.8.0/12 skills against 0.11.0/15) and nothing noticed.
#
# Anthony's brother named the fix: setup should be a paragraph plus a server
# address, "and updates would be confined to the reflow package". So the skills
# are compiled into the binary and served by `list_skills` / `get_skill`, with
# the catalogue in the server instructions — which is the one channel a client
# puts in the agent's context without being asked.
#
# THE LIST IS KEPT, EMPTY, ON PURPOSE. Emptying it is what makes the existing
# prune path below remove the copies a previous install left — untouched ones
# deleted, edited ones kept and reported. Deleting the mechanism would have
# stranded every existing install with skills that shadow the served ones.
# Directory trees copied wholesale into the project.
#
# SLASH COMMANDS ARE THE ONE EXCEPTION TO `dec:skills-served`, and the exception
# is narrow enough to state precisely (Anthony's call, 2026-07-28,
# req:kit-reaches-the-agent).
#
# That decision says nothing is copied into a consumer repo, because a copy goes
# stale and the agent then loads last release's content in preference to what the
# server would serve. The skills themselves are the case it was written for: they
# are long, they change with almost every release, and a stale one is wrong in
# ways nobody notices.
#
# A command is a different animal. Each is four lines that name a skill and say
# how to report it — no version-coupled content, nothing that goes stale when the
# skill behind it changes, because the skill is still fetched fresh at use time.
# What a command CAN go stale about is naming a skill that no longer exists, and
# `skill_lint` now fails on exactly that, so the one real failure mode is caught
# in this repo rather than discovered in someone else's.
#
# Without them a consumer install is experienced as broken rather than thin: the
# skills are reachable but nothing tells you they are, which is the same
# invisibility this requirement was raised for.
TREES: list[tuple[Path, str]] = [
    (KIT / "commands", ".claude/commands"),
]


# Frontmatter every harness agrees on. A skill whose `name` is malformed, or
# does not match its directory, is **silently ignored** — no error anywhere, it
# simply never loads. That is the failure this project forbids elsewhere, so the
# installer refuses to ship one rather than letting it disappear quietly.
SKILL_NAME = re.compile(r"[a-z0-9]+(-[a-z0-9]+)*")
PORTABLE_FIELDS = {"name", "description", "license", "compatibility", "metadata"}


def check_skills() -> list[str]:
    """Problems that would make an installed skill fail to load."""
    problems = []
    root = KIT / "skills"
    for d in sorted(p for p in root.iterdir() if p.is_dir()):
        f = d / "SKILL.md"
        if not f.exists():
            problems.append(f"{d.name}: no SKILL.md (must be capitalised)")
            continue
        text = f.read_text()
        m = re.match(r"^---\n(.*?)\n---\n", text, re.S)
        if not m:
            problems.append(f"{d.name}: no YAML frontmatter")
            continue
        fm = dict(re.findall(r"^(\w[\w-]*):\s*(.*)$", m.group(1), re.M))
        name, desc = fm.get("name", ""), fm.get("description", "")
        if not SKILL_NAME.fullmatch(name):
            problems.append(f"{d.name}: name {name!r} is not lowercase-with-hyphens")
        elif name != d.name:
            problems.append(f"{d.name}: name {name!r} does not match the directory")
        if not desc:
            problems.append(f"{d.name}: no description — agents match on it to decide whether "
                            f"to load the skill at all")
        elif len(desc) > 1024:
            problems.append(f"{d.name}: description is {len(desc)} chars (max 1024)")
        extra = set(fm) - PORTABLE_FIELDS
        if extra:
            problems.append(f"{d.name}: {sorted(extra)} are not read by every harness "
                            f"(OpenCode takes only {sorted(PORTABLE_FIELDS)})")
    return problems


def kit_version() -> dict:
    """Identify the kit so a later run can tell whether it moved.

    Two homes, one answer: in a checkout, Cargo.toml + git metadata; in an
    installed release kit (BL-15), the KIT_VERSION.json the release workflow
    wrote — a tarball has no git history, and the stamp is what stands in
    for it.
    """
    stamp = REPO / "KIT_VERSION.json"
    if stamp.exists():
        try:
            return json.loads(stamp.read_text())
        except json.JSONDecodeError:
            pass  # fall through and report what can still be known

    def git(*args: str) -> str | None:
        try:
            out = subprocess.run(
                ["git", "-C", str(REPO), *args],
                capture_output=True, text=True, timeout=10,
            )
            return out.stdout.strip() or None if out.returncode == 0 else None
        except Exception:
            return None

    version = None
    cargo = REPO / "Cargo.toml"
    if cargo.exists():
        for line in cargo.read_text().splitlines():
            if line.startswith("version ="):
                version = line.split('"')[1]
                break
    return {
        "reflow2_version": version,
        "commit": git("rev-parse", "--short", "HEAD"),
        "committed_at": git("log", "-1", "--format=%cI"),
        "source": str(REPO),
    }


REMOTE = "https://github.com/sligara7/reflow2.git"


def upstream_head() -> str | None:
    """The newest commit on the remote's default branch, or None.

    `git ls-remote` needs no clone and no fetch. Returns None on any failure —
    offline, no access, no git, slow network — because "I could not check" must
    never look like "you are up to date", and must never block an install.
    """
    try:
        out = subprocess.run(
            ["git", "ls-remote", REMOTE, "HEAD"],
            capture_output=True, text=True, timeout=15,
        )
        if out.returncode != 0 or not out.stdout.strip():
            return None
        return out.stdout.split()[0][:7]
    except Exception:
        return None


def staleness(local_commit: str | None) -> str:
    """One line on whether this kit is behind the remote.

    Deliberately not a nag and deliberately not automatic on every server start:
    a network call per session would be intrusive and would hang offline. It runs
    when someone deliberately asks — which is what this script is.
    """
    installed = kit_version().get("source") == "release-tarball"
    if local_commit is None:
        return "checkout: unknown (no git metadata here)"
    head = upstream_head()
    if head is None:
        return "upstream: could not check (offline, or no access to the repo)"
    if head.startswith(local_commit) or local_commit.startswith(head):
        return f"upstream: current ({local_commit})"
    if installed:
        # An installed kit has no checkout to pull; the update is the installer,
        # which replaces the binary and the kit together — the skew BL-32/BL-18
        # exist to catch cannot open between them.
        return (
            f"upstream: BEHIND — this kit is at {local_commit}, the repo is at {head}.\n"
            f"  A release may not exist for every commit; to update to the newest release,\n"
            f"  re-run the installer (it replaces the binary and the kit together, and\n"
            f"  never touches your design graphs)."
        )
    return (
        f"upstream: BEHIND — this checkout is at {local_commit}, the remote is at {head}.\n"
        f"  Update in this order, or your project gets current instructions on an old server:\n"
        f"    1. git -C {REPO} pull --rebase\n"
        f"    2. cargo build -p reflow2-mcp --release      # rebuild before re-running this\n"
        f"    3. python3 {REPO}/tools/reflow2_init.py <your project>"
    )


def find_binary(override: str | None = None) -> Path | None:
    """The reflow2-mcp binary: an explicit --binary wins, then a checkout's
    target/ dirs, then PATH — the installed-kit case, where there is no
    checkout at all (BL-15)."""
    if override:
        p = Path(override).expanduser().resolve()
        return p if p.exists() else None
    for build in ("release", "debug"):
        p = REPO / "target" / build / "reflow2-mcp"
        if p.exists():
            return p
    if which := shutil.which("reflow2-mcp"):
        return Path(which)
    return None


def binary_is_stale(binary: Path) -> str | None:
    """Is the built binary older than the source it was built from?

    The quiet failure this catches: pull reflow2, re-run this script, forget to
    rebuild. You end up with current instructions driving an old server — and
    the mismatch is invisible until a tool behaves differently than the skills
    say it will. (The array-shape fix is exactly that: same tool name, different
    response.)
    """
    newest = 0.0
    for root in (REPO / "crates", REPO / "schema"):
        if not root.exists():
            continue
        for f in root.rglob("*"):
            if f.is_file() and f.suffix in (".rs", ".yaml", ".toml"):
                newest = max(newest, f.stat().st_mtime)
    if newest > binary.stat().st_mtime:
        return (
            f"the binary at {binary} is older than the source it was built from.\n"
            f"  Rebuild before using it:  cargo build -p reflow2-mcp --release\n"
            f"  Otherwise your project has current instructions driving an old server."
        )
    return None


# Each harness names the server map differently and shapes the entry
# differently, but they all describe the same stdio process. One generator,
# several files — a project opened in a different tool should just work.
#
#   .mcp.json       Claude Code (Grok CLI also loads it as a compatibility
#                   source, so this one file covers both)
#   opencode.json   OpenCode — no .mcp.json compatibility
#   .vscode/mcp.json  Copilot / VS Code — likewise
#
# `extract` pulls the binary path back out of an existing entry so a customised
# config can be recognised and left alone.
#
# EVERY generated config passes `--shared`, and that default is the whole point
# of the flag existing. Without it each session spawns its own process against
# the same directory, the store's single-writer lock admits exactly one, and
# every other session gets the degraded surface — so a second concurrent session
# is broken *by the configuration we ship*, not by anything the user did.
#
# That is not hypothetical. A StoryFlow fleet of three sessions plus a worker
# pool ran for five days on this default and concluded the design graph was
# single-holder by nature: they built a HOLD/RELEASE convention around it, voted
# on giving each session its own graph, and read the design through best-effort
# store copies. `--http` — which would have solved it — had shipped in the very
# binary they were running. A capability you have to reconfigure your way into
# is one most users never reach, so sharing is now what you get by default and
# working alone is the special case (it costs nothing: one session simply starts
# one server and is its only client).
MCP_CONFIGS = [
    {
        "path": ".mcp.json",
        "key": "mcpServers",
        "entry": lambda b, g: {
            "command": str(b),
            "args": ["--graph-path", str(g), "--shared"],
        },
        "extract": lambda e: e.get("command"),
        "extra": {},
    },
    {
        "path": "opencode.json",
        "key": "mcp",
        "entry": lambda b, g: {
            "type": "local",
            "command": [str(b), "--graph-path", str(g), "--shared"],
            "enabled": True,
        },
        # OpenCode takes command+args as one array; the binary is its head.
        "extract": lambda e: (e.get("command") or [None])[0],
        "extra": {"$schema": "https://opencode.ai/config.json"},
    },
    {
        "path": ".vscode/mcp.json",
        "key": "servers",
        "entry": lambda b, g: {
            "command": str(b),
            "args": ["--graph-path", str(g), "--shared"],
        },
        "extract": lambda e: e.get("command"),
        "extra": {},
    },
]


def write_mcp_config(project: Path, spec: dict, binary: Path, force: bool) -> str:
    """Add or refresh reflow2's server entry, disturbing nothing else.

    Merged rather than written whole, for two reasons. `opencode.json` is that
    tool's *entire* config — theme, model, permissions — so overwriting it
    would throw away settings that have nothing to do with us. And a project
    may already run other MCP servers; they must survive.

    Merging also fixes a silent failure: the previous version bailed out
    whenever the file existed without a `reflow2` entry, so any project that
    already used one MCP server never got reflow2 installed at all, while the
    run still reported success.
    """
    path = project / spec["path"]
    # RELATIVE, deliberately (2026-07-25). The binary path has to be absolute —
    # there is no PATH to rely on — but the graph path does not, and an absolute
    # one is a footgun for the case people actually hit: several sessions on one
    # machine. Copy an absolute config into a second git worktree and both
    # sessions open the SAME store, so the second one loses the single-writer
    # lock and gets the degraded server; with `./.reflow2/graph` each worktree
    # opens its own. reflow2's own config has used the relative form all along.
    #
    # The assumption it rests on, stated because it is the failure to look for:
    # the harness launches the server with the project as its working directory.
    # Every harness in MCP_CONFIGS does. If one did not, the session would open
    # an empty graph elsewhere rather than fail — which is why the installer
    # prints the resolved path it expects.
    graph = Path(".") / ".reflow2" / "graph"
    entry = spec["entry"](binary, graph)
    label = spec["path"]

    existing: dict = {}
    if path.exists():
        try:
            existing = json.loads(path.read_text())
        except json.JSONDecodeError:
            return f"{label} is not valid JSON — left alone, fix it by hand"
        if not isinstance(existing, dict):
            return f"{label} is not a JSON object — left alone, fix it by hand"

        servers_val = existing.get(spec["key"], {})
        if not isinstance(servers_val, dict):
            return (f"{label} has a non-object {spec['key']!r} value — "
                    f"left alone, fix it by hand")
        current = servers_val.get("reflow2")
        if isinstance(current, dict) and not force:
            pointed_at = spec["extract"](current)
            if pointed_at and pointed_at != str(binary):
                return (
                    f"{label} LEFT ALONE — its reflow2 entry points at {pointed_at}, "
                    f"not {binary} (re-run with --force-mcp to repoint it)"
                )
            if current == entry:
                return f"{label} unchanged"

    merged = dict(existing)
    for k, v in spec["extra"].items():
        merged.setdefault(k, v)
    servers = dict(merged.get(spec["key"], {}))
    servers["reflow2"] = entry
    merged[spec["key"]] = servers

    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(merged, indent=2) + "\n")
    kept = [n for n in servers if n != "reflow2"]
    if kept:
        return f"{label} (reflow2 added; kept {', '.join(sorted(kept))})"
    return label


POINTER_LINE = (
    "> **reflow2 is installed here.** The design graph is this project's memory — read "
    "[{side}]({side}) and consult it before writing or changing code."
)


# The conventions a harness reads FIRST and will not find any other way. If a
# project owns none of INSTRUCTION_FILES, a pointer is CREATED here rather than
# only appended to what already exists (req:kit-reaches-the-agent).
#
# Found in use on 2026-07-28: dynograph-foundation had neither AGENTS.md nor
# CLAUDE.md, so the install wrote a fresh AGENTS.md, reported success, and left
# the kit invisible — Claude Code reads CLAUDE.md first and never opened it. That
# is the SAME defect class this file already documents from storyflow, in the
# opposite direction: the earlier fix protected an EXISTING CLAUDE.md and did not
# cover a project with no instruction file at all, which is the ordinary state of
# a repo that has never been agent-worked, and therefore the ordinary state of an
# adopt target.
#
# So the rule is not "protect what exists" but REACH WHAT READS. Kept to
# CLAUDE.md deliberately rather than creating every convention: writing
# GEMINI.md, .cursorrules and the rest into a repo that asked for none of them is
# spam, and the sidecar we create (AGENTS.md) already IS the agents.md
# convention that the others increasingly follow.
CREATE_IF_NO_INSTRUCTION_FILE = ["CLAUDE.md"]


def pointer_targets(project: Path, reflow2_doc: str) -> list[Path]:
    """The project's own instruction files that should point at reflow2's.

    Every convention in [`INSTRUCTION_FILES`] that exists, except the file
    reflow2's own instructions live in — a file must not point at itself.

    When the project owns NONE of them, the primary-harness conventions in
    [`CREATE_IF_NO_INSTRUCTION_FILE`] are returned even though they do not exist
    yet, so [`ensure_pointer`] creates them. An install that writes a file no
    harness opens has not installed anything, however loudly it reports success.
    """
    existing = [
        project / rel
        for rel in INSTRUCTION_FILES
        if rel != reflow2_doc and (project / rel).exists()
    ]
    if existing:
        return existing
    return [
        project / rel for rel in CREATE_IF_NO_INSTRUCTION_FILE if rel != reflow2_doc
    ]


def ensure_pointer(instruction_file: Path, reflow2_doc: str) -> str | None:
    """Point an instruction file at reflow2's, creating it only if the project
    has no instruction file at all.

    Appends one marked line to a file the project owns. When the file does not
    exist — which [`pointer_targets`] only allows for the primary-harness
    conventions, and only when the project owns no instruction file whatsoever —
    it is created carrying that same line and nothing else. Deliberately minimal:
    a file reflow2 invents in someone's repo should say where to look and get out
    of the way, not become a second place project instructions live.
    """
    line = POINTER_LINE.format(side=reflow2_doc)
    if not instruction_file.exists():
        instruction_file.parent.mkdir(parents=True, exist_ok=True)
        instruction_file.write_text(
            f"# {instruction_file.name}\n\n"
            f"**Read [{reflow2_doc}]({reflow2_doc}).** It is the primary instruction file for "
            f"this repo and follows the [agents.md](https://agents.md) convention, so every "
            f"agent working here reads the same thing.\n\n"
            f"This file exists only because some harnesses read `{instruction_file.name}` "
            f"first. Keeping the content in one place rather than duplicating it is "
            f"deliberate: a rule only one collaborator's tool ever sees is worse than no rule "
            f"at all.\n\n" + line + "\n"
        )
        return (
            f"{instruction_file.name}  (CREATED — this project had no instruction file, and "
            f"{reflow2_doc} alone is not read first by every harness; without this the install "
            f"succeeds and the kit stays invisible)"
        )
    text = instruction_file.read_text()
    if reflow2_doc in text:
        return None
    instruction_file.write_text(text.rstrip("\n") + "\n\n" + line + "\n")
    return (
        f"{instruction_file.name}  (appended one marked line pointing at "
        f"{reflow2_doc} — without it an agent reading this file never learns "
        f"reflow2 exists)"
    )


# What counts as evidence that this project is a system that already exists,
# as opposed to an empty directory about to become one. Used to decide which
# next steps to print — genesis, or adopt.
#
# Keyed on the project rather than on our own install artifacts: the earlier
# version branched on "did we write a sidecar?", so storyflow — 2,643 source
# files, months of history — was told to describe what it wanted to build,
# and the adopt skill was never mentioned (F2, same trial).
SOURCE_SUFFIXES = {
    ".py", ".rs", ".ts", ".tsx", ".js", ".jsx", ".go", ".java", ".rb", ".c",
    ".cc", ".cpp", ".h", ".hpp", ".cs", ".swift", ".kt", ".php", ".scala",
    ".ex", ".exs", ".sh", ".sql", ".svelte", ".vue",
}
SKIP_DIRS = {
    ".git", ".reflow2", "node_modules", "target", "build", "dist", "venv",
    ".venv", "__pycache__", ".claude", ".grok", ".vscode", ".github",
}


def existing_system(project: Path, cap: int = 25) -> str | None:
    """Why this looks like a system that already exists, or None.

    Counts source files, stopping at `cap` — the question is "is there
    substantial code here?", not "how much", and a full walk of a large repo
    is a slow way to answer a yes/no.
    """
    found = 0
    for path in project.rglob("*"):
        if any(part in SKIP_DIRS for part in path.parts):
            continue
        if path.is_file() and path.suffix in SOURCE_SUFFIXES:
            found += 1
            if found >= cap:
                break
    if found >= cap:
        return f"{cap}+ source files"
    if found:
        return f"{found} source file(s)"
    return None


# What must never be committed, and why. Both are MACHINE state: the graph is
# RocksDB files and a lock, and every MCP config carries an absolute path to
# *this* machine's binary. reflow2's own repo has ignored both from the start;
# a consumer project did not, which is the inconsistency this closes.
#
# The MCP configs matter more than they look. Committed, they reach a
# collaborator pointing at a binary that does not exist on their machine — and
# the installer will correctly REFUSE to repoint an entry somebody may have
# customised, so they get a loud line they have to notice and act on. Ignored,
# each person's `reflow2_init.py` run just writes their own.
IGNORE_LINES = [
    ("# reflow2's local design graph — machine state, NOT the design.\n"
     "# The shareable record is an EXPORT, committed at docs/design/<project>.json.\n"
     "# Ask your agent to `export_graph` there; that file is what teammates read and\n"
     "# what the CI design gate checks. Do not ignore it.",
     ".reflow2/"),
    ("# reflow2's MCP configs — they carry an absolute path to YOUR binary",
     ".mcp.json"),
    ("", "opencode.json"),
    ("", ".vscode/mcp.json"),
    ("# reflow2's loop-nudge hook — an absolute path to YOUR kit",
     ".claude/settings.local.json"),
]


def gitignore_block() -> str:
    out = []
    for comment, line in IGNORE_LINES:
        if comment:
            out.append(comment)
        out.append(line)
    return "\n".join(out) + "\n"


def already_tracked(project: Path, rel: str) -> bool:
    """Is this path committed already? .gitignore does not untrack a tracked
    file, so saying "ignored" without saying that would be a half-truth.

    Works for a DIRECTORY entry (`.reflow2/`) as well as a file: git reports a
    directory as tracked when anything inside it is. Verified 2026-08-08 in a
    scratch repo — commit two files under `.reflow2/`, add the ignore rule
    afterwards, and `git ls-files --error-unmatch '.reflow2/'` exits 0. That
    matters because the two callers below USED TO SKIP every entry ending in
    `/`, on the assumption this could not answer for a directory. It can."""
    try:
        return subprocess.run(
            ["git", "ls-files", "--error-unmatch", rel],
            cwd=project, capture_output=True, timeout=10,
        ).returncode == 0
    except (OSError, subprocess.SubprocessError):
        return False


def untrack_hint(rel: str) -> str:
    """The command that actually untracks `rel` — `-r` for a directory, because
    `git rm --cached .reflow2/` without it fails and an unusable remedy is the
    same as no remedy."""
    return f"git rm -r --cached {rel.rstrip('/')}" if rel.endswith("/") else f"git rm --cached {rel}"


# The coherence loop's out-of-band trigger, wired to the harness's own events.
#
# WHY IT GOES IN settings.LOCAL.json: the command carries an absolute path to
# THIS machine's kit, exactly like the MCP configs, and `.claude/settings.json`
# is a file teams commit and share. A collaborator inheriting your path gets a
# hook that fails silently — the "broken" state reflow2 now reports and the
# worst of the three, because the settings file looks right. The local file is
# per-machine by convention, so each person's own install writes their own.
#
# WHY IT POINTS AT THE KIT rather than a copy in the project: the script then
# updates with the reflow2 package and nothing in the consumer's repository can
# go stale (req:thin-install). `KIT` is wherever this installer is running from,
# so the nudge always comes from the same install as the installer.
HOOK_EVENTS = [
    ("SessionStart", None),
    ("PostToolUse", "mcp__reflow2__.*"),
    ("PostToolUse", "Edit|Write|MultiEdit|NotebookEdit"),
    ("Stop", None),
]


def hook_command() -> str:
    return f'python3 "{REPO / "tools" / "loop_nudge.py"}"'


def detected_indent(text: str, default: str = "  ") -> str | int:
    """The indent this file already uses, so re-serialising it does not
    reformat every line somebody else wrote.

    Returns the whitespace ITSELF rather than a width, because `json.dumps`
    accepts either and a tab-indented file must come back tab-indented. The
    width-only first version silently normalised tabs to two spaces — caught by
    this function's own test rather than by a user, which is the order this
    project prefers.

    REPORTED BY ALEX, 2026-08-13, as *"the init should not write over the
    .claude/settings.local.json if it already exists"*. It never did overwrite —
    `ensure_hooks` merges, and a hook you repointed is left alone — but it
    re-serialised the whole document with a fixed 2-space indent, so a compact
    10-line settings file came back as 58 expanded lines and EVERY LINE SHOWED
    IN THE DIFF. **A merge you cannot distinguish from an overwrite is an
    overwrite as far as trust goes**, which is the standard `ensure_hooks`' own
    docstring already sets for itself.

    This does not make the diff minimal — `json.dumps` still normalises array
    layout, and format-preserving JSON editing needs a different parser than the
    stdlib has. It removes the largest and most alarming part of the churn, and
    the summary line now says outright that nothing was removed.
    """
    for line in text.splitlines():
        stripped = line.lstrip(" \t")
        if stripped and stripped != line and not stripped.startswith("//"):
            return line[: len(line) - len(stripped)]
    return default


def ensure_hooks(project: Path, force: bool) -> list[str]:
    """Register the loop nudge, disturbing nothing else in the settings file.

    Merged rather than written whole, and a reflow2 hook the user has repointed
    is LEFT ALONE and reported — same rule as the MCP config, and for the same
    reason: a hook is something people customise, and an installer that undoes
    that silently is one nobody trusts near their settings again.
    """
    path = project / ".claude" / "settings.local.json"
    command = hook_command()
    script = REPO / "tools" / "loop_nudge.py"
    if not script.exists():
        return [
            f".claude/settings.local.json  SKIPPED — {script} is not there, so registering a "
            f"hook pointing at it would create the silently-broken state reflow2 warns about"
        ]

    data: dict = {}
    raw = ""
    if path.exists():
        raw = path.read_text()
        try:
            data = json.loads(raw)
        except json.JSONDecodeError:
            return [
                f"{path.name}  SKIPPED (malformed JSON — left exactly as it is; reflow2 will "
                f"tell every session that no nudge is installed)"
            ]
        if not isinstance(data, dict):
            return [f"{path.name}  SKIPPED (not a JSON object — left alone)"]

    hooks = data.setdefault("hooks", {})
    if not isinstance(hooks, dict):
        return [f"{path.name}  SKIPPED (its `hooks` is not an object — left alone)"]

    added, kept = [], []
    for event, matcher in HOOK_EVENTS:
        groups = hooks.setdefault(event, [])
        if not isinstance(groups, list):
            kept.append(f"{event} (not a list — left alone)")
            continue
        # Ours if any hook in a matching group already runs the nudge.
        existing = None
        for group in groups:
            if not isinstance(group, dict) or group.get("matcher") != matcher:
                continue
            for hook in group.get("hooks", []) or []:
                if isinstance(hook, dict) and "loop_nudge" in str(hook.get("command", "")):
                    existing = hook
        if existing is not None:
            if existing.get("command") == command or not force:
                if existing.get("command") != command:
                    kept.append(f"{event} (yours runs {existing['command']})")
                continue
            existing["command"] = command
            added.append(f"{event} (repointed)")
            continue
        entry = {"type": "command", "command": command}
        groups.append({"matcher": matcher, "hooks": [entry]} if matcher else {"hooks": [entry]})
        added.append(event if not matcher else f"{event}[{matcher}]")

    notes = []
    if added:
        path.parent.mkdir(parents=True, exist_ok=True)
        # Keep the indent this file already used (see `detected_indent`): a
        # merge that reformats every line is indistinguishable from a rewrite.
        path.write_text(json.dumps(data, indent=detected_indent(raw)) + "\n")
        existed = " MERGED into your existing file — nothing was removed;" if raw else ""
        notes.append(
            f".claude/settings.local.json {existed} (loop nudge registered: {', '.join(added)} — "
            f"the session-end backstop the coherence loop rests on)"
        )
    if kept:
        notes.append(
            f".claude/settings.local.json  LEFT ALONE for {', '.join(kept)} "
            f"(re-run with --force-hooks to repoint)"
        )
    if not notes:
        notes.append(".claude/settings.local.json unchanged (loop nudge already registered)")
    return notes


def ensure_gitattributes(project: Path) -> str | None:
    """Ask git to merge the design record with reflow2's driver, not by lines.

    The repo-side half of the pair: `.gitattributes` names the file and the
    driver, and travels with the project so every collaborator's clone agrees
    on WHICH files need it. `ensure_merge_driver` supplies the other half, which
    cannot travel.

    Without this a consumer project gets git's line merge on its design export —
    and two people who edited entirely DIFFERENT parts of the design still
    collide, because a graph serialised as one JSON document has no line-level
    independence. reflow2's own repo has carried this rule for months; a project
    installed by this script never got it.
    """
    rel = design_record_path(project).relative_to(project).as_posix()
    line = f"{rel} merge=reflow2"
    attrs = project / ".gitattributes"
    existing = attrs.read_text() if attrs.exists() else ""
    if "merge=reflow2" in existing:
        return None
    block = (
        "\n" if existing and not existing.endswith("\n") else ""
    ) + (
        "\n# The design export is one large JSON document, so two people who edited\n"
        "# DIFFERENT parts of the design still collide here — and git resolves it by\n"
        "# lines, which is the wrong unit for a graph. reflow2's three-way merge\n"
        "# compares per node and per property against the common ancestor, so disjoint\n"
        "# work merges itself and only a real both-sides conflict stops for a human.\n"
        "#\n"
        "# The driver itself is per-clone (git will not let a repo configure an\n"
        "# executable); reflow2_init.py sets it. Without it git falls back to its\n"
        "# normal text merge, which is safe — you just resolve the JSON by hand.\n"
        f"{line}\n"
    )
    attrs.write_text(existing + block)
    verb = "created" if not existing else "added to"
    return (
        f".gitattributes  ({verb} — {rel} merges per NODE, not by lines; "
        f"git's line merge is the wrong unit for a graph)"
    )


def ensure_merge_driver(project: Path, binary: Path) -> str | None:
    """Register reflow2's three-way merge driver in THIS clone.

    `.gitattributes` already says `docs/design/<...>.json merge=reflow2`, and
    that half travels with the repo. **The driver itself cannot**: git
    deliberately refuses to let a repository configure an executable, so it must
    be set per clone — and until now nothing set it. A collaborator who cloned
    and pulled got git's LINE-BASED text merge on a multi-megabyte JSON graph,
    where two people who edited entirely different parts of the design still
    collide. `.gitattributes`' own comment calls that "safe — it just means you
    resolve the JSON by hand", which is true and is not a thing anyone will do.

    ANTHONY, 2026-08-13, option B of
    `dec:idea-feedback-arrives-by-git-push-and-pull`: he asked whether his
    brother could clone, write feedback into the graph, push, and have it arrive
    on the next pull. That is the design's intended path — and this was one of
    three steps in it that fail silently.

    WRITTEN WITH `--local`, so it lands in this clone's own config and never
    touches the user's global git. NEVER OVERWRITES a driver somebody already
    set: a merge driver is exactly the kind of thing a person customises, and an
    installer that silently repoints one is the same failure `write_mcp_config`
    and `ensure_hooks` already refuse.
    """
    if not (project / ".git").exists():
        return None  # not a git checkout; nothing to configure and nothing to say
    attrs = project / ".gitattributes"
    if not attrs.exists() or "merge=reflow2" not in attrs.read_text():
        # The repo does not ask for the driver, so configuring one would be
        # answering a question nobody asked.
        return None

    def git_config(*args: str) -> subprocess.CompletedProcess:
        return subprocess.run(
            ["git", "-C", str(project), "config", "--local", *args],
            capture_output=True, text=True,
        )

    existing = git_config("--get", "merge.reflow2.driver")
    driver = f'"{binary}" --merge-driver %O %A %B'
    if existing.returncode == 0 and existing.stdout.strip():
        if existing.stdout.strip() == driver:
            return None  # already ours and already current
        return (
            "merge.reflow2.driver  LEFT ALONE — yours runs "
            f"{existing.stdout.strip()} (a merge driver is something people "
            "customise; change it yourself if that is stale)"
        )
    name = git_config("merge.reflow2.name", "reflow2 design export merge")
    drv = git_config("merge.reflow2.driver", driver)
    if name.returncode != 0 or drv.returncode != 0:
        err = (drv.stderr or name.stderr or "").strip().splitlines()
        return (
            "merge.reflow2.driver  NOT set — "
            f"{err[0] if err else 'git config failed'}. Without it git text-merges "
            "the design export by LINES, which is the wrong unit for a graph."
        )
    return (
        "merge.reflow2.driver  (registered — the design export now merges per "
        "NODE against the common ancestor, so disjoint work merges itself; git "
        "cannot carry this in the repo, so each clone sets it once)"
    )


def design_record_path(project: Path) -> Path:
    """Where the SHAREABLE design record goes — the export teammates read.

    `docs/design/<project>.json`, matching what reflow2 does for itself and what
    `reflow2_check.py --export` expects. A convention rather than a setting: the
    point of naming it here is that a new project no longer has to invent one,
    and every message that mentions the record can name the same path.
    """
    return project / "docs" / "design" / f"{project.name}.json"


def ensure_design_record(project: Path, binary: Path) -> str | None:
    """Write the shareable export — ALWAYS, even on a brand-new project.

    ALEX ASKED FOR EXACTLY THIS, 2026-08-13: *"it should just make the file and
    it should not be .gitignored. For some reason I thought it already created
    that file."* Two users independently expected the file to exist, which is
    the strongest evidence there is about where somebody looks.

    ⚠️ THE FIRST VERSION OF THIS ONLY WROTE THE FILE WHEN A GRAPH ALREADY
    EXISTED, and that was wrong — it is silent on a FRESH project, which is the
    only case Alex hit. The reasoning was that exporting an empty graph opens a
    store and mints an identity nobody asked for, the thing `describe_designs`
    refuses to do. **That analogy does not hold here.** `describe_designs`
    INSPECTS, and looking must not create; this is the installer, and the moment
    somebody runs it is exactly the moment the project adopts reflow2. It
    already writes `.reflow2/`, the MCP configs and an instruction file — minting
    the design id is the smallest of those commitments, and `design_identity`
    says the id is assigned once and never changes, so doing it here is doing it
    at the only unambiguous point.

    An empty export is a real document: stamp, `graph_id`, `content_hash`, and
    zero nodes. Committing it means the first real export CHAINS FROM IT rather
    than starting a lineage nobody can check.

    Never overwrites: a record already there may be one they committed.
    """
    dest = design_record_path(project)
    if dest.exists():
        return None  # theirs now — never overwrite a record they may have committed
    graph = project / ".reflow2" / "graph"
    rel = dest.relative_to(project)
    manually = (
        f"{rel}  NOT created — ask your agent to `export_graph` to that path. "
        f"It is the record your teammates read and what the CI design gate checks."
    )
    # NEVER FATAL, and never an exception. This is the first tool a new user
    # meets; a missing or unrunnable binary must produce a line they can act on,
    # not a traceback over an installer that had otherwise succeeded. It also
    # keeps `install()` callable without spawning anything, which is what the
    # installer's own test suite relies on.
    if binary is None or not Path(binary).exists():
        return manually
    try:
        out = subprocess.run(
            [str(binary), "--graph-path", str(graph), "--export"],
            capture_output=True, text=True, timeout=120,
        )
        if out.returncode != 0:
            out = subprocess.run(
                [str(binary), "--graph-path", str(graph), "--export-snapshot"],
                capture_output=True, text=True, timeout=120,
            )
    except (OSError, subprocess.SubprocessError):
        return manually
    if out.returncode != 0:
        return manually
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_text(out.stdout)
    return (
        f"{dest.relative_to(project)}  (CREATED — the shareable design record. "
        f".reflow2/ is machine state and ignored; THIS is the file to commit)"
    )


def ensure_gitignore(project: Path) -> list[str]:
    """Keep machine state out of version control — the graph directory and the
    MCP configs. Appends or creates, idempotent, reported. Also reports any of
    them git is *already* tracking, because ignoring a tracked file does
    nothing at all until it is untracked."""
    notes = []
    gi = project / ".gitignore"
    existing = gi.read_text() if gi.exists() else ""
    missing = [line for _, line in IGNORE_LINES if line not in existing]
    if missing:
        if existing:
            gi.write_text(existing.rstrip("\n") + "\n\n" + gitignore_block())
            notes.append(f".gitignore  (added {', '.join(missing)} — machine state)")
        else:
            gi.write_text(gitignore_block())
            notes.append(".gitignore  (created — ignoring the graph and the MCP configs)")
    # NO `endswith("/")` SKIP. It used to be here, and `.reflow2/` is the only
    # directory entry in IGNORE_LINES — so the one path most likely to be
    # already tracked was the one path this warning could never fire for. It is
    # the most likely because the graph store is created the moment you first
    # USE reflow2, long before anyone runs the installer that ignores it: a
    # `git add .` in that window tracks it permanently, and the ignore line
    # added afterwards does nothing at all.
    #
    # Reported 2026-08-08 by Alex, on a project installed at 0.11.0: his
    # .gitignore contained `.reflow2` and git was still tracking
    # `.reflow2/graph/LOG` and `.reflow2/graph/fulltext/*.json`. He read his
    # .gitignore, drew the correct conclusion from it, and the installer stayed
    # quiet — req:no-silent-fallback, in the first tool a new user meets.
    for _, line in IGNORE_LINES:
        if already_tracked(project, line):
            what = "carries this machine's absolute paths"
            if line.endswith("/"):
                what = "is machine state — a RocksDB store and a search index"
            notes.append(
                f"{line}  IS COMMITTED and {what} — "
                f"ignoring it changes nothing until you untrack it:  {untrack_hint(line)}"
            )
    return notes


def planned_changes(project: Path) -> list[str]:
    """What a run would create or overwrite, without touching anything."""
    changes = []
    for src, rel in FILES:
        dst = project / rel
        if owner := foreign_owner(src, dst):
            side = project / SIDECAR.get(rel, f"REFLOW2_{rel}")
            if not side.exists() or not filecmp.cmp(src, side, shallow=False):
                verb = "create" if not side.exists() else "update"
                changes.append(f"{verb}  {side.name}  (keeping your own {rel} — {owner})")
        elif not dst.exists():
            changes.append(f"create  {rel}")
        elif not filecmp.cmp(src, dst, shallow=False):
            changes.append(f"update  {rel}")
    for src, rel in TREES:
        for path in sorted(src.rglob("*")):
            if path.is_dir():
                continue
            dst = project / rel / path.relative_to(src)
            label = str(Path(rel) / path.relative_to(src))
            if not dst.exists():
                changes.append(f"create  {label}")
            elif not filecmp.cmp(path, dst, shallow=False):
                changes.append(f"update  {label}")
    # Files a previous kit installed that this one no longer ships — the
    # thirty-odd skill copies, on the first update after dec:skills-served.
    # --check that stayed silent about thirty deletions would be exactly the
    # silent change this project forbids, and it is the run people trust
    # before they let it touch a repo they care about.
    shipped = {rel for _, rel in FILES}
    shipped |= {SIDECAR.get(rel, f"REFLOW2_{rel}") for _, rel in FILES}
    for src, rel in TREES:
        shipped |= {str(Path(rel) / p.relative_to(src))
                    for p in src.rglob("*") if p.is_file()}
    for rel, recorded in sorted(installed_manifest(project).items()):
        if rel in shipped or not (project / rel).exists():
            continue
        if file_sha(project / rel) == recorded:
            changes.append(f"remove  {rel}  ({why_gone(rel)})")
        else:
            changes.append(f"keep    {rel}  (your edits — not removed)")
    for spec in MCP_CONFIGS:
        path = project / spec["path"]
        if not path.exists():
            changes.append(f"create  {spec['path']}")
        else:
            try:
                obj = json.loads(path.read_text())
                servers_val = obj.get(spec["key"], {}) if isinstance(obj, dict) else None
            except json.JSONDecodeError:
                obj, servers_val = None, None
            if obj is None or not isinstance(servers_val, dict):
                changes.append(f"skip    {spec['path']} (malformed — the run "
                               f"will leave it alone)")
            elif servers_val.get("reflow2") is None:
                changes.append(f"update  {spec['path']} (add the reflow2 server)")
    reflow2_doc = "REFLOW2.md" if (project / "REFLOW2.md").exists() or any(
        foreign_owner(src, project / rel) for src, rel in FILES if rel == "AGENTS.md"
    ) else "AGENTS.md"
    for target in pointer_targets(project, reflow2_doc):
        # The target may not exist yet: `pointer_targets` deliberately returns
        # the primary-harness conventions when the project owns NO instruction
        # file, because that is the case `ensure_pointer` has to create rather
        # than skip. This branch is the CHECK side of the same fact and read the
        # file unguarded, so `--check` crashed on the very case the create path
        # exists for — an empty project directory, which is the first thing a
        # new user points this at.
        if not target.exists():
            changes.append(
                f"create  {target.name} (this project has no instruction file; "
                f"without it the install stays invisible to the primary harness)"
            )
        elif reflow2_doc not in target.read_text():
            changes.append(
                f"append  one marked pointer line to your {target.name} (→ {reflow2_doc})"
            )
    if not (project / ".reflow2").exists():
        changes.append("create  .reflow2/")
    gi = project / ".gitignore"
    existing = gi.read_text() if gi.exists() else ""
    missing = [line for _, line in IGNORE_LINES if line not in existing]
    if missing:
        verb = "create" if not existing else "append"
        changes.append(f"{verb}  .gitignore  ({', '.join(missing)} — machine state)")
    # Same missing case as ensure_gitignore, and worse here: `--check` is what
    # you run to find out what is wrong BEFORE touching anything, so a silent
    # --check is the one that sends you away reassured.
    for _, line in IGNORE_LINES:
        if already_tracked(project, line):
            changes.append(f"report  {line} is committed — you will need `{untrack_hint(line)}`")
    return changes


def backup_graph(project: Path, binary: Path) -> str | None:
    """Export the design before changing anything around it.

    An update replaces the instructions and can precede a rebuilt binary with a
    different schema. The graph itself is not touched by this script, but the
    cheapest insurance against the *next* step going wrong is a copy taken
    before this one — and the export is deterministic, so a backup directory
    under version control shows what changed in the design rather than a fresh
    blob each time.

    Kept beside the graph, not in /tmp: systemd-tmpfiles clears that, which
    would quietly throw away the thing being kept.

    FALLS BACK TO `--export-snapshot` WHEN THE GRAPH IS HELD, and since sharing
    became the default that is the ORDINARY case, not an edge one: a plain
    `--export` opens the store, the shared server already holds the single-writer
    lock, and the backup was skipped on every update of a project anyone had open
    — which is exactly when a backup is worth having. Found by running it on
    brainmaker, 2026-07-28: "backup SKIPPED — could not export the graph". The
    snapshot is best-effort and not crash-consistent, which is the right trade
    for insurance taken before a step that is not going to touch the graph
    anyway; a failure of BOTH is still only reported, never fatal.
    """
    graph = project / ".reflow2" / "graph"
    if not graph.exists():
        return None  # nothing designed yet
    out = subprocess.run(
        [str(binary), "--graph-path", str(graph), "--export"],
        capture_output=True, text=True, timeout=120,
    )
    if out.returncode != 0:
        out = subprocess.run(
            [str(binary), "--graph-path", str(graph), "--export-snapshot"],
            capture_output=True, text=True, timeout=120,
        )
    if out.returncode != 0:
        # Report rather than abort: a failed backup should not block an update
        # that might be exactly what fixes the binary that could not read it.
        first = (out.stderr or "").strip().splitlines()
        return f"backup SKIPPED — could not export the graph: {first[0] if first else 'unknown error'}"
    stamp = datetime.datetime.now(datetime.timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    dest = project / ".reflow2" / "backups" / f"design-{stamp}.json"
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_text(out.stdout)
    return f"backed the design up to {dest.relative_to(project)}"



def file_sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def is_skill_path(rel: str) -> bool:
    """A path a previous kit installed as a harness-loaded skill."""
    return rel.startswith((".claude/skills/", ".grok/skills/"))


def why_gone(rel: str) -> str:
    """Why a file the kit used to ship is being removed.

    "No longer shipped" is true and useless for the thirty-odd skill files a
    first update after `dec:skills-served` deletes: someone watching their repo
    empty out is owed the reason, not the mechanism.
    """
    return ("now served by the MCP server — call list_skills / get_skill"
            if is_skill_path(rel) else "no longer shipped by the kit")


def installed_manifest(project: Path) -> dict:
    """What the last install wrote, rel path -> sha256.

    Empty for pre-manifest installs (or an unreadable stamp) — those fall back
    to the older heading-based ownership check and gain a manifest on this
    write, so the clobber window closes after one update (BL-54).
    """
    try:
        data = json.loads((project / STAMP).read_text())
    except (OSError, json.JSONDecodeError):
        return {}
    files = data.get("installed_files") if isinstance(data, dict) else None
    return files if isinstance(files, dict) else {}


def place_kit_file(src: Path, dst: Path, rel: str, old_manifest: dict,
                   new_manifest: dict, done: list, label: str | None = None) -> bool:
    """Write one kit-owned file, refusing to clobber local edits (BL-54).

    Ownership is proven by the manifest: the file is ours to refresh only when
    its current content matches what the kit last installed. A mismatch means
    the user edited it — the edit is kept and reported, never overwritten.
    """
    recorded = old_manifest.get(rel)
    if dst.exists() and recorded is not None and file_sha(dst) != recorded:
        new_manifest[rel] = recorded  # still on the books; keep tracking it
        done.append(f"{rel}  LEFT ALONE — differs from what the kit installed "
                    f"(your edits); delete the file to accept the kit copy")
        return False
    changed = not dst.exists() or not filecmp.cmp(src, dst, shallow=False)
    shutil.copy2(src, dst)
    new_manifest[rel] = file_sha(dst)
    if changed:
        done.append(label or rel)
    return True


def install(project: Path, binary: Path, force_mcp: bool) -> list[str]:
    if problems := check_skills():
        raise SystemExit(
            "refusing to install: these skills would be silently ignored by the agent\n  "
            + "\n  ".join(problems)
        )
    done = []
    if note := backup_graph(project, binary):
        done.append(note)
    # Where reflow2's own instructions end up: AGENTS.md normally, REFLOW2.md
    # when the project already owns that filename.
    reflow2_doc = "AGENTS.md"
    old_manifest = installed_manifest(project)
    new_manifest: dict = {}
    for src, rel in FILES:
        dst = project / rel
        dst.parent.mkdir(parents=True, exist_ok=True)
        owner = foreign_owner(src, dst)
        recorded = old_manifest.get(rel)
        if not owner and dst.exists() and recorded is not None and file_sha(dst) != recorded:
            owner = "it differs from what the kit installed (your local edits)"
        if owner:
            # The project (or the user's edits) own this path. Overwriting it
            # destroys the instructions the project actually runs on — and
            # AGENTS.md is exactly the file every brownfield target has.
            side_rel = SIDECAR.get(rel, f"REFLOW2_{rel}")
            side = project / side_rel
            # The sidecar obeys the same rule: a project may already own a
            # file at the sidecar path too (BL-54).
            if side.exists() and foreign_owner(src, side) \
                    and old_manifest.get(side_rel) is None:
                done.append(f"{side_rel}  LEFT ALONE — the project has its own "
                            f"file here too; the kit's {rel} was not installed")
            else:
                place_kit_file(src, side, side_rel, old_manifest, new_manifest,
                               done, label=f"{side_rel}  (kept your own {rel} — {owner})")
            reflow2_doc = side.name
            continue
        place_kit_file(src, dst, rel, old_manifest, new_manifest, done)
    for src, rel in TREES:
        for path in sorted(src.rglob("*")):
            if path.is_dir():
                continue
            file_rel = str(Path(rel) / path.relative_to(src))
            dst = project / rel / path.relative_to(src)
            dst.parent.mkdir(parents=True, exist_ok=True)
            place_kit_file(path, dst, file_rel, old_manifest, new_manifest, done)
    # Files a previous kit shipped that this one no longer does are pruned —
    # but only when untouched since we wrote them (BL-54): an edited copy is
    # kept, loudly. Without this, a renamed skill lived on forever downstream.
    for rel, recorded in sorted(old_manifest.items()):
        if rel in new_manifest:
            continue
        stale = project / rel
        if not stale.exists():
            continue
        if file_sha(stale) == recorded:
            stale.unlink()
            done.append(f"{rel}  removed ({why_gone(rel)})")
        else:
            new_manifest[rel] = recorded
            if is_skill_path(rel):
                # Not merely untidy: your harness DOES auto-load a file in a
                # skills directory, and a served skill is never offered — so an
                # edited copy silently wins over every future release of that
                # skill. Keeping it is right; keeping it quietly is not.
                done.append(f"{rel}  kept (your edits) — your harness will keep "
                            f"loading it, and it SHADOWS the served skill of the "
                            f"same name; delete it to follow the served one")
            else:
                done.append(f"{rel}  no longer shipped by the kit, but it has "
                            f"your edits — left in place")

    # MCP config, with the binary path already resolved — the step people
    # previously had to hand-edit, and the one most likely to be got wrong.
    for spec in MCP_CONFIGS:
        done.append(write_mcp_config(project, spec, binary, force_mcp))
    # The loop's session-end backstop. Without it nothing interrupts a session
    # that finishes owing the design a check — and until 2026-07-25 the
    # installer wired none, so every consumer project ran without one.
    done.extend(ensure_hooks(project, force_mcp))

    # A file nobody points at is invisible: an agent reads the project's own
    # instructions and never learns reflow2 exists (BL-22's lesson — shipping
    # the file is not shipping the capability). Every instruction file the
    # project already has gets one marked line, same rule as the merged MCP
    # configs: add and report, never overwrite. Idempotent by content.
    for target in pointer_targets(project, reflow2_doc):
        if pointer := ensure_pointer(target, reflow2_doc):
            done.append(pointer)

    (project / ".reflow2").mkdir(exist_ok=True)
    done.extend(ensure_gitignore(project))
    if record := ensure_design_record(project, binary):
        done.append(record)
    # Order matters: the repo-side rule must exist before the per-clone driver
    # looks for it, or a fresh project configures nothing on its first install
    # and only picks the driver up on a later run.
    if attrs := ensure_gitattributes(project):
        done.append(attrs)
    if driver := ensure_merge_driver(project, binary):
        done.append(driver)
    stamp = project / STAMP
    stamp_data = kit_version()
    stamp_data["installed_files"] = dict(sorted(new_manifest.items()))
    stamp.write_text(json.dumps(stamp_data, indent=2) + "\n")
    return done


def main() -> int:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("project", help="the project directory (created if absent)")
    ap.add_argument("--check", action="store_true",
                    help="report what would change; write nothing")
    ap.add_argument("--force-mcp", action="store_true",
                    help="rewrite .mcp.json even if it was customised")
    ap.add_argument("--binary", metavar="PATH",
                    help="path to the reflow2-mcp binary (default: this "
                         "checkout's target/, then PATH)")
    opts = ap.parse_args()

    project = Path(opts.project).expanduser().resolve()
    if not KIT.exists():
        print(f"error: kit not found at {KIT}", file=sys.stderr)
        return 1

    binary = find_binary(opts.binary)
    if binary is None and not opts.check:
        if opts.binary:
            print(f"error: no binary at {opts.binary}", file=sys.stderr)
        else:
            print(
                "error: reflow2-mcp was not found (not built in this checkout, "
                "not on PATH).\n"
                "  From a checkout:  cargo build -p reflow2-mcp --release\n"
                "  (first build compiles RocksDB — around ten minutes, then cached)\n"
                "  Or install a prebuilt binary with tools/install.sh, then pass "
                "--binary if it is not on PATH.",
                file=sys.stderr,
            )
        return 1

    existing = project / STAMP
    previously = None
    if existing.exists():
        try:
            previously = json.loads(existing.read_text())
        except json.JSONDecodeError:
            pass

    if opts.check:
        if not project.exists():
            print(f"{project} does not exist yet — a run would create it.")
            return 0
        changes = planned_changes(project)
        if previously:
            print(f"installed from reflow2 {previously.get('reflow2_version')} "
                  f"({previously.get('commit')})")
        print(f"now at reflow2 {kit_version()['reflow2_version']} ({kit_version()['commit']})")
        print(staleness(kit_version().get("commit")))
        print()
        if changes:
            print(f"{len(changes)} change(s) a run would make:")
            for c in changes:
                print(f"  {c}")
        else:
            print("kit is up to date.")
        if binary is None:
            print("\nbinary: not built — cargo build -p reflow2-mcp --release")
        elif (stale := binary_is_stale(binary)) is not None:
            print(f"\nbinary: STALE — {stale}")
        else:
            print(f"\nbinary: current ({binary})")
        return 0

    updating = project.exists() and previously is not None
    project.mkdir(parents=True, exist_ok=True)
    done = install(project, binary, opts.force_mcp)

    stale = binary_is_stale(binary)

    verb = "Updated" if updating else "Set up"
    print(f"{verb} reflow2 in {project}\n")
    for d in done:
        print(f"  {d}")
    print()

    if stale:
        print(f"WARNING: {stale}\n")

    if updating:
        print(f"Was: reflow2 {previously.get('reflow2_version')} ({previously.get('commit')})")
        print(f"Now: reflow2 {kit_version()['reflow2_version']} ({kit_version()['commit']})")
        print("\nYour design graph and your own files were not touched.")
        # An update on an existing system whose graph is still empty is
        # someone who installed before `adopt` shipped, or who never started.
        # Saying nothing leaves them exactly where F2 left storyflow.
        graph = project / ".reflow2" / "graph"
        if existing_system(project) and not graph.exists():
            doc = "REFLOW2.md" if (project / "REFLOW2.md").exists() else "AGENTS.md"
            print(
                f"\nThe design graph is still empty. For a system that already exists, "
                f"run the\n  **adopt** skill — it recovers the design from what was built. "
                f"See {doc}."
            )
    else:
        print(f"{staleness(kit_version().get('commit'))}\n")
        # The two starting states want opposite advice: a greenfield project
        # begins with a brief; an existing one begins with what already exists,
        # and telling its owner to "describe what you want to build" points
        # them down the wrong path.
        #
        # Branch on the PROJECT, not on our own install artifacts. The first
        # version asked "did we write a sidecar?", which is a fact about
        # reflow2 — so storyflow (2,643 source files, months of history, no
        # AGENTS.md) was told to describe what it wanted to build, and never
        # heard of the adopt skill (F2,
        # docs/trials/2026-07-20-adopt-storyflow.md).
        reflow2_doc = "REFLOW2.md" if (project / "REFLOW2.md").exists() else "AGENTS.md"
        if evidence := existing_system(project):
            print(f"This looks like a system that already exists ({evidence}).")
            if (project / "REFLOW2.md").exists():
                print("Your own AGENTS.md was left alone — reflow2's instructions are in")
                print("REFLOW2.md, and your instruction file(s) gained one pointer line.")
            print()
            print("Next: open your agent here and run the **adopt** skill — genesis's sibling")
            print("  for a system that already exists. It recovers the design from what was")
            print("  built: a breadth-first coarse scan, static and dynamic analysis, intent")
            print(f"  only from sources OUTSIDE the implementation, then validation against")
            print(f"  the original. See {reflow2_doc}.")
        else:
            print("Deliberately NOT created: src/, build files, language choice — what kind of")
            print("project this is comes out of the design, not out of a scaffold.")
            print()
            print("Next: open your agent here and tell it, in a paragraph, what you want to build.")
            print("  It reads AGENTS.md, connects to reflow2, and starts asking you about the")
            print("  parts you left out. The brief does not need to be complete — that is the point.")
        # ALEX, 2026-08-13, first run on a real work project: "That .gitignore
        # ignores .reflow2 folder. Where does it store the JSON file that can be
        # committed for the graph? I can't find the artifact that can be shared
        # by other project members." He was right, and it was worse than he put
        # it: the word "export" appeared NOWHERE a user would read — not in
        # AGENTS.md, not in this summary — only in a parenthetical inside the
        # .gitignore, which parses only if you already know what an export is.
        # reflow2's central promise is a shareable design record and a fresh
        # install shipped neither the file nor the instruction to make one.
        print()
        rel = design_record_path(project).relative_to(project)
        print(f"The record you SHARE: {rel}  — created, and NOT git-ignored.")
        print("  .reflow2/ is this machine's store and is ignored on purpose; that file is the")
        print("  EXPORT of it, and it is what your teammates read and what the CI design gate")
        print("  checks. COMMIT IT. Re-export to the same path as the design grows — one")
        print("  commit per pull request should write it, and it should be the last.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
