#!/usr/bin/env python3
"""A lesson is served at the step it concerns — over the wire, on the real binary.

`req:a-lesson-is-served-at-the-step-it-concerns` (increment 438). The Rust
suite pins the mechanism through handler calls; what only the wire can show is
that `tools/list` — the listing a harness actually reads before every call —
carries the lesson on the named tool's description, and that the listing of an
EMPTY design is byte-for-byte what it was (the toolsnap goldens pin that too,
but from a different process; this checks it in the same one, before and after).

Usage (from the repo root, after `cargo build -p reflow2-mcp`):

    python3 tools/test_a_lesson_is_served_at_the_step.py

Exits 0 when every pin holds, 1 otherwise. Standard library only.
"""

import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile

REPO = pathlib.Path(__file__).resolve().parent.parent
BINARY = REPO / "target" / "debug" / "reflow2-mcp"


class Server:
    def __init__(self, *args):
        self.proc = subprocess.Popen(
            [str(BINARY), *args],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            bufsize=1,
            env={**os.environ, "RUST_LOG": "warn"},
        )
        self.ident = 0
        self.rpc(
            "initialize",
            {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "lesson-probe", "version": "0"},
            },
        )
        self.proc.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n")
        self.proc.stdin.flush()

    def rpc(self, method, params=None):
        self.ident += 1
        msg = {"jsonrpc": "2.0", "method": method, "id": self.ident}
        if params is not None:
            msg["params"] = params
        self.proc.stdin.write(json.dumps(msg) + "\n")
        self.proc.stdin.flush()
        line = self.proc.stdout.readline()
        if not line:
            raise SystemExit("server closed stdout")
        return json.loads(line)

    def call(self, name, arguments):
        return self.rpc("tools/call", {"name": name, "arguments": arguments})

    def tools(self):
        return {t["name"]: t for t in self.rpc("tools/list", {})["result"]["tools"]}

    def close(self):
        self.proc.stdin.close()
        self.proc.terminate()
        try:
            self.proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self.proc.kill()


def main() -> int:
    if not BINARY.exists():
        print(f"SKIP: {BINARY} not built (cargo build -p reflow2-mcp)")
        return 0
    failures = []

    def check(name, ok, detail=""):
        print(("ok   " if ok else "FAIL ") + name + (f" — {detail}" if detail and not ok else ""))
        if not ok:
            failures.append(name)

    work = pathlib.Path(tempfile.mkdtemp(prefix="reflow2-lesson-at-step-"))
    try:
        s = Server("--graph-path", str(work / "graph"))
        before = s.tools()
        check("an empty design lists 180+ tools", len(before) >= 180, str(len(before)))
        check(
            "no listed description carries a lesson block on an empty design",
            not any("LESSONS THIS DESIGN HOLDS" in (t.get("description") or "") for t in before.values()),
        )
        r = s.call(
            "add_project",
            {"id": "proj:p", "name": "P", "description": "A project with a lesson."},
        )
        check("the project write succeeded", "error" not in r and not r["result"].get("isError"), str(r)[:200])
        r = s.call(
            "record_finding",
            {
                "id": "fact:format-before-the-export",
                "subject_id": "proj:p",
                "name": "Format before the export",
                "statement": "cargo fmt after the export forces a second export.",
                "valid_from": "2026-09-12",
                "steps": ["export_graph", "ci-gate"],
            },
        )
        check("the finding with steps was recorded", "error" not in r and not r["result"].get("isError"), str(r)[:300])

        after = s.tools()
        d = after["export_graph"].get("description") or ""
        check(
            "export_graph's listed description now carries the lesson",
            "LESSONS THIS DESIGN HOLDS FOR `export_graph`" in d and "fact:format-before-the-export" in d,
            d[-300:],
        )
        check(
            "the served description is kept whole and the lesson appended",
            d.startswith(before["export_graph"].get("description") or ""),
        )
        touched = [n for n, t in after.items() if "LESSONS THIS DESIGN HOLDS" in (t.get("description") or "")]
        check("exactly one tool carries it", touched == ["export_graph"], str(touched))
        unchanged = sum(
            1 for n in before if n != "export_graph" and before[n].get("description") == after[n].get("description")
        )
        check("every other description is unchanged", unchanged == len(before) - 1, f"{unchanged} of {len(before) - 1}")

        r = s.call("get_skill", {"name": "ci-gate"})
        sc = r.get("result", {}).get("structuredContent") or {}
        ids = [i.get("id") for i in (sc.get("lessons") or {}).get("items", [])]
        check("get_skill(ci-gate) carries the lesson", ids == ["fact:format-before-the-export"], str(ids))
        r = s.call("get_skill", {"name": "brainstorm"})
        sc = r.get("result", {}).get("structuredContent") or {}
        check("get_skill(brainstorm) carries none", "lessons" not in sc)

        r = s.call(
            "record_finding",
            {
                "id": "fact:typo",
                "subject_id": "proj:p",
                "name": "Typo",
                "statement": "x",
                "steps": ["export-graph"],
            },
        )
        text = json.dumps(r)
        check("a step nothing serves is refused, naming the nearest", ("error" in r or r["result"].get("isError")) and "export_graph" in text, text[:300])
        s.close()
    finally:
        shutil.rmtree(work, ignore_errors=True)

    if failures:
        print(f"\nFAIL: {len(failures)} pin(s) broken: {failures}")
        return 1
    print("\nOK: a lesson is served at the step it concerns, and nowhere else.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
