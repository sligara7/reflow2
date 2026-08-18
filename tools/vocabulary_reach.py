#!/usr/bin/env python3
"""Which of the design vocabulary can nothing actually write?

`req:declared-vocabulary-is-reachable-from-the-surface`. The failure this
measures happened FOUR TIMES IN TWO DAYS and was found by hand every time:

  Verification.description   declared, fulltext, the embedding field — used ONCE
                             in 164 nodes, because `add_verification` had no
                             parameter for it and `name` was the only string on
                             offer. Read as "nobody wants descriptions"; it was
                             unreachable, not unwanted.
  SUPERSEDES                 declared as an edge type, ZERO edges, while nine
                             nodes named their successor in prose instead.
  create_node's CAS          shipped demanding a precondition value it did not
                             return. Every core test passed.
  GOVERNED_BY.ruling         would have been reachable only through raw
                             create_edge; caught only because the first had
                             happened hours earlier.

⭐ WHY THIS READS THE CORPUS AND NOT THE SOURCE

The obvious instrument compares declared property names against served tool
parameter names. That was tried first and it OVERCOUNTS BADLY: it reported 70
of 215 properties (32%) unreachable, including `TemporalFact.valid_from`, which
is perfectly writable. A name match cannot see a tool that sets a property
under a different parameter name, or one written indirectly by `record_change`,
`reconcile_artifacts` or `link_artifact`.

So the primary signal here is USAGE IN A REAL DESIGN: how many nodes of a type
actually carry each declared property. A property carried by ZERO nodes across
a mature graph is evidence, not inference — and it is exactly how three of the
four above were spotted. The name match is still computed and reported, but as
a SECOND column and explicitly as a hint.

⚠️ WHAT THIS CANNOT CONCLUDE, and the requirement says so in as many words:

  (a) An unreachable field must NOT be read as one to delete. Three of the four
      above were WANTED and got a parameter.
  (b) Raw `create_node` / `create_edge` must NOT count as reachability. They
      accept an arbitrary property bag, which is precisely why they hid all
      four — a check computed against the generic escape hatch answers clean
      forever. They are excluded from the tool-parameter column below.
  (c) Zero usage is not proof of unreachability. A property can be writable and
      simply never have been needed. The output separates "zero AND no tool
      parameter" (the candidates) from "zero but a tool accepts it" (silent,
      and probably just unused).

Reads the committed export and the committed toolsnaps; opens no store and
takes no lock. stdlib + PyYAML only.
"""

from __future__ import annotations

import collections
import json
import pathlib
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent
EXPORT = REPO / "docs/design/reflow2.json"
SCHEMA_DIR = REPO / "schema"
TOOLSNAPS = REPO / "tools/toolsnaps"

# The generic escape hatches. Excluded deliberately — see (b) above.
GENERIC = {"create_node", "create_nodes", "create_edge", "create_edges"}


def declared() -> tuple[dict[str, dict], dict[str, dict]]:
    """Every declared node property and edge property, from schema/*.yaml."""
    try:
        import yaml
    except ImportError:
        print("PyYAML required", file=sys.stderr)
        raise SystemExit(2)
    nodes: dict[str, dict] = {}
    edges: dict[str, dict] = {}
    for f in sorted(SCHEMA_DIR.glob("*.yaml")):
        doc = (yaml.safe_load(f.read_text()) or {}).get("schema") or {}
        for name, spec in (doc.get("node_types") or {}).items():
            nodes.setdefault(name, {}).update((spec or {}).get("properties") or {})
        for name, spec in (doc.get("edge_types") or {}).items():
            edges.setdefault(name, {}).update((spec or {}).get("properties") or {})
    return nodes, edges


def tool_parameters() -> set[str]:
    """Every parameter name a NON-GENERIC served tool accepts."""
    params: set[str] = set()
    for f in sorted(TOOLSNAPS.glob("*.json")):
        if f.stem in GENERIC:
            continue
        snap = json.loads(f.read_text())
        params |= set(((snap.get("inputSchema") or {}).get("properties") or {}).keys())
    return params


