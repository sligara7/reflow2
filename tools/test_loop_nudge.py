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

    def test_a_rendering_written_with_nothing_stored_names_session_artifacts(self):
        run_hook(self.project, edit_tool(path="docs/design/flow.svg"))
        # Record the change, PROPAGATE it (BL-163) and link the artifact so the
        # earlier shapes clear and this one is what remains.
        for op in ("record_change", "propagate_change", "link_artifact", "add_capability"):
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
        for op in ("record_change", "propagate_change", "link_artifact", "add_capability"):
            run_hook(self.project, post_tool(f"mcp__reflow2__{op}"))
        run_hook(self.project, post_tool("mcp__reflow2__detect_gaps"))
        run_hook(self.project, post_tool("mcp__reflow2__add_capability"))
        reason = self._stop_reason()
        self.assertNotIn("session-artifacts", reason or "")

    def test_a_stored_rendering_gets_no_nudge(self):
        run_hook(self.project, edit_tool(path="docs/x.svg"))
        for op in ("record_change", "propagate_change", "link_artifact",
                   "content_put", "add_capability"):
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


if __name__ == "__main__":
    unittest.main(verbosity=2)
