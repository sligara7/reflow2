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
import glob
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import traceback

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


_TOML_SECTION = re.compile(r"^\s*\[([^\]]+)\]\s*$")
_TOML_INLINE = re.compile(r"^\s*([A-Za-z0-9_.-]+)\s*=\s*\{(.+)\}\s*$")
_TOML_UNCLOSED = re.compile(r"^\s*([A-Za-z0-9_.-]+)\s*=\s*\{[^}]*$")
_TOML_KV = re.compile(r'([A-Za-z0-9_-]+)\s*=\s*"([^"]*)"')


def _dependency_tables(path: str) -> tuple[list[dict], list[str]]:
    """Every `*dependencies` table in a Cargo manifest, and what could not be read.

    Prefers `tomllib` (Python 3.11+), which is exact. Falls back to a LINE READER
    when it is absent — and that fallback exists because of a measured failure,
    not a hypothetical one: this gate shipped reading only `tomllib`, and its
    first CI run reported "could not read any build file to check them" on
    reflow2's own repo. The runner is ubuntu-22.04, which carries Python 3.10.
    Most CI in the world is older than 3.11, so the dependency check was inert
    almost everywhere it mattered while being perfectly correct locally.

    The fallback is deliberately narrow — single-line inline tables, which is how
    Cargo pins are almost always written — and it REPORTS WHAT IT CANNOT READ
    rather than skipping quietly. A parser that silently drops the shapes it does
    not know rebuilds, one level down, exactly the silence this gate was extended
    to remove. The kit stays install-free: no `tomli`, no pip, because reflow2
    installs into other people's projects and a runtime dependency is a cost they
    did not agree to.
    """
    try:
        import tomllib
    except ModuleNotFoundError:
        tomllib = None

    if tomllib is not None:
        try:
            with open(path, "rb") as fh:
                data = tomllib.load(fh)
        except (OSError, ValueError):
            return [], [f"{path}: unreadable as TOML"]
        return [
            data.get("dependencies", {}) or {},
            data.get("dev-dependencies", {}) or {},
            (data.get("workspace", {}) or {}).get("dependencies", {}) or {},
        ], []

    tables: dict[str, dict] = {}
    unparsed: list[str] = []
    section = ""
    try:
        with open(path, encoding="utf-8") as fh:
            lines = fh.readlines()
    except OSError:
        return [], [f"{path}: unreadable"]

    for line in lines:
        if (m := _TOML_SECTION.match(line)) is not None:
            section = m.group(1).strip()
            # `[dependencies.serde]` is a sub-table per dependency. Legal TOML and
            # not handled here; say so rather than pretend the section was empty.
            if re.search(r"(^|\.)dependencies\.", section):
                unparsed.append(f"{path}: [{section}] sub-table form not read")
            continue
        if not section.endswith("dependencies"):
            continue
        if (m := _TOML_INLINE.match(line)) is not None:
            tables.setdefault(section, {})[m.group(1)] = dict(
                _TOML_KV.findall(m.group(2))
            )
        elif _TOML_UNCLOSED.match(line) is not None:
            unparsed.append(
                f"{path}: '{_TOML_UNCLOSED.match(line).group(1)}' spans lines, not read"
            )
    return list(tables.values()), unparsed


