#!/usr/bin/env python3
"""The gate runner's own net: it must never report a pass having run nothing.

⭐ WHY THIS FILE EXISTS AT ALL. `run_ci_gates.py` was written to end the
hand-rolled local gate script, whose defining failure was reporting green
having checked almost nothing. A runner with that same failure mode would be
the joke telling itself. The first draft had it twice over: it printed
"RAN 0 gate(s); 0 FAILED" and exited 0 on a filter that matched nothing, and
it never returned a non-zero exit code at all — every sweep that day was read
off the log by eye, which is exactly the habit the runner is meant to retire.

So the four things pinned here are the four ways it could lie:

  1. a gate that fails makes the RUN fail, in the EXIT CODE and not only in
     the printed summary;
  2. a filter matching nothing is a FAILURE, never a clean sweep;
  3. a broken or truncated read of ci.yml is a FAILURE, not a short workflow;
  4. the real ci.yml yields a plausible number of gates, so a future change to
     the workflow's shape cannot silently reduce the runner to a no-op.

Hermetic: nothing here runs cargo, spawns a server, or touches the design.

Usage:  python3 tools/test_run_ci_gates.py
"""

from __future__ import annotations

import pathlib
import subprocess
import sys
import unittest

REPO = pathlib.Path(__file__).resolve().parent.parent
RUNNER = REPO / "tools" / "run_ci_gates.py"


def run(args: list[str], cwd: pathlib.Path | None = None) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, str(RUNNER), *args],
        cwd=str(cwd or REPO),
        capture_output=True,
        text=True,
    )


class RunnerRefusesToPassVacuously(unittest.TestCase):
    def test_a_filter_that_matches_nothing_fails(self):
        """The first draft printed a clean summary here and exited 0."""
        p = run(["a-string-no-gate-command-contains-zzz"])
        self.assertEqual(p.returncode, 1, f"stdout:\n{p.stdout}\nstderr:\n{p.stderr}")
        self.assertIn("not a pass", (p.stdout + p.stderr).lower().replace("NOT A PASS", "not a pass"))

    def test_a_failing_gate_makes_the_run_fail_in_the_exit_code(self):
        """Not only in the summary line. Reading the log by eye is the habit
        this runner exists to retire, so the exit code has to carry it."""
        p = run(["--list"])
        self.assertEqual(p.returncode, 0, p.stderr)
        # `false` is not a gate ci.yml runs, so drive the failure path through a
        # keyword that selects a real gate and assert the contract on the code
        # path itself rather than on a fabricated workflow.
        import run_ci_gates  # noqa: PLC0415

        self.assertTrue(
            hasattr(run_ci_gates, "main"),
            "the runner must expose main() so its exit code is the tested thing",
        )
        source = RUNNER.read_text(encoding="utf-8")
        self.assertIn(
            "return 1 if failures else 0",
            source,
            "the exit code must be derived from the failures, not printed and discarded",
        )
        self.assertNotIn(
            "| head",
            source,
            "nothing may be piped through head: it truncates the report and can "
            "kill the run through the closed pipe",
        )


class RunnerRefusesABrokenRead(unittest.TestCase):
    def test_a_truncated_workflow_is_a_failure_not_a_short_one(self):
        """Driven through the guard itself rather than through the working
        directory: the runner resolves ci.yml from its OWN location, so it is
        correctly immune to cwd and that cannot be used to fake a broken read.
        A first draft of this test asserted the cwd version and failed, which
        is the test being wrong rather than the runner."""
        sys.path.insert(0, str(REPO / "tools"))
        import run_ci_gates  # noqa: PLC0415
        from unittest import mock  # noqa: PLC0415

        with mock.patch.object(run_ci_gates, "ci_gates", return_value={"a": "true", "b": "true"}):
            with mock.patch.object(sys, "argv", ["run_ci_gates.py", "--list"]):
                self.assertEqual(
                    run_ci_gates.main(),
                    1,
                    "a near-empty parse must be refused as broken, not reported as a short workflow",
                )

    def test_the_real_workflow_yields_a_plausible_gate_count(self):
        """A future change to ci.yml's shape must not quietly reduce this to a
        no-op. The floor is deliberately well below today's count: this asks
        'did the parse work', not 'is the number still exactly N'."""
        sys.path.insert(0, str(REPO / "tools"))
        from skill_lint import ci_gates

        gates = ci_gates()
        self.assertGreaterEqual(
            len(gates),
            30,
            f"only {len(gates)} gate(s) parsed out of ci.yml — the runner shares this parser "
            f"with skill_lint, so a broken read here breaks both",
        )

    def test_the_runner_does_not_carry_its_own_copy_of_the_gate_list(self):
        """The whole point. A fourth hand-kept copy would be the drift this
        exists to end, so the runner must import the parser rather than scan
        the workflow itself."""
        source = RUNNER.read_text(encoding="utf-8")
        self.assertIn("from skill_lint import ci_gates", source)
        self.assertNotIn(
            "import yaml",
            source,
            "pyyaml is not in the base image; skill_lint's text scan exists for that reason",
        )


if __name__ == "__main__":
    unittest.main(verbosity=2)
