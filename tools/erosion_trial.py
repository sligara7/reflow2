#!/usr/bin/env python3
"""Erosion trial — does the design still describe what actually shipped?

The failure this measures is not a drift *event*. It is the write → test → fix →
test → fix → … → release cycle, where every individual step is legitimate — a
test failed, someone fixed the code — and nobody ever decides to diverge from the
design. After N iterations the code is the truth, the design is fiction, and no
single moment was wrong.

So the question is not "did a file change?" (the answer is always yes, and always
"I know, I fixed a bug"). It is:

    at release, does the design describe what was released?

Run:  python3 tools/erosion_trial.py [--cycles N]
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


def sha(text: str) -> str:
    return "sha256:" + hashlib.sha256(text.encode()).hexdigest()[:16]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--cycles", type=int, default=5)
    ap.add_argument("--binary", default=str(REPO / "target/debug/reflow2-mcp"))
    args = ap.parse_args()

    tmp = tempfile.mkdtemp(prefix="reflow2-erosion-")
    s = Server(args.binary, str(pathlib.Path(tmp) / "graph"))
    findings: list[tuple[str, bool, str]] = []

    def note(q: str, ok: bool, detail: str = "") -> None:
        findings.append((q, ok, detail))
        print(f"  {'YES' if ok else 'NO ':3}  {q}")
        if detail:
            print(f"        {detail}")

    try:
        # ---- The design, as authored. Good, complete, coherent. -----------
        print("== the design as authored ==")
        s.call("genesis", {"project_id": "proj:x", "name": "Payments",
                           "objective": "Take money reliably", "domain": "software"})
        s.call("add_requirement", {"id": "req:idem",
                                   "name": "Charges are idempotent",
                                   "statement": "Retrying a charge must never double-bill."})
        s.call("add_capability", {"id": "cap:charge", "name": "Charge a card",
                                  "description": "Charges once per idempotency key, "
                                                 "rejecting duplicates within 24h."})
        s.call("satisfies", {"from_id": "cap:charge", "to_id": "req:idem"})
        s.call("add_component", {"id": "cmp:pay", "name": "Payments service",
                                 "description": "Owns charging."})
        s.call("allocate", {"from_id": "cap:charge", "to_id": "cmp:pay"})

        code = "def charge(key):\n    # v1: as designed\n    ...\n"
        s.call("link_artifact", {"artifact_id": "art:charge", "name": "charge.py",
                                 "location": "src/charge.py", "artifact_type": "code",
                                 "target_type": "Capability", "target_id": "cap:charge",
                                 "checksum": sha(code)})
        s.call("add_verification", {"id": "ver:charge", "name": "charge tests",
                                    "method": "test", "level": "unit"})
        s.call("verifies", {"verification_id": "ver:charge",
                            "target_type": "Capability", "target_id": "cap:charge"})
        s.call("set_verification_status", {"verification_id": "ver:charge", "status": "passing"})
        print(f"   {s.call('graph_report')['total_nodes']} nodes, "
              f"{len(s.call('detect_gaps'))} gaps — a clean thread")

        # ---- N rounds of test → fix → accept. Each one legitimate. --------
        print(f"\n== {args.cycles} rounds of test-fails / fix-code / accept ==")
        drifts = 0
        for i in range(1, args.cycles + 1):
            # The test fails, so the code is fixed. The 4th fix quietly widens
            # the dedup window from 24h to 7d — a real behaviour change that
            # nobody writes back into the capability description.
            change = ("# fix: widen dedup window 24h -> 7d\n" if i == 4
                      else f"# fix {i}: edge case\n")
            code += change
            s.call("set_verification_status", {"verification_id": "ver:charge", "status": "failing"})
            rec = s.call("reconcile_artifacts", {
                "observed": [{"artifact_id": "art:charge", "present": True,
                              "checksum": sha(code)}],
                "record_events": True, "at": f"2026-07-19T0{i}:00:00Z"})
            drifts += len(rec.get("findings", []))
            # "Yes, I know, I fixed a bug" — accept the new reality as baseline.
            # BL-33: silent accept no longer exists; the careless answer is to
            # claim design_holds every time — including cycle 4, where it is a
            # lie. The lie is now dated and on the record.
            s.call("set_artifact_checksum", {"artifact_id": "art:charge",
                                             "checksum": sha(code),
                                             "disposition": "design_holds",
                                             "note": f"fix {i}: tests pass again"})
            s.call("set_verification_status", {"verification_id": "ver:charge", "status": "passing"})
        print(f"   {drifts} drift finding(s) raised and accepted across {args.cycles} cycles")

        # ---- Ship it. -----------------------------------------------------
        print("\n== release ==")
        s.call("add_release", {"id": "rel:1", "name": "v1.0", "version": "1.0"})
        s.call("add_environment", {"id": "env:prod", "name": "Production",
                                   "env_type": "production"})
        s.call("deploy_to", {"release_id": "rel:1", "environment_id": "env:prod",
                             "status": "active"})

        # ---- What does the design now know? -------------------------------
        print("\n== after N cycles and a release, does the design know? ==")
        gaps = s.call("detect_gaps")
        sources = sorted({g["gap_source"] for g in gaps})
        # Genuine since BL-35: the ledger reports the whole movement history —
        # how many times the code moved, and what each move was claimed to mean.
        led = s.call("confirmation_ledger")
        cap_led = next((cl for cl in led["claims"] if cl["capability_id"] == "cap:charge"), {})
        note("the design reports how the code moved under it, and how each move was answered",
             cap_led.get("drift_events") == drifts and
             cap_led.get("design_holds_claims") == drifts and
             cap_led.get("design_edits") == 0,
             f"{cap_led.get('drift_events')} drifts, {cap_led.get('design_holds_claims')} "
             f"'design holds' claims, {cap_led.get('design_edits')} design edits — the five "
             "claims with zero edits ARE the erosion signature, now legible")

        cap = s.call("get_node", {"node_type": "Capability", "id": "cap:charge"})
        desc = cap["properties"]["description"]
        note("the capability description still says 24h (now false)",
             "24h" in desc, desc)
        note("anything flags a description contradicted by its own artifact's history", False
             if "24h" in desc else True,
             "nothing compares a description against what the code became")

        accepts = [e for e in s.call("scan_nodes", {"node_type": "ChangeEvent"})
                   if e["node_id"].startswith("chg:accept-")]
        note("every accept had to answer the second question, on the record",
             len(accepts) == drifts,
             f"{len(accepts)} recorded accept claim(s) for {drifts} drift(s) — "
             "cycle 4's 'design_holds' is a lie, but a dated, auditable one")

        # Tightened (was `> 0`, which one surviving event weakly satisfied while
        # four of five had collapsed into it): every drift must leave its own
        # event, or "drifted once" and "drifted N times" are the same graph.
        events = len(s.call("scan_nodes", {"node_type": "DriftEvent"}))
        note("the graph records EVERY drift, not just the latest",
             events == drifts,
             f"{events} DriftEvent(s) retained for {drifts} drift(s)")

        # Genuine since BL-34: record the manifest, then ask the question.
        s.call("release_includes", {"release_id": "rel:1", "target_type": "Artifact",
                                    "target_id": "art:charge", "as_checksum": sha(code)})
        rr = s.call("release_report", {"release_id": "rel:1"})
        note("the release records WHAT it contains",
             rr["artifacts"] == [["art:charge", sha(code)]],
             f"manifest: {rr['artifacts']} — checksum frozen at cut")
        note("you can ask 'does what shipped match what was designed?'",
             rr["capabilities_covered"] == ["cap:charge"]
             and rr["built_capabilities_not_covered"] == [],
             "release_report: covered=" + str(rr["capabilities_covered"]) +
             " not_covered=" + str(rr["built_capabilities_not_covered"]) +
             " — the STRUCTURAL answer is yes; whether the description still tells "
             "the truth about the 7-day window is the ledger's 5-claims-0-edits line")

        # Genuine check (was a hardcoded fail before BL-30): flip the one
        # verification red and see whether coverage notices.
        s.call("set_verification_status", {"verification_id": "ver:charge",
                                           "status": "failing"})
        red = s.call("graph_report")["verification"]
        s.call("set_verification_status", {"verification_id": "ver:charge",
                                           "status": "passing"})
        green = s.call("graph_report")["verification"]
        note("verification coverage distinguishes 'passing' from 'merely present'",
             red["capabilities_verified"] == 0 and green["capabilities_verified"] == 1,
             f"failing: {red['capabilities_verified']} verified, passing: {green['capabilities_verified']}")

    finally:
        s.close()
        shutil.rmtree(tmp, ignore_errors=True)

    missed = [f for f in findings if not f[1]]
    print("\n" + "=" * 62)
    print(f"  {len(findings) - len(missed)}/{len(findings)} of what the design should know, it knows")
    print("=" * 62)
    return 1 if missed else 0


if __name__ == "__main__":
    sys.exit(main())
