#!/usr/bin/env python3
"""An empty answer says which empty it is — the class contract, checked on the wire.

Starts the built binary on an EMPTY store, calls every no-argument read-only
tool by name (from `tools/list`, so a new tool joins the moment it is served),
and fails if a reply whose primary list/count/value is empty carries no
sentence saying WHY — `empty_because`, or the loop-debt `loop_hint` that
`ok_read_empty_speaks` writes. Checked on the wire, not in-process, because
the obligation is about what a consumer receives.

Measured 2026-09-11 before any fix: 39 no-arg read-only tools, 10 empty on an
empty design, 1 saying which empty (open_questions), 9 bare. A bare zero from
`hierarchy_issues` reads exactly like a clean design; from
`manual_work_report` exactly like nobody did work by hand — its own
description says the opposite, and the description is not the reply.

    python3 tools/empty_speaks.py            # exit 0 when every empty speaks
    python3 tools/empty_speaks.py --bin target/release/reflow2-mcp
"""
from __future__ import annotations
import argparse, json, os, pathlib, shutil, sys, tempfile
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from smoke_mcp import Server

SPEAKS = ("empty_because", "loop_hint")

def is_empty(v) -> bool:
    if not isinstance(v, dict):
        return False
    if v.get("count") == 0 or v.get("items") == []:
        return True
    if "value" in v and v["value"] is None:
        return True
    return False

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
            if not ro or req:
                continue
            resp = s.rpc("tools/call", {"name": t["name"], "arguments": {}})
            res = resp.get("result") or {}
            if "error" in resp or res.get("isError"):
                skipped.append((t["name"], "refused")); continue
            v = res.get("structuredContent")
            if v is None:
                skipped.append((t["name"], "prose")); continue
            if not is_empty(v):
                continue
            if any(isinstance(v.get(k), str) and v[k].strip() for k in SPEAKS):
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
