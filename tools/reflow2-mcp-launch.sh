#!/usr/bin/env bash
#
# reflow2 MCP launch wrapper.
#
# Purpose: guarantee the reflow2-mcp binary is built from CURRENT source before
# the MCP server starts serving. `.mcp.json` invokes this instead of the raw
# binary, so every new session and every `/mcp` reconnect gets a fresh build with
# zero manual steps.
#
# Why content-hash and not just `cargo build`: cargo's freshness check keys on
# file mtimes, and git operations (pull --rebase, checkout) can leave source
# mtimes OLDER than the last-built binary. When that happens `cargo build`
# no-ops on genuinely-changed source and a stale binary serves for a whole
# session. Hashing file CONTENT sidesteps the mtime trap entirely — we rebuild
# iff the source bytes changed (or the binary is missing).
#
# CRITICAL: nothing here may write to stdout. stdout is the MCP JSON-RPC channel;
# a stray byte corrupts the protocol. All diagnostics and build output go to
# stderr (which Claude Code surfaces in the MCP server log).
#
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo"

bin="target/debug/reflow2-mcp"
stamp="target/debug/.reflow2-mcp.srchash"

# Content hash of everything that can change the compiled binary: every crate's
# Rust sources plus the manifests and lockfile (dependency changes matter too).
srchash() {
  {
    find crates -path '*/src/*.rs' -type f -print0 | sort -z | xargs -0 sha256sum
    sha256sum Cargo.toml Cargo.lock crates/*/Cargo.toml 2>/dev/null
  } | sha256sum | cut -d' ' -f1
}

want="$(srchash)"
have=""
[ -f "$stamp" ] && have="$(cat "$stamp")"

if [ ! -x "$bin" ] || [ "$want" != "$have" ]; then
  echo "reflow2-launch: source changed or binary missing — building reflow2-mcp…" >&2
  if cargo build -p reflow2-mcp >&2; then
    # Record the hash only on success, so a failed build re-triggers next launch.
    printf '%s' "$want" > "$stamp"
    echo "reflow2-launch: build ok." >&2
  else
    echo "reflow2-launch: BUILD FAILED — refusing to serve a stale/broken binary." >&2
    echo "reflow2-launch: fix the build, then reconnect reflow2 (/mcp)." >&2
    exit 1
  fi
else
  echo "reflow2-launch: binary current (source hash ${want:0:12}), skipping build." >&2
fi

# Hand off stdin/stdout/stderr to the real server. Pass through --graph-path etc.
exec "$bin" "$@"
