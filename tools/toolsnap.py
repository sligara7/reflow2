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
import time
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from smoke_mcp import Server  # noqa: E402  (path set above)

SNAP_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "toolsnaps")


# Set by main() before any verdict is printed, so every outcome — match, drift
# and regenerate — says which binary it read.
BINARY_PROVENANCE = ""


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


def provenance(binary: str) -> str:
    """What this comparison actually read, and whether it is current.

    THE GATE USED TO SAY "ALL 149 TOOLSNAPS MATCH" WITHOUT SAYING AGAINST
    WHAT. Measured 2026-08-16: a tool description was changed, RELEASE was
    rebuilt, and this reported the surface unchanged — because `--bin`
    defaults to `target/debug/reflow2-mcp`, which was an hour stale. The
    committed snapshot still held the old text and the gate whose whole job is
    noticing a changed surface reported an unchanged one.

    So the verdict now names the binary and says whether any source file is
    NEWER than it. mtime is a weak signal on its own — git operations can
    leave sources older than a build, which is why the launch wrapper hashes
    content instead — but it is only ever used here to ADD a warning, never to
    suppress one, so a false negative costs nothing that was not already lost.

    `req:a-report-says-what-it-swept-and-whether-its-checks-ran`.
    """
    built = os.path.getmtime(binary)
    newest, newest_at = None, 0.0
    for root, _dirs, files in os.walk("crates"):
        if os.sep + "src" not in root + os.sep:
            continue
        for f in files:
            if not f.endswith(".rs"):
                continue
            path = os.path.join(root, f)
            at = os.path.getmtime(path)
            if at > newest_at:
                newest, newest_at = path, at
    stamp = time.strftime("%Y-%m-%d %H:%M", time.localtime(built))
    line = f"compared against {os.path.relpath(binary)} (built {stamp})"
    if newest is not None and newest_at > built:
        line += (f"\n⚠️  STALE: {newest} is newer than that binary. This verdict is "
                 f"about code that is no longer on disk — rebuild and re-run:\n"
                 f"       cargo build -p reflow2-mcp")
    return line


# TOOL FAMILIES THAT MUST OFFER THE SAME SURFACE
# =============================================================================
#
# A family is a set of tools a user reads as siblings. Nothing made them stay
# siblings, and on 2026-08-29 that cost was measured.
#
# `cap:an-acknowledgement-says-whose-judgement-it-was` was built 2026-08-23 and
# gave `acknowledge_gap` / `acknowledge_gaps` an optional `approver`. It never
# reached `acknowledge_defect`, which went on minting an ACCEPTED Decision —
# settled intent — with no parameter that could carry a name:
#
#     acknowledge_gap(s)   HAS the param     168 acknowledgements,  51 attributed
#     acknowledge_defect   NO param           12 acknowledgements,   0 attributed
#
# ⭐ THE ZERO IS STRUCTURAL. It is not that callers skipped the field; there was
# no field. All 51 approver edges on the gap side are dated 2026-08-23/24, the
# day the parameter shipped, so there is no evidence the parameter is ignored
# when it exists. It surfaced as three CI failures against the ENFORCED
# `rule:design-intent-moves-only-on-the-owners-word`, with nine more of the same
# shape already grandfathered.
#
# 🛑 SO THIS PINS THE CLASS, NOT THE INSTANCE. Adding `approver` to one tool
# fixes those twelve nodes; it does nothing about the next capability that lands
# on one sibling and not the others. What was maintained by hand with nothing
# checking it was the FAMILY, and this is the check.
#
# The parameter may sit anywhere in the schema: a bulk form carries it PER ITEM
# (`acknowledge_gaps` takes `gaps`, and `approver` lives inside each entry),
# which is deliberate — a batch under one shared approver would record a
# judgement nobody made for every item but one.
FAMILY_INVARIANTS: list[tuple[str, str, str]] = [
    (
        "acknowledge_",
        "approver",
        "an acknowledgement mints an ACCEPTED Decision — settled intent — so it "
        "must be able to record whose judgement it was "
        "(rule:design-intent-moves-only-on-the-owners-word)",
    ),
]


def _schema_mentions(schema: object, prop: str) -> bool:
    """Is `prop` a property anywhere in this schema, at any depth?

    Depth matters: a bulk form declares the field inside its item schema rather
    than at the top level, and a check that only looked one level down would
    report `acknowledge_gaps` as missing a parameter it actually carries.
    """
    if isinstance(schema, dict):
        props = schema.get("properties")
        if isinstance(props, dict) and prop in props:
            return True
        return any(_schema_mentions(v, prop) for v in schema.values())
    if isinstance(schema, list):
        return any(_schema_mentions(v, prop) for v in schema)
    return False


def family_invariants(live: dict[str, dict]) -> int:
    """Every member of a named family offers the family's required parameter."""
    problems = 0
    for prefix, prop, why in FAMILY_INVARIANTS:
        members = sorted(n for n in live if n.startswith(prefix))
        missing = [
            n for n in members if not _schema_mentions(live[n].get("inputSchema"), prop)
        ]
        if missing:
            problems += len(missing)
            print(f"\n=== FAMILY DRIFT: {prefix}* must all take `{prop}` ===")
            print(f"  why: {why}")
            print(f"  family ({len(members)}): {', '.join(members)}")
            for n in missing:
                print(f"  MISSING in {n}")
    return problems


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

    family = family_invariants(live)

    print("\n" + "=" * 62)
    problems = len(drifted) + len(added) + len(removed) + family
    if problems:
        print(f"TOOLSNAP DRIFT ({problems}): "
              f"{len(drifted)} changed, {len(added)} added, {len(removed)} removed.")
        print("If the surface change is intentional, regenerate deliberately:")
        print("    python3 tools/toolsnap.py --update")
        print("and commit the diff — a reviewer should see exactly what moved.")
        print(BINARY_PROVENANCE)
        return 1
    print(f"ALL {len(live_names)} TOOLSNAPS MATCH — the served surface is unchanged.")
    print(f"and every tool family offers its required parameter "
          f"({len(FAMILY_INVARIANTS)} invariant(s) checked).")
    print(BINARY_PROVENANCE)
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

    global BINARY_PROVENANCE
    BINARY_PROVENANCE = provenance(binary)

    live = live_tools(binary)
    return update(live) if args.update else check(live)


if __name__ == "__main__":
    sys.exit(main())
