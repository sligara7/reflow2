#!/usr/bin/env python3
"""Profile the tools an agent actually drives: wall-clock and reply bytes per call.

Usage: agent_surface_profile.py <binary> <graph_path> <label> [--runs N]

Speaks stdio JSON-RPC via tools/smoke_mcp.py's Server, so it measures the
product surface (one process, one store, direct transport) rather than the
shared daemon. Reports MEDIAN of N runs; the first call of each tool is
recorded separately as 'cold'. Bytes = len(json.dumps(structuredContent)).
"""
import json, os, statistics, sys, time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from smoke_mcp import Server  # noqa: E402

binary, graph, label = sys.argv[1], sys.argv[2], sys.argv[3]
runs = int(sys.argv[sys.argv.index("--runs") + 1]) if "--runs" in sys.argv else 3

# The set the usage instrument ranked: top by volume and by PageRank, plus the
# two orientation calls every session is told to make. Read-only where a
# write would change the store under later runs.
CALLS = [
    ("loop_status",      {}),
    ("search_design",    {"query": "persistence store durability", "limit": 10}),
    ("get_node",         {"node_type": "Capability", "id": "cap:store"}),
    ("detect_gaps",      {}),
    ("open_questions",   {}),
    ("what_next",        {}),
    ("detect_defects",   {}),
    ("claim_report",     {}),
    ("describe_schema",  {"node_type": "Decision"}),
    ("graph_report",     {}),
]

t0 = time.perf_counter()
s = Server(binary, graph)
startup = time.perf_counter() - t0

print(f"\n=== AGENT SURFACE PROFILE — {label} ===", flush=True)
print(f"binary : {binary}\ngraph  : {graph}", flush=True)
print(f"startup to handshake : {startup*1000:8.1f} ms", flush=True)
print(f"{'tool':18} {'cold ms':>9} {'warm ms':>9} {'reply bytes':>12}  err", flush=True)
rows = []
for tool, args in CALLS:
    times, size = [], None
    for i in range(runs):
        t = time.perf_counter()
        try:
            resp = s.rpc("tools/call", {"name": tool, "arguments": args})
        except SystemExit as e:
            print(f"{tool}: server died: {e}", file=sys.stderr); sys.exit(2)
        dt = time.perf_counter() - t
        times.append(dt)
        if size is None:
            r = resp.get("result", {})
            sc = r.get("structuredContent")
            content = r.get("content", [])
            size = len(json.dumps(sc)) if sc is not None else sum(len(c.get("text", "")) for c in content)
            err = bool(r.get("isError")) or "error" in resp
    rows.append((tool, times[0], statistics.median(times[1:]) if runs > 1 else times[0], size, err))
    _t, _c, _w, _s, _e = rows[-1]
    print(f"{_t:18} {_c*1000:9.1f} {_w*1000:9.1f} {_s:12,}  {'ERR' if _e else ''}", flush=True)

tot_warm = sum(r[2] for r in rows); tot_bytes = sum(r[3] for r in rows)
print(f"{'TOTAL (warm)':18} {'':>9} {tot_warm*1000:9.1f} {tot_bytes:12,}")
print("=== END ===")
s.proc.terminate()
