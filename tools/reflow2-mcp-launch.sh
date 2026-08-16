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
# Rust sources, the manifests and lockfile (dependency changes matter too), and
# the two trees that are COMPILED IN from outside `src/`.
#
# ⚠️ THIS HASH IS A GATE IN FRONT OF CARGO'S OWN CHANGE DETECTION, so anything
# it misses, cargo is never asked about. That is what made the omissions below
# bite rather than merely being incomplete:
#
#   schema/*.yaml            — `include_str!`d by reflow2-core (schema.rs:19-38),
#                              so a schema-only edit changed the served
#                              VOCABULARY and left this hash identical. Measured
#                              2026-08-16: the server answered `describe_schema`
#                              with the pre-#199 `change_type` enum while main
#                              already declared `defect_fix`, and the session
#                              nearly recorded the exact fiction #199 was merged
#                              to end. It was caught only by grepping the YAML
#                              and comparing — nothing in the tool's own reply
#                              says which build answered it.
#
#   getting-started/skills/  — embedded by crates/reflow2-mcp/build.rs, which
#                              ALREADY declares `cargo:rerun-if-changed` on this
#                              tree. Cargo would have rebuilt correctly; this
#                              wrapper skipped calling cargo at all, so a
#                              mechanism that was right was defeated by the gate
#                              in front of it. Editing a SKILL.md served the old
#                              text with no warning.
#
# Tests are deliberately OUT: they are separate targets and cannot change the
# binary this serves.
srchash() {
  {
    find crates -path '*/src/*.rs' -type f -print0 | sort -z | xargs -0 sha256sum
    find schema -name '*.yaml' -type f -print0 | sort -z | xargs -0 sha256sum
    find getting-started/skills -name 'SKILL.md' -type f -print0 | sort -z | xargs -0 sha256sum
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
