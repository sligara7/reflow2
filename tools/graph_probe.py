#!/usr/bin/env python3
"""Ask the graph what this session left — detached, so nothing waits for it.

`dec:the-stop-hook-asks-the-graph-asynchronously`. The loop nudge has always
reasoned from the session's own tool TALLY: writes since a loop check, edits,
ChangeEvents recorded, skills loaded. That is a proxy, and it is blind in the
one direction that matters — a session that goes through the loop's motions
correctly (write, then `loop_status`) trips no counting branch, whatever it
actually left behind in the graph.

This is the half that asks the graph instead. It is a separate process on
purpose, and the reason is a measurement rather than an architecture
preference:

    loop_status                     22.6s   (repeatably, on reflow2's own graph)
    loop_status since_export=true   37.5s
    claim_report                     0.04s

A Stop hook that blocks for 23 seconds is worse than no hook, and none of the
sub-second calls answer the question. So the hook SPAWNS this and returns
immediately; the answer lands in a file and a LATER stop reads it. A session
stops once per TURN, not once per session, so on anything longer than a couple
of exchanges the answer arrives while the agent is still alive to act on it.

🛑 THE COMMENT THIS CORRECTS. `loop_nudge.py` said for months that the hook
"cannot read the graph — the session's own server holds the single-writer
lock". That was true before the shared server and is false now: the shared
server answers stateless MCP over the URL in `.reflow2/graph.server.json`, and
a read costs nothing but time. The blocker was read as a CAPABILITY limit for
months when it was a LATENCY limit, and those take opposite fixes.

THE TRANSPORT, in full, because each missing piece is a separate refusal and
rediscovering them one at a time is an afternoon. MCP revision 2026-07-28
deletes protocol-level sessions and the initialize handshake, so there is no
handshake to do — but SEP-2243's `Mcp-Method` / `Mcp-Name` headers must mirror
the body or the transport answers -32020 before any handler is reached, AND the
protocol version must appear in the request's `_meta` as well as the header.
The reply is either JSON or SSE frames carrying it. `tools/stateless_seat_probe.py`
exercises the same surface and is where this shape was first proven.

WHAT IT WRITES, and what it deliberately does not: the probe file holds the
counts and a BASELINE, never a verdict. Deciding whether the numbers deserve an
interruption is the hook's job, so the policy lives in one place and this stays
a measurement. It also never writes `reported` — once-per-session is already
`claim_nudge`'s promise in the tally file, and a second flag for the same
guarantee is a second thing to get out of step.

stdlib only, exits 0 on every failure. A probe that breaks a session is worse
than one that says nothing, and a failure is recorded IN the file (as `error`)
rather than swallowed — the same rule the nudge's own `warn` follows.
"""

from __future__ import annotations

import json
import os
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

# The revision that removes sessions and the handshake, and from which the
# SEP-2243 standard headers become required. Matches stateless_seat_probe.py.
PROTOCOL = "2026-07-28"

# The one call that answers the question. Everything cheaper answers a
# different one — see the table in the module docstring.
TOOL = "loop_status"

# Generous, because nothing is waiting: the measured call is ~23s and a loaded
# box or a colder graph can be several times that. The cost of a timeout that
# is too tight is a probe that never lands and a nudge that never speaks.
TIMEOUT_S = 300

# The debt classes worth a delta, in the order a nudge should mention them:
# what the design does not yet KNOW first, what it has not yet CHECKED after.
# `structural_defects` is deliberately absent — 60 of them have stood for weeks
# on this design, HEAL churns the count on unrelated edits, and a class that
# moves without anybody touching it produces a nudge nobody caused.
COUNTS = (
    "unsurfaced_gaps",
    "unanswered_questions",
    "unwritten_answers",
    "undispositioned_drift",
    "unproven_capabilities",
    "unexamined_claims",
    "unsettled_assigned_decisions",
)


def state_dir() -> Path:
    return Path(".reflow2") / "loop-nudge"


def safe_name(session_id: str) -> str:
    safe = "".join(c if c.isalnum() or c in "-_" else "_" for c in session_id)
    return safe or "unknown"


