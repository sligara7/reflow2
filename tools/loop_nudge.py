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
- **PostToolUse** — two things it counts per session, from the tool name:
  - reflow2 graph *writes* — a loop check (`loop_status` / `detect_gaps` /
    `detect_defects`) resets the count. Resolve steps (answering, acknowledging,
    dispositioning drift) are loop participation: neither debt nor a full check
    — ignored. *Any* reflow2 call, even a read, marks the session as having
    engaged the design brain at all.
  - harness file *edits* (`Edit` / `Write` / `MultiEdit` / `NotebookEdit`) —
    counted only to catch the session that edits code while making **zero**
    reflow2 calls: the total-bypass blind spot (BL-90). This is upstream of the
    write-nudge — the agent that ignores the design brain entirely is exactly
    the one the write count never sees.
- **Stop** — the backstop, blocking ONCE with the reason the agent needs:
  - graph writes finished with no loop check → "call loop_status", or
  - the session never touched reflow2 at all and edited enough files → "the
    graph was never consulted; start with loop_status, impact-check before
    further edits, link-artifacts after".
  A second stop (`stop_hook_active`) always proceeds — a nudge that can loop
  forever is a hostage-taker, not a trigger. The two cases are mutually
  exclusive: any graph write means reflow2 was touched, so the bypass case
  cannot also be armed.

Deliberately does NOT read the graph: the session's own MCP server holds the
single-writer lock, and the committed export can be a session stale. The hook
counts events and points at `loop_status`; the *graph* answers what is owed —
which also means the hook cannot know which edited files are design-relevant, so
the bypass backstop stays blunt (a count threshold, once-only) on purpose.
State is one small JSON per session under `.reflow2/loop-nudge/` (gitignored
with the rest of `.reflow2/`). A hook must never break a session: any failure
here warns on stderr and exits 0.

Stdlib only, no arguments needed. Two thresholds, both env-tunable:
`REFLOW2_LOOP_NUDGE_THRESHOLD` (default 1) — unchecked graph writes before the
Stop backstop fires; `REFLOW2_LOOP_NUDGE_EDIT_THRESHOLD` (default 3) — file
edits in a zero-reflow2 session before the bypass backstop fires.
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

# The harness's own file-write tools — counted only for the total-bypass
# backstop (BL-90). A session that touches reflow2 at all never trips it.
EDIT_TOOLS = {"Edit", "Write", "MultiEdit", "NotebookEdit"}

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


def read_state(session_id: str) -> dict:
    """Session tally: graph writes, file edits, and whether reflow2 was touched
    at all. Older state files carried only `writes`; the others default."""
    try:
        raw = json.loads(state_file(session_id).read_text())
        return {
            "writes": int(raw.get("writes", 0)),
            "edits": int(raw.get("edits", 0)),
            "touched": bool(raw.get("touched", False)),
        }
    except (OSError, ValueError, KeyError, TypeError):
        return {"writes": 0, "edits": 0, "touched": False}


def write_state(session_id: str, state: dict) -> None:
    d = state_dir()
    d.mkdir(parents=True, exist_ok=True)
    state_file(session_id).write_text(json.dumps({
        "writes": int(state.get("writes", 0)),
        "edits": int(state.get("edits", 0)),
        "touched": bool(state.get("touched", False)),
    }))
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


def env_threshold(name: str, default: int) -> int:
    try:
        return max(1, int(os.environ.get(name, str(default))))
    except ValueError:
        return default


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
        # This project's reflow2 server; the op is the last __ segment. Any
        # call — even a read — counts as having engaged the design brain.
        if "reflow2" in tool and "__" in tool:
            op = tool.rsplit("__", 1)[-1]
            state = read_state(session)
            state["touched"] = True
            if op in LOOP_OPS:
                state["writes"] = 0
            elif is_write(op):
                state["writes"] += 1
            write_state(session, state)
            return 0
        # A harness file-write, tallied only for the total-bypass backstop.
        if tool in EDIT_TOOLS:
            state = read_state(session)
            state["edits"] += 1
            write_state(session, state)
        return 0

    if kind == "Stop":
        if event.get("stop_hook_active"):
            return 0  # already nudged once — never hold the session hostage
        state = read_state(session)

        # Graph writes finished without a loop check (the original nudge).
        n = state["writes"]
        if n >= env_threshold("REFLOW2_LOOP_NUDGE_THRESHOLD", 1):
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

        # The upstream bypass (BL-90): code edited, the graph never consulted at
        # all. Blunt by design — the hook cannot know which files are design-
        # relevant, so a count threshold and the once-only rule bound the noise.
        if not state["touched"]:
            e = state["edits"]
            if e >= env_threshold("REFLOW2_LOOP_NUDGE_EDIT_THRESHOLD", 3):
                print(json.dumps({
                    "decision": "block",
                    "reason": (
                        f"reflow2: {e} file(s) edited this session and the design "
                        f"graph was never consulted. Start with loop_status; run "
                        f"impact-check before further edits and link-artifacts "
                        f"after, so as-built stays honest. (This nudge fires once; "
                        f"stopping again proceeds.)"
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
