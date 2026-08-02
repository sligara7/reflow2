#!/usr/bin/env python3
"""The gate-list cross-check's own regression net (BL-159).

`skill_lint.py` now asserts that AGENTS.md's "A change is done when all of
these are clean" block and `.github/workflows/ci.yml` are two records of ONE
contract. The checks themselves run against the real files, which is the point;
what CANNOT be tested that way is the parsing underneath them, because the real
files only ever exercise the cases that happen to be present today.

These are hermetic and cover the traps — each one is a way the cross-check
could pass while the two lists disagreed, which would be worse than not having
it, since a lint that cannot fail reads as a guarantee.
"""

from __future__ import annotations

import pathlib
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import skill_lint  # noqa: E402


class TestGateIdentity(unittest.TestCase):
    """`_gate_identity` decides what counts as a gate and what to call it."""

    def test_a_script_stem_is_matched_exactly_not_as_a_substring(self):
        """THE TRAP, and it is live in this repo rather than hypothetical.

        `check_doc_versions.py` and `test_check_doc_versions.py` are two
        different suites and CI runs both. A substring match would see the
        second's name inside... no — it would see the FIRST's name inside the
        SECOND, and report the second as already covered. That is exactly the
        silence this check exists to break, so it is pinned.
        """
        a = skill_lint._gate_identity("python3 tools/check_doc_versions.py")
        b = skill_lint._gate_identity("python3 tools/test_check_doc_versions.py")
        self.assertEqual(a, "check_doc_versions")
        self.assertEqual(b, "test_check_doc_versions")
        self.assertNotEqual(a, b)
        self.assertIn(a, b, "the substring relationship is real — that is why exact matters")

    def test_cargo_gates_are_distinguished_by_package_and_scope(self):
        """`cargo test --workspace` and `cargo test -p reflow2-core
        --no-default-features` are different gates catching different breakage
        (the second is the in-memory backend on its own). Collapsing them to
        `cargo test` would let either stand in for the other."""
        ws = skill_lint._gate_identity("cargo test --workspace")
        core = skill_lint._gate_identity("cargo test -p reflow2-core --no-default-features")
        mcp = skill_lint._gate_identity("cargo clippy -p reflow2-mcp --all-targets -- -D warnings")
        self.assertEqual(ws, "cargo test --workspace")
        self.assertEqual(core, "cargo test -p reflow2-core")
        self.assertEqual(mcp, "cargo clippy -p reflow2-mcp")
        self.assertEqual(len({ws, core, mcp}), 3)

    def test_flags_are_dropped_from_the_identity_on_purpose(self):
        """Identity answers "is this documented at all"; the fidelity check
        answers "is it documented correctly". They must be separate, because the
        defect that filed BL-159 was a flags difference on a gate that WAS
        listed — folding flags into identity would report it as simply missing
        and folding it out of both would miss it entirely."""
        self.assertEqual(
            skill_lint._gate_identity("cargo clippy -p reflow2-core --all-targets"),
            skill_lint._gate_identity(
                "cargo clippy -p reflow2-core --no-default-features --all-targets -- -D warnings"
            ),
        )

    def test_setup_steps_are_not_gates(self):
        """A gate is something a change can be *clean* against. Installing a
        dependency and building the binary the instruments need are neither."""
        self.assertIsNone(skill_lint._gate_identity("python3 -m pip install --quiet pyyaml"))
        self.assertIsNone(skill_lint._gate_identity("cargo build -p reflow2-mcp"))

    def test_a_non_gate_line_returns_none_rather_than_a_junk_identity(self):
        for line in ("", "   ", "echo hello", "python3", "cargo"):
            with self.subTest(line=line):
                self.assertIsNone(skill_lint._gate_identity(line))


