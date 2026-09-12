#!/usr/bin/env python3
"""`--export-to` reaches the DAEMON, not just the client that was given it.

# The defect this pins, found while building the feature it pins

In `--shared` mode — which is what `.mcp.json` uses and what `reflow2_init.py`
installs — the process a harness launches is a PROXY. It holds no graph and
performs no writes; it forwards to a long-lived daemon that `shared::spawn_daemon`
starts with a fixed argument list.

So giving the client `--export-to` and stopping there would have meant the flag
every installed project now passes **has no effect at all in the default
configuration**, while every test that drove a plain stdio server passed. That
is exactly the shape this repo keeps meeting — a mechanism wired into the one
place that motivated it, with the siblings left alone — and it is what
`fact:the-unknown-field-interception-was-dead-from-the-day-it-shipped` is about.

Inspection found it. Inspection is not a test, so this is the test: drive a real
`--shared` client over stdio, write one node, and require the file to appear.
A pass means the flag crossed the process boundary.

Usage (from the repo root, after `cargo build -p reflow2-mcp`):

    python3 tools/test_export_to_reaches_the_daemon.py

Exits 0 when the export lands, 1 otherwise. Standard library only.
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

# The debounce is 2 s of quiet with a 10 s ceiling; give it room without making
# a failure take a minute to report.
DEADLINE = 30.0


def rpc(proc, ident, method, params=None):
    msg = {"jsonrpc": "2.0", "method": method, "id": ident}
    if params is not None:
        msg["params"] = params
    proc.stdin.write(json.dumps(msg) + "\n")
    proc.stdin.flush()
    line = proc.stdout.readline()
    if not line:
        raise SystemExit(f"server closed stdout.\nstderr:\n{proc.stderr.read()}")
    return json.loads(line)


def main() -> int:
    if not BINARY.exists():
        print(f"SKIP: {BINARY} not built (cargo build -p reflow2-mcp)")
        return 0

    work = pathlib.Path(tempfile.mkdtemp(prefix="reflow2-export-to-daemon-"))
    graph = work / "graph"
    export = work / "docs" / "design" / "kept-current.json"
    export.parent.mkdir(parents=True)
    proc = None
    try:
        proc = subprocess.Popen(
            [
                str(BINARY),
                "--graph-path",
                str(graph),
                "--export-to",
                str(export),
                "--shared",
            ],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
            env={**os.environ, "RUST_LOG": "warn"},
        )
        rpc(
            proc,
            1,
            "initialize",
            {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "export-to-probe", "version": "0"},
            },
        )
        proc.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n")
        proc.stdin.flush()

        assert not export.exists(), "nothing should be on disk before the first write"

        out = rpc(
            proc,
            2,
            "tools/call",
            {
                "name": "add_requirement",
                "arguments": {
                    "id": "req:the-flag-crossed-the-process-boundary",
                    "name": "The flag crossed the process boundary",
                    "statement": "A --shared client passes --export-to to the daemon it starts.",
                },
            },
        )
        if "error" in out:
            print(f"FAIL: the write itself failed: {out['error']}")
            return 1

        start = time.monotonic()
        while time.monotonic() - start < DEADLINE:
            if export.exists():
                break
            time.sleep(0.25)

        if not export.exists():
            print(
                f"FAIL: {export} never appeared in {DEADLINE:.0f}s.\n"
                "      The --shared client is a proxy: if it does not forward --export-to to\n"
                "      the daemon it spawns, the write-through never runs in the configuration\n"
                "      every installed project actually uses."
            )
            # NOT proc.stderr.read() here: the client is still running, so the
            # pipe never closes and the read blocks forever — which turned a
            # 30-second failure into a hung run the first time this was proved
            # to fail. The daemon's own diagnostics are in the server log beside
            # the graph, which outlives this process.
            print(f"      the daemon's log is beside {graph}")
            return 1

        doc = json.loads(export.read_text())
        ids = {n.get("node_id") for n in doc.get("nodes", [])}
        if "req:the-flag-crossed-the-process-boundary" not in ids:
            print(f"FAIL: {export} exists but does not hold the node that was written: {ids}")
            return 1

        print(
            f"OK: --export-to reached the daemon — {export.name} was written "
            f"{time.monotonic() - start:.1f}s after the write, with nobody asking."
        )
        return 0
    finally:
        if proc is not None:
            proc.stdin.close()
            proc.terminate()
            try:
                proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                proc.kill()
        # Stop the daemon this test started, so it does not outlive the run and
        # hold a store lock on a directory that is about to vanish.
        subprocess.run(
            [str(BINARY), "--graph-path", str(graph), "--stop-shared"],
            capture_output=True,
            text=True,
            timeout=30,
        )
        shutil.rmtree(work, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
