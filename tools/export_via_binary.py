#!/usr/bin/env python3
"""Export the design THROUGH A CHOSEN BINARY, so the stamp says what shipped.

Why this exists, and why it is a tool rather than a snippet
-----------------------------------------------------------
An export carries a `stamp` naming the reflow2 version that WROTE it. The live
MCP server is whatever binary was running when the session began, so on a
release cut — where step 1 bumps the version and step 6 exports — exporting
through the live server stamps the OLD version. v0.20.0's committed export said
`0.19.0` for exactly this reason, and v0.21.0 nearly repeated it.

`--export` cannot fix it: that flag writes to STDOUT, and an export copied into
place has NO LINEAGE, because the `prev_content_hash` chain is built from the
file already sitting at the target path. So the last step has to call the
`export_graph` TOOL with a `path`, against a server running the NEW binary.

This has been hand-rolled in a scratchpad for three consecutive cuts. It is kit.

    tools/export_via_binary.py --source live.json --out docs/design/reflow2.json

`--source` is an export document taken from wherever the design actually lives
(usually the live server, which holds the RocksDB lock). It is imported into a
throwaway graph driven by THIS repo's freshly-built binary, and exported once
onto `--out`, which must already hold the committed export so the chain links.
"""
import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile

BIN = "target/debug/reflow2-mcp"


def rpc(proc, msg):
    proc.stdin.write(json.dumps(msg) + "\n")
    proc.stdin.flush()
    while True:
        line = proc.stdout.readline()
        if not line:
            raise SystemExit("server closed the connection")
        try:
            got = json.loads(line)
        except json.JSONDecodeError:
            continue
        if got.get("id") == msg.get("id"):
            return got


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--source", required=True,
                    help="export document to replay (from the live server)")
    ap.add_argument("--out", required=True,
                    help="path to write; must hold the committed export so lineage links")
    ap.add_argument("--bin", default=BIN)
    ap.add_argument("--accept-divergence", action="store_true",
                    help="write even if the export would DROP design the target holds. "
                         "Only when you can NAME what is dropped and mean it.")
    args = ap.parse_args()

    binary = os.path.abspath(args.bin)
    if not os.path.exists(binary):
        print(f"binary not found: {binary}\nBuild it first:  cargo build -p reflow2-mcp")
        return 1

    tmp = tempfile.mkdtemp(prefix="reflow2-replay-")
    graph = os.path.join(tmp, "graph")
    try:
        # Import through the NEW binary. `--import` adopts the document's own
        # graph_id, so identity is preserved rather than renamed (BL-169).
        r = subprocess.run([binary, "--graph-path", graph, "--import", args.source],
                           capture_output=True, text=True)
        if r.returncode != 0:
            print("import failed:\n" + (r.stderr or r.stdout)[:4000])
            return 1

        proc = subprocess.Popen([binary, "--graph-path", graph],
                                stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                stderr=subprocess.DEVNULL, text=True, bufsize=1)
        rpc(proc, {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2024-11-05", "capabilities": {},
            "clientInfo": {"name": "export-via-binary", "version": "0"}}})
        proc.stdin.write(json.dumps(
            {"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n")
        proc.stdin.flush()

        arguments = {"path": os.path.abspath(args.out), "overwrite": True}
        if args.accept_divergence:
            arguments["accept_divergence"] = True
        reply = rpc(proc, {"jsonrpc": "2.0", "id": 2, "method": "tools/call",
                           "params": {"name": "export_graph", "arguments": arguments}})
        proc.terminate()

        if "error" in reply:
            print("EXPORT REFUSED:\n" + json.dumps(reply["error"], indent=1)[:4000])
            print("\nIf this names dropped design you deliberately retracted, and you can "
                  "NAME each item, re-run with --accept-divergence.")
            return 1

        payload = reply["result"]
        sc = payload.get("structuredContent")
        if sc is None:
            sc = json.loads(payload["content"][0]["text"])
        print(f"nodes {sc['nodes']}  edges {sc['edges']}  bytes {sc['bytes']}")
        print(f"stamp reflow2_version : {sc['stamp']['reflow2_version']}")
        print(f"content_hash          : {sc['content_hash']}")
        print(f"prev_content_hash     : {sc['prev_content_hash']}")
        print("\nVerify prev_content_hash equals the content_hash of the export that was "
              "committed at --out before this ran, or the lineage chain has a hole.")
        return 0
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