def main() -> int:
    node_props, edge_props = declared()
    params = tool_parameters()
    doc = json.loads(EXPORT.read_text())

    # Corpus usage.
    per_type: dict[str, collections.Counter] = collections.defaultdict(collections.Counter)
    type_counts: collections.Counter = collections.Counter()
    for n in doc["nodes"]:
        t = n["node_type"]
        type_counts[t] += 1
        for k in n.get("properties", {}):
            per_type[t][k] += 1
    edge_used: collections.Counter = collections.Counter()
    edge_prop_used: dict[str, collections.Counter] = collections.defaultdict(collections.Counter)
    for e in doc["edges"]:
        edge_used[e["edge_type"]] += 1
        for k in e.get("properties", {}) or {}:
            edge_prop_used[e["edge_type"]][k] += 1

    unreachable, unused_but_offered, no_instances, edge_dead = [], [], [], []
    total_props = 0
    for t, props in sorted(node_props.items()):
        for p in sorted(props):
            total_props += 1
            used = per_type[t][p]
            if used:
                continue
            if type_counts[t] == 0:
                # ⭐ "0 of 0" IS A VACUOUS ZERO and this bucket exists because
                # the first run of this instrument did not have it: six node
                # types have no instances at all, and every property on them
                # landed in the candidate list looking like evidence. A design
                # that has never created an EnvironmentRule says NOTHING about
                # whether EnvironmentRule.authority is writable. Reporting them
                # together would have been this tool committing the exact
                # defect its own epoch is named after.
                no_instances.append((t, p, 0))
            elif p not in params:
                unreachable.append((t, p, type_counts[t]))
            else:
                unused_but_offered.append((t, p, type_counts[t]))
    for et in sorted(edge_props):
        if edge_used[et] == 0:
            edge_dead.append((et, "the EDGE TYPE itself", 0))
        else:
            for p in sorted(edge_props[et]):
                if edge_prop_used[et][p] == 0:
                    edge_dead.append((et, p, edge_used[et]))

    print(f"declared node properties : {total_props} across {len(node_props)} types")
    print(f"declared edge types      : {len(edge_props)}")
    print(f"non-generic tool params  : {len(params)}  (create_node/create_edge excluded)")
    print()
    print("=" * 74)
    print(f"CANDIDATES — the type IS used, the property NEVER is, and no typed tool")
    print(f"accepts the name: {len(unreachable)}")
    print("=" * 74)
    for t, p, n in unreachable:
        print(f"  {t}.{p}".ljust(52) + f"(0 of {n} {t} nodes)")
    print()
    print("-" * 74)
    print(f"UNUSED BUT OFFERED — zero uses, but a typed tool does accept the name: {len(unused_but_offered)}")
    print("  Probably genuinely unused rather than unreachable. Reported so the")
    print("  count above cannot be quoted as 'everything the design cannot write'.")
    print("-" * 74)
    for t, p, n in unused_but_offered:
        print(f"  {t}.{p}".ljust(52) + f"(0 of {n})")
    print()
    print("-" * 74)
    print(f"SAYS NOTHING — the node type has NO INSTANCES, so a zero on its")
    print(f"properties is vacuous rather than evidence: {len(no_instances)}")
    print("  Across " + ", ".join(sorted({t for t, _, _ in no_instances})) + ".")
    print("-" * 74)
    print()
    print("-" * 74)
    print(f"EDGE VOCABULARY WITH NO INSTANCES: {len(edge_dead)}")
    print("-" * 74)
    for et, what, n in edge_dead:
        print(f"  {et}.{what}".ljust(52) + f"({n} edges of this type exist)")
    print()
    print("READ THE MODULE DOCSTRING BEFORE ACTING ON ANY OF THIS: zero usage is")
    print("evidence, not proof, and an unreachable field is never automatically")
    print("one to delete.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
