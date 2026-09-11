#!/usr/bin/env python3
"""A missing required argument names the tool and what it wants — checked ON THE WIRE.

🛑 THE REASON THIS GATE IS END-TO-END AND NOT A UNIT TEST.

reflow2 has intercepted `unknown field` deserialisation refusals in
`ReflowService::call_tool` since v0.51.0, to add one sentence about a client
whose tool list predates the server. On 2026-09-11 that branch was measured
against a real binary for the first time and **it had never fired**: rmcp 3
returns a deserialisation failure as `Ok(CallToolResponse::Complete)` carrying
`isError: true`, not as `Err`, so the `Err` arm reached nothing on the wire.
The test behind it called `stale_client_hint` as a pure function, so it passed
every day while the feature did nothing.

That is the whole argument for this file. A unit test proves the SENTENCE; only
a server proves the CALLER RECEIVES IT.

WHAT IS CHECKED, for every served tool declaring at least one required
parameter (139 of 180, across 230 required parameters, at the time of writing):
call it with `{}` and require that the refusal

  · names the TOOL — a caller with several in flight cannot otherwise tell
    which one refused;
  · still names the missing FIELD;
  · is not the bare deserialiser string, which names neither.

Plus one `unknown field` probe, so the older interception can never go dead
again the way it did.

    python3 tools/refusal_speaks.py
    python3 tools/refusal_speaks.py --bin target/release/reflow2-mcp
"""
from __future__ import annotations
import argparse, json, pathlib, shutil, sys, tempfile
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from smoke_mcp import Server

BARE = "failed to deserialize parameters"

# Tools that cannot be probed with `{}` because the empty call is LEGAL for
# them — their required field has a schema default, or the router answers
# before deserialising. An entry is a claim that `{}` is not a missing-argument
# call, never that the obligation does not apply.
EXEMPT: dict[str, str] = {}


def refusal_text(res: dict) -> str | None:
    """The refusal a caller actually sees, or None if the call was not refused."""
    if not res.get("isError"):
        return None
    blocks = res.get("content") or []
    return (blocks[0].get("text", "") if blocks else "") or ""


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--bin", default="target/debug/reflow2-mcp")
    a = ap.parse_args()

    tmp = pathlib.Path(tempfile.mkdtemp(prefix="refusal-speaks-"))
    try:
        s = Server(a.bin, str(tmp / "graph"))
        try:
            tools = s.rpc("tools/list", {})["result"]["tools"]
            required_tools = [
                t for t in tools
                if (t.get("inputSchema") or {}).get("required")
                and t["name"] not in EXEMPT
            ]

            failures: list[str] = []
            probed = 0
            for t in required_tools:
                name = t["name"]
                res = s.rpc("tools/call", {"name": name, "arguments": {}})["result"]
                text = refusal_text(res)
                if text is None:
                    # Not refused at all: `{}` was a legal call. That is a fact
                    # about the tool, not a failure of this contract — but it
                    # must be DECLARED, or the gate silently shrinks.
                    failures.append(
                        f"{name}: called with {{}} and was NOT refused, though its schema "
                        f"declares required {(t['inputSchema'] or {}).get('required')}. "
                        f"Either the schema is wrong or this belongs in EXEMPT with the reason."
                    )
                    continue
                probed += 1
                if text.startswith(BARE):
                    failures.append(f"{name}: bare deserialiser string — {text[:120]!r}")
                elif name not in text:
                    failures.append(f"{name}: refusal does not name the tool — {text[:120]!r}")

            # The interception that was dead for months. Any tool with an
            # optional-only surface will do; loop_status is stable and cheap.
            res = s.rpc(
                "tools/call",
                {"name": "loop_status", "arguments": {"zz_no_such_field": 1}},
            )["result"]
            text = refusal_text(res) or ""
            if "Reconnect" not in text:
                failures.append(
                    "loop_status: an UNKNOWN-field refusal no longer carries the "
                    f"stale-client sentence — {text[:160]!r}. This interception was dead "
                    "from v0.51.0 to 2026-09-11 because nothing asked a server."
                )
        finally:
            s.close()
    finally:
        shutil.rmtree(tmp, ignore_errors=True)

    print(f"  probed {probed} tool(s) declaring a required argument")
    if failures:
        print(f"  FAIL  {len(failures)} refusal(s) do not name the tool and what it wants:")
        for f in failures:
            print(f"          {f}")
        return 1
    print("  PASS  every missing-argument refusal names the tool, the field, and its purpose")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
