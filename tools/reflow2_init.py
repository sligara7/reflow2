#!/usr/bin/env python3
"""Set up (or update) reflow2 in a project.

    python3 tools/reflow2_init.py ~/projects/my-thing
    python3 tools/reflow2_init.py ~/projects/my-thing --check    # what would change
    python3 tools/reflow2_init.py ~/projects/my-thing            # re-run to update

Installs the **design environment** and nothing else: the agent instructions, the
skills, an MCP config with the binary path already filled in, and the directory
the design graph lives in.

It deliberately creates no `src/` layout, no build file, no language choice, no
project scaffolding of any kind — because *you don't know the project type yet,
and neither should this script*. What kind of project this is, and therefore how
its code should be laid out, is a decision the design loop makes with you. A
scaffold that guessed would be committing a design decision before the design
exists, which is the thing reflow2 is for.

Re-run it any time to update: the kit is copied into your project, so it
otherwise freezes at install time while reflow2 keeps moving. Re-running
refreshes the instructions and skills, leaves your design graph and your own
files alone, and tells you exactly what changed.

Standard library only.
"""
from __future__ import annotations

import argparse
import datetime
import filecmp
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
FILES = [
    (KIT / "AGENTS.md", "AGENTS.md"),
]

# Where kit content goes when the project already owns that filename. AGENTS.md
# is the case that matters: every brownfield target has one, and it is the file
# the project actually runs on.
SIDECAR = {"AGENTS.md": "REFLOW2.md"}


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

