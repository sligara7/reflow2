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
import subprocess
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

    # ---- the pointer reaches the file the agent actually reads -------------
    #
    # req:kit-reaches-the-agent. Found in USE, not in review: installing into
    # dynograph-foundation reported success and left the kit invisible, because
    # that repo had no instruction file at all and Claude Code reads CLAUDE.md
    # first. Same defect class this installer already documents from storyflow,
    # opposite direction — the earlier fix protected an EXISTING CLAUDE.md.

    def test_a_project_with_no_instruction_file_still_gets_reached(self):
        p = self.project()
        self.install(p)

        claude = p / "CLAUDE.md"
        self.assertTrue(
            claude.exists(),
            "a repo with no instruction file must still get one the primary harness reads — "
            "AGENTS.md alone is what made the install succeed and stay invisible",
        )
        self.assertIn("AGENTS.md", claude.read_text())

    def test_the_created_pointer_is_a_pointer_and_not_a_second_home_for_instructions(self):
        p = self.project()
        self.install(p)

        text = (p / "CLAUDE.md").read_text()
        self.assertIn("reflow2 is installed here", text)
        # Deliberately small. A file reflow2 invents in someone's repo says where
        # to look and gets out of the way; duplicating the instructions would
        # create the second copy that goes stale, which is dec:skills-served's
        # whole complaint.
        self.assertLess(
            len(text), 1200, f"the created pointer should stay a pointer:\n{text}"
        )

    def test_an_existing_instruction_file_is_appended_to_not_replaced(self):
        p = self.project()
        (p / "CLAUDE.md").write_text("# House rules\n\nRun the linter before pushing.\n")
        self.install(p)

        text = (p / "CLAUDE.md").read_text()
        self.assertIn("Run the linter before pushing.", text, "the project's own rules must survive")
        self.assertIn("AGENTS.md", text)

    def test_nothing_is_invented_when_the_project_already_has_a_convention(self):
        # The counterweight to the fix: writing CLAUDE.md, GEMINI.md and the rest
        # into a repo that asked for none of them is spam. Creation happens ONLY
        # when the project owns no instruction file whatsoever.
        p = self.project()
        (p / "GEMINI.md").write_text("# Gemini\n\nProject notes.\n")
        self.install(p)

        self.assertFalse(
            (p / "CLAUDE.md").exists(),
            "a project that already has an instruction file must not have another invented",
        )
        self.assertIn("AGENTS.md", (p / "GEMINI.md").read_text())

    def test_the_slash_commands_ship_with_the_kit(self):
        # Anthony's call, 2026-07-28: the one narrow exception to
        # dec:skills-served. A command carries no version-coupled content — it
        # names a skill and says how to report it — so a copy cannot go stale
        # the way a copied SKILL does. Without them the install is experienced
        # as broken rather than thin: the skills are reachable and nothing tells
        # you they are.
        p = self.project()
        self.install(p)

        cmds = p / ".claude" / "commands"
        self.assertTrue(cmds.is_dir(), "the kit's slash commands must be installed")
        names = {f.name for f in cmds.glob("*.md")}
        self.assertIn("gaps.md", names)
        self.assertIn("where.md", names)
        # Whatever the kit holds is what lands — no curated subset that drifts
        # from the source.
        kit = {f.name for f in (init.KIT / "commands").glob("*.md")}
        self.assertEqual(names, kit)

    def test_skills_are_still_not_copied(self):
        # The counterweight. Shipping commands must NOT be read as reopening
        # dec:skills-served: the skills themselves stay served, because a stale
        # skill is wrong in ways nobody notices.
        p = self.project()
        self.install(p)

        for tree in (".claude/skills", ".grok/skills"):
            self.assertFalse(
                (p / tree).exists(),
                f"{tree} must still not be installed — commands are the exception, not skills",
            )

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
        # No skills anywhere — `.claude/` itself is allowed to exist now, because
        # the loop-nudge hook lives in `.claude/settings.local.json`. That file
        # cannot churn the REPOSITORY, which is what this test is about: it is
        # machine state carrying an absolute path, and the installer gitignores
        # it for exactly that reason. Asserting on the whole directory was a
        # proxy for "no copied kit files", and the proxy stopped meaning that.
        for tree in (".claude/skills", ".grok/skills", ".grok"):
            self.assertFalse((p / tree).exists(), f"{tree} must not exist")
        self.assertIn(
            ".claude/settings.local.json",
            (p / ".gitignore").read_text(),
            "and the one file under .claude must be ignored, or it would churn",
        )

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

    def test_the_graph_path_is_relative_so_two_worktrees_do_not_collide(self):
        """The six-sessions-on-one-machine case, which is what this is for.

        An absolute graph path copied into a second git worktree points BOTH
        sessions at the same store: the second loses the single-writer lock and
        gets the degraded server. Relative, each worktree opens its own. The
        binary path stays absolute — there is no PATH to rely on.
        """
        p = self.project()
        self.install(p)

        mcp = json.loads((p / ".mcp.json").read_text())["mcpServers"]["reflow2"]
        self.assertEqual(mcp["args"][0], "--graph-path")
        self.assertFalse(
            pathlib.Path(mcp["args"][1]).is_absolute(),
            f"the graph path must be relative: {mcp['args'][1]}",
        )
        self.assertIn(".reflow2", mcp["args"][1])
        self.assertTrue(
            pathlib.Path(mcp["command"]).is_absolute(),
            "but the binary path must stay absolute",
        )

    def test_every_generated_config_shares_the_design_by_default(self):
        """Two sessions on ONE project must both work — the default decides it.

        Without `--shared` each session spawns its own process against the same
        store, the single-writer lock admits one, and every other session gets
        the degraded surface. That is a broken second session caused by the
        configuration we ship, and it cost a real fleet five days: they built a
        turn-taking convention around a limitation that a flag in the binary they
        were already running had removed.

        Asserted for EVERY harness, not just `.mcp.json`: the failure is per
        config file, so a check that covered one would go green while a user on
        another editor kept hitting the lock.
        """
        p = self.project()
        self.install(p)

        mcp = json.loads((p / ".mcp.json").read_text())["mcpServers"]["reflow2"]
        self.assertIn("--shared", mcp["args"], ".mcp.json must share by default")

        oc = json.loads((p / "opencode.json").read_text())["mcp"]["reflow2"]
        self.assertIn("--shared", oc["command"], "opencode.json must share by default")

        vs = json.loads((p / ".vscode/mcp.json").read_text())["servers"]["reflow2"]
        self.assertIn("--shared", vs["args"], ".vscode/mcp.json must share by default")
        # Same for the other two harnesses, or the guarantee is one-harness deep.
        oc = json.loads((p / "opencode.json").read_text())["mcp"]["reflow2"]
        self.assertFalse(pathlib.Path(oc["command"][2]).is_absolute(), oc["command"])
        vs = json.loads((p / ".vscode/mcp.json").read_text())["servers"]["reflow2"]
        self.assertFalse(pathlib.Path(vs["args"][1]).is_absolute(), vs["args"])

    def test_the_mcp_configs_are_gitignored_like_the_graph(self):
        """They carry an absolute path to THIS machine's binary. Committed, they
        reach a collaborator pointing at a binary that does not exist there."""
        p = self.project()
        self.install(p)

        ignored = (p / ".gitignore").read_text()
        for line in (".reflow2/", ".mcp.json", "opencode.json", ".vscode/mcp.json"):
            self.assertIn(line, ignored, f"{line} must be ignored")

    def test_an_existing_gitignore_is_appended_to_not_replaced(self):
        p = self.project()
        (p / ".gitignore").write_text("# mine\n/target\nnode_modules/\n")
        self.install(p)

        ignored = (p / ".gitignore").read_text()
        self.assertIn("/target", ignored, "the project's own rules survive")
        self.assertIn("node_modules/", ignored)
        self.assertIn(".mcp.json", ignored)

    def test_a_config_git_already_tracks_is_reported_not_silently_ignored(self):
        """Ignoring a tracked file does nothing until it is untracked, so
        saying "ignored" without saying that would be a half-truth — and the
        user would keep shipping their absolute paths to their collaborator.
        """
        p = self.project()
        subprocess.run(["git", "init", "-q"], cwd=p, check=True)
        (p / ".mcp.json").write_text(json.dumps({"mcpServers": {}}))
        subprocess.run(["git", "add", "-f", ".mcp.json"], cwd=p, check=True)

        done = self.install(p)

        self.assertTrue(
            any("git rm --cached .mcp.json" in d for d in done),
            f"the run must say what to do about it: {done}",
        )
        # And --check says it too, before anything moves.
        self.assertTrue(
            any("git rm --cached" in c for c in init.planned_changes(p)),
            "--check must disclose it as well",
        )

    def test_the_loop_nudge_hook_is_installed(self):
        """Until 2026-07-25 the installer wired no hooks, so every consumer
        project ran with no session-end backstop at all — the one trigger that
        fires when an agent has stopped calling anything."""
        p = self.project()
        self.install(p)

        settings = json.loads((p / ".claude/settings.local.json").read_text())
        events = settings["hooks"]
        self.assertIn("Stop", events, "the backstop is the point")
        commands = [
            hook["command"]
            for groups in events.values()
            for group in groups
            for hook in group["hooks"]
        ]
        self.assertTrue(all("loop_nudge" in c for c in commands), commands)
        # It points at the KIT, not at a copy in the project: the script then
        # updates with the package and nothing here can go stale.
        self.assertTrue(
            all(str(init.REPO) in c for c in commands),
            f"the hook must run the installed kit's script: {commands}",
        )

    def test_the_hook_goes_in_the_local_settings_not_the_shared_one(self):
        """It carries an absolute path to THIS machine's kit. In the shared
        settings.json a collaborator inherits a hook that fails silently —
        which is the 'broken' state reflow2 reports, and the worst of them,
        because the file looks right."""
        p = self.project()
        self.install(p)

        self.assertTrue((p / ".claude/settings.local.json").exists())
        self.assertFalse(
            (p / ".claude/settings.json").exists(),
            "the shared settings file is not ours to write",
        )
        self.assertIn(".claude/settings.local.json", (p / ".gitignore").read_text())

    def test_other_hooks_and_settings_survive(self):
        p = self.project()
        (p / ".claude").mkdir()
        (p / ".claude/settings.local.json").write_text(json.dumps({
            "model": "opus",
            "hooks": {"Stop": [{"hooks": [{"type": "command", "command": "echo mine"}]}]},
        }))
        self.install(p)

        settings = json.loads((p / ".claude/settings.local.json").read_text())
        self.assertEqual(settings["model"], "opus", "unrelated settings survive")
        stop = [h["command"] for g in settings["hooks"]["Stop"] for h in g["hooks"]]
        self.assertIn("echo mine", stop, "their own Stop hook survives")
        self.assertTrue(any("loop_nudge" in c for c in stop), stop)

    def test_a_hook_the_user_repointed_is_left_alone(self):
        """Same rule as the MCP config: a hook is something people customise,
        and an installer that undoes that silently is one nobody trusts near
        their settings again."""
        p = self.project()
        (p / ".claude").mkdir()
        (p / ".claude/settings.local.json").write_text(json.dumps({
            "hooks": {"Stop": [{"hooks": [
                {"type": "command", "command": "python3 /my/own/loop_nudge.py"}
            ]}]},
        }))

        done = self.install(p)

        stop = [
            h["command"]
            for g in json.loads((p / ".claude/settings.local.json").read_text())["hooks"]["Stop"]
            for h in g["hooks"]
        ]
        self.assertEqual(stop, ["python3 /my/own/loop_nudge.py"], "their version stands")
        self.assertTrue(any("LEFT ALONE" in d for d in done), f"and it is reported: {done}")

    def test_installing_twice_does_not_duplicate_the_hook(self):
        p = self.project()
        self.install(p)
        self.install(p)

        settings = json.loads((p / ".claude/settings.local.json").read_text())
        stop = [h for g in settings["hooks"]["Stop"] for h in g["hooks"]]
        self.assertEqual(len(stop), 1, f"idempotent, or a re-run stacks nudges: {stop}")

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

    # ---- the graph store is reported when git already tracks it ------------
    #
    # Alex, 2026-08-08, on a project installed at 0.11.0: his .gitignore
    # contained `.reflow2` and git was STILL tracking `.reflow2/graph/LOG` and
    # `.reflow2/graph/fulltext/*.json`. .gitignore never untracks, so the rule
    # was inert — and the installer, which has a warning for exactly this, said
    # nothing, because it skipped every IGNORE_LINES entry ending in `/` and
    # `.reflow2/` is the only one.

    def tracked_graph_project(self):
        """A project shaped like Alex's: graph files committed BEFORE the
        ignore rule existed, which is the ordinary order of events."""
        p = self.project()
        run = lambda *a: subprocess.run(a, cwd=p, capture_output=True, check=True)
        run("git", "init", "-q", ".")
        (p / ".reflow2" / "graph" / "fulltext").mkdir(parents=True)
        (p / ".reflow2" / "graph" / "LOG").write_text("rocksdb log\n")
        (p / ".reflow2" / "graph" / "fulltext" / "meta.json").write_text("{}\n")
        run("git", "add", "-A")
        run("git", "-c", "user.email=t@t", "-c", "user.name=t", "commit", "-qm", "before the rule")
        return p

    def test_a_committed_graph_store_is_reported_not_silently_ignored(self):
        p = self.tracked_graph_project()
        notes = init.ensure_gitignore(p)

        hit = [n for n in notes if ".reflow2/" in n and "IS COMMITTED" in n]
        self.assertTrue(hit, f"the tracked graph store must be reported: {notes}")
        self.assertIn("git rm -r --cached .reflow2", hit[0],
                      "and the remedy must be the one that works on a directory")

    def test_check_also_reports_it_because_check_is_what_you_run_first(self):
        p = self.tracked_graph_project()
        changes = init.planned_changes(p)

        self.assertTrue(
            any(".reflow2/" in c and "is committed" in c for c in changes),
            f"--check is what you run to find out what is wrong: {changes}",
        )

    def test_an_untracked_graph_store_is_not_reported(self):
        # The counterweight: the ordinary project must stay quiet, or the
        # warning becomes noise and stops being read.
        p = self.project()
        subprocess.run(["git", "init", "-q", "."], cwd=p, capture_output=True, check=True)
        (p / ".reflow2" / "graph").mkdir(parents=True)
        (p / ".reflow2" / "graph" / "LOG").write_text("rocksdb log\n")

        notes = init.ensure_gitignore(p)
        self.assertFalse([n for n in notes if "IS COMMITTED" in n],
                         f"nothing is tracked, so nothing should be reported: {notes}")