def probe_file(session_id: str) -> Path:
    return state_dir() / f"{safe_name(session_id)}.probe.json"


def lock_file(session_id: str) -> Path:
    return state_dir() / f"{safe_name(session_id)}.probe.lock"


def server_url() -> str | None:
    """The shared server's URL, or None when there is no shared server.

    Absence is the ordinary case for a project served over stdio, and it is not
    an error: the whole feature simply stays quiet. Reported as such rather than
    as a failure, so the hook can tell "no server here" from "the server would
    not answer", which are different facts about the same silence.
    """
    try:
        config = json.loads(Path(".reflow2/graph.server.json").read_text())
    except (OSError, ValueError):
        return None
    url = config.get("url")
    return url if isinstance(url, str) and url.startswith("http") else None


def call(url: str, tool: str, arguments: dict | None = None) -> dict:
    """One stateless MCP tools/call. Returns the structured content, or raises."""
    payload = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": tool,
            "arguments": arguments or {},
            # Required IN THE BODY as well as in the header. Omitting it is
            # answered by the transport, not the tool, so the error names the
            # protocol rather than the call and reads like a version mismatch.
            "_meta": {
                "io.modelcontextprotocol/protocolVersion": PROTOCOL,
                "io.modelcontextprotocol/clientCapabilities": {},
            },
        },
    }
    headers = {
        "Content-Type": "application/json",
        "Accept": "application/json, text/event-stream",
        "MCP-Protocol-Version": PROTOCOL,
        # SEP-2243: these MIRROR the body and are checked before routing.
        "Mcp-Method": "tools/call",
        "Mcp-Name": tool,
    }
    request = urllib.request.Request(
        url, data=json.dumps(payload).encode(), headers=headers
    )
    with urllib.request.urlopen(request, timeout=TIMEOUT_S) as response:
        raw = response.read().decode()
    message = None
    for line in raw.splitlines():
        if line.startswith("data:") and line[5:].strip():
            message = json.loads(line[5:].strip())
    if message is None:
        message = json.loads(raw)
    if "error" in message:
        raise RuntimeError(str(message["error"])[:300])
    result = message.get("result", message)
    return result.get("structuredContent", result)


def counts_of(status: dict) -> dict:
    """The integer debt classes, and only those.

    A class the server does not report is OMITTED rather than defaulted to
    zero: a missing key means an older server that never computed it, and
    calling that zero would manufacture a delta the moment the server is
    upgraded mid-session. Absent on one side of a comparison means no delta,
    which is the quiet direction.
    """
    out = {}
    for key in COUNTS:
        value = status.get(key)
        if isinstance(value, int) and not isinstance(value, bool):
            out[key] = value
    return out


def read_existing(path: Path) -> dict:
    try:
        existing = json.loads(path.read_text())
        return existing if isinstance(existing, dict) else {}
    except (OSError, ValueError):
        return {}


def write_atomically(path: Path, payload: dict) -> None:
    tmp = path.with_suffix(f".{os.getpid()}.tmp")
    tmp.write_text(json.dumps(payload, indent=1))
    os.replace(tmp, path)


def read_change_ids(session_id: str) -> list:
    """The ChangeEvent ids the hook's tally recorded for this session.

    Read from the tally rather than passed on the command line so the probe
    always asks about the session's CURRENT set — a probe spawned two turns ago
    would otherwise ask about a stale one, and the answer would look current.
    """
    try:
        raw = json.loads((state_dir() / f"{safe_name(session_id)}.json").read_text())
    except (OSError, ValueError):
        return []
    ids = raw.get("change_ids") if isinstance(raw, dict) else None
    return [str(c) for c in ids][:64] if isinstance(ids, list) else []


