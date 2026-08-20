#!/usr/bin/env python3
"""Do the walls this design declares actually hold in the source?

`req:modularity-computed` — *modularity is computed from the design, not
asserted by the architecture diagram.* This is the instrument that computes it
for reflow2 itself, by holding the declared decomposition up against the real
Rust import graph.

⭐ THE MEASUREMENT THAT MOTIVATED IT, 2026-08-20

PR #262 made reflow2's module graph acyclic — 53 core modules and 25 MCP
modules, zero cycles at top-module and file granularity. Lift *those same
acyclic edges* up to the 8 declared subsystems, using each Component's CONTAINS
placement, and **two cycles appear**:

    sys:store        <-> sys:vocabulary
    sys:coherence-loop <-> sys:time-history

Neither is a defect in the source. The first is placement: the four-module
kernel everything sits on — `nodes` (46 of 52 modules transitively depend on
it), `provenance` (44), `schema` (43), `graph` (42) — is *cut in half* by the
declared decomposition, `nodes`/`schema` in one subsystem and `graph`/
`provenance` in the other. Any bisection of a mutually-dependent kernel
produces a cycle at the level above it whatever the code does. The second is a
single back-edge, `compare -> report`, the same shape all three of #262's
cycles had.

Why that matters rather than being a curiosity: **a wall you can hand a change
to is one that traffic crosses in a single direction.** At a mutually coupled
boundary there is nothing for a blast radius to stop at, so severability there
is not merely unmeasured — it is false.

⭐ WHY REFLOW2'S OWN CYCLE DETECTOR CANNOT FIND THIS, AND IS NOT BROKEN

reflow2 has a circular-dependency detector and it correctly reports none, because
the design holds exactly **two** coupling edges touching any of the 8
subsystems, and both are `sys:store CONSUMES` an external interface. There are
zero subsystem-to-subsystem edges, so the detector has nothing to run on. Its
green reads as *"your subsystems are acyclic"* and means *"your subsystems have
no modelled coupling"* — opposite facts wearing the same answer.

That is the general shape, and it is the reason this file reads the source
rather than the graph: every detector reflow2 ships checks the CONSISTENCY OF
EDGES THAT EXIST. A missing coupling edge is a defect of ABSENCE, and absence
is what the detector set does not ask about.

⚠️ WHAT THIS CANNOT CONCLUDE — the trap is one day older than the tool

An import graph is **coupling, not a contract.** On 2026-08-19 a mechanical
scan proposed 91 CONSUMES edges against the real graph's 17; of 33 modules said
to consume dyno-core, 95 of 95 references were TYPES (`Value` x76, `DynoError`
x18, `PropertySpec` x1) and not one was a call. The sparse hand-authored set
was right. See `fact:adopt-ran-on-reflow2-and-the-mechanical-contract-recovery-
collapsed`.

So these numbers are the right input for *"where could a wall go, and does the
one I declared hold?"* and the WRONG input for CONSUMES. **Nothing here should
be imported into the graph as a contract edge.**

Two smaller limits, stated so they are not rediscovered:

  (a) Comments and string literals are stripped before anything is read. The
      first pass of the adopt run reported a fourth cycle that was a rustdoc
      link in a comment; stripping prose removed 26 of 175 edges — 15% of the
      model — and the correction revealed a real three-hop cycle the noise had
      masked. **Never derive structure from prose.**
  (b) Components are matched to modules by NAME. A Component with no module of
      its name is reported as unmatched rather than as coupled or uncoupled,
      because those are different facts.

⚠️ THIS IS AN INSTRUMENT, NOT A GATE. It always exits 0. Whether a subsystem
cycle should STOP THE BUILD is a governance question with an owner, and the
`governance-proposal` skill exists precisely so that a tool does not answer it
by default. `tools/reflow2_check.py` is the gate.
"""

import argparse
import json
import os
import re
import sys
from collections import defaultdict

DEFAULT_CRATES = ("crates/reflow2-core", "crates/reflow2-mcp")
DEFAULT_EXPORT = "docs/design/reflow2.json"


def strip_prose(src: str) -> str:
    """Remove comments and both string-literal forms. See limit (a) above."""
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


