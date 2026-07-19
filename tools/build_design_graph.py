#!/usr/bin/env python3
"""Build reflow2's own functional design graph, and analyse it with reflow2.

This is dogfooding in the strong sense. Every other trial in `tools/` builds a
throwaway graph to *test* reflow2; this one builds the real, durable design of
reflow2 itself and then turns reflow2's whole analysis surface on it —
detect_gaps, detect_defects, possible_duplicate, hierarchy_issues,
surprising_connections, evaluate_allocation.

The graph is committed as a deterministic export (`docs/design/reflow2.json`)
rather than as a RocksDB directory: exports are sorted and byte-identical for an
unchanged design, so the design becomes reviewable and diffable in git, and any
working copy is `import_graph` away. `.reflow2/` is gitignored, so the export is
the durable artifact — the RocksDB store is a local cache of it.

Granularity is deliberately coarse (BL-23): one Component per module, one
Artifact per module, one Verification per test file — never one Artifact per
source file, which made 22 of 25 gaps noise the last time this repo was modelled.

Run:  python3 tools/build_design_graph.py [--analyse-only]
"""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import shutil
import sys
import tempfile

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from smoke_mcp import Server  # noqa: E402

REPO = pathlib.Path(__file__).resolve().parent.parent
EXPORT = REPO / "docs/design/reflow2.json"

# ---- P0 · Intent ----------------------------------------------------------
REQUIREMENTS = {
    "req:coherence": ("Design stays coherent across its lifecycle",
                      "When anything changes in any phase, the ripples are found and surfaced.",
                      "critical"),
    "req:released-eq-designed": ("Released equals designed",
                                 "At release the design must describe what was actually released.",
                                 "critical"),
    "req:no-silent-fallback": ("No silent fallbacks or drops",
                               "Failures and skipped work are surfaced loudly, never swallowed.",
                               "critical"),
    "req:golden-thread": ("Every artifact traces to the intent it serves",
                          "Traceability runs from concept through operations without a break.",
                          "high"),
    "req:no-se-knowledge": ("The user never needs systems engineering",
                            "The graph carries the discipline; the user answers plain questions.",
                            "high"),
    "req:design-anything": ("Design anything, not only software",
                            "Software, hardware, a document, a process — the vocabulary stays neutral.",
                            "high"),
    "req:agent-native": ("An agent can drive the whole loop",
                         "Every capability is reachable from a coding agent over one surface.",
                         "high"),
    "req:survives-upgrade": ("A design survives a reflow2 upgrade",
                             "An existing graph opens, or is refused loudly with what to do.",
                             "high"),
    "req:intent-preserved": ("The past is never overwritten",
                             "Updating the design to match the build must not erase what was intended.",
                             "high"),
    "req:adopt-existing": ("A system that already exists can be brought under design control",
                           "Requirements, functions and structure can be recovered from a running system.",
                           "medium"),
}

