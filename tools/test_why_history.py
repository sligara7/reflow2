#!/usr/bin/env python3
"""Tests for tools/why_history.py — a git history read as the `why` skill's
agenda.

Hermetic and stdlib-only, and needs no reflow2 binary: each case builds a
small real git repository with a known history, writes a design export by hand
(the script reads the export as a file, never through a server), and runs the
script as a subprocess. The contract pinned here:

- the MAIN LINE is what is read: a merged branch arrives as ONE change carrying
  every commit it brought in, and a `(#12)` pull-request commit is one change;
- consecutive direct commits by one author, close in time and in the same area,
  are one change; a different author, a gap of days or a different area splits;
- noise is dropped BY RULE and counted by rule — a bot, a lockfile-only bump,
  a CI-only edit, a formatting pass, a small typo fix — and a lockfile riding
  along with real work never hides the work;
- a revert is flagged and is never noise;
- a change is EXPLAINED when a ChangeEvent in the export names one of its
  commits, WAITING when an open follow-up quotes one, and REVIEWED when it sits
  behind the feature's cursor with neither — so coverage is computed from the
  design, never kept in a second record;
- `--feature` finds the feature's folders from the Artifacts that realize it,
  and its cursor from its newest open `why_cursor` finding;
- it refuses (exit 2) rather than guessing when it cannot answer.
"""

from __future__ import annotations

import json
import os
import pathlib
import subprocess
import sys
import tempfile
import unittest

DRIVER = pathlib.Path(__file__).resolve().parent / "why_history.py"

OWNER = ("Dana Owner", "dana@example.com")
COLLEAGUE = ("Sam Colleague", "sam@example.com")
BOT = ("dependabot[bot]", "49699333+dependabot[bot]@users.noreply.github.com")


