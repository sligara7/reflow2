#!/usr/bin/env python3
"""The nudge path, proven end to end — not the script, the *installation*.

`req:nudge-path-proven`. `tools/test_loop_nudge.py` already covers the script's
logic given its inputs, and it passed happily the whole time nobody had checked
that the hook was registered, that the command in the settings file resolves to
a script that exists, or that running it the way the harness runs it produces
anything at all. A green suite over a mechanism nothing invokes is exactly the
shape the StoryFlow fleet ran into when it measured **zero wakes** from two
plausible monitor implementations and banned both by name.

So this test does not import the script or call it with arguments of its own
choosing. It reads `.claude/settings.json`, takes the command the harness would
actually run, and runs *that* — with the JSON a real Stop hook receives on
stdin.

What it can prove: the hook is registered, its script exists, it fires when the
loop is owed something, and it stays quiet when it is not (a nudge that cries
wolf is a nudge people learn to skip). What no test here can prove is that the
harness itself invokes hooks — that is the harness's contract, and the honest
answer is to say so rather than to fake it.

stdlib only, hermetic, no network.
"""

from __future__ import annotations

import json
import os
import pathlib
import shutil
import subprocess
import tempfile
import unittest

REPO = pathlib.Path(__file__).resolve().parent.parent
SETTINGS = REPO / ".claude" / "settings.json"


def registered_stop_commands() -> list[str]:
    """Every Stop-hook command this project registers, as written."""
    if not SETTINGS.exists():
        return []
    data = json.loads(SETTINGS.read_text())
    out = []
    for group in data.get("hooks", {}).get("Stop", []):
        for hook in group.get("hooks", []):
            command = hook.get("command")
            if command:
                out.append(command)
    return out


def nudge_command() -> str | None:
    for command in registered_stop_commands():
        if "loop_nudge" in command:
            return command
    return None


def run_registered(command: str, payload: dict, cwd: pathlib.Path) -> subprocess.CompletedProcess:
    """Run the hook exactly as registered, with the environment the harness sets."""
    env = {**os.environ, "CLAUDE_PROJECT_DIR": str(REPO)}
    return subprocess.run(
        command,
        shell=True,  # the settings file holds a shell command line, so run one
        input=json.dumps(payload),
        capture_output=True,
        text=True,
        cwd=cwd,
        env=env,
        timeout=60,
    )


class NudgeIsInstalled(unittest.TestCase):
    def test_a_stop_hook_is_registered_at_all(self):
        self.assertIsNotNone(
            nudge_command(),
            "no Stop hook mentions loop_nudge — the loop's only session-end "
            "backstop is not installed, and nothing else would have told you",
        )

    def test_the_registered_command_points_at_a_script_that_exists(self):
        """The dangerous middle case: settings that look right, a net that is not
        there. It fails silently at exactly the moment it is needed."""
        command = nudge_command()
        self.assertIsNotNone(command)
        token = next(t for t in command.split() if "loop_nudge" in t)
        path = token.strip('"').strip("'").replace("$CLAUDE_PROJECT_DIR/", "")
        script = REPO / path
        self.assertTrue(script.exists(), f"the hook runs {token}, which is not there")

    def test_the_session_start_hook_is_registered_too(self):
        """Orientation is the other half: a session that never learns the graph
        exists cannot owe it anything."""
        data = json.loads(SETTINGS.read_text())
        starts = [
            hook.get("command", "")
            for group in data.get("hooks", {}).get("SessionStart", [])
            for hook in group.get("hooks", [])
        ]
        self.assertTrue(
            any("loop_nudge" in c for c in starts),
            f"SessionStart should orient the session on the graph: {starts}",
        )


