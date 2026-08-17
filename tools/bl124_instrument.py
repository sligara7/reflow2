#!/usr/bin/env python3
"""BL-124 instrument — what do acknowledgement records do to structural analysis?

An acknowledgement Decision is a statement *about* the design, wired into the
design with `GOVERNED_BY` edges to everything it acknowledges. `design_network()`
has three consumers — `unthreaded_cluster`, betweenness centrality, and
`surprising_connections` — and until BL-124 all three counted those records as
design structure.

Run it against reflow2's own committed export before and after the change and
diff the output. A number that moves is the claim; a number that does not is a
finding too, and worth recording rather than assumed.

    python3 tools/bl124_instrument.py <graph-dir> [--import docs/design/reflow2.json]
"""

from __future__ import annotations

import json
import subprocess
import sys
import os
import shutil

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from smoke_mcp import Server  # noqa: E402

BIN = "target/debug/reflow2-mcp"


def call(server: Server, name: str, args: dict):
    r = server.rpc("tools/call", {"name": name, "arguments": args})
    if "error" in r:
        return {"_error": r["error"].get("message", "")[:300]}
    txt = r["result"]["content"][0]["text"]
    try:
        return json.loads(txt)
    except Exception:
        return {"_raw": txt[:300]}


def main() -> None:
    graph_dir = sys.argv[1]
    export = "docs/design/reflow2.json"

    if os.path.exists(graph_dir):
        shutil.rmtree(graph_dir)
    subprocess.run(
        [BIN, "--graph-path", graph_dir, "--import", export],
        check=True,
        capture_output=True,
    )

    s = Server(BIN, graph_dir)

    doc = json.load(open(export))
    acks = {n["node_id"] for n in doc["nodes"] if n["node_id"].startswith("decision:ack:")}
    print(f"acknowledgement records in the graph: {len(acks)}")
    print(f"nodes {len(doc['nodes'])}  edges {len(doc['edges'])}")

    # 1. Structural defects — islands, SPOFs, and whether an ack sits inside one.
    d = call(s, "detect_defects", {})
    # {swept, defects} unscoped since 2026-08-17; the old shape was the array
    # envelope {count, items}. Both are read so this instrument keeps working
    # against an older server as well as a current one.
    items = d.get("defects", d.get("items", []))
    print(f"\n[defects] total {len(items)}  swept {d.get('swept', {}).get('nodes', '?')} node(s)")
    for it in items:
        aff = it.get("affected_ids", [])
        inside = [a for a in aff if a in acks]
        print(f"  {it['category']:<26} affected={len(aff):<4} acks_inside={len(inside)}  id={it['id']}")

    # 2. Surprising connections — does review bookkeeping rank as a surprise?
    sc = call(s, "surprising_connections", {})
    rows = sc.get("items", sc if isinstance(sc, list) else [])
    print(f"\n[surprises] returned {len(rows)}")
    for r in rows[:12]:
        f, t = r.get("from_id", "?"), r.get("to_id", "?")
        mark = "  <-- ACK" if f in acks or t in acks else ""
        print(f"  {r.get('surprise', 0):>6.2f}  {f} -> {t}{mark}")
    print(f"  ack-involving surprises in full list: "
          f"{sum(1 for r in rows if r.get('from_id') in acks or r.get('to_id') in acks)}")

    # 3. Centrality — the number propagate_change reports per impacted node.
    pr = call(s, "propagate_from", {"seed_ids": ["cmp:graph"], "max_depth": 3, "full": True})
    if "_error" in pr:
        raise SystemExit(f"propagate_from failed: {pr['_error']}")
    imp = pr.get("impacted", [])
    ranked = sorted(imp, key=lambda n: -n.get("centrality", 0))
    print(f"\n[centrality] impacted {len(imp)} from cmp:graph")
    for n in ranked[:12]:
        mark = "  <-- ACK" if n["node_id"] in acks else ""
        print(f"  {n.get('centrality', 0):.6f}  {n['node_id']}{mark}")
    print(f"  ack nodes inside this blast radius: "
          f"{sum(1 for n in imp if n['node_id'] in acks)}")

    # 4. Community structure — the number most likely to move, since Leiden
    #    sees every acknowledgement edge as design coupling.
    gr = call(s, "graph_report", {})
    alloc = gr.get("allocation", {})
    print(f"\n[communities] modularity {alloc.get('modularity')}  "
          f"components {alloc.get('component_count')}  misplaced {alloc.get('misplaced_count')}")
    sur = gr.get("surprising_connections") or gr.get("surprises") or []
    print(f"  graph_report surprises: {len(sur)}")

    s.proc.terminate()


if __name__ == "__main__":
    main()
