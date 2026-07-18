#!/usr/bin/env python3
"""End-to-end smoke test for the reflow2-mcp server.

Drives the **built binary** over stdio JSON-RPC through the whole coherence
loop, the way a coding agent actually would: bootstrap, capture intent including
a contract, detect gaps, register a built file, edit it, catch the drift, follow
it back up to the requirement, find a dependency cycle, and reopen the graph to
prove it persisted.

This is deliberately not a Rust test. `cargo test` exercises the library; this
exercises the shipped binary, the stdio transport, the generated tool schemas,
and the JSON an agent receives back — the layer where wiring mistakes live.

Usage (from the repo root, after `cargo build -p reflow2-mcp`):

    python3 tools/smoke_mcp.py
    python3 tools/smoke_mcp.py --bin target/release/reflow2-mcp
    python3 tools/smoke_mcp.py --keep-graph   # leave the graph dir for poking at

Exits 0 when every check passes, 1 otherwise. Standard library only.
"""
from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile


class Server:
    """A running reflow2-mcp process, spoken to over stdio JSON-RPC."""

    def __init__(self, binary: str, graph_path: str) -> None:
        self.binary = binary
        self.graph_path = graph_path
        self._id = 0
        self.proc = subprocess.Popen(
            [binary, "--graph-path", graph_path],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
            env={**os.environ, "RUST_LOG": "warn"},
        )
        self.handshake()

    def rpc(self, method: str, params=None, notify: bool = False):
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
            err = self.proc.stderr.read()
            raise SystemExit(f"server closed stdout unexpectedly.\nstderr:\n{err}")
        return json.loads(line)

    def handshake(self) -> dict:
        init = self.rpc(
            "initialize",
            {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "smoke_mcp", "version": "0"},
            },
        )
        self.rpc("notifications/initialized", {}, notify=True)
        return init

    def call(self, tool: str, args=None):
        """Call a tool and return its payload, raising on any error."""
        resp = self.rpc("tools/call", {"name": tool, "arguments": args or {}})
        if "error" in resp:
            raise RuntimeError(f"{tool}: JSON-RPC error: {resp['error']}")
        result = resp["result"]
        if result.get("isError"):
            raise RuntimeError(f"{tool}: tool error: {result.get('content')}")
        if "structuredContent" in result:
            return result["structuredContent"]
        return json.loads(result["content"][0]["text"])

    def close(self) -> None:
        self.proc.stdin.close()
        try:
            self.proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self.proc.kill()


class Checks:
    def __init__(self) -> None:
        self.failures: list[str] = []

    def ok(self, label: str, cond: bool, detail="") -> bool:
        if cond:
            print(f"  PASS  {label}")
        else:
            print(f"  FAIL  {label}   {detail}")
            self.failures.append(label)
        return bool(cond)

    def note(self, msg: str) -> None:
        print(f"  note  {msg}")


