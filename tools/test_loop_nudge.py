#!/usr/bin/env python3
"""Tests for tools/loop_nudge.py — the BL-74 trigger hook.

Hermetic and stdlib-only, like test_init.py: each case runs the script as a
subprocess with a hook-shaped stdin JSON in a temp project directory, because
the subprocess boundary (stdin, stdout contract, exit code) IS the interface
Claude Code calls. A hook that breaks a session is worse than no hook, so the
never-crash contract is tested as hard as the counting.
"""

from __future__ import annotations

import json
import pathlib
import subprocess
import sys
import tempfile
import unittest

SCRIPT = pathlib.Path(__file__).resolve().parent / "loop_nudge.py"


def run_hook(cwd: pathlib.Path, payload, env: dict | None = None):
    import os
    full_env = dict(os.environ)
    if env:
        full_env.update(env)
    return subprocess.run(
        [sys.executable, str(SCRIPT)],
        input=payload if isinstance(payload, str) else json.dumps(payload),
        capture_output=True, text=True, cwd=cwd, env=full_env, timeout=30,
    )


def post_tool(tool: str, session: str = "s1") -> dict:
    return {"hook_event_name": "PostToolUse", "session_id": session,
            "tool_name": tool}


def edit_tool(tool: str = "Edit", session: str = "s1") -> dict:
    return {"hook_event_name": "PostToolUse", "session_id": session,
            "tool_name": tool}


def stop(session: str = "s1", active: bool = False) -> dict:
    return {"hook_event_name": "Stop", "session_id": session,
            "stop_hook_active": active}


