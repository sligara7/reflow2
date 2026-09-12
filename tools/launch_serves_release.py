#!/usr/bin/env python3
"""The launcher serves a RELEASE binary, and never builds one.

`req:a-session-runs-a-release-binary`. The wrapper is what every session on a
contributor's machine actually talks to, and the two properties that matter are
both one careless edit away from reverting:

  1. the profile it serves defaults to release, not debug;
  2. in release mode it does not run cargo — a ~34-minute librocksdb-sys build
     is not something a session start may decide to do on your behalf.

Measured 2026-09-11 on the same bytes with only the profile differing: a
propagate walk 5,851 ms -> 62 ms warm, design_regions 12.5 s -> 2.0 s warm.
Every cost number in the 2026-09-10/11 tool sweep was taken on debug, because
the wrapper served debug and nothing said so.

WHAT THIS GATE CANNOT SEE, stated so nobody reads a pass as more than it is:
it checks the SCRIPT, not a running session. Whether the binary on disk is
current is a different question, answered by the wrapper's own banner and by
`graph_report`'s `served_by.stale`. This gate would pass on a machine whose
release binary is a month old.
"""

import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
LAUNCH = REPO / "tools" / "reflow2-mcp-launch.sh"
REBUILD = REPO / "tools" / "reflow2-rebuild.sh"


def fail(msg: str) -> None:
    print(f"FAIL: {msg}", file=sys.stderr)
    sys.exit(1)


def main() -> None:
    for script in (LAUNCH, REBUILD):
        if not script.is_file():
            fail(f"{script.relative_to(REPO)} is missing")
        subprocess.run(["bash", "-n", str(script)], check=True)

    text = LAUNCH.read_text()

    # 1. The default profile is release.
    m = re.search(r'profile="\$\{REFLOW2_PROFILE:-(\w+)\}"', text)
    if not m:
        fail("the launcher no longer reads REFLOW2_PROFILE with a default")
    if m.group(1) != "release":
        fail(f"the launcher defaults to '{m.group(1)}', not release")

    # 2. The release path runs no cargo. Split on the profile branch and check
    #    the else arm, which is the release one.
    if 'if [ "$profile" = "debug" ]; then' not in text:
        fail("the launcher no longer branches on the profile the way this gate reads it")
    release_arm = text.split('if [ "$profile" = "debug" ]; then', 1)[1].rsplit("\nelse\n", 1)[-1]
    if "cargo build" in release_arm:
        fail("the release path runs cargo — it must refuse or warn, never build")

    # 3. Staleness is never silent: the release arm must say something when the
    #    stamp does not match. This is the guard on the failure that not
    #    rebuilding reintroduces (2026-08-16, a stale binary served the
    #    pre-#199 change_type enum and nobody was told).
    if '"$want" != "$have"' not in release_arm:
        fail("the release path no longer compares the source hash against the binary's stamp")

    # 4. The refusal names the way out rather than just failing.
    if "reflow2-rebuild.sh" not in release_arm:
        fail("the release path does not name tools/reflow2-rebuild.sh as the fix")

    print("OK: launcher defaults to release, builds nothing there, and says so when stale")


if __name__ == "__main__":
    main()