def run(binary: str, graph_path: str) -> int:
    c = Checks()
    s = Server(binary, graph_path)

    print("== tool surface (what an agent sees) ==")
    tools = s.rpc("tools/list", {})["result"]["tools"]
    names = {t["name"] for t in tools}
    print(f"  {len(names)} tools exposed")
    for expected in (
        "genesis", "detect_gaps", "gap_to_prompt", "propagate_from",
        "add_interface", "provides", "consumes",
        "link_artifact", "reconcile_artifacts", "set_artifact_checksum",
        "add_verification", "verifies", "add_release", "add_environment",
        "deploy_to", "add_decision", "governed_by",
        "detect_defects", "propose_heal",
    ):
        c.ok(f"tool exposed: {expected}", expected in names)

    by_name = {t["name"]: t for t in tools}
    schema = by_name.get("reconcile_artifacts", {}).get("inputSchema", {})
    c.ok(
        "reconcile_artifacts input schema is usable",
        "observed" in schema.get("properties", {}),
        list(schema.get("properties", {})),
    )

    print("\n== 0. GENESIS ==")
    g = s.call("genesis", {
        "project_id": "proj:smoke", "name": "Smoke",
        "objective": "Prove the loop runs", "domain": "software",
    })
    c.ok("bootstraps a project", g.get("project_id") == "proj:smoke", g)

    print("\n== 1. capture intent, including a contract ==")
    s.call("add_requirement", {"id": "req:physics", "name": "Realistic physics",
                               "statement": "Ball flight must look plausible."})
    s.call("add_capability", {"id": "cap:flight", "name": "Ball flight",
                              "description": "Simulate ball trajectory."})
    s.call("add_capability", {"id": "cap:display", "name": "Scoreboard display",
                              "description": "Show the score."})
    s.call("add_component", {"id": "cmp:physics", "name": "Physics engine",
                             "description": "Runs the sim."})
    s.call("add_component", {"id": "cmp:ui", "name": "Scoreboard UI",
                             "description": "Draws the board."})
    s.call("satisfies", {"from_id": "cap:flight", "to_id": "req:physics"})
    s.call("allocate", {"from_id": "cap:flight", "to_id": "cmp:physics"})
    s.call("allocate", {"from_id": "cap:display", "to_id": "cmp:ui"})
    s.call("add_interface", {"id": "ifc:state", "name": "Game state feed"})
    s.call("provides", {"from_id": "cmp:physics", "to_id": "ifc:state"})
    s.call("consumes", {"from_id": "cmp:ui", "to_id": "ifc:state"})
    c.ok("contract recorded with both sides",
         s.call("get_node", {"node_type": "Interface", "id": "ifc:state"}) is not None)

    print("\n== 2. DETECT gaps, and the ask-the-user handshake ==")
    gaps = s.call("detect_gaps")
    sources = [g["gap_source"] for g in gaps]
    c.ok("gaps detected", len(gaps) > 0, sources)
    c.ok("a fully paired contract is not reported as a gap",
         "unprovided_interface" not in sources, sources)
    h1 = s.call("gap_to_prompt", {"gap": gaps[0], "answers": []})
    if c.ok("handshake asks the agent for phrasing", h1.get("status") == "needs_llm", h1):
        h2 = s.call("gap_to_prompt", {
            "gap": gaps[0],
            "answers": [{"id": p["id"], "text": "Which part should own this?"}
                        for p in h1["prompts"]],
        })
        c.ok("handshake returns a user-facing question",
             h2.get("status") == "ok" and "question" in h2.get("prompt", {}), h2)

    print("\n== 3. register a built file, with a drift baseline ==")
    s.call("link_artifact", {
        "artifact_id": "art:flight", "name": "BallFlight.cs",
        "location": "src/BallFlight.cs", "artifact_type": "code",
        "target_type": "Capability", "target_id": "cap:flight",
        "checksum": "sha256:v1",
    })
    flagged = {
        i for g in s.call("detect_gaps")
        if g["gap_source"] == "unrealized_capability" for i in g["affected_ids"]
    }
    c.ok("the linked capability is no longer unrealized", "cap:flight" not in flagged, flagged)
    c.ok("the unbuilt one now is (build phase has begun)", "cap:display" in flagged, flagged)
    c.note("the first link_artifact switches this detector ON — total gap count rising is correct")

    print("\n== 3b. answer the gaps DETECT raises (the write side) ==")
    before = {g["gap_source"] for g in s.call("detect_gaps")}
    c.ok("verification gap is raised", "build_without_verification" in before, before)
    c.ok("deploy/operate gap is raised", "no_deploy_operate" in before, before)

    s.call("add_verification", {"id": "ver:flight", "name": "Ball flight tests",
                                "method": "test", "level": "unit"})
    s.call("verifies", {"verification_id": "ver:flight",
                        "target_type": "Capability", "target_id": "cap:flight"})
    s.call("add_release", {"id": "rel:v1", "name": "Smoke v1", "version": "1.0.0"})
    s.call("add_environment", {"id": "env:prod", "name": "Production",
                               "env_type": "production"})
    s.call("deploy_to", {"release_id": "rel:v1", "environment_id": "env:prod",
                         "status": "active"})
    s.call("add_decision", {"id": "dec:engine", "name": "Custom physics",
                            "decision": "Write our own physics rather than use a library.",
                            "rationale": "Softball arcs need tuning a general engine won't give."})
    s.call("governed_by", {"from_type": "Component", "from_id": "cmp:physics",
                           "to_type": "Decision", "to_id": "dec:engine"})

    after = {g["gap_source"] for g in s.call("detect_gaps")}
    c.ok("verification gap closed", "build_without_verification" not in after, after)
    c.ok("deploy/operate gap closed", "no_deploy_operate" not in after, after)

    s.call("set_verification_status", {"verification_id": "ver:flight",
                                       "status": "failing"})
    radius = s.call("propagate_from", {"seed_ids": ["ver:flight"]})
    c.ok("a failing check reaches the requirement it protects",
         any(n["node_id"] == "req:physics" for n in radius["impacted"]),
         [n["node_id"] for n in radius["impacted"]])

    print("\n== 4. reconcile: nothing changed ==")
    r = s.call("reconcile_artifacts", {"observed": [
        {"artifact_id": "art:flight", "present": True, "checksum": "sha256:v1"}]})
    c.ok("a matching hash is not drift", r["findings"] == [] and r["unchanged"] == 1, r)

    print("\n== 5. reconcile: the file was edited ==")
    r = s.call("reconcile_artifacts", {
        "observed": [{"artifact_id": "art:flight", "present": True,
                      "checksum": "sha256:v2"}],
        "record_events": True, "detected_at": "1970-01-01T00:00:00Z"})
    c.ok("checksum_change detected",
         [f["kind"] for f in r["findings"]] == ["checksum_change"], r["findings"])
    c.ok("seeds name the design the file realizes",
         r["propagation_seeds"] == ["cap:flight"], r["propagation_seeds"])
    c.ok("a DriftEvent was recorded", len(r["recorded_events"]) == 1, r["recorded_events"])

    print("\n== 6. the change reaches the intent behind it ==")
    radius = s.call("propagate_from", {"seed_ids": r["propagation_seeds"]})
    reached = {n["node_id"]: n["direction"] for n in radius["impacted"]}
    c.ok("reaches the requirement", "req:physics" in reached, list(reached))
    c.ok("and reaches it upstream", reached.get("req:physics") == "upstream", reached)
    c.ok("partial-result fields are present (no silent drops)",
         "unknown_seeds" in radius and "truncated_beyond_depth" in radius, list(radius))

    print("\n== 7. accepting the change clears the drift ==")
    s.call("set_artifact_checksum", {"artifact_id": "art:flight", "checksum": "sha256:v2"})
    r = s.call("reconcile_artifacts", {"observed": [
        {"artifact_id": "art:flight", "present": True, "checksum": "sha256:v2"}]})
    c.ok("an accepted change is the new baseline", r["findings"] == [], r["findings"])

    print("\n== 8. structural health: a cycle through contracts ==")
    s.call("add_interface", {"id": "ifc:score", "name": "Score input"})
    s.call("provides", {"from_id": "cmp:ui", "to_id": "ifc:score"})
    s.call("consumes", {"from_id": "cmp:physics", "to_id": "ifc:score"})
    defects = s.call("detect_defects")
    cyc = next((d for d in defects if d["category"] == "circular_dependency"), None)
    c.ok("circular dependency found through the contracts", cyc is not None,
         [d["category"] for d in defects])
    if cyc:
        c.ok("the loop is shown as a readable path", "→" in cyc["message"], cyc["message"])
        c.note(cyc["message"])

    print("\n== 9. HEAL proposes, never auto-fixes a judgement call ==")
    p = s.call("propose_heal", {})
    c.ok("requires human review", p.get("requires_human_review") is True, p.get("summary"))
    c.ok("offers a cycle break for the human",
         "cycle break" in [x["kind"] for x in p.get("generated_content", [])],
         [x["kind"] for x in p.get("generated_content", [])])
    c.ok("skipped_operations reported", "skipped_operations" in p, list(p))

    print("\n== 10. the design survives a restart ==")
    s.close()
    s = Server(binary, graph_path)
    n = s.call("get_node", {"node_type": "Interface", "id": "ifc:state"})
    c.ok("graph reopened with its contents intact",
         n is not None and n.get("node_id") == "ifc:state", n)
    s.close()

    print("\n" + "=" * 62)
    if c.failures:
        print(f"FAILED ({len(c.failures)}):")
        for f in c.failures:
            print(f"  - {f}")
        return 1
    print("ALL CHECKS PASSED — the loop runs end to end against the built binary.")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--bin", default="target/debug/reflow2-mcp",
                    help="path to the reflow2-mcp binary (default: %(default)s)")
    ap.add_argument("--graph-path", default=None,
                    help="graph directory (default: a fresh temp dir, removed afterwards)")
    ap.add_argument("--keep-graph", action="store_true",
                    help="do not delete the graph directory on exit")
    args = ap.parse_args()

    binary = os.path.abspath(args.bin)
    if not os.path.exists(binary):
        print(f"binary not found: {binary}\nBuild it first:  cargo build -p reflow2-mcp")
        return 1

    graph_path = args.graph_path or tempfile.mkdtemp(prefix="reflow2-smoke-")
    if args.graph_path:
        shutil.rmtree(graph_path, ignore_errors=True)  # always start clean

    try:
        return run(binary, graph_path)
    finally:
        if args.keep_graph:
            print(f"\ngraph kept at: {graph_path}")
        else:
            shutil.rmtree(graph_path, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
