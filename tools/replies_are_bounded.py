#!/usr/bin/env python3
"""Every read tool whose reply outgrows the budget must offer a way to bound it.

⭐ WHY THIS GATE EXISTS. Measured 2026-09-13 against reflow2's own 4,403-node
design (`fact:the-unbounded-reply-sweep-2026-09-13`): of 181 served tools, THREE
accepted `budget_chars`, and nine others returned more than the 30,000-character
default with no bound at all — the worst of them by 490x. Each of the three had
been fixed by hand, separately, after somebody personally met that tool's
overflow. Three fixed one at a time against 178 not fixed is the
instance-not-class pattern this project already records, and toolsnaps cannot
catch it: a golden pins the tools that EXIST, so a NEW tool that overflows with
no bound is a clean diff and a green build.

🛑 THE FAILURE IS NOT A SLOW CALL. It is the CLIENT refusing the payload, at
which point the session sees a wall of harness text and reflow2 never gets to
say "narrow your question". A reader who receives nothing cannot be told
anything.

⚠️ AND THE REASON THIS SCRIPT IS NOISIER THAN IT LOOKS: A DETECTOR THAT HAD
NOTHING TO RUN ON READS EXACTLY LIKE ONE THAT RAN CLEAN. On an empty graph every
reply is tiny, nothing exceeds any budget, and this gate would pass while
checking nothing whatsoever. So it SEEDS a real design from the committed export
and then states, in its own output, how many replies were actually large enough
to test. If that number is zero the gate FAILS rather than passes, because
"no tool overflowed" and "I could not make any tool overflow" are different
facts and must never share an exit code.

Usage (from the repo root, after `cargo build -p reflow2-mcp`):

    python3 tools/replies_are_bounded.py
    python3 tools/replies_are_bounded.py --bin target/release/reflow2-mcp
    python3 tools/replies_are_bounded.py --export docs/design/reflow2.json

Exits 0 when every oversized reply came from a tool that declares a bound,
1 on any unbounded overflow — or on having had nothing to measure.
Standard library only.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import shutil
import sys
import tempfile

from smoke_mcp import Server

DEFAULT_BUDGET = 30_000

# Tools whose payload is a DOCUMENT rather than a report. A character budget
# cannot shorten one honestly — half an export, half an instruction set or half
# a published surface reads exactly like a complete one, and nothing in the
# reply would say otherwise. Each of these instead refuses to send an oversized
# whole and names the narrower call that works, which is a bound of a different
# shape. They are listed here so the exemption is a REVIEWED decision rather
# than an absence somebody has to notice.
DOCUMENT_SHAPED = {
    # its job IS the whole document; needs streaming or a path, not a budget
    "export_graph": "returns the design document itself",
    # withholds whole + hands back the section manifest (see skills_tools.rs)
    "get_instructions": "returns an instruction document; bounded via `section`",
    # withholds whole + hands back counts and hash (see exchange.rs)
    "export_surface": "returns a published contract; bounded via `path`",
}

# Calling these would write, block, or take a lock — this gate is read-only.
SKIP = {"import_graph", "mint_seat", "propose_heal", "reconcile_dependencies"}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--bin", default="target/debug/reflow2-mcp")
    ap.add_argument("--export", default="docs/design/reflow2.json")
    ap.add_argument("--budget", type=int, default=DEFAULT_BUDGET)
    a = ap.parse_args()

    export = pathlib.Path(a.export)
    if not export.exists():
        print(f"FAIL: no export at {export} to seed a realistic design from.", file=sys.stderr)
        print("      Without it this gate would check an empty graph and pass vacuously.", file=sys.stderr)
        return 1

    tmp = pathlib.Path(tempfile.mkdtemp(prefix="replies-bounded-"))
    try:
        s = Server(a.bin, str(tmp / "graph"))
        try:
            # ① Seed. Without this the whole gate is theatre.
            res = s.call("import_graph", {"path": str(export.resolve())})
            seeded = json.dumps(res)[:400]

            tools = s.rpc("tools/list", {})["result"]["tools"]
            by_name = {t["name"]: t for t in tools}

            # ② Every tool callable with no arguments is a tool an agent can
            #    fire blind, which is exactly when an overflow is unexpected.
            candidates = [
                t["name"]
                for t in tools
                if not (t.get("inputSchema") or {}).get("required")
                and t["name"] not in SKIP
            ]

            failures: list[str] = []
            oversized = 0
            measured = 0
            rows: list[tuple[int, str, str]] = []

            for name in sorted(candidates):
                try:
                    raw = json.dumps(s.call(name, {}))
                except Exception as e:  # a tool that errors is not this gate's business
                    rows.append((0, name, f"(skipped: {type(e).__name__})"))
                    continue
                measured += 1
                size = len(raw)
                schema = (by_name[name].get("inputSchema") or {}).get("properties") or {}
                declares = "budget_chars" in schema

                if size <= a.budget:
                    rows.append((size, name, "fits"))
                    continue

                oversized += 1
                if name in DOCUMENT_SHAPED:
                    rows.append((size, name, f"document-shaped: {DOCUMENT_SHAPED[name]}"))
                elif declares:
                    rows.append((size, name, "over budget, but BOUNDED"))
                    # A declared bound that does not actually bind is worse than
                    # none, because the schema says the reader is protected.
                    bounded = json.dumps(s.call(name, {"budget_chars": a.budget}))
                    if len(bounded) > size:
                        failures.append(
                            f"{name}: declares budget_chars but passing it made the reply "
                            f"LARGER ({size} -> {len(bounded)})."
                        )
                else:
                    rows.append((size, name, "OVER BUDGET AND UNBOUNDED"))
                    failures.append(
                        f"{name}: {size} characters against a budget of {a.budget}, and its "
                        f"schema declares no `budget_chars`. Add one via reply_budget, or — if "
                        f"its payload is a document a budget cannot honestly shorten — add it "
                        f"to DOCUMENT_SHAPED with the reason."
                    )

            for size, name, verdict in sorted(rows, reverse=True):
                print(f"{size:>9}  {name:<26} {verdict}")

            print()
            print(f"seeded from {export} -> {seeded[:120]}")
            print(f"measured {measured} no-argument read tool(s); {oversized} exceeded {a.budget} chars")

            # ③ THE ANTI-VACUITY CHECK. This is the clause that makes the gate
            #    honest, and it is the one most likely to be deleted by somebody
            #    who thinks a green run means all is well.
            if oversized == 0:
                print(
                    "\nFAIL: NOTHING EXCEEDED THE BUDGET, so this run proved nothing.\n"
                    "      That is not a clean result — it means the seeded design was too\n"
                    "      small to make any reply overflow, and a gate that cannot fail is\n"
                    "      not a gate. Seed from a larger export, or lower --budget until at\n"
                    "      least one tool is actually exercised.",
                    file=sys.stderr,
                )
                return 1

            if failures:
                print("\n=== UNBOUNDED OVERFLOW ===", file=sys.stderr)
                for f in failures:
                    print(f"  · {f}", file=sys.stderr)
                return 1

            print("\nOK: every oversized reply came from a tool that offers a bound.")
            return 0
        finally:
            s.close()
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