class TheNudgeActuallyFires(unittest.TestCase):
    """Runs the REGISTERED command, in a scratch directory, with real payloads."""

    def setUp(self):
        self.command = nudge_command()
        if not self.command:
            self.skipTest("no nudge hook registered")
        self.dir = pathlib.Path(tempfile.mkdtemp(prefix="reflow2-nudgepath-"))
        self.addCleanup(shutil.rmtree, self.dir, ignore_errors=True)
        self.session = f"nudge-path-{os.getpid()}"

    def post_tool_use(self, tool: str):
        return run_registered(
            self.command,
            {
                "session_id": self.session,
                "hook_event_name": "PostToolUse",
                "tool_name": tool,
                "cwd": str(self.dir),
            },
            self.dir,
        )

    def stop(self, already_nudged: bool = False):
        """A Stop event. `already_nudged` is the harness's `stop_hook_active`,
        set when it re-enters after a block — the flag that makes the nudge fire
        once instead of holding a session hostage."""
        return run_registered(
            self.command,
            {
                "session_id": self.session,
                "hook_event_name": "Stop",
                "cwd": str(self.dir),
                "stop_hook_active": already_nudged,
            },
            self.dir,
        )

    def decision(self, result) -> dict:
        """What the harness reads: a JSON decision on stdout.

        NOT the exit code. The first version of this test asserted non-zero and
        failed against a working hook — a test that encodes the wrong contract
        is worse than no test, since it would have been "fixed" by breaking the
        thing it guards. Claude Code's Stop hooks block by printing
        {"decision":"block","reason":...} and exiting 0.
        """
        if not result.stdout.strip():
            return {}
        return json.loads(result.stdout)

    def test_graph_writes_with_no_loop_check_are_interrupted_at_stop(self):
        """THE test: the path from 'an agent wrote to the design and never
        checked the loop' to 'the agent is told, out of band, before it stops'."""
        for _ in range(3):
            self.post_tool_use("mcp__reflow2__add_requirement")

        result = self.stop()

        decision = self.decision(result)
        self.assertEqual(
            decision.get("decision"),
            "block",
            f"the session must be interrupted: {result.stdout!r} {result.stderr!r}",
        )
        reason = decision.get("reason", "")
        self.assertIn("loop_status", reason, reason)
        self.assertIn("graph write", reason, f"the reason must name what happened: {reason}")

    def test_a_session_that_checked_the_loop_is_left_alone(self):
        """A nudge that cries wolf is one people learn to skip, so silence when
        the loop is satisfied is as load-bearing as the interruption."""
        self.post_tool_use("mcp__reflow2__add_requirement")
        self.post_tool_use("mcp__reflow2__loop_status")

        result = self.stop()

        self.assertEqual(
            self.decision(result),
            {},
            f"a session that ran the loop check must not be interrupted: {result.stdout!r}",
        )

    def test_a_session_that_did_nothing_is_left_alone(self):
        result = self.stop()
        self.assertEqual(self.decision(result), {}, result.stdout)

    def test_it_fires_once_and_then_lets_the_session_finish(self):
        """The backstop must not become a wall: a hook that blocks forever is a
        hook someone disables, and then there is no backstop at all."""
        for _ in range(3):
            self.post_tool_use("mcp__reflow2__add_requirement")

        first = self.stop()
        second = self.stop(already_nudged=True)

        self.assertEqual(self.decision(first).get("decision"), "block", "the first stop interrupts")
        self.assertEqual(
            self.decision(second),
            {},
            f"and the second lets go, on the harness's stop_hook_active: {second.stdout!r}",
        )


class TheServerKnowsWhetherTheNetExists(unittest.TestCase):
    """The backstop for projects with NO hook — which is every consumer project,
    since `reflow2_init.py` installs none."""

    BINARY = REPO / "target" / "debug" / "reflow2-mcp"

    @classmethod
    def setUpClass(cls):
        if not cls.BINARY.exists():
            raise unittest.SkipTest(f"{cls.BINARY} not built")

    def handshake_instructions(self, project: pathlib.Path) -> str:
        proc = subprocess.Popen(
            [str(self.BINARY), "--graph-path", str(project / ".reflow2" / "graph")],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
            env={**os.environ, "RUST_LOG": "error"},
        )
        self.addCleanup(proc.terminate)
        proc.stdin.write(
            json.dumps(
                {
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "initialize",
                    "params": {
                        "protocolVersion": "2025-06-18",
                        "capabilities": {},
                        "clientInfo": {"name": "t", "version": "1"},
                    },
                }
            )
            + "\n"
        )
        proc.stdin.flush()
        return json.loads(proc.stdout.readline())["result"].get("instructions", "")

    def test_a_project_with_no_hook_is_told_so_at_handshake(self):
        project = pathlib.Path(tempfile.mkdtemp(prefix="reflow2-nonudge-"))
        self.addCleanup(shutil.rmtree, project, ignore_errors=True)

        instructions = self.handshake_instructions(project)

        self.assertIn("NO SESSION-END NUDGE", instructions, instructions[:400])
        self.assertIn(
            "loop_status",
            instructions,
            "and it must say what to do instead of the missing net",
        )

    def test_a_project_with_the_hook_is_not_nagged(self):
        project = pathlib.Path(tempfile.mkdtemp(prefix="reflow2-withnudge-"))
        self.addCleanup(shutil.rmtree, project, ignore_errors=True)
        (project / ".claude").mkdir(parents=True)
        (project / "tools").mkdir()
        (project / "tools" / "loop_nudge.py").write_text("#!/usr/bin/env python3\n")
        (project / ".claude" / "settings.json").write_text(
            json.dumps(
                {
                    "hooks": {
                        "Stop": [
                            {
                                "hooks": [
                                    {
                                        "type": "command",
                                        "command": 'python3 "$CLAUDE_PROJECT_DIR/tools/loop_nudge.py"',
                                    }
                                ]
                            }
                        ]
                    }
                }
            )
        )

        instructions = self.handshake_instructions(project)

        self.assertNotIn(
            "NO SESSION-END NUDGE",
            instructions,
            "a project that HAS the net must not be told it does not",
        )


if __name__ == "__main__":
    os.chdir(REPO)
    unittest.main(verbosity=2)
