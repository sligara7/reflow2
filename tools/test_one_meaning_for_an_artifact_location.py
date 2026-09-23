#!/usr/bin/env python3
"""The server and the CI gate give ONE answer about every artifact location.

# The class this pins (2026-09-23)

Two instruments ask "does the file this Artifact points at still match the
design?": the server, when `loop_status` and `reconcile_artifacts` measure
the registered set (crates/reflow2-mcp/src/measure.rs), and the CI gate
(tools/reflow2_check.py). Each decided on its own what a location MEANS, and
every new shape of location got taught to one of them only:

- A URI (`https://...`) was "not judged" to the gate and MISSING to the
  server, so flo2's session went on reporting twelve missing files after its
  CI turned green
  (fact:the-server-measures-a-uri-location-as-an-absent-file-while-the-gate-calls-it-not-judged-2026-09-23).
- A path outside the project was `outside_root` and not a finding to the
  server, and was hashed or failed as missing by the gate depending on which
  machine ran it
  (fact:the-out-of-tree-notion-exists-but-is-keyed-on-the-path-shape-and-only-one-instrument-honours-it).
- A `file#fragment` was measured as the file by the server and was missing to
  the gate.

The same Artifact in the same state was a note to one instrument and a red
build to the other. So this does not test either instrument alone. It builds
ONE project holding every location shape, asks both, and requires the same
verdict from each: ok, changed, missing, or not judged.

Usage (from the repo root, after `cargo build -p reflow2-mcp`):

    python3 tools/test_one_meaning_for_an_artifact_location.py

Exits 0 when the two agree on every shape, 1 otherwise. Standard library only.
"""

import hashlib
import json
import os
import pathlib
import re
import subprocess
import sys
import tempfile

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from smoke_mcp import Server  # noqa: E402

REPO = pathlib.Path(__file__).resolve().parent.parent
BINARY = os.environ.get("REFLOW2_BIN") or str(REPO / "target" / "debug" / "reflow2-mcp")
CHECK = REPO / "tools" / "reflow2_check.py"


def sha(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def build(tmp: pathlib.Path):
    """A project with one Artifact per location shape, and what each SHOULD be.

    The expected verdicts are the agreement, not either instrument's old
    answer: a file under the root is measured; anything the project cannot
    measure (a URI, somewhere outside it, a directory) is not judged and is
    never a failure; a path under the root with nothing there is missing.
    """
    proj = tmp / "proj"
    (proj / "src").mkdir(parents=True)
    (proj / ".reflow2").mkdir()
    (proj / "docs").mkdir()
    (proj / "src" / "same.py").write_bytes(b"print('same')\n")
    (proj / "src" / "moved.py").write_bytes(b"print('after')\n")
    (proj / "src" / "part.py").write_bytes(b"def main(): pass\n")
    elsewhere = tmp / "elsewhere.txt"
    elsewhere.write_bytes(b"not this project's\n")

    same = sha(b"print('same')\n")
    shapes = [
        # (id, location, checksum recorded in the design, expected verdict)
        ("art:a-file-that-matches", "src/same.py", same, "ok"),
        ("art:a-file-that-changed", "src/moved.py", sha(b"print('before')\n"), "changed"),
        ("art:a-file-that-is-gone", "src/gone.py", same, "missing"),
        ("art:a-fragment-of-a-file", "src/part.py#main", sha(b"def main(): pass\n"), "ok"),
        ("art:an-absolute-path-inside", str(proj / "src" / "same.py"), same, "ok"),
        ("art:a-directory", "docs", None, "not_judged"),
        ("art:a-uri", "https://example.invalid/spec.yaml", same, "not_judged"),
        ("art:an-absolute-path-outside", str(elsewhere), sha(elsewhere.read_bytes()), "not_judged"),
        ("art:an-escape-by-dotdot", "../elsewhere.txt", sha(elsewhere.read_bytes()), "not_judged"),
    ]
    s = Server(BINARY, str(proj / ".reflow2" / "graph"))
    try:
        for ident, location, checksum, _ in shapes:
            args = {"id": ident, "name": ident, "artifact_type": "document", "location": location}
            if checksum:
                args["checksum"] = checksum
            s.call("add_artifact", args)
        export = proj / "design.json"
        s.call("export_graph", {"path": str(export), "overwrite": True})
    finally:
        s.close()
    return proj, export, shapes


def server_verdicts(proj: pathlib.Path, ids) -> dict:
    """What the SERVER says — the no-argument reconcile that loop_status also runs."""
    out = subprocess.run(
        [BINARY, "--graph-path", str(proj / ".reflow2" / "graph"),
         "--call", "reconcile_artifacts", "--args", "{}"],
        capture_output=True, text=True, timeout=120,
    )
    if out.returncode != 0:
        raise SystemExit(f"the server's reconcile did not run:\n{out.stderr[-2000:]}")
    reply = json.loads(out.stdout)
    verdict = {i: "ok" for i in ids}
    for f in reply.get("findings", []):
        kind = f.get("kind")
        if kind == "missing_artifact":
            verdict[f["artifact_id"]] = "missing"
        elif kind == "checksum_change":
            verdict[f["artifact_id"]] = "changed"
        elif kind == "no_baseline":
            verdict[f["artifact_id"]] = "not_judged"
    for u in reply.get("measurement", {}).get("unmeasurable", []):
        verdict[u["artifact_id"]] = "not_judged"
    return verdict


def gate_verdicts(proj: pathlib.Path, export: pathlib.Path, ids) -> dict:
    """What the CI GATE says, read from the lines it prints per artifact."""
    out = subprocess.run(
        [sys.executable, str(CHECK), "--export", str(export), "--root", str(proj),
         "--bin", BINARY],
        capture_output=True, text=True, timeout=300,
    )
    if out.returncode == 2:
        raise SystemExit(f"the gate could not run:\n{out.stdout[-2000:]}\n{out.stderr[-2000:]}")
    verdict = {i: "ok" for i in ids}
    for line in out.stdout.splitlines():
        m = re.search(r"DRIFT\s+(art:\S+): (missing_artifact|checksum_change)", line)
        if m:
            verdict[m.group(1)] = "missing" if m.group(2) == "missing_artifact" else "changed"
            continue
        m = re.search(r"not judged: (art:\S+)", line) or re.search(
            r"drift: (art:\S+): no_baseline", line)
        if m:
            verdict[m.group(1)] = "not_judged"
    return verdict


def main() -> int:
    if not os.path.exists(BINARY):
        print(f"SKIP: {BINARY} is not built")
        return 1
    with tempfile.TemporaryDirectory() as t:
        proj, export, shapes = build(pathlib.Path(t))
        ids = [s[0] for s in shapes]
        server = server_verdicts(proj, ids)
        gate = gate_verdicts(proj, export, ids)

    failed = 0
    for ident, location, _, expected in shapes:
        s, g = server[ident], gate[ident]
        ok = s == g == expected
        failed += not ok
        print(f"{'ok  ' if ok else 'FAIL'}  {location!r:44} expected {expected:10} "
              f"server {s:10} gate {g}")
    if failed:
        print(f"\n{failed} location shape(s) where the server and the gate do not give the "
              f"agreed answer")
        return 1
    print("\nthe server and the gate agree on every location shape")
    return 0


if __name__ == "__main__":
    sys.exit(main())