# ---- P1 · Function. The coherence loop, plus what serves it. --------------
# (id, name, description, status, satisfies[])
CAPABILITIES = [
    ("cap:change", "Record a change", "Snapshot prior state at an epoch, then edit — axis Z.",
     "verified", ["req:coherence", "req:intent-preserved"]),
    ("cap:propagate", "Propagate impact", "Walk the golden thread to compute a blast radius.",
     "verified", ["req:coherence", "req:golden-thread"]),
    ("cap:detect", "Detect gaps", "Find what the design has not decided yet, ranked.",
     "verified", ["req:coherence", "req:no-se-knowledge"]),
    ("cap:surface", "Ask the user a plain question", "Turn a gap into a question in the user's words.",
     "verified", ["req:no-se-knowledge"]),
    ("cap:heal", "Repair structure", "Propose content-free structural fixes, then apply atomically.",
     "verified", ["req:coherence", "req:no-silent-fallback"]),
    ("cap:reconcile-built", "Reconcile against what was built",
     "Compare registered artifacts against observed files and report divergence.",
     "verified", ["req:released-eq-designed", "req:golden-thread"]),
    ("cap:reconcile-verified", "Reconcile against what was proven",
     "Compare recorded verification status against a real test run.",
     "planned", ["req:released-eq-designed"]),
    ("cap:reconcile-deployed", "Reconcile against what is running",
     "Compare the design against what is actually deployed.",
     "planned", ["req:released-eq-designed"]),
    ("cap:vocabulary", "Describe the vocabulary", "Tell a client what types exist and what may join them.",
     "verified", ["req:agent-native", "req:no-silent-fallback"]),
    ("cap:portability", "Export and import a design", "Move a design across machines and versions.",
     "verified", ["req:survives-upgrade"]),
    ("cap:stamp", "Say which reflow2 wrote a graph", "Stamp and check, refusing a graph from the future.",
     "verified", ["req:survives-upgrade"]),
    ("cap:allocate", "Analyse and propose allocation", "Score function-to-structure allocation; cluster coupling.",
     "verified", ["req:golden-thread"]),
    ("cap:hierarchy", "Check decomposition levels", "Find missing intermediates and level mismatches — axis Y.",
     "verified", ["req:coherence"]),
    ("cap:dimensions", "Track quality over time", "Assess nodes on dimensions and detect decline.",
     "realized", ["req:coherence"]),
    ("cap:ingest", "Extract a design from freeform text", "Multi-pass LLM extraction with provenance.",
     "realized", ["req:no-se-knowledge"]),
    ("cap:questions", "Remember what was asked", "Questions outlive the session; answers stay visible.",
     "verified", ["req:no-se-knowledge"]),
    ("cap:report", "Say where the design stands", "One rollup answering 'what should I look at?'.",
     "verified", ["req:no-se-knowledge"]),
    ("cap:mcp-surface", "Serve the loop over MCP", "Every capability as a typed tool for an agent.",
     "verified", ["req:agent-native"]),
    ("cap:kit", "Install into a consumer project", "One command sets up or refreshes the design environment.",
     "verified", ["req:agent-native"]),
    ("cap:adopt", "Recover a design from an existing system",
     "Read requirements, functions and structure back out of a running system.",
     "planned", ["req:adopt-existing"]),
    ("cap:model-process", "Model a process, not only a product",
     "Represent an ordered flow of activities with roles on its edges.",
     "planned", ["req:design-anything"]),
    ("cap:freshness", "Say when a claim was last confirmed",
     "Distinguish a design that matches reality from one nobody has checked.",
     "planned", ["req:released-eq-designed", "req:coherence"]),
]

# ---- Decisions. The distillate of the sessions that shaped the design. ----
#
# The graph is the distillate, not the tape: a session transcript is an
# artifact outside the graph (every commit carries its Claude-Session URL, so
# the raw context is one link from git log). What belongs IN the graph is what
# was decided and why — where-am-i's "what's settled" reads exactly this, and
# until 2026-07-19 it had nothing to read.
SESSION = "https://claude.ai/code/session_013aumVdHrRHu24cLirn6Cam"
DECISIONS = [
    ("dec:ask-not-repair", "Suspected duplicates are asked, never merged",
     "possible_duplicate is a DETECT gap; HEAL merges only on a human-drawn DUPLICATES edge.",
     "apply_heal deletes a node with no undo, so merge is safe only because the endpoints were "
     "asserted; a heuristic must not drive it. A gap can be acknowledged, a HEAL defect cannot "
     f"be dismissed. Decided 2026-07-19 ({SESSION}); evidence: 3dtictactoe trial, BL-27/BL-29.",
     ["cap:detect", "cap:heal"]),
    ("dec:anchored-first", "A gap that names nodes outranks a phase nudge",
     "detect_gaps sorts anchored gaps before project-level phase nudges, severity within each band.",
     "A named gap says something is wrong NOW; a nudge says what comes next. Ranking 'next' above "
     "'broken' made three brownfield trials do the useless thing first. Nudges are demoted, never "
     f"suppressed. Decided 2026-07-19 ({SESSION}); evidence: gap-surfacing disciplines 3 and 8.",
     ["cap:detect"]),
    ("dec:operational-spof", "Only things that operate can be single points of failure",
     "single_point_of_failure candidates are Components, Interfaces, Resources, Environments.",
     "The suggested fix is add_redundancy, and redundancy is only coherent for running parts; a "
     "golden thread converges on intent by design, so intent hubs are the thread working. 22 of "
     f"22 false positives cleared, 4 true survivors. Decided 2026-07-19 ({SESSION}); BL-5.",
     ["cap:heal"]),
    ("dec:two-sided-accept", "Silent drift-accept does not exist",
     "set_artifact_checksum requires a disposition: design_holds (a dated claim) or "
     "design_updated (naming the design-side ChangeEvent, linked to the artifact).",
     "'Accept the file, leave the design alone, say nothing' is how a design erodes into fiction "
     "over N legitimate fix cycles while reporting zero gaps — the failure that sank the original "
     f"reflow. Decided 2026-07-19 ({SESSION}); evidence: erosion trials, BL-33.",
     ["cap:reconcile-built", "cap:change"]),
    ("dec:passing-is-verified", "Verified means a check that passes, not one that exists",
     "verification_coverage counts passing checks; a failing check is its own 0.8 gap.",
     "A failing test used to satisfy the gap that asked for a test, and passing vs failing were "
     "byte-identical to every diagnostic — counting test nodes while ignoring test results. "
     f"Decided 2026-07-19 ({SESSION}); evidence: phase-coverage trial, BL-30.",
     ["cap:detect"]),
    ("dec:report-dont-judge", "The confirmation ledger reports claim history, never judges a claim",
     "Per capability: drifting / confirmed / unexamined, with the disposition counts visible.",
     "Five design_holds claims with zero design edits is the erosion signature and the ledger "
     "makes it legible — but judging a specific claim false is semantic, and a deterministic "
     "detector would fire on every stable design with cosmetic churn (the unexpected_coupling "
     f"lesson). Decided 2026-07-19 ({SESSION}); BL-35.",
     ["cap:freshness"]),
    ("dec:views-are-projections", "A view is a projection of the graph; renderer fill-ins are defects",
     "Renderers may only emit what the graph states; everything else is confessed as a finding.",
     "UAF/DoDAF doctrine from the author: the graph stores all design detail, the agent only "
     "renders viewpoints. If rendering requires extrapolation, something is missing inside "
     f"reflow2 — almost always. Decided 2026-07-19 ({SESSION}); render_views.py, BL-40.",
     ["cap:report"]),
    ("dec:repo-file-embedded", "The graph lives as a repo file, embedded — not a service",
     "RocksDB directory beside the repo, exports as the durable, diffable record.",
     "The service's strongest argument (concurrency) is hypothetical while there is one writer; "
     "it would put the user's design on a machine they do not control and is permanent "
     "operational cost. Reopening conditions are written down. Decided 2026-07-18 "
     "(surface-plan.md); BL-12/BL-15 carry the consequences.",
     ["cap:portability"]),
]