def observe_dependencies(root: str, declared: list) -> tuple[list, list[str]]:
    """What the BUILD actually pins, read fresh from build files.

    Returns `(observations, sources_read, unparsed)`. An empty `sources_read`
    means this gate could not look — which is reported, never treated as "depends
    on nothing" (`reconcile_dependencies` insists on that distinction and the
    gate must honour it). `unparsed` names manifest shapes that WERE seen and
    could not be read, so a partial read is never mistaken for a complete one.

    MATCHED BY SOURCE, NOT BY NAME, and that is the whole difficulty. A design
    declares ONE dependency — `dynograph-foundation` — while the build names
    five crates from it (`dynograph-core`, `-storage`, `-graph`, …). Matching on
    name would report the declaration as unobserved and all five crates as
    undeclared, which is four false findings and one backwards one. The stable
    identifier both sides share is the SOURCE (a git URL, a registry), so
    observations are grouped by source and reported under the declared name.

    Only Cargo is read today. That is a real limit, not a hidden one: a project
    whose pins live in package.json, pyproject.toml, go.mod or versions.env gets
    `sources_read == []` and is told so plainly.
    """
    by_source: dict[str, dict] = {}
    sources_read: list[str] = []
    unparsed: list[str] = []

    manifests = [os.path.join(root, "Cargo.toml")]
    manifests += sorted(glob.glob(os.path.join(root, "crates", "*", "Cargo.toml")))
    for manifest in manifests:
        if not os.path.exists(manifest):
            continue
        tables, could_not_read = _dependency_tables(manifest)
        unparsed.extend(could_not_read)
        if not tables and could_not_read:
            continue
        sources_read.append(os.path.relpath(manifest, root))
        for table in tables:
            for crate, spec in (table or {}).items():
                if not isinstance(spec, dict):
                    continue  # a bare "1.2.3" is a crates.io pin with no shared source
                source = spec.get("git") or spec.get("path") or spec.get("registry")
                if not source:
                    continue
                # A git dep's VERSION is whatever pins it. `tag` first: it is the
                # only one a human wrote on purpose and the only one a provider
                # can be held to.
                version = spec.get("tag") or spec.get("rev") or spec.get("branch") or spec.get("version")
                entry = by_source.setdefault(
                    source, {"version": version, "components": set(), "features": set()}
                )
                entry["components"].add(crate)
                for feat in spec.get("features", []) or []:
                    entry["features"].add(feat)
                if entry["version"] is None:
                    entry["version"] = version

    # Report each observed source under the DECLARED name when one claims it, so
    # the reconcile compares like with like. An observation matching nothing
    # declared keeps its own source as its name — that is the "reliance nobody
    # agreed to", and it should surface rather than be quietly dropped.
    declared_by_source = {
        (d.get("source") or "").strip(): d.get("name") for d in declared if d.get("source")
    }
    observations = []
    for source, entry in sorted(by_source.items()):
        if entry["version"] is None:
            continue  # a path/registry dep with no pin says nothing about version
        observations.append(
            {
                "name": declared_by_source.get(source, source),
                "version": entry["version"],
                "components": sorted(entry["components"]),
                "features": sorted(entry["features"]),
                "observed_in": ", ".join(sources_read),
            }
        )
    return observations, sources_read, unparsed


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
        f"of git and this severs it.\n"
        f"      THE RULE THIS ENFORCES is `dec:export-once-per-pr`, accepted "
        f"2026-08-01: a branch may hold as many commits as it likes, exactly ONE "
        f"of them may write {path}, and it should be the last. This check is the "
        f"only thing that says so out loud, so if you did not know the rule, that "
        f"is what you have just met.\n"
        f"      THREE CAUSES. (1) The export was written somewhere else and "
        f"copied into place — the link is built from the file already at the "
        f"target, so there was nothing to link to. (2) The design was exported "
        f"MORE THAN ONCE since the last commit, chaining through an intermediate "
        f"that will never be committed. Both are fixed the same way: restore the "
        f"committed file (`git checkout {path}`) and export straight onto it, "
        f"ONCE. (3) THIS PR ALREADY HAS AN EARLIER EXPORTING COMMIT, and your "
        f"chain is sound — `git log --name-only origin/main..HEAD -- {path}` "
        f"names them. Restoring and re-exporting does NOT fix this one; FOLD the "
        f"exporting commits into a single last commit instead. Cause (3) is why "
        f"the wording used to mislead: it listed only the first two, and its "
        f"advice was already what the author had done."
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
                # A DIRECTORY IS PRESENT AND UNHASHABLE, WHICH IS A THIRD STATE
                # this loop did not have. It used to fall straight into
                # hash_file() and raise IsADirectoryError out of main() — see
                # below for why that exit code was the worse half of the bug.
                #
                # Reported as present WITHOUT a checksum, which the core already
                # has a word for: `no_baseline`, "cannot be judged — surfaced
                # rather than treated as unchanged". That is exactly true of a
                # directory, and it is a note rather than a failure, so a
                # consumer who registers one gets an honest "I could not judge
                # this" instead of a crash or a quiet pass.
                #
                # ⭐ REFLOW2'S OWN GRAPH CANNOT REACH THIS LINE: 182 artifacts
                # carry a location and NONE resolves to a directory, while
                # dev_storyflow has 11. CI has run this gate on every push for
                # weeks and never once executed this branch (reported by the
                # api-boss fleet, 2026-08-15, and verified here).
                if os.path.isdir(path):
                    observed.append({"artifact_id": art["node_id"], "present": True})
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

            # DEPENDENCIES — the design says which version of another design it
            # depends on (req:design-dependencies-declared, accepted). That has
            # been checkable since the capability shipped and NOTHING A CONSUMER
            # RUNS EVER CHECKED IT: this gate reconciled artifacts and stopped.
            # A declaration nobody verifies is a promise, not a check.
            #
            # ⭐ WHY THE EMPTY CASE IS HANDLED SEPARATELY RATHER THAN JUST
            # RECONCILING: with no observations every declared dependency comes
            # back `unobserved`, so a project whose pins this gate cannot read
            # would fail for having declared anything at all — punishing the
            # correct behaviour. Silence about what was not checked is the bug;
            # inventing findings about it is a worse one.
            # The tool answers `{manifest, report}` — the report is the nested
            # half. Reading `declared` off the top level silently yields [] and
            # the gate then reports "0 declared … agree", which is a FALSE GREEN
            # in the code written to prevent false greens. Caught by checking the
            # number against a design known to declare one; it would never have
            # been caught by the gate passing.
            deps = (server.call("reconcile_dependencies", {"observed": []}) or {}).get("report", {})
            declared_deps = deps.get("declared", []) or []
            observed_deps, sources_read, unparsed = observe_dependencies(
                opts.root, declared_deps
            )
            for shape in unparsed:
                notes.append(f"dependencies: {shape}")

            if declared_deps and not sources_read:
                notes.append(
                    f"dependencies: {len(declared_deps)} declared, and this gate could not read "
                    f"any build file to check them — NOTHING VERIFIED THE PINS. Only Cargo is "
                    f"read today; a project pinning elsewhere is unchecked, not clean."
                )
            elif declared_deps or observed_deps:
                report = (server.call(
                    "reconcile_dependencies", {"observed": observed_deps}
                ) or {}).get("report", {})
                for finding in report.get("findings", []):
                    kind = finding.get("kind")
                    detail = finding.get("detail") or kind
                    if kind in ("undeclared", "version_mismatch", "unobserved"):
                        failures.append(
                            f"DEPEND {finding.get('dependency')}: {kind} — {detail} "
                            f"Fix the pin, or re-declare it (declare_dependency), then re-export."
                        )
                    else:
                        notes.append(f"dependency: {finding.get('dependency')}: {kind} — {detail}")
                if not report.get("findings"):
                    notes.append(
                        f"dependencies: {len(declared_deps)} declared, "
                        f"{len(observed_deps)} observed in {', '.join(sources_read)} — agree"
                    )
            # Nothing declared AND nothing observed is genuinely quiet: the design
            # has said it depends on no other design, which is a statement, not a
            # silence — and reconcile's own note already says so if asked.

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


