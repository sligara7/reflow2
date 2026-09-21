#!/usr/bin/env python3
"""Run the build gates locally — exactly the ones ci.yml runs, in its order.

⭐ WHY THIS EXISTS. Three records of the gate contract already exist:
`.github/workflows/ci.yml` runs them, AGENTS.md lists them for a person, and
`skill_lint.py` fails the build if those two disagree on coverage or on flags.
**Nothing executed the list.** So every session that wanted a local pass before
pushing wrote its own script from memory, and three sessions produced three
different subsets:

  · 2026-08-?? a split cargo run looked equivalent to `--workspace` and was 46
    tests short (`ver:a-session-can-report-and-read-back-hand-rolled-work`).
  · 2026-09-?? a local run covered a subset of the full job and missed
    `smoke_mcp.py`, the gate that would have caught the defect
    (`chg:accept-9d03ab6c044e6ae6`, whose lesson reads "A subset of a gate set
    is not the gate set").
  · 2026-09-20 a third ran 17 of 48, called `render_skills_and_tools` without
    `--check`, ran both clippy invocations without `-D warnings`, and piped
    `cargo test` through `head -40` — which truncates the report at the
    fortieth of roughly two hundred test binaries AND can take the run down
    through the closed pipe. The log showed forty consecutive passes and no
    failures, which is indistinguishable from a clean full run and is the more
    reassuring of the two readings.

🛑 SO THE ONE RULE HERE IS THAT IT READS THE LIST RATHER THAN RESTATING IT.
A fourth hand-kept copy of the gate set would be the very drift this exists to
end. It imports `ci_gates()` from `skill_lint`, which is already the parser the
build trusts and is itself covered by `test_skill_lint.py` — writing a second
workflow parser here would repeat, one file over, the mistake that
`a_reply_is_sent_once.py` was written to catch.

⚠️ AND NOTHING IS PIPED THROUGH `head`. The last 25 lines of each gate are
shown, but the command's own pipeline is left intact so its exit code is the
real one.

⚠️ RUNNING ZERO GATES IS A FAILURE, NOT A PASS. A filter that matches nothing
would otherwise print a clean summary having checked nothing whatsoever —
the same shape as a detector with nothing to run on.

Usage (from the repo root):

    python3 tools/run_ci_gates.py                    # everything ci.yml runs
    python3 tools/run_ci_gates.py --list             # show them, run none
    python3 tools/run_ci_gates.py reflow2_check.py   # only gates matching a word
    python3 tools/run_ci_gates.py clippy cargo test  # several words, any match

Exits 0 when every gate it ran passed, 1 on any failure or on running none.
Standard library only — deliberately, because pyyaml is not in the base image.
"""

from __future__ import annotations

import argparse
import os
import pathlib
import subprocess
import sys
import time

REPO = pathlib.Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPO / "tools"))

from skill_lint import ci_gates  # noqa: E402  (path set above)

TAIL_LINES = 25


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("keywords", nargs="*", help="only run gates whose command contains one of these")
    ap.add_argument("--list", action="store_true", help="print the gate list and run nothing")
    a = ap.parse_args()

    gates = ci_gates()
    # A parse that came back nearly empty is a broken read, not a clean
    # workflow. skill_lint refuses to lint against one; this refuses to run
    # against one, for the same reason.
    if len(gates) < 10:
        print(
            f"FAIL: read only {len(gates)} gate(s) out of ci.yml — that is a broken parse, "
            f"not a short workflow. Refusing to report a pass against it.",
            file=sys.stderr,
        )
        return 1

    commands = list(gates.values())
    if a.keywords:
        commands = [c for c in commands if any(k in c for k in a.keywords)]

    if a.list:
        for c in commands:
            print(c)
        print(f"\n{len(commands)} gate(s) of {len(gates)} in ci.yml")
        return 0

    if not commands:
        print(
            f"FAIL: no gate matched {a.keywords!r}, so nothing ran. "
            f"Running zero gates is not a pass — use --list to see the names.",
            file=sys.stderr,
        )
        return 1

    failures: list[tuple[str, int]] = []
    started = time.time()
    for command in commands:
        print(f"\n### {command}", flush=True)
        t = time.time()
        proc = subprocess.run(
            ["bash", "-eo", "pipefail", "-c", command],
            cwd=REPO,
            env=os.environ.copy(),
            capture_output=True,
            text=True,
        )
        output = (proc.stdout + proc.stderr).strip()
        print("\n".join(output.splitlines()[-TAIL_LINES:]), flush=True)
        print(f"--- exit {proc.returncode} in {time.time() - t:.1f}s", flush=True)
        if proc.returncode != 0:
            failures.append((command, proc.returncode))

    print("\n" + "=" * 60)
    print(f"RAN {len(commands)} gate(s) of {len(gates)} in {time.time() - started:.0f}s; {len(failures)} FAILED")
    for command, code in failures:
        print(f"  FAIL (exit {code})  {command}")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