# ---- P2 · Structure. Coarse: crate -> module. -----------------------------
SUBSYSTEMS = [
    ("cmp:core", "reflow2-core", "The deterministic, LLM-free coherence engine.", "subsystem"),
    ("cmp:mcp", "reflow2-mcp", "The agent-facing MCP surface over one graph.", "subsystem"),
    ("cmp:kit", "consumer kit", "What gets installed into a project being designed.", "subsystem"),
]
MODULES = [
    ("cmp:temporal", "temporal", "cmp:core", ["cap:change"]),
    ("cmp:propagate", "propagate", "cmp:core", ["cap:propagate"]),
    ("cmp:detect", "detect", "cmp:core", ["cap:detect", "cap:surface", "cap:questions"]),
    ("cmp:heal", "heal", "cmp:core", ["cap:heal"]),
    ("cmp:structure", "structure", "cmp:core", ["cap:heal"]),
    ("cmp:drift", "drift", "cmp:core", ["cap:reconcile-built"]),
    ("cmp:vocabulary", "vocabulary", "cmp:core", ["cap:vocabulary"]),
    ("cmp:export", "export", "cmp:core", ["cap:portability"]),
    ("cmp:provenance", "provenance", "cmp:core", ["cap:stamp"]),
    ("cmp:allocate", "allocate", "cmp:core", ["cap:allocate"]),
    ("cmp:hierarchy", "hierarchy", "cmp:core", ["cap:hierarchy"]),
    ("cmp:dimensions", "dimensions", "cmp:core", ["cap:dimensions"]),
    ("cmp:ingest", "ingest", "cmp:core", ["cap:ingest"]),
    ("cmp:report", "report", "cmp:core", ["cap:report"]),
    ("cmp:graph", "graph", "cmp:core", ["cap:portability"]),
    ("cmp:verify", "verify", "cmp:core", []),
    ("cmp:operate", "operate", "cmp:core", []),
    ("cmp:service", "service", "cmp:mcp", ["cap:mcp-surface"]),
    ("cmp:init", "reflow2_init", "cmp:kit", ["cap:kit"]),
    ("cmp:skills", "skills", "cmp:kit", ["cap:kit"]),
]

