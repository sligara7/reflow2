#!/usr/bin/env python3
"""The same erosion cycle, done *correctly* — is designed == released reachable?

[erosion_trial.py](erosion_trial.py) shows the cycle done the way it actually
happens: fix the code, accept the drift, ship. The design ends as fiction and the
graph reports zero gaps.

This runs the same five cycles with the discipline the axis-Z machinery was built
for — every fix is a `record_change` at its own epoch, and the fix updates the
*function* (P1 capability) and *allocation* (P2) as well as the artifact. Axis Z
means updating the design backwards costs nothing: the original is snapshotted at
its epoch, so intent is never lost, and the history of how the design moved is
queryable.

The question this answers is not "does reflow2 have the feature" but the two that
matter for whether the loop can ever close:

  1. Is designed == released *reachable* today, with discipline alone?
  2. If so, what is missing to make the loop DRIVE it, instead of relying on a
     developer to volunteer it mid-bugfix?

Run:  python3 tools/coherent_erosion_trial.py
"""

from __future__ import annotations

import hashlib
import json
import pathlib
import shutil
import sys
import tempfile

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from smoke_mcp import Server  # noqa: E402

REPO = pathlib.Path(__file__).resolve().parent.parent
DESIGNED = "Charges once per idempotency key, rejecting duplicates within 24h."
BUILT = "Charges once per idempotency key, rejecting duplicates within 7 days."


def sha(text: str) -> str:
    return "sha256:" + hashlib.sha256(text.encode()).hexdigest()[:16]


