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

KIT_AGENTS = (init.KIT / "AGENTS.md").read_text()
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
        # Skills in every directory some harness actually reads (BL-22).
        for tree in (".claude/skills", ".grok/skills"):
            self.assertTrue((p / tree / "adopt" / "SKILL.md").exists(), tree)
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

    def test_skills_trees_are_identical_copies(self):
        # One kit, two directories — a divergence would mean harnesses run
        # different skills depending on which directory they read.
        p = self.project()
        self.install(p)
        claude = sorted(
            f.relative_to(p / ".claude/skills")
            for f in (p / ".claude/skills").rglob("*") if f.is_file()
        )
        grok = sorted(
            f.relative_to(p / ".grok/skills")
            for f in (p / ".grok/skills").rglob("*") if f.is_file()
        )
        self.assertEqual(claude, grok)
        for rel in claude:
            self.assertEqual(
                (p / ".claude/skills" / rel).read_bytes(),
                (p / ".grok/skills" / rel).read_bytes(),
                rel,
            )

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

    def test_user_edited_skill_survives_an_update(self):
        p = self.project()
        self.install(p)
        skill = p / ".claude/skills/adopt/SKILL.md"
        original = skill.read_text()
        skill.write_text(original + "\nMy local house rule.\n")

        done = self.install(p)

        self.assertIn("My local house rule.", skill.read_text(),
                      "a user's edit to an installed file must survive an update")
        self.assertTrue(any("LEFT ALONE" in d and "adopt/SKILL.md" in d for d in done),
                        f"the withheld refresh must be reported: {done}")
        # Deleting the file accepts the kit copy on the next run.
        skill.unlink()
        self.install(p)
        self.assertEqual(skill.read_text(), original)

    def test_a_file_the_kit_no_longer_ships_is_pruned_only_when_untouched(self):
        p = self.project()
        self.install(p)
        stamp = json.loads((p / ".reflow2/kit-version.json").read_text())
        # Simulate two files a previous kit shipped: one untouched, one edited.
        gone = p / ".claude/skills/old-skill/SKILL.md"
        gone.parent.mkdir(parents=True)
        gone.write_text("obsolete kit content\n")
        edited = p / ".claude/skills/old-edited/SKILL.md"
        edited.parent.mkdir(parents=True)
        edited.write_text("obsolete but edited\n")
        stamp["installed_files"][".claude/skills/old-skill/SKILL.md"] = \
            init.file_sha(gone)
        stamp["installed_files"][".claude/skills/old-edited/SKILL.md"] = \
            init.file_sha(edited)
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