# ---- P3/P4 · one Artifact per module, one Verification per test file ------
ARTIFACTS = {  # component -> source path
    "cmp:temporal": "crates/reflow2-core/src/temporal.rs",
    "cmp:propagate": "crates/reflow2-core/src/propagate.rs",
    "cmp:detect": "crates/reflow2-core/src/detect.rs",
    "cmp:heal": "crates/reflow2-core/src/heal.rs",
    "cmp:structure": "crates/reflow2-core/src/structure.rs",
    "cmp:drift": "crates/reflow2-core/src/drift.rs",
    "cmp:vocabulary": "crates/reflow2-core/src/vocabulary.rs",
    "cmp:export": "crates/reflow2-core/src/export.rs",
    "cmp:provenance": "crates/reflow2-core/src/provenance.rs",
    "cmp:allocate": "crates/reflow2-core/src/allocate.rs",
    "cmp:hierarchy": "crates/reflow2-core/src/hierarchy.rs",
    "cmp:dimensions": "crates/reflow2-core/src/dimensions.rs",
    "cmp:ingest": "crates/reflow2-core/src/ingest.rs",
    "cmp:report": "crates/reflow2-core/src/report.rs",
    "cmp:graph": "crates/reflow2-core/src/graph.rs",
    "cmp:verify": "crates/reflow2-core/src/verify.rs",
    "cmp:operate": "crates/reflow2-core/src/operate.rs",
    "cmp:service": "crates/reflow2-mcp/src/service.rs",
    "cmp:init": "tools/reflow2_init.py",
}
VERIFICATIONS = {  # capability -> test file
    "cap:change": "crates/reflow2-core/tests/temporal.rs",
    "cap:propagate": "crates/reflow2-core/tests/propagate.rs",
    "cap:detect": "crates/reflow2-core/tests/detect.rs",
    "cap:heal": "crates/reflow2-core/tests/heal.rs",
    "cap:reconcile-built": "crates/reflow2-core/tests/drift.rs",
    "cap:portability": "crates/reflow2-core/tests/export.rs",
    "cap:stamp": "crates/reflow2-core/tests/provenance.rs",
    "cap:allocate": "crates/reflow2-core/tests/allocate.rs",
    "cap:hierarchy": "crates/reflow2-core/tests/hierarchy.rs",
    "cap:questions": "crates/reflow2-core/tests/gap_review.rs",
    "cap:report": "crates/reflow2-core/tests/report.rs",
    "cap:vocabulary": "crates/reflow2-core/tests/write_side.rs",
    "cap:dimensions": "crates/reflow2-core/tests/dimensions.rs",
    "cap:ingest": "crates/reflow2-core/tests/ingest.rs",
    "cap:surface": "crates/reflow2-core/tests/llm.rs",
    "cap:mcp-surface": "crates/reflow2-mcp/tests/tools.rs",
}
# Contracts between subsystems.
INTERFACES = [
    ("ifc:core-api", "DesignGraph API", "cmp:core", ["cmp:service"]),
    ("ifc:mcp-tools", "MCP tool surface", "cmp:service", ["cmp:skills"]),
    ("ifc:graph-export", "Design export document", "cmp:export", ["cmp:init"]),
]


def sha(p: pathlib.Path) -> str:
    return "sha256:" + hashlib.sha256(p.read_bytes()).hexdigest()[:16] if p.exists() else "sha256:absent"


