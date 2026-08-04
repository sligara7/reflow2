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

    def scan_all(self, node_type: str) -> list:
        """Every node of `node_type` — paged, because one reply is not all of them.

        `scan_nodes` answers with as many nodes as fit and says what it withheld
        (`total` vs `returned`, plus `omitted`, `next_offset`, `capped_by`).
        `call` unwraps the `{count, items}` envelope to the items and throws
        those fields away, so a capped page arrives here looking exactly like a
        complete set — and this gate then asserted `exhaustive: true` over it.

        Measured on reflow2's own design 2026-08-04: `capped_by: "size"`,
        `total: 144`, `returned: 124`, `omitted: 20`. Twenty registered
        artifacts were never hashed, so a drifted file among them could not be
        reported — `art:tools-coherence` drifted in this very commit and the
        gate passed it in silence, while `reconcile_artifacts` named it the
        moment it was asked directly. A gate that measures 86% of the tree and
        reports as though it measured all of it is the false-green this whole
        file exists to prevent.

        The count is checked, not assumed: paging that silently comes up short
        would rebuild the same bug one layer down.
        """
        out, offset = [], 0
        while True:
            resp = self._rpc(
                "tools/call",
                {
                    "name": "scan_nodes",
                    "arguments": {"node_type": node_type, "offset": offset},
                },
            )
            if "error" in resp:
                die(2, f"scan_nodes: {resp['error'].get('message', resp['error'])}")
            env = resp["result"].get("structuredContent") or {}
            out.extend(env.get("items") or [])
            nxt, total = env.get("next_offset"), env.get("total")
            if nxt is None or nxt <= offset:
                break
            offset = nxt
        if total is not None and len(out) != total:
            die(
                2,
                f"scan_nodes({node_type}) paged to {len(out)} of {total} — the sweep "
                f"is short, and a short sweep reports OK over whatever it missed.",
            )
        return out

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
            # Presence, not exact set: an orientation read can add a sibling
            # `loop_hint` (BL-91) to the {count, items} envelope, and that extra
            # key must not defeat the list unwrap.
            if isinstance(value, dict) and {"count", "items"} <= value.keys():
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


def hash_file(path: str) -> str:
    """The whole sha256 of the file — what an honest observer computes.

    This used to truncate the digest to the registered checksum's length,
    because designs register anything from 16 hex chars to the full 64 and
    `reconcile_artifacts` compared strings. That workaround is why the gate
    reported OK on 2026-08-01 in the same minute a direct sweep of the same
    clean tree called 51 artifacts drifted: **the compensation lived in the
    wrong layer**, so every consumer that was not this file hit the bug. The
    core now answers it for everyone (BL-160, `artifact::checksums_agree`), and
    the gate reports what it actually measured."""
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 16), b""):
            h.update(chunk)
    return f"sha256:{h.hexdigest()}"


def _git(args: list[str], cwd: str) -> str | None:
    """Run a git command, or None if git is unavailable or the command failed.
    Never raises: the lineage check is a bonus, and a project without git must
    still be able to run the gate."""
    try:
        out = subprocess.run(
            ["git", *args], capture_output=True, text=True, timeout=60, cwd=cwd
        )
    except (OSError, subprocess.SubprocessError):
        return None
    return out.stdout if out.returncode == 0 else None


def _repo_relative(path: str) -> tuple[str, str] | None:
    """`(repo_root, path_within_repo)`, or None when the file is not in a git
    working tree. `git show REV:path` only understands repo-relative paths, so
    an absolute --export would otherwise skip the check without saying so."""
    directory = os.path.dirname(os.path.abspath(path)) or "."
    top = _git(["rev-parse", "--show-toplevel"], directory)
    if not top:
        return None
    root = top.strip()
    try:
        rel = os.path.relpath(os.path.abspath(path), root)
    except ValueError:
        return None
    if rel.startswith(".."):
        return None
    return root, rel.replace(os.sep, "/")