def module_path(root: str, path: str, crate_root_name: str) -> str:
    rel = os.path.relpath(path, root)
    if rel in ("lib.rs", "main.rs"):
        # NOT "": an empty name is falsy, and an earlier draft silently dropped
        # every edge out of main.rs — which is where `degraded` and `registry`
        # are used from, so both read as uncoupled when they are not.
        return crate_root_name
    rel = rel[:-3]
    if rel.endswith("/mod"):
        rel = rel[:-4]
    return rel.replace(os.sep, "::")


def scan_crate(crate: str):
    """Module graph for one crate, file-granular. Returns (modules, edges)."""
    src_root = os.path.join(crate, "src")
    self_name = os.path.basename(crate).replace("-", "_")
    files = [
        os.path.join(dp, f)
        for dp, _, fns in os.walk(src_root)
        for f in fns
        if f.endswith(".rs")
    ]
    crate_root_name = os.path.basename(crate) + "-root"
    mods = {module_path(src_root, f, crate_root_name): f for f in files}
    known = set(mods)
    pattern = re.compile(
        r"\b(crate|super|self|" + re.escape(self_name) + r")"
        r"((?:::[A-Za-z_][A-Za-z0-9_]*)+)"
    )

    def resolve(parts):
        for k in range(len(parts), 0, -1):
            candidate = "::".join(parts[:k])
            if candidate in known:
                return candidate
        return None

    edges = defaultdict(set)
    for mod, path in sorted(mods.items()):
        text = strip_prose(open(path, encoding="utf-8", errors="replace").read())
        parent = "::".join(mod.split("::")[:-1]) if "::" in mod else ""
        for match in pattern.finditer(text):
            kind, tail = match.group(1), match.group(2)
            parts = tail.strip(":").split("::")
            if kind in ("crate", self_name):
                base = []
            elif kind == "self":
                base = mod.split("::") if mod != crate_root_name else []
            else:
                base = parent.split("::") if parent else []
            target = resolve(base + parts)
            if target is not None and target != mod:
                edges[mod].add(target)
    return known, edges


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
    """For each node, everything that transitively depends on it."""
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


