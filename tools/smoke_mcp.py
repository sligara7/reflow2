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


def _unwrap(value):
    """Undo the list envelope the server adds.

    MCP requires `structuredContent` to be an object, so a tool returning a list
    sends `{"count": n, "items": [...]}`. Callers want the list.
    """
    if isinstance(value, dict) and set(value) == {"count", "items"}:
        return value["items"]
    return value


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
            return _unwrap(result["structuredContent"])
        return json.loads(result["content"][0]["text"])

    def call_expect_error(self, tool: str, args=None):
        """Call a tool that should be refused; return the error text, or None if
        it unexpectedly succeeded. A refusal that never arrives is the bug."""
        resp = self.rpc("tools/call", {"name": tool, "arguments": args or {}})
        if "error" in resp:
            return str(resp["error"])
        result = resp["result"]
        if result.get("isError"):
            return str(result.get("content"))
        return None

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
        "contain_component", "set_requirement_status", "open_questions", "answer_question",
        "export_graph", "import_graph",
        "add_interface", "provides", "consumes",
        "link_artifact", "reconcile_artifacts", "set_artifact_checksum",
        "add_verification", "verifies", "add_release", "add_environment",
        "deploy_to", "add_decision", "governed_by",
        "detect_defects", "propose_heal", "describe_schema",
        "set_capability_status", "set_provenance",
    ):
        c.ok(f"tool exposed: {expected}", expected in names)

    by_name = {t["name"]: t for t in tools}
    schema = by_name.get("reconcile_artifacts", {}).get("inputSchema", {})
    c.ok(
        "reconcile_artifacts input schema is usable",
        "observed" in schema.get("properties", {}),
        list(schema.get("properties", {})),
    )

    # BL-28. Every advertised parameter must declare a type.
    #
    # This asserts the *schema*, not behaviour through a client, and that
    # distinction is the whole point. Five parameters were declared
    # `serde_json::Value`, whose generated schema says nothing about the type;
    # a client with nothing to marshal against is free to guess, and the
    # clients disagreed — grok build sent a JSON object, Claude Code sent the
    # object as a string, and the string was rejected. Every call below in this
    # file passed a Python dict and stayed green throughout, because this file
    # is also a client we wrote. Only the published contract catches it.
    def untyped(sub) -> bool:
        if not isinstance(sub, dict):
            return False
        return "type" not in sub and not any(
            k in sub for k in ("$ref", "anyOf", "oneOf", "allOf", "enum", "const")
        )

    untyped_params = []
    for t in tools:
        for pname, pschema in (t["inputSchema"].get("properties") or {}).items():
            if untyped(pschema):
                untyped_params.append(f"{t['name']}.{pname}")
            # An array of untyped items has the same defect one level down.
            if isinstance(pschema, dict) and untyped(pschema.get("items")):
                untyped_params.append(f"{t['name']}.{pname}[]")
    c.ok(
        "every advertised tool parameter declares a type (BL-28)",
        not untyped_params,
        untyped_params,
    )

    # The vocabulary must be discoverable before anything is written, because a
    # blind trial that could not see it brute-forced fourteen edge types and then
    # used the one that happened to validate. Checked here rather than only in
    # cargo tests: every other layer is a client we wrote.
    print("\n== schema discovery (BL-1) ==")
    vocab = s.call("describe_schema", {})
    c.ok("every node type is discoverable", len(vocab.get("node_types", [])) == 27,
         len(vocab.get("node_types", [])))
    c.ok("every edge type is discoverable", len(vocab.get("edge_types", [])) == 53,
         len(vocab.get("edge_types", [])))

    exact = s.call("describe_schema", {"from": "Capability", "to": "Component"})
    c.ok("a modelled pair reports an exact match", exact.get("exact_matches", 0) >= 1, exact.get("note"))

    # The trial's own question. Nothing models Release -> Component; the answer
    # must say so rather than handing back the wildcard edge that validates.
    loose = s.call("describe_schema", {"from": "Release", "to": "Component"})
    c.ok("an unmodelled pair reports no exact match", loose.get("exact_matches") == 0, loose.get("note"))
    c.ok("and says so in words", "wildcard" in loose.get("note", "") or "No edge type" in loose.get("note", ""),
         loose.get("note"))

    node = s.call("describe_schema", {"node_type": "Component"})
    c.ok("a node type lists the edges it can carry",
         any(m["edge_type"] == "PROVIDES" for m in node.get("outgoing", [])))

    # A rejection must say what would have worked — the trial's sharper complaint.
    try:
        s.call("create_edge", {
            "edge_type": "PACKAGES", "from_type": "Release", "from_id": "rel:x",
            "to_type": "Component", "to_id": "cmp:x",
        })
        c.ok("a bogus edge type is rejected", False, "it was accepted")
    except RuntimeError as e:
        c.ok("a bogus edge type is rejected", True)
        c.ok("and the rejection points at describe_schema", "describe_schema" in str(e), str(e)[:200])

    # BL-2/BL-3: the write side of the assembly hierarchy and of a requirement's
    # standing. hierarchy_issues shipped as a reader with no writer, so it
    # returned [] for want of input; status was in the schema but unwritable.
    print("\n== decomposition and requirement status (BL-2, BL-3) ==")
    for cid, lvl in (("cmp:station", "system"), ("cmp:suite", "subsystem"),
                     ("cmp:probe", "component")):
        s.call("add_component", {"id": cid, "name": cid, "description": "part",
                                 "level": lvl})
    s.call("contain_component", {"from_id": "cmp:station", "to_id": "cmp:suite"})
    s.call("contain_component", {"from_id": "cmp:suite", "to_id": "cmp:probe"})
    c.ok("a clean spine reports no hierarchy issues",
         len(s.call("hierarchy_issues")) == 0, s.call("hierarchy_issues"))

    # Skipping a level must be caught — proof the detector is fed, not just quiet.
    s.call("add_component", {"id": "cmp:bolt", "name": "Bolt", "description": "p",
                             "level": "component"})
    s.call("contain_component", {"from_id": "cmp:station", "to_id": "cmp:bolt"})
    kinds = [i["kind"] for i in s.call("hierarchy_issues")]
    c.ok("skipping a level is reported", "missing_intermediate_level" in kinds, kinds)

    s.call("add_requirement", {"id": "req:maybe", "name": "Maybe",
                               "statement": "We might not do this."})
    upd = s.call("set_requirement_status", {"requirement_id": "req:maybe",
                                            "status": "dropped"})
    c.ok("a requirement's status is writable",
         upd["properties"]["status"] == "dropped", upd["properties"].get("status"))
    c.ok("and its statement survives the change",
         upd["properties"]["statement"] == "We might not do this.")
    nagged = any("req:maybe" in d.get("affected_ids", [])
                 for d in s.call("detect_defects"))
    c.ok("a dropped requirement stops being nagged by HEAL too", not nagged)

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

    # BL-27. Adopting a system that already exists needs to say two things a
    # greenfield design never does: this already ships, and I inferred it rather
    # than being told it. Both were unsayable, so ophyd's 15 shipped capabilities
    # landed as `planned` — a graph asserting a production system is entirely
    # unbuilt — and provenance got smuggled into statement text as `[EXTERNAL]`.
    # Driven here through the real MCP path because that is where the last
    # untyped-parameter bug hid from three test layers we wrote ourselves.
    print("\n== 1b. brownfield: status and provenance at the surface (BL-27) ==")
    s.call("add_capability", {"id": "cap:shipped", "name": "Device locking",
                              "description": "Serialises device access.",
                              "status": "realized"})
    shipped = s.call("get_node", {"node_type": "Capability", "id": "cap:shipped"})
    c.ok("a capability that already ships is not recorded as planned",
         shipped["properties"].get("status") == "realized", shipped["properties"])
    planned = s.call("get_node", {"node_type": "Capability", "id": "cap:flight"})
    c.ok("an unstated status still defaults to planned",
         planned["properties"].get("status") == "planned", planned["properties"])

    s.call("set_capability_status", {"capability_id": "cap:display", "status": "in_progress"})
    moved = s.call("get_node", {"node_type": "Capability", "id": "cap:display"})
    c.ok("set_capability_status moves it and keeps the description",
         moved["properties"].get("status") == "in_progress"
         and moved["properties"].get("description") == "Show the score.",
         moved["properties"])

    s.call("set_provenance", {"node_type": "Requirement", "node_id": "req:physics",
                              "provenance": "inferred"})
    inferred = s.call("get_node", {"node_type": "Requirement", "id": "req:physics"})
    c.ok("an inferred requirement says so in a queryable property",
         inferred["properties"].get("provenance") == "inferred"
         and "EXTERNAL" not in inferred["properties"].get("statement", ""),
         inferred["properties"])
    c.ok("provenance defaults to authored",
         s.call("get_node", {"node_type": "Capability", "id": "cap:flight"})
          ["properties"].get("provenance") == "authored")

    print("\n== 2. DETECT gaps, and the ask-the-user handshake ==")
    gaps = s.call("detect_gaps")
    sources = [g["gap_source"] for g in gaps]
    c.ok("gaps detected", len(gaps) > 0, sources)
    c.ok("a fully paired contract is not reported as a gap",
         "unprovided_interface" not in sources, sources)

    # BL-27: the direction DETECT was blind in. cap:shipped above satisfies no
    # requirement — the probe both brownfield trials ran (3dtictactoe's
    # cap:draw-detection, ophyd's cap:qserver-auth), where the orphan went
    # unmentioned while the requirement gaps were all reported. It is marked
    # inferred, so it reads as a feature in production nothing asked for and
    # leads the list.
    c.ok("a capability nothing asked for is reported (BL-27)",
         "unmotivated_capability" in sources, sources)
    orphan = next(g for g in gaps if g["gap_source"] == "unmotivated_capability")
    c.ok("and it names the capability, not the project",
         orphan["affected_ids"] == ["cap:shipped"], orphan["affected_ids"])
    c.ok("an inferred orphan outranks an unsatisfied requirement",
         orphan["severity"] > max((g["severity"] for g in gaps
                                   if g["gap_source"] == "unsatisfied_requirement"),
                                  default=0.0),
         [(g["gap_source"], round(g["severity"], 2)) for g in gaps])

    # BL-27: real duplicate detection. cmp:physics and cmp:ui below are given an
    # identical capability set, which is 3dtictactoe's shape — two components
    # each holding the same three capabilities, one of them dead code, where
    # detect_defects returned 8 defects and none was `duplicate`. HEAL's rule
    # reads a DUPLICATES edge somebody already drew, so it computes nothing.
    # This is asked rather than repaired: apply_heal's merge deletes a node, and
    # a heuristic must not drive that.
    s.call("add_capability", {"id": "cap:dup-a", "name": "Grid state",
                              "description": "Holds the grid."})
    s.call("add_capability", {"id": "cap:dup-b", "name": "Victory check",
                              "description": "Spots a win."})
    s.call("add_component", {"id": "cmp:board", "name": "Board",
                             "description": "First attempt, never instantiated."})
    s.call("add_component", {"id": "cmp:engine", "name": "GameState",
                             "description": "The one that shipped."})
    for cap in ("cap:dup-a", "cap:dup-b"):
        s.call("satisfies", {"from_id": cap, "to_id": "req:physics"})
        for cmp_id in ("cmp:board", "cmp:engine"):
            s.call("allocate", {"from_id": cap, "to_id": cmp_id})

    dup_gaps = [g for g in s.call("detect_gaps")
                if g["gap_source"] == "possible_duplicate"]
    c.ok("two components with the same capabilities are reported (BL-27)",
         len(dup_gaps) == 1, [g["title"] for g in dup_gaps])
    c.ok("and it names both, so the user can answer it",
         dup_gaps and dup_gaps[0]["affected_ids"] == ["cmp:board", "cmp:engine"],
         dup_gaps[0]["affected_ids"] if dup_gaps else None)
    c.ok("and shows the overlap it measured, not just a verdict",
         dup_gaps and "2 of 2" in dup_gaps[0]["evidence"],
         dup_gaps[0]["evidence"] if dup_gaps else None)
    # It stays a question: HEAL must not have turned it into an applicable merge.
    merge_ops = [o for o in s.call("propose_heal", {"strategy": "balanced"})["operations"]
                 if o["op"].get("type") == "merge" or "Merge" in str(o["op"])]
    c.ok("a suspected duplicate never becomes an applicable merge",
         not any("cmp:board" in str(o) or "cmp:engine" in str(o) for o in merge_ops),
         merge_ops)

    gaps = s.call("detect_gaps")
    sources = [g["gap_source"] for g in gaps]

    # BL-27: a gap that names nodes describes something wrong NOW; a phase
    # nudge describes what comes next. Never rank "next" above "broken" — an
    # agent works this list top-down, and three brownfield trials watched it do
    # the useless thing first. Asserted on the ordered JSON an agent actually
    # receives, not just in the Rust sort.
    anchored = [i for i, g in enumerate(gaps) if g["affected_ids"]]
    unanchored = [i for i, g in enumerate(gaps) if not g["affected_ids"]]
    c.ok("every anchored gap outranks every phase nudge (BL-27)",
         not anchored or not unanchored or max(anchored) < min(unanchored),
         [(g["gap_source"], round(g["severity"], 2), len(g["affected_ids"])) for g in gaps])

    # BL-6b: a cross-community coupling is a signal, not a question. It fires on
    # correct architecture — an Interface bridges two clusters by construction —
    # so it informs via graph_report instead of demanding an answer.
    c.ok("coupling is not reported as a gap",
         "unexpected_coupling" not in sources, sources)
    # BL-23: per-file verification coverage is counted, not asked. One VERIFIES
    # edge per source file was 22 of 25 gaps on reflow2's own design.
    c.ok("per-file coverage is not reported as a gap",
         "unverified_artifact" not in sources, sources)
    cov = s.call("graph_report")["verification"]
    c.ok("but the coverage number is in the report", cov["capabilities"] >= 1, cov)
    c.ok("but the coupling signal still reaches the report",
         "surprising" in s.call("graph_report"))

    # The acknowledge → reviewed round trip, including the JSON an agent reads.
    # Nothing covered this before, and BL-6b changed the shape of a ReviewedGap.
    ack_gap = gaps[0]
    s.call("acknowledge_gap", {"gap_id": ack_gap["id"],
                               "affected_ids": ack_gap["affected_ids"],
                               "reason": "deliberate for v1"})
    open_ids = {g["id"] for g in s.call("detect_gaps")}
    c.ok("an acknowledged gap leaves the open list", ack_gap["id"] not in open_ids)
    reviewed = s.call("reviewed_gaps")
    match = [r for r in reviewed if r["gap_id"] == ack_gap["id"]]
    c.ok("and appears in reviewed_gaps with its reason",
         len(match) == 1 and match[0]["reason"] == "deliberate for v1", reviewed)

    # An acknowledgement whose detector no longer exists must still be listed,
    # marked retired — a reviewed list that shrinks unexplained is the dishonesty
    # the split exists to avoid.
    s.call("acknowledge_gap", {"gap_id": "gap:deadbeefdeadbeef",
                               "affected_ids": [], "reason": "coupling is the product"})
    retired = [r for r in s.call("reviewed_gaps") if r.get("retired")]
    c.ok("an acknowledgement outliving its detector is still reported",
         len(retired) == 1 and "gap" not in retired[0], retired)

    # BL-28: the fix is a typed schema, NOT a server that accepts both shapes.
    # A stringified object must still be rejected — accepting it would be the
    # silent fallback AGENTS.md rule 4 forbids, and would hide the next client
    # that marshals wrongly.
    stringly = s.rpc("tools/call", {
        "name": "gap_to_prompt",
        "arguments": {"gap": json.dumps(ack_gap), "answers": []},
    })
    c.ok(
        "a stringified object is still rejected, not silently accepted (BL-28)",
        "error" in stringly or stringly.get("result", {}).get("isError"),
        stringly,
    )

    # Put a question on the record so the restart below can prove it survives.
    asked_wording = "Is this coupling deliberate?"
    prep = s.call("gap_to_prompt", {"gap": ack_gap, "answers": []})
    s.call("gap_to_prompt", {
        "gap": ack_gap,
        "answers": [{"id": p["id"], "text": asked_wording} for p in prep["prompts"]],
        "asked_at": "2026-07-18T10:00:00Z",
    })
    outstanding = s.call("open_questions")
    c.ok("asking a gap records the question", len(outstanding) == 1, outstanding)

    s.call("withdraw_gap_acknowledgement", {"gap_id": ack_gap["id"]})
    s.call("withdraw_gap_acknowledgement", {"gap_id": "gap:deadbeefdeadbeef"})
    c.ok("withdrawing puts the gap back",
         ack_gap["id"] in {g["id"] for g in s.call("detect_gaps")})
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

    cov = s.call("graph_report")["verification"]
    c.ok("coverage counts the registered file, without asking about it",
         cov["artifacts"] >= 1 and cov["artifacts_verified"] == 0, cov)

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

    # BL-30 (S half): a check that FAILS must be a gap, not a satisfaction. The
    # erosion trial found the gap "how will you confirm this works?" being
    # closed by a test proving it does not, with the failure invisible in
    # detect_gaps, detect_defects and graph_report alike.
    s.call("set_verification_status", {"verification_id": "ver:flight",
                                       "status": "failing"})
    red_gaps = s.call("detect_gaps")
    red = next((g for g in red_gaps if g["gap_source"] == "failing_verification"), None)
    c.ok("a failing check is surfaced as a gap (BL-30)", red is not None,
         sorted({g["gap_source"] for g in red_gaps}))
    c.ok("and it names both the check and the thing it checks",
         red is not None and red["affected_ids"] == ["cap:flight", "ver:flight"],
         red["affected_ids"] if red else None)
    c.ok("and it outranks every absence-shaped gap",
         red is not None and red_gaps[0]["gap_source"] == "failing_verification",
         [(g["gap_source"], round(g["severity"], 2)) for g in red_gaps[:3]])
    c.ok("and coverage does not count the failing check as verification",
         s.call("graph_report")["verification"]["capabilities_verified"] == 0)
    s.call("set_verification_status", {"verification_id": "ver:flight",
                                       "status": "passing"})
    c.ok("green again: the gap clears and coverage counts it",
         not any(g["gap_source"] == "failing_verification" for g in s.call("detect_gaps"))
         and s.call("graph_report")["verification"]["capabilities_verified"] == 1)

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

    # BL-29: apply_heal used to execute whatever it was handed. A proposal with
    # a made-up issue id, naming two nodes no detector had called duplicates,
    # was applied and deleted one of them. Driven over the real MCP path because
    # that is exactly how a client reaches it — ApplyHealReq takes caller JSON.
    print("\n== 9b. apply_heal refuses a proposal HEAL never made (BL-29) ==")
    forged = {
        "target_id": "proj:smoke", "summary": "forged", "strategy": "balanced",
        "issues_addressed": [], "operations": [{
            "issue_id": "heal:0000000000000000",
            "op": {"Merge": {"keep_type": "Component", "keep_id": "cmp:board",
                             "remove_type": "Component", "remove_id": "cmp:engine"}}}],
        "generated_content": [], "skipped_operations": [],
        "requires_human_review": True, "confidence": 0.0,
    }
    refused = s.call_expect_error("apply_heal", {"proposal": forged})
    c.ok("a forged merge is refused", refused is not None and
         "not one HEAL proposes" in str(refused), refused)
    c.ok("and the node it named is still there",
         s.call("get_node", {"node_type": "Component", "id": "cmp:engine"}) is not None)

    print("\n== 10. the design survives a restart ==")
    first_gaps = sorted(g["id"] for g in s.call("detect_gaps"))
    # Snapshot what is on the record right before the process ends, so the
    # check after reopening is against reality rather than an assumption.
    before_restart = s.call("open_questions")

    s.close()
    s = Server(binary, graph_path)
    n = s.call("get_node", {"node_type": "Interface", "id": "ifc:state"})
    c.ok("graph reopened with its contents intact",
         n is not None and n.get("node_id") == "ifc:state", n)

    # BL-4: a question already put to the user must survive the restart, with
    # the wording they saw. This is the whole point — before it, the next
    # session re-derived the same gap and asked again, which the blind trial
    # called "the stateless-agent problem reflow2 is supposed to solve".
    reopened = s.call("open_questions")
    c.ok("a question asked last session is still open in this one",
         len(reopened) == 1, reopened)
    c.ok("and the exact wording the user saw survived",
         reopened == before_restart, {"before": before_restart, "after": reopened})
    answered_gap = reopened[0]["gap_id"]
    s.call("answer_question", {"gap_id": answered_gap, "answer": "Yes — deliberate."})

    # BL-25: answering does not by itself settle anything. While the gap is still
    # open the question stays visible, marked answered and carrying the reply —
    # otherwise a later session sees a bare open gap and asks all over again.
    still = s.call("open_questions")
    c.ok("an answered question stays visible while its gap is open",
         len(still) == 1 and still[0]["status"] == "answered", still)
    c.ok("and brings back what the user said",
         still[0].get("answer") == "Yes — deliberate.", still[0].get("answer"))

    # BL-20: the whole design as a portable document. Exercised across a real
    # process here, not just in-process: export, restore into a second graph in
    # a separate server, and check it diagnoses the same. That is the migration
    # path — export with the old build, import with the new.
    doc = s.call("export_graph")
    c.ok("the design exports whole",
         len(doc["nodes"]) > 0 and doc["stamp"]["node_types"] > 0, doc.get("stamp"))
    c.ok("and the export is byte-identical on a second run",
         json.dumps(s.call("export_graph"), sort_keys=False) == json.dumps(doc, sort_keys=False))

    restore_path = graph_path + "-restored"
    r = Server(binary, restore_path)
    rep = r.call("import_graph", {"document": doc})
    c.ok("it imports whole into a fresh graph in another process",
         rep["nodes_written"] == len(doc["nodes"]) and not rep["skipped_edges"], rep)
    c.ok("and the restored design diagnoses the same",
         len(r.call("detect_gaps")) == len(s.call("detect_gaps")))
    c.ok("and re-exports to the same document",
         r.call("export_graph")["nodes"] == doc["nodes"])
    r.close()

    # BL-39: --import, the sibling of --export. Without it a design could be
    # read out without speaking MCP but never written back, so a committed
    # export, a backup, or a design built elsewhere could only be restored by
    # passing the whole document through the tool boundary — and the consumer
    # skills, which run against the live graph, could only ever see a design the
    # session itself built.
    cli_path = graph_path + "-cli"
    doc_file = graph_path + "-doc.json"
    with open(doc_file, "w") as fh:
        json.dump(doc, fh)
    imp = subprocess.run([binary, "--graph-path", cli_path, "--import", doc_file],
                         capture_output=True, text=True)
    c.ok("a design imports from the command line, without speaking MCP",
         imp.returncode == 0 and "imported" in imp.stderr, imp.stderr.strip()[-200:])
    exp = subprocess.run([binary, "--graph-path", cli_path, "--export"],
                         capture_output=True, text=True)
    c.ok("and the CLI round trip is byte-identical",
         exp.returncode == 0 and json.loads(exp.stdout) == doc)

    # stdin, so `--export | ssh … --import -` works.
    pipe = subprocess.run([binary, "--graph-path", graph_path + "-pipe", "--import", "-"],
                          input=json.dumps(doc), capture_output=True, text=True)
    c.ok("and it reads the document from stdin", pipe.returncode == 0, pipe.stderr.strip()[-160:])

    # The failure an operator actually hits: a server already holds the graph.
    # RocksDB is single-writer, and the raw error names neither the cause nor
    # the fix.
    held = subprocess.run([binary, "--graph-path", graph_path, "--import", doc_file],
                          capture_output=True, text=True)
    c.ok("a graph already open elsewhere is refused with what to do about it",
         held.returncode != 0 and "single-writer" in held.stderr and "Stop that server" in held.stderr,
         held.stderr.strip()[-200:])
    c.ok("a document that is not an export is refused by name",
         subprocess.run([binary, "--graph-path", graph_path + "-x", "--import", __file__],
                        capture_output=True, text=True).returncode != 0)

    # BL-19: the graph carries a record of which reflow2 wrote it, beside the
    # store rather than inside it (RocksDB owns its own directory).
    import os
    stamp = graph_path + ".meta.json"
    c.ok("the graph is stamped with the reflow2 that wrote it", os.path.exists(stamp), stamp)
    if os.path.exists(stamp):
        meta = json.load(open(stamp))
        c.ok("and the stamp records the vocabulary, not just a version",
             meta.get("node_types", 0) > 0 and meta.get("edge_types", 0) > 0, meta)

    # Cross-process determinism. A HashSet's iteration order is seeded per
    # process, so anything derived from it (community detection, and every gap
    # that follows) can differ between runs on an unchanged graph. That would
    # make reviewing a gap pointless — the id it was accepted under might not
    # come back — so it has to be checked here, where the processes are real.
    second_gaps = sorted(g["id"] for g in s.call("detect_gaps"))
    c.ok("the same graph gives the same gaps in a fresh process",
         first_gaps == second_gaps,
         f"{len(first_gaps)} vs {len(second_gaps)}")

    # Acknowledging is what settles an answered question — done after the
    # determinism check above, which has to compare an unmutated graph.
    ack_target = [g for g in s.call("detect_gaps") if g["id"] == answered_gap][0]
    s.call("acknowledge_gap", {"gap_id": answered_gap,
                               "affected_ids": ack_target["affected_ids"],
                               "reason": "deliberate for v1"})
    c.ok("acknowledging the gap leaves nothing outstanding",
         s.call("open_questions") == [])
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