def _export_at(rev: str, root: str, rel: str) -> dict | None:
    """The export document as of a git revision, or None when there isn't one
    (untracked, or the revision predates the file)."""
    out = _git(["show", f"{rev}:{rel}"], root)
    if not out or not out.strip():
        return None
    try:
        return json.loads(out)
    except ValueError:
        return None


def _export_pair(path: str, doc: dict) -> tuple[dict, dict] | None:
    """This export and the one it replaced, or None when unanswerable.

    Two contexts, one rule. Before a commit the working file is new and its
    predecessor is HEAD's version; in CI the working file IS HEAD's version, so
    the pair is HEAD against HEAD~1. Either way we return a document and the one
    it replaced.

    Shared by every check that compares an export with its predecessor, rather
    than being reimplemented per check. Two copies of a predicate drift, and
    when they do they give contradictory answers about the same file — the
    defect [BL-177] records in `reflow2_init.py`, where the dry run and the real
    run disagreed because each tested its own version of "would this change?".
    """
    located = _repo_relative(path)
    if located is None:
        return None  # not in a git working tree — nothing to compare against
    root, rel = located
    head = _export_at("HEAD", root, rel)
    if head is None:
        return None  # untracked, no commits yet, or the commit introducing it
    if head.get("content_hash") != doc.get("content_hash"):
        current, previous = doc, head  # a new export, not yet committed
    else:
        previous = _export_at("HEAD~1", root, rel)
        if previous is None:
            return None  # HEAD is the first commit carrying this export
        current = head
    if previous.get("content_hash") == current.get("content_hash"):
        return None  # content unchanged — nothing replaced anything
    return current, previous


def check_export_identity(path: str, doc: dict) -> str | None:
    """Refuse a design that changed its NAME without anyone saying so (BL-169).

    `graph_id` is the design's durable identity: minted once, never negotiated,
    and it namespaces every stored key — so a graph reopened under a different
    name finds nothing and presents as an empty design. It is also inside the
    export's `content_hash`, which means a rename is indistinguishable from
    ordinary content change to every other check here.

    That is not hypothetical. On 2026-08-02 an export replayed through a temp
    graph came back as `05a6fbe860bf7a23` where the design had been `reflow2`
    since its first commit, and it was committed and pushed. **The lineage check
    passed** (the chain was intact across the rename), the integrity check
    passed (the hash matched its own content), and **both CI jobs were green.**
    The only signal anywhere was a `provenance_note` string in `compare_designs`
    that nothing gates on. A design's identity moving is either deliberate or a
    bug, and it must not be able to happen quietly.

    Returns a failure message, or None when sound or unanswerable. A first
    export has no predecessor to disagree with, and an unidentified document
    (`graph_id: ""`, legitimate for a hand-authored one — BL-138) is not a
    rename: absence of a name is not a different name.
    """
    pair = _export_pair(path, doc)
    if pair is None:
        return None
    current, previous = pair
    was, now = previous.get("graph_id"), current.get("graph_id")
    if not was or not now or was == now:
        return None
    return (
        f"IDENTITY  '{path}' changed the design's name from '{was}' to '{now}'. "
        f"`graph_id` is minted once and never negotiated — it namespaces every "
        f"stored key, so a store reopened under a different name finds nothing "
        f"and reads as an EMPTY design — and it sits inside the content hash, "
        f"which is why every other check here passes across a rename. The usual "
        f"cause is a replay: an export imported into a TEMP graph through the "
        f"`import_graph` tool and re-exported from there takes the temp store's "
        f"name. Seed a replay with the CLI (`reflow2-mcp --graph-path <tmp> "
        f"--import <doc>`), which adopts the document's identity into an empty "
        f"store. If the rename is deliberate, commit it on its own so it is "
        f"reviewable as what it is."
    )