def load_design(export_path):
    doc = json.load(open(export_path, encoding="utf-8"))
    node_type = {n["node_id"]: n["node_type"] for n in doc["nodes"]}
    props = {n["node_id"]: n.get("properties", {}) for n in doc["nodes"]}
    components = [i for i, t in node_type.items() if t == "Component"]
    subsystems = {c for c in components if props[c].get("level") == "subsystem"}

    parent = {}
    provides = defaultdict(set)
    design_dep = defaultdict(set)
    for e in doc["edges"]:
        a, b, t = e["from_id"], e["to_id"], e["edge_type"]
        if t == "CONTAINS" and a in subsystems and node_type.get(b) == "Component":
            parent[b] = a
        elif t == "PROVIDES" and node_type.get(a) == "Component":
            provides[b].add(a)
        elif t == "DEPENDS_ON" and node_type.get(a) == node_type.get(b) == "Component":
            design_dep[a].add(b)
    for e in doc["edges"]:
        if e["edge_type"] == "CONSUMES" and node_type.get(e["from_id"]) == "Component":
            for p in provides.get(e["to_id"], ()):
                if p != e["from_id"]:
                    design_dep[e["from_id"]].add(p)
    return components, subsystems, parent, design_dep


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--export", default=DEFAULT_EXPORT, help="committed design export")
    ap.add_argument("--crate", action="append", dest="crates", help="repeatable")
    args = ap.parse_args()
    crates = args.crates or list(DEFAULT_CRATES)

    # --- the source side -------------------------------------------------
    code_edges = set()
    per_crate = []
    for crate in crates:
        mods, edges = scan_crate(crate)
        per_crate.append((crate, mods, edges))
        for a, targets in edges.items():
            for b in targets:
                # leaf name, so `tools::query` and a cmp:query line up
                la, lb = a.split("::")[-1], b.split("::")[-1]
                if la != lb:
                    code_edges.add((la, lb))
    code_degree = defaultdict(int)
    for a, b in code_edges:
        code_degree[a] += 1
        code_degree[b] += 1

    print("=" * 74)
    print("MODULE GRAPH — imports only, comments and string literals stripped")
    print("=" * 74)
    for crate, mods, edges in per_crate:
        total = sum(len(v) for v in edges.values())
        cycles = find_cycles(mods, edges)
        up = reverse_reach(mods, edges)
        widest = sorted(mods, key=lambda m: -len(up[m]))[:5]
        print(f"  {crate}: {len(mods)} modules, {total} edges, {len(cycles)} cycle(s)")
        for c in cycles:
            print(f"      CYCLE: {' <-> '.join(c)}")
        print(
            "      kernel (widest blast radius): "
            + ", ".join(f"{m} {len(up[m])}" for m in widest)
        )

    # --- the declared side ------------------------------------------------
    if not os.path.exists(args.export):
        print(f"\nno design export at {args.export} — source half only.")
        return 0
    components, subsystems, parent, design_dep = load_design(args.export)
    print()
    print("=" * 74)
    print("DO THE DECLARED WALLS HOLD? — subsystem graph induced by real imports")
    print("=" * 74)
    mod_to_sub = {c.split(":", 1)[1]: s for c, s in parent.items()}
    sub_edges = defaultdict(set)
    evidence = defaultdict(list)
    inside = crossing = 0
    for a, b in code_edges:
        sa, sb = mod_to_sub.get(a), mod_to_sub.get(b)
        if sa is None or sb is None:
            continue
        if sa == sb:
            inside += 1
        else:
            crossing += 1
            sub_edges[sa].add(sb)
            evidence[(sa, sb)].append((a, b))
    print(f"  {len(subsystems)} subsystem(s); {len(parent)} component(s) placed in one")
    print(f"  code edges with both ends placed: {inside} inside, {crossing} crossing")
    print("  (crossing is not itself a fault — a foundation layer is SUPPOSED to")
    print("   be depended on. Only a TWO-WAY crossing denies you a wall.)")
    sub_cycles = find_cycles(subsystems, sub_edges)
    print()
    if not sub_cycles:
        print("  NO SUBSYSTEM CYCLES — every declared wall is crossed one way only.")
    else:
        print(f"  {len(sub_cycles)} SUBSYSTEM CYCLE(S) — these walls cannot be severed:")
    for cycle in sub_cycles:
        print(f"    CYCLE: {' <-> '.join(cycle)}")
        for a in cycle:
            for b in sorted(sub_edges.get(a, ())):
                if b in cycle:
                    pairs = sorted(set(evidence[(a, b)]))
                    shown = ", ".join(f"{x}->{y}" for x, y in pairs[:5])
                    tail = " ..." if len(pairs) > 5 else ""
                    print(f"      {a} -> {b}  via {len(pairs)}: {shown}{tail}")

    # --- absence: a zero the source contradicts ---------------------------
    print()
    print("=" * 74)
    print("ZEROES THE SOURCE CONTRADICTS — a false 'nothing depends on me'")
    print("=" * 74)
    design_degree = defaultdict(int)
    for a, targets in design_dep.items():
        design_degree[a] += len(targets)
        for b in targets:
            design_degree[b] += 1
    contradicted, unmatched = [], []
    for c in sorted(components):
        if design_degree[c]:
            continue
        leaf = c.split(":", 1)[1]
        if leaf in code_degree and code_degree[leaf]:
            contradicted.append((c, code_degree[leaf]))
        elif leaf not in code_degree:
            unmatched.append(c)
    print(f"  {len(contradicted)} component(s) with NO coupling edge in the design")
    print("  whose same-named module IS coupled in the source:")
    for c, deg in sorted(contradicted, key=lambda x: -x[1]):
        print(f"      {c:<34} design degree 0, code degree {deg}")
    print()
    print(f"  {len(unmatched)} uncoupled component(s) name no module — not a verdict,")
    print("  a different fact: " + ", ".join(unmatched[:8]) + (" ..." if len(unmatched) > 8 else ""))
    unplaced = sorted(set(components) - set(parent) - subsystems)
    if unplaced:
        print()
        print(f"  {len(unplaced)} component(s) in NO subsystem at all:")
        print("      " + ", ".join(unplaced))

    print()
    print("READ THE MODULE DOCSTRING BEFORE ACTING ON ANY OF THIS. An import graph")
    print("is coupling, not a contract; none of it belongs in the graph as CONSUMES.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
