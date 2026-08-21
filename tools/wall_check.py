#!/usr/bin/env python3
"""Do the walls this design declares actually hold in its source? — any project.

`req:modularity-computed` — *modularity is computed from the design, not
asserted by the architecture diagram.* This computes it, for whatever project
the design describes, by holding the declared decomposition up against the real
import graph of the files the design itself points at.

⭐ IT NEEDS NO CONFIGURATION, AND THAT IS THE WHOLE DESIGN. The design already
says where its code lives: every `Artifact` carries a `location`, and `REALIZES`
says which `Component` that file is part of. So the file set, the
component mapping and the project layout all come from the graph. There is
nothing to point at a repo, no language to declare, no paths to keep in step.

WHAT IT REPLACED, and why that matters more than it sounds. Until 2026-08-21
this tool GUESSED the mapping: it walked `crates/*/src` — hardcoded — and matched
a Rust module to a Component by NAME. Measured against the declared mapping on
reflow2's own graph, the two disagreed in both directions: name-matching reached
48 components, the graph reached 45, with 4 only in the graph and 7 only in the
name match. A tool that answers "is your decomposition sound" cannot be guessing
which file belongs to which part of it. And the guess is structurally blind to
every non-Rust file — the declared mapping reaches `.py`, `.yaml` and `.md`
because a location is just a path.

⭐ THE MEASUREMENT THAT MOTIVATED THE TOOL, kept because it is the case to beat.
PR #262 made reflow2's module graph acyclic. Lifting those same acyclic edges to
the declared subsystems produced TWO CYCLES — `store` ↔ `vocabulary` and
`coherence-loop` ↔ `time-history`. Neither was a defect in the source: the first
was a five-module kernel bisected by the decomposition, the second a single
back-edge. A wall you can hand a change to is one traffic crosses in ONE
direction; at a mutually coupled boundary there is nothing for a blast radius to
stop at, so severability there is not unmeasured — it is false.

⚠️ WHAT THIS CANNOT CONCLUDE — the trap that predates the tool.

An import graph is **coupling, not a contract**. On 2026-08-19 a mechanical scan
proposed 91 CONSUMES against a real 17; of 33 modules said to consume dyno-core,
95 of 95 references were TYPES and not one was a call. The sparse hand-authored
set was right. So these numbers are the right input for *"where could a wall go,
and does the one I declared hold?"* and the WRONG input for CONSUMES. **Nothing
here should be written into the graph as a contract edge.**

Three smaller limits, stated so they are not rediscovered:

  (a) Comments and string literals are stripped before anything is read. The
      first adopt pass reported a cycle that was a rustdoc link in a comment;
      stripping prose removed 15% of the model and revealed a real cycle the
      noise had masked. **Never derive structure from prose.**
  (b) Only languages with a scanner here are read. Everything else is COUNTED
      AND NAMED under "could not read", never silently skipped — a file the
      tool cannot parse is not a file with no dependencies.
  (c) An import it cannot resolve to a registered file is counted and reported
      for the same reason. A low resolution rate means the answer is thin, and
      you should be told rather than left to assume coverage.

⚠️ THIS IS AN INSTRUMENT, NOT A GATE. It always exits 0. Whether a subsystem
cycle should STOP THE BUILD is a governance question with an owner, and the
`governance-proposal` skill exists precisely so a tool does not answer it by
default. `tools/reflow2_check.py` is the gate.
"""

import argparse
import json
import os
import re
import sys
from collections import defaultdict

DEFAULT_EXPORT = "docs/design/reflow2.json"


# ---------------------------------------------------------------- prose stripping


def strip_rust(src: str) -> str:
    """Remove Rust comments and both string-literal forms. See limit (a)."""
    out = []
    i, n = 0, len(src)
    while i < n:
        c = src[i]
        if c == "/" and i + 1 < n and src[i + 1] == "/":
            while i < n and src[i] != "\n":
                i += 1
        elif c == "/" and i + 1 < n and src[i + 1] == "*":
            depth, i = 1, i + 2
            while i < n and depth:
                if src[i] == "/" and i + 1 < n and src[i + 1] == "*":
                    depth, i = depth + 1, i + 2
                elif src[i] == "*" and i + 1 < n and src[i + 1] == "/":
                    depth, i = depth - 1, i + 2
                else:
                    i += 1
        elif c == "r" and i + 1 < n and src[i + 1] in '#"':
            j, hashes = i + 1, 0
            while j < n and src[j] == "#":
                hashes, j = hashes + 1, j + 1
            if j < n and src[j] == '"':
                end = '"' + "#" * hashes
                k = src.find(end, j + 1)
                i = (k + len(end)) if k != -1 else n
            else:
                out.append(c)
                i += 1
        elif c == '"':
            i += 1
            while i < n:
                if src[i] == "\\":
                    i += 2
                elif src[i] == '"':
                    i += 1
                    break
                else:
                    i += 1
        else:
            out.append(c)
            i += 1
    return "".join(out)


