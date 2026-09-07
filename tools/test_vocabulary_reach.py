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

    # THE BUCKET THE FUSED FILTER COULD NOT PRODUCE, pinned on live members of
    # it. `Actor.actor_type` is populated ONLY because it declares a schema
    # default, and `Capability.tier` only through generic create_node — the two
    # ways a property looks adopted while nothing on the surface can set it.
    for prop in ("Actor.actor_type", "Capability.tier"):
        check(
            f"{prop} is reported as unreachable though it is populated",
            prop in out,
            "a declared property no typed tool accepts must be reported whether or not "
            "anything wrote it — usage is not reach",
        )

    # ⭐ THE PAIR THAT PROMPTED ALL OF THIS IS NOW REACHABLE, AND THIS ASSERTION
    # IS THE MOVED PIN. It read `prop in out` until 2026-09-07, when the two
    # properties a user reported were given parameters on the tools that make
    # the nodes — `description` on add_epoch and plan_epoch, `priority` on
    # add_requirement. Both were observed failing in the old direction before
    # the parameters landed, which is what makes the fix evidenced rather than
    # asserted. If either reappears here, a constructor lost a parameter.
    for prop in ("Requirement.priority", "DesignEpoch.description"):
        check(
            f"{prop} is NO LONGER unreachable — its constructor takes it",
            prop not in out,
            "a user reported this field missing and it was given a parameter; seeing it "
            "here again means the constructor lost it",
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

    baseline = REPO / "tools/vocabulary_reach_baseline.json"

    # ── THE SPLIT BY INTENT ────────────────────────────────────────────────
    #
    # A flat count of unreachable properties conflates two opposite things:
    # a HOLE (nobody can write it and somebody wants to) and a property an
    # OPERATION writes, where a caller parameter would be actively wrong —
    # letting somebody hand-set a creation timestamp or a computed mirror hash
    # is a forgery hazard, not a feature. Measured 2026-09-07: of 43 entries,
    # roughly two thirds were the second kind. The number could not be read as
    # a to-do list, which is the same fused-question defect this instrument was
    # already fixed for once, one layer along.
    entries = []
    if baseline.exists():
        entries = json.loads(baseline.read_text()).get("entries", [])
    check(
        "the baseline classifies each entry rather than listing it flat",
        bool(entries) and all(isinstance(e, dict) for e in entries),
        "entries must be objects carrying a kind and a reason, not bare strings",
    )
    KINDS = {"machine_written", "hole", "unused"}
    unclassified = [
        e.get("property")
        for e in entries
        if e.get("kind") not in KINDS or not str(e.get("reason", "")).strip()
    ]
    check(
        "every entry declares a kind AND a reason a reader can disagree with",
        not unclassified,
        f"unclassified: {unclassified[:6]}",
    )
    machine = [e for e in entries if e.get("kind") == "machine_written"]
    check(
        "a machine-written entry names the operation that writes it",
        all("written_by" in e and str(e["written_by"]).strip() for e in machine),
        "a claim that something else writes it is unfalsifiable without naming what",
    )
    check(
        "the report separates holes from what an operation writes",
        "HOLE" in out.upper() and "OPERATION" in out.upper(),
        "a single count conflates a gap with a deliberate state and cannot be acted on",
    )

    # --check is what makes the answer reach somebody: it is the mode CI runs.
    r2 = run("--check")
    check(
        "--check passes against the committed baseline",
        r2.returncode == 0,
        f"exit={r2.returncode}: {r2.stdout[-500:]}{r2.stderr[-300:]}",
    )

    # And it must actually FAIL on a new one, or wiring it into CI buys nothing.
    check("a baseline is committed", baseline.exists(), "the check needs something to compare to")
    if baseline.exists():
        original = baseline.read_text()
        try:
            trimmed = json.loads(original)
            entries = trimmed.get("entries", [])
            check(
                "the baseline is not empty",
                bool(entries),
                "an empty baseline would pass forever and prove nothing",
            )
            if entries:
                # Drop one known entry: the instrument must then report it as new.
                dropped = entries[0]["property"]
                trimmed["entries"] = entries[1:]
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