def run_guarded() -> int:
    """`main()`, with the exit-code contract actually enforced.

    THE CONTRACT IS ENFORCED HERE OR NOWHERE. This module's docstring promises
    "0 coherent · 1 gate failed · 2 could not run", and a dedicated code exists
    for could-not-run precisely so a CI job can tell a drifted design from a
    broken tool. Until this guard existed, ANY unhandled exception in `main()`
    landed in Python's own exit 1 — the code reserved for "gate failed" — so the
    one confusion the three-code design was built to prevent was reachable from
    every line of it. Not a silent pass; a MISFILED one, and invisible in a badge
    or a required-check summary where the traceback is the only tell.

    ⭐ A NAMED FUNCTION RATHER THAN A BARE `try` UNDER `__main__`, because a
    guard nobody can call is a claim nobody can check. The first version of this
    fix lived under `__main__` and its test passed with the guard REMOVED — it
    forced a fault `die()` already handled, so it proved nothing. Every fault
    this tool knows about is already routed to 2 correctly; the guard is for the
    ones it does not know about, and the only honest way to exercise that is to
    hand it one.

    `die()` raises SystemExit, which must pass through untouched so an explicit
    exit code wins — and it does, because SystemExit derives from BaseException
    rather than Exception.
    """
    try:
        return main()
    except Exception:  # noqa: BLE001 — deliberate: any unexpected fault is a 2.
        traceback.print_exc()
        print(
            "reflow2_check: the check could not run — this is exit 2, not a gate failure",
            file=sys.stderr,
        )
        return 2


if __name__ == "__main__":
    sys.exit(run_guarded())