def strip_python(src: str) -> str:
    """Remove Python comments, docstrings and string literals. See limit (a).

    Triple-quoted forms first: a module docstring naming other modules is
    exactly the prose that must not become structure.
    """
    src = re.sub(r'"""(?:.|\n)*?"""', "", src)
    src = re.sub(r"'''(?:.|\n)*?'''", "", src)
    src = re.sub(r'"(?:[^"\\\n]|\\.)*"', '""', src)
    src = re.sub(r"'(?:[^'\\\n]|\\.)*'", "''", src)
    src = re.sub(r"#[^\n]*", "", src)
    return src


# ---------------------------------------------------------------- import scanning


def imports_rust(text: str, path: str, by_module: dict) -> tuple:
    """(resolved file paths, unresolved symbol count) for one Rust file."""
    crate_root = None
    parts = path.split(os.sep)
    if "src" in parts:
        crate_root = os.sep.join(parts[: parts.index("src") + 1])
    hits, misses = set(), 0
    # `crate::x`, `super::x`, `self::x`, and `some_crate::x`
    for m in re.finditer(r"\b([A-Za-z_][A-Za-z0-9_]*)((?:::[A-Za-z_][A-Za-z0-9_]*)+)", text):
        head, tail = m.group(1), m.group(2)
        first = tail.strip(":").split("::")[0]
        if head in ("crate", "self", "super"):
            target = by_module.get((crate_root, first))
        elif "_" in head or head.islower():
            # another crate in this workspace, e.g. `reflow2_core::nodes`
            target = by_module.get((head.replace("_", "-"), first))
        else:
            continue
        if target:
            hits.add(target)
        elif head in ("crate", "self", "super"):
            misses += 1
    return hits, misses


def imports_python(text: str, path: str, by_module: dict) -> tuple:
    """(resolved file paths, unresolved symbol count) for one Python file."""
    hits, misses = set(), 0
    names = set()
    for m in re.finditer(r"^\s*import\s+([A-Za-z_][\w.]*)", text, re.M):
        names.add(m.group(1).split(".")[-1])
    for m in re.finditer(r"^\s*from\s+\.*([A-Za-z_][\w.]*)\s+import", text, re.M):
        names.add(m.group(1).split(".")[-1])
    for m in re.finditer(r"^\s*from\s+\.+\s*import\s+([A-Za-z_]\w*)", text, re.M):
        names.add(m.group(1))
    for name in names:
        target = by_module.get((None, name))
        if target and target != path:
            hits.add(target)
        elif target is None:
            misses += 1
    return hits, misses


SCANNERS = {
    ".rs": (strip_rust, imports_rust),
    ".py": (strip_python, imports_python),
}


# ---------------------------------------------------------------- graph algorithms


def find_cycles(nodes, edges):
    """Tarjan SCCs of size > 1 (plus self-loops), each sorted."""
    index, low, on_stack, stack, found, counter = {}, {}, {}, [], [], [0]

    def visit(v):
        index[v] = low[v] = counter[0]
        counter[0] += 1
        stack.append(v)
        on_stack[v] = True
        for w in sorted(edges.get(v, ())):
            if w not in index:
                visit(w)
                low[v] = min(low[v], low[w])
            elif on_stack.get(w):
                low[v] = min(low[v], index[w])
        if low[v] == index[v]:
            component = []
            while True:
                w = stack.pop()
                on_stack[w] = False
                component.append(w)
                if w == v:
                    break
            if len(component) > 1 or v in edges.get(v, ()):
                found.append(sorted(component))

    for v in sorted(nodes):
        if v not in index:
            visit(v)
    return found