class WhyHistory(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory(prefix="why-history-test-")
        self.root = pathlib.Path(self._tmp.name)
        self.git("init", "-q", "-b", "main")
        self.git("config", "commit.gpgsign", "false")
        self.day = 0

    def tearDown(self):
        self._tmp.cleanup()

    # -- building a history ------------------------------------------------

    def git(self, *args: str, author=OWNER, date: str | None = None) -> str:
        env = dict(os.environ)
        env.update({
            "GIT_AUTHOR_NAME": author[0], "GIT_AUTHOR_EMAIL": author[1],
            "GIT_COMMITTER_NAME": author[0], "GIT_COMMITTER_EMAIL": author[1],
        })
        if date:
            env["GIT_AUTHOR_DATE"] = env["GIT_COMMITTER_DATE"] = date
        out = subprocess.run(["git", *args], cwd=self.root, env=env,
                             capture_output=True, text=True, check=True)
        return out.stdout.strip()

    def commit(self, message: str, files: dict[str, str], author=OWNER, days_later: float = 3) -> str:
        """Write `files`, commit them `days_later` after the previous commit,
        and return the new commit's id."""
        self.day += days_later
        for path, text in files.items():
            p = self.root / path
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(text)
            self.git("add", path)
        self.git("commit", "-q", "-m", message, author=author, date=self._date())
        return self.git("rev-parse", "HEAD")

    def _date(self) -> str:
        seconds = 1609459200 + int(self.day * 86400)  # 2021-01-01 + day offset
        return f"@{seconds} +0000"

    def run_driver(self, *args: str) -> subprocess.CompletedProcess:
        return subprocess.run([sys.executable, str(DRIVER), "--repo", str(self.root), *args],
                              capture_output=True, text=True)

    def json_of(self, *args: str) -> dict:
        run = self.run_driver(*args, "--json")
        self.assertEqual(run.returncode, 0, run.stderr)
        return json.loads(run.stdout)

    def export(self, nodes: list[dict], edges: list[dict] | None = None) -> str:
        path = self.root.parent / f"{self.root.name}-design.json"
        path.write_text(json.dumps({"nodes": nodes, "edges": edges or []}))
        self.addCleanup(lambda: path.unlink(missing_ok=True))
        return str(path)

    def standard_history(self) -> dict[str, str]:
        """One feature (src/export) with a real history, plus every kind of noise."""
        c = {}
        c["add"] = self.commit("Add CSV export", {"src/export/csv.py": "v1\n"})
        c["pdf"] = self.commit("Add PDF export", {"src/export/pdf.py": "v1\n"}, days_later=0.1)
        c["bot"] = self.commit("Bump lodash", {"package-lock.json": "{}\n"}, author=BOT)
        c["lock"] = self.commit("Update dependencies", {"Cargo.lock": "x\n"})
        c["ci"] = self.commit("Faster CI", {".github/workflows/ci.yml": "on: push\n"})
        c["fmt"] = self.commit("style: run prettier", {"src/export/csv.py": "v1 \n"})
        c["typo"] = self.commit("Fix typo in label", {"src/export/pdf.py": "v1.\n"})
        c["tz"] = self.commit("Store export times in local time", {"src/export/csv.py": "local\n"})
        self.git("revert", "--no-edit", c["tz"], date=self._date())
        c["revert"] = self.git("rev-parse", "HEAD")
        c["split"] = self.commit("Split the CSV into two files", {"src/export/csv2.py": "two\n"},
                                 author=COLLEAGUE)
        c["mixed"] = self.commit("Move export button, refresh lockfile",
                                 {"src/export/button.py": "file menu\n", "Cargo.lock": "y\n"})
        c["other"] = self.commit("Tweak login screen", {"src/login/screen.py": "v2\n"})
        return c

    # -- the contract ------------------------------------------------------

    def test_noise_is_dropped_by_rule_and_counted_by_rule(self):
        self.standard_history()
        s = self.json_of()
        self.assertEqual(s["noise_by_rule"],
                         {"bot": 1, "ci": 1, "dependencies": 1, "formatting": 1, "typo": 1})
        self.assertEqual(s["noise"], 5)
        self.assertEqual(s["to_ask_about"], s["changes"] - 5)

    def test_a_lockfile_riding_along_with_real_work_never_hides_the_work(self):
        c = self.standard_history()
        p = self.json_of("page", "--paths", "src/export", "--size", "50")
        mixed = [ch for ch in p["page"] if c["mixed"] in ch["commits"]]
        self.assertEqual(len(mixed), 1)
        self.assertIsNone(mixed[0]["noise"])

    def test_a_revert_is_flagged_and_is_never_noise(self):
        c = self.standard_history()
        s = self.json_of()
        self.assertEqual(s["reverts"], 1)
        p = self.json_of("page", "--paths", "src/export", "--size", "50")
        rev = [ch for ch in p["page"] if c["revert"] in ch["commits"]][0]
        self.assertTrue(rev["revert"])
        self.assertEqual(rev["reverts"], c["tz"])

    def test_close_commits_by_one_author_in_one_area_are_one_change(self):
        c = self.standard_history()
        p = self.json_of("page", "--paths", "src/export", "--size", "50")
        first = p["page"][0]
        self.assertEqual(first["commits"], [c["add"], c["pdf"]],
                         "two commits a few hours apart by the same person are one change")
        self.assertEqual(first["subject"], "Add CSV export (+1 more)")
        others = [ch for ch in p["page"] if c["split"] in ch["commits"]]
        self.assertEqual(others[0]["authors"], ["Sam Colleague"])

    def test_a_merged_branch_is_one_change_carrying_every_commit_it_brought(self):
        base = self.commit("Start", {"src/export/csv.py": "v1\n"})
        self.git("checkout", "-q", "-b", "feature")
        a = self.commit("Step one", {"src/export/a.py": "a\n"})
        b = self.commit("Step two", {"src/export/b.py": "b\n"})
        self.git("checkout", "-q", "main")
        self.git("merge", "-q", "--no-ff", "feature", "-m",
                 "Merge pull request #12 from dana/feature\n\nAdd the two-step export",
                 date=self._date())
        squash = self.commit("Add export history (#13)", {"src/export/h.py": "h\n"}, days_later=0.1)
        p = self.json_of("page", "--paths", "src/export")
        kinds = [(ch["kind"], ch["pr"]) for ch in p["page"]]
        self.assertEqual(kinds, [("direct", None), ("merge", "#12"), ("pull_request", "#13")])
        merge = p["page"][1]
        self.assertEqual(merge["subject"], "Add the two-step export")
        self.assertIn(a, merge["commits"])
        self.assertIn(b, merge["commits"])
        self.assertEqual(p["page"][2]["commits"], [squash])
        self.assertEqual(p["page"][0]["commits"], [base])

    def test_coverage_is_computed_from_the_design_explained_waiting_reviewed_left(self):
        c = self.standard_history()
        design = self.export([
            {"node_type": "ChangeEvent", "node_id": "chg:why-export-add",
             "properties": {"commits": c["pdf"][:9], "rationale_basis": "recalled"}},
            {"node_type": "TemporalFact", "node_id": "fact:follow-up-ask-sam",
             "properties": {"fact_type": "follow_up",
                            "statement": f"Ask Sam why the CSV was split (commit {c['split'][:7]})"}},
        ])
        p = self.json_of("page", "--paths", "src/export", "--export", design,
                         "--after", c["revert"][:10])
        cov = p["coverage"]
        # Seven changes touch src/export: add+pdf, fmt, typo, tz, revert, split, mixed.
        self.assertEqual(cov["changes"], 7)
        self.assertEqual(cov["noise"], 2)
        self.assertEqual(cov["explained"], 1, "a ChangeEvent naming ANY commit of a change explains it")
        self.assertEqual(cov["waiting"], 1)
        self.assertEqual(cov["reviewed_no_record"], 2, "tz and the revert sit behind the cursor")
        self.assertEqual(cov["left"], 1)
        self.assertEqual([ch["commits"] for ch in p["page"]], [[c["mixed"]]])

    def test_a_feature_is_found_by_its_capability_its_folders_and_its_cursor(self):
        c = self.standard_history()
        design = self.export(
            [
                {"node_type": "Capability", "node_id": "cap:export", "properties": {"name": "Export"}},
                {"node_type": "Artifact", "node_id": "art:export-dir",
                 "properties": {"location": "src/export"}},
                {"node_type": "TemporalFact", "node_id": "fact:why-cursor-export-old",
                 "properties": {"fact_type": "why_cursor", "subject_id": "cap:export",
                                "valid_from": "2026-01-01", "value": json.dumps({"after": c["add"]})}},
                {"node_type": "TemporalFact", "node_id": "fact:why-cursor-export",
                 "properties": {"fact_type": "why_cursor", "subject_id": "cap:export",
                                "valid_from": "2026-02-01", "value": json.dumps({"after": c["tz"]})}},
            ],
            [{"edge_type": "REALIZES", "from_id": "art:export-dir", "to_id": "cap:export"}],
        )
        p = self.json_of("page", "--feature", "cap:export", "--export", design)
        self.assertEqual(p["paths"], ["src/export"])
        self.assertEqual(p["after"], c["tz"], "the NEWEST open cursor wins")
        self.assertEqual(p["page"][0]["commits"], [c["revert"]])
        s = self.json_of("--export", design)
        self.assertEqual(s["features_walked"], 1)

    def test_the_page_is_numbered_and_says_what_comes_next(self):
        self.standard_history()
        p = self.json_of("page", "--paths", "src/export", "--size", "2")
        self.assertEqual([ch["n"] for ch in p["page"]], [1, 4])
        self.assertTrue(p["more"])
        run = self.run_driver("page", "--paths", "src/export", "--size", "2", "--me", "Dana")
        self.assertIn(f"--after {p['next_after']}", run.stdout)
        self.assertNotIn("(Dana Owner)", run.stdout, "--me hides your own name, not others'")

    def test_a_utc_date_written_with_z_is_read_on_every_supported_python(self):
        # CI, 2026-09-23: git on the runner prints UTC as `...Z`, and Python
        # before 3.11 refuses that in datetime.fromisoformat — every case here
        # crashed on ubuntu-22.04 while passing on a laptop whose git prints
        # `+00:00`. Pinned at the parser, so it fails on 3.10 whatever git does.
        sys.path.insert(0, str(DRIVER.parent))
        import why_history
        z = why_history.when({"date": "2021-01-04T02:24:00Z"})
        offset = why_history.when({"date": "2021-01-04T02:24:00+00:00"})
        self.assertEqual(z, offset)

    def test_it_refuses_rather_than_guesses(self):
        self.standard_history()
        self.assertEqual(self.run_driver("page").returncode, 2, "a page with no feature")
        bad = self.run_driver("page", "--paths", "src/export", "--after", "deadbeef")
        self.assertEqual(bad.returncode, 2, "a cursor that names no change is not 'nothing left'")
        design = self.export([])
        nowhere = self.run_driver("page", "--feature", "cap:nowhere", "--export", design)
        self.assertEqual(nowhere.returncode, 2)
        self.assertIn("link_artifact", nowhere.stderr)
        not_git = subprocess.run([sys.executable, str(DRIVER), "--repo", tempfile.gettempdir()],
                                 capture_output=True, text=True)
        self.assertEqual(not_git.returncode, 2)


if __name__ == "__main__":
    unittest.main(verbosity=2)
