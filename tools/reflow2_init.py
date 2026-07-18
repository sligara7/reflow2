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
TREES = [
    (KIT / ".grok", ".grok"),
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
    if not (project / ".mcp.json").exists():
        changes.append("create  .mcp.json")
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
    mcp = project / ".mcp.json"
    config = {
        "mcpServers": {
            "reflow2": {
                "command": str(binary),
                "args": ["--graph-path", str(project / ".reflow2" / "graph")],
            }
        }
    }
    if mcp.exists() and not force_mcp:
        # Never clobber a config someone has adjusted; say so instead.
        try:
            existing = json.loads(mcp.read_text())
            cmd = existing.get("mcpServers", {}).get("reflow2", {}).get("command")
            if cmd and cmd != str(binary):
                done.append(
                    f".mcp.json LEFT ALONE — it points at {cmd}, not {binary} "
                    f"(re-run with --force-mcp to repoint it)"
                )
            else:
                done.append(".mcp.json unchanged")
        except json.JSONDecodeError:
            done.append(".mcp.json is not valid JSON — left alone, fix it by hand")
    else:
        mcp.write_text(json.dumps(config, indent=2) + "\n")
        done.append(".mcp.json")

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
            print("up to date — nothing to do.")
        return 0

    updating = project.exists() and previously is not None
    project.mkdir(parents=True, exist_ok=True)
    done = install(project, binary, opts.force_mcp)

    verb = "Updated" if updating else "Set up"
    print(f"{verb} reflow2 in {project}\n")
    for d in done:
        print(f"  {d}")
    print()

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
