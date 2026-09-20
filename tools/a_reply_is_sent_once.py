#!/usr/bin/env python3
"""Every JSON reply carries its payload ONCE, on every tool, not just on one.

⭐ WHY THIS GATE EXISTS. `tests/reply_is_sent_once.rs` has pinned this invariant
since 2026-08-23 — and it pins it on `detect_gaps`. One tool out of 191, in a
crate that enumerates the served surface for a dozen other properties
(`every_required_parameter_is_described`, `one_tool_declares_its_output_contract`,
`every_constructor_can_say_what_the_thing_is`, and more).

MEASURED 2026-09-20 against a real binary, three handshake names, one scratch
graph each: SIX tools were still sending the payload twice, byte for byte, to
every client — get_instructions (64,120 bytes on the wire for 31,374 of
payload), list_skills, get_skill, find_skills, usage_report, design_identity.
A private helper in `tools/skills_tools.rs` built a second reply shape, its
doc-comment asserting "Same shape every other tool returns". That was true when
it was written and stopped being true when `json_result` became a signpost, and
nothing read it for 28 days.

🛑 THE COST IS NOT ONLY BYTES. `content_policy::apply` rewrites a reply only if
it is ALREADY in signpost shape, so the per-client rule was inert on exactly
those six: OpenCode's empty-block fallback — the one it fills from
structuredContent itself — never fired on the tools carrying the instructions
and the skills. Grok was served correctly by accident.

⚠️ AND A DETECTOR WITH NOTHING TO RUN ON READS EXACTLY LIKE ONE THAT RAN CLEAN.
On an empty graph most replies are trivial and a shape check still passes while
checking almost nothing, so this seeds a real design from the committed export
and STATES how many replies it actually measured. Zero measured is a FAILURE,
not a pass.

Usage (from the repo root, after `cargo build -p reflow2-mcp`):

    python3 tools/a_reply_is_sent_once.py
    python3 tools/a_reply_is_sent_once.py --bin target/release/reflow2-mcp

Exits 0 when every structured reply carries one signpost and one payload,
1 on any duplicate, any silence, or on having had nothing to measure.
Standard library only.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import sys
import tempfile

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from smoke_mcp import Server  # noqa: E402  (path set above)

# Tools that answer with a DOCUMENT rather than a record: the text block is the
# answer and there is no structured half to point at. They are recognised on the
# wire by the absence of `structuredContent`, so this list is documentation
# rather than control flow — it exists so a reader knows the shape is expected.
DOCUMENT_SHAPED = {
    "graph_report_markdown": "Markdown has no structure to declare (BL-48).",
}

# Two of the six tools that drifted require an argument, so a no-argument sweep
# cannot reach them. Named here with a cheap, real value each rather than left
# unmeasured — the point of the gate is the class, and half a class is how the
# drift survived in the first place.
EXTRA_CALLS = {
    "get_skill": {"name": "capture-intent"},
    "find_skills": {"query": "record what the user just decided"},
}

# Tools that must not be fired blind in a gate: they mutate the store, spawn
# work, or take minutes. Shape is checked on the ~40 reads instead.
SKIP = {"genesis", "import_graph", "export_graph", "design_regions"}


def payload_is_duplicated(text: str, structured) -> bool:
    """Is the text block just the payload again, in any serialization?"""
    stripped = text.strip()
    if not stripped.startswith(("{", "[")):
        return False
    try:
        return json.loads(stripped) == structured
    except (ValueError, TypeError):
        # It opens like JSON but does not parse, or does not compare. Either way
        # a text block that begins with a brace is not a sentence.
        return True


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--bin", default="target/debug/reflow2-mcp")
    ap.add_argument("--export", default="docs/design/reflow2.json")
    a = ap.parse_args()

    export = pathlib.Path(a.export)
    if not export.exists():
        print(f"FAIL: no export at {export} to seed a realistic design from.", file=sys.stderr)
        print("      Without it this gate would check an empty graph and pass vacuously.", file=sys.stderr)
        return 1

    tmp = pathlib.Path(tempfile.mkdtemp(prefix="reply-sent-once-"))
    failures: list[str] = []
    rows: list[tuple[str, str]] = []
    signposts: dict[str, list[str]] = {}
    measured = 0

    s = Server(a.bin, str(tmp / "graph"))
    try:
        # ① Seed. Without this the whole gate is theatre.
        s.call("import_graph", {"path": str(export.resolve())})

        tools = s.rpc("tools/list", {})["result"]["tools"]
        calls: list[tuple[str, dict]] = [
            (t["name"], {})
            for t in tools
            if not (t.get("inputSchema") or {}).get("required") and t["name"] not in SKIP
        ]
        served = {t["name"] for t in tools}
        for name, args in EXTRA_CALLS.items():
            if name in served:
                calls.append((name, args))
            else:
                failures.append(f"{name}: named in EXTRA_CALLS but this build does not serve it.")

        # ② Every reply, one rule.
        for name, args in sorted(calls):
            # The RAW result, not Server.call's unwrapped payload: this gate is
            # about the envelope, which call() is designed to hide.
            try:
                resp = s.rpc("tools/call", {"name": name, "arguments": args})
            except Exception as e:  # a tool that refuses is not this gate's business
                rows.append((name, f"(skipped: {type(e).__name__})"))
                continue
            if "error" in resp:
                rows.append((name, "(skipped: JSON-RPC error)"))
                continue
            result = resp["result"]
            if result.get("isError"):
                rows.append((name, "(skipped: refusal)"))
                continue

            structured = result.get("structuredContent")
            blocks = result.get("content") or []
            text = blocks[0].get("text", "") if blocks else ""

            if structured is None:
                rows.append((name, "document-shaped: " + DOCUMENT_SHAPED.get(name, "no structured half")))
                continue

            measured += 1

            if payload_is_duplicated(text, structured):
                failures.append(
                    f"{name}: the text block carries the payload AGAIN "
                    f"({len(text)} chars beside {len(json.dumps(structured))} of structuredContent). "
                    f"Build the reply with json_result, not a private helper."
                )
                rows.append((name, "DUPLICATE"))
                continue
            if not text:
                failures.append(
                    f"{name}: an EMPTY content block is the silent failure — a client reading the "
                    f"wrong field cannot tell reflow2 apart from reflow2 not being there."
                )
                rows.append((name, "SILENT"))
                continue
            if "structuredContent" not in text:
                failures.append(f"{name}: the text block must NAME the field to read: {text[:120]!r}")
                rows.append((name, "UNSIGNPOSTED"))
                continue
            # NOT ASSERTED: that the signpost is SMALLER than what it points at.
            # `reply_is_sent_once.rs` asserts that, correctly, on detect_gaps —
            # a payload of tens of thousands of characters. Asked of the whole
            # surface it is simply false: the signpost is one fixed sentence,
            # and `mirrors` answers in 132 characters. Seven tools failed on it
            # here with nothing wrong, and the only way to keep it would be a
            # size threshold — which this project has already ruled out twice
            # ("a loud detector needs a different QUESTION, not a tuned
            # number"). The question that survives generalisation is whether the
            # payload was sent TWICE, and that is asked directly above.

            signposts.setdefault(text, []).append(name)
            rows.append((name, "signpost"))
    finally:
        s.proc.terminate()

    # ③ ONE signpost, not many. Several near-identical sentences is the same
    #    second-builder drift arriving a different way.
    if len(signposts) > 1:
        shapes = "; ".join(
            f"{len(names)} tool(s) say {text[:60]!r}" for text, names in signposts.items()
        )
        failures.append(f"the surface carries {len(signposts)} different signposts, not one: {shapes}")

    for name, verdict in rows:
        if verdict not in ("signpost",):
            print(f"  {verdict:<20} {name}")
    print(f"\nmeasured {measured} structured reply(ies) across {len(rows)} tool call(s)")

    # ④ Nothing measured is a failure, never a pass.
    if measured == 0:
        print("FAIL: nothing was measured, so this gate checked nothing.", file=sys.stderr)
        return 1

    if failures:
        print(f"\nFAIL: a reply is sent once — {len(failures)} finding(s):", file=sys.stderr)
        for f in failures:
            print(f"  {f}", file=sys.stderr)
        return 1

    print("OK: every structured reply carries one payload and one signpost.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
