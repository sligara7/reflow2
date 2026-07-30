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

WHAT IT MEASURES. One client, two `claim_region` calls under two contributors,
then `claim_report`: how many DISTINCT seats did one client's requests produce?
One is correct. More than one means seat identity is gone, and with it
`cap:claim-liveness` (a claim's owner is whoever last called), the stale-seat
refusal (`req:stale-seat-knows` fires against your own previous write) and
`claim_report` itself (one session reported as N owners).

WHAT IT FINDS TODAY, and it is a baseline failing on purpose in the sense of
docs/sharpening.md — NOT wired into CI as a pass/fail gate:

  stdio                      1 seat   — one process, one service, unaffected
  http 2025-06-18 (legacy)   1 seat   — Mcp-Session-Id, service per session
  http 2026-07-28            N seats  — sessionless: rmcp builds a handler per
                                        REQUEST, so ReflowService::share mints a
                                        fresh seat on every call

The 2026-07-28 row is not a bug in rmcp and not something a config flag fixes:
`StreamableHttpServerConfig::legacy_session_mode` is documented to apply only to
protocol versions < 2026-07-28, and "requests negotiating that version are always
served statelessly regardless of this setting". The CLIENT chooses, so reflow2
cannot decline. The prescribed replacement is a server-minted handle passed as an
ordinary tool argument -- which `ClaimReq.seat` already accepts. What is undecided
is how a client OBTAINS and CARRIES that handle without every tool call growing a
parameter no human should have to think about; that is the open decision.

Exits non-zero while any supported transport gives one client more than one seat.
When the fix lands this goes green, and it is then worth promoting to a gate.

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
    """Two claims from ONE client. Returns {contributor: seat} and any failure."""
    seeded = structured(client.call("add_project", id="proj:seat-probe", name="Seat probe"))
    if "__raw__" in seeded:
        return {}, f"could not seed the graph: {json.dumps(seeded)[:300]}"
    for contributor in ("probe-one", "probe-two"):
        client.call("add_contributor", id=contributor, name=contributor)
        claimed = structured(client.call(
            "claim_region", contributor_id=contributor, seed_id="proj:seat-probe",
            depth=1, at="2026-07-30",
        ))
        if "__raw__" in claimed:
            return {}, f"claim_region failed: {json.dumps(claimed)[:300]}"
    report = structured(client.call("claim_report"))
    claims = report.get("claims", []) if isinstance(report, dict) else []
    return {c.get("contributor_id"): c.get("seat") for c in claims}, None


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

    print("\n" + "=" * 62)
    if failures:
        print("SEAT IDENTITY DOES NOT SURVIVE EVERY SUPPORTED TRANSPORT:")
        for f in failures:
            print(f"  - {f}")
        print("\nreq:seat-identity-survives-stateless-mcp is the open requirement. The")
        print("prescribed fix is a server-minted handle passed as an ordinary tool")
        print("argument; ClaimReq.seat already accepts one. What is undecided is how a")
        print("client obtains and carries it. Baseline failing on purpose (sharpening.md);")
        print("NOT a CI gate until the fix lands.")
        print("=" * 62)
        return 1
    print("ONE CLIENT, ONE SEAT on every supported transport.")
    print("=" * 62)
    return 0


if __name__ == "__main__":
    sys.exit(main())
