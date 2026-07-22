#!/usr/bin/env python3
"""reflow2 check — the consumer CI coherence gate (BL-66).

Answers one question on every commit, loudly: **does the committed design still
describe this build?** It reads the design from the committed export (never the
live `.reflow2/graph` — that directory is gitignored, machine-local, and
single-writer, so CI cannot and should not open it), recomputes every
registered artifact's hash from the working tree, reconciles, and runs the gap
detectors.

    tools/reflow2_check.py                          # design.json, cwd as root
    tools/reflow2_check.py --export docs/design/reflow2.json
    tools/reflow2_check.py --gap-threshold 0.9

The build FAILS (exit 1) when:
  - a registered artifact changed or vanished with no two-sided accept — an
    accepted drift updates the export, so a red here means the accept step was
    skipped, which is exactly the erosion this gate exists to catch; or
  - an **anchored** gap (one that names design nodes) at or above
    `--gap-threshold` (default 0.8) is open. Gaps the team has consciously
    accepted via `acknowledge_gap` are not reported by `detect_gaps`, so
    acknowledging — with a reason, on the record — is the sanctioned way to go
    green without fixing. Phase-level nudges ("what comes next") never fail
    the build; they are advice, not defects.

Everything else is printed but does not gate: `no_baseline` artifacts (no hash
registered — register one via the link-artifacts flow), sub-threshold gaps,
and unanchored nudges. Exit codes: 0 coherent · 1 gate failed · 2 could not
run (missing export/binary — never a silent pass).

Standard library only; needs the `reflow2-mcp` binary (`--bin`, `$REFLOW2_BIN`,
on PATH, or a local cargo build).
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile

_REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def die(code: int, msg: str) -> None:
    print(f"reflow2_check: {msg}", file=sys.stderr)
    sys.exit(code)


def default_bin() -> str:
    env = os.environ.get("REFLOW2_BIN")
    if env:
        return env
    for candidate in (
        os.path.join(_REPO_ROOT, "target", "debug", "reflow2-mcp"),
        os.path.join(_REPO_ROOT, "target", "release", "reflow2-mcp"),
    ):
        if os.path.exists(candidate):
            return candidate
    found = shutil.which("reflow2-mcp")
    return found or "reflow2-mcp"


class Server:
    """A short-lived reflow2-mcp process spoken to over stdio JSON-RPC.

    The same tiny client as tools/reflow2_cli.py, embedded so this file is
    self-contained — it ships in the consumer kit alone.
    """

    def __init__(self, binary: str, graph_path: str) -> None:
        try:
            self.proc = subprocess.Popen(
                [binary, "--graph-path", graph_path],
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                bufsize=1,
                env={**os.environ, "RUST_LOG": os.environ.get("RUST_LOG", "warn")},
            )
        except FileNotFoundError:
            die(2, f"binary not found: {binary} (set --bin or $REFLOW2_BIN)")
        self._id = 0
        self._rpc(
            "initialize",
            {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "reflow2_check", "version": "0"},
            },
        )
        self._rpc("notifications/initialized", {}, notify=True)

    def _rpc(self, method: str, params=None, notify: bool = False):
        msg = {"jsonrpc": "2.0", "method": method}
        if params is not None:
            msg["params"] = params
        if not notify:
            self._id += 1
            msg["id"] = self._id
        self.proc.stdin.write(json.dumps(msg) + "\n")
        self.proc.stdin.flush()
        if notify:
            return None
        line = self.proc.stdout.readline()
        if not line:
            err = (self.proc.stderr.read() or "").strip()
            die(2, f"server exited without responding.\n{err}")
        return json.loads(line)

    def call(self, tool: str, args: dict):
        resp = self._rpc("tools/call", {"name": tool, "arguments": args})
        if "error" in resp:
            die(2, f"{tool}: {resp['error'].get('message', resp['error'])}")
        result = resp["result"]
        if result.get("isError"):
            blocks = result.get("content") or []
            text = blocks[0].get("text") if blocks else str(result)
            die(2, f"{tool}: {text}")
        if "structuredContent" in result:
            value = result["structuredContent"]
            if isinstance(value, dict) and set(value) == {"count", "items"}:
                return value["items"]
            return value
        blocks = result.get("content") or []
        return json.loads(blocks[0]["text"]) if blocks else None

    def close(self) -> None:
        try:
            self.proc.stdin.close()
        except Exception:
            pass
        self.proc.terminate()
        self.proc.wait(timeout=10)


def hash_file(path: str, registered: str | None) -> str | None:
    """sha256 of the file, truncated to match the registered checksum's length
    (designs register anything from 16 hex chars to the full 64; reconcile
    compares strings, so the observation must speak the same dialect)."""
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 16), b""):
            h.update(chunk)
    digest = h.hexdigest()
    if registered and registered.startswith("sha256:"):
        digest = digest[: len(registered) - len("sha256:")]
    return f"sha256:{digest}"


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--export", default="design.json", help="committed design export (JSON)")
    ap.add_argument("--root", default=".", help="project root artifact locations are relative to")
    ap.add_argument("--bin", default=default_bin(), help="reflow2-mcp binary")
    ap.add_argument(
        "--gap-threshold",
        type=float,
        default=0.8,
        help="anchored gaps at/above this severity fail the build (default 0.8)",
    )
    opts = ap.parse_args()

    if not os.path.exists(opts.export):
        die(
            2,
            f"no design export at '{opts.export}'. Commit one "
            f"(export_graph to a repo path, or reflow2-mcp --export) and point --export at it — "
            f"the gate reads the committed design, never the live .reflow2/ store.",
        )

    failures: list[str] = []
    notes: list[str] = []

    with tempfile.TemporaryDirectory(prefix="reflow2-check-") as tmp:
        graph = os.path.join(tmp, "graph")
        imported = subprocess.run(
            [opts.bin, "--graph-path", graph, "--import", opts.export],
            capture_output=True,
            text=True,
        )
        if imported.returncode != 0:
            die(2, f"could not import '{opts.export}':\n{imported.stderr.strip()}")

        server = Server(opts.bin, graph)
        try:
            artifacts = server.call("scan_nodes", {"node_type": "Artifact"}) or []
            observed = []
            for art in artifacts:
                props = art.get("properties", {})
                location = props.get("location") or props.get("name")
                registered = props.get("checksum")
                path = os.path.join(opts.root, location) if location else None
                if not path or not os.path.exists(path):
                    observed.append({"artifact_id": art["node_id"], "present": False})
                    continue
                entry = {"artifact_id": art["node_id"], "present": True}
                checksum = hash_file(path, registered)
                if checksum:
                    entry["checksum"] = checksum
                observed.append(entry)

            drift = server.call(
                "reconcile_artifacts", {"observed": observed, "exhaustive": True}
            )
            for finding in drift.get("findings", []):
                kind = finding.get("kind")
                what = f"{finding.get('artifact_id')}: {kind}"
                if kind in ("checksum_change", "missing"):
                    failures.append(
                        f"DRIFT  {what} — the build no longer matches the committed design. "
                        f"Reconcile and accept two-sided (set_artifact_checksum), then re-export."
                    )
                else:
                    notes.append(f"drift: {what}")

            gaps = server.call("detect_gaps", {}) or []
            for gap in gaps:
                anchored = bool(gap.get("affected_ids"))
                severity = float(gap.get("severity", 0.0))
                line = f"{gap.get('id')} [{severity:.2f}] {gap.get('title')}"
                if anchored and severity >= opts.gap_threshold:
                    failures.append(
                        f"GAP    {line} — fix it, or accept it on the record (acknowledge_gap)."
                    )
                else:
                    notes.append(f"gap: {line}" + ("" if anchored else " (phase nudge)"))
        finally:
            server.close()

    for note in notes:
        print(f"  note  {note}")
    for failure in failures:
        print(f"  FAIL  {failure}")
    if failures:
        print(f"\nreflow2 check: FAILED — {len(failures)} finding(s), {len(notes)} note(s).")
        return 1
    print(f"\nreflow2 check: OK — design and build agree ({len(notes)} note(s)).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
