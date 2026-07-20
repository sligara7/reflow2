#!/usr/bin/env python3
"""Phase-coverage trial — does the golden thread still carry weight after P2?

The question this exists to answer is the one that sank the original reflow:
the early phases (intent → function → structure) went well, and then build,
test and deploy proceeded as if none of it had happened. Every reflow2 trial so
far stops at or before P2, so the whole evidence base is drawn from the phases
reflow1 was already good at.

Method — deliberate probes, not observation. The two highest-value findings in
the trial record (ophyd's unmotivated capability, 3dtictactoe's orphan) came
from seeding a defect on purpose and checking whether the tool noticed. This
does that for P3/P4/P5: build a realistic graph of reflow2's own repo, inject
the divergences each phase is supposed to catch, and score whether the graph
surfaced them.

A probe is scored CAUGHT only if the graph names it without being told where to
look. "The write succeeded" is not the question; whether the design notices
reality moving underneath it is.

Run:  python3 tools/phase_trial.py [--binary PATH] [--graph PATH]
Exits non-zero if any probe is uncaught, so the miss is loud.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import shutil
import subprocess
import sys
import tempfile

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from smoke_mcp import Server  # noqa: E402

REPO = pathlib.Path(__file__).resolve().parent.parent


def sha(path: pathlib.Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()[:16]


class Probes:
    """Scoreboard. A probe is a divergence we inject on purpose."""

    def __init__(self) -> None:
        self.rows: list[tuple[str, str, bool, str]] = []

    def score(self, phase: str, what: str, caught: bool, detail: str = "") -> None:
        self.rows.append((phase, what, caught, detail))
        mark = "CAUGHT " if caught else "MISSED "
        print(f"  {mark} [{phase}] {what}")
        if detail:
            print(f"          {detail}")

    def report(self) -> int:
        print("\n" + "=" * 62)
        by_phase: dict[str, list[bool]] = {}
        for phase, _, caught, _ in self.rows:
            by_phase.setdefault(phase, []).append(caught)
        for phase, results in sorted(by_phase.items()):
            hit = sum(results)
            print(f"  {phase}: {hit}/{len(results)} caught")
        missed = [r for r in self.rows if not r[2]]
        print("=" * 62)
        if missed:
            print(f"\n{len(missed)} probe(s) MISSED — the graph did not notice:")
            for phase, what, _, _ in missed:
                print(f"  [{phase}] {what}")
            return 1
        print("\nAll probes caught.")
        return 0


def seed_design(s: Server, files: dict[str, pathlib.Path]) -> None:
    """P0-P2: reflow2's own design, coarse-grained (one Component per module).

    Deliberately coarse — BL-23's lesson is that one Artifact per source file
    made 22 of 25 gaps noise on this very repo.
    """
    s.call("genesis", {"project_id": "proj:reflow2", "name": "Reflow 2.0",
                       "objective": "Keep a design coherent across its whole lifecycle",
                       "domain": "software"})

    reqs = {
        "req:coherence": ("Design stays coherent", "A change in any phase surfaces its ripples."),
        "req:no-silent": ("No silent fallbacks", "Failures and skipped work are surfaced, never swallowed."),
        "req:golden-thread": ("Traceability end to end", "Every artifact traces back to the intent it serves."),
        "req:survives-upgrade": ("A graph survives an upgrade", "An existing design opens after reflow2 changes."),
    }
    for rid, (name, stmt) in reqs.items():
        s.call("add_requirement", {"id": rid, "name": name, "statement": stmt})

    caps = {
        "cap:detect": ("Gap detection", "Find what the design has not decided yet.", ["req:coherence", "req:golden-thread"]),
        "cap:propagate": ("Impact propagation", "Walk the blast radius of a change.", ["req:coherence"]),
        "cap:heal": ("Structural repair", "Propose and apply content-free fixes.", ["req:coherence"]),
        "cap:drift": ("As-built reconciliation", "Compare the design against what was built.", ["req:golden-thread"]),
        "cap:export": ("Export and import", "Move a design across a version boundary.", ["req:survives-upgrade"]),
        "cap:surface": ("Agent-native surface", "Drive the whole loop over MCP.", ["req:no-silent"]),
    }
    for cid, (name, desc, sat) in caps.items():
        s.call("add_capability", {"id": cid, "name": name, "description": desc, "status": "realized"})
        for r in sat:
            s.call("satisfies", {"from_id": cid, "to_id": r})

    s.call("add_component", {"id": "cmp:core", "name": "reflow2-core",
                             "description": "The deterministic LLM-free core.", "level": "subsystem"})
    s.call("add_component", {"id": "cmp:mcp", "name": "reflow2-mcp",
                             "description": "The MCP surface.", "level": "subsystem"})
    for cid, parent in [("cmp:detect", "cmp:core"), ("cmp:propagate", "cmp:core"),
                        ("cmp:heal", "cmp:core"), ("cmp:drift", "cmp:core"),
                        ("cmp:export", "cmp:core"), ("cmp:service", "cmp:mcp")]:
        s.call("add_component", {"id": cid, "name": cid.split(":")[1],
                                 "description": f"The {cid.split(':')[1]} module."})
        s.call("contain_component", {"from_id": parent, "to_id": cid})

    for cap, cmp_id in [("cap:detect", "cmp:detect"), ("cap:propagate", "cmp:propagate"),
                        ("cap:heal", "cmp:heal"), ("cap:drift", "cmp:drift"),
                        ("cap:export", "cmp:export"), ("cap:surface", "cmp:service")]:
        s.call("allocate", {"from_id": cap, "to_id": cmp_id})


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--binary", default=str(REPO / "target/debug/reflow2-mcp"))
    ap.add_argument("--graph", default=None)
    args = ap.parse_args()

    tmp = tempfile.mkdtemp(prefix="reflow2-phase-trial-")
    graph = args.graph or str(pathlib.Path(tmp) / "graph")
    # Real files from this repo, copied so a probe can mutate them safely.
    work = pathlib.Path(tmp) / "src"
    work.mkdir()
    sources = {
        "detect.rs": REPO / "crates/reflow2-core/src/detect.rs",
        "heal.rs": REPO / "crates/reflow2-core/src/heal.rs",
        "drift.rs": REPO / "crates/reflow2-core/src/drift.rs",
        "export.rs": REPO / "crates/reflow2-core/src/export.rs",
        "propagate.rs": REPO / "crates/reflow2-core/src/propagate.rs",
        "service.rs": REPO / "crates/reflow2-mcp/src/service.rs",
    }
    files = {}
    for name, src in sources.items():
        dst = work / name
        shutil.copy(src, dst)
        files[name] = dst

    p = Probes()
    s = Server(args.binary, graph)
    try:
        print("== seed P0-P2 (intent, function, structure) ==")
        seed_design(s, files)
        rep = s.call("graph_report")
        print(f"   {rep['total_nodes']} nodes, {rep['gap_count']} gaps, {rep['defect_count']} defects")

        run_p3(s, p, files)
        run_p4(s, p, files)
        run_p5(s, p)

        print("\n== the thread, end to end ==")
        run_thread(s, p, files)
    finally:
        s.close()
        shutil.rmtree(tmp, ignore_errors=True)

    return p.report()


def run_p3(s: Server, p: Probes, files: dict[str, pathlib.Path]) -> None:
    print("\n== P3 · realization: does the design notice what was actually built? ==")
    for name, path in files.items():
        cap = {"detect.rs": "cap:detect", "heal.rs": "cap:heal", "drift.rs": "cap:drift",
               "export.rs": "cap:export", "propagate.rs": "cap:propagate",
               "service.rs": "cap:surface"}[name]
        s.call("link_artifact", {
            "artifact_id": f"art:{name}", "name": name, "location": str(path),
            "artifact_type": "code", "target_type": "Capability", "target_id": cap,
            "checksum": sha(path)})

    # Probe 1 — a file changed after it was registered.
    files["detect.rs"].write_text(files["detect.rs"].read_text() + "\n// drifted\n")
    observed = [{"artifact_id": f"art:{n}", "present": True, "checksum": sha(f)}
                for n, f in files.items()]
    # Probe 2 — a file the design says exists, deleted.
    files["heal.rs"].unlink()
    observed = [o for o in observed if o["artifact_id"] != "art:heal.rs"]
    observed.append({"artifact_id": "art:heal.rs", "present": False})

    rec = s.call("reconcile_artifacts", {"observed": observed})
    found = {f["kind"]: f for f in rec.get("findings", [])}
    p.score("P3", "a file changed since it was registered",
            "checksum_change" in found, f"kinds: {sorted(found)}")
    p.score("P3", "a file the design says exists, deleted",
            "missing_artifact" in found, f"kinds: {sorted(found)}")

    # Probe 3 — a capability that nothing realizes.
    s.call("add_capability", {"id": "cap:unbuilt", "name": "Unbuilt thing",
                              "description": "Designed, never written.", "status": "planned"})
    s.call("satisfies", {"from_id": "cap:unbuilt", "to_id": "req:coherence"})
    s.call("allocate", {"from_id": "cap:unbuilt", "to_id": "cmp:core"})
    sources = [g["gap_source"] for g in s.call("detect_gaps")]
    p.score("P3", "a capability designed but never built",
            "unrealized_capability" in sources, f"gaps: {sorted(set(sources))}")

    # Probe 4 — a real source file no artifact registers.
    stray = files["export.rs"].parent / "untracked.rs"
    stray.write_text("// a real file the design knows nothing about\n")
    rec2 = s.call("reconcile_artifacts", {"observed": [
        {"artifact_id": "art:untracked.rs", "present": True, "checksum": sha(stray)}]})
    kinds2 = {f["kind"] for f in rec2.get("findings", [])}
    p.score("P3", "a built file the design has never heard of",
            "undocumented_addition" in kinds2, f"kinds: {sorted(kinds2)}")


def run_p4(s: Server, p: Probes, files: dict[str, pathlib.Path]) -> None:
    print("\n== P4 · verification: does the design know what is actually proven? ==")
    s.call("add_verification", {"id": "ver:detect", "name": "detect.rs tests",
                                "method": "test", "level": "unit"})
    s.call("verifies", {"verification_id": "ver:detect",
                        "target_type": "Capability", "target_id": "cap:detect"})
    s.call("set_verification_status", {"verification_id": "ver:detect", "status": "passing"})

    # Probe 5 — a capability claiming to be verified with nothing verifying it.
    s.call("set_capability_status", {"capability_id": "cap:heal", "status": "verified"})
    gaps = s.call("detect_gaps")
    sources = [g["gap_source"] for g in gaps]
    named = [g for g in gaps if "cap:heal" in g.get("affected_ids", [])]
    p.score("P4", "a capability with no verification is surfaced at all",
            bool(named), f"sources: {sorted(set(sources))}")
    # Sharper: the status field now *claims* verified while nothing verifies it.
    # Being told "this is unverified" is not the same as being told the design
    # contradicts itself, and only the second is a coherence failure.
    contradiction = any("verified" in (g.get("description", "") + g.get("evidence", ""))
                        and "cap:heal" in g.get("affected_ids", []) for g in gaps)
    p.score("P4", "the design NOTICES a status claiming verified that is not",
            contradiction,
            "unverified_capability fires either way; nothing reads status=verified as a claim")

    # Probe 6 — a verification that FAILED. Does it reach the requirement behind it?
    s.call("add_verification", {"id": "ver:drift", "name": "drift.rs tests",
                                "method": "test", "level": "unit"})
    s.call("verifies", {"verification_id": "ver:drift",
                        "target_type": "Capability", "target_id": "cap:drift"})
    s.call("set_verification_status", {"verification_id": "ver:drift", "status": "failing"})
    gaps = s.call("detect_gaps")
    named = any("ver:drift" in g.get("affected_ids", []) or "cap:drift" in g.get("affected_ids", [])
                for g in gaps)
    p.score("P4", "a FAILING verification is surfaced as a problem", named,
            f"gap sources now: {sorted({g['gap_source'] for g in gaps})}")

    # Probe 7 — reconciliation: the graph says passing, the run says failed.
    # CAUGHT only if detect_gaps names the divergence without being told where
    # to look. (ver:detect is recorded passing above.)
    r = s.call("reconcile_verification", {
        "observed": [{"verification_id": "ver:detect", "outcome": "failed"}],
        "record_events": True})
    gaps = s.call("detect_gaps")
    named = [g for g in gaps if g["gap_source"] == "unresolved_drift"
             and "ver:detect" in g.get("affected_ids", [])]
    p.score("P4", "a way to reconcile recorded status against a real test run",
            bool(r["findings"]) and bool(named),
            f"findings={len(r['findings'])}, gap sources: {sorted({g['gap_source'] for g in gaps})}")


def run_p5(s: Server, p: Probes) -> None:
    print("\n== P5 · deploy & operate: does the design know what is actually running? ==")
    s.call("add_release", {"id": "rel:v020", "name": "v0.2.0", "version": "0.2.0"})
    s.call("add_environment", {"id": "env:dev", "name": "Developer machine", "env_type": "development"})
    s.call("deploy_to", {"release_id": "rel:v020", "environment_id": "env:dev", "status": "active"})

    # The release models its contents — everything built so far ships in it…
    for name in ("detect.rs", "heal.rs", "drift.rs", "export.rs", "propagate.rs",
                 "service.rs"):
        s.call("release_includes", {"release_id": "rel:v020", "target_type": "Artifact",
                                    "target_id": f"art:{name}"})

    # Probe 8 — …except this: built, and in no release.
    s.call("add_component", {"id": "cmp:orphaned-build", "name": "Undeployed part",
                             "description": "Built, in no release."})
    s.call("add_capability", {"id": "cap:orphan-fn", "name": "Orphan function",
                              "description": "Built but never shipped.",
                              "status": "realized"})
    s.call("satisfies", {"from_id": "cap:orphan-fn", "to_id": "req:coherence"})
    s.call("allocate", {"from_id": "cap:orphan-fn", "to_id": "cmp:orphaned-build"})
    s.call("link_artifact", {"artifact_id": "art:orphan", "name": "orphan.rs",
                             "location": "src/orphan.rs", "artifact_type": "code",
                             "target_type": "Component", "target_id": "cmp:orphaned-build",
                             "checksum": "sha256:orphan"})
    gaps = s.call("detect_gaps")
    named = [g for g in gaps if g["gap_source"] == "unreleased_component"
             and "cmp:orphaned-build" in g.get("affected_ids", [])]
    p.score("P5", "a built component that no release contains", bool(named),
            f"gap sources: {sorted({g['gap_source'] for g in gaps})}")

    # Probe 9 — reconciliation against what is actually deployed (BL-9).
    # The declaration says v0.2.0 is active on the dev machine; the observation
    # says nothing runs there. CAUGHT only if detect_gaps names the divergence
    # without being told where to look.
    r = s.call("reconcile_deployment", {
        "observed": [{"environment_id": "env:dev", "running": []}],
        "record_events": True})
    gaps = s.call("detect_gaps")
    named = [g for g in gaps if g["gap_source"] == "unresolved_drift"
             and "env:dev" in g.get("affected_ids", [])
             and "rel:v020" in g.get("affected_ids", [])]
    p.score("P5", "a way to reconcile the design against what is really deployed",
            bool(r["findings"]) and bool(named),
            f"findings={len(r['findings'])}, gap sources: {sorted({g['gap_source'] for g in gaps})}")


def run_thread(s: Server, p: Probes, files: dict[str, pathlib.Path]) -> None:
    """The whole point: does a P3 change reach the P0 intent behind it?"""
    rec = s.call("reconcile_artifacts", {"observed": [
        {"artifact_id": "art:detect.rs", "present": True, "checksum": sha(files["detect.rs"])}]})
    seeds = rec.get("propagation_seeds") or []
    p.score("thread", "a changed file yields seeds to propagate from", bool(seeds), f"seeds: {seeds}")

    if seeds:
        blast = s.call("propagate_from", {"seed_ids": seeds, "max_depth": 6})
        reached = {n["node_id"] if isinstance(n, dict) else n
                   for n in (blast.get("impacted") or blast.get("nodes") or [])}
        txt = json.dumps(blast)
        p.score("thread", "and the ripple reaches the REQUIREMENT behind the code",
                "req:" in txt, f"reached {len(reached)} node(s)")

    # And the reverse: intent -> code.
    blast = s.call("propagate_from", {"seed_ids": ["req:coherence"], "max_depth": 6})
    p.score("thread", "a change to intent reaches the FILES that implement it",
            "art:" in json.dumps(blast),
            "requirement -> artifact traversal")


if __name__ == "__main__":
    sys.exit(main())
