#!/usr/bin/env python3
"""Does a FRESH session get a working shared design with zero setup?

Drives `reflow2-mcp --shared` exactly as an MCP client does — spawn, speak
newline-delimited JSON-RPC on stdio — because the claim under test is about the
DEFAULT PATH, not about a server an experienced operator stood up by hand.
"""
import json, subprocess, sys, threading, time

BIN, G = sys.argv[1], sys.argv[2]

class Seat:
    def __init__(self, name):
        self.name = name
        self.p = subprocess.Popen([BIN, "--graph-path", G, "--shared"],
                                  stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                  stderr=subprocess.PIPE, text=True, bufsize=1)
        self.n = 0
        self.lock = threading.Lock()
    def send(self, method, params=None, notify=False):
        with self.lock:
            self.n += 1
            m = {"jsonrpc": "2.0", "method": method}
            if params is not None: m["params"] = params
            if not notify: m["id"] = self.n
            self.p.stdin.write(json.dumps(m) + "\n"); self.p.stdin.flush()
            if notify: return None
            line = self.p.stdout.readline()
            if not line.strip():
                raise RuntimeError(f"{self.name}: server closed the channel; stderr:\n" +
                                   self.p.stderr.read()[-2000:])
            return json.loads(line)
    def init(self):
        r = self.send("initialize", {"protocolVersion": "2025-06-18", "capabilities": {},
                                     "clientInfo": {"name": self.name, "version": "0"}})
        self.send("notifications/initialized", {}, notify=True)
        return r
    def tool(self, n, a): return self.send("tools/call", {"name": n, "arguments": a})

fail = []
t0 = time.time()
a = Seat("seat-A")
r = a.init()
info = (r or {}).get("result", {}).get("serverInfo")
print(f"seat-A initialize -> {info}   ({time.time()-t0:.1f}s, COLD: it had to start the server)")
if not info: fail.append("seat-A got no serverInfo")

names = [t["name"] for t in a.send("tools/list", {})["result"]["tools"]]
print(f"seat-A tool count: {len(names)}")
if len(names) < 50: fail.append(f"seat-A saw only {len(names)} tools")
if "reflow2_unavailable" in names:
    fail.append("seat-A got the DEGRADED surface, not the design")

t1 = time.time()
b, c = Seat("seat-B"), Seat("seat-C")
b.init(); c.init()
print(f"seat-B and seat-C attached to the SAME server ({time.time()-t1:.1f}s, warm)")

errs = []
def write(seat, who):
    try:
        txt = json.dumps(seat.tool("add_requirement",
              {"id": f"req:{who}", "name": f"from {who}", "statement": f"written by {who}"}))
        if '"error"' in txt or "failed to" in txt:
            errs.append((who, txt[:200]))
    except Exception as e:
        errs.append((who, repr(e)))

ths = [threading.Thread(target=write, args=(s, w))
       for s, w in [(a, "alpha"), (b, "bravo"), (c, "charlie")]]
[t.start() for t in ths]; [t.join() for t in ths]
print("concurrent write failures:", errs if errs else "NONE")
if errs: fail.append(f"concurrent writes failed: {errs}")

for seat, who in [(a, "A"), (b, "B"), (c, "C")]:
    txt = json.dumps(seat.tool("export_graph", {}))
    seen = [x for x in ("req:alpha", "req:bravo", "req:charlie") if x in txt]
    print(f"  seat-{who} reads {len(seen)}/3 peer writes -> {seen}")
    if len(seen) != 3: fail.append(f"seat-{who} saw only {seen}")

# CONTROL — the same read must be CAPABLE of not finding something. Without this
# a read that returned everything-shaped garbage would score 3/3.
ctrl = "req:nobody_ever_wrote_this" in json.dumps(a.tool("export_graph", {}))
print(f"  CONTROL — an id nobody wrote is absent: {not ctrl}")
if ctrl: fail.append("control failed: the read finds ids that were never written")

for s in (a, b, c):
    s.p.terminate()

print()
if fail:
    print("RESULT: FAILED")
    for f in fail: print("  -", f)
    sys.exit(1)
print("RESULT: PASS — three fresh sessions, zero setup, shared read+write")
