#!/usr/bin/env python3
"""The OpenCode loop-nudge plugin, driven exactly as OpenCode drives it.

WHY A TEST FOR AN ADAPTER. The plugin holds no judgement — every threshold and
counter stays in loop_nudge.py — so the only thing that can break is the
TRANSLATION, and translation failures here are silent in the worst direction:
they make the nudge say nothing. That is precisely what happened while this was
being written. The first version turned `reflow2_add_decision` into the
operation `decision`, `is_write()` did not recognise it, the write count stayed
at zero, and the stop nudge simply never fired. Nothing errored.

So this drives the four hooks the way OpenCode would and asserts the nudge
ACTUALLY ARRIVES — not that the code ran.

Skips rather than fails where node is absent: a suite that goes red because an
optional runtime is missing teaches people to ignore red.

    python3 tools/test_opencode_plugin.py
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PLUGIN = ROOT / "getting-started" / "plugins" / "reflow2-loop-nudge.js"

# OpenCode runs on Bun, which loads ESM from a .js file. Node decides by
# extension, so the harness imports a .mjs copy — the shipped file stays .js
# because that is what OpenCode's plugin directory expects.
HARNESS = r"""
import { Reflow2LoopNudge, translateToolName } from './plugin.mjs'

const dir = process.argv[2]
const hooks = await Reflow2LoopNudge({ directory: dir, worktree: dir })
const out = { hooks: Object.keys(hooks), steps: {}, names: {} }

if (!hooks['chat.message']) { console.log(JSON.stringify(out)); process.exit(0) }

const SID = 'ses_test_' + process.pid
const drain = async () => { const o = { system: [] }; await hooks['experimental.chat.system.transform']({ sessionID: SID }, o); return o.system }

await hooks['chat.message']({ sessionID: SID })
out.steps.session_start = await drain()

await hooks['tool.execute.after']({ tool: 'reflow2_add_decision', sessionID: SID, args: { id: 'dec:x' } })
out.steps.after_write = await drain()

await hooks.event({ event: { type: 'session.idle', properties: { sessionID: SID } } })
out.steps.stop = await drain()

// A loop check resets the debt, so a second stop must stay quiet.
const SID2 = 'ses_test2_' + process.pid
const drain2 = async () => { const o = { system: [] }; await hooks['experimental.chat.system.transform']({ sessionID: SID2 }, o); return o.system }
await hooks['chat.message']({ sessionID: SID2 }); await drain2()
await hooks['tool.execute.after']({ tool: 'reflow2_add_decision', sessionID: SID2, args: { id: 'dec:y' } })
await hooks['tool.execute.after']({ tool: 'reflow2_loop_status', sessionID: SID2, args: {} })
await hooks.event({ event: { type: 'session.idle', properties: { sessionID: SID2 } } })
out.steps.stop_after_loop_check = await drain2()

await hooks.event({ event: { type: 'session.next.prompted', properties: { sessionID: SID } } })
out.steps.unrelated_event = await drain()

for (const n of ['mcp__reflow2__add_decision','reflow2_add_decision','reflow2.add_decision','reflow2/loop_status','reflow2-add_capability','edit','write','apply_patch','bash'])
  out.names[n] = translateToolName(n)

console.log(JSON.stringify(out))
"""

EXPECTED_NAMES = {
    "mcp__reflow2__add_decision": "mcp__reflow2__add_decision",
    "reflow2_add_decision": "mcp__reflow2__add_decision",
    "reflow2.add_decision": "mcp__reflow2__add_decision",
    "reflow2/loop_status": "mcp__reflow2__loop_status",
    "reflow2-add_capability": "mcp__reflow2__add_capability",
    "edit": "Edit",
    "write": "Write",
    "apply_patch": "MultiEdit",
    "bash": "bash",
}

failures: list[str] = []


def check(ok: bool, what: str) -> None:
    print(("  ok   " if ok else "  FAIL ") + what)
    if not ok:
        failures.append(what)


def run(design: bool) -> dict:
    """Drive the plugin in a temp directory, with or without a design."""
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp)
        if design:
            (work / ".reflow2" / "graph").mkdir(parents=True)
        (work / "plugin.mjs").write_text(PLUGIN.read_text())
        (work / "harness.mjs").write_text(HARNESS)
        env = dict(os.environ, REFLOW2_LOOP_NUDGE=str(ROOT / "tools" / "loop_nudge.py"))
        proc = subprocess.run(
            ["node", str(work / "harness.mjs"), str(work)],
            capture_output=True, text=True, timeout=180, env=env, cwd=work,
        )
        if proc.returncode != 0:
            raise RuntimeError(f"harness failed: {proc.stderr[:400]}")
        return json.loads(proc.stdout.strip().splitlines()[-1])


def main() -> int:
    if shutil.which("node") is None:
        print("SKIP: node is not installed; the OpenCode plugin cannot be driven here.")
        return 0
    if not PLUGIN.exists():
        print(f"FAIL: plugin missing at {PLUGIN}")
        return 1

    print("the plugin registers the four hooks it needs")
    got = run(design=True)
    for hook in ("chat.message", "tool.execute.after", "event",
                 "experimental.chat.system.transform"):
        check(hook in got["hooks"], f"registers {hook}")

    print("\ntool names reach loop_nudge.py in the shape it parses")
    for name, want in EXPECTED_NAMES.items():
        check(got["names"].get(name) == want, f"{name} -> {want}")

    print("\nthe three events behave")
    start = got["steps"]["session_start"]
    check(len(start) == 1 and "reflow2" in start[0],
          "SessionStart puts the orientation line in front of the agent")
    check(got["steps"]["after_write"] == [],
          "a graph write is counted silently, with nothing said")
    stop = got["steps"]["stop"]
    check(len(stop) == 1 and "loop_status" in stop[0],
          "session.idle after an unchecked write delivers the nudge")
    check(got["steps"]["stop_after_loop_check"] == [],
          "a loop check settles the debt, so the next stop stays quiet")
    check(got["steps"]["unrelated_event"] == [],
          "an event that is not session.idle is ignored")

    print("\nsilent where there is no design")
    quiet = run(design=False)
    check(quiet["steps"].get("session_start") == []
          or quiet["steps"].get("session_start") is None,
          "a directory with no .reflow2 gets no nudge at all")

    print()
    if failures:
        print(f"{len(failures)} check(s) failed")
        return 1
    print("all checks passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
