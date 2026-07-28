#!/usr/bin/env python3
"""The three cases that decide whether shared mode is safe as the DEFAULT.

1. Simultaneous cold start — several sessions racing to elect a server.
2. A session dying must not disturb its peers (no session is the holder).
3. The server dying must not strand attached sessions (self-heal).
"""
import json, os, signal, subprocess, sys, threading, time

BIN, G = sys.argv[1], sys.argv[2]
fail = []

class Seat:
    def __init__(self, name):
        self.name = name
        self.p = subprocess.Popen([BIN, "--graph-path", G, "--shared"],
                                  stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                  stderr=subprocess.PIPE, text=True, bufsize=1)
        self.n = 0; self.lock = threading.Lock()
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
                raise RuntimeError(f"{self.name}: channel closed")
            return json.loads(line)
    def init(self):
        r = self.send("initialize", {"protocolVersion": "2025-06-18", "capabilities": {},
                                     "clientInfo": {"name": self.name, "version": "0"}})
        self.send("notifications/initialized", {}, notify=True)
        return r
    def tool(self, n, a): return self.send("tools/call", {"name": n, "arguments": a})

def rendezvous():
    try:
        with open(G + ".server.json") as f: return json.load(f)
    except Exception: return None

# ---------------------------------------------------------------- 1. cold race
print("=== 1. SIMULTANEOUS COLD START — 4 sessions, no server, all at once ===")
seats, results = [], {}
def start(i):
    try:
        s = Seat(f"race-{i}"); seats.append(s)
        s.init()
        # H1 (w-74c2989e): `serverInfo` is ALSO returned by the degraded surface,
        # so asserting on it means "a process is alive", not "this session has a
        # design". It reported 4/4 up in a run where all four were degraded and
        # the design was unreachable. What separates them is the tool surface.
        names = [t["name"] for t in s.send("tools/list", {})["result"]["tools"]]
        results[i] = (len(names) >= 50 and "reflow2_unavailable" not in names)
    except Exception as e:
        results[i] = f"EXC {e!r}"
ths = [threading.Thread(target=start, args=(i,)) for i in range(4)]
t0 = time.time(); [t.start() for t in ths]; [t.join() for t in ths]
print(f"   all 4 initialized in {time.time()-t0:.1f}s -> {results}")
if not all(v is True for v in results.values()):
    fail.append(f"cold race: not every session came up: {results}")

# Exactly ONE server may exist. Several daemons were spawned; the store lock is
# the arbiter, so all but one must have exited.
time.sleep(1)
procs = subprocess.run(["pgrep", "-fa", "serve-shared"], capture_output=True, text=True).stdout
live = [l for l in procs.splitlines() if G in l]
print(f"   shared servers alive for this graph: {len(live)}   (must be exactly 1)")
if len(live) != 1:
    fail.append(f"cold race elected {len(live)} servers, not 1:\n" + "\n".join(live))

rv = rendezvous()
print(f"   rendezvous published: {rv['url'] if rv else None} (pid {rv['pid'] if rv else '-'})")
if not rv: fail.append("cold race published no rendezvous")

# and they must all share ONE design
seats[0].tool("add_requirement", {"id": "req:race", "name": "race", "statement": "one design"})
seen = sum(1 for s in seats if "req:race" in json.dumps(s.tool("export_graph", {})))
print(f"   sessions seeing the same design: {seen}/4")
if seen != 4: fail.append(f"cold race: only {seen}/4 shared one design")

# --------------------------------------------------- 2. a session dying is fine
print("\n=== 2. A SESSION DIES — peers must be undisturbed ===")
victim = seats[0]
victim.p.kill(); victim.p.wait()
time.sleep(1)
survivors = 0
for s in seats[1:]:
    try:
        rid = f"req:after_death_{s.name}"
        s.tool("add_requirement", {"id": rid, "name": "x", "statement": "peer still works"})
        # H2 (w-74c2989e): counting "the call did not raise" made this step
        # FULLY vacuous — it printed 3/3 in a run with no server, no design and
        # no write that could possibly have landed. Only a read-back proves it.
        if rid in json.dumps(s.tool("export_graph", {})):
            survivors += 1
        else:
            fail.append(f"{s.name}: write reported no error but is NOT in the design")
    except Exception as e:
        fail.append(f"{s.name} broke when a PEER session died: {e!r}")
print(f"   peers still writing after a session was killed: {survivors}/3")
if survivors != 3: fail.append("killing one session disturbed its peers")

# ------------------------------------------- 3. the server dying must self-heal
print("\n=== 3. THE SERVER DIES — attached sessions must recover ===")
rv = rendezvous()
if not rv:
    # H3 (w-74c2989e): this used to be `rv["pid"]` on None — a TypeError
    # traceback, exit 1 but with RESULT/fail-list never printed. The verdict
    # belongs on the last line, not a Python error about NoneType.
    fail.append("no rendezvous to kill — skipping step 3 (an earlier step already failed)")
    rv = None
else:
    os.kill(rv["pid"], signal.SIGKILL)   # SIGKILL: no cleanup, worst case, stale record left behind
if rv:
    print(f"   SIGKILLed the shared server (pid {rv['pid']}) — stale rendezvous deliberately left in place")
time.sleep(1)
s = seats[1]
try:
    if not rv: raise RuntimeError("step 3 skipped: no server existed to kill")
    out = json.dumps(s.tool("add_requirement", {"id": "req:after_server_death", "name": "heal",
                                                "statement": "written after the server was killed"}))
    healed = '"error"' not in out
    print(f"   next tool call on a live session: {'RECOVERED' if healed else 'FAILED'}")
    if not healed:
        fail.append(f"session did not recover from server death: {out[:300]}")
    else:
        # and the write must really be in the design, not merely not-an-error
        back = json.dumps(s.tool("export_graph", {}))
        if "req:after_server_death" not in back:
            fail.append("post-recovery write reported success but is not in the design")
        else:
            print("   the post-recovery write is really in the design (read back)")
        if "req:race" not in back:
            fail.append("the replacement server lost the design written before the crash")
        else:
            print("   the design written BEFORE the crash survived the restart")
except Exception as e:
    fail.append(f"session raised on server death instead of recovering: {e!r}")

for s in seats:
    try: s.p.terminate()
    except Exception: pass

print()
if fail:
    print("RESULT: FAILED"); [print("  -", f) for f in fail]; sys.exit(1)
print("RESULT: PASS — election is single-winner, sessions are independent, and a dead server self-heals")
