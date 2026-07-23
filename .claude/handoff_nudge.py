#!/usr/bin/env python3
"""Handoff nudge — enforce that a *working* session leaves a next-session note.

A hook cannot AUTHOR the handoff (only the agent knows what the session did), so
this does the enforceable half: it blocks the Stop ONCE when the session made
committed work but never updated the Claude Code handoff memory
(`~/.claude/projects/<project>/memory/`). The "automatic" the user asked for is
really "you cannot cleanly end a working session without leaving the note."

Mirrors `tools/loop_nudge.py`'s Stop discipline: block once, and a second stop
(`stop_hook_active`) always proceeds — a nudge that can loop forever is a
hostage-taker, not a trigger. A hook must never break a session: any failure
warns on stderr and exits 0. Fail-safe throughout — when it cannot tell, it does
NOT block.

Two events, read from the hook's stdin JSON:

- **SessionStart** — snapshot a baseline: current git HEAD and the newest mtime
  among the memory dir's `*.md` files, into one small JSON per session under
  `.reflow2/handoff-nudge/` (gitignored with the rest of `.reflow2/`).
- **Stop** — if HEAD advanced since the baseline (real, committed work) but the
  memory dir's newest mtime did not (no handoff written this session), block once.

Stdlib only, no arguments. `HANDOFF_NUDGE_MIN_COMMITS` (default 1) sets how many
new commits count as "a working session".
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import time
from pathlib import Path


def state_dir() -> Path:
    return Path(".reflow2") / "handoff-nudge"


def state_file(session_id: str) -> Path:
    safe = "".join(c if c.isalnum() or c in "-_" else "_" for c in session_id)
    d = state_dir()
    d.mkdir(parents=True, exist_ok=True)
    # Opportunistic tidy-up: a week-old session file is a dead session.
    cutoff = time.time() - 7 * 24 * 3600
    for old in d.glob("*.json"):
        try:
            if old.stat().st_mtime < cutoff:
                old.unlink()
        except OSError:
            pass
    return d / f"{safe or 'unknown'}.json"


def git_head() -> str:
    try:
        r = subprocess.run(
            ["git", "rev-parse", "HEAD"], capture_output=True, text=True, timeout=5
        )
        return r.stdout.strip() if r.returncode == 0 else ""
    except Exception:  # noqa: BLE001
        return ""


def memory_dir(event: dict) -> Path | None:
    """The Claude Code memory dir for this project, or None if not found.

    Prefer the transcript path (its parent is the project dir); fall back to
    deriving it from cwd the way Claude Code mangles the project path (/ -> -).
    Returns None when neither yields an existing directory, so the caller can
    fail safe rather than nudge on a guess.
    """
    tp = event.get("transcript_path")
    if tp:
        cand = Path(tp).expanduser().parent / "memory"
        if cand.is_dir():
            return cand
    cwd = event.get("cwd") or os.getcwd()
    mangled = str(Path(cwd).resolve()).replace("/", "-")
    cand = Path.home() / ".claude" / "projects" / mangled / "memory"
    return cand if cand.is_dir() else None


def newest_mtime(d: Path | None) -> float:
    if d is None:
        return 0.0
    try:
        return max((p.stat().st_mtime for p in d.glob("*.md")), default=0.0)
    except OSError:
        return 0.0


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
        try:
            state_file(session).write_text(
                json.dumps({"head": git_head(), "mtime": newest_mtime(memory_dir(event))})
            )
        except OSError:
            pass
        return 0

    if kind == "Stop":
        if event.get("stop_hook_active"):
            return 0  # already nudged once — never hold the session hostage
        try:
            base = json.loads(state_file(session).read_text())
        except (OSError, ValueError):
            return 0  # no baseline (e.g. hook installed mid-session) — fail safe

        md = memory_dir(event)
        if md is None:
            return 0  # cannot locate the memory dir — never block on a guess

        try:
            min_commits = max(1, int(os.environ.get("HANDOFF_NUDGE_MIN_COMMITS", "1")))
        except ValueError:
            min_commits = 1

        base_head = base.get("head", "")
        cur_head = git_head()
        worked = bool(cur_head) and cur_head != base_head
        if worked and min_commits > 1 and base_head:
            try:
                r = subprocess.run(
                    ["git", "rev-list", "--count", f"{base_head}..{cur_head}"],
                    capture_output=True, text=True, timeout=5,
                )
                if r.returncode == 0 and int(r.stdout.strip() or "0") < min_commits:
                    worked = False
            except Exception:  # noqa: BLE001
                pass

        handoff_written = newest_mtime(md) > float(base.get("mtime", 0.0))

        if worked and not handoff_written:
            print(json.dumps({
                "decision": "block",
                "reason": (
                    "Session handoff: you committed work this session but did not update the "
                    "next-session memory note. Write/update the handoff in "
                    f"{md} — the next-session note plus its MEMORY.md line — so the next "
                    "session opens oriented, then stop again. (Fires once; stopping again "
                    "proceeds.)"
                ),
            }))
        return 0

    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as e:  # noqa: BLE001 — a hook must never break a session
        print(f"handoff_nudge: skipped ({e})", file=sys.stderr)
        sys.exit(0)
