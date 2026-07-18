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
import filecmp
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


def planned_changes(project: Path) -> list[str]:
    """What a run would create or overwrite, without touching anything."""
    changes = []
    for src, rel in FILES:
        dst = project / rel
        if not dst.exists():
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
    return changes


def install(project: Path, binary: Path, force_mcp: bool) -> list[str]:
    done = []
    for src, rel in FILES:
        dst = project / rel
        dst.parent.mkdir(parents=True, exist_ok=True)
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
        print("Deliberately NOT created: src/, build files, language choice — what kind of")
        print("project this is comes out of the design, not out of a scaffold.")
        print()
        print("Next: open your agent here and tell it, in a paragraph, what you want to build.")
        print("  It reads AGENTS.md, connects to reflow2, and starts asking you about the")
        print("  parts you left out. The brief does not need to be complete — that is the point.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
