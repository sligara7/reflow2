#!/usr/bin/env python3
"""F1: a COPIED project must not attach to the original's server.

`cp -r` a project and `<graph>.server.json` travels with it, naming a live server
that holds the ORIGINAL design. Probing only proves *a reflow2* is listening — so
without a check on the recorded graph path, the copy's session writes its design
into somebody else's store, silently.
"""
import json, os, shutil, subprocess, sys, threading, time

BIN = sys.argv[1]
BASE = sys.argv[2]
ORIG, COPY = BASE + "/orig/.reflow2/graph", BASE + "/copy/.reflow2/graph"
fail = []

class Seat:
    def __init__(self, g, name):
        self.p = subprocess.Popen([BIN, "--graph-path", g, "--shared"], stdin=subprocess.PIPE,
                                  stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, bufsize=1)
        self.n = 0; self.lock = threading.Lock()
    def send(self, m, p=None, notify=False):
        with self.lock:
            self.n += 1
            msg = {"jsonrpc": "2.0", "method": m}
            if p is not None: msg["params"] = p
            if not notify: msg["id"] = self.n
            self.p.stdin.write(json.dumps(msg) + "\n"); self.p.stdin.flush()
            if notify: return
            line = self.p.stdout.readline()
            if not line.strip(): raise RuntimeError("channel closed")
            return json.loads(line)
    def init(self):
        self.send("initialize", {"protocolVersion": "2025-06-18", "capabilities": {},
                                 "clientInfo": {"name": "s", "version": "0"}})
        self.send("notifications/initialized", {}, notify=True)
    def tool(self, n, a): return self.send("tools/call", {"name": n, "arguments": a})

shutil.rmtree(BASE, ignore_errors=True)
os.makedirs(BASE + "/orig/.reflow2"); os.makedirs(BASE + "/copy/.reflow2")

# original: a live shared server holding a real design
a = Seat(ORIG, "orig"); a.init()
a.tool("add_requirement", {"id": "req:in_original", "name": "orig", "statement": "belongs to the original"})
rv = json.load(open(ORIG + ".server.json"))
print(f"original server: {rv['url']}  recorded graph_path={rv['graph_path']}")

# now COPY the whole .reflow2 directory — sidecars and all, exactly as cp -r does
for name in os.listdir(BASE + "/orig/.reflow2"):
    src = BASE + "/orig/.reflow2/" + name
    dst = BASE + "/copy/.reflow2/" + name
    shutil.copytree(src, dst) if os.path.isdir(src) else shutil.copy2(src, dst)
print("copied the project (including the .server.json sidecar)")
assert os.path.exists(COPY + ".server.json"), "the copy must carry the sidecar, or the test is vacuous"

b = Seat(COPY, "copy"); b.init()
b.tool("add_requirement", {"id": "req:in_copy", "name": "copy", "statement": "belongs to the copy"})

orig_now = json.dumps(a.tool("export_graph", {}))
copy_now = json.dumps(b.tool("export_graph", {}))

leaked = "req:in_copy" in orig_now
print(f"  the copy's write appears in the ORIGINAL design: {leaked}   (MUST be False)")
if leaked: fail.append("THE COPY WROTE INTO THE ORIGINAL'S STORE — F1 is not fixed")

if "req:in_copy" not in copy_now:
    fail.append("the copy's own write did not land in the copy")
else:
    print("  the copy's write is in the COPY's design: True")

# CONTROL — the harness must be capable of seeing a leak at all. The original's
# own requirement is visible to the original; if this were False the probe above
# could not have detected anything.
if "req:in_original" not in orig_now:
    fail.append("control failed: the original's own write is not visible to it")
else:
    print("  CONTROL — the original still reads its own write: True")

servers = subprocess.run(["pgrep", "-fa", "serve-shared"], capture_output=True, text=True).stdout
n = len([l for l in servers.splitlines() if BASE in l])
print(f"  distinct shared servers running: {n}   (expect 2 — one per design)")
if n != 2: fail.append(f"expected 2 servers (one per design), saw {n}")

for s in (a, b): s.p.terminate()
time.sleep(0.5)
for g in (ORIG, COPY):
    subprocess.run([BIN, "--graph-path", g, "--stop-shared"], capture_output=True)

print()
if fail:
    print("RESULT: FAILED"); [print("  -", f) for f in fail]; sys.exit(1)
print("RESULT: PASS — a copied project elects its OWN server and cannot write into the original")
