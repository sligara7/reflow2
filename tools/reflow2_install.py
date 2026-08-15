#!/usr/bin/env python3
"""Install reflow2 **once per machine**, so no project ever needs setting up.

    python3 tools/reflow2_install.py                  # install machine-wide
    python3 tools/reflow2_install.py --check          # what would change
    python3 tools/reflow2_install.py --uninstall      # take it back out

WHY THIS EXISTS. Anthony, 2026-07-28: *"we need to make this as easy as
possible. it is getting too hard to just start a project and then there are
multiple steps after just to get it working."* Getting to a first design action
took an installer run, an agent restart, and a slash command that was not
shipped. Every one of those was per-project, and none of them had to be.

WHAT IT DOES, and the reasoning is the same for all four: reflow2's per-project
footprint was never per-project in nature.

  1. THE SERVER, registered at USER scope. `--graph-path .reflow2/graph` is
     relative to the working directory, so one registration still gives every
     project its own graph. Paired with `--only-if-present`, a directory that
     never opted into a design gets the LATENT surface and nothing is created
     in it — which is what makes a machine-wide registration safe to make.
  2. THE SLASH COMMANDS, into `~/.claude/commands/`. They are pure pointers at
     served skills (`dec:commands-are-the-exception`), so a global copy carries
     nothing that can go stale, and `/genesis` is then available in a directory
     that has never heard of reflow2 — which is exactly where it is needed.
  3. THE LOOP-NUDGE HOOKS, into `~/.claude/settings.json`. Safe globally
     because `loop_nudge.py` is a no-op wherever `.reflow2/` does not exist.
  4. A `reflow2` COMMAND on PATH, so setting up a shared repo later is
     `reflow2 init .` and not a python invocation naming an absolute path into
     a checkout the user may not have.

WHAT IS DELIBERATELY NOT INSTALLED: the skills and the working instructions.
Both are served by the server (`dec:skills-served`, `req:thin-install`), so they
always match the binary and there is no copy to go stale.

WHAT STAYS PER-PROJECT, and only when you want it: `reflow2_init.py` writes the
pointer file and project-scope MCP config for a repo you SHARE, so a teammate's
agent is told reflow2 governs this code. Solo work needs none of it.

Standard library only. Idempotent: re-run any time, including after upgrading
reflow2. It merges into config files rather than overwriting them, and reports
every file it did not touch and why.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
KIT = REPO / "getting-started"
HOME = Path.home()

SERVER_NAME = "reflow2"
# Relative on purpose: resolved against each session's working directory, which
# is what makes one registration serve every project.
GRAPH_PATH = ".reflow2/graph"
SERVER_ARGS = ["--graph-path", GRAPH_PATH, "--shared", "--only-if-present"]

CLAUDE_COMMANDS = HOME / ".claude" / "commands"
CLAUDE_SETTINGS = HOME / ".claude" / "settings.json"
OPENCODE_CONFIG = HOME / ".config" / "opencode" / "opencode.json"
BIN_DIR = Path(os.environ.get("REFLOW2_BIN_DIR", HOME / ".local" / "bin"))

# The hook matchers, kept identical to the per-project install so a machine-wide
# seat and a repo-scoped one behave the same. `loop_nudge.py` decides for itself
# whether the directory has a design; see its `design_present` guard.
HOOK_EVENTS = [
    ("SessionStart", None),
    ("PostToolUse", "mcp__reflow2__.*"),
    ("PostToolUse", "Edit|Write|MultiEdit|NotebookEdit"),
    ("Stop", None),
]


def say(msg: str = "") -> None:
    print(msg)


def find_binary(explicit: str | None) -> Path:
    """The reflow2-mcp this machine should run.

    Order: what the user named, then PATH, then this checkout's release build,
    then its debug build. A checkout that has only ever been built for debug is
    the normal state of a development machine, and refusing it would send the
    developer on a ten-minute RocksDB build to install something that works.
    """
    if explicit:
        p = Path(explicit).expanduser().resolve()
        if not p.exists():
            sys.exit(f"error: --binary {p} does not exist")
        return p
    found = shutil.which("reflow2-mcp")
    if found:
        return Path(found).resolve()
    for candidate in (REPO / "target/release/reflow2-mcp", REPO / "target/debug/reflow2-mcp"):
        if candidate.exists():
            return candidate.resolve()
    sys.exit(
        "error: no reflow2-mcp found. Install a release:\n"
        "  curl -fsSL https://raw.githubusercontent.com/sligara7/reflow2/main/tools/install.sh | sh\n"
        "or build one: cargo build --release -p reflow2-mcp"
    )


def nudge_script() -> Path:
    return REPO / "tools" / "loop_nudge.py"


# ---------------------------------------------------------------- MCP registry


def claude_cli() -> str | None:
    return shutil.which("claude")


def claude_server_registered() -> bool:
    """Whether Claude Code can see a `reflow2` server AT ALL — any scope.

    Asked through the CLI rather than by reading a config file, because which
    file holds user scope is Claude Code's business and has moved before.

    IT CANNOT ANSWER THE SCOPED QUESTION, and saying so is the point: `claude
    mcp get` takes a NAME and no `--scope` (only `add` and `remove` do), so a
    project-scope server from a `.mcp.json` in the working directory answers
    yes here too. This is therefore used for REPORTING only — never to decide
    whether to register, which is done unconditionally because it is idempotent.

    Found 2026-07-28 by running the installer's own `--check` on a machine that
    had never been installed: the call passed `--scope user`, the CLI rejected
    the unknown option, the exception path answered "no", and the answer looked
    right for the wrong reason. A check that cannot be wrong in only one
    direction is a check that has to say what it is measuring.
    """
    cli = claude_cli()
    if not cli:
        return False
    try:
        out = subprocess.run(
            [cli, "mcp", "get", SERVER_NAME],
            capture_output=True, text=True, timeout=30,
        )
        return out.returncode == 0
    except (OSError, subprocess.SubprocessError):
        return False


def register_claude(binary: Path, check: bool) -> str:
    cli = claude_cli()
    if not cli:
        return ("skip    Claude Code: the `claude` CLI is not on PATH — register by hand:\n"
                f"          claude mcp add -s user {SERVER_NAME} -- {binary} "
                f"{' '.join(SERVER_ARGS)}")
    if check:
        # Deliberately not "update" vs "create": the CLI cannot say which scope
        # an existing `reflow2` lives in, and guessing would report a
        # project-scope server as though user scope were already set up.
        seen = " (a server of this name is already visible — scope unknown)" \
            if claude_server_registered() else ""
        return f"register  Claude Code user-scope MCP server `{SERVER_NAME}`{seen}"
    # Remove first: `add` refuses a name that exists, and re-registering is how
    # an upgrade moves the binary path. Failure to remove is fine (absent).
    subprocess.run([cli, "mcp", "remove", SERVER_NAME, "--scope", "user"],
                   capture_output=True, text=True)
    out = subprocess.run(
        [cli, "mcp", "add", "-s", "user", SERVER_NAME, "--", str(binary), *SERVER_ARGS],
        capture_output=True, text=True,
    )
    if out.returncode != 0:
        detail = (out.stderr or out.stdout).strip().splitlines()
        return f"FAILED  Claude Code registration: {detail[-1] if detail else 'unknown error'}"
    return f"ok      Claude Code user-scope MCP server `{SERVER_NAME}`"


def unregister_claude(check: bool) -> str:
    cli = claude_cli()
    if not cli:
        return "skip    Claude Code: no `claude` CLI on PATH"
    if check:
        return ("remove  Claude Code user-scope MCP server" if claude_server_registered()
                else "ok      Claude Code: nothing registered")
    subprocess.run([cli, "mcp", "remove", SERVER_NAME, "--scope", "user"],
                   capture_output=True, text=True)
    return "ok      Claude Code user-scope MCP server removed"


def register_opencode(binary: Path, check: bool) -> str:
    """OpenCode's user config. Merged, never overwritten — it holds the user's
    own servers and settings and this is a guest in it."""
    entry = {
        "type": "local",
        "command": [str(binary), *SERVER_ARGS],
        "enabled": True,
    }
    existing: dict = {}
    if OPENCODE_CONFIG.exists():
        try:
            existing = json.loads(OPENCODE_CONFIG.read_text())
        except json.JSONDecodeError:
            return f"skip    {OPENCODE_CONFIG} is not valid JSON — left alone, fix it by hand"
    if not isinstance(existing, dict):
        return f"skip    {OPENCODE_CONFIG} is not a JSON object — left alone"
    servers = existing.get("mcp")
    if servers is not None and not isinstance(servers, dict):
        return f"skip    {OPENCODE_CONFIG} has a non-object `mcp` — left alone"
    if (servers or {}).get(SERVER_NAME) == entry:
        return f"ok      {OPENCODE_CONFIG} already current"
    if check:
        return f"update  {OPENCODE_CONFIG} (register `{SERVER_NAME}`)"
    existing.setdefault("$schema", "https://opencode.ai/config.json")
    existing.setdefault("mcp", {})[SERVER_NAME] = entry
    OPENCODE_CONFIG.parent.mkdir(parents=True, exist_ok=True)
    OPENCODE_CONFIG.write_text(json.dumps(existing, indent=2) + "\n")
    return f"ok      {OPENCODE_CONFIG}"


def unregister_opencode(check: bool) -> str:
    if not OPENCODE_CONFIG.exists():
        return "ok      OpenCode: nothing to remove"
    try:
        existing = json.loads(OPENCODE_CONFIG.read_text())
    except json.JSONDecodeError:
        return f"skip    {OPENCODE_CONFIG} is not valid JSON — left alone"
    if not isinstance(existing, dict) or SERVER_NAME not in (existing.get("mcp") or {}):
        return "ok      OpenCode: nothing to remove"
    if check:
        return f"remove  `{SERVER_NAME}` from {OPENCODE_CONFIG}"
    del existing["mcp"][SERVER_NAME]
    OPENCODE_CONFIG.write_text(json.dumps(existing, indent=2) + "\n")
    return f"ok      `{SERVER_NAME}` removed from {OPENCODE_CONFIG}"


# ------------------------------------------------------------------- commands


def install_commands(check: bool) -> list[str]:
    src = KIT / "commands"
    if not src.is_dir():
        return [f"skip    no commands in the kit at {src}"]
    out = []
    for f in sorted(src.glob("*.md")):
        target = CLAUDE_COMMANDS / f.name
        if target.exists() and target.read_bytes() == f.read_bytes():
            continue
        verb = "update" if target.exists() else "create"
        if not check:
            CLAUDE_COMMANDS.mkdir(parents=True, exist_ok=True)
            target.write_bytes(f.read_bytes())
        out.append(f"{verb}  ~/.claude/commands/{f.name}")
    return out or ["ok      ~/.claude/commands already current"]


def remove_commands(check: bool) -> list[str]:
    src = KIT / "commands"
    out = []
    for f in sorted(src.glob("*.md")) if src.is_dir() else []:
        target = CLAUDE_COMMANDS / f.name
        if not target.exists():
            continue
        if target.read_bytes() != f.read_bytes():
            # Someone edited it. Their file, their call — reflow2 does not get
            # to delete work it did not write.
            out.append(f"keep    ~/.claude/commands/{f.name} (your edits — not removed)")
            continue
        if not check:
            target.unlink()
        out.append(f"remove  ~/.claude/commands/{f.name}")
    return out or ["ok      ~/.claude/commands: nothing to remove"]


# ----------------------------------------------------------------------- hooks


def hook_command(nudge: Path) -> str:
    return f'python3 "{nudge}"'


def read_settings() -> tuple[dict, str | None]:
    if not CLAUDE_SETTINGS.exists():
        return {}, None
    try:
        d = json.loads(CLAUDE_SETTINGS.read_text())
    except json.JSONDecodeError:
        return {}, f"{CLAUDE_SETTINGS} is not valid JSON — left alone, fix it by hand"
    if not isinstance(d, dict):
        return {}, f"{CLAUDE_SETTINGS} is not a JSON object — left alone"
    return d, None


def install_hooks(nudge: Path, check: bool) -> str:
    settings, problem = read_settings()
    if problem:
        return f"skip    {problem}"
    cmd = hook_command(nudge)
    hooks = settings.setdefault("hooks", {}) if isinstance(settings.get("hooks", {}), dict) else None
    if hooks is None:
        return f"skip    {CLAUDE_SETTINGS} has a non-object `hooks` — left alone"

    changed = False
    for event, matcher in HOOK_EVENTS:
        entries = hooks.setdefault(event, [])
        if not isinstance(entries, list):
            return f"skip    {CLAUDE_SETTINGS} has a non-list `hooks.{event}` — left alone"
        # Replace any entry that is ours (same matcher, a loop_nudge command),
        # so an upgrade that moves the kit does not leave two hooks firing.
        mine = [
            e for e in entries
            if isinstance(e, dict) and e.get("matcher") == matcher
            and any("loop_nudge.py" in str(h.get("command", ""))
                    for h in e.get("hooks", []) if isinstance(h, dict))
        ]
        wanted = {"hooks": [{"type": "command", "command": cmd}]}
        if matcher is not None:
            wanted["matcher"] = matcher
        if mine == [wanted]:
            continue
        changed = True
        if not check:
            for e in mine:
                entries.remove(e)
            entries.append(wanted)

    if not changed:
        return "ok      ~/.claude/settings.json hooks already current"
    if check:
        return "update  ~/.claude/settings.json (loop nudge: SessionStart, PostToolUse ×2, Stop)"
    CLAUDE_SETTINGS.parent.mkdir(parents=True, exist_ok=True)
    CLAUDE_SETTINGS.write_text(json.dumps(settings, indent=2) + "\n")
    return "ok      ~/.claude/settings.json (loop nudge registered)"


def remove_hooks(check: bool) -> str:
    settings, problem = read_settings()
    if problem:
        return f"skip    {problem}"
    hooks = settings.get("hooks")
    if not isinstance(hooks, dict):
        return "ok      ~/.claude/settings.json: no hooks to remove"
    removed = False
    for event, entries in list(hooks.items()):
        if not isinstance(entries, list):
            continue
        keep = [
            e for e in entries
            if not (isinstance(e, dict) and any(
                "loop_nudge.py" in str(h.get("command", ""))
                for h in e.get("hooks", []) if isinstance(h, dict)))
        ]
        if len(keep) != len(entries):
            removed = True
            if not check:
                hooks[event] = keep
    if not removed:
        return "ok      ~/.claude/settings.json: no reflow2 hooks"
    if check:
        return "update  ~/.claude/settings.json (remove the loop-nudge hooks)"
    CLAUDE_SETTINGS.write_text(json.dumps(settings, indent=2) + "\n")
    return "ok      ~/.claude/settings.json (loop-nudge hooks removed)"


# ------------------------------------------------------------ the `reflow2` cmd


WRAPPER = """#!/bin/sh
# reflow2 — installed by reflow2_install.py. Subcommands run the kit's tools;
# anything else is handed to the server binary, so `reflow2 --version` and
# `reflow2 --export` work exactly as the binary does.
set -eu
KIT="{kit}"
BIN="{binary}"
case "${{1:-}}" in
  init)      shift; exec python3 "$KIT/../tools/reflow2_init.py" --binary "$BIN" "$@" ;;
  update)    shift; exec python3 "$KIT/../tools/reflow2_init.py" --binary "$BIN" --update "$@" ;;
  install)   shift; exec python3 "$KIT/../tools/reflow2_install.py" --binary "$BIN" "$@" ;;
  check)     shift; exec python3 "$KIT/../tools/reflow2_check.py" "$@" ;;
  ""|help|-h|--help)
    cat <<'EOF'
