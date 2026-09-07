#!/usr/bin/env python3
"""The reachability instrument must ask reachability WITHOUT asking adoption.

# The failure this pins

On 2026-09-07 the dev_storyflow agent reported that `add_epoch` had no
`description` and `add_requirement` no `priority`. Both properties are declared
in the schema and no typed tool accepts either — and the instrument built to
find exactly that reported neither, because its candidate filter required the
property to be UNUSED as well as unoffered:

    if used:
        continue          # <- the fused filter

`Requirement.priority` declares `default: medium`, and dynograph injects a
declared default at write, so every requirement carries it and the property
reads as fully adopted while no tool on the surface can set it.
`DesignEpoch.description` reads as adopted because a handful of `create_node`
calls wrote one — and it is that type's EMBEDDING FIELD, so the field search
finds an epoch by cannot be written by the tool that makes one.

The two questions were fused. REACHABILITY — does any typed tool accept this
name? — is a property of the surface and needs no usage data at all. ADOPTION —
does anyone write it? — is a property of the graph. Answering both in one
filter makes each unable to report what the other hides. A user found in one
session what the instrument structurally could not.

Cause on the record as
`fact:defect-a-declared-property-can-be-unreachable-and-invisible-to-the-reach-instrument-when-a-default-populates-it`.

# What is pinned, and why it is the CLASS

Not "these two properties are reported" — that pins today and leaves the class
open. What is pinned is that the instrument SEPARATES the two questions: that a
property no typed tool accepts is reported whether or not anything wrote it,
with the reason it looks adopted stated beside it. The two reported properties
are the fixtures, because a test with no case is a test of nothing.

Also pinned: `--check` exits non-zero when a property becomes unreachable that
the baseline does not already carry. That is the half that makes the answer
reach somebody — the instrument was wired into no workflow step, no gate script
and no build target, so it ran only when a person remembered it
(`fact:the-reachability-instrument-is-wired-into-no-gate-so-its-filter-was-never-the-only-cause`).
"""

from __future__ import annotations

import json
import pathlib
import subprocess
import sys
import tempfile

REPO = pathlib.Path(__file__).resolve().parent.parent
SCRIPT = REPO / "tools/vocabulary_reach.py"

failures: list[str] = []


def check(label: str, ok: bool, detail: str = "") -> None:
    print(f"  {'PASS' if ok else 'FAIL'}  {label}" + ("" if ok else f"   {detail}"))
    if not ok:
        failures.append(label)


def run(*args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, str(SCRIPT), *args],
        capture_output=True,
        text=True,
        cwd=REPO,
    )


def main() -> int:
    print("== the reachability instrument separates reachability from adoption ==")

    r = run()
    out = r.stdout
    check("the report runs", r.returncode == 0, r.stderr[-400:])

    # THE TWO REPORTED PROPERTIES. Both are declared, neither is accepted by any
    # typed tool, and both were invisible before the two questions were split.
    for prop in ("Requirement.priority", "DesignEpoch.description"):
        check(
            f"{prop} is reported as unreachable",
            prop in out,
            "a declared property no typed tool accepts must be reported whether or not "
            "anything wrote it — this is the pair a user found and the instrument could not",
        )

    # The REASON it looked adopted has to be on the line, or the reader cannot
    # tell a property hidden by a schema default from one written by hand
    # through the generic escape hatch. Those have different fixes.
    check(
        "the report distinguishes a default-populated property from an escape-hatch one",
        "default" in out.lower() and "create_node" in out,
        "the reason a property looks adopted decides the fix and must be stated",
    )

    # The instrument must still refuse to conflate the two questions in the other
    # direction: a property a tool DOES accept is not reported as unreachable
    # merely because nothing has written it.
    check(
        "a property a tool accepts is still reported separately",
        "UNUSED BUT OFFERED" in out,
        "the adoption question must survive the split, not be replaced by it",
    )

    # --check is what makes the answer reach somebody: it is the mode CI runs.
    r2 = run("--check")
    check(
        "--check passes against the committed baseline",
        r2.returncode == 0,
        f"exit={r2.returncode}: {r2.stdout[-500:]}{r2.stderr[-300:]}",
    )

    # And it must actually FAIL on a new one, or wiring it into CI buys nothing.
    baseline = REPO / "tools/vocabulary_reach_baseline.json"
    check("a baseline is committed", baseline.exists(), "the check needs something to compare to")
    if baseline.exists():
        original = baseline.read_text()
        try:
            trimmed = json.loads(original)
            entries = trimmed.get("unreachable", [])
            check(
                "the baseline is not empty",
                bool(entries),
                "an empty baseline would pass forever and prove nothing",
            )
            if entries:
                # Drop one known entry: the instrument must then report it as new.
                dropped = entries[0]
                trimmed["unreachable"] = entries[1:]
                baseline.write_text(json.dumps(trimmed, indent=2) + "\n")
                r3 = run("--check")
                check(
                    "--check FAILS when a property is unreachable that the baseline lacks",
                    r3.returncode != 0,
                    "a check that cannot go red is not a check",
                )
                check(
                    "the failure names the property",
                    dropped in (r3.stdout + r3.stderr),
                    f"the reader must be told which one: {r3.stdout[-300:]}",
                )
        finally:
            baseline.write_text(original)

    print()
    if failures:
        print(f"{len(failures)} check(s) FAILED")
        return 1
    print("All vocabulary-reach checks passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