def reverse_reach(nodes, edges):
    rev = defaultdict(set)
    for a, targets in edges.items():
        for b in targets:
            rev[b].add(a)
    out = {}
    for n in nodes:
        seen, stack = set(), [n]
        while stack:
            v = stack.pop()
            for w in rev.get(v, ()):
                if w not in seen:
                    seen.add(w)
                    stack.append(w)
        seen.discard(n)
        out[n] = seen
    return out


# ---------------------------------------------------------------- the design side


def read_design(export_path):
    """Component → files, containment, levels and declared coupling, from the graph."""
    doc = json.load(open(export_path, encoding="utf-8"))
    node_type = {n["node_id"]: n["node_type"] for n in doc["nodes"]}
    props = {n["node_id"]: n.get("properties", {}) for n in doc["nodes"]}
    components = [i for i, t in node_type.items() if t == "Component"]

    files_of = defaultdict(list)
    for e in doc["edges"]:
        if (
            e["edge_type"] == "REALIZES"
            and node_type.get(e["from_id"]) == "Artifact"
            and e["to_id"] in set(components)
        ):
            loc = props[e["from_id"]].get("location")
            if loc:
                files_of[e["to_id"]].append(loc)

    parent, provides, consumes, declared = {}, defaultdict(set), defaultdict(set), defaultdict(set)
    for e in doc["edges"]:
        a, b, t = e["from_id"], e["to_id"], e["edge_type"]
        if t == "CONTAINS" and node_type.get(a) == node_type.get(b) == "Component":
            parent[b] = a
        elif t == "PROVIDES" and node_type.get(a) == "Component":
            provides[b].add(a)
        elif t == "CONSUMES" and node_type.get(a) == "Component":
            consumes[a].add(b)
        elif t == "DEPENDS_ON" and node_type.get(a) == node_type.get(b) == "Component":
            declared[a].add(b)
    for consumer, ifaces in consumes.items():
        for iface in ifaces:
            for provider in provides.get(iface, ()):
                if provider != consumer:
                    declared[consumer].add(provider)
    level = {c: (props[c].get("level") or "component") for c in components}
    return components, files_of, parent, level, declared


# ---------------------------------------------------------------- the source side


def scan(files_of):
    """Real coupling between components, from the files the design points at."""
    owner, by_module = {}, {}
    unreadable, unsupported = [], defaultdict(list)
    for comp, paths in files_of.items():
        for p in paths:
            owner[p] = comp
            ext = os.path.splitext(p)[1]
            if ext not in SCANNERS:
                unsupported[ext].append(p)
                continue
            if ext == ".rs":
                parts = p.split(os.sep)
                crate_root = os.sep.join(parts[: parts.index("src") + 1]) if "src" in parts else None
                stem = os.path.splitext(os.path.basename(p))[0]
                by_module[(crate_root, stem)] = p
                if crate_root:
                    crate_name = os.path.basename(os.path.dirname(crate_root))
                    by_module[(crate_name, stem)] = p
            else:
                by_module[(None, os.path.splitext(os.path.basename(p))[0])] = p

    edges, unresolved, read = defaultdict(set), 0, 0
    for comp, paths in files_of.items():
        for p in paths:
            ext = os.path.splitext(p)[1]
            if ext not in SCANNERS or not os.path.exists(p):
                if ext in SCANNERS:
                    unreadable.append(p)
                continue
            strip, find = SCANNERS[ext]
            text = strip(open(p, encoding="utf-8", errors="replace").read())
            hits, missed = find(text, p, by_module)
            unresolved += missed
            read += 1
            for target in hits:
                other = owner.get(target)
                if other and other != comp:
                    edges[comp].add(other)
    return edges, dict(unsupported), unreadable, unresolved, read


