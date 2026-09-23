#!/usr/bin/env python3
"""Turn a real test run into per-FILE outcomes that reconcile_verification reads.

A test runner reports in files and test names; the design records checks. This
is the bridge between the two, so a run can be fed back with
`reconcile_verification(observed_by_file=..., record_events=true,
detected_at=...)` without hand-mapping a single id. It is the first half of
step 4 (dec:step-4-is-built-on-real-runs-with-no-threshold-and-unscored-good-news):
the doubt layer is only worth building once real failures actually flow.

Three sources, any mix, one JSON list out (worst outcome per file wins):

  --cargo FILE      the text output of `cargo test --no-fail-fast` (`-` = stdin).
                    Each test TARGET's "test result:" line is its file's
                    outcome. A target that failed to compile prints no result
                    and is reported as failed, not dropped.
  --junit FILE      JUnit XML — what pytest (--junitxml), cargo-nextest, jest,
                    go-junit-report and most CI runners write. Grouped by each
                    testcase's `file` attribute; testcases with none are
                    counted and named in stderr, never guessed.
  --run-python GLOB run each matching python test script and record its exit
                    code (0 = passed). This converter is named run_to_files.py,
                    NOT test_*.py, so that glob never runs it as a test. reflow2's own tools/test_*.py are plain
                    unittest scripts with no shared runner, so their exit code
                    IS their result.

Paths come out relative to --root (default: the current directory), the form a
design records `location` in. Nothing here reads or writes the design.
"""

from __future__ import annotations

import argparse
import glob
import os
import json
import pathlib
import re
import subprocess
import sys
import xml.etree.ElementTree as ET

RANK = {"passed": 0, "skipped": 1, "failed": 2}

RUNNING = re.compile(r"^\s*Running (?:unittests )?(\S+) \((\S+)\)")
RESULT = re.compile(r"^test result: (ok|FAILED)\.")
FAILED_TARGET = re.compile(r"^\s*`-p (\S+) --(test|lib|bin) ?(\S*)`")
# CI runners colour cargo's output; a raw log carries the escapes.
ANSI = re.compile(r"\x1b\[[0-9;]*m")


def worst(into: dict[str, str], location: str, outcome: str) -> None:
    """Keep the worst outcome seen for a file: one failure fails the file."""
    if location not in into or RANK[outcome] > RANK[into[location]]:
        into[location] = outcome


SKIP_DIRS = {"target", ".git", "node_modules", ".venv"}


def crate_dirs(root: pathlib.Path) -> list[pathlib.Path]:
    """Every directory holding a Cargo.toml, found once, never descending into
    build output — a `target/` tree is the bulk of a Rust checkout."""
    found = []
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in SKIP_DIRS]
        if "Cargo.toml" in filenames:
            found.append(pathlib.Path(dirpath))
    return found


def resolve_cargo_path(
    printed: str, binary: str, crates: list[pathlib.Path], root: pathlib.Path
) -> str | None:
    """cargo prints a target's path relative to ITS crate; find which crate."""
    # the deps binary is `<crate_or_stem>-<hash>`; crate names use `_`.
    stem = pathlib.Path(binary).name.rsplit("-", 1)[0]
    candidates = [c / printed for c in crates if (c / printed).exists()]
    if len(candidates) > 1:
        # a unittests src/lib.rs exists in every crate: disambiguate by name.
        depth = len(pathlib.Path(printed).parts)
        named = [
            c for c in candidates if c.parents[depth - 1].name.replace("-", "_") == stem
        ]
        candidates = named or candidates
    if len(candidates) != 1:
        return None
    return str(candidates[0].resolve().relative_to(root.resolve()))


def from_cargo(text: str, root: pathlib.Path, out: dict[str, str]) -> list[str]:
    unresolved: list[str] = []
    current: str | None = None
    crates = crate_dirs(root)
    text = ANSI.sub("", text)
    for line in text.splitlines():
        m = RUNNING.match(line)
        if m:
            current = resolve_cargo_path(m.group(1), m.group(2), crates, root)
            if current is None:
                unresolved.append(m.group(1))
            continue
        r = RESULT.match(line.strip())
        if r and current:
            worst(out, current, "passed" if r.group(1) == "ok" else "failed")
            current = None
    # Targets cargo lists as failed after the run — including ones that never
    # compiled and so never printed a result line.
    for line in text.splitlines():
        f = FAILED_TARGET.match(line)
        if f and f.group(2) == "test" and f.group(3):
            for c in crates:
                p = c / "tests" / f"{f.group(3)}.rs"
                if p.exists():
                    worst(out, str(p.resolve().relative_to(root.resolve())), "failed")
    return unresolved


def from_junit(path: str, root: pathlib.Path, out: dict[str, str]) -> int:
    no_file = 0
    tree = ET.parse(path)
    for case in tree.iter("testcase"):
        f = case.get("file")
        if not f:
            no_file += 1
            continue
        if case.find("failure") is not None or case.find("error") is not None:
            outcome = "failed"
        elif case.find("skipped") is not None:
            outcome = "skipped"
        else:
            outcome = "passed"
        p = pathlib.Path(f)
        rel = str(p.resolve().relative_to(root.resolve())) if p.is_absolute() else str(p)
        worst(out, rel.removeprefix("./"), outcome)
    return no_file


def from_python(pattern: str, root: pathlib.Path, out: dict[str, str]) -> None:
    for script in sorted(glob.glob(str(root / pattern))):
        rc = subprocess.run(
            [sys.executable, script], cwd=root, capture_output=True, text=True
        ).returncode
        rel = str(pathlib.Path(script).resolve().relative_to(root.resolve()))
        worst(out, rel, "passed" if rc == 0 else "failed")


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--cargo", action="append", default=[])
    ap.add_argument("--junit", action="append", default=[])
    ap.add_argument("--run-python", action="append", default=[])
    ap.add_argument("--root", default=".")
    a = ap.parse_args(argv)
    if not (a.cargo or a.junit or a.run_python):
        ap.error("give at least one of --cargo, --junit, --run-python")
    root = pathlib.Path(a.root)
    out: dict[str, str] = {}
    for c in a.cargo:
        text = sys.stdin.read() if c == "-" else pathlib.Path(c).read_text()
        # A log with no test targets in it is not a clean run. MEASURED
        # 2026-09-23: `gh run view --log` exited 0 with an EMPTY log, and this
        # printed "files: 0" — the reading an all-clear and an empty input
        # share. Refuse it by name instead.
        if not any(RUNNING.match(line) for line in ANSI.sub("", text).splitlines()):
            print(
                f"refused: {c} holds no cargo test target at all (no 'Running' line) — "
                "an empty or truncated log is not a run. For a GitHub Actions job, "
                "`gh api repos/<owner>/<repo>/actions/jobs/<id>/logs` returns the log "
                "when `gh run view --log` comes back empty.",
                file=sys.stderr,
            )
            return 2
        for u in from_cargo(text, root, out):
            print(f"note: could not place cargo target {u}", file=sys.stderr)
    for j in a.junit:
        n = from_junit(j, root, out)
        if n:
            print(f"note: {n} JUnit testcase(s) name no file and were not placed", file=sys.stderr)
    for g in a.run_python:
        from_python(g, root, out)
    json.dump(
        [{"location": k, "outcome": v} for k, v in sorted(out.items())],
        sys.stdout,
        indent=1,
    )
    print()
    counts = {o: sum(1 for v in out.values() if v == o) for o in RANK}
    print(f"files: {len(out)} — {counts}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
