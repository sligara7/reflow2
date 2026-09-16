#!/usr/bin/env python3
"""Read a git diff as a design change: which requirements does this commit reach,
and which changed files can the design not see at all?

    python3 tools/impact_of_diff.py                       # what this branch changed, vs main
    python3 tools/impact_of_diff.py --range v0.61.1..HEAD # a specific range
    python3 tools/impact_of_diff.py --full                # every impacted node, with its hop chain
    python3 tools/impact_of_diff.py --fail-on-unmapped    # exit 1 if any changed file lands nowhere

reflow2's impact analysis runs design-first: record a ChangeEvent on the node you
are about to touch, then propagate. A developer at review time holds the
OPPOSITE thing — a diff — and wants to know what it touches upstream. The
machinery for that direction was already built: `reconcile_artifacts` takes the
observed hashes of registered files and returns `propagation_seeds`, the design
nodes those files realize, and `propagate_from` walks the golden thread upward
from there. Nothing drove it from a commit. This does, and it is wiring in the
same family as reflow2_check.py — no core change, and the core still does no
file I/O: the hashes are computed here and supplied.

⭐ THE ONE DESIGN POINT, AND THE REASON THIS IS A FILE RATHER THAN A ONE-LINER:
the answer is only as complete as artifact registration. A changed file no
Artifact points at lands on NOTHING, and a blast radius that silently omits it
reads as "this change is safe" when the truth is "this change is invisible".
That is a confident understatement, and a reviewer will believe it. So unmapped
files are reported FIRST, counted, and never folded into a clean result; with
`--fail-on-unmapped` they are an exit code. The design-side half of the same
hole is the `unrealized_capability` gap; this is the artifact-side half.

Exit codes, the same contract as reflow2_check.py: 0 the analysis ran (with
`--fail-on-unmapped`, and every changed file was mapped) · 1 the analysis ran
and `--fail-on-unmapped` found unmapped files · 2 it could not run.

Recorded as dec:idea-diff-driven-impact (Alex Sligar's 2026-08-05 survey) and
built on Anthony's word, 2026-09-16.
"""

from __future__ import annotations

import argparse
import json
import os
import pathlib
import subprocess
import sys
import tempfile

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from reflow2_check import Server, default_bin, hash_file  # noqa: E402

_REPO_ROOT = str(pathlib.Path(__file__).resolve().parent.parent)


def die(code: int, msg: str) -> None:
    print(f"impact_of_diff: {msg}", file=sys.stderr)
    sys.exit(code)


# ---------------------------------------------------------------------------
# What changed
# ---------------------------------------------------------------------------


def _git(args: list[str], cwd: str) -> str | None:
    try:
        out = subprocess.run(
            ["git", *args], capture_output=True, text=True, timeout=60, cwd=cwd
        )
    except (OSError, subprocess.SubprocessError):
        return None
    return out.stdout if out.returncode == 0 else None


def changed_files(root: str, rng: str | None, include_working_tree: bool) -> tuple[dict[str, bool], str]:
    """Repo-relative paths the change touched, each mapped to whether the file
    is PRESENT afterwards (a deletion is a change too, and reconcile_artifacts
    reports it as `missing_artifact` when told `present: false`).

    Returns the map and a sentence saying what was compared, because a blast
    radius with no stated baseline is a number nobody can check.
    """
    top = _git(["rev-parse", "--show-toplevel"], root)
    if not top:
        die(2, f"{root} is not inside a git working tree")
    top = top.strip()
    files: dict[str, bool] = {}
    parts: list[str] = []

    if rng:
        base, _, head = rng.partition("..")
        head = head or "HEAD"
        base = base or None
        if base is None:
            die(2, f"--range needs a base: `A..B` or `A..` (got {rng!r})")
        spec = [base, head]
        compared = f"{base}..{head}"
    else:
        merge_base = None
        for candidate in ("origin/HEAD", "origin/main", "origin/master", "main", "master"):
            mb = _git(["merge-base", "HEAD", candidate], top)
            if mb:
                merge_base, compared_to = mb.strip(), candidate
                break
        if not merge_base:
            die(2, "no merge-base with main/master could be found; pass --range A..B")
        spec = [merge_base, "HEAD"]
        compared = f"commits since the merge-base with {compared_to}"

    status = _git(["diff", "--name-status", *spec], top)
    if status is None:
        die(2, f"git diff {' '.join(spec)} failed")
    for line in status.splitlines():
        cols = line.split("\t")
        if len(cols) < 2:
            continue
        code, path = cols[0], cols[-1]
        files[os.path.normpath(path)] = not code.startswith("D")
    parts.append(compared)

    if include_working_tree:
        porcelain = _git(["status", "--porcelain"], top)
        if porcelain is not None:
            for line in porcelain.splitlines():
                if len(line) < 4:
                    continue
                path = line[3:]
                if " -> " in path:
                    path = path.split(" -> ", 1)[1]
                path = os.path.normpath(path.strip().strip('"'))
                files[path] = os.path.exists(os.path.join(top, path))
            parts.append("uncommitted work")

    return files, " and ".join(parts)


