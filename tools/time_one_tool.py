import os, sys, time, json, statistics
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from smoke_mcp import Server
binary, graph, tool = sys.argv[1], sys.argv[2], sys.argv[3]
args = json.loads(sys.argv[4]) if len(sys.argv) > 4 else {}
s = Server(binary, graph); ts = []
for _ in range(3):
    t = time.perf_counter(); r = s.rpc("tools/call", {"name": tool, "arguments": args}); ts.append(time.perf_counter() - t)
sc = r.get("result", {}).get("structuredContent") or {}
extra = f"  sync entries={len(sc.get('sync', []))}" if tool == "sync_status" else ""
print(f"{tool:14} {graph.split('/')[-1]:12} cold {ts[0]*1000:8.0f} ms  warm {statistics.median(ts[1:])*1000:8.0f} ms{extra}")
s.proc.terminate()
