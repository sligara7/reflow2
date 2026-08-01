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


def edit_tool(tool: str = "Edit", session: str = "s1", path: str = "") -> dict:
    ev = {"hook_event_name": "PostToolUse", "session_id": session,
          "tool_name": tool}
    if path:
        ev["tool_input"] = {"file_path": path}
    return ev


def stop(session: str = "s1", active: bool = False) -> dict:
    return {"hook_event_name": "Stop", "session_id": session,
            "stop_hook_active": active}


class LoopNudge(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory(prefix="loop-nudge-test-")
        self.project = pathlib.Path(self._tmp.name)
        # A project that has opted into being designed. Since the hooks can be
        # registered machine-wide, the script is a no-op without this — which is
        # its own case, below.
        (self.project / ".reflow2").mkdir()

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

    def test_session_start_names_both_doors_and_asserts_no_design(self):
        """A project set up minutes ago gets this line too.

        The hook cannot know whether a design exists — it runs before the
        server is reachable and the graph meta file carries no node counts —
        so the line must not claim one does, and must name the skill for the
        empty case. Anthony, 2026-07-28: set up a fresh project, typed
        `/genesis`, and got nothing; the session had meanwhile been told a
        design graph was here and to read it back with where-am-i.
        """
        r = run_hook(self.project, {"hook_event_name": "SessionStart",
                                    "session_id": "s1"})
        self.assertNotIn("has a design graph", r.stdout)
        for door in ("genesis", "adopt", "where-am-i"):
            self.assertIn(door, r.stdout)

    def test_silent_and_stateless_where_no_design_was_started(self):
        """The hooks can be registered ONCE for the whole machine, so this
        script runs in every directory the user opens — including the ones that
        will never have a design. It must say nothing, count nothing and leave
        nothing behind there, or a machine-wide install is pure noise."""
        import shutil
        shutil.rmtree(self.project / ".reflow2")
        for payload in ({"hook_event_name": "SessionStart", "session_id": "s1"},
                        post_tool("mcp__reflow2__add_requirement"),
                        edit_tool(), stop()):
            r = run_hook(self.project, payload)
            self.assertEqual(r.returncode, 0)
            self.assertEqual(r.stdout, "")
        self.assertFalse((self.project / ".reflow2").exists())

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



class SkillTriggers(unittest.TestCase):
    """cap:skill-triggers — the nudge names the skill the SITUATION calls for.

    The load-bearing property is not that the shapes fire; it is that they add
    NO new interruptions. Each shape only refines a nudge the hook had already
    decided to send, so the count of nudges is unchanged and only the sentence
    improves. `ver:skill-triggers`'s own counterweight says a trigger firing on
    correct work is the BL-23/BL-42 failure, and this exists to reduce nagging.
    """

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.project = pathlib.Path(self.tmp.name)
        (self.project / ".reflow2").mkdir()

    def tearDown(self):
        self.tmp.cleanup()

    def _stop_reason(self, session="s1"):
        r = run_hook(self.project, stop(session))
        if not r.stdout.strip():
            return None
        return json.loads(r.stdout).get("reason", "")

    def test_an_edit_with_no_change_event_names_impact_check(self):
        run_hook(self.project, edit_tool())
        run_hook(self.project, post_tool("mcp__reflow2__add_capability"))
        reason = self._stop_reason()
        self.assertIn("impact-check", reason)
        # The cheap entry point survives the refinement — see the existing
        # test_stop_blocks_once_when_writes_went_unchecked, which caught its loss.
        self.assertIn("loop_status", reason)

    def test_a_recorded_change_with_no_artifact_link_names_link_artifacts(self):
        run_hook(self.project, edit_tool())
        run_hook(self.project, post_tool("mcp__reflow2__record_change"))
        reason = self._stop_reason()
        self.assertIn("link-artifacts", reason)
        self.assertNotIn("impact-check", reason)

    def test_captured_intent_with_no_gap_pass_names_detect_and_ask(self):
        run_hook(self.project, post_tool("mcp__reflow2__add_requirement"))
        reason = self._stop_reason()
        self.assertIn("The shape says", reason)
        self.assertIn("detect-and-ask —", reason)

    def test_loop_status_alone_is_not_a_gap_pass(self):
        """loop_status reports debt; it puts no question to anyone.

        Driven through the HOOK rather than through match_shape, because the
        distinction lives in which op increments the tally — a direct call to
        the matcher cannot see it, and an earlier version of this test missed a
        mutation for exactly that reason.
        """
        run_hook(self.project, post_tool("mcp__reflow2__loop_status"))
        run_hook(self.project, post_tool("mcp__reflow2__add_requirement"))
        # The capture is unchecked, and loop_status did not ask anything.
        # NOTE: assert the shape MARKER — the generic message also contains the
        # string "detect-and-ask" ("run detect-and-ask / check-health"), so a
        # bare substring check cannot tell the two apart and silently passed a
        # mutation that made loop_status count as a gap pass.
        reason = self._stop_reason()
        self.assertIn("The shape says", reason)
        self.assertIn("detect-and-ask —", reason)

    def test_a_real_gap_pass_clears_the_shape(self):
        run_hook(self.project, post_tool("mcp__reflow2__add_requirement"))
        run_hook(self.project, post_tool("mcp__reflow2__detect_gaps"))
        run_hook(self.project, post_tool("mcp__reflow2__add_capability"))
        reason = self._stop_reason()
        self.assertNotIn("The shape says", reason)

    def test_THE_COUNTERWEIGHT_a_session_that_did_it_right_gets_nothing(self):
        """The case that matters most. A session that edited, recorded the
        change, linked the artifact, captured intent AND ran the gap pass has
        no shape — and, having run detect_gaps, no nudge at all."""
        run_hook(self.project, edit_tool())
        for op in ("record_change", "link_artifact", "add_requirement", "detect_gaps"):
            run_hook(self.project, post_tool(f"mcp__reflow2__{op}"))
        self.assertIsNone(self._stop_reason(), "correct work must be met with silence")

    def test_shapes_never_arm_the_hook_by_themselves(self):
        """A shape with NO unchecked writes stays silent. The matcher refines an
        existing nudge; it can never create one."""
        run_hook(self.project, edit_tool())
        run_hook(self.project, post_tool("mcp__reflow2__add_capability"))
        run_hook(self.project, post_tool("mcp__reflow2__detect_gaps"))  # clears writes
        self.assertIsNone(self._stop_reason())

    def test_a_rendering_written_with_nothing_stored_names_session_artifacts(self):
        run_hook(self.project, edit_tool(path="docs/design/flow.svg"))
        # Record the change and link the artifact so the earlier shapes clear
        # and this one is what remains.
        for op in ("record_change", "link_artifact", "add_capability"):
            run_hook(self.project, post_tool(f"mcp__reflow2__{op}"))
        run_hook(self.project, post_tool("mcp__reflow2__detect_gaps"))
        run_hook(self.project, post_tool("mcp__reflow2__add_capability"))
        reason = self._stop_reason()
        self.assertIn("session-artifacts", reason)
        # THE FILTER TRAVELS WITH THE TRIGGER: the hook cannot tell an orphan
        # from an explanation, so it must not imply it can.
        self.assertIn("if nothing points at it, do not", reason)

    def test_an_ordinary_code_edit_is_not_a_rendering(self):
        run_hook(self.project, edit_tool(path="crates/reflow2-core/src/heal.rs"))
        for op in ("record_change", "link_artifact", "add_capability"):
            run_hook(self.project, post_tool(f"mcp__reflow2__{op}"))
        run_hook(self.project, post_tool("mcp__reflow2__detect_gaps"))
        run_hook(self.project, post_tool("mcp__reflow2__add_capability"))
        reason = self._stop_reason()
        self.assertNotIn("session-artifacts", reason or "")

    def test_a_stored_rendering_gets_no_nudge(self):
        run_hook(self.project, edit_tool(path="docs/x.svg"))
        for op in ("record_change", "link_artifact", "content_put", "add_capability"):
            run_hook(self.project, post_tool(f"mcp__reflow2__{op}"))
        run_hook(self.project, post_tool("mcp__reflow2__detect_gaps"))
        run_hook(self.project, post_tool("mcp__reflow2__add_capability"))
        reason = self._stop_reason()
        self.assertNotIn("session-artifacts", reason or "")

    def test_an_unrecognised_shape_still_gets_the_generic_nudge(self):
        # Bookkeeping only: a write with no edits and no captures matches no
        # shape, and must still be nudged with the original wording.
        run_hook(self.project, post_tool("mcp__reflow2__release_includes"))
        reason = self._stop_reason()
        self.assertIn("loop_status", reason)
        self.assertNotIn("The shape says", reason)


if __name__ == "__main__":
    unittest.main(verbosity=2)