# ---------------------------------------------------------------------------
# What the design knows about those files
# ---------------------------------------------------------------------------


def artifact_index(doc: dict) -> dict[str, dict]:
    """`location` -> artifact, for every registered Artifact that names a file.
    Normalised so `./a/b` and `a/b` agree; the export is the record, so it is
    read from the file rather than asked of a server — a reviewer can run this
    against a committed export with nothing else in hand."""
    out: dict[str, dict] = {}
    for node in doc.get("nodes", []):
        if node.get("node_type") != "Artifact":
            continue
        loc = (node.get("properties") or {}).get("location")
        if not loc or not isinstance(loc, str):
            continue
        out[os.path.normpath(loc)] = node
    return out


def map_files(files: dict[str, bool], index: dict[str, dict], export_rel: str | None):
    """Split the change into what the design can see and what it cannot.

    A file maps to the Artifact whose `location` is that path, or — because a
    directory artifact claims everything beneath it — to the nearest Artifact
    whose location is one of its parent directories. The export document itself
    is set aside rather than reported as unmapped: it changes on every design
    write by construction and is the record, not the build.
    """
    mapped: list[tuple[str, dict, bool]] = []
    unmapped: list[str] = []
    record: list[str] = []
    for path, present in sorted(files.items()):
        if export_rel and os.path.normpath(path) == os.path.normpath(export_rel):
            record.append(path)
            continue
        hit = index.get(path)
        if hit is None:
            parent = os.path.dirname(path)
            while parent and parent != ".":
                if parent in index:
                    hit = index[parent]
                    break
                parent = os.path.dirname(parent)
        if hit is None:
            unmapped.append(path)
        else:
            mapped.append((path, hit, present))
    return mapped, unmapped, record


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n", 1)[0])
    ap.add_argument("--export", default="docs/design/reflow2.json",
                    help="the committed design export (default docs/design/reflow2.json)")
    ap.add_argument("--root", default=".", help="repo root the export's locations are relative to")
    ap.add_argument("--range", dest="rng", default=None,
                    help="git range A..B to analyse (default: merge-base with main .. HEAD)")
    ap.add_argument("--working-tree", action="store_true",
                    help="also include uncommitted changes")
    ap.add_argument("--full", action="store_true",
                    help="every impacted node with its hop chain, not the summary")
    ap.add_argument("--depth", type=int, default=5, help="how far to walk (default 5)")
    ap.add_argument("--fail-on-unmapped", action="store_true",
                    help="exit 1 if any changed file lands on no registered artifact")
    ap.add_argument("--json", action="store_true", help="machine-readable output")
    ap.add_argument("--bin", default=None, help="reflow2-mcp binary (default: target/, then PATH)")
    opts = ap.parse_args()

    root = os.path.abspath(opts.root)
    export_path = opts.export if os.path.isabs(opts.export) else os.path.join(root, opts.export)
    if not os.path.exists(export_path):
        die(2, f"no export at {export_path}")
    with open(export_path, encoding="utf-8") as fh:
        doc = json.load(fh)

    files, compared = changed_files(root, opts.rng, opts.working_tree)
    try:
        export_rel = os.path.relpath(export_path, root)
    except ValueError:
        export_rel = None
    mapped, unmapped, record = map_files(files, artifact_index(doc), export_rel)

    result: dict = {
        "compared": compared,
        "changed_files": len(files),
        "unmapped": unmapped,
        "record_only": record,
        "mapped": [],
        "seeds": [],
        "impact": None,
    }

    if mapped:
        binary = opts.bin or default_bin()
        with tempfile.TemporaryDirectory(prefix="impact-of-diff-") as tmp:
            graph = os.path.join(tmp, "graph")
            imported = subprocess.run(
                [binary, "--graph-path", graph, "--import", export_path],
                capture_output=True, text=True, timeout=300,
            )
            if imported.returncode != 0:
                die(2, f"could not import the export: {imported.stderr.strip()[:400]}")
            server = Server(binary, graph)
            try:
                observed = []
                for path, art, present in mapped:
                    entry = {"artifact_id": art["node_id"], "present": present}
                    if present:
                        entry["checksum"] = hash_file(os.path.join(root, path))
                    observed.append(entry)
                    result["mapped"].append({"path": path, "artifact_id": art["node_id"], "present": present})
                recon = server.call("reconcile_artifacts", {"observed": observed, "exhaustive": False})
                seeds = sorted(set(recon.get("propagation_seeds") or []))
                result["seeds"] = seeds
                result["reconcile"] = {
                    k: v for k, v in recon.items()
                    if k != "propagation_seeds" and not isinstance(v, (list, dict))
                }
                if seeds:
                    result["impact"] = server.call(
                        "propagate_from", {"seed_ids": seeds, "max_depth": opts.depth, "full": opts.full}
                    )
            finally:
                server.close()

    if opts.json:
        print(json.dumps(result, indent=2, sort_keys=True))
    else:
        render(result)

    if opts.fail_on_unmapped and unmapped:
        return 1
    return 0


