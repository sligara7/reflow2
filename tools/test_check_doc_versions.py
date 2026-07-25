#!/usr/bin/env python3
"""Tests for the doc-version drift checker.

The check itself is small; what it must never do is pass quietly. These pin the
two behaviours that make it worth having:

  - a version claim that disagrees with the build FAILS, and
  - a pattern that matches NOTHING also fails.

The second is the one that matters. The upstream script this is adapted from
greps with `|| true`, so rewording a doc line disables its check with no signal —
documented there as known brittleness, and forbidden here by
`req:no-silent-fallback`. A checker that can be switched off by editing prose
will be, eventually, by someone who never knew it was watching.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

TOOL = Path(__file__).resolve().parent / "check_doc_versions.py"
REPO = TOOL.parent.parent


def run() -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(TOOL)], capture_output=True, text=True, cwd=REPO
    )


def current_version() -> str:
    """The workspace version, read the same way the checker reads it.

    Derived rather than hardcoded, because a fixture pinned to a literal version
    fails on every release — which is exactly what happened cutting v0.11.0, and
    a test that breaks on release day is a test people learn to edit rather than
    trust.
    """
    for line in (REPO / "Cargo.toml").read_text().splitlines():
        if line.strip().startswith("version = "):
            return line.split('"')[1]
    raise AssertionError("no workspace version in Cargo.toml")


CLAIM = f"**Shipping at v{current_version()}.**"


def test_the_repo_as_it_stands_passes() -> None:
    r = run()
    assert r.returncode == 0, f"the committed docs must agree with the build:\n{r.stdout}"
    assert "OK" in r.stdout


def test_a_stale_claim_is_caught() -> None:
    agents = REPO / "AGENTS.md"
    original = agents.read_text()
    try:
        agents.write_text(original.replace(CLAIM, "**Shipping at v0.0.1.**"))
        r = run()
        assert r.returncode == 1, "a stale version claim must fail the build"
        assert "DRIFT" in r.stdout
        assert "0.0.1" in r.stdout and current_version() in r.stdout, (
            "the report must name BOTH what the prose says and what the build "
            f"says, or nobody can act on it:\n{r.stdout}"
        )
    finally:
        agents.write_text(original)


def test_rewording_the_prose_fails_loudly_rather_than_disabling_the_check() -> None:
    # THE test. This is the whole reason this exists rather than a `grep || true`.
    agents = REPO / "AGENTS.md"
    original = agents.read_text()
    try:
        agents.write_text(
            original.replace(CLAIM, f"**Currently shipping v{current_version()}.**")
        )
        r = run()
        assert r.returncode == 1, (
            "a pattern that matches nothing must FAIL — a silently skipped check "
            "is indistinguishable from a passing one"
        )
        assert "UNMATCHED" in r.stdout
        assert "reworded" in r.stdout, (
            "and it must say what to do about it, since the person who hits this "
            f"is mid-edit and did nothing wrong:\n{r.stdout}"
        )
    finally:
        agents.write_text(original)


def main() -> int:
    tests = [
        test_the_repo_as_it_stands_passes,
        test_a_stale_claim_is_caught,
        test_rewording_the_prose_fails_loudly_rather_than_disabling_the_check,
    ]
    failed = 0
    for t in tests:
        try:
            t()
            print(f"PASS  {t.__name__}")
        except AssertionError as e:
            print(f"FAIL  {t.__name__}: {e}")
            failed += 1
    print(f"\n{len(tests) - failed}/{len(tests)} passed")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
