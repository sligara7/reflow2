#!/usr/bin/env python3
"""A `--shared` client OUTSIDE a project attaches to the server already holding it.

# The defect this pins (2026-09-22)

A daemon started inside its project as `--graph-path .reflow2/graph` recorded
that string, as typed, in its rendezvous. A client started anywhere else (a hub
over several designs, a script, a tool) opened the SAME store by its absolute
path. It resolved the relative record against its OWN working directory, read
the rendezvous as belonging to a different design, and started a rival daemon.
The rival could not take the store lock, and after 30 s the session was served
nothing but `reflow2_unavailable`.

Found from the dev_reflow2 hub, which connects to each child project's running
server as one more seat:
fact:shared-attach-by-absolute-path-missed-the-running-server-and-spawned-a-rival-2026-09-22.

The unit tests in `shared.rs` pin the comparison. This pins the behaviour a
user sees, over the real binary: the second client attaches, quickly, to the
same daemon, with the whole tool surface.

Usage (from the repo root, after `cargo build -p reflow2-mcp`):

    python3 tools/test_shared_attach_from_another_folder.py

Exits 0 when the outside client attaches, 1 otherwise. Standard library only.
"""

import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import time

REPO = pathlib.Path(__file__).resolve().parent.parent
BINARY = REPO / "target" / "debug" / "reflow2-mcp"

# Attaching to a live server is sub-second. The failure mode waits out a 30 s
# readiness deadline, so anything near that is the bug, not a slow machine.
ATTACH_BUDGET = 10.0


def client(cwd: pathlib.Path, graph: str) -> subprocess.Popen:
    return subprocess.Popen(
        [str(BINARY), "--graph-path", graph, "--shared"],
        cwd=cwd,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
        bufsize=1,
        env={**os.environ, "RUST_LOG": "warn"},
    )


def rpc(proc, ident, method, params=None):
    msg = {"jsonrpc": "2.0", "method": method, "id": ident}
    if params is not None:
        msg["params"] = params
    proc.stdin.write(json.dumps(msg) + "\n")
    proc.stdin.flush()
    line = proc.stdout.readline()
    if not line:
        raise SystemExit("client closed stdout")
    return json.loads(line)


def tool_names(proc) -> list:
    rpc(
        proc,
        1,
        "initialize",
        {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "attach-from-elsewhere", "version": "0"},
        },
    )
    proc.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n")
    proc.stdin.flush()
    return [t["name"] for t in rpc(proc, 2, "tools/list", {})["result"]["tools"]]


def rendezvous_pid(project: pathlib.Path):
    path = project / ".reflow2" / "graph.server.json"
    try:
        return json.loads(path.read_text()).get("pid")
    except (OSError, ValueError):
        return None


def stop(proc):
    if proc is None:
        return
    proc.stdin.close()
    proc.terminate()
    try:
        proc.wait(timeout=10)
    except subprocess.TimeoutExpired:
        proc.kill()


def main() -> int:
    if not BINARY.exists():
        print(f"SKIP: {BINARY} not built (cargo build -p reflow2-mcp)")
        return 0

    work = pathlib.Path(tempfile.mkdtemp(prefix="reflow2-attach-elsewhere-"))
    project = work / "project"
    elsewhere = work / "somewhere-else"
    # `.reflow2/` exists in every real project: `reflow2_init.py` creates it. A
    # bare folder fails earlier and for an unrelated reason (the daemon opens
    # its log beside the store before anything creates the directory), which
    # is not what this test is about.
    (project / ".reflow2").mkdir(parents=True)
    elsewhere.mkdir()
    inside = outside = None
    try:
        # The owner's session: inside the project, relative path, exactly as
        # `.mcp.json` installs it. This starts the daemon.
        inside = client(project, ".reflow2/graph")
        if len(tool_names(inside)) < 2:
            print("FAIL: the first client (inside the project) did not get a working server")
            return 1
        daemon = rendezvous_pid(project)
        if daemon is None:
            print("FAIL: no rendezvous was published beside the store")
            return 1
        # THE CAUSE, checked at the writer. On Linux a reader can recover a
        # relative record from /proc/<pid>/cwd, so the attach below would pass
        # even if the daemon still wrote the path as typed. Check the record.
        recorded = json.loads((project / ".reflow2" / "graph.server.json").read_text())["graph_path"]
        if not os.path.isabs(recorded):
            print(
                f"FAIL: the daemon recorded its store as {recorded!r}, which means a different "
                "store to every reader outside its own folder"
            )
            return 1

        # The hub's session: another folder, absolute path to the same store.
        start = time.monotonic()
        outside = client(elsewhere, str(project / ".reflow2" / "graph"))
        names = tool_names(outside)
        took = time.monotonic() - start

        if names == ["reflow2_unavailable"] or len(names) < 2:
            print(
                f"FAIL: the outside client was served {names} after {took:.1f}s.\n"
                "      It did not recognise the running server as holding this store, started a\n"
                "      rival, and the rival lost the store lock."
            )
            return 1
        if took > ATTACH_BUDGET:
            print(f"FAIL: attached, but only after {took:.1f}s (budget {ATTACH_BUDGET:.0f}s)")
            return 1
        if rendezvous_pid(project) != daemon:
            print(
                f"FAIL: the daemon changed (pid {daemon} -> {rendezvous_pid(project)}): the outside "
                "client replaced the owner's server instead of joining it"
            )
            return 1

        print(
            f"OK: a client outside the project attached to the running server (pid {daemon}) "
            f"in {took:.2f}s with {len(names)} tools."
        )
        return 0
    finally:
        stop(outside)
        stop(inside)
        # Stop the daemon this test started, spelled the way it was started, so it
        # does not outlive the run holding a lock on a directory about to vanish.
        subprocess.run(
            [str(BINARY), "--graph-path", ".reflow2/graph", "--stop-shared"],
            cwd=project,
            capture_output=True,
            text=True,
            timeout=30,
        )
        shutil.rmtree(work, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
