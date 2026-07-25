#!/usr/bin/env python3
"""Tests for reflow2_init.py — the consumer-kit installer (cap:kit).

The self-model's one long-standing true gap: nothing automated checked the
installer, and its failure history is exactly the silent kind this project
forbids — a stale build command in a pointer file (BL-26), the kit invisible
to the primary instruction file (F1), an existing MCP server blocking the
install while the run reported success (fixed in write_mcp_config).

stdlib only, like the installer itself. No network, no binary spawn: a fresh
temp project has no graph, so backup_graph is a no-op, and tests drive
install() directly rather than main() (whose staleness banner does a
git ls-remote). Run:  python3 tools/test_init.py
"""
from __future__ import annotations

import importlib.util
import json
import pathlib
import shutil
import tempfile
import unittest

HERE = pathlib.Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location("reflow2_init", HERE / "reflow2_init.py")
init = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(init)

# What actually lands in a project: the short, stable pointer. The full
# working instructions are served by get_instructions (req:thin-install).
KIT_AGENTS = (init.KIT / "POINTER.md").read_text()
FAKE_BINARY = pathlib.Path("/nonexistent/target/debug/reflow2-mcp")


class InstallerTest(unittest.TestCase):
    def setUp(self):
        self.tmp = pathlib.Path(tempfile.mkdtemp(prefix="reflow2-init-test-"))
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)

    def project(self, name="proj") -> pathlib.Path:
        p = self.tmp / name
        p.mkdir()
        return p

    def install(self, project, force_mcp=False):
        return init.install(project, FAKE_BINARY, force_mcp)

    # ---- the full kit lands ------------------------------------------------

    def test_fresh_install_lays_down_the_full_kit(self):
        p = self.project()
        self.install(p)

        self.assertEqual((p / "AGENTS.md").read_text(), KIT_AGENTS)
        # NO SKILLS ARE COPIED (dec:skills-served). They are compiled into the
        # binary and served by list_skills / get_skill, so an upgrade cannot
        # leave a project holding last release's skills — which is exactly what
        # it did, four releases running, without anything noticing.
        for tree in (".claude/skills", ".grok/skills"):
            self.assertFalse((p / tree).exists(), f"{tree} must not be installed")
        # All three MCP configs point the reflow2 entry at the given binary.
        mcp = json.loads((p / ".mcp.json").read_text())
        self.assertEqual(mcp["mcpServers"]["reflow2"]["command"], str(FAKE_BINARY))
        oc = json.loads((p / "opencode.json").read_text())
        self.assertEqual(oc["mcp"]["reflow2"]["command"][0], str(FAKE_BINARY))
        vs = json.loads((p / ".vscode" / "mcp.json").read_text())
        self.assertEqual(vs["servers"]["reflow2"]["command"], str(FAKE_BINARY))
        # Graph dir ignored, install stamped.
        self.assertIn(".reflow2/", (p / ".gitignore").read_text())
        stamp = json.loads((p / ".reflow2" / "kit-version.json").read_text())
        self.assertIn("reflow2_version", stamp)

    def test_an_upgrade_touches_nothing_in_the_project(self):
        # Re-running the SAME version changes nothing. Necessary, and weaker
        # than the requirement — test_a_new_release_touches_nothing below is
        # the one that actually answers Alex.
        p = self.project()
        self.install(p)
        before = {
            f.relative_to(p): f.read_bytes()
            for f in p.rglob("*")
            if f.is_file() and ".reflow2" not in f.parts
        }

        self.install(p)

        after = {
            f.relative_to(p): f.read_bytes()
            for f in p.rglob("*")
            if f.is_file() and ".reflow2" not in f.parts
        }
        self.assertEqual(before, after, "an update must not rewrite the project")

    def test_a_new_release_touches_nothing_in_the_project(self):
        """req:thin-install, answered properly.

        Alex: "you wouldn't need to change anything in your repo again and
        updates would be confined to the reflow package." Re-running the same
        version proves almost nothing — the question is what a NEWER reflow2
        does, so this simulates one: the project holds what an older kit
        installed (manifest and file agreeing, as they would after a clean
        install), and the kit has since moved on.

        Before the served instructions, this test failed: the ~20 KB AGENTS.md
        changed with almost every release, so every upgrade produced a diff in a
        repository that has nothing to do with reflow2's release cycle.
        """
        p = self.project()
        self.install(p)

        # The release moves: rewrite the project's copy as an OLDER kit's output
        # and record that hash, so the installer sees a file it owns which no
        # longer matches the kit it ships.
        doc = p / "AGENTS.md"
        older = doc.read_text().replace(
            "## Start here, every session", "## Getting started (older wording)"
        )
        self.assertNotEqual(older, doc.read_text(), "the fixture must actually differ")
        doc.write_text(older)
        stamp = json.loads((p / ".reflow2/kit-version.json").read_text())
        stamp["installed_files"]["AGENTS.md"] = init.file_sha(doc)
        (p / ".reflow2/kit-version.json").write_text(json.dumps(stamp))

        planned = init.planned_changes(p)

        # The pointer file is allowed to be refreshed — it is small and stable —
        # but nothing about the WORKING INSTRUCTIONS or the skills may move,
        # because neither lives here any more.
        self.assertFalse(
            any("skills" in c for c in planned),
            f"no skill file may be installed or updated by an upgrade: {planned}",
        )
        for tree in (".claude", ".grok"):
            self.assertFalse((p / tree).exists(), f"{tree} must not exist")

    def test_the_installed_file_is_a_pointer_not_the_instructions(self):
        """The size difference IS the requirement: what a project holds must be
        the part that does not change between releases."""
        p = self.project()
        self.install(p)

        installed = (p / "AGENTS.md").read_text()
        served = (init.KIT / "AGENTS.md").read_text()

        self.assertLess(
            len(installed),
            len(served) / 3,
            "the installed file must be a pointer, not a copy of the instructions",
        )
        for tool in ("get_instructions", "list_skills", "get_skill"):
            self.assertIn(tool, installed, f"the pointer must name {tool}")

    # ---- never overwrite what the project owns -----------------------------

    def test_foreign_agents_md_is_kept_and_kit_goes_to_sidecar(self):
        p = self.project()
        own = "# My project rules\n\nDo not touch.\n"
        (p / "AGENTS.md").write_text(own)
        self.install(p)

        self.assertTrue(
            (p / "AGENTS.md").read_text().startswith(own.rstrip("\n")),
            "the project's own AGENTS.md content survives",
        )
        self.assertEqual((p / "REFLOW2.md").read_text(), KIT_AGENTS)
        # And the surviving file points at the sidecar (F1's contract).
        self.assertIn("REFLOW2.md", (p / "AGENTS.md").read_text())

    def test_pointer_reaches_every_instruction_convention(self):
        # F2, the storyflow lesson: the file the agent reads FIRST must name
        # reflow2, whatever convention the project uses.
        p = self.project()
        for rel in ["CLAUDE.md", ".cursorrules", ".github/copilot-instructions.md"]:
            f = p / rel
            f.parent.mkdir(parents=True, exist_ok=True)
            f.write_text(f"# {rel}\n")
        self.install(p)

        for rel in ["CLAUDE.md", ".cursorrules", ".github/copilot-instructions.md"]:
            self.assertIn("AGENTS.md", (p / rel).read_text(), rel)
        self.assertNotIn(
            "> **reflow2 is installed here.**",
            (p / "AGENTS.md").read_text(),
            "the kit's own doc must not point at itself",
        )

    def test_an_older_kit_agents_md_is_ours_to_refresh(self):
        # foreign_owner identifies the kit by its first heading, so a kit file
        # from an older install is refreshed in place, no sidecar.
        p = self.project()
        first_heading = KIT_AGENTS.lstrip().splitlines()[0]
        (p / "AGENTS.md").write_text(first_heading + "\n\nolder kit body\n")
        self.install(p)
        self.assertEqual((p / "AGENTS.md").read_text(), KIT_AGENTS)
        self.assertFalse((p / "REFLOW2.md").exists())

    # ---- MCP config: merge, never clobber ----------------------------------

    def test_mcp_merge_preserves_other_servers_and_unrelated_keys(self):
        p = self.project()
        (p / ".mcp.json").write_text(json.dumps({
            "mcpServers": {"other": {"command": "/usr/bin/other"}},
            "unrelated": {"keep": True},
        }))
        (p / "opencode.json").write_text(json.dumps({
            "theme": "dark",
            "mcp": {"other": {"type": "local", "command": ["/usr/bin/other"]}},
        }))
        self.install(p)

        mcp = json.loads((p / ".mcp.json").read_text())
        self.assertEqual(mcp["mcpServers"]["other"]["command"], "/usr/bin/other")
        self.assertEqual(mcp["unrelated"], {"keep": True})
        self.assertIn("reflow2", mcp["mcpServers"])
        oc = json.loads((p / "opencode.json").read_text())
        self.assertEqual(oc["theme"], "dark")
        self.assertIn("other", oc["mcp"])
        self.assertIn("reflow2", oc["mcp"])

    def test_customised_entry_is_left_alone_without_force(self):
        p = self.project()
        theirs = {"mcpServers": {"reflow2": {
            "command": "/their/own/reflow2-mcp",
            "args": ["--graph-path", "elsewhere"],
        }}}
        (p / ".mcp.json").write_text(json.dumps(theirs))
        done = self.install(p)

        kept = json.loads((p / ".mcp.json").read_text())
        self.assertEqual(
            kept["mcpServers"]["reflow2"]["command"], "/their/own/reflow2-mcp",
            "a repoint the user made by hand is not ours to undo",
        )
        self.assertTrue(
            any("LEFT ALONE" in d for d in done),
            f"the skip must be reported, not silent: {done}",
        )

        self.install(p, force_mcp=True)
        repointed = json.loads((p / ".mcp.json").read_text())
        self.assertEqual(
            repointed["mcpServers"]["reflow2"]["command"], str(FAKE_BINARY),
            "--force-mcp is the explicit consent to repoint",
        )

    def test_invalid_json_is_reported_and_never_clobbered(self):
        p = self.project()
        (p / ".mcp.json").write_text("{not json")
        done = self.install(p)
        self.assertEqual((p / ".mcp.json").read_text(), "{not json")
        self.assertTrue(any("not valid JSON" in d for d in done), done)

    # ---- running twice is safe ---------------------------------------------

    def test_install_is_idempotent(self):
        p = self.project()
        (p / "CLAUDE.md").write_text("# mine\n")
        self.install(p)
        snapshot = {
            f.relative_to(p): f.read_bytes()
            for f in sorted(p.rglob("*"))
            if f.is_file() and "kit-version" not in f.name
        }
        done = self.install(p)

        after = {
            f.relative_to(p): f.read_bytes()
            for f in sorted(p.rglob("*"))
            if f.is_file() and "kit-version" not in f.name
        }
        self.assertEqual(snapshot, after, "a second run must change nothing")
        self.assertEqual(
            (p / "CLAUDE.md").read_text().count("reflow2"), 1,
            "the pointer line is appended once, not per run",
        )
        self.assertFalse(
            any(d.endswith(".md") or "skills" in d for d in done),
            f"an unchanged file must not be reported as installed: {done}",
        )