# ---------------------------------------------------------------- report


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--export", default=DEFAULT_EXPORT, help="the committed design export")
    args = ap.parse_args()

    if not os.path.exists(args.export):
        print(f"no design export at {args.export} — nothing to check against.")
        return 0
    components, files_of, parent, level, declared = read_design(args.export)
    edges, unsupported, unreadable, unresolved, read = scan(files_of)

    print("=" * 74)
    print("COVERAGE — what this answer is actually built on")
    print("=" * 74)
    print(f"  {len(components)} component(s) in the design")
    print(f"  {len(files_of)} of them point at a file, via an Artifact location and REALIZES")
    print(f"  {read} file(s) read; {sum(len(v) for v in edges.values())} coupling edge(s) found")
    if unsupported:
        print("  COULD NOT READ (no scanner for the language), counted not skipped:")
        for ext, paths in sorted(unsupported.items()):
            print(f"      {ext or '(no extension)'}: {len(paths)} file(s), e.g. {paths[0]}")
    if unreadable:
        print(f"  {len(unreadable)} file(s) the design names and disk does not have: {unreadable[:3]}")
    unmapped = [c for c in components if c not in files_of]
    if unmapped:
        print(f"  {len(unmapped)} component(s) point at NO file, so nothing here speaks about them:")
        print(f"      {', '.join(sorted(unmapped)[:8])}{' ...' if len(unmapped) > 8 else ''}")
    if unresolved:
        print(f"  {unresolved} import(s) resolved to no registered file — the answer is that much thinner")

    print()
    print("=" * 74)
    print("DO THE DECLARED WALLS HOLD?")
    print("=" * 74)
    by_level = defaultdict(list)
    for c in components:
        by_level[level[c]].append(c)
    for lvl in sorted(by_level, key=lambda x: len(by_level[x]), reverse=True):
        members = by_level[lvl]
        # lift the component coupling to this level through containment
        def up(c):
            seen = set()
            while c in parent and level.get(c) != lvl and c not in seen:
                seen.add(c)
                c = parent[c]
            return c if level.get(c) == lvl else None

        lifted, evidence = defaultdict(set), defaultdict(list)
        for a, targets in edges.items():
            for b in targets:
                ua, ub = up(a), up(b)
                if ua and ub and ua != ub:
                    lifted[ua].add(ub)
                    evidence[(ua, ub)].append((a, b))
        crossing = sum(len(v) for v in lifted.values())
        cycles = find_cycles(set(members), lifted)
        # "inside" only means something where a part HAS an inside. At the leaf
        # level every edge crosses by construction, and printing "0 inside"
        # there states an arithmetic certainty as if it were a measurement.
        nests = any(parent.get(c) in members for c in parent)
        if nests:
            inside = sum(1 for a, ts in edges.items() for b in ts if up(a) and up(a) == up(b))
            shape = f"{inside} edge(s) inside one, {crossing} crossing"
        else:
            shape = f"{crossing} edge(s) between them"
        print(f"\n  {lvl}: {len(members)} part(s), {shape}")
        if not any(up(a) for a in edges):
            print("      nothing at this level carries coupling — SILENT about it, not clean")
            continue
        if not cycles:
            print("      NO CYCLES — every wall at this level is crossed one way only")
        for cyc in cycles:
            print(f"      CYCLE: {' <-> '.join(cyc)}")
            for a in sorted(cyc):
                for b in sorted(lifted.get(a, ())):
                    if b in cyc:
                        ev = sorted(set(evidence[(a, b)]))
                        shown = ", ".join(f"{x}->{y}" for x, y in ev[:4])
                        print(f"         {a} -> {b}  via {len(ev)}: {shown}")

    print()
    print("=" * 74)
    print("WHAT THE DESIGN SAYS vs WHAT THE SOURCE DOES")
    print("=" * 74)
    real_pairs = {(a, b) for a, ts in edges.items() for b in ts}
    declared_pairs = {(a, b) for a, ts in declared.items() for b in ts}
    both = real_pairs & declared_pairs
    print(f"  {len(declared_pairs)} declared coupling pair(s); {len(real_pairs)} in the source; {len(both)} agree")
    undeclared = sorted(real_pairs - declared_pairs)
    print(f"  {len(undeclared)} pair(s) the SOURCE has and the design does not:")
    for a, b in undeclared[:10]:
        print(f"      {a} -> {b}")
    if len(undeclared) > 10:
        print(f"      ... and {len(undeclared) - 10} more")
    unbacked = sorted(declared_pairs - real_pairs)
    print(f"  {len(unbacked)} pair(s) the DESIGN has and the source does not — NOT defects:")
    print("      a contract can be real without an import (a process boundary, a")
    print("      file format, a human step), and this tool only reads imports.")
    for a, b in unbacked[:6]:
        print(f"      {a} -> {b}")

    print()
    print("READ THE MODULE DOCSTRING BEFORE ACTING ON ANY OF THIS. An import graph")
    print("is coupling, not a contract; none of it belongs in the graph as CONSUMES.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