reflow2 — a persistent design brain for building things with an AI agent.

  reflow2 install       install machine-wide (every project, no per-project setup)
  reflow2 init <dir>    set up ONE repo to carry its design (for a project you share)
  reflow2 update        bring THIS project's reflow2 files up to the version
                        installed on this machine  (--check to preview)
  reflow2 check         run the design gate against the committed export

Anything else is passed to reflow2-mcp: reflow2 --version, reflow2 --export, ...

TWO THINGS CAN BE OUT OF DATE, and one command does not do both.
`reflow2 update` refreshes THIS PROJECT against the reflow2 already installed
here; it never downloads anything. To update reflow2 ITSELF, re-run the
installer — `reflow2 update` says so when it notices the binary is behind.
Your design graph is never touched by either.

Once installed, start a design by opening your agent in any directory and
running /genesis (new project) or /adopt (code that already exists).
EOF
    exit 0 ;;
esac
exec "$BIN" "$@"
"""


def install_wrapper(binary: Path, check: bool) -> str:
    target = BIN_DIR / "reflow2"
    body = WRAPPER.format(kit=KIT, binary=binary)
    if target.exists() and target.read_text() == body:
        return f"ok      {target} already current"
    verb = "update" if target.exists() else "create"
    if not check:
        BIN_DIR.mkdir(parents=True, exist_ok=True)
        target.write_text(body)
        target.chmod(0o755)
    return f"{verb}  {target}"


def remove_wrapper(check: bool) -> str:
    target = BIN_DIR / "reflow2"
    if not target.exists():
        return "ok      no `reflow2` wrapper to remove"
    if not check:
        target.unlink()
    return f"remove  {target}"


# ------------------------------------------------------------------------ main


def main() -> int:
    ap = argparse.ArgumentParser(
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    ap.add_argument("--check", action="store_true",
                    help="report what would change, write nothing")
    ap.add_argument("--uninstall", action="store_true",
                    help="remove the machine-wide install (design graphs untouched)")
    ap.add_argument("--binary", metavar="PATH",
                    help="the reflow2-mcp to register (default: PATH, then this checkout)")
    args = ap.parse_args()

    if args.uninstall:
        say("Removing reflow2's machine-wide install"
            + (" — CHECK ONLY, nothing written" if args.check else ""))
        say("")
        for line in [unregister_claude(args.check), unregister_opencode(args.check),
                     *remove_commands(args.check), remove_hooks(args.check),
                     remove_wrapper(args.check)]:
            say(f"  {line}")
        say("")
        say("Your design graphs and every project's own files were not touched.")
        return 0

    binary = find_binary(args.binary)
    nudge = nudge_script()

    # WORDING MATTERS HERE, and this line was reported as alarming (Alex, 2026-08-12).
    # "Installing reflow2 for every project on this machine" reads as though the
    # installer is about to reach into every project directory and change it. It is
    # not: nothing outside ~/.local and ~/.claude is touched, and a project only
    # gains reflow2 when someone points it there. Say what it actually does.
    say(f"Making reflow2 globally available to projects on this machine"
        + (" — CHECK ONLY, nothing written" if args.check else ""))
    say(f"  binary  {binary}")
    say("")

    lines = [
        register_claude(binary, args.check),
        register_opencode(binary, args.check),
        *install_commands(args.check),
    ]
    if nudge.exists():
        lines.append(install_hooks(nudge, args.check))
    else:
        lines.append(f"skip    loop-nudge hooks: {nudge} not found")
    lines.append(install_wrapper(binary, args.check))
    for line in lines:
        say(f"  {line}")

    failed = [line for line in lines if line.startswith("FAILED")]
    say("")
    if args.check:
        say("Nothing was written. Re-run without --check to apply.")
        return 0
    if failed:
        say("Some steps FAILED — reflow2 is not fully installed. Fix the above and re-run.")
        return 1

    if str(BIN_DIR) not in os.environ.get("PATH", "").split(os.pathsep):
        say(f"NOTE: {BIN_DIR} is not on your PATH. Add it so `reflow2` works:")
        say(f'  export PATH="{BIN_DIR}:$PATH"')
        say("")
    say("Done. There is nothing to set up per project:")
    say("")
    say("  cd any-directory && claude")
    say("  /genesis   — start a design for a new project")
    say("  /adopt     — bring code that already exists under design control")
    say("")
    say("Directories you never design stay untouched — no graph is created until you ask.")
    say("For a repo you SHARE, `reflow2 init .` adds the pointer file a teammate's agent reads.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