def render(r: dict) -> None:
    print(f"impact_of_diff: {r['changed_files']} file(s) changed ({r['compared']})")
    if r["unmapped"]:
        print(f"\n⚠️  {len(r['unmapped'])} changed file(s) the design CANNOT SEE — no Artifact points at them, "
              f"so nothing below accounts for them:")
        for p in r["unmapped"]:
            print(f"    {p}")
        print("    Register each with link_artifact against the capability it realizes; "
              "until then this radius is an understatement.")
    else:
        print("\nevery changed file maps to a registered artifact.")
    if r["record_only"]:
        print(f"\n{len(r['record_only'])} file(s) are the design record itself and are not build: "
              + ", ".join(r["record_only"]))
    if r["mapped"]:
        print(f"\n{len(r['mapped'])} file(s) map to registered artifacts:")
        for m in r["mapped"]:
            flag = "" if m["present"] else "  (deleted)"
            print(f"    {m['path']}  ->  {m['artifact_id']}{flag}")
    seeds = r["seeds"]
    if not r["mapped"]:
        print("\nnothing to propagate: no changed file is registered.")
        return
    if not seeds:
        print("\nno propagation seeds: the changed artifacts realize nothing the design links onward, "
              "or their content did not move.")
        return
    print(f"\n{len(seeds)} design node(s) the change lands on: " + ", ".join(seeds))
    imp = r["impact"] or {}
    for band in imp.get("counts_by_distance") or []:
        print(f"    distance {band['distance']}: {band['count']} node(s)")
    ring = imp.get("direct_ring") or []
    reqs = [n for n in ring if n.get("node_type") == "Requirement"]
    caps = [n for n in ring if n.get("node_type") == "Capability"]
    if caps:
        print("\ncapabilities the changed files realize:")
        for n in caps:
            print(f"    {n['node_id']}")
    if reqs:
        print("\nrequirements reached directly:")
        for n in reqs:
            print(f"    {n['node_id']}")
    risks = imp.get("risk_crossings") or []
    if risks:
        print(f"\n{len(risks)} risk crossing(s):")
        for n in risks[:20]:
            print(f"    d{n.get('distance')}  {n['node_id']}")
        if len(risks) > 20:
            print(f"    … and {len(risks) - 20} more")
    bounds = imp.get("boundary_crossings") or []
    if bounds:
        print("\npublished boundaries crossed: " + ", ".join(bounds))
    if imp.get("impacted"):
        print(f"\nevery impacted node, with the hops that reach it:")
        for n in imp["impacted"]:
            hops = " -> ".join(
                f"{h.get('from_id')} {h.get('edge_type')} {h.get('to_id')}" for h in n.get("via") or []
            )
            flags = "".join(
                f"  [{f}]" for f, on in (("risk", n.get("crosses_risk_edge")),
                                         ("published boundary", n.get("crosses_published_boundary")))
                if on
            )
            print(f"    d{n.get('distance')}  {n.get('node_id')} ({n.get('node_type')}){flags}")
            if hops:
                print(f"          via {hops}")
    trunc = imp.get("truncated_beyond_depth")
    if trunc:
        print(f"\n{trunc} node(s) lie beyond depth {imp.get('max_depth')} and were not walked.")


if __name__ == "__main__":
    sys.exit(main())
