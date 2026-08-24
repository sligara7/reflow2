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
import time
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

    STILL TRUE OF THE SHAPES, AND NO LONGER TRUE OF THE FILE (BL-163). The
    matcher below still cannot arm the hook — `test_shapes_never_arm_the_hook_by
    _themselves` pins that and still passes. But BL-163 added a Stop branch that
    DOES arm, for the recorded-but-never-propagated session, because no shape
    could ever reach it: that session has no unchecked writes and has touched
    reflow2, so there was no nudge for a shape to refine. See `PropagatePrecedes`
    below, which carries that branch's own counterweights.
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
        # BL-163 added an EARLIER shape between these two — a change recorded
        # and never propagated — so propagating is now part of clearing the way
        # to this one, exactly as recording the change already was. The shapes
        # are ordered earliest-first in the loop and PROPAGATE precedes the
        # as-built reconcile, so this ordering is the file's own stated rule
        # rather than a new one.
        run_hook(self.project, post_tool("mcp__reflow2__propagate_change"))
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
        change, PROPAGATED it, linked the artifact, captured intent AND ran the
        gap pass has no shape — and, having run detect_gaps, no nudge at all.

        BL-163 ADDED `propagate_change` TO THIS FIXTURE, and that is a change to
        what "did it right" MEANS, not a weakening of the counterweight. The
        assertion is untouched: correct work is still met with silence. What
        moved is the definition of correct — recording a change and never asking
        what it reaches is the bookkeeping-after this hook's own message calls
        out, so a session that stops there was never doing it right; it was
        merely invisible. The sibling test below pins the exact fixture this one
        used to have, and requires it to fire.
        """
        run_hook(self.project, edit_tool())
        for op in ("record_change", "propagate_change", "link_artifact",
                   "add_requirement", "detect_gaps"):
            run_hook(self.project, post_tool(f"mcp__reflow2__{op}"))
        self.assertIsNone(self._stop_reason(), "correct work must be met with silence")

    def test_BL163_the_old_counterweight_fixture_is_exactly_the_defect(self):
        """The fixture the test above carried before BL-163, which passed.

        Edited code, recorded the change, linked the artifact, captured intent,
        ran the gap pass — and never once asked what the change reached. Every
        older branch reads this as a clean session: `record_change` is a graph
        write, `detect_gaps` clears the write count, and `touched` is true, so
        neither the write nudge nor the bypass nudge can see it. That is how a
        whole session of bookkeeping-after passed for correct work.
        """
        run_hook(self.project, edit_tool())
        for op in ("record_change", "link_artifact", "add_requirement", "detect_gaps"):
            run_hook(self.project, post_tool(f"mcp__reflow2__{op}"))
        reason = self._stop_reason()
        self.assertIsNotNone(reason, "bookkeeping-after must no longer read as clean")
        self.assertIn("propagate_change was never called", reason)
        self.assertIn("impact-check", reason)

    def test_shapes_never_arm_the_hook_by_themselves(self):
        """A shape with NO unchecked writes stays silent. The matcher refines an
        existing nudge; it can never create one."""
        run_hook(self.project, edit_tool())
        run_hook(self.project, post_tool("mcp__reflow2__add_capability"))
        run_hook(self.project, post_tool("mcp__reflow2__detect_gaps"))  # clears writes
        self.assertIsNone(self._stop_reason())

    def test_an_ordinary_code_edit_is_not_a_rendering(self):
        run_hook(self.project, edit_tool(path="crates/reflow2-core/src/heal.rs"))
        for op in ("record_change", "propagate_change", "link_artifact", "add_capability"):
            run_hook(self.project, post_tool(f"mcp__reflow2__{op}"))
        run_hook(self.project, post_tool("mcp__reflow2__detect_gaps"))
        run_hook(self.project, post_tool("mcp__reflow2__add_capability"))
        reason = self._stop_reason()
        self.assertNotIn("session-artifacts", reason or "")

    def test_a_stored_rendering_gets_no_nudge(self):
        run_hook(self.project, edit_tool(path="docs/x.svg"))
        for op in ("record_change", "propagate_change", "link_artifact",
                   "add_capability"):
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


class PropagatePrecedes(unittest.TestCase):
    """BL-163 — the trigger measures a ChangeEvent's PRESENCE, not its PRECEDENCE.

    `CHANGE_OPS` held `record_change` and `add_change_event` — both RECORDING
    ops — and no set counted `propagate_change` at all, so the hook could not
    separate recording from looking because it never counted looking. The
    impact-check shape fired on `changes == 0`, i.e. only when a session recorded
    NOTHING; a session that edited code and wrote its ChangeEvents up afterwards
    had `changes > 0` and was met with silence, while every one of those events
    was the bookkeeping-after the hook's own message says is not the loop.

    The three clauses of the fix are each pinned by a counterweight below, and
    the FIRST of them is the one the row demanded: a session that propagated
    must get nothing.
    """

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.project = pathlib.Path(self.tmp.name)
        (self.project / ".reflow2").mkdir()

    def tearDown(self):
        self.tmp.cleanup()

    def _stop_reason(self, session="s1", env=None):
        r = run_hook(self.project, stop(session), env=env)
        if not r.stdout.strip():
            return None
        return json.loads(r.stdout).get("reason", "")

    def _worked(self, *ops, session="s1", edits=1):
        for _ in range(edits):
            run_hook(self.project, edit_tool(session=session))
        for op in ops:
            run_hook(self.project, post_tool(f"mcp__reflow2__{op}", session=session))

    # --- the true positive -------------------------------------------------

    def test_a_change_recorded_and_never_propagated_is_bookkeeping(self):
        """The defect, end to end, through the hook the harness actually runs.

        `loop_status` here is what makes the case sharp: it clears the write
        count, so the write nudge is disarmed and the session looks clean to
        every branch that existed before this one.
        """
        self._worked("record_change", "loop_status")
        reason = self._stop_reason()
        self.assertIsNotNone(reason)
        self.assertIn("propagate_change was never called", reason)
        self.assertIn("impact-check", reason)
        self.assertIn("Bookkeeping is not the loop", reason)

    def test_add_change_event_is_recording_too_not_looking(self):
        """Both CHANGE_OPS are recording ops — the bulk of the finding in one
        line — so the other one must not clear this either."""
        self._worked("add_change_event", "loop_status")
        reason = self._stop_reason()
        self.assertIsNotNone(reason)
        self.assertIn("propagate_change was never called", reason)

    # --- clause 1: `propagates == 0`. THE COUNTERWEIGHT THE ROW DEMANDED ----

    def test_THE_COUNTERWEIGHT_a_session_that_propagated_gets_nothing(self):
        """The case to pin first (BL-23, BL-42): a trigger that fires on correct
        work is the failure this whole family exists to avoid."""
        self._worked("record_change", "propagate_change", "loop_status")
        self.assertIsNone(self._stop_reason(),
                          "a session that looked at the blast radius did the work")

    def test_propagate_from_is_looking_too(self):
        """The speculative half. The impact-check skill says a "what would this
        touch?" goes straight to `propagate_from` with seed ids, so it is the
        same act and must clear the same trigger."""
        self._worked("record_change", "propagate_from", "loop_status")
        self.assertIsNone(self._stop_reason())

    # --- clause 2: `changes > 0` -------------------------------------------

    def test_a_capture_is_not_a_recorded_change(self):
        """`changes > 0` is what keeps this from becoming a second bypass nudge.
        A session that edited and captured intent — but recorded no ChangeEvent —
        is the OLDER shape's business, not this branch's."""
        self._worked("add_capability", "detect_gaps", "loop_status")
        self.assertIsNone(self._stop_reason())

    # --- clause 3: `edits > 0` ---------------------------------------------

    def test_a_pure_design_session_is_never_asked_to_propagate(self):
        """Nothing on disk moved, so there is no blast radius to compute. A
        design session that records a change and stops is not this defect."""
        for op in ("record_change", "loop_status"):
            run_hook(self.project, post_tool(f"mcp__reflow2__{op}"))
        self.assertIsNone(self._stop_reason())

    # --- the BL-161 precedent: a negative claim needs an intact tally -------

    def test_a_rebuilt_tally_never_claims_propagate_was_never_called(self):
        """"You never propagated" is a NEGATIVE claim, and a tally rebuilt from
        an unreadable one cannot support it — the calls that would refute it are
        exactly what was lost. Same rule, same reason, as the bypass branch.
        """
        self._worked("record_change", "loop_status")
        d = self.project / ".reflow2" / "loop-nudge"
        (d / "s1.json").write_text("{corrupt")
        # Rebuild the tally through the hook, then re-establish the shape.
        self._worked("record_change", "loop_status")
        self.assertIsNone(self._stop_reason(),
                          "a rebuilt tally must not make the negative claim")

    # --- bounds and promises -----------------------------------------------

    def test_the_propagate_threshold_is_configurable(self):
        """Two sessions, because the once-only claim is per session and a second
        stop on the same one would be silent for the wrong reason."""
        env = {"REFLOW2_LOOP_NUDGE_PROPAGATE_THRESHOLD": "2"}
        self._worked("record_change", "propagate_change", "loop_status", session="s1")
        self.assertIsNotNone(self._stop_reason(session="s1", env=env),
                             "one propagate is under a threshold of two")
        self._worked("record_change", "propagate_change", "propagate_change",
                     "loop_status", session="s2")
        self.assertIsNone(self._stop_reason(session="s2", env=env),
                          "two propagates meet the threshold")

    def test_the_propagate_nudge_fires_once(self):
        """Both other blocking branches keep this promise on the same footing,
        so this one has to as well (BL-111)."""
        self._worked("record_change", "loop_status")
        self.assertIsNotNone(self._stop_reason())
        self.assertIsNone(self._stop_reason(), "the claim is spent")

    def test_the_write_nudge_still_takes_precedence(self):
        """Unchecked writes are the more urgent debt and keep their branch: this
        one is reached only once the write count is clear."""
        self._worked("record_change")  # no loop check — writes outstanding
        reason = self._stop_reason()
        self.assertIn("graph write", reason)


class StateIntegrity(unittest.TestCase):
    """BL-161 — the tally survives concurrency, and a rebuilt one never lies.

    The defect this class exists for survived three sessions because it looked
    intermittent. `write_state` was `write_text` (truncate, then write) and
    `read_state` swallowed a parse failure into an all-zero tally — so one
    hook process reading while another wrote got zeros and stored them, wiping
    `touched`/`artifacts`/`gap_pass` for the rest of the session. The session
    that consulted the graph constantly was then told, at Stop, that it never
    had. PostToolUse hooks run as separate processes and parallel tool batches
    are ordinary, so this was the common case wearing a rare face.
    """

    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory(prefix="nudge-state-test-")
        self.project = pathlib.Path(self._tmp.name)
        (self.project / ".reflow2").mkdir()

    def tearDown(self):
        self._tmp.cleanup()

    def state(self, session: str = "s1") -> dict:
        f = self.project / ".reflow2" / "loop-nudge" / f"{session}.json"
        return json.loads(f.read_text()) if f.exists() else {}

    def burst(self, payloads):
        """Run every payload CONCURRENTLY — the condition the bug needs.

        Feed every stdin and close it BEFORE waiting on any process. The first
        version of this looped `communicate()`, which blocks until that child
        exits — so the children ran one at a time and the test passed against
        the unfixed script. The mutation check caught it: removing the atomic
        write and removing the lock both left it green.
        """
        import os
        procs = [
            subprocess.Popen(
                [sys.executable, str(SCRIPT)],
                stdin=subprocess.PIPE, stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL, text=True,
                cwd=self.project, env=dict(os.environ),
            )
            for _ in payloads
        ]
        for p, payload in zip(procs, payloads):
            p.stdin.write(json.dumps(payload))
            p.stdin.close()
        for p in procs:
            p.wait(timeout=60)
        return [p.returncode for p in procs]

    def test_a_concurrent_burst_neither_wipes_nor_undercounts_the_tally(self):
        # THE REPRODUCTION. Against the pre-fix script this returned
        # touched=false, artifacts=0 and edits=6 of 60 — every sticky field
        # wiped and 90% of the increments lost.
        for _ in range(4):
            run_hook(self.project, post_tool("mcp__reflow2__link_artifact"))
        seeded = self.state()
        self.assertTrue(seeded["touched"])
        self.assertEqual(seeded["artifacts"], 4)

        codes = self.burst([edit_tool(path="/tmp/a.rs") for _ in range(60)])
        self.assertTrue(all(c == 0 for c in codes), "a hook must never break")

        after = self.state()
        self.assertEqual(after["edits"], 60, "every concurrent increment lands")
        self.assertTrue(after["touched"], "a sticky field is not wiped")
        self.assertEqual(after["artifacts"], 4, "nor a cumulative one")
        self.assertEqual(after["writes"], 4)

    def test_a_rebuilt_tally_never_claims_the_graph_was_never_consulted(self):
        # THE COUNTERWEIGHT THAT MATTERS. A tally rebuilt after an unreadable
        # one keeps counting — an existing case requires that — but it can no
        # longer support the hook's ONE negative claim, because the calls that
        # would have refuted it are exactly what was lost.
        d = self.project / ".reflow2" / "loop-nudge"
        d.mkdir(parents=True)
        (d / "s1.json").write_text("{corrupt")
        for _ in range(5):
            run_hook(self.project, edit_tool(path="/tmp/a.rs"))

        self.assertTrue(self.state()["reset"], "the restart is on the record")
        self.assertGreaterEqual(self.state()["edits"], 1, "and it kept counting")

        r = run_hook(self.project, stop())
        self.assertEqual(r.returncode, 0)
        self.assertNotIn(
            "never consulted", r.stdout,
            "a tally rebuilt from nothing cannot prove nothing happened",
        )

    def test_a_rebuilt_tally_still_nudges_on_unchecked_writes(self):
        # THE COUNTERWEIGHT TO THE COUNTERWEIGHT: `reset` must not become an
        # off switch. The write nudge is a POSITIVE claim — these writes
        # happened and went unchecked — and a restart does not undermine it.
        d = self.project / ".reflow2" / "loop-nudge"
        d.mkdir(parents=True)
        (d / "s1.json").write_text("{corrupt")
        run_hook(self.project, post_tool("mcp__reflow2__add_capability"))

        self.assertTrue(self.state()["reset"])
        self.assertEqual(self.state()["writes"], 1, "counting restarts")

        r = run_hook(self.project, stop())
        self.assertIn("loop_status", r.stdout, "the positive claim survives")

    def test_the_bulk_forms_count_as_the_singular_ones_do(self):
        # BL-153 shipped bulk forms so the common path stops being N calls, and
        # these sets kept only the singular names — so a session doing
        # everything right THROUGH THE NEW TOOLS tallied as having done none of
        # it. [BL-152]'s shape landing on the trigger that judges the loop.
        for tool, field in (
            ("mcp__reflow2__set_artifact_checksums", "artifacts"),
            ("mcp__reflow2__create_nodes", "captures"),
            ("mcp__reflow2__gaps_to_prompts", "gap_pass"),
        ):
            with self.subTest(tool=tool):
                session = tool.rsplit("__", 1)[-1]
                run_hook(self.project, post_tool(tool, session=session))
                self.assertEqual(
                    self.state(session).get(field), 1,
                    f"{tool} must tally as {field}, like the form it replaces",
                )


class FiresOnce(unittest.TestCase):
    """BL-111 — the nudge's own promise, computed instead of merely stated.

    Every nudge ends *"this nudge fires once; stopping again proceeds"*, and
    that rested entirely on the harness's `stop_hook_active` — a flag covering
    ONE stop cycle that is never persisted. So the rule implemented was *once
    per stop cycle* while the rule advertised was *once per session*, and the
    gap bites hardest exactly where the nudge cannot be satisfied: a session
    whose server is unreachable gets nudged at every stop with no action
    available that would stop it, which is when someone disables the hook.
    """

    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory(prefix="nudge-once-test-")
        self.project = pathlib.Path(self._tmp.name)
        (self.project / ".reflow2").mkdir()

    def tearDown(self):
        self._tmp.cleanup()

    def test_a_second_stop_cycle_does_not_nudge_again(self):
        run_hook(self.project, post_tool("mcp__reflow2__add_capability"))
        first = run_hook(self.project, stop())
        self.assertIn("block", first.stdout)
        # A NEW stop cycle: the harness sets stop_hook_active only within one
        # cycle, so this is the case the old code could not see.
        second = run_hook(self.project, stop())
        self.assertEqual(second.stdout.strip(), "", "the promise is once per SESSION")
        self.assertEqual(second.returncode, 0)

    def test_the_bypass_branch_keeps_the_same_promise(self):
        for _ in range(4):
            run_hook(self.project, edit_tool(path="/tmp/a.rs"))
        first = run_hook(self.project, stop())
        self.assertIn("never consulted", first.stdout)
        second = run_hook(self.project, stop())
        self.assertEqual(second.stdout.strip(), "",
                         "both blocking branches promise it, so both keep it")

    def test_two_registrations_firing_at_once_still_nudge_once(self):
        # THE CASE THIS WAS FILED FROM, and it is not exotic: reflow2 installs
        # machine-wide AND a project can carry its own registration, and the two
        # command spellings do not dedupe — so two hook processes run on the
        # same Stop. Without an atomic claim both read nudged=false and both
        # print, which is the doubled message the user saw.
        import os
        run_hook(self.project, post_tool("mcp__reflow2__add_capability"))
        procs = [
            subprocess.Popen(
                [sys.executable, str(SCRIPT)],
                stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                stderr=subprocess.DEVNULL, text=True,
                cwd=self.project, env=dict(os.environ),
            )
            for _ in range(2)
        ]
        for p in procs:
            p.stdin.write(json.dumps(stop()))
            p.stdin.close()
        outs = []
        for p in procs:
            outs.append(p.stdout.read())
            p.wait(timeout=60)
        spoke = [o for o in outs if o.strip()]
        self.assertEqual(len(spoke), 1,
                         f"exactly one of two concurrent hooks may speak, got {outs}")

    def test_a_nudge_that_was_never_earned_leaves_the_claim_unspent(self):
        # THE COUNTERWEIGHT: `nudged` must be set by NUDGING, not by stopping.
        # A session with nothing owed stops silently and keeps its one nudge, or
        # the flag would become a way to burn the trigger by stopping early.
        quiet = run_hook(self.project, stop())
        self.assertEqual(quiet.stdout.strip(), "")
        run_hook(self.project, post_tool("mcp__reflow2__add_capability"))
        earned = run_hook(self.project, stop())
        self.assertIn("block", earned.stdout,
                      "an unspent claim must still be there when debt appears")


class SkillLoadsAreCounted(unittest.TestCase):
    """`cap:skill-loads-are-counted` — the session that did everything else
    right and never opened a skill.

    The hole `cap:skill-triggers` cannot reach: every shape it recognises lives
    in the graph-write stream, and "no skill has been loaded" is not a graph
    write. Twice the decay was found by a human asking an agent.

    Most of these are COUNTERWEIGHTS rather than the feature. This branch ARMS a
    nudge, which `cap:skill-triggers` deliberately never did, so what matters as
    much as its firing is the set of correct sessions it stays quiet for.
    """

    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory(prefix="loop-nudge-skills-")
        self.project = pathlib.Path(self._tmp.name)
        (self.project / ".reflow2").mkdir()

    def tearDown(self):
        self._tmp.cleanup()

    def _worked(self, session="s1", n=4):
        """A session that did DESIGN work, checked the loop, and moved n files.

        Clean by every other branch's reckoning: the write is settled by the
        loop check, so `writes` is 0; no ChangeEvent, so the propagate branch
        stays quiet; reflow2 was touched, so the bypass branch stays quiet.
        Whatever fires on this shape fires on an otherwise-correct session.
        """
        run_hook(self.project, post_tool("mcp__reflow2__add_capability", session))
        run_hook(self.project, post_tool("mcp__reflow2__loop_status", session))
        for _ in range(n):
            run_hook(self.project, edit_tool(session=session, path="/tmp/a.rs"))

    def test_a_session_that_worked_and_loaded_no_skill_is_told(self):
        self._worked()
        out = run_hook(self.project, stop()).stdout
        self.assertIn("block", out)
        self.assertIn("no skill was ever loaded", out)
        # The message must name the way IN, not just the failing. A nudge that
        # says "you should have used a skill" and not how is the shape
        # `dec:a-failing-check-says-against-what` exists to prevent.
        self.assertIn("list_skills", out)
        self.assertIn("get_skill", out)

    def test_loading_one_skill_silences_it(self):
        # THE CENTRAL COUNTERWEIGHT. One `get_skill` and this must never speak.
        self._worked()
        run_hook(self.project, post_tool("mcp__reflow2__get_skill"))
        out = run_hook(self.project, stop()).stdout
        self.assertEqual(out.strip(), "",
                         "a session that opened a skill has nothing owed here")

    def test_listing_skills_is_not_loading_one(self):
        # Knowing the menu exists is not reading the recipe. If `list_skills`
        # counted, a session could tick the box without opening anything —
        # which is the exact self-deception this capability exists to end.
        self._worked()
        run_hook(self.project, post_tool("mcp__reflow2__list_skills"))
        out = run_hook(self.project, stop()).stdout
        self.assertIn("no skill was ever loaded", out)

    def test_a_small_session_is_left_alone(self):
        # Below the edit floor. Most sessions are small, and nagging every one
        # of them is how a trigger gets ignored (BL-23, BL-42).
        self._worked(n=1)
        out = run_hook(self.project, stop()).stdout
        self.assertEqual(out.strip(), "", "a one-file session is not a practice failure")

    def test_a_session_that_never_touched_reflow2_gets_the_bypass_message(self):
        # Not this branch's case: the session that ignored the design brain
        # entirely needs the blunter message, and getting BOTH would be two
        # interruptions for one failure.
        for _ in range(4):
            run_hook(self.project, edit_tool(path="/tmp/a.rs"))
        out = run_hook(self.project, stop()).stdout
        self.assertIn("never consulted", out)
        self.assertNotIn("no skill was ever loaded", out)

    def test_it_never_speaks_over_a_branch_that_already_fired(self):
        # Placed LAST so it can only fill silence. Here the write nudge is owed,
        # AND no skill was loaded — exactly one message may result.
        run_hook(self.project, post_tool("mcp__reflow2__add_capability"))
        for _ in range(4):
            run_hook(self.project, edit_tool(path="/tmp/a.rs"))
        out = run_hook(self.project, stop()).stdout
        self.assertIn("block", out)
        self.assertNotIn("no skill was ever loaded", out)
        self.assertEqual(out.count('"decision"'), 1, "one stop, one message")

    def test_a_rebuilt_tally_never_claims_no_skill_was_loaded(self):
        # A NEGATIVE CLAIM on an amnesiac count. The unreadable tally lost
        # exactly the `get_skill` calls that would refute it (BL-161).
        d = self.project / ".reflow2" / "loop-nudge"
        d.mkdir(parents=True)
        (d / "s1.json").write_text("{corrupt")
        self._worked()
        out = run_hook(self.project, stop()).stdout
        self.assertNotIn("no skill was ever loaded", out)

    def test_it_keeps_the_once_per_session_promise(self):
        self._worked()
        first = run_hook(self.project, stop())
        self.assertIn("no skill was ever loaded", first.stdout)
        second = run_hook(self.project, stop())
        self.assertEqual(second.stdout.strip(), "",
                         "every blocking branch makes the promise, so every one keeps it")

    def test_the_count_never_leaves_the_session_tally(self):
        # THE LINE `dec:adoption-is-reported-the-loop-still-computes` DRAWS,
        # held as a property rather than a promise: adoption is session state,
        # never design state. If a skill count ever reached the graph it would
        # become something the loop could reason from, which is precisely what
        # `dec:loop-status-state-not-history` forbids.
        #
        # Checked two ways, because either alone is weak. First: the hook writes
        # nothing into the project at all — no export, no store, no stray file.
        before = sorted(p.name for p in self.project.iterdir())
        self._worked()
        run_hook(self.project, post_tool("mcp__reflow2__get_skill"))
        run_hook(self.project, stop())
        after = sorted(p.name for p in self.project.iterdir())
        self.assertEqual(before, after,
                         "the hook must leave the designed project untouched")

        # Second: the counter is absent from every module that computes loop
        # debt. A grep, with a POSITIVE CONTROL so a silently-wrong path cannot
        # pass it by matching nothing.
        core = pathlib.Path(__file__).resolve().parent.parent / "crates" / "reflow2-core" / "src"
        sources = list(core.rglob("*.rs"))
        self.assertTrue(sources, "positive control: the core sources must be findable")
        blob = "\n".join(p.read_text(errors="ignore") for p in sources)
        self.assertIn("loop_status", blob, "positive control: the loop lives here")
        self.assertNotIn("SKILL_OPS", blob)
        self.assertNotIn('"skills"', blob,
                         "no skill count may be readable by anything that computes debt")


class AsksTheGraph(unittest.TestCase):
    """cap:stop-nudge-asks-the-graph — the branch that reads the graph instead
    of the session's tool tally.

    THE CASE THIS SUITE EXISTS FOR is `test_a_level_never_fires`. Every other
    case here would pass on an implementation that nudged whenever the design
    carried debt — and that implementation would speak in every session forever,
    which is the fire-on-correct-work failure `ver:skill-triggers` names. The
    delta is the feature; the counts are just how it is computed.

    The probe files are hand-written rather than produced by a live probe: the
    unit under test is the POLICY (what deserves an interruption), and coupling
    it to a 23-second call would make the suite untestable and would test the
    server instead. `test_the_probe_is_spawned_and_nothing_waits_for_it` covers
    the seam between the two.
    """

    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory(prefix="loop-nudge-graph-")
        self.project = pathlib.Path(self._tmp.name)
        (self.project / ".reflow2").mkdir()
        self.nudge_dir = self.project / ".reflow2" / "loop-nudge"
        self.nudge_dir.mkdir()

    def tearDown(self):
        self._tmp.cleanup()

    def write_probe(self, session="s1", baseline=None, counts=None, **extra):
        """A probe file as graph_probe.py would leave it.

        `taken_at` is deliberately AFTER the baseline's: a probe that is its own
        baseline is a single reading and must not produce a delta, which is its
        own case below.
        """
        record = {"session_id": session, "taken_at": 2000.0,
                  "counts_taken_at": 2000.0}
        if counts is not None:
            record["counts"] = counts
        if baseline is not None:
            record["baseline"] = {"taken_at": 1000.0, "counts": baseline}
        record.update(extra)
        (self.nudge_dir / f"{session}.probe.json").write_text(json.dumps(record))
        return record

    def tally(self, session="s1", **fields):
        """A session tally that trips NO counting branch.

        Every graph-branch case needs this: the branch speaks last, so a tally
        that armed any earlier nudge would test the ordering rather than the
        feature. `writes: 0` (checked), `edits: 0` (nothing on disk) and
        `touched` is the shape of a session that did the loop correctly — which
        is exactly the session this branch exists to reach.
        """
        state = {"writes": 0, "edits": 0, "touched": True, "changes": 0,
                 "propagates": 0, "artifacts": 0, "captures": 0, "gap_pass": 0,
                 "renderings": 0, "skills": 1, "wrote": 4, "reset": False,
                 "nudged": False}
        state.update(fields)
        (self.nudge_dir / f"{session}.json").write_text(json.dumps(state))

    def blocked(self, session="s1"):
        r = run_hook(self.project, stop(session))
        self.assertEqual(r.returncode, 0)
        return json.loads(r.stdout) if r.stdout.strip() else None

    # ---- the trigger --------------------------------------------------------

    def test_a_rise_since_the_baseline_fires_and_names_both_numbers(self):
        self.tally()
        self.write_probe(baseline={"unsurfaced_gaps": 7},
                         counts={"unsurfaced_gaps": 10})
        out = self.blocked()
        self.assertIsNotNone(out, "a count this session raised must be reported")
        self.assertEqual(out["decision"], "block")
        # BOTH numbers, not just the delta: "3 new gaps" leaves the reader
        # unable to tell a session that doubled the debt from one that added
        # three to a hundred.
        self.assertIn("7", out["reason"])
        self.assertIn("10", out["reason"])
        self.assertIn("detect-and-ask", out["reason"])

    def test_a_level_never_fires(self):
        """THE CASE THE FEATURE IS JUDGED ON.

        reflow2's own design has carried 7 unsurfaced gaps and 60 structural
        defects for weeks. A nudge keyed on the LEVEL would fire in every
        session forever and be a nag rather than a trigger. Standing debt is
        not this session's doing and this branch must not claim it is.
        """
        self.tally()
        self.write_probe(baseline={"unsurfaced_gaps": 7, "unproven_capabilities": 1},
                         counts={"unsurfaced_gaps": 7, "unproven_capabilities": 1})
        self.assertIsNone(self.blocked(), "standing debt is not this session's")

    def test_a_count_that_fell_is_not_a_delta(self):
        self.tally()
        self.write_probe(baseline={"unsurfaced_gaps": 10},
                         counts={"unsurfaced_gaps": 4})
        self.assertIsNone(self.blocked(), "settling debt must never be nudged")

    def test_structural_defects_are_not_a_trigger_class(self):
        """HEAL's count moves on edits nobody made to it, so a delta there
        would report a session for something it did not do. Deliberately
        absent from COUNTS in graph_probe.py and from COUNT_ADVICE here."""
        self.tally()
        self.write_probe(baseline={"structural_defects": 60},
                         counts={"structural_defects": 91})
        self.assertIsNone(self.blocked())

    def test_every_advertised_class_can_actually_fire(self):
        """A positive control over the whole table.

        COUNT_ADVICE and graph_probe.COUNTS are two lists of the same keys in
        two processes with no shared module between them. A key measured there
        with no sentence here is silent debt; this is what notices.
        """
        import importlib.util
        spec = importlib.util.spec_from_file_location("ln", SCRIPT)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        probe_spec = importlib.util.spec_from_file_location(
            "gp", SCRIPT.parent / "graph_probe.py")
        probe = importlib.util.module_from_spec(probe_spec)
        probe_spec.loader.exec_module(probe)

        advised = [key for key, _, _ in module.COUNT_ADVICE]
        self.assertEqual(sorted(advised), sorted(probe.COUNTS),
                         "every measured class needs a sentence, and vice versa")
        for key in advised:
            with self.subTest(key=key):
                self.tally(session=key)
                self.write_probe(session=key, baseline={key: 1}, counts={key: 2})
                out = self.blocked(session=key)
                self.assertIsNotNone(out, f"{key} is measured but cannot speak")

    # ---- the cases where it must say nothing --------------------------------

    def test_no_baseline_means_no_nudge(self):
        """A delta needs two readings. A session whose SessionStart probe never
        landed has one, and one reading cannot support a claim about what the
        session changed — the same rule the `reset` guard applies to the tally.
        """
        self.tally()
        self.write_probe(counts={"unsurfaced_gaps": 99})
        self.assertIsNone(self.blocked())

    def test_a_probe_that_is_its_own_baseline_says_nothing(self):
        self.tally()
        (self.nudge_dir / "s1.probe.json").write_text(json.dumps({
            "taken_at": 1000.0, "counts_taken_at": 1000.0,
            "counts": {"unsurfaced_gaps": 7},
            "baseline": {"taken_at": 1000.0, "counts": {"unsurfaced_gaps": 7}},
        }))
        self.assertIsNone(self.blocked())

    def test_a_class_missing_from_one_reading_is_not_a_delta(self):
        """The shape of a server upgraded mid-session: a class the older build
        never computed is absent, not zero. Reading absence as zero would
        manufacture a delta out of a version change and blame the session."""
        self.tally()
        self.write_probe(baseline={}, counts={"unwritten_answers": 3})
        self.assertIsNone(self.blocked())

    def test_an_unreadable_probe_is_silent(self):
        self.tally()
        (self.nudge_dir / "s1.probe.json").write_text("{not json")
        self.assertIsNone(self.blocked())

    def test_no_probe_at_all_is_silent(self):
        self.tally()
        self.assertIsNone(self.blocked())

    def test_a_project_with_no_shared_server_never_spawns_a_probe(self):
        """The ordinary stdio project. There is no URL to ask, that is normal,
        and it must cost nothing and say nothing."""
        self.tally(writes=0, edits=2, wrote=2)
        r = run_hook(self.project, stop())
        self.assertEqual(r.stdout, "")
        self.assertFalse(list(self.nudge_dir.glob("*.probe.*")),
                         "no server config means no probe and no lock file")

    # ---- it must not become a second interruption ---------------------------

    def test_it_never_adds_a_second_nudge_to_an_interrupted_session(self):
        """`claim_nudge` is one flag for the whole session across every branch.
        A session already being interrupted is not interrupted twice — this can
        only speak into what would otherwise have been silence."""
        self.tally(writes=3, wrote=3)          # arms the original write nudge
        self.write_probe(baseline={"unsurfaced_gaps": 7},
                         counts={"unsurfaced_gaps": 10})
        first = self.blocked()
        self.assertIsNotNone(first)
        self.assertIn("graph write", first["reason"],
                      "the counting branch speaks first")
        # Keyed on the graph branch's OWN words, not on "detect-and-ask" — the
        # generic write nudge already names that skill, so asserting its absence
        # tested the wording of the other branch rather than which branch spoke.
        self.assertNotIn("not counted from your tool calls", first["reason"])
        # And the graph branch does not then get its own turn.
        self.assertIsNone(self.blocked(), "one interruption per session, still")

    def test_a_second_stop_in_the_same_cycle_proceeds(self):
        self.tally()
        self.write_probe(baseline={"unsurfaced_gaps": 7},
                         counts={"unsurfaced_gaps": 10})
        r = run_hook(self.project, stop(active=True))
        self.assertEqual(r.stdout, "", "a nudge that can loop is a hostage-taker")

    # ---- what it says -------------------------------------------------------

    def test_a_stale_server_binary_is_declared_beside_the_numbers(self):
        """`served_by.stale` means every COMPUTED count in that answer came
        from a binary no longer on disk. The nudge still fires — those are the
        best numbers there are — but it must not present them as current."""
        self.tally()
        self.write_probe(baseline={"unsurfaced_gaps": 7},
                         counts={"unsurfaced_gaps": 10},
                         served_by={"stale": True, "reflow2_version": "0.39.0"})
        out = self.blocked()
        self.assertIn("STALE", out["reason"])
        self.assertIn("--stop-shared", out["reason"],
                      "a warning with no remedy is half a warning")

    def test_it_says_the_number_came_from_the_graph_not_the_tally(self):
        """The whole point of the branch, and the agent cannot act on it
        correctly without knowing which kind of claim it is."""
        self.tally()
        self.write_probe(baseline={"unsurfaced_gaps": 7},
                         counts={"unsurfaced_gaps": 10})
        reason = self.blocked()["reason"]
        self.assertIn("loop_status", reason)
        self.assertIn("not counted from your tool calls", reason)

    def test_it_tells_the_agent_to_confirm_because_the_reading_can_be_stale(self):
        """The answer is whatever the last probe found, and a session still in
        flight settles some of what it raises — observed on the session that
        built this, where gaps read 7 → 10 mid-work and finished at 6. Waiting
        for a fresh probe is the 23-second block the async shape exists to
        avoid, so the reading states its age and points at the call that
        answers now. A nudge that cannot be checked is an accusation."""
        self.tally()
        self.write_probe(baseline={"unsurfaced_gaps": 7},
                         counts={"unsurfaced_gaps": 10})
        reason = self.blocked()["reason"]
        self.assertIn("CONFIRM WITH loop_status", reason)
        self.assertIn("s ago", reason, "the reading must state its own age")

    # ---- the cross-session fallback -----------------------------------------

    def test_a_verdict_the_last_session_never_heard_surfaces_at_session_start(self):
        self.write_probe(session="old", baseline={"unsurfaced_gaps": 7},
                         counts={"unsurfaced_gaps": 10})
        r = run_hook(self.project, {"hook_event_name": "SessionStart",
                                    "session_id": "new"})
        self.assertEqual(r.returncode, 0)
        self.assertIn("session before this one", r.stdout)
        self.assertIn("10", r.stdout)
        # ...and exactly once. An unstoppable reminder is the hostage-taking
        # the once-only rule exists to prevent, one session further out.
        again = run_hook(self.project, {"hook_event_name": "SessionStart",
                                        "session_id": "newer"})
        self.assertNotIn("session before this one", again.stdout)

    def test_session_start_still_prints_the_orientation_with_no_probe(self):
        r = run_hook(self.project, {"hook_event_name": "SessionStart",
                                    "session_id": "s1"})
        self.assertIn("Orient first", r.stdout)
        self.assertNotIn("session before this one", r.stdout)

    # ---- the seam between the hook and the probe ----------------------------

    def test_the_probe_is_spawned_and_nothing_waits_for_it(self):
        """The one case that crosses the process boundary.

        A closed port is used deliberately: the probe fails fast, so the test
        stays hermetic and quick while still exercising the real spawn, the
        real lock file, and the real failure path. What is asserted is the
        contract that made the async shape necessary — the HOOK RETURNS
        IMMEDIATELY. `loop_status` is 22.6 seconds; if this ever blocks on the
        probe, the feature has become the thing it was designed not to be.
        """
        (self.project / ".reflow2" / "graph.server.json").write_text(
            json.dumps({"url": "http://127.0.0.1:1/"}))
        self.tally(writes=0, edits=1, wrote=1)
        started = time.time()
        r = run_hook(self.project, stop())
        elapsed = time.time() - started
        self.assertEqual(r.returncode, 0)
        self.assertEqual(r.stdout, "", "a spawn is not a verdict")
        self.assertLess(elapsed, 5.0,
                        "the Stop hook must never wait on the graph")
        # The probe really ran, really failed, and really said so.
        probe = self.nudge_dir / "s1.probe.json"
        for _ in range(100):
            if probe.exists():
                break
            time.sleep(0.1)
        self.assertTrue(probe.exists(), "the detached probe never wrote its file")
        data = json.loads(probe.read_text())
        self.assertIn("error", data, "a failed probe records the failure")
        self.assertNotIn("counts", data)
        self.assertFalse((self.nudge_dir / "s1.probe.lock").exists(),
                         "the probe clears its own lock on the way out")
        # And a failed probe is silent, not a guess.
        self.assertIsNone(self.blocked())

    def test_a_read_only_session_is_never_asked_about(self):
        """Asking costs a 23-second call on the same server the session is
        using. A session that wrote nothing and edited nothing has left nothing
        for the graph to report."""
        (self.project / ".reflow2" / "graph.server.json").write_text(
            json.dumps({"url": "http://127.0.0.1:1/"}))
        self.tally(writes=0, edits=0, wrote=0, touched=True)
        run_hook(self.project, stop())
        self.assertFalse((self.nudge_dir / "s1.probe.lock").exists())
        self.assertFalse((self.nudge_dir / "s1.probe.json").exists())

    def test_a_probe_in_flight_is_not_started_twice(self):
        (self.project / ".reflow2" / "graph.server.json").write_text(
            json.dumps({"url": "http://127.0.0.1:1/"}))
        self.tally(writes=0, edits=1, wrote=1)
        (self.nudge_dir / "s1.probe.lock").write_text(
            json.dumps({"started": time.time(), "reason": "held"}))
        run_hook(self.project, stop())
        self.assertFalse((self.nudge_dir / "s1.probe.json").exists(),
                         "a probe already in flight must not be duplicated")

    def test_a_recent_answer_is_not_re_asked(self):
        (self.project / ".reflow2" / "graph.server.json").write_text(
            json.dumps({"url": "http://127.0.0.1:1/"}))
        self.tally(writes=0, edits=1, wrote=1)
        (self.nudge_dir / "s1.probe.json").write_text(json.dumps({
            "taken_at": time.time(), "counts": {"unsurfaced_gaps": 7},
        }))
        run_hook(self.project, stop())
        self.assertFalse((self.nudge_dir / "s1.probe.lock").exists(),
                         "the debounce must hold between turns")

    def test_a_failed_probe_does_not_erase_the_last_good_reading(self):
        """Each probe run builds a fresh record, so without the carry-forward a
        server that went away mid-session would take the session's only
        comparable measurement with it — and the nudge would fall silent about
        debt already measured rather than report it as of when it was seen."""
        (self.project / ".reflow2" / "graph.server.json").write_text(
            json.dumps({"url": "http://127.0.0.1:1/"}))
        good = {"session_id": "s1", "taken_at": 1500.0, "counts_taken_at": 1500.0,
                "counts": {"unsurfaced_gaps": 10}, "clean": False,
                "baseline": {"taken_at": 1000.0,
                             "counts": {"unsurfaced_gaps": 7}}}
        (self.nudge_dir / "s1.probe.json").write_text(json.dumps(good))
        r = subprocess.run(
            [sys.executable, str(SCRIPT.parent / "graph_probe.py"), "s1"],
            capture_output=True, text=True, cwd=self.project, timeout=60)
        self.assertEqual(r.returncode, 0)
        data = json.loads((self.nudge_dir / "s1.probe.json").read_text())
        self.assertIn("error", data)
        self.assertEqual(data["counts"], {"unsurfaced_gaps": 10})
        self.assertEqual(data["counts_taken_at"], 1500.0,
                         "a carried reading keeps its OWN age, or the nudge "
                         "would report a preserved number as a fresh one")
        self.assertEqual(data["baseline"]["counts"], {"unsurfaced_gaps": 7},
                         "the baseline is written once and never overwritten")

    def test_the_kit_ships_every_script_the_hook_depends_on(self):
        """The silent-absence trap, gated.

        `spawn_probe` returns quietly when the probe script is not beside it —
        correct behaviour for a kit that predates the feature, and indis-
        tinguishable from a graph with nothing to report. The release workflow
        stages the kit with one `cp` per tool, so a sibling added here and not
        there ships a hook whose graph branch never runs and never says why:
        the same shape as a detector reporting zero because it had nothing to
        run on.

        Asserted against the workflow rather than against a built tarball
        because that is where the omission would happen, and it fails at the
        commit rather than at the release.
        """
        workflow = (SCRIPT.parent.parent / ".github" / "workflows"
                    / "release.yml")
        self.assertTrue(workflow.exists(), "positive control: the workflow must be findable")
        staged = workflow.read_text()
        self.assertIn("cp tools/loop_nudge.py", staged,
                      "positive control: this is how the kit stages a tool")
        import importlib.util
        spec = importlib.util.spec_from_file_location("ln", SCRIPT)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        self.assertIn(f"cp tools/{module.PROBE_SCRIPT.name}", staged,
                      f"the kit does not ship {module.PROBE_SCRIPT.name}, so the "
                      f"graph branch would be silently absent for every consumer")

    def test_the_probe_reports_a_missing_server_as_unavailable_not_as_failure(self):
        r = subprocess.run(
            [sys.executable, str(SCRIPT.parent / "graph_probe.py"), "s1"],
            capture_output=True, text=True, cwd=self.project, timeout=60)
        self.assertEqual(r.returncode, 0)
        data = json.loads((self.nudge_dir / "s1.probe.json").read_text())
        self.assertIn("unavailable", data)
        self.assertNotIn("error", data,
                         "no shared server is a normal state, not a failure")


class AsksWhatWasMadeFalse(unittest.TestCase):
    """The retired-observations ask — `unclaimed_findings` reaching the session.

    THE CASE THIS SUITE IS JUDGED ON is `test_it_never_arms_a_nudge_by_itself`.
    Anthony chose ask-don't-block deliberately: this trigger is keyed on a
    computation nobody has field-tested, and a brand-new trigger that can stop a
    session is exactly the thing that becomes wallpaper. Every other assertion
    here would pass on an implementation that blocked; that one would not.
    """

    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory(prefix="loop-nudge-retired-")
        self.project = pathlib.Path(self._tmp.name)
        (self.project / ".reflow2").mkdir()
        self.nudge_dir = self.project / ".reflow2" / "loop-nudge"
        self.nudge_dir.mkdir()

    def tearDown(self):
        self._tmp.cleanup()

    def probe(self, session="s1", candidates=None, count=None, **extra):
        rec = {"session_id": session, "taken_at": 2000.0, "counts_taken_at": 2000.0}
        if candidates is not None:
            rec["unclaimed"] = {
                "count": len(candidates) if count is None else count,
                "candidates": candidates,
                "subjects_examined": 1,
                "asked_about": 1,
            }
        rec.update(extra)
        (self.nudge_dir / f"{session}.probe.json").write_text(json.dumps(rec))

    def tally(self, session="s1", **fields):
        state = {"writes": 0, "edits": 0, "touched": True, "changes": 0,
                 "propagates": 0, "artifacts": 0, "captures": 0, "gap_pass": 0,
                 "renderings": 0, "skills": 1, "wrote": 4, "reset": False,
                 "nudged": False, "change_ids": []}
        state.update(fields)
        (self.nudge_dir / f"{session}.json").write_text(json.dumps(state))

    def stop_out(self, session="s1"):
        r = run_hook(self.project, stop(session))
        self.assertEqual(r.returncode, 0)
        return json.loads(r.stdout) if r.stdout.strip() else None

    ONE = [{"finding_id": "fact:service-is-slow",
            "name": "the service is slow", "valid_from": "2026-08-21"}]

    # ---- the restraint -----------------------------------------------------

    def test_it_never_arms_a_nudge_by_itself(self):
        """ASK, DON'T BLOCK — Anthony 2026-08-24.

        A clean tally trips no counting branch. The shortlist must not turn that
        silence into an interruption, however good the shortlist is.
        """
        self.tally()
        self.probe(candidates=self.ONE)
        self.assertIsNone(self.stop_out(),
                          "the ask must never be the reason a session is stopped")

    def test_it_rides_along_on_a_nudge_already_firing(self):
        """Free by construction: it speaks only where one was happening anyway."""
        self.tally(writes=3, wrote=3)          # arms the original write nudge
        self.probe(candidates=self.ONE)
        out = self.stop_out()
        self.assertIsNotNone(out)
        self.assertIn("graph write", out["reason"], "the host message survives")
        self.assertIn("MAKE FALSE", out["reason"])
        self.assertIn("the service is slow", out["reason"])
        self.assertIn("2026-08-21", out["reason"], "when it was taken travels with it")

    def test_it_says_candidate_and_names_the_tool_that_closes_one(self):
        # A shortlist with no remedy is an accusation, and one presented as a
        # verdict invites closing something still true — the expensive mistake.
        self.tally(writes=1, wrote=1)
        self.probe(candidates=self.ONE)
        reason = self.stop_out()["reason"]
        self.assertIn("CANDIDATE", reason)
        self.assertIn("invalidates", reason)
        self.assertIn("never by overwriting", reason,
                      "closing preserves — the rule the first closure ever made broke")

    # ---- silence, where silence is right ------------------------------------

    def test_an_empty_shortlist_adds_nothing_to_a_firing_nudge(self):
        self.tally(writes=2, wrote=2)
        self.probe(candidates=[])
        reason = self.stop_out()["reason"]
        self.assertIn("graph write", reason)
        self.assertNotIn("MAKE FALSE", reason)

    def test_no_probe_answer_at_all_is_silent(self):
        self.tally(writes=2, wrote=2)
        self.probe()                      # probe ran, never asked the question
        self.assertNotIn("MAKE FALSE", self.stop_out()["reason"])

    def test_a_long_shortlist_names_three_and_counts_the_rest(self):
        rows = [{"finding_id": f"fact:{i}", "name": f"finding {i}"} for i in range(9)]
        self.tally(writes=1, wrote=1)
        self.probe(candidates=rows, count=9)
        reason = self.stop_out()["reason"]
        self.assertIn("finding 0", reason)
        self.assertIn("and 6 more", reason,
                      "what was left out is counted, never silently dropped")

    # ---- the ids the question needs ----------------------------------------

    def test_a_recorded_change_contributes_its_id(self):
        """`unclaimed_findings` can only answer against specific events, and this
        hook is the one place that sees them written."""
        run_hook(self.project, {"hook_event_name": "PostToolUse",
                                "session_id": "s1",
                                "tool_name": "mcp__reflow2__add_change_event",
                                "tool_input": {"id": "chg:one"}})
        run_hook(self.project, {"hook_event_name": "PostToolUse",
                                "session_id": "s1",
                                "tool_name": "mcp__reflow2__record_change",
                                "tool_input": {"id": "chg:two"}})
        state = json.loads((self.nudge_dir / "s1.json").read_text())
        self.assertEqual(state["change_ids"], ["chg:one", "chg:two"])

    def test_the_same_event_written_twice_is_carried_once(self):
        for _ in range(3):
            run_hook(self.project, {"hook_event_name": "PostToolUse",
                                    "session_id": "s1",
                                    "tool_name": "mcp__reflow2__add_change_event",
                                    "tool_input": {"id": "chg:same"}})
        state = json.loads((self.nudge_dir / "s1.json").read_text())
        self.assertEqual(state["change_ids"], ["chg:same"],
                         "revising an event is one event, not three")

    def test_a_write_with_no_id_still_counts_as_a_change(self):
        run_hook(self.project, {"hook_event_name": "PostToolUse",
                                "session_id": "s1",
                                "tool_name": "mcp__reflow2__add_change_event"})
        state = json.loads((self.nudge_dir / "s1.json").read_text())
        self.assertEqual(state["changes"], 1, "the count is unaffected")
        self.assertEqual(state["change_ids"], [])

    def test_the_id_list_is_bounded(self):
        for i in range(40):
            run_hook(self.project, {"hook_event_name": "PostToolUse",
                                    "session_id": "s1",
                                    "tool_name": "mcp__reflow2__add_change_event",
                                    "tool_input": {"id": f"chg:{i}"}})
        ids = json.loads((self.nudge_dir / "s1.json").read_text())["change_ids"]
        self.assertEqual(len(ids), 25, "a bulk session must not grow the tally forever")
        self.assertEqual(ids[-1], "chg:39", "the newest are the ones kept")

    # ---- the late delivery --------------------------------------------------

    def test_an_ask_the_last_session_never_heard_surfaces_at_session_start(self):
        self.probe(session="old", candidates=self.ONE)
        r = run_hook(self.project, {"hook_event_name": "SessionStart",
                                    "session_id": "new"})
        self.assertIn("MAKE FALSE", r.stdout)
        self.assertIn("session before this one", r.stdout)
        again = run_hook(self.project, {"hook_event_name": "SessionStart",
                                        "session_id": "newer"})
        self.assertNotIn("MAKE FALSE", again.stdout, "and exactly once")


if __name__ == "__main__":
    unittest.main(verbosity=2)