def main() -> int:
    session = sys.argv[1] if len(sys.argv) > 1 else "unknown"
    path = probe_file(session)
    state_dir().mkdir(parents=True, exist_ok=True)

    existing = read_existing(path)
    started = time.time()
    record: dict = {"taken_at": started, "session_id": session}

    url = server_url()
    if url is None:
        # Not a failure. The hook reads `unavailable` and stays silent, and the
        # distinction survives into the file so a later reader is not left
        # guessing whether the probe ran and found nothing.
        record["unavailable"] = "no shared server (.reflow2/graph.server.json absent)"
    else:
        try:
            status = call(url, TOOL)
            record["counts"] = counts_of(status)
            record["counts_taken_at"] = started
            record["clean"] = bool(status.get("clean"))
            record["next"] = [str(n) for n in (status.get("next") or [])][:8]
            # THE SECOND QUESTION, and the only one in the loop that asks what
            # a session made FALSE rather than what it owes. Cheap and bounded:
            # it costs one adjacency walk per recorded event, and an empty id
            # list skips it entirely.
            change_ids = read_change_ids(session)
            if change_ids:
                try:
                    asked = call(url, "unclaimed_findings",
                                 {"change_event_ids": change_ids})
                    record["unclaimed"] = {
                        "count": asked.get("count", 0),
                        "candidates": (asked.get("candidates") or [])[:6],
                        "subjects_examined": asked.get("subjects_examined", 0),
                        "asked_about": len(change_ids),
                    }
                except (urllib.error.URLError, urllib.error.HTTPError, OSError,
                        ValueError, RuntimeError) as e:
                    # NAMED, not swallowed. The loop_status half of this probe
                    # may have succeeded, and a silently missing second answer
                    # would read as "nothing was retired".
                    record["unclaimed_error"] = f"{type(e).__name__}: {e}"[:200]
            served = status.get("served_by")
            if isinstance(served, dict):
                # Carried because a `stale: true` here means every COMPUTED
                # rollup in this answer came from a binary no longer on disk —
                # the trap `fact:an-agent-that-cannot-name-its-version-reports-fixed-bugs`
                # names. A nudge built on those numbers would be arguing from a
                # version nobody is running.
                record["served_by"] = served
        except (urllib.error.URLError, urllib.error.HTTPError, OSError,
                ValueError, RuntimeError) as e:
            record["error"] = f"{type(e).__name__}: {e}"[:400]

    # A FAILED PROBE MUST NOT ERASE THE LAST GOOD READING. Each run builds a
    # fresh record, so without this a server that went away mid-session would
    # take the session's only comparable measurement with it — and the nudge
    # would fall silent about debt that had already been measured rather than
    # report it as of when it was seen. The reading is carried forward WITH its
    # own timestamp, so the hook can say how old it is instead of implying it
    # is current. `counts_taken_at` is what makes the carry honest; dropping it
    # would turn a preserved reading into a fresh-looking one.
    if "counts" not in record and isinstance(existing.get("counts"), dict):
        record["counts"] = existing["counts"]
        record["counts_taken_at"] = existing.get("counts_taken_at",
                                                 existing.get("taken_at"))
        for carried in ("clean", "next", "served_by", "unclaimed"):
            if carried in existing:
                record[carried] = existing[carried]

    record["duration_s"] = round(time.time() - started, 2)

    # THE BASELINE IS WRITTEN ONCE AND NEVER OVERWRITTEN. It is what makes the
    # nudge a statement about THIS SESSION rather than about the design: fired
    # on a level, it would report 7 gaps and 60 defects that have stood for
    # weeks, every session, forever — the fire-on-correct-work failure
    # `ver:skill-triggers` exists to prevent. A session with no baseline gets
    # no nudge at all, because a delta needs two readings and one is not two.
    baseline = existing.get("baseline")
    if not isinstance(baseline, dict) and "counts" in record:
        baseline = {"taken_at": record["taken_at"], "counts": record["counts"]}
    if isinstance(baseline, dict):
        record["baseline"] = baseline

    write_atomically(path, record)
    try:
        lock_file(session).unlink()
    except OSError:
        pass
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as e:  # noqa: BLE001 — never break the session that spawned it
        print(f"graph_probe: skipped ({e})", file=sys.stderr)
        sys.exit(0)
