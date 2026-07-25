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
    """The dynograph-foundation git tag the workspace pins.

    Every foundation crate must sit on ONE tag. Five separate pins that drifted
    apart would be a real defect in its own right, so disagreement is reported
    here rather than silently resolved by taking the first.
    """
    text = (REPO / "Cargo.toml").read_text()
    tags = set(re.findall(r'dynograph-[a-z]+ = \{[^}]*tag = "(v[^"]+)"', text))
    if not tags:
        sys.exit("FAIL: could not find any dynograph-foundation tag pin in Cargo.toml")
    if len(tags) > 1:
        sys.exit(
            "FAIL: the foundation crates are pinned to DIFFERENT tags "
            f"({', '.join(sorted(tags))}) — they must move together"
        )
    return tags.pop()


# --------------------------------------------------------------------------
# The claims. Each is (file, label, regex, expected). The regex must capture the
# version in group 1, and must be anchored on a literal phrase from the prose so
# that historical mentions nearby are not swept up.
# --------------------------------------------------------------------------


def claims(reflow2: str, foundation: str) -> list[tuple[str, str, str, str]]:
    return [
        (
            "AGENTS.md",
            "foundation pin prose",
            r"`(v[0-9]+\.[0-9]+\.[0-9]+)` at time of writing",
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
    print(f"reflow2 {reflow2} · dynograph-foundation {foundation}\n")

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
