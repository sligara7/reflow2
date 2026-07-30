#!/usr/bin/env python3
"""Does ONE client keep ONE seat? Per transport, per protocol version.

`req:seat-per-client` says who is working is a property of the SESSION, not of
the server process. `req:seat-identity-survives-stateless-mcp` says that
guarantee rests on a transport detail the MCP 2026-07-28 revision deletes.

This is the instrument that turns that from a reading of the changelog into a
measurement. `tools/test_shared_sessions.py` proves the *other* half — that two
different clients get two DIFFERENT seats — and nothing proved the complement,
which is why the whole suite stayed green through the rmcp v3 upgrade while this
was already broken. A tick is only as wide as the case its evidence exercises.

IT MEASURES TWO THINGS, which are the two halves of `dec:stateless-seat-handle`.

1. MINT-AND-CARRY HOLDS. `mint_seat` once, then two `claim_region` calls under two
   contributors carrying that handle, then `claim_report`: how many DISTINCT seats
   did one client produce? One is correct. More than one means seat identity is
   gone, and with it `cap:claim-liveness` (a claim's owner becomes whoever called
   last), the stale-seat refusal (`req:stale-seat-knows` firing against your own
   previous write) and `claim_report` itself (one session reported as N owners).

2. THE BACKSTOP IS LOUD. A claim with NO seat must be REFUSED on a sessionless
   transport and SERVED on one with a session — and the refusal must name
   `mint_seat`, or it is a no without a remedy (rule 4). Silently minting a
   per-request seat is the failure this exists to prevent: it reports success
   while recording an owner that changes under the caller.

WHAT IT FINDS (green as of 2026-07-30, when the fix landed):

  stdio                      1 seat · no-seat claim SERVED    (session supplies it)
  http 2025-06-18 (legacy)   1 seat · no-seat claim SERVED    (Mcp-Session-Id)
  http 2026-07-28            1 seat · no-seat claim REFUSED   (handler per REQUEST)

Note what the third row means: the sessionless transport WORKS, it just requires
the handle. No config flag could have avoided that —
`StreamableHttpServerConfig::legacy_session_mode` is documented to apply only to
protocol versions < 2026-07-28, and "requests negotiating that version are always
served statelessly regardless of this setting". The CLIENT chooses the version, so
reflow2 could only decide what identity means once the choice was made.

Exits non-zero if any transport gives one client more than one seat, if a no-seat
claim is answered where it should be refused, if one is refused where the session
could have supplied a seat, or if the refusal fails to say what would have worked.
It began as a docs/sharpening.md baseline failing on purpose and is now a CI gate
in the `full` job — an invariant nobody enforces is one that rots.

stdlib only; skips cleanly when the binary is absent.
"""

from __future__ import annotations

import json
import os
import pathlib
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request

REPO = pathlib.Path(__file__).resolve().parent.parent
BINARY = REPO / "target" / "debug" / "reflow2-mcp"

# The revision that removes protocol-level sessions (SEP-2567) and the
# initialize handshake (SEP-2575), and from which the SEP-2243 standard headers
# become required.
STATELESS = "2026-07-28"
# A version that still has sessions, as the control: the same probe, the same
# tools, the only variable being what the client negotiated.
LEGACY = "2025-06-18"


