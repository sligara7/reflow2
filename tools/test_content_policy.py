#!/usr/bin/env python3
"""A reply takes the shape its client can read — measured on the wire, real binary.

WHY THE WIRE AND NOT A UNIT TEST. The policy is keyed on the client's name from
`clientInfo`, which only exists on a real handshake, and it is applied in the
server's `call_tool`, which only a real `tools/call` passes through. The unit
tests in content_policy.rs prove the reshaping; this proves the KEYING — that a
client calling itself `grok-shell-reflow2` gets the payload in the text block
while `claude-code` still gets the one-sentence signpost, from one binary with
no flag. Measured 2026-09-15 on Anthony's own Grok Build session: four
structured tools reached the model as the signpost and nothing else.
"""
from __future__ import annotations

import json
import os
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from smoke_mcp import Server  # noqa: E402

BIN = os.environ.get("REFLOW2_MCP", "target/debug/reflow2-mcp")
SIGNPOST_HEAD = "This reply's payload is in `structuredContent`."

failures: list[str] = []


def check(label: str, ok: bool, detail: str = "") -> None:
    print(("  ok    " if ok else "  FAIL  ") + label + (f" — {detail}" if detail and not ok else ""))
    if not ok:
        failures.append(label)


def raw_result(client: str, tool: str, args: dict | None = None, extra=()) -> dict:
    """One fresh server, one handshake under `client`, one call; the raw result."""
    with tempfile.TemporaryDirectory() as d:
        s = Server(BIN, os.path.join(d, "graph"), client_name=client, extra_args=tuple(extra))
        try:
            resp = s.rpc("tools/call", {"name": tool, "arguments": args or {}})
        finally:
            s.close()
    if "error" in resp:
        raise SystemExit(f"{tool} as {client}: JSON-RPC error {resp['error']}")
    return resp["result"]


def text_of(result: dict) -> str | None:
    blocks = result.get("content") or []
    if len(blocks) != 1 or blocks[0].get("type") != "text":
        return None
    return blocks[0]["text"]


def main() -> int:
    if not os.path.exists(BIN):
        print(f"no binary at {BIN}; build with `cargo build -p reflow2-mcp`", file=sys.stderr)
        return 2

    # 1. A client that reads structuredContent gets the signpost — the payload once.
    r = raw_result("claude-code", "open_questions")
    check("claude-code: payload is in structuredContent", r.get("structuredContent") is not None)
    check("claude-code: text block is the signpost", (text_of(r) or "").startswith(SIGNPOST_HEAD), text_of(r))

    # 2. Grok — the client that reported the outage three times — gets the payload
    #    in the text block too, as JSON equal to structuredContent, with no flag.
    r = raw_result("grok-shell-reflow2", "open_questions")
    t = text_of(r)
    parsed = None
    try:
        parsed = json.loads(t) if t is not None else None
    except ValueError:
        parsed = None
    check("grok-shell-reflow2: text block is the payload as JSON", parsed is not None and parsed == r.get("structuredContent"), (t or "")[:120])

    # 3. OpenCode gets an EMPTY text block — measured: it fills the text from
    #    structuredContent only when the text block is empty.
    r = raw_result("opencode", "open_questions")
    check("opencode: no text block", (r.get("content") or []) == [], json.dumps(r.get("content"))[:120])
    check("opencode: structuredContent still present", r.get("structuredContent") is not None)

    # 4. The operator's flag overrides the per-client rule, for a client nobody named.
    r = raw_result("some-unknown-tui", "open_questions", extra=("--content-policy", "duplicate"))
    t = text_of(r)
    try:
        parsed = json.loads(t) if t is not None else None
    except ValueError:
        parsed = None
    check("--content-policy duplicate: an unknown client gets the payload in the text block", parsed is not None and parsed == r.get("structuredContent"))

    r = raw_result("grok-shell-reflow2", "open_questions", extra=("--content-policy", "signpost"))
    check("--content-policy signpost: overrides even grok back to the signpost", (text_of(r) or "").startswith(SIGNPOST_HEAD))

    # 5. Only the signpost shape is rewritten. A prose tool and a refusal reach
    #    grok exactly as they reach everyone.
    r = raw_result("grok-shell-reflow2", "graph_report_markdown")
    check("grok: a prose tool's text is untouched", (text_of(r) or "").lstrip().startswith("#"), (text_of(r) or "")[:80])
    r = raw_result("grok-shell-reflow2", "get_node", {"node_type": "Decision"})
    check("grok: a refusal still arrives in full as text", r.get("isError") is True and "id" in (text_of(r) or ""), (text_of(r) or "")[:120])

    print()
    if failures:
        print(f"content policy: FAILED — {len(failures)} check(s): " + "; ".join(failures))
        return 1
    print("content policy: OK — the reply takes the shape its client can read")
    return 0


if __name__ == "__main__":
    sys.exit(main())
