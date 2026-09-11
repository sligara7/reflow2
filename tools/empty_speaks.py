#!/usr/bin/env python3
"""An empty answer says which empty it is — the class contract, checked on the wire.

Two passes, both on the wire because the obligation is about what a consumer
receives:

  ① NO-ARGUMENT reads on an EMPTY store — "nothing here" must say whether that
    is an all-clear or an absence.
  ② ARG-TAKING reads given a DELIBERATELY ABSENT id — "nothing found" must say
    whether the referent is missing or merely has nothing. Either a refusal
    naming the id, or an empty reply carrying the sentence, is a pass; a bare
    `{"value":null}` / `{"count":0}` / `{}` is not.

Both take the tool list from `tools/list`, so a new tool joins the class the
moment it is served rather than when somebody remembers.

Measured 2026-09-11 before any fix. Pass ①: 39 no-arg read-only tools, 10
empty on an empty design, 1 saying which empty (open_questions), 9 bare. A
bare zero from `hierarchy_issues` reads exactly like a clean design; from
`manual_work_report` exactly like nobody did work by hand — its own
description says the opposite, and the description is not the reply. Pass ②:
13 id-taking reads probed, 8 refused well, 1 spoke, 4 bare — and
`readiness_report` did something worse than bare, answering `ungated` with a
full summary about a subject that does not exist.

    python3 tools/empty_speaks.py            # exit 0 when every empty speaks
    python3 tools/empty_speaks.py --bin target/release/reflow2-mcp
"""
from __future__ import annotations
import argparse, json, os, pathlib, shutil, sys, tempfile
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from smoke_mcp import Server

SPEAKS = ("empty_because", "loop_hint")

# A STRUCTURED answer to "which empty" counts, and is better than prose.
# `propagate_from` returns `unknown_seeds` and `unclaimed_findings` returns
# `unknown_events`, each NAMING the id that resolved to nothing — which is
# exactly the fact a bare zero withholds, in a form a machine can read. A
# non-empty list here is a pass; an empty one says nothing and is not.
NAMES_THE_MISSING = ("unknown_seeds", "unknown_events", "unknown_ids")

def is_empty(v) -> bool:
    if not isinstance(v, dict):
        return False
    if v == {}:
        return True
    if v.get("count") == 0 or v.get("items") == []:
        return True
    if "value" in v and v["value"] is None:
        return True
    if "node" in v and v["node"] is None:
        return True
    return False

ABSENT = "zz:deliberately-absent"

def probe_args(schema: dict):
    """Arguments that make an arg-taking read look for something ABSENT.

    Returns None unless at least one required parameter is id-shaped — a tool
    whose required input is a path, a document or a free-text query has no
    "referent that does not exist" to probe for, and forcing one would test
    something else.
    """
    props = schema.get("properties") or {}
    args, id_shaped = {}, False
    for p in schema.get("required") or []:
        spec = props.get(p, {})
        t = spec.get("type")
        t = t if isinstance(t, str) else (t[0] if isinstance(t, list) else None)
        if (p == "id" or p.endswith("_id")) and t == "string":
            args[p], id_shaped = ABSENT, True
        elif (p.endswith("_ids") or p.endswith("_keys")) and t == "array":
            args[p], id_shaped = [ABSENT], True
        elif spec.get("enum"):
            legal = [x for x in spec["enum"] if x]
            if not legal:
                return None
            args[p] = legal[0]
        elif t == "string":
            args[p] = "x"
        elif t == "number":
            args[p] = 1.0
        elif t == "array":
            args[p] = []
        else:
            return None
    return args if id_shaped else None


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--bin", default="target/debug/reflow2-mcp")
    a = ap.parse_args()
    store = tempfile.mkdtemp(prefix="reflow2-empty-")
    s = Server(a.bin, store)
    bare, spoke, skipped = [], [], []
    try:
        listed = s.rpc("tools/list", {})["result"]["tools"]
        for t in sorted(listed, key=lambda t: t["name"]):
            ro = (t.get("annotations") or {}).get("readOnlyHint")
            req = (t.get("inputSchema") or {}).get("required") or []
            if not ro:
                continue
            if req:
                args = probe_args(t.get("inputSchema") or {})
                if args is None:
                    continue
            else:
                args = {}
            resp = s.rpc("tools/call", {"name": t["name"], "arguments": args})
            res = resp.get("result") or {}
            if "error" in resp or res.get("isError"):
                # A refusal naming the absent referent IS the answer — better
                # than an empty reply, and the pattern budget_report and
                # flow_report already set.
                spoke.append(t["name"]) if args else skipped.append((t["name"], "refused"))
                continue
            v = res.get("structuredContent")
            if v is None:
                skipped.append((t["name"], "prose")); continue
            if not is_empty(v):
                continue
            said = any(isinstance(v.get(k), str) and v[k].strip() for k in SPEAKS) or any(
                isinstance(v.get(k), list) and v[k] for k in NAMES_THE_MISSING
            )
            if said:
                spoke.append(t["name"])
            else:
                bare.append((t["name"], json.dumps(v)[:80]))
    finally:
        s.close(); shutil.rmtree(store, ignore_errors=True)
    print(f"empty on an empty design: {len(spoke) + len(bare)}   speaking: {len(spoke)}   BARE: {len(bare)}"
          f"   (skipped: {len(skipped)} refused/prose)")
    for n, b in bare:
        print(f"  BARE  {n:<28} {b}")
    if bare:
        print("\nFAIL: an empty reply must say which empty it is — `empty_because` (what was swept and "
              "why nothing came back) or the loop-debt `loop_hint`. A bare zero reads as a pass.")
        return 1
    print("OK: every empty answer says which empty it is.")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
