#!/usr/bin/env python3
"""Assert that version claims in reflow2's docs match what the build actually uses.

The drift class: a bump lands in `Cargo.toml` and the prose still advertises the
previous one. Cargo pins dependencies and enforces it; nothing enforces a
sentence. So a reader orienting from `AGENTS.md` gets a confident, wrong answer —
and confidently wrong is worse than absent, because nobody goes to check.

Adapted from dynograph-foundation's `scripts/check-doc-versions.sh`, WITH ONE
DELIBERATE DIFFERENCE. That script greps with `|| true`, so a pattern matching
nothing produces no output and the check passes; its own header documents this as
known brittleness — "reworking any of those phrases in the docs disables the
corresponding check silently". reflow2 holds `req:no-silent-fallback` at critical
priority, so here **a pattern that matches nothing is a FAILURE**. Rewording a
doc line breaks the build loudly and you fix both sides together, which is the
whole point: a check that can be disabled by editing prose is a check that will
be, eventually, by someone who never knew it existed.

Targeted patterns only — never a generic "any X.Y.Z" scan. Historical references
("the nine releases up to v0.10.1", "as of the v0.5.0 as-released work") are
CORRECT prose about the past and must not be rewritten by a version bump. That is
why each claim is anchored on a literal phrase rather than matched by shape.

Usage:  python3 tools/check_doc_versions.py
Exit 0 = every targeted claim agrees with the build.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent

# --------------------------------------------------------------------------
# What the build actually says. These are the facts every doc claim is checked
# against; read from the manifests rather than passed in, so the checker cannot
# be run against a version somebody asserted on the command line.
# --------------------------------------------------------------------------


def reflow2_version() -> str:
    """reflow2's own workspace version."""
    text = (REPO / "Cargo.toml").read_text()
    m = re.search(r'^\[workspace\.package\](?:.|\n)*?^version = "([^"]+)"', text, re.M)
    if not m:
        sys.exit("FAIL: could not read the workspace version from Cargo.toml")
    return m.group(1)


def foundation_tag() -> str:
    """The dynograph-foundation tag the absorbed code was taken FROM.

    Until 2026-08-24 this read a git-tag pin out of the workspace `Cargo.toml`.
    `dec:absorb-the-foundation-subset-and-end-the-dependency` removed the last
    pin, so that question has no answer any more — but the FACT it protected
    survives the move, because the decision requires every absorbed module to
    carry a provenance header naming the tag and files it took. That header is
    now the source of truth, and it is still read from the tree rather than from
    prose, which is the property that made this check worth having.

    Every absorbed module must name ONE tag, for the same reason five pins had
    to agree: modules taken from different vintages of the same repository is a
    real defect, so disagreement is reported rather than resolved by taking the
    first. **Finding no header at all is also a failure** — that is the objection
    the decision recorded against absorbing anything (vendoring turns a visible
    dependency into an invisible one), and a silently-passing check here is
    exactly how the header would rot away unnoticed.
    """
    headers = sorted((REPO / "crates/reflow2-core/src").rglob("*.rs"))
    tags: dict[str, list[str]] = {}
    for path in headers:
        for tag in re.findall(r"^//! tag\s+(v[0-9]+\.[0-9]+\.[0-9]+)\s*$", path.read_text(), re.M):
            tags.setdefault(tag, []).append(str(path.relative_to(REPO)))
    if not tags:
        sys.exit(
            "FAIL: no absorbed-code provenance header names a dynograph-foundation "
            "tag. dec:absorb-the-foundation-subset-and-end-the-dependency REQUIRES "
            "one in every absorbed module — see crates/reflow2-core/src/foundation/mod.rs"
        )
    if len(tags) > 1:
        detail = "; ".join(f"{t}: {', '.join(f)}" for t, f in sorted(tags.items()))
        sys.exit(
            "FAIL: absorbed modules name DIFFERENT foundation tags "
            f"({detail}) — code taken from mixed vintages of one repository"
        )
    return next(iter(tags))


# --------------------------------------------------------------------------
# The claims. Each is (file, label, regex, expected). The regex must capture the
# version in group 1, and must be anchored on a literal phrase from the prose so
# that historical mentions nearby are not swept up.
# --------------------------------------------------------------------------


def claims(reflow2: str, foundation: str) -> list[tuple[str, str, str, str]]:
    return [
        (
            "AGENTS.md",
            "foundation provenance prose",
            r"absorbed from it at `(v[0-9]+\.[0-9]+\.[0-9]+)`",
            foundation,
        ),
        (
            "AGENTS.md",
            "current-state shipping version",
            r"\*\*Shipping at v([0-9]+\.[0-9]+\.[0-9]+)\.\*\*",
            reflow2,
        ),
        (
            "AGENTS.md",
            "loop-status shipping version",
            r"is built and shipping as of\s*\n?v([0-9]+\.[0-9]+\.[0-9]+)",
            reflow2,
        ),
    ]


def main() -> int:
    reflow2 = reflow2_version()
    foundation = foundation_tag()
    print(f"reflow2 {reflow2} · absorbed from dynograph-foundation {foundation}\n")

    failures: list[str] = []
    for filename, label, pattern, expected in claims(reflow2, foundation):
        path = REPO / filename
        if not path.exists():
            failures.append(f"MISSING FILE: {filename} (claim '{label}' cannot be checked)")
            continue
        text = path.read_text()
        found = re.findall(pattern, text)

        # The difference from the upstream script: no matches is a FAILURE.
        # A silently-skipped check is indistinguishable from a passing one, and
        # that is exactly how a doc line rots after someone rewords it.
        if not found:
            failures.append(
                f"UNMATCHED: {filename} ({label}) — the pattern found nothing. "
                f"Either the prose was reworded (update the pattern in "
                f"tools/check_doc_versions.py alongside it) or the claim was "
                f"deleted (remove it here). A check that matches nothing is not "
                f"a check that passes."
            )
            continue

        for got in found:
            if got.lstrip("v") != expected.lstrip("v"):
                failures.append(
                    f"DRIFT: {filename} ({label}) — says {got}, build says {expected}"
                )

    if failures:
        print("\n".join(failures))
        print(f"\ncheck_doc_versions: FAILED — {len(failures)} finding(s).")
        return 1

    n = len(claims(reflow2, foundation))
    print(f"check_doc_versions: OK — {n} targeted claim(s) agree with the build.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