class TestCommentStripping(unittest.TestCase):
    def test_a_trailing_annotation_is_not_part_of_the_command(self):
        """The AGENTS.md block annotates its commands. Comparing the annotation
        as part of the command would make every documented gate look different
        from the way CI runs it, and the fidelity check would cry wolf on all of
        them at once — which is how a useful check gets switched off."""
        self.assertEqual(
            skill_lint._strip_comment("cargo test --workspace     # both crates"),
            "cargo test --workspace",
        )
        self.assertEqual(skill_lint._strip_comment("   cargo fmt --check  "), "cargo fmt --check")


class TestRealFilesAgree(unittest.TestCase):
    """The cross-check reads two real files. These assert the READS work — a
    parser that silently returned nothing would make every check vacuously
    pass, which is the failure mode the guards in `main()` refuse."""

    def test_ci_yml_yields_a_plausible_number_of_gates(self):
        gates = skill_lint.ci_gates()
        self.assertGreaterEqual(len(gates), 10, f"parsed only {len(gates)}: {sorted(gates)}")
        self.assertIn("cargo fmt", gates)
        self.assertIn("skill_lint", gates)

    def test_the_agents_block_is_found_and_non_empty(self):
        listed, omitted, found = skill_lint.documented_gates()
        self.assertTrue(found, "the anchor sentence or the fenced block moved")
        self.assertGreaterEqual(len(listed), 5)
        self.assertGreaterEqual(len(omitted), 5)

    def test_prose_in_a_later_paragraph_is_not_read_as_an_omission(self):
        """Found by this check failing on its own commit: the note under the
        block gained a paragraph explaining the lint, which mentions `cargo` and
        `python3` in backticks, and both were reported as gates CI had stopped
        running. Only the first paragraph names omissions."""
        _, omitted, _ = skill_lint.documented_gates()
        self.assertNotIn("cargo", omitted)
        self.assertNotIn("python3", omitted)


class TestFirstQuoteParagraph(unittest.TestCase):
    """The paragraph scoping, pinned on synthetic text.

    It needs its own cases because against the REAL AGENTS.md the
    `cargo`/`python3` exclusion happens to cover the same ground: removing the
    scoping alone leaves every check green. The two guards are not redundant in
    general — this one catches any other backticked word a later paragraph adds
    — so it is tested where the redundancy does not hide it.
    """

    QUOTE = (
        "> The full job also runs `phase_trial` and `test_init` — so green here\n"
        "> is not green there.\n"
        ">\n"
        "> **A later note** that mentions `toolsnap` and `smoke_mcp` in prose.\n"
        "\nOrdinary paragraph mentioning `check_doc_versions`.\n"
    )

    def test_only_the_first_paragraph_is_returned(self):
        para = skill_lint._first_quote_paragraph(self.QUOTE)
        self.assertIn("phase_trial", para)
        self.assertIn("test_init", para)
        self.assertNotIn("toolsnap", para, "a later quote paragraph leaked in")
        self.assertNotIn("smoke_mcp", para)
        self.assertNotIn("check_doc_versions", para, "text outside the quote leaked in")

    def test_a_bare_quote_marker_ends_the_paragraph(self):
        self.assertNotIn("later note", skill_lint._first_quote_paragraph(self.QUOTE))

    def test_text_before_the_quote_is_skipped_not_consumed(self):
        para = skill_lint._first_quote_paragraph("\n\nsome prose\n\n" + self.QUOTE)
        self.assertIn("phase_trial", para)
        self.assertNotIn("some prose", para)

    def test_no_quote_at_all_yields_nothing_rather_than_the_whole_document(self):
        self.assertEqual(skill_lint._first_quote_paragraph("no quote here\nat all\n"), "")

    def test_the_two_lists_do_not_overlap_pointlessly(self):
        """A gate cannot be both in the everyday subset and declared omitted
        from it. Not currently reachable through the checks in `main()`, so it
        is asserted here rather than left to be discovered."""
        listed, omitted, _ = skill_lint.documented_gates()
        both = sorted(set(listed) & omitted)
        self.assertFalse(both, f"listed AND declared omitted: {both}")


if __name__ == "__main__":
    unittest.main(verbosity=2)