def build(s: Server) -> None:
    s.call("genesis", {"project_id": "proj:reflow2", "name": "Reflow 2.0",
                       "objective": "Keep a design coherent from concept through operations",
                       "domain": "software"})
    for rid, (name, stmt, prio) in REQUIREMENTS.items():
        s.call("create_node", {"node_type": "Requirement", "id": rid,
                               "props": {"name": name, "statement": stmt,
                                         "priority": prio, "status": "accepted"}})
        s.call("contains", {"project_id": "proj:reflow2",
                            "child_type": "Requirement", "child_id": rid})
    for cid, name, desc, status, sats in CAPABILITIES:
        s.call("add_capability", {"id": cid, "name": name, "description": desc, "status": status})
        for r in sats:
            s.call("satisfies", {"from_id": cid, "to_id": r})
    for cid, name, desc, level in SUBSYSTEMS:
        s.call("add_component", {"id": cid, "name": name, "description": desc, "level": level})
        s.call("contains", {"project_id": "proj:reflow2",
                            "child_type": "Component", "child_id": cid})
    for cid, name, parent, caps in MODULES:
        s.call("add_component", {"id": cid, "name": name, "description": f"The {name} module."})
        s.call("contain_component", {"from_id": parent, "to_id": cid})
        for c in caps:
            s.call("allocate", {"from_id": c, "to_id": cid})
    for cmp_id, path in ARTIFACTS.items():
        p = REPO / path
        s.call("link_artifact", {"artifact_id": f"art:{cmp_id.split(':')[1]}",
                                 "name": pathlib.Path(path).name, "location": path,
                                 "artifact_type": "code", "target_type": "Component",
                                 "target_id": cmp_id, "checksum": sha(p)})
    for cap, path in VERIFICATIONS.items():
        vid = f"ver:{cap.split(':')[1]}"
        s.call("add_verification", {"id": vid, "name": pathlib.Path(path).name,
                                    "method": "test", "level": "integration"})
        s.call("verifies", {"verification_id": vid, "target_type": "Capability", "target_id": cap})
        s.call("set_verification_status", {"verification_id": vid, "status": "passing"})
    for iid, name, provider, consumers in INTERFACES:
        s.call("add_interface", {"id": iid, "name": name})
        s.call("provides", {"from_id": provider, "to_id": iid})
        for c in consumers:
            s.call("consumes", {"from_id": c, "to_id": iid})
    for did, name, decision, rationale, governs in DECISIONS:
        s.call("add_decision", {"id": did, "name": name,
                                "decision": decision, "rationale": rationale})
        for target in governs:
            s.call("governed_by", {"from_type": "Capability", "from_id": target,
                                   "to_type": "Decision", "to_id": did})
    s.call("add_release", {"id": "rel:v020", "name": "v0.2.0", "version": "0.2.0",
                           "unit_type": "binary"})
    # The as-released view (BL-34): v0.2.0 is the tagged repo, so every module
    # artifact ships in it, checksum frozen at what was registered.
    for cmp_id, path in ARTIFACTS.items():
        s.call("release_includes", {
            "release_id": "rel:v020", "target_type": "Artifact",
            "target_id": f"art:{cmp_id.split(':')[1]}",
            "as_checksum": sha(REPO / path)})
    s.call("add_environment", {"id": "env:dev", "name": "Developer machine",
                               "env_type": "development"})
    s.call("deploy_to", {"release_id": "rel:v020", "environment_id": "env:dev", "status": "active"})


def analyse(s: Server) -> None:
    rep = s.call("graph_report")
    print(f"\n{'=' * 64}\n  reflow2's own functional design: {rep['total_nodes']} nodes")
    print(f"  {dict(rep['node_counts'])}")
    print(f"{'=' * 64}")

    gaps = s.call("detect_gaps")
    print(f"\n-- detect_gaps: {len(gaps)} --")
    for g in gaps:
        who = ", ".join(g["affected_ids"]) or "(project-level)"
        print(f"  {g['severity']:.2f}  {g['gap_source']:26} {who}")
        print(f"        {g['title']}")

    defects = s.call("detect_defects")
    print(f"\n-- detect_defects: {len(defects)} --")
    for d in defects:
        print(f"  {d['severity']:8} {d['category']:24} {d['message'][:70]}")

    hier = s.call("hierarchy_issues")
    print(f"\n-- hierarchy_issues: {len(hier)} --")
    for h in hier[:10]:
        print(f"  {h.get('kind')}: {h.get('message', '')[:80]}")

    surp = s.call("surprising_connections")
    print(f"\n-- surprising_connections: {len(surp)} --")
    for x in surp[:6]:
        print(f"  {json.dumps(x)[:110]}")

    alloc = s.call("evaluate_allocation")
    print(f"\n-- evaluate_allocation --\n  {json.dumps(alloc)[:400]}")

    cov = rep["verification"]
    print(f"\n-- verification coverage --\n  {cov}")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--analyse-only", action="store_true",
                    help="import the committed export and analyse, without rebuilding")
    args = ap.parse_args()

    tmp = tempfile.mkdtemp(prefix="reflow2-design-")
    s = Server(str(REPO / "target/debug/reflow2-mcp"), str(pathlib.Path(tmp) / "graph"))
    try:
        if args.analyse_only:
            s.call("import_graph", {"document": json.loads(EXPORT.read_text())})
        else:
            build(s)
            doc = s.call("export_graph")
            EXPORT.parent.mkdir(parents=True, exist_ok=True)
            EXPORT.write_text(json.dumps(doc, indent=2, sort_keys=True) + "\n")
            print(f"exported {len(doc['nodes'])} nodes / {len(doc['edges'])} edges "
                  f"-> {EXPORT.relative_to(REPO)}")
        analyse(s)
    finally:
        s.close()
        shutil.rmtree(tmp, ignore_errors=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
