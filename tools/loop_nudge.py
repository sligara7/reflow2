#!/usr/bin/env python3
"""reflow2 loop nudge — the trigger half of the coherence loop (BL-74 rung a).

The field lesson this exists for: told to "use reflow2 extensively", an agent
under operational load kept the graph's *bookkeeping* current through the raw
write tools while the capture→detect→ask→decide loop silently stopped — and
"under load, a mood loses to whatever has a trigger." The in-band halves are
already built: write results carry a `loop_hint`, and `loop_status` is the one
cheap call that says what the loop is owed. This script is the out-of-band
trigger that fires them, wired to the harness's own events (Claude Code hooks;
the snippet lives in the kit's AGENTS.md, step 0a).

One script, three events, read from the hook's stdin JSON:

- **SessionStart** — prints the orientation line into the session's context:
  orient on the graph first, and `loop_status` is the in-flight pulse-check.
- **PostToolUse** — counts reflow2 graph *writes* per session; a loop check
  (`loop_status` / `detect_gaps` / `detect_defects`) resets the count.
  Resolve steps (answering, acknowledging, dispositioning drift) are loop
  participation: neither debt nor a full check — ignored.
- **Stop** — the backstop: if the session wrote to the graph and no loop check
  ever ran, block the stop ONCE with the reason the agent needs. A second stop
  (`stop_hook_active`) always proceeds — a nudge that can loop forever is a
  hostage-taker, not a trigger.

Deliberately does NOT read the graph: the session's own MCP server holds the
single-writer lock, and the committed export can be a session stale. The hook
counts events and points at `loop_status`; the *graph* answers what is owed.
State is one small JSON per session under `.reflow2/loop-nudge/` (gitignored
with the rest of `.reflow2/`). A hook must never break a session: any failure
here warns on stderr and exits 0.

Stdlib only, no arguments needed. `REFLOW2_LOOP_NUDGE_THRESHOLD` (default 1)
sets how many uncheckd writes it takes for the Stop backstop to fire.
"""

from __future__ import annotations

import json
import os
import sys
import time
from pathlib import Path

# Ops that ARE the loop check — seeing one clears the debt counter.
LOOP_OPS = {"loop_status", "detect_gaps", "detect_defects"}

# Graph writes beyond the add_/create_/delete_ prefixes. Unknown ops fall
# through to "ignored" — this is a backstop, not an accountant, and a missed
# count only softens the nudge, never wrongs the user.
EXTRA_WRITE_OPS = {
    "allocate", "consumes", "contain_component", "contains", "deploy_to",
    "genesis", "import_graph", "link_artifact", "part_of_flow", "pin_at_epoch",
    "precedes", "provides", "record_change", "release_includes",
    "require_resource", "satisfies", "set_capability_status",
    "set_provenance", "set_requirement_status", "set_verification_status",
    "verifies",
}

SESSION_START_TEXT = (
    "reflow2: this project has a design graph. Orient first — open_questions, "
    "then the where-am-i skill — before touching code. While you work, "
    "loop_status is the one cheap call that says what the coherence loop is "
    "owed; the Stop hook will nudge if graph writes finish without one."
)


def state_dir() -> Path:
    return Path(".reflow2") / "loop-nudge"


def state_file(session_id: str) -> Path:
    safe = "".join(c if c.isalnum() or c in "-_" else "_" for c in session_id)
    return state_dir() / f"{safe or 'unknown'}.json"


def read_writes(session_id: str) -> int:
    try:
        return int(json.loads(state_file(session_id).read_text())["writes"])
    except (OSError, ValueError, KeyError, TypeError):
        return 0


def write_writes(session_id: str, count: int) -> None:
    d = state_dir()
    d.mkdir(parents=True, exist_ok=True)
    state_file(session_id).write_text(json.dumps({"writes": count}))
    # Opportunistic tidy-up: session files a week old are dead sessions.
    cutoff = time.time() - 7 * 24 * 3600
    for old in d.glob("*.json"):
        try:
            if old.stat().st_mtime < cutoff:
                old.unlink()
        except OSError:
            pass


def is_write(op: str) -> bool:
    return op.startswith(("add_", "create_", "delete_")) or op in EXTRA_WRITE_OPS


def main() -> int:
    try:
        event = json.load(sys.stdin)
    except (ValueError, OSError):
        return 0
    if not isinstance(event, dict):
        return 0
    kind = event.get("hook_event_name", "")
    session = str(event.get("session_id") or "unknown")

    if kind == "SessionStart":
        print(SESSION_START_TEXT)
        return 0

    if kind == "PostToolUse":
        tool = str(event.get("tool_name") or "")
        # Only this project's reflow2 server; the op is the last __ segment.
        if "reflow2" not in tool or "__" not in tool:
            return 0
        op = tool.rsplit("__", 1)[-1]
        if op in LOOP_OPS:
            write_writes(session, 0)
        elif is_write(op):
            write_writes(session, read_writes(session) + 1)
        return 0

    if kind == "Stop":
        if event.get("stop_hook_active"):
            return 0  # already nudged once — never hold the session hostage
        try:
            threshold = max(1, int(os.environ.get("REFLOW2_LOOP_NUDGE_THRESHOLD", "1")))
        except ValueError:
            threshold = 1
        n = read_writes(session)
        if n >= threshold:
            print(json.dumps({
                "decision": "block",
                "reason": (
                    f"reflow2: {n} graph write(s) this session and no loop check. "
                    f"Call loop_status — if its `next` list names debt, run "
                    f"detect-and-ask / check-health before finishing. Bookkeeping "
                    f"is not the loop. (This nudge fires once; stopping again "
                    f"proceeds.)"
                ),
            }))
        return 0

    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as e:  # noqa: BLE001 — a hook must never break a session
        print(f"loop_nudge: skipped ({e})", file=sys.stderr)
        sys.exit(0)
