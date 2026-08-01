#!/usr/bin/env python3
"""Which parts of reflow2's surface do real sessions actually drive?

The sixth instrument (docs/sharpening.md §4), and the first that measures the
SURFACE rather than the engine. The other five ask whether the design brain
works; this asks whether anyone can find it — which is a different failure and
was invisible to all of them.

It reads Claude Code session transcripts (JSONL) and reports three things:

  1. CALL COUNTS per served tool, and which tools were never called at all.
  2. THE SELF-LOOP SHARE — what fraction of tool-to-tool transitions are the
     same tool called again. A high share means the surface is missing a bulk
     form, not that the work is inherently repetitive: nobody calls one tool
     144 times in a session because they want to.
  3. PAGERANK over the transition graph WITH SELF-LOOPS REMOVED — which tools
     real workflows route THROUGH. This deliberately disagrees with raw counts,
     and the disagreement is the point: raw volume finds the batching problems,
     PageRank finds the spine. A tool that is high-volume and low-PageRank is a
     bookkeeping loop; a tool that is high-PageRank is load-bearing and had
     better be well surfaced.

Plus the same read for SKILLS, via `get_skill` calls.

READ-ONLY AND OFFLINE. It looks at transcripts on disk, talks to no server, and
writes nothing back into any design — "looking is not writing"
(`dec:loop-status-state-not-history`). Nothing here becomes graph state.

HONEST LIMIT, and it must be stated wherever the output is quoted: absence is
weak evidence. This sees only the transcripts still on disk, so "never called"
means "not called in the retained sample". Calibrated 2026-08-01 against a known
case — `set_project_mode` appears as text in ten transcripts with zero recorded
calls, so a tool can be discussed far more than it is used, and a tool used
before the retention window looks identical to one never used at all.

    python3 tools/surface_usage.py [--json out.json] [--projects-dir DIR]
"""

from __future__ import annotations

import argparse
import glob
import json
import os
from collections import Counter, defaultdict

PREFIX = "mcp__reflow2__"
DEFAULT_ROOT = os.path.expanduser("~/.claude/projects")
DAMPING = 0.85
ITERATIONS = 100


def sessions(root: str):
    """(project, path) for every transcript under `root`."""
    for d in sorted(glob.glob(os.path.join(root, "*"))):
        for f in sorted(glob.glob(os.path.join(d, "*.jsonl"))):
            yield os.path.basename(d), f


def tool_calls(path: str):
    """(tool_name, input) in order for one session, tolerant of partial lines.

    A transcript being written by a live session can end mid-line, and a single
    unparseable line must not cost the whole file.
    """
    with open(path, "r", errors="replace") as fh:
        for line in fh:
            line = line.strip()
            if not line or '"tool_use"' not in line:
                continue
            try:
                obj = json.loads(line)
            except ValueError:
                continue
            content = (obj.get("message") or {}).get("content")
            if not isinstance(content, list):
                continue
            for block in content:
                if isinstance(block, dict) and block.get("type") == "tool_use":
                    yield block.get("name", "?"), block.get("input") or {}


def pagerank(nodes, edges):
    """PageRank over a weighted directed graph given as {(a, b): weight}.

    Hand-rolled rather than taken from `dynograph_graph::pagerank` only because
    this is a Python probe and the crate is Rust; a productised version should
    use the crate, which is already a pinned dependency (see BL-149).
    """
    idx = {n: i for i, n in enumerate(nodes)}
    out = defaultdict(list)
    for (a, b), w in edges.items():
        out[idx[a]].append((idx[b], float(w)))
    n = len(nodes)
    if n == 0:
        return {}
    rank = [1.0 / n] * n
    for _ in range(ITERATIONS):
        nxt = [(1 - DAMPING) / n] * n
        dangling = 0.0
        for i in range(n):
            total = sum(w for _, w in out[i])
            if total == 0.0:
                dangling += rank[i]
                continue
            for j, w in out[i]:
                nxt[j] += DAMPING * rank[i] * w / total
        for i in range(n):
            nxt[i] += DAMPING * dangling / n
        rank = nxt
    return {nodes[i]: rank[i] for i in range(n)}


def served_tools(repo_root: str) -> set[str]:
    """The tool names this build actually serves, from the committed toolsnaps."""
    snaps = glob.glob(os.path.join(repo_root, "tools", "toolsnaps", "*.json"))
    return {os.path.basename(p)[:-5] for p in snaps}


