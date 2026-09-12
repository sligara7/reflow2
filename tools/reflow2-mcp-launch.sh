#!/usr/bin/env bash
#
# reflow2 MCP launch wrapper.
#
# Purpose: serve a RELEASE-profile binary to every session, and rebuild it only
# when the builder says so. `.mcp.json` invokes this instead of the raw binary.
#
# ═══ WHY THE PROFILE CHANGED, 2026-09-11 ═══
#
# Measured on the same bytes and the same path, profile the only difference:
#
#     propagate_from, 4 consecutive calls, one process
#       debug    5851 / 5942 / 5904 / 5784 ms
#       release  1614 /   70 /   88 /   62 ms      -> 93x on the walk
#     design_regions   12.5 s (debug)  ->  5.5 s cold / 2.0 s warm (release)
#
# Every session on this machine had been served `target/debug`, so every cost
# number in the 2026-09-10/11 tool sweep was taken on an unoptimized binary.
# req:a-session-runs-a-release-binary is the user's half of the ruling that
# kept RocksDB: its compile cost is the builder's, its benefit is the user's.
#
# ═══ WHY IT NO LONGER BUILDS FOR YOU ═══
#
# A release build takes ~34 minutes here, dominated by librocksdb-sys compiling
# C++ from source. The content-hash auto-rebuild below would therefore turn any
# source edit into a half-hour cold start on the next session. So the shape is
# SERVE RELEASE, REBUILD DELIBERATELY:
#
#   * release (the default) — never builds. Missing binary: refuse and name the
#     command. Binary older than your source: serve it, and SAY SO LOUDLY.
#   * debug (REFLOW2_PROFILE=debug) — the edit-compile loop, unchanged: content
#     hash, auto-rebuild, refuse to serve a broken build.
#
# ⚠️ THE HAZARD THIS TRADE REINTRODUCES, AND WHAT ANSWERS IT. The auto-rebuild
# existed because a stale binary served stale VOCABULARY in silence: on
# 2026-08-16 the server answered `describe_schema` with the pre-#199
# `change_type` enum while main already declared `defect_fix`, and the session
# nearly recorded the exact fiction #199 was merged to end. Not rebuilding
# brings that failure mode back, so staleness here is never silent — see the
# banner below — and `graph_report`'s `served_by.stale` is the second guard,
# read from the running process rather than from this script.
#
# CRITICAL: nothing here may write to stdout. stdout is the MCP JSON-RPC channel;
# a stray byte corrupts the protocol. All diagnostics and build output go to
# stderr (which Claude Code surfaces in the MCP server log).
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo"

profile="${REFLOW2_PROFILE:-release}"
case "$profile" in
  release|debug) ;;
  *)
    echo "reflow2-launch: REFLOW2_PROFILE must be 'release' or 'debug', got '$profile'." >&2
    exit 1
    ;;
esac

bin="target/$profile/reflow2-mcp"
stamp="target/$profile/.reflow2-mcp.srchash"

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

if [ "$profile" = "debug" ]; then
  # The edit-compile loop, unchanged: build on content change, refuse a broken build.
  if [ ! -x "$bin" ] || [ "$want" != "$have" ]; then
    echo "reflow2-launch: debug profile — source changed or binary missing, building…" >&2
    if cargo build -p reflow2-mcp >&2; then
      printf '%s' "$want" > "$stamp"
      echo "reflow2-launch: build ok." >&2
    else
      echo "reflow2-launch: BUILD FAILED — refusing to serve a stale/broken binary." >&2
      echo "reflow2-launch: fix the build, then reconnect reflow2 (/mcp)." >&2
      exit 1
    fi
  else
    echo "reflow2-launch: debug binary current (source hash ${want:0:12}), skipping build." >&2
  fi
else
  # Release: never build here. A ~34-minute compile is not something a session start
  # may decide to do on your behalf.
  if [ ! -x "$bin" ]; then
    echo "reflow2-launch: no release binary at $bin, and this wrapper does not build one." >&2
    echo "reflow2-launch: build it once  ->  tools/reflow2-rebuild.sh" >&2
    echo "reflow2-launch: or serve debug ->  REFLOW2_PROFILE=debug (edit-compile loop)" >&2
    exit 1
  fi
  if [ "$want" != "$have" ]; then
    banner() { printf 'reflow2-launch: # %-60s #\n' "$1" >&2; }
    printf 'reflow2-launch: %s\n' "################################################################" >&2
    banner "SERVING A RELEASE BINARY OLDER THAN YOUR SOURCE."
    banner "Its tools, schema and skills are the ones it was built with,"
    banner "not the ones in your tree. A schema edit that is not in this"
    banner "binary is served as vocabulary that no longer exists."
    banner ""
    banner "  source  ${want:0:12}"
    banner "  binary  ${have:0:12}${have:+ }${have:-never stamped}"
    banner "  rebuild tools/reflow2-rebuild.sh  (~34 min, RocksDB C++)"
    printf 'reflow2-launch: %s\n' "################################################################" >&2
  else
    echo "reflow2-launch: release binary current (source hash ${want:0:12})." >&2
  fi
fi

# Hand off stdin/stdout/stderr to the real server. Pass through --graph-path etc.
exec "$bin" "$@"