class ManifestTest(InstallerTest):
    """BL-54: ownership is proven by the install manifest, not guessed."""

    def test_user_edited_kit_file_survives_an_update(self):
        p = self.project()
        self.install(p)
        doc = p / "AGENTS.md"
        original = doc.read_text()
        doc.write_text(original + "\nMy local house rule.\n")

        done = self.install(p)

        self.assertIn("My local house rule.", doc.read_text(),
                      "a user's edit to an installed file must survive an update")
        self.assertTrue(any("kept your own AGENTS.md" in d for d in done),
                        f"the withheld refresh must be reported: {done}")
        # Deleting the file accepts the kit copy on the next run.
        doc.unlink()
        self.install(p)
        self.assertEqual(doc.read_text(), original)

    def test_an_old_kits_skill_copies_are_removed_and_the_reason_is_given(self):
        # The migration case, and the one a user actually sees: their repo
        # loses thirty-odd files on the first update after dec:skills-served.
        # Being told "no longer shipped" would be true and useless.
        p = self.project()
        self.install(p)
        stamp = json.loads((p / ".reflow2/kit-version.json").read_text())
        copied = p / ".claude/skills/adopt/SKILL.md"
        copied.parent.mkdir(parents=True)
        copied.write_text("what the old kit installed\n")
        stamp["installed_files"][".claude/skills/adopt/SKILL.md"] = init.file_sha(copied)
        (p / ".reflow2/kit-version.json").write_text(json.dumps(stamp))

        # --check must say so BEFORE anything moves.
        planned = init.planned_changes(p)
        self.assertTrue(
            any("remove" in c and "adopt/SKILL.md" in c and "served" in c for c in planned),
            f"--check must disclose the removal: {planned}",
        )

        done = self.install(p)

        self.assertFalse(copied.exists())
        self.assertTrue(any("served by the MCP server" in d for d in done), done)

    def test_an_edited_skill_copy_is_kept_and_the_shadowing_is_named(self):
        # The dangerous half: a harness DOES auto-load a file in .claude/skills,
        # and a served skill is never offered — so an edited copy silently wins
        # over every future release of that skill. Keeping it is right; keeping
        # it quietly is not.
        p = self.project()
        self.install(p)
        stamp = json.loads((p / ".reflow2/kit-version.json").read_text())
        mine = p / ".claude/skills/adopt/SKILL.md"
        mine.parent.mkdir(parents=True)
        mine.write_text("what the old kit installed\n")
        stamp["installed_files"][".claude/skills/adopt/SKILL.md"] = init.file_sha(mine)
        (p / ".reflow2/kit-version.json").write_text(json.dumps(stamp))
        mine.write_text("what the old kit installed, plus MY house rule\n")

        done = self.install(p)

        self.assertTrue(mine.exists(), "an edited file is never deleted")
        self.assertTrue(
            any("SHADOWS the served skill" in d for d in done),
            f"the shadowing must be named, not just the retention: {done}",
        )

    def test_a_file_the_kit_no_longer_ships_is_pruned_only_when_untouched(self):
        p = self.project()
        self.install(p)
        stamp = json.loads((p / ".reflow2/kit-version.json").read_text())
        # Two files a previous kit shipped: one untouched, one edited.
        gone = p / "docs/old-kit-note.md"
        gone.parent.mkdir(parents=True)
        gone.write_text("obsolete kit content\n")
        edited = p / "docs/old-kit-edited.md"
        edited.write_text("obsolete but edited\n")
        stamp["installed_files"]["docs/old-kit-note.md"] = init.file_sha(gone)
        stamp["installed_files"]["docs/old-kit-edited.md"] = init.file_sha(edited)
        (p / ".reflow2/kit-version.json").write_text(json.dumps(stamp))
        edited.write_text("obsolete but edited BY THE USER\n")

        done = self.install(p)

        self.assertFalse(gone.exists(), "an untouched obsolete kit file is pruned")
        self.assertTrue(edited.exists(), "an edited obsolete file is kept")
        self.assertTrue(any("removed (no longer shipped" in d for d in done), done)
        self.assertTrue(any("your edits — left in place" in d for d in done), done)

    def test_a_non_object_servers_value_is_left_alone_not_a_crash(self):
        p = self.project()
        (p / ".mcp.json").write_text(json.dumps({"mcpServers": ["not", "a", "dict"]}))

        done = self.install(p)  # must not raise

        self.assertEqual(json.loads((p / ".mcp.json").read_text())["mcpServers"],
                         ["not", "a", "dict"], "the malformed file is untouched")
        self.assertTrue(any("left alone" in d and ".mcp.json" in d for d in done), done)


if __name__ == "__main__":
    unittest.main(verbosity=2)