def measure(root: str):
    per_tool = Counter()
    per_project = defaultdict(Counter)
    skills = Counter()
    transitions = Counter()
    sessions_using = defaultdict(set)
    other_tools = Counter()
    scanned = 0

    for project, path in sessions(root):
        scanned += 1
        sid = f"{project}:{os.path.basename(path)}"
        previous = None
        for name, args in tool_calls(path):
            if not name.startswith(PREFIX):
                other_tools[name] += 1
                # Only chain reflow2 -> reflow2 steps: a Bash call between two
                # graph writes is not a step in a design workflow.
                previous = None
                continue
            tool = name[len(PREFIX):]
            per_tool[tool] += 1
            per_project[project][tool] += 1
            sessions_using[tool].add(sid)
            if tool == "get_skill" and args.get("name"):
                skills[args["name"]] += 1
            if previous is not None:
                transitions[(previous, tool)] += 1
            previous = tool

    return {
        "scanned": scanned,
        "per_tool": per_tool,
        "per_project": per_project,
        "skills": skills,
        "transitions": transitions,
        "sessions_using": sessions_using,
        "other_tools": other_tools,
    }


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--json", help="write the full result here")
    ap.add_argument("--projects-dir", default=DEFAULT_ROOT)
    ap.add_argument("--top", type=int, default=15)
    args = ap.parse_args()

    repo = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    m = measure(args.projects_dir)
    per_tool, transitions = m["per_tool"], m["transitions"]

    total_calls = sum(per_tool.values())
    if total_calls == 0:
        print(f"no reflow2 tool calls found under {args.projects_dir}")
        return 0

    served = served_tools(repo)
    never = sorted(served - set(per_tool)) if served else []

    total_steps = sum(transitions.values())
    self_loops = sum(n for (a, b), n in transitions.items() if a == b)
    share = self_loops / total_steps if total_steps else 0.0

    real = {(a, b): n for (a, b), n in transitions.items() if a != b}
    nodes = sorted({x for pair in real for x in pair})
    ranks = pagerank(nodes, real)

    print(f"sessions {m['scanned']}   reflow2 calls {total_calls}   "
          f"distinct tools {len(per_tool)}"
          + (f" of {len(served)} served" if served else ""))
    print()
    print(f"SELF-LOOP SHARE  {self_loops}/{total_steps} = {share:.1%} of transitions "
          f"are the same tool called again")
    print("  the batching offenders (self-loops · total calls · sessions):")
    for (a, _b), n in sorted(
        ((k, v) for k, v in transitions.items() if k[0] == k[1]),
        key=lambda kv: -kv[1],
    )[: args.top]:
        s = len(m["sessions_using"][a])
        print(f"    {a:<28} {n:>5} · {per_tool[a]:>5} · {s:>2} sessions "
              f"(~{per_tool[a] // max(s, 1)}/session)")
    print()
    print("PAGERANK over real (non-self-loop) transitions — what workflows route THROUGH:")
    for tool in sorted(ranks, key=lambda t: -ranks[t])[: args.top]:
        print(f"    {ranks[tool]:.4f}  {tool}")
    print()
    skill_calls = per_tool.get("get_skill", 0)
    print(f"SKILLS: {skill_calls} get_skill calls in {total_calls} tool calls"
          + (f"  (one per {total_calls // skill_calls} calls)" if skill_calls else ""))
    for s, n in m["skills"].most_common():
        print(f"    {n:>3}  {s}")
    if never:
        print()
        print(f"NEVER CALLED in the retained sample ({len(never)} of {len(served)} served) — "
              "absence is WEAK evidence, see the module docstring:")
        for t in never:
            print(f"    {t}")

    if args.json:
        json.dump(
            {
                "sessions_scanned": m["scanned"],
                "calls_total": total_calls,
                "self_loop_share": share,
                "self_loops": self_loops,
                "transition_steps": total_steps,
                "per_tool": dict(per_tool.most_common()),
                "sessions_using": {k: len(v) for k, v in m["sessions_using"].items()},
                "per_project": {p: dict(c.most_common()) for p, c in m["per_project"].items()},
                "skills": dict(m["skills"].most_common()),
                "pagerank": dict(sorted(ranks.items(), key=lambda kv: -kv[1])),
                "never_called": never,
                "transitions": {f"{a}>{b}": n for (a, b), n in transitions.most_common()},
            },
            open(args.json, "w"),
            indent=1,
        )
        print(f"\nwrote {args.json}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
