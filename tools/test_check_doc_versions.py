#!/usr/bin/env python3
"""Tests for the doc-version drift checker.

The check itself is small; what it must never do is pass quietly. These pin the
two behaviours that make it worth having:

  - a version claim that disagrees with the build FAILS,
  - a pattern that matches NOTHING also fails, and
  - the FACT every claim is measured against is itself read from the tree, and
    refuses to answer when the tree stops saying it.

The third arrived late, and the gap it closed is worth naming. When the
foundation was absorbed (2026-08-24) the git-tag pin this checker read vanished,
so the checker went red — correctly — and the fix repointed it at the provenance
headers the absorbed modules carry. THIS SUITE STAYED GREEN THROUGH THAT ENTIRE
REWRITE, because it only ever exercised the claim-matching half and never the
fact-reading half. A green suite said nothing about the function being replaced.

The second is the one that matters. The upstream script this is adapted from
greps with `|| true`, so rewording a doc line disables its check with no signal —
documented there as known brittleness, and forbidden here by
`req:no-silent-fallback`. A checker that can be switched off by editing prose
will be, eventually, by someone who never knew it was watching.
"""

from __future__ import annotations

import re
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


def provenance_files() -> list[Path]:
    """Every source file carrying an absorbed-code provenance tag line."""
    return [
        f
        for f in sorted((REPO / "crates/reflow2-core/src").rglob("*.rs"))
        if re.search(r"^//! tag\s+v[0-9]+\.[0-9]+\.[0-9]+\s*$", f.read_text(), re.M)
    ]


def test_the_provenance_headers_are_where_the_tag_comes_from() -> None:
    # Not a tautology: it pins WHICH files answer the question, so deleting the
    # last provenance header is a test failure and not a silent behaviour change.
    files = provenance_files()
    assert files, (
        "no absorbed module carries a provenance tag — "
        "dec:absorb-the-foundation-subset-and-end-the-dependency requires one"
    )
    r = run()
    assert r.returncode == 0
    assert "absorbed from dynograph-foundation" in r.stdout, r.stdout


def test_losing_every_provenance_header_fails_rather_than_passing_empty() -> None:
    # The objection the absorption decision recorded, made mechanical: vendoring
    # turns a visible dependency into an invisible one, and the headers are the
    # only successor to the pin's written record. If they rot away, say so.
    files = provenance_files()
    originals = {f: f.read_text() for f in files}
    try:
        for f, text in originals.items():
            f.write_text(re.sub(r"^//! tag\s+v[0-9.]+\s*$", "//! tag      (removed)", text, flags=re.M))
        r = run()
        assert r.returncode != 0, (
            "with no provenance header anywhere, the checker has nothing to "
            "measure doc claims against and must refuse rather than pass"
        )
        assert "no absorbed-code provenance header" in r.stdout + r.stderr
    finally:
        for f, text in originals.items():
            f.write_text(text)


def test_modules_absorbed_from_different_vintages_are_caught() -> None:
    # The successor to "five pins that drifted apart". Code taken from two
    # vintages of one repository is a real defect, so it must not be resolved
    # by quietly taking the first tag.
    files = provenance_files()
    assert len(files) > 1, "needs at least two provenance headers to disagree"
    victim = files[0]
    original = victim.read_text()
    try:
        victim.write_text(
            re.sub(r"^//! tag\s+v[0-9.]+\s*$", "//! tag      v9.9.9", original, flags=re.M)
        )
        r = run()
        assert r.returncode != 0, "mixed provenance vintages must fail"
        assert "DIFFERENT foundation tags" in r.stdout + r.stderr
    finally:
        victim.write_text(original)


def main() -> int:
    tests = [
        test_the_repo_as_it_stands_passes,
        test_a_stale_claim_is_caught,
        test_rewording_the_prose_fails_loudly_rather_than_disabling_the_check,
        test_the_provenance_headers_are_where_the_tag_comes_from,
        test_losing_every_provenance_header_fails_rather_than_passing_empty,
        test_modules_absorbed_from_different_vintages_are_caught,
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
