#!/usr/bin/env python3
"""Toolsnaps — one committed golden JSON per MCP tool, CI-diffed.

The surface an agent binds to is the `tools/list` payload: a tool's name, the
prose it picks the tool by, its input schema, and its annotations. That surface
has changed silently before and nothing noticed — a parameter lost its type
(BL-28), a stale binary served an old shape (BL-32), prose moved into the wrong
envelope (BL-48). This makes the surface a *reviewed artifact*: every tool's
served schema is frozen in `tools/toolsnaps/<tool>.json`, and this script fails
if the live binary disagrees with the committed golden. A real surface change is
then a deliberate `--update` that shows up in the diff, named tool by tool — the
BL-28/32/48 bug family turned into a mechanical tripwire.

This drives the **built binary** over real stdio, for the same reason
smoke_mcp.py does: every home-grown client agrees with the server we wrote, so
only the shipped wire format is trustworthy. It reuses smoke_mcp's Server.

Usage (from the repo root, after `cargo build -p reflow2-mcp`):

    python3 tools/toolsnap.py            # check: live surface vs committed goldens
    python3 tools/toolsnap.py --update   # regenerate the goldens (review the diff!)
    python3 tools/toolsnap.py --bin target/release/reflow2-mcp

Exits 0 when every tool matches its golden, 1 on any drift (or a tool added or
removed without a corresponding golden). Standard library only.
"""
from __future__ import annotations

import argparse
import difflib
import json
import os
import shutil
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from smoke_mcp import Server  # noqa: E402  (path set above)

SNAP_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "toolsnaps")


def canonical(tool: dict) -> str:
    """The stable, reviewable form of one served tool.

    The whole served object is snapshotted — name, description, inputSchema,
    annotations, and anything else the surface grows — so any change to what a
    client sees shows up in the diff. Sorted keys and a trailing newline keep
    the file diff-friendly and independent of the server's field order.
    """
    return json.dumps(tool, indent=2, sort_keys=True, ensure_ascii=False) + "\n"


def live_tools(binary: str) -> dict[str, dict]:
    graph_path = tempfile.mkdtemp(prefix="reflow2-toolsnap-")
    try:
        s = Server(binary, graph_path)
        try:
            tools = s.rpc("tools/list", {})["result"]["tools"]
        finally:
            s.close()
    finally:
        shutil.rmtree(graph_path, ignore_errors=True)
    return {t["name"]: t for t in tools}


def snap_path(name: str) -> str:
    return os.path.join(SNAP_DIR, f"{name}.json")


def update(live: dict[str, dict]) -> int:
    os.makedirs(SNAP_DIR, exist_ok=True)
    # Remove goldens for tools that no longer exist, so a deleted tool cannot
    # leave a stale snapshot behind (a silent drop of its own kind).
    existing = {f[:-5] for f in os.listdir(SNAP_DIR) if f.endswith(".json")}
    removed = sorted(existing - set(live))
    for name in removed:
        os.remove(snap_path(name))
    written = 0
    for name, tool in sorted(live.items()):
        with open(snap_path(name), "w", encoding="utf-8") as fh:
            fh.write(canonical(tool))
        written += 1
    print(f"wrote {written} toolsnap(s) to {os.path.relpath(SNAP_DIR)}", end="")
    print(f", removed {len(removed)} stale" if removed else "")
    for name in removed:
        print(f"  - removed {name}.json")
    return 0


def check(live: dict[str, dict]) -> int:
    if not os.path.isdir(SNAP_DIR):
        print(f"no toolsnaps directory at {SNAP_DIR}\n"
              f"Create it first:  python3 tools/toolsnap.py --update")
        return 1
    golden = {f[:-5] for f in os.listdir(SNAP_DIR) if f.endswith(".json")}
    live_names = set(live)

    added = sorted(live_names - golden)
    removed = sorted(golden - live_names)
    drifted: list[str] = []

    for name in sorted(live_names & golden):
        want = open(snap_path(name), encoding="utf-8").read()
        have = canonical(live[name])
        if want != have:
            drifted.append(name)
            print(f"\n=== DRIFT: {name} (committed golden vs live binary) ===")
            diff = difflib.unified_diff(
                want.splitlines(keepends=True),
                have.splitlines(keepends=True),
                fromfile=f"toolsnaps/{name}.json (committed)",
                tofile=f"{name} (live)",
            )
            sys.stdout.writelines(diff)

    for name in added:
        print(f"\n=== NEW TOOL with no golden: {name} ===")
        print("  a tool shipped without a committed toolsnap")
    for name in removed:
        print(f"\n=== GOLDEN with no tool: {name} ===")
        print("  a committed toolsnap has no matching served tool")

    print("\n" + "=" * 62)
    problems = len(drifted) + len(added) + len(removed)
    if problems:
        print(f"TOOLSNAP DRIFT ({problems}): "
              f"{len(drifted)} changed, {len(added)} added, {len(removed)} removed.")
        print("If the surface change is intentional, regenerate deliberately:")
        print("    python3 tools/toolsnap.py --update")
        print("and commit the diff — a reviewer should see exactly what moved.")
        return 1
    print(f"ALL {len(live_names)} TOOLSNAPS MATCH — the served surface is unchanged.")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--bin", default="target/debug/reflow2-mcp",
                    help="path to the reflow2-mcp binary (default: %(default)s)")
    ap.add_argument("--update", "--bless", action="store_true", dest="update",
                    help="regenerate the golden toolsnaps (review the diff before committing)")
    args = ap.parse_args()

    binary = os.path.abspath(args.bin)
    if not os.path.exists(binary):
        print(f"binary not found: {binary}\nBuild it first:  cargo build -p reflow2-mcp")
        return 1

    live = live_tools(binary)
    return update(live) if args.update else check(live)


if __name__ == "__main__":
    sys.exit(main())
