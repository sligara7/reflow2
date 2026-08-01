#!/usr/bin/env python3
"""Evidence-quality instrument (BL-106 / BL-126 / BL-136).

The family's whole claim is that reflow2 has always recorded that a check EXISTS
and that it PASSES, and never what its evidence COVERS. That claim is only worth
believing if the new reports say something about a real design — so run this
against reflow2's own committed export and read what comes back.

Three axes, three questions:

  TIME (BL-106)         which claims rest on a check older than the last accepted
                        change to what it covers?
  INPUT (BL-126)        which claims are proven only at fixed parameter values —
                        and how many checks state no scope at all?
  INDEPENDENCE (BL-136) which passing checks are consumed, because the thing they
                        verify was fitted to them?

A number that moves is the claim; a number that does NOT move is a finding too,
and worth recording rather than assumed (the BL-124 lesson: that item's asserted
consequence was half right, and only running the instrument said which half).

    python3 tools/evidence_quality_instrument.py <graph-dir>
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from smoke_mcp import Server  # noqa: E402

BIN = "target/debug/reflow2-mcp"
EXPORT = "docs/design/reflow2.json"


def call(server: Server, name: str, args: dict):
    r = server.rpc("tools/call", {"name": name, "arguments": args})
    if "error" in r:
        return {"_error": r["error"].get("message", "")[:300]}
    txt = r["result"]["content"][0]["text"]
    try:
        return json.loads(txt)
    except Exception:
        return {"_raw": txt[:300]}


def main() -> None:
    graph_dir = sys.argv[1]
    if os.path.exists(graph_dir):
        shutil.rmtree(graph_dir)
    subprocess.run(
        [BIN, "--graph-path", graph_dir, "--import", EXPORT],
        check=True,
        capture_output=True,
    )
    s = Server(BIN, graph_dir)

    doc = json.load(open(EXPORT))
    print(f"nodes {len(doc['nodes'])}  edges {len(doc['edges'])}")

    # ---- TIME ---------------------------------------------------------------
    led = call(s, "confirmation_ledger", {})
    if "_error" in led:
        raise SystemExit(f"confirmation_ledger failed: {led['_error']}")
    claims = led.get("claims", [])
    print(f"\n[TIME · BL-106] {len(claims)} claims with built artifacts")
    print(f"  stale                 {led.get('stale_verification')}")
    print(f"  unknown (undated)     {led.get('unknown_verification_freshness')}")
    print(f"  current               "
          f"{sum(1 for c in claims if c.get('verification_freshness') == 'current')}")
    for c in claims:
        if c.get("verification_freshness") == "stale":
            print(f"    STALE  checked {c.get('last_verified_at')} · "
                  f"accepted {c.get('last_claim_at')}  {c['capability_id']}")

    # ---- INPUT and INDEPENDENCE --------------------------------------------
    ev = call(s, "evidence_report", {})
    if "_error" in ev:
        raise SystemExit(f"evidence_report failed: {ev['_error']}")
    caps = ev.get("capabilities", [])
    print(f"\n[INPUT · BL-126] {len(caps)} capabilities with passing evidence")
    print(f"  narrowly proven       {ev.get('narrowly_proven')}")
    print(f"  with unscoped checks  {ev.get('with_unscoped_checks')}")
    unscoped = sum(c.get("unscoped_checks", 0) for c in caps)
    print(f"  unscoped checks total {unscoped}")
    for c in caps:
        if c.get("pinned_everywhere"):
            print(f"    NARROW  pinned={c['pinned_everywhere']}  {c['capability_id']}")

    print(f"\n[INDEPENDENCE · BL-136]")
    print(f"  not independently verified  {ev.get('not_independently_verified')}")
    consumed = sum(len(c.get("consumed_checks", [])) for c in caps)
    print(f"  consumed checks total       {consumed}")
    for c in caps:
        for cc in c.get("consumed_checks", []):
            print(f"    CONSUMED  {cc['verification_id']} -> {c['capability_id']}")

    # The honest reading. Zero narrow and zero consumed on THIS graph is the
    # expected result and not a null one: reflow2's design carries no fitted
    # values and no check has ever stated its input scope, because until now
    # there was nowhere to say either. What the instrument must show is that the
    # UNSCOPED count is large — the silence the family exists to make visible.
    print(f"\n[reading] {unscoped} of this design's passing checks state no input "
          f"scope at all.")
    print("  That silence is the finding. It is not evidence of breadth, and "
          "before this\n  change there was no way to tell the difference.")


if __name__ == "__main__":
    main()
