#!/usr/bin/env python3
"""Tests for tools/check_kit_manifest.py — `ver:kit-manifest-agrees`.

Hermetic and stdlib-only. Each case builds a synthetic project whose stamp is
constructed FROM THE REAL KIT SOURCES, so agreement means agreement with what
this repo actually ships rather than with a fixture that could drift from it.

The cases that matter most are the ones asserting the check stays QUIET. A gate
that fires on everything is indistinguishable from a gate that fires on nothing —
both get switched off — and this one has to survive being wired into CI, where a
false positive costs every contributor.
"""

from __future__ import annotations

import hashlib
import json
import pathlib
import subprocess
import sys
import tempfile
import unittest

REPO = pathlib.Path(__file__).resolve().parent.parent
SCRIPT = REPO / "tools" / "check_kit_manifest.py"
sys.path.insert(0, str(REPO / "tools"))
import check_kit_manifest as chk  # noqa: E402
import reflow2_init as kit  # noqa: E402


def run(project: pathlib.Path):
    return subprocess.run([sys.executable, str(SCRIPT), str(project)],
                          capture_output=True, text=True, timeout=60)


def sha(p: pathlib.Path) -> str:
    return hashlib.sha256(p.read_bytes()).hexdigest()


class KitManifest(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory(prefix="kit-manifest-test-")
        self.project = pathlib.Path(self._tmp.name)
        (self.project / ".reflow2").mkdir()

    def tearDown(self):
        self._tmp.cleanup()

    def agreeing_manifest(self) -> dict:
        """Exactly what a current install of this kit would record."""
        out = {}
        for rel, src in chk.shipped_sources().items():
            # The project here owns nothing, so no sidecar substitution.
            out[rel] = sha(src)
        return out

    def write_stamp(self, **over):
        source = kit.kit_version()
        stamp = {
            "reflow2_version": source.get("version") or source.get("reflow2_version"),
            "commit": source.get("commit"),
            "installed_files": self.agreeing_manifest(),
        }
        stamp.update(over)
        (self.project / kit.STAMP).write_text(json.dumps(stamp, indent=2) + "\n")
        return stamp

    # ---- the quiet cases ------------------------------------------------

    def test_an_agreeing_installation_passes(self):
        # THE ONE THAT KEEPS THE GATE USABLE. Built from the real kit sources, so
        # if this ever fails the kit and the checker have genuinely disagreed.
        self.write_stamp()
        r = run(self.project)
        self.assertEqual(r.returncode, 0, r.stdout)
        self.assertIn("OK", r.stdout)

    def test_a_project_with_no_stamp_is_not_a_failure(self):
        # A project nobody installed into cannot have a stale manifest. Exit 2,
        # distinct from both pass and fail, because "nothing to check" is a third
        # answer and collapsing it into either one is a lie.
        r = run(self.project)
        self.assertEqual(r.returncode, 2)
        self.assertIn("nothing installed", r.stdout)

    def test_a_pre_manifest_install_is_noted_not_failed(self):
        # Installs before BL-54 recorded no file list. There is nothing to check
        # and one install closes it, so this is context and not drift.
        self.write_stamp(installed_files={})
        r = run(self.project)
        self.assertEqual(r.returncode, 0, r.stdout)
        self.assertIn("pre-manifest install", r.stdout)

    def test_a_recorded_file_absent_from_disk_is_a_note_not_a_finding(self):
        # Deliberate boundary: whether files are still ON DISK is the INSTALL's
        # business — place_kit_file reads a mismatch as the user's edits and keeps
        # them. This check compares the manifest to the KIT, not to the tree.
        self.write_stamp()
        r = run(self.project)
        self.assertEqual(r.returncode, 0, r.stdout)
        self.assertIn("not on disk", r.stdout)
        self.assertNotIn("FAIL", r.stdout)

    def test_the_sidecar_is_accepted_for_the_file_it_stands_in_for(self):
        # When a project already owns AGENTS.md, kit content lands at REFLOW2.md,
        # and the manifest records THAT name. Without this the checker would
        # demand a path no install ever writes.
        #
        # POSITIVE CONTROL FIRST: the substitution must actually be under test,
        # or this passes by describing a case that never arises.
        self.assertEqual(chk.sidecar_of("AGENTS.md"), "REFLOW2.md",
                         "positive control: AGENTS.md must have a sidecar")
        manifest = self.agreeing_manifest()
        src = dict(chk.shipped_sources())["AGENTS.md"]
        del manifest["AGENTS.md"]
        manifest["REFLOW2.md"] = sha(src)
        self.write_stamp(installed_files=manifest)
        r = run(self.project)
        self.assertEqual(r.returncode, 0, r.stdout)
        self.assertNotIn("AGENTS.md", r.stdout)

    # ---- the three findings ---------------------------------------------

    def test_a_stale_version_is_caught(self):
        # THE ORIGINAL FRICTION, in one assertion: reflow2's own manifest was
        # four releases stale and nothing noticed.
        self.write_stamp(reflow2_version="0.0.1")
        r = run(self.project)
        self.assertEqual(r.returncode, 1)
        self.assertIn("stale_version", r.stdout)
        self.assertIn("0.0.1", r.stdout)

    def test_a_shipped_file_missing_from_the_manifest_is_caught(self):
        manifest = self.agreeing_manifest()
        victim = sorted(manifest)[0]
        del manifest[victim]
        self.write_stamp(installed_files=manifest)
        r = run(self.project)
        self.assertEqual(r.returncode, 1)
        self.assertIn("does not record it", r.stdout)
        self.assertIn(victim, r.stdout)

    def test_a_dead_manifest_entry_is_caught(self):
        # Retired by the kit AND absent from the tree, so the entry tracks
        # nothing — and the install's prune loop skips absent files, so it never
        # clears itself.
        manifest = self.agreeing_manifest()
        manifest[".claude/skills/adopt/SKILL.md"] = "0" * 64
        self.write_stamp(installed_files=manifest)
        r = run(self.project)
        self.assertEqual(r.returncode, 1)
        self.assertIn("not on disk", r.stdout)
        # And it explains the retirement rather than just naming it — why_gone
        # exists for the person watching their repo empty out.
        self.assertIn("served by the MCP server", r.stdout)

    def test_an_in_sync_skill_mirror_is_a_note_not_a_finding(self):
        # reflow2's own repo keeps `.claude/skills/` and `.grok/skills/` as
        # deliberate byte-identical mirrors of the kit source, enforced by
        # skill_lint. Flagging those would make this gate permanently red on the
        # one repository that maintains them correctly.
        rel = ".claude/skills/adopt/SKILL.md"
        source = chk.kit.KIT / "skills" / "adopt" / "SKILL.md"
        # POSITIVE CONTROL. The first cut resolved this path with a doubled
        # `skills/` segment, so it never existed and every mirror read as
        # drifted — the check "found" ten problems that were not there. A path
        # that cannot resolve must never look like a difference, so assert the
        # fixture is real before trusting the verdict.
        self.assertTrue(source.is_file(), f"positive control: {source} must exist")
        dst = self.project / rel
        dst.parent.mkdir(parents=True, exist_ok=True)
        dst.write_bytes(source.read_bytes())
        manifest = self.agreeing_manifest()
        manifest[rel] = sha(dst)
        self.write_stamp(installed_files=manifest)
        r = run(self.project)
        self.assertEqual(r.returncode, 0, r.stdout)
        self.assertIn("in-sync mirror", r.stdout)
        self.assertNotIn("FAIL", r.stdout)

    def test_a_drifted_skill_mirror_is_caught_as_shadowing(self):
        # THE CASE WITH A CONSEQUENCE: a harness auto-loads it, a served skill is
        # never offered when a local one exists, so a stale copy silently beats
        # every future release of that skill.
        rel = ".claude/skills/adopt/SKILL.md"
        dst = self.project / rel
        dst.parent.mkdir(parents=True, exist_ok=True)
        dst.write_text("---\nname: adopt\n---\nan old copy of this skill\n")
        manifest = self.agreeing_manifest()
        manifest[rel] = sha(dst)
        self.write_stamp(installed_files=manifest)
        r = run(self.project)
        self.assertEqual(r.returncode, 1)
        self.assertIn("shadowing", r.stdout)
        self.assertIn("dec:skills-served", r.stdout)

    def test_a_content_mismatch_is_caught(self):
        manifest = self.agreeing_manifest()
        victim = sorted(manifest)[0]
        manifest[victim] = "f" * 64
        self.write_stamp(installed_files=manifest)
        r = run(self.project)
        self.assertEqual(r.returncode, 1)
        self.assertIn("content:", r.stdout)
        self.assertIn("different kit", r.stdout)

    def test_an_unreadable_stamp_is_a_finding_not_a_crash(self):
        # It records what the kit owns. Unreadable means the next install cannot
        # tell its own files from the user's — the clobber decision has no input.
        (self.project / kit.STAMP).write_text("{ not json")
        r = run(self.project)
        self.assertEqual(r.returncode, 1)
        self.assertIn("unreadable", r.stdout)

    # ---- the checker's own honesty --------------------------------------

    def test_the_shipped_set_is_read_from_the_installer(self):
        # The defect class this file is about, reproduced INSIDE the checker: a
        # second copy of the shipped list would drift from the first. So assert
        # the checker sees what the installer ships, both halves.
        shipped = chk.shipped_sources()
        self.assertTrue(shipped, "positive control: the kit must ship something")
        for _, rel in kit.FILES:
            self.assertIn(rel, shipped, "the flat FILES list must be covered")
        for _, rel_dir in kit.TREES:
            self.assertTrue(any(r.startswith(f"{rel_dir}/") for r in shipped),
                            f"the {rel_dir} tree must be covered")
        for src in shipped.values():
            self.assertTrue(src.is_file(), f"{src} must exist to be hashed")

    def test_it_writes_nothing_into_the_project_it_checks(self):
        # Looking is not writing. A gate that mutates what it inspects cannot be
        # run to find out whether it would complain.
        self.write_stamp()
        before = {p: p.read_bytes() for p in self.project.rglob("*") if p.is_file()}
        run(self.project)
        after = {p: p.read_bytes() for p in self.project.rglob("*") if p.is_file()}
        self.assertEqual(before, after)


if __name__ == "__main__":
    unittest.main(verbosity=2)