def check_export_chain(path: str, doc: dict) -> str | None:
    """Verify this export links to its predecessor (`dec:export-hash-chain`).

    The chain gives the design a history independent of git: each export records
    the `content_hash` of the one it replaced. `export_graph --path` builds that
    link from **whatever file is already at the target path**, so exporting to a
    scratch path and copying the result into place severs it — silently, which is
    how six consecutive commits lost the link in July 2026 with the gate green,
    the loop clean and zero gaps every time (BL-107).

    Two contexts, one rule. Before a commit the working file is new and its
    predecessor is HEAD's version; in CI the working file IS HEAD's version, so
    the pair to check is HEAD against HEAD~1. Either way we compare a document
    with the one it replaced.

    Returns a failure message, or None when sound OR when there is nothing to
    check against — an unanswerable question is skipped, never guessed. The
    chain deliberately does not advance while content is unchanged, and a first
    export has no predecessor; neither is a break.
    """
    pair = _export_pair(path, doc)
    if pair is None:
        return None
    current, previous = pair
    expected = previous.get("content_hash")
    actual = current.get("prev_content_hash")
    if not expected or actual == expected:
        return None
    was = "nothing" if actual is None else actual
    return (
        f"LINEAGE  '{path}' does not link to the export it replaced: it records "
        f"{was} where {expected} is expected. The design's history is independent "
        f"of git and this severs it. Two causes, both fixed the same way. Either "
        f"the export was written somewhere else and copied into place — the link "
        f"is built from the file already at the target, so there was nothing to "
        f"link to — or the design was exported MORE THAN ONCE since the last "
        f"commit, chaining through an intermediate that will never be committed "
        f"and leaving the committed history a hole. Restore the committed file "
        f"(`git checkout {path}`) and export straight onto it, once."
    )


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

    # Integrity first (dec:export-hash-chain): the export carries a hash of its
    # own content, so a committed record that was hand-edited or corrupted is
    # detectable before anything downstream trusts it. The canonical form must
    # byte-match the Rust side's: compact separators, sorted keys, raw unicode
    # (tools/smoke_mcp.py pins the two implementations against each other).
    try:
        with open(opts.export, encoding="utf-8") as fh:
            doc = json.load(fh)
    except (OSError, ValueError) as e:
        die(2, f"could not read '{opts.export}' as JSON: {e}")
    embedded = doc.get("content_hash")
    if embedded:
        canonical = json.dumps(
            {"edges": doc.get("edges", []), "graph_id": doc.get("graph_id"),
             "nodes": doc.get("nodes", [])},
            sort_keys=True, ensure_ascii=False, separators=(",", ":"),
        )
        actual = "sha256:" + hashlib.sha256(canonical.encode("utf-8")).hexdigest()
        if actual != embedded:
            failures.append(
                f"INTEGRITY  '{opts.export}' does not match its own content_hash — the "
                f"committed design record was edited outside reflow2 or corrupted. "
                f"Re-export from the graph, or review what changed it."
            )
    else:
        notes.append("integrity: export predates content hashing (no content_hash)")

    # LINEAGE (BL-107) — a severed chain used to be completely silent.
    broken_chain = check_export_chain(opts.export, doc)
    if broken_chain:
        failures.append(broken_chain)

    # IDENTITY (BL-169) — a rename passes every check above, because graph_id is
    # inside the content hash and the chain links across it perfectly well.
    renamed = check_export_identity(opts.export, doc)
    if renamed:
        failures.append(renamed)

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
            # Paged, not one reply: `exhaustive: true` below is a CLAIM, and it
            # was false by 20 artifacts until this used scan_all.
            artifacts = server.scan_all("Artifact")
            observed = []
            for art in artifacts:
                props = art.get("properties", {})
                location = props.get("location") or props.get("name")
                path = os.path.join(opts.root, location) if location else None
                if not path or not os.path.exists(path):
                    observed.append({"artifact_id": art["node_id"], "present": False})
                    continue
                observed.append(
                    {
                        "artifact_id": art["node_id"],
                        "present": True,
                        "checksum": hash_file(path),
                    }
                )

            drift = server.call(
                "reconcile_artifacts", {"observed": observed, "exhaustive": True}
            )
            for finding in drift.get("findings", []):
                kind = finding.get("kind")
                what = f"{finding.get('artifact_id')}: {kind}"
                if kind in ("checksum_change", "missing_artifact"):
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