def free_port() -> int:
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def start_http(graph: pathlib.Path, port: int) -> subprocess.Popen:
    server = subprocess.Popen(
        [str(BINARY), "--graph-path", str(graph), "--http", f"127.0.0.1:{port}"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env={**os.environ, "RUST_LOG": "error"},
    )
    deadline = time.time() + 60
    while time.time() < deadline:
        try:
            with socket.create_connection(("127.0.0.1", port), timeout=1):
                return server
        except OSError:
            if server.poll() is not None:
                raise SystemExit(f"server exited: {server.stderr.read()}")
            time.sleep(0.2)
    raise SystemExit(f"server never came up on 127.0.0.1:{port}")


def _parse(raw: str):
    """A streamable-HTTP reply is either JSON or SSE frames carrying it."""
    message = None
    for line in raw.splitlines():
        if line.startswith("data:") and line[5:].strip():
            try:
                message = json.loads(line[5:].strip())
            except json.JSONDecodeError:
                pass
    if message is None and raw.strip():
        try:
            message = json.loads(raw)
        except json.JSONDecodeError:
            message = {"unparsed": raw[:400]}
    return message


class StdioClient:
    """The transport Claude Code and grok build actually use."""

    label = "stdio"

    def __init__(self, graph: pathlib.Path):
        self.proc = subprocess.Popen(
            [str(BINARY), "--graph-path", str(graph)],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            env={**os.environ, "RUST_LOG": "error"},
        )
        self.id = 0
        self._rpc("initialize", {
            "protocolVersion": LEGACY,
            "capabilities": {},
            "clientInfo": {"name": "seat-probe", "version": "1"},
        })
        self._notify("notifications/initialized")

    def _rpc(self, method: str, params: dict):
        self.id += 1
        self.proc.stdin.write(
            json.dumps({"jsonrpc": "2.0", "id": self.id, "method": method, "params": params}) + "\n"
        )
        self.proc.stdin.flush()
        line = self.proc.stdout.readline()
        return json.loads(line) if line.strip() else None

    def _notify(self, method: str):
        self.proc.stdin.write(json.dumps({"jsonrpc": "2.0", "method": method}) + "\n")
        self.proc.stdin.flush()

    def call(self, tool: str, **args):
        return self._rpc("tools/call", {"name": tool, "arguments": args})

    def close(self):
        self.proc.terminate()
        self.proc.wait(timeout=10)
        for pipe in (self.proc.stdin, self.proc.stdout, self.proc.stderr):
            pipe.close()


class HttpClient:
    """One client over streamable HTTP, at whichever protocol version it names.

    At LEGACY it does the initialize handshake and carries Mcp-Session-Id. At
    STATELESS there is no handshake at all: the version travels in the
    MCP-Protocol-Version header and in each request's `_meta`, and the SEP-2243
    Mcp-Method / Mcp-Name headers must mirror the body or the transport answers
    -32020 before the handler is reached.
    """

    def __init__(self, url: str, version: str):
        self.url = url
        self.version = version
        self.stateless = version >= STATELESS
        self.session = None
        self.id = 0
        self.label = f"http {version}" + (" (stateless)" if self.stateless else " (legacy)")
        if not self.stateless:
            message, self.session = self._post({
                "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": {"protocolVersion": version, "capabilities": {},
                           "clientInfo": {"name": "seat-probe", "version": "1"}},
            })
            if not (message and "result" in message):
                raise SystemExit(f"handshake failed: {message}")
            self.id = 1
            self._post({"jsonrpc": "2.0", "method": "notifications/initialized"})

    def _headers(self, payload: dict) -> dict:
        headers = {
            "Content-Type": "application/json",
            "Accept": "application/json, text/event-stream",
        }
        if self.session:
            headers["Mcp-Session-Id"] = self.session
        if self.stateless:
            headers["MCP-Protocol-Version"] = self.version
            method = payload.get("method")
            if method:
                headers["Mcp-Method"] = method
            name = (payload.get("params") or {}).get("name")
            if name:
                headers["Mcp-Name"] = name
        return headers

    def _post(self, payload: dict):
        request = urllib.request.Request(
            self.url, data=json.dumps(payload).encode(), headers=self._headers(payload)
        )
        try:
            with urllib.request.urlopen(request, timeout=60) as response:
                return _parse(response.read().decode()), response.headers.get("Mcp-Session-Id")
        except urllib.error.HTTPError as e:
            return {"http_error": e.code, "body": e.read().decode()[:400]}, None

    def call(self, tool: str, **args):
        self.id += 1
        payload = {
            "jsonrpc": "2.0", "id": self.id, "method": "tools/call",
            "params": {"name": tool, "arguments": args},
        }
        if self.stateless:
            payload["params"]["_meta"] = {
                "io.modelcontextprotocol/protocolVersion": self.version,
                "io.modelcontextprotocol/clientCapabilities": {},
            }
        return self._post(payload)[0]

    def close(self):
        pass


def structured(message):
    if not message or "result" not in message:
        return {"__raw__": message}
    return message["result"].get("structuredContent", message["result"])


def seats_of(client) -> tuple[dict, str | None]:
    """Two claims from ONE client, minting a seat first and carrying it.

    Mint-and-carry is what an agent is supposed to do (`dec:stateless-seat-handle`):
    call `mint_seat` once, keep the handle, pass it on every claim. It is correct on
    EVERY transport -- in a session the supplied seat simply wins over the one the
    session would have provided -- which is why the probe does the same thing on all
    three cases instead of special-casing the sessionless one. If this ever reports
    more than one distinct seat, mint-and-carry has stopped working.

    Returns {contributor: seat} and any failure.
    """
    seeded = structured(client.call("add_project", id="proj:seat-probe", name="Seat probe"))
    if "__raw__" in seeded:
        return {}, f"could not seed the graph: {json.dumps(seeded)[:300]}"

    minted = structured(client.call("mint_seat"))
    seat = minted.get("seat") if isinstance(minted, dict) else None
    if not seat:
        return {}, f"mint_seat did not return a seat: {json.dumps(minted)[:300]}"

    for contributor in ("probe-one", "probe-two"):
        client.call("add_contributor", id=contributor, name=contributor)
        claimed = structured(client.call(
            "claim_region", contributor_id=contributor, seed_id="proj:seat-probe",
            depth=1, at="2026-07-30", seat=seat,
        ))
        if "__raw__" in claimed:
            return {}, f"claim_region failed: {json.dumps(claimed)[:300]}"
    report = structured(client.call("claim_report"))
    claims = report.get("claims", []) if isinstance(report, dict) else []
    return {c.get("contributor_id"): c.get("seat") for c in claims}, None


def refusal_of(client) -> tuple[bool, str]:
    """Claim with NO seat, and report whether it was refused and how usefully.

    The backstop half of the decision. On a sessionless transport, omitting the
    seat must FAIL and say what would have worked -- because the alternative,
    minting one per request, succeeds while recording an owner that changes under
    the caller. On a session it must SUCCEED, because there the service's own seat
    genuinely identifies the client and always has.

    Returns (refused, what it said).
    """
    client.call("add_contributor", id="probe-bare", name="probe-bare")
    result = client.call(
        "claim_region", contributor_id="probe-bare", seed_id="proj:seat-probe",
        depth=1, at="2026-07-30",
    )
    # Kept WHOLE, not truncated: the assertion on this string is that it names the
    # remedy, and the remedy is at the end of a deliberately explanatory message.
    # Truncating here reported a real refusal as a rule-4 violation.
    if isinstance(result, dict) and "error" in result:
        return True, str(result["error"].get("message", ""))
    if isinstance(result, dict) and "http_error" in result:
        return True, f"HTTP {result['http_error']}: {result.get('body', '')}"
    payload = structured(result)
    # A tool-level refusal comes back as isError with the reason in `content`.
    if isinstance(payload, dict) and payload.get("isError"):
        blocks = payload.get("content") or []
        said = " ".join(b.get("text", "") for b in blocks if isinstance(b, dict))
        return True, said
    return False, json.dumps(payload)[:200]


def main() -> int:
    if not BINARY.exists():
        print(f"SKIP: {BINARY} not built (cargo build -p reflow2-mcp)")
        return 0

    root = pathlib.Path(tempfile.mkdtemp(prefix="reflow2-seat-probe-"))
    rows, failures = [], []

    cases = [("stdio", None), ("legacy", LEGACY), ("stateless", STATELESS)]
    for name, version in cases:
        graph = root / name
        server = None
        try:
            if version is None:
                client = StdioClient(graph)
            else:
                port = free_port()
                server = start_http(graph, port)
                client = HttpClient(f"http://127.0.0.1:{port}/", version)
            seats, problem = seats_of(client)
            # Both halves of the decision are measured on the SAME live client,
            # so the refusal check cannot accidentally be answered by a fresh
            # session that would have had a valid seat anyway.
            refused, said = (None, "") if problem else refusal_of(client)
            client.close()
        finally:
            if server is not None:
                server.terminate()
                server.wait(timeout=10)
                server.stderr.close()
                server.stdout.close()

        if problem:
            print(f"  ERROR  {client.label}: {problem}")
            failures.append(f"{client.label}: {problem}")
            continue

        distinct = sorted({s for s in seats.values() if s})
        rows.append((client.label, len(seats), distinct))
        verdict = "ok" if len(distinct) == 1 else "SEAT IDENTITY LOST"
        print(f"  {verdict:>18}  {client.label}: "
              f"{len(seats)} claim(s) from one client -> {len(distinct)} distinct seat(s)")
        for seat in distinct:
            print(f"                      {seat}")
        if len(distinct) != 1:
            failures.append(
                f"{client.label}: one client produced {len(distinct)} seats {distinct}"
            )

        # The backstop: omitting the seat must be refused exactly where a
        # per-request seat would otherwise be minted silently, and allowed
        # exactly where the session's own seat is genuinely the client's.
        should_refuse = version is not None and version >= STATELESS
        if refused == should_refuse:
            if refused:
                names_the_fix = "mint_seat" in said
                print(f"                  ok  {client.label}: a claim with NO seat is refused"
                      f"{'' if names_the_fix else ' — BUT DOES NOT NAME mint_seat'}")
                if not names_the_fix:
                    failures.append(
                        f"{client.label}: the refusal does not say what would have worked "
                        f"(rule 4): {said}"
                    )
            else:
                print(f"                  ok  {client.label}: a claim with no seat is served "
                      f"from the session's own seat, as it always was")
        elif should_refuse:
            print(f"     SILENT FALLBACK  {client.label}: a claim with NO seat SUCCEEDED on a "
                  f"sessionless transport -> {said}")
            failures.append(
                f"{client.label}: omitting the seat was answered rather than refused, so the "
                f"claim's owner changes per request while the call reports success"
            )
        else:
            print(f"        OVER-REFUSAL  {client.label}: a claim with no seat was REFUSED on a "
                  f"transport that has a session -> {said}")
            failures.append(
                f"{client.label}: refused a claim on a transport where the session's own seat "
                f"is valid — this breaks callers that never needed a seat"
            )

    print("\n" + "=" * 62)
    if failures:
        print("SEAT IDENTITY DOES NOT SURVIVE EVERY SUPPORTED TRANSPORT:")
        for f in failures:
            print(f"  - {f}")
        print("\nreq:seat-identity-survives-stateless-mcp and dec:stateless-seat-handle are")
        print("what this measures: mint_seat once, carry the handle, and a claim with no")
        print("seat is refused where a per-request one would otherwise be minted silently.")
        print("=" * 62)
        return 1
    print("ONE CLIENT, ONE SEAT on every supported transport — and a claim with no")
    print("seat is refused exactly where the session cannot supply one.")
    print("=" * 62)
    return 0


if __name__ == "__main__":
    sys.exit(main())
