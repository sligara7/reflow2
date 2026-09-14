#!/usr/bin/env python3
"""A capability that claims reach beyond this repo is realized by something a consumer gets.

The class this pins (2026-09-14, `fact:root-cause-the-wall-check-was-generalised-in-code-
and-never-in-reach-and-the-checkout-cannot-feel-it`): `tools/wall_check.py` was built here,
generalised in code to "any project, with no configuration", allocated to a served component —
and reached a consumer through no served tool, no skill and no detector. This checkout could
not feel the absence because it has the script. A consumer re-wrote the analysis by hand 25
days later and filed it as "unknown".

THE RULE: a Capability whose own text claims reach beyond this repository — "any project",
"a reflow2 user", "whatever project", "consumer" — must be REALIZED by at least one artifact a
consumer actually receives: the served binary (`crates/`) or the kit (`getting-started/`). One
realized ONLY by scripts under `tools/` is a promise this repo can keep and a consumer cannot
collect. The gate reports the claim sentence beside the realizers so the fix is either to serve
it or to bound the claim — never to delete the capability to go green.

Reads the committed export; exits 1 on any such capability; standard library only.

    python3 tools/check_consumer_reach.py docs/design/reflow2.json
"""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

CLAIM = re.compile(
    r"any project|a reflow2 user|whatever project|on your project|every project|"
    r"consumer(?:'s)? (?:project|install|design|repo)|a consumer",
    re.I,
)
REACHABLE_PREFIXES = ("crates/", "getting-started/")
RELEASE_WORKFLOW = Path(__file__).resolve().parent.parent / ".github" / "workflows" / "release.yml"
KIT_COPY = re.compile(r"cp\s+(tools/\S+)\s+kit-stage/")


def kit_shipped_tools(workflow_text: str | None = None) -> set[str]:
    """The `tools/` files the release workflow copies INTO the kit tarball.

    Read off the delivery contract itself rather than a hand list here, so a
    file added to the kit later is reachable without anyone editing this gate
    — and a file dropped from it stops counting. `reflow2_install.py` is one:
    the release one-liner fetches install.sh, which runs it, so a consumer does
    receive it although it lives under tools/.
    """
    if workflow_text is None:
        try:
            workflow_text = RELEASE_WORKFLOW.read_text(encoding="utf-8")
        except OSError:
            return set()
    return set(KIT_COPY.findall(workflow_text))


def _text(props: dict) -> str:
    return " ".join(str(props.get(k) or "") for k in ("name", "description"))


def check(doc: dict, shipped: set[str] | None = None) -> list[str]:
    shipped = kit_shipped_tools() if shipped is None else shipped
    nodes = {n["node_id"]: n for n in doc.get("nodes", [])}
    realizers: dict[str, list[str]] = {}
    for e in doc.get("edges", []):
        if e.get("edge_type") == "REALIZES":
            realizers.setdefault(e["to_id"], []).append(e["from_id"])
    failures: list[str] = []
    for nid, n in nodes.items():
        if n.get("node_type") != "Capability":
            continue
        props = n.get("properties") or {}
        if props.get("status") not in ("realized", "verified"):
            continue
        m = CLAIM.search(_text(props))
        if not m:
            continue
        locs = [
            (nodes.get(a, {}).get("properties") or {}).get("location") or ""
            for a in realizers.get(nid, [])
        ]
        if not locs:
            continue  # nothing realizes it at all — that is a different gap, and detect_gaps has it
        if any(loc.startswith(REACHABLE_PREFIXES) or loc in shipped for loc in locs):
            continue
        if all(loc.startswith("tools/") for loc in locs):
            failures.append(
                f"{nid}: claims reach beyond this repo ({m.group(0)!r}) but is realized only by "
                f"{', '.join(sorted(locs))} — a consumer never receives those. Serve it, or bound "
                f"the claim."
            )
    return failures


def main() -> int:
    path = sys.argv[1] if len(sys.argv) > 1 else "docs/design/reflow2.json"
    with open(path, encoding="utf-8") as fh:
        doc = json.load(fh)
    failures = check(doc)
    for f in failures:
        print(f"  FAIL  {f}")
    if failures:
        print(f"consumer reach: FAILED — {len(failures)} capabilit(y/ies) promise what a consumer cannot get.")
        return 1
    print("consumer reach: OK — every capability that claims reach beyond this repo is realized by something a consumer receives.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