# Skills go to every directory some harness actually searches. There is no
# single location: `.claude/skills/` is read by Claude Code, OpenCode and
# Copilot/VS Code (the latter two name it "Claude-compatible" outright), while
# Grok CLI reads only `.grok/skills/`. Installing one of them means the other
# harnesses see an AGENTS.md naming skills they cannot load — which is exactly
# what happened: the kit shipped `.grok/` alone, so the Grok trial under
# opencode found no reflow2 skills at all and had to read the files by hand.
#
# Adding a harness here is one line. See docs/skills/README.md for the tables.
TREES = [
    (KIT / "skills", ".claude/skills"),
    (KIT / "skills", ".grok/skills"),
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
    """Identify the kit so a later run can tell whether it moved."""
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
    """One line on whether this checkout is behind the remote.

    Deliberately not a nag and deliberately not automatic on every server start:
    a network call per session would be intrusive and would hang offline. It runs
    when someone deliberately asks — which is what this script is.
    """
    if local_commit is None:
        return "checkout: unknown (no git metadata here)"
    head = upstream_head()
    if head is None:
        return "upstream: could not check (offline, or no access to the repo)"
    if head.startswith(local_commit) or local_commit.startswith(head):
        return f"upstream: current ({local_commit})"
    return (
        f"upstream: BEHIND — this checkout is at {local_commit}, the remote is at {head}.\n"
        f"  Update in this order, or your project gets current instructions on an old server:\n"
        f"    1. git -C {REPO} pull --rebase\n"
        f"    2. cargo build -p reflow2-mcp --release      # rebuild before re-running this\n"
        f"    3. python3 {REPO}/tools/reflow2_init.py <your project>"
    )


def find_binary() -> Path | None:
    for build in ("release", "debug"):
        p = REPO / "target" / build / "reflow2-mcp"
        if p.exists():
            return p
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
MCP_CONFIGS = [
    {
        "path": ".mcp.json",
        "key": "mcpServers",
        "entry": lambda b, g: {"command": str(b), "args": ["--graph-path", str(g)]},
        "extract": lambda e: e.get("command"),
        "extra": {},
    },
    {
        "path": "opencode.json",
        "key": "mcp",
        "entry": lambda b, g: {
            "type": "local",
            "command": [str(b), "--graph-path", str(g)],
            "enabled": True,
        },
        # OpenCode takes command+args as one array; the binary is its head.
        "extract": lambda e: (e.get("command") or [None])[0],
        "extra": {"$schema": "https://opencode.ai/config.json"},
    },
    {
        "path": ".vscode/mcp.json",
        "key": "servers",
        "entry": lambda b, g: {"command": str(b), "args": ["--graph-path", str(g)]},
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
    graph = project / ".reflow2" / "graph"
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

        current = existing.get(spec["key"], {}).get("reflow2")
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


def ensure_pointer(agents_md: Path, side_name: str) -> str | None:
    """Append one marked pointer line to the project's own instruction file,
    unless it already mentions the sidecar. Returns a report line, or None."""
    text = agents_md.read_text()
    if side_name in text:
        return None
    line = POINTER_LINE.format(side=side_name)
    agents_md.write_text(text.rstrip("\n") + "\n\n" + line + "\n")
    return (
        f"{agents_md.name}  (appended one marked line pointing at {side_name} — "
        f"without it the agent never learns reflow2 exists)"
    )


def ensure_gitignore(project: Path) -> str | None:
    """Keep the graph directory out of version control: it is machine-local
    RocksDB state (binary files and a lock); the durable, reviewable record is
    an export. Appends or creates, idempotent, reported. Returns None when
    `.reflow2` is already covered."""
    gi = project / ".gitignore"
    if gi.exists():
        if any(".reflow2" in line for line in gi.read_text().splitlines()):
            return None
        gi.write_text(
            gi.read_text().rstrip("\n")
            + "\n\n# reflow2's local design graph (machine state; the durable record is an export)\n.reflow2/\n"
        )
        return ".gitignore  (added .reflow2/ — the graph is machine-local state)"
    gi.write_text(
        "# reflow2's local design graph (machine state; the durable record is an export)\n.reflow2/\n"
    )
    return ".gitignore  (created, ignoring .reflow2/ — the graph is machine-local state)"


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
            if side.name not in dst.read_text():
                changes.append(f"append  one marked pointer line to your {rel} (→ {side.name})")
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
    for spec in MCP_CONFIGS:
        path = project / spec["path"]
        if not path.exists():
            changes.append(f"create  {spec['path']}")
        else:
            try:
                current = json.loads(path.read_text()).get(spec["key"], {}).get("reflow2")
            except (json.JSONDecodeError, AttributeError):
                current = None
            if current is None:
                changes.append(f"update  {spec['path']} (add the reflow2 server)")
    if not (project / ".reflow2").exists():
        changes.append("create  .reflow2/")
    gi = project / ".gitignore"
    if not gi.exists():
        changes.append("create  .gitignore  (ignoring .reflow2/)")
    elif not any(".reflow2" in line for line in gi.read_text().splitlines()):
        changes.append("append  .reflow2/ to .gitignore")
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
    """
    graph = project / ".reflow2" / "graph"
    if not graph.exists():
        return None  # nothing designed yet
    out = subprocess.run(
        [str(binary), "--graph-path", str(graph), "--export"],
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


def install(project: Path, binary: Path, force_mcp: bool) -> list[str]:
    if problems := check_skills():
        raise SystemExit(
            "refusing to install: these skills would be silently ignored by the agent\n  "
            + "\n  ".join(problems)
        )
    done = []
    if note := backup_graph(project, binary):
        done.append(note)
    for src, rel in FILES:
        dst = project / rel
        dst.parent.mkdir(parents=True, exist_ok=True)
        if owner := foreign_owner(src, dst):
            # The project has its own file here. Overwriting it destroys the
            # instructions the project actually runs on — and AGENTS.md is
            # exactly the file every brownfield target already has.
            side = project / SIDECAR.get(rel, f"REFLOW2_{rel}")
            changed = not side.exists() or not filecmp.cmp(src, side, shallow=False)
            shutil.copy2(src, side)
            if changed:
                done.append(f"{side.name}  (kept your own {rel} — {owner})")
            # A sidecar nobody points at is invisible: the agent reads the
            # project's own file and never learns reflow2 exists (the BL-22
            # lesson — shipping the file is not shipping the capability). One
            # marked line is appended, same rule as the merged MCP configs:
            # add and report, never overwrite. Idempotent by content.
            if pointer := ensure_pointer(dst, side.name):
                done.append(pointer)
            continue
        changed = not dst.exists() or not filecmp.cmp(src, dst, shallow=False)
        shutil.copy2(src, dst)
        if changed:
            done.append(rel)
    for src, rel in TREES:
        for path in sorted(src.rglob("*")):
            if path.is_dir():
                continue
            dst = project / rel / path.relative_to(src)
            dst.parent.mkdir(parents=True, exist_ok=True)
            changed = not dst.exists() or not filecmp.cmp(path, dst, shallow=False)
            shutil.copy2(path, dst)
            if changed:
                done.append(str(Path(rel) / path.relative_to(src)))

    # MCP config, with the binary path already resolved — the step people
    # previously had to hand-edit, and the one most likely to be got wrong.
    for spec in MCP_CONFIGS:
        done.append(write_mcp_config(project, spec, binary, force_mcp))

    (project / ".reflow2").mkdir(exist_ok=True)
    if note := ensure_gitignore(project):
        done.append(note)
    stamp = project / STAMP
    stamp.write_text(json.dumps(kit_version(), indent=2) + "\n")
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
    opts = ap.parse_args()

    project = Path(opts.project).expanduser().resolve()
    if not KIT.exists():
        print(f"error: kit not found at {KIT}", file=sys.stderr)
        return 1

    binary = find_binary()
    if binary is None and not opts.check:
        print(
            "error: reflow2-mcp is not built yet.\n"
            "  cargo build -p reflow2-mcp --release\n"
            "(first build compiles RocksDB — around ten minutes, then cached)",
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
    else:
        print(f"{staleness(kit_version().get('commit'))}\n")
        # The two starting states want opposite advice: a greenfield project
        # begins with a brief; an existing one begins with what already exists,
        # and telling its owner to "describe what you want to build" points
        # them down the wrong path (BL-27's conversion probe).
        if (project / "REFLOW2.md").exists():
            print("This project already had its own AGENTS.md, so the reflow2 instructions")
            print("are in REFLOW2.md and your file gained one pointer line — nothing else.")
            print()
            print("Next: open your agent here and run the **adopt** skill — genesis's sibling")
            print("  for a system that already exists. It recovers the design from what was")
            print("  built: a breadth-first coarse scan, static and dynamic analysis, intent")
            print("  only from sources OUTSIDE the implementation, then validation against")
            print("  the original. See REFLOW2.md.")
        else:
            print("Deliberately NOT created: src/, build files, language choice — what kind of")
            print("project this is comes out of the design, not out of a scaffold.")
            print()
            print("Next: open your agent here and tell it, in a paragraph, what you want to build.")
            print("  It reads AGENTS.md, connects to reflow2, and starts asking you about the")
            print("  parts you left out. The brief does not need to be complete — that is the point.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