class SecondUserFirstRun(unittest.TestCase):
    """Alex's first run on a real work project, 2026-08-13.

    Two reports, both reproduced before being fixed, and this class exists so
    neither can come back quietly. The class of finding matters as much as the
    findings: BOTH were visible only to somebody meeting the output for the
    first time (`fact:second-user-first-run-report-2026-08-13`).
    """

    def project(self):
        d = pathlib.Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, d, True)
        return d

    # --- "I can't find the artifact that can be shared by other members" ----

    def test_the_gitignore_names_where_the_shareable_record_goes(self):
        p = self.project()
        init.ensure_gitignore(p)
        gi = (p / ".gitignore").read_text()

        self.assertIn(".reflow2/", gi, "the store is still machine state")
        # The whole defect: it ignored the store and never said what to commit.
        self.assertIn("docs/design/", gi)
        self.assertIn("export_graph", gi)

    def test_the_agent_instructions_name_the_record_too(self):
        # The .gitignore answers the human who goes looking. This answers the
        # agent, which is what actually makes the file get written.
        pointer = (HERE.parent / "getting-started" / "POINTER.md").read_text()
        self.assertIn("docs/design/", pointer)
        self.assertIn("export_graph", pointer)

    def test_the_record_path_is_one_convention_not_a_guess(self):
        p = self.project()
        self.assertEqual(
            init.design_record_path(p),
            p / "docs" / "design" / f"{p.name}.json",
        )

    def test_a_fresh_project_gets_the_record_because_that_is_where_he_looked(self):
        # THE CASE ALEX ACTUALLY HIT. An earlier version of the fix wrote the
        # file only when a graph already existed, which is silent on exactly the
        # fresh project he was standing in. "it should just make the file."
        p = self.project()
        binary = init.find_binary(None)
        if binary is None:
            self.skipTest("no reflow2-mcp binary to export with")

        note = init.ensure_design_record(p, binary)
        dest = init.design_record_path(p)

        self.assertIsNotNone(note, "a fresh project must still get its record")
        self.assertTrue(dest.exists(), f"{dest} must exist: {note}")
        doc = json.loads(dest.read_text())
        self.assertEqual(doc["nodes"], [], "an empty design is still a real document")
        self.assertIn("content_hash", doc, "and it seeds the lineage chain")

    def test_the_record_is_not_swept_up_by_the_ignore_rules(self):
        # The defect in one line: init's other export lands in .reflow2/backups/,
        # inside the very directory this same installer ignores.
        p = self.project()
        init.ensure_gitignore(p)
        ignored = (p / ".gitignore").read_text()
        rel = str(init.design_record_path(p).relative_to(p))
        self.assertFalse(
            any(line.strip() and rel.startswith(line.strip().rstrip("/"))
                for line in ignored.splitlines() if not line.startswith("#")),
            f"{rel} must not be covered by an ignore rule:\n{ignored}",
        )

    def test_an_existing_record_is_never_overwritten(self):
        p = self.project()
        (p / ".reflow2" / "graph").mkdir(parents=True)
        dest = init.design_record_path(p)
        dest.parent.mkdir(parents=True)
        dest.write_text('{"theirs": true}')

        self.assertIsNone(init.ensure_design_record(p, pathlib.Path("/nonexistent")))
        self.assertEqual(dest.read_text(), '{"theirs": true}',
                         "a record they may have committed is theirs, not ours")

    # --- "init should not write over .claude/settings.local.json" ----------

    def test_an_existing_settings_file_keeps_its_own_indent(self):
        # It never overwrote — it MERGED and then re-serialised at a fixed
        # 2-space indent, so every line moved and the diff was
        # indistinguishable from a rewrite.
        p = self.project()
        (p / ".claude").mkdir()
        settings = p / ".claude" / "settings.local.json"
        settings.write_text(
            '{\n'
            '    "permissions": {\n'
            '        "allow": ["Bash(x)"]\n'
            '    }\n'
            '}\n'
        )
        init.ensure_hooks(p, force=False)
        after = settings.read_text()

        self.assertIn('    "permissions"', after,
                      f"their 4-space indent must survive:\n{after}")
        self.assertEqual(json.loads(after)["permissions"], {"allow": ["Bash(x)"]},
                         "and nothing of theirs may be lost")

    def test_detected_indent_falls_back_when_there_is_nothing_to_read(self):
        self.assertEqual(init.detected_indent(""), "  ")
        self.assertEqual(init.detected_indent('{"a":1}'), "  ", "no indented line to learn from")
        self.assertEqual(init.detected_indent('{\n\t"a": 1\n}'), "\t",
                         "a tab-indented file must come back tab-indented, not normalised")
        self.assertEqual(init.detected_indent('{\n    "a": 1\n}'), "    ")

    def test_a_users_own_hook_survives_alongside_reflow2s(self):
        p = self.project()
        (p / ".claude").mkdir()
        settings = p / ".claude" / "settings.local.json"
        settings.write_text(json.dumps({
            "hooks": {"Stop": [{"hooks": [{"type": "command", "command": "echo mine"}]}]}
        }, indent=2))

        init.ensure_hooks(p, force=False)
        stop = json.loads(settings.read_text())["hooks"]["Stop"]
        commands = [h.get("command", "") for g in stop for h in g.get("hooks", [])]

        self.assertTrue(any("echo mine" in c for c in commands), "theirs must survive")
        self.assertTrue(any("loop_nudge" in c for c in commands), "ours must be added")


if __name__ == "__main__":
    unittest.main(verbosity=2)
