#!/usr/bin/env python3
"""Exercise reflow2 tools against an ISOLATED COPY of the design graph.

The live graph at .reflow2/graph is never opened. The copy is built by
importing the committed export into a fresh store, which is the documented
path and cannot disturb a running server.

Usage:
    python3 tools/sweep_harness.py --plan <plan.json> --out <results.jsonl>

A plan is a JSON list of {"tool": name, "args": {...}, "label": "...",
"expect": "ok"|"refusal"} steps, run in order against one server.
"""
from __future__ import annotations
import argparse, json, os, pathlib, shutil, subprocess, sys, time

ROOT = pathlib.Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "tools"))
from smoke_mcp import Server  # reuse the stdio JSON-RPC client


def build_copy(dest: pathlib.Path, export: pathlib.Path, binary: str) -> dict:
    """Fresh store, seeded from the committed export. Returns import summary."""
    if dest.exists():
        shutil.rmtree(dest)
    dest.mkdir(parents=True)
    s = Server(binary, str(dest))
    try:
        res = s.call("import_graph", {"path": str(export)})
        return res
    finally:
        s.close()


def run_plan(plan, graph_path: str, binary: str, out_path: pathlib.Path) -> dict:
    s = Server(binary, graph_path)
    tally = {"ok": 0, "refused": 0, "mismatch": 0, "crashed": 0}
    with out_path.open("w") as fh:
        for i, step in enumerate(plan):
            tool, args = step["tool"], step.get("args", {})
            expect = step.get("expect", "ok")
            rec = {"i": i, "tool": tool, "label": step.get("label", ""),
                   "expect": expect, "args_keys": sorted(args)}
            t0 = time.time()
            try:
                resp = s.rpc("tools/call", {"name": tool, "arguments": args})
                rec["ms"] = round((time.time() - t0) * 1000)
                if "error" in resp:
                    rec["outcome"] = "refused"
                    rec["detail"] = str(resp["error"])[:4000]
                else:
                    result = resp["result"]
                    if result.get("isError"):
                        rec["outcome"] = "refused"
                        rec["detail"] = str(result.get("content"))[:4000]
                    else:
                        rec["outcome"] = "ok"
                        payload = result.get("structuredContent")
                        if payload is None:
                            payload = result.get("content")
                        blob = json.dumps(payload, sort_keys=True)
                        rec["reply_chars"] = len(blob)
                        rec["reply"] = blob[:12000]
                        rec["truncated"] = len(blob) > 12000
            except Exception as e:  # server died or protocol broke
                rec["ms"] = round((time.time() - t0) * 1000)
                rec["outcome"] = "crashed"
                rec["detail"] = f"{type(e).__name__}: {e}"[:2000]
            if rec["outcome"] == "crashed":
                tally["crashed"] += 1
            elif rec["outcome"] == expect or (expect == "refusal" and rec["outcome"] == "refused"):
                tally["ok" if rec["outcome"] == "ok" else "refused"] += 1
            else:
                tally["mismatch"] += 1
                rec["MISMATCH"] = f"expected {expect}, got {rec['outcome']}"
            fh.write(json.dumps(rec) + "\n")
            fh.flush()
            if rec["outcome"] == "crashed":
                break
    s.close()
    return tally


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--plan", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--graph", default=None, help="reuse an existing copy")
    ap.add_argument("--fresh", action="store_true", help="rebuild the copy first")
    ap.add_argument("--binary", default=str(ROOT / "target/debug/reflow2-mcp"))
    ap.add_argument("--export", default=str(ROOT / "docs/design/reflow2.json"))
    a = ap.parse_args()

    graph = pathlib.Path(a.graph) if a.graph else pathlib.Path(
        os.environ.get("SWEEP_GRAPH", "/tmp/claude-1000/sweep/graph"))
    if a.fresh or not graph.exists():
        live = ROOT / ".reflow2" / "graph"
        assert graph.resolve() != live.resolve(), "refusing to target the LIVE graph"
        summary = build_copy(graph, pathlib.Path(a.export), a.binary)
        print(f"copy built at {graph}: {json.dumps(summary)[:300]}")

    plan = json.loads(pathlib.Path(a.plan).read_text())
    tally = run_plan(plan, str(graph), a.binary, pathlib.Path(a.out))
    print(f"steps={len(plan)} " + " ".join(f"{k}={v}" for k, v in tally.items()))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