class LoopNudge(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory(prefix="loop-nudge-test-")
        self.project = pathlib.Path(self._tmp.name)

    def tearDown(self):
        self._tmp.cleanup()

    def writes(self, session: str = "s1") -> int:
        f = self.project / ".reflow2" / "loop-nudge" / f"{session}.json"
        return json.loads(f.read_text())["writes"] if f.exists() else 0

    def edits(self, session: str = "s1") -> int:
        f = self.project / ".reflow2" / "loop-nudge" / f"{session}.json"
        return json.loads(f.read_text()).get("edits", 0) if f.exists() else 0

    def test_session_start_prints_the_orientation(self):
        r = run_hook(self.project, {"hook_event_name": "SessionStart",
                                    "session_id": "s1"})
        self.assertEqual(r.returncode, 0)
        self.assertIn("Orient first", r.stdout)
        self.assertIn("loop_status", r.stdout)

    def test_graph_writes_are_counted_per_session(self):
        for tool in ("mcp__reflow2__add_capability", "mcp__reflow2__satisfies",
                     "mcp__reflow2__create_node"):
            r = run_hook(self.project, post_tool(tool))
            self.assertEqual(r.returncode, 0)
            self.assertEqual(r.stdout, "")
        self.assertEqual(self.writes(), 3)
        run_hook(self.project, post_tool("mcp__reflow2__add_requirement", "s2"))
        self.assertEqual(self.writes("s2"), 1)
        self.assertEqual(self.writes("s1"), 3, "sessions do not share a counter")

    def test_a_loop_check_resets_the_count(self):
        run_hook(self.project, post_tool("mcp__reflow2__add_capability"))
        run_hook(self.project, post_tool("mcp__reflow2__loop_status"))
        self.assertEqual(self.writes(), 0)
        run_hook(self.project, post_tool("mcp__reflow2__add_capability"))
        run_hook(self.project, post_tool("mcp__reflow2__detect_gaps"))
        self.assertEqual(self.writes(), 0)

    def test_reads_resolves_and_foreign_tools_are_ignored(self):
        for tool in ("mcp__reflow2__scan_nodes",          # read
                     "mcp__reflow2__answer_question",     # resolve step
                     "mcp__reflow2__set_artifact_checksum",  # disposition
                     "mcp__reflow2__acknowledge_gap",     # resolve step
                     "Bash", "mcp__other__add_capability"):
            run_hook(self.project, post_tool(tool))
        self.assertEqual(self.writes(), 0)

    def test_stop_blocks_once_when_writes_went_unchecked(self):
        run_hook(self.project, post_tool("mcp__reflow2__add_capability"))
        r = run_hook(self.project, stop())
        self.assertEqual(r.returncode, 0)
        out = json.loads(r.stdout)
        self.assertEqual(out["decision"], "block")
        self.assertIn("loop_status", out["reason"])
        self.assertIn("1 graph write", out["reason"])

        # The second stop always proceeds — a nudge, never a hostage-taker.
        r2 = run_hook(self.project, stop(active=True))
        self.assertEqual(r2.stdout, "")

    def test_stop_passes_when_the_loop_ran(self):
        run_hook(self.project, post_tool("mcp__reflow2__add_capability"))
        run_hook(self.project, post_tool("mcp__reflow2__loop_status"))
        r = run_hook(self.project, stop())
        self.assertEqual(r.stdout, "", "no debt, no nudge")

    def test_stop_passes_on_a_read_only_session(self):
        run_hook(self.project, post_tool("mcp__reflow2__scan_nodes"))
        r = run_hook(self.project, stop())
        self.assertEqual(r.stdout, "")

    def test_threshold_is_configurable(self):
        env = {"REFLOW2_LOOP_NUDGE_THRESHOLD": "3"}
        for _ in range(2):
            run_hook(self.project, post_tool("mcp__reflow2__add_capability"))
        self.assertEqual(run_hook(self.project, stop(), env=env).stdout, "")
        run_hook(self.project, post_tool("mcp__reflow2__add_capability"))
        out = json.loads(run_hook(self.project, stop(), env=env).stdout)
        self.assertEqual(out["decision"], "block")

    # --- BL-90: the total-bypass backstop (edited code, zero reflow2 calls) ---

    def test_file_edits_are_counted_per_session(self):
        for tool in ("Edit", "Write", "MultiEdit", "NotebookEdit"):
            r = run_hook(self.project, edit_tool(tool))
            self.assertEqual(r.returncode, 0)
            self.assertEqual(r.stdout, "", "counting an edit is silent")
        self.assertEqual(self.edits(), 4)

    def test_stop_nudges_the_session_that_edited_but_never_touched_reflow2(self):
        for _ in range(3):  # default edit threshold is 3
            run_hook(self.project, edit_tool("Edit"))
        r = run_hook(self.project, stop())
        self.assertEqual(r.returncode, 0)
        out = json.loads(r.stdout)
        self.assertEqual(out["decision"], "block")
        self.assertIn("never consulted", out["reason"])
        self.assertIn("loop_status", out["reason"])
        self.assertIn("3 file", out["reason"])

        # Fires once — a second stop always proceeds.
        r2 = run_hook(self.project, stop(active=True))
        self.assertEqual(r2.stdout, "")

    def test_edits_below_threshold_do_not_nudge(self):
        for _ in range(2):  # under the default threshold of 3
            run_hook(self.project, edit_tool("Write"))
        self.assertEqual(run_hook(self.project, stop()).stdout, "")

    def test_touching_reflow2_at_all_disarms_the_bypass_nudge(self):
        # A single read means the agent DID consult the graph — no bypass.
        run_hook(self.project, post_tool("mcp__reflow2__scan_nodes"))
        for _ in range(5):
            run_hook(self.project, edit_tool("Edit"))
        self.assertEqual(run_hook(self.project, stop()).stdout, "",
                         "engaged the design brain, so no bypass nudge")

    def test_edit_threshold_is_configurable(self):
        env = {"REFLOW2_LOOP_NUDGE_EDIT_THRESHOLD": "5"}
        for _ in range(4):
            run_hook(self.project, edit_tool("Edit"))
        self.assertEqual(run_hook(self.project, stop(), env=env).stdout, "")
        run_hook(self.project, edit_tool("Edit"))
        out = json.loads(run_hook(self.project, stop(), env=env).stdout)
        self.assertEqual(out["decision"], "block")

    def test_graph_write_nudge_takes_precedence_over_edits(self):
        # Wrote the graph AND edited: the write case is what needs the loop.
        run_hook(self.project, edit_tool("Edit"))
        run_hook(self.project, post_tool("mcp__reflow2__add_capability"))
        out = json.loads(run_hook(self.project, stop()).stdout)
        self.assertEqual(out["decision"], "block")
        self.assertIn("graph write", out["reason"])

    def test_garbage_never_breaks_the_session(self):
        for payload in ("not json at all", "[]", json.dumps({"no": "event"}),
                        json.dumps({"hook_event_name": "PostToolUse"})):
            r = run_hook(self.project, payload)
            self.assertEqual(r.returncode, 0, payload)
        # A corrupted state file is survived, not crashed on.
        d = self.project / ".reflow2" / "loop-nudge"
        d.mkdir(parents=True)
        (d / "s1.json").write_text("{corrupt")
        r = run_hook(self.project, post_tool("mcp__reflow2__add_capability"))
        self.assertEqual(r.returncode, 0)
        self.assertEqual(self.writes(), 1, "count restarts from the readable truth")


if __name__ == "__main__":
    unittest.main(verbosity=2)
