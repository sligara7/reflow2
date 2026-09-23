#!/usr/bin/env python3
"""The run-to-files converter's own net: a real run must reach the design as
per-file outcomes, and nothing it cannot place may read as a pass."""

from __future__ import annotations

import pathlib
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import test_run_to_files as conv  # noqa: E402


def tree() -> pathlib.Path:
    root = pathlib.Path(tempfile.mkdtemp(prefix="run2files-"))
    for crate in ("alpha-core", "alpha-mcp"):
        d = root / "crates" / crate
        (d / "src").mkdir(parents=True)
        (d / "tests").mkdir()
        (d / "Cargo.toml").write_text("[package]\n")
        (d / "src" / "lib.rs").write_text("")
    (root / "crates/alpha-core/tests/good.rs").write_text("")
    (root / "crates/alpha-mcp/tests/bad.rs").write_text("")
    (root / "crates/alpha-mcp/tests/broken.rs").write_text("")
    # build output must never be searched or matched
    (root / "target/debug").mkdir(parents=True)
    (root / "target/debug/Cargo.toml").write_text("")
    return root


ESC = "\x1b[1m\x1b[92m"
CARGO = f"""\
{ESC}     Running\x1b[0m unittests src/lib.rs (target/debug/deps/alpha_core-0123abcd)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running unittests src/lib.rs (target/debug/deps/alpha_mcp-4567abcd)
test result: ok. 1 passed; 0 failed
{ESC}     Running\x1b[0m tests/good.rs (target/debug/deps/good-89abcdef)
test result: ok. 2 passed; 0 failed
     Running tests/bad.rs (target/debug/deps/bad-89abcdef)
test result: FAILED. 1 passed; 1 failed
error: 2 targets failed:
    `-p alpha-mcp --test bad`
    `-p alpha-mcp --test broken`
"""


class Cargo(unittest.TestCase):
    def setUp(self):
        self.root = tree()
        self.out: dict[str, str] = {}
        self.unresolved = conv.from_cargo(CARGO, self.root, self.out)

    def test_each_target_is_placed_in_its_own_crate(self):
        self.assertEqual(self.out["crates/alpha-core/src/lib.rs"], "passed")
        self.assertEqual(self.out["crates/alpha-mcp/src/lib.rs"], "passed")
        self.assertEqual(self.out["crates/alpha-core/tests/good.rs"], "passed")
        self.assertEqual(self.unresolved, [])

    def test_colour_codes_from_a_ci_log_do_not_hide_a_target(self):
        # The two coloured lines are the first unittests and good.rs.
        self.assertIn("crates/alpha-core/tests/good.rs", self.out)

    def test_a_failed_target_is_failed(self):
        self.assertEqual(self.out["crates/alpha-mcp/tests/bad.rs"], "failed")

    def test_a_target_that_never_printed_a_result_is_failed_not_dropped(self):
        self.assertEqual(self.out["crates/alpha-mcp/tests/broken.rs"], "failed")

    def test_build_output_is_never_matched(self):
        self.assertFalse(any(k.startswith("target/") for k in self.out))


class Junit(unittest.TestCase):
    def test_worst_outcome_per_file_and_unplaced_cases_are_counted(self):
        root = tree()
        xml = root / "r.xml"
        xml.write_text(
            """<testsuite>
  <testcase classname="a" name="one" file="tests/a.py"/>
  <testcase classname="a" name="two" file="tests/a.py"><failure/></testcase>
  <testcase classname="b" name="one" file="./tests/b.py"><skipped/></testcase>
  <testcase classname="c" name="nofile"/>
</testsuite>"""
        )
        out: dict[str, str] = {}
        no_file = conv.from_junit(str(xml), root, out)
        self.assertEqual(out, {"tests/a.py": "failed", "tests/b.py": "skipped"})
        self.assertEqual(no_file, 1)


class Python(unittest.TestCase):
    def test_exit_code_is_the_result(self):
        root = tree()
        (root / "tools").mkdir()
        (root / "tools/test_ok.py").write_text("raise SystemExit(0)\n")
        (root / "tools/test_no.py").write_text("raise SystemExit(1)\n")
        out: dict[str, str] = {}
        conv.from_python("tools/test_*.py", root, out)
        self.assertEqual(out, {"tools/test_no.py": "failed", "tools/test_ok.py": "passed"})


class Worst(unittest.TestCase):
    def test_a_later_pass_never_hides_an_earlier_failure(self):
        out: dict[str, str] = {}
        conv.worst(out, "f", "failed")
        conv.worst(out, "f", "passed")
        conv.worst(out, "f", "skipped")
        self.assertEqual(out["f"], "failed")


if __name__ == "__main__":
    unittest.main(verbosity=2)