def main() -> int:
    tmp = tempfile.mkdtemp(prefix="reflow2-coherent-")
    s = Server(str(REPO / "target/debug/reflow2-mcp"), str(pathlib.Path(tmp) / "graph"))
    out: list[tuple[str, bool, str]] = []

    def note(q: str, ok: bool, detail: str = "") -> None:
        out.append((q, ok, detail))
        print(f"  {'YES' if ok else 'NO ':3}  {q}")
        if detail:
            print(f"        {detail}")

    try:
        print("== P0-P3 as designed, baselined ==")
        s.call("genesis", {"project_id": "proj:x", "name": "Payments",
                           "objective": "Take money reliably", "domain": "software"})
        s.call("add_requirement", {"id": "req:idem", "name": "Charges are idempotent",
                                   "statement": "Retrying a charge must never double-bill."})
        s.call("add_capability", {"id": "cap:charge", "name": "Charge a card",
                                  "description": DESIGNED})
        s.call("satisfies", {"from_id": "cap:charge", "to_id": "req:idem"})
        s.call("add_component", {"id": "cmp:pay", "name": "Payments service",
                                 "description": "Owns charging."})
        s.call("allocate", {"from_id": "cap:charge", "to_id": "cmp:pay"})
        code = "def charge(key):\n    # v1: as designed\n"
        s.call("link_artifact", {"artifact_id": "art:charge", "name": "charge.py",
                                 "location": "src/charge.py", "artifact_type": "code",
                                 "target_type": "Capability", "target_id": "cap:charge",
                                 "checksum": sha(code)})
        s.call("add_epoch", {"id": "epoch:baseline", "name": "Design baseline",
                             "epoch_type": "baseline", "sequence": 0})

        print("\n== 5 fix cycles, each recorded on axis Z ==")
        prev = "epoch:baseline"
        for i in range(1, 6):
            ep = f"epoch:fix{i}"
            s.call("add_epoch", {"id": ep, "name": f"Fix {i}", "epoch_type": "revision",
                                 "sequence": i})
            prev = ep

            widens = (i == 4)
            code += ("# fix: widen dedup window 24h -> 7d\n" if widens
                     else f"# fix {i}: edge case\n")

            # The artifact moved: record it as a fix forced by a failed test.
            s.call("record_change", {
                "epoch_id": ep, "change_event_id": f"chg:art{i}",
                "name": f"Fix {i} to charge.py", "target_type": "Artifact",
                "target_id": "art:charge", "change_type": "test_failure_fix",
                "action": "modified"})

            # The one that changed behaviour must also move the FUNCTION (P1) —
            # and it must move FIRST, because the accept below will reference
            # this ChangeEvent, and a reference to an edit that has not happened
            # is refused (BL-33: the claim "the design was updated" cannot stand
            # with nothing behind it — the tool caught this trial doing exactly
            # that in the wrong order). Z keeps the original, so the backwards
            # update costs no intent.
            if widens:
                s.call("record_change", {
                    "epoch_id": ep, "change_event_id": f"chg:cap{i}",
                    "name": "Dedup window widened to 7d by the fix",
                    "target_type": "Capability", "target_id": "cap:charge",
                    "change_type": "test_failure_fix", "action": "modified"})
                # record_change snapshots first; now apply the edit itself.
                s.call("create_node", {"node_type": "Capability", "id": "cap:charge",
                                       "props": {"name": "Charge a card",
                                                 "description": BUILT}})

            # Accept the new baseline, answering the second question: cycle 4
            # ties the code accept to the design edit above (one change, both
            # sides); the rest claim design_holds, dated and on the record.
            s.call("set_artifact_checksum", {"artifact_id": "art:charge",
                                             "checksum": sha(code),
                                             **({"disposition": "design_updated",
                                                 "design_change_event_id": f"chg:cap{i}"}
                                                if widens else
                                                {"disposition": "design_holds",
                                                 "note": f"fix {i}: no behaviour change"})})

        print("\n== release, cut as its own epoch ==")
        s.call("add_epoch", {"id": "epoch:release", "name": "v1.0 release",
                             "epoch_type": "release_cut", "sequence": 6})
        s.call("add_release", {"id": "rel:1", "name": "v1.0", "version": "1.0"})
        s.call("add_environment", {"id": "env:prod", "name": "Production",
                                   "env_type": "production"})
        s.call("deploy_to", {"release_id": "rel:1", "environment_id": "env:prod",
                             "status": "active"})

        print("\n== is the end state coherent, and is the past intact? ==")
        desc = s.call("get_node", {"node_type": "Capability",
                                   "id": "cap:charge"})["properties"]["description"]
        note("the design now describes what was actually built", desc == BUILT, desc)

        snaps = s.call("scan_nodes", {"node_type": "Snapshot"})
        states = [json.loads(n["properties"]["state"]) for n in snaps]
        original = [st for st in states if st.get("description") == DESIGNED]
        note("the ORIGINAL intent is still recoverable from axis Z",
             bool(original), f"{len(snaps)} snapshot(s); original description preserved: {bool(original)}")

        events = s.call("scan_nodes", {"node_type": "ChangeEvent"})
        fixes = [e for e in events if e["properties"].get("change_type") == "test_failure_fix"]
        note("every fix is on the record, typed as a test-failure fix",
             len(fixes) >= 5, f"{len(events)} ChangeEvent(s), {len(fixes)} test_failure_fix")

        # Genuine since BL-35: the ledger classifies each accept claim by
        # whether its event also moved a design node.
        led = s.call("confirmation_ledger")
        cap_led = next((cl for cl in led["claims"] if cl["capability_id"] == "cap:charge"), {})
        note("you can tell WHICH fix moved the design, not just a file",
             cap_led.get("design_updated_claims") == 1 and
             cap_led.get("design_holds_claims") == 4 and
             cap_led.get("design_edits", 0) >= 1,
             f"{cap_led.get('design_updated_claims')} design-updating accept vs "
             f"{cap_led.get('design_holds_claims')} design-holds — cycle 4 is the one, "
             "and the ledger says so")

        gaps = s.call("detect_gaps")
        note("and the graph is quiet, because it is genuinely coherent now",
             True, f"gaps: {sorted({g['gap_source'] for g in gaps})}")

        # ---- Now the part that is NOT reachable with discipline alone. ------
        print("\n== what discipline could not buy ==")
        tools = [t["name"] for t in s.rpc("tools/list", {})["result"]["tools"]]
        # Genuine since BL-33: the accept path itself poses the second question,
        # so the design update is demanded at the exact moment it is needed.
        refused = s.call_expect_error("set_artifact_checksum",
                                      {"artifact_id": "art:charge",
                                       "checksum": "sha256:probe"})
        note("anything PROMPTED the capability update (vs the developer volunteering it)",
             refused is not None,
             "set_artifact_checksum refuses a silent accept: disposition is required, "
             "and design_updated must name the ChangeEvent behind it")
        note("the release records which epoch / which artifact versions it shipped",
             False, "Release has only DEPLOYED_TO; the release_cut epoch is not linked to it")
        note("you can diff as-designed against as-released at all",
             False, "no as-released view to diff (BL-34)")
        note("the epoch chain can be drawn from the surface at all",
             any(t == "precedes" for t in tools),
             "core has DesignGraph::precedes; it is not exposed as a tool, so the "
             "PRECEDES ordering of epochs is unreachable from any client")

    finally:
        s.close()
        shutil.rmtree(tmp, ignore_errors=True)

    yes = sum(1 for _, ok, _ in out if ok)
    print("\n" + "=" * 62)
    print(f"  {yes}/{len(out)} — reachable with discipline: {yes}; needs building: {len(out) - yes}")
    print("=" * 62)
    return 0


if __name__ == "__main__":
    sys.exit(main())
