#!/usr/bin/env bash
#
# Rebuild the reflow2 MCP binary that sessions are served, DELIBERATELY.
#
# `tools/reflow2-mcp-launch.sh` serves `target/release/reflow2-mcp` and never
# builds it. This is the other half: you run this when you want the binary your
# next session talks to to match your source.
#
# WHY IT IS A SEPARATE ACT. A release build here takes roughly 34 minutes,
# dominated by librocksdb-sys compiling C++ from source. Rebuilding on every
# source change — which is what the launcher used to do for the debug profile —
# would turn any edit into a half-hour cold start on the next session. So the
# decision to spend that time is yours, and the launcher only tells you when
# the binary has fallen behind (req:a-session-runs-a-release-binary).
#
# WHAT IT COSTS, AND WHY IT IS WORTH IT. Measured 2026-09-11, same bytes and
# same path with only the profile differing: a propagate walk 5,851 ms -> 62 ms
# warm, design_regions 12.5 s -> 2.0 s warm. The compile cost is the builder's,
# the benefit is every session's.
#
# ON A MEMORY-CONSTRAINED BOX, give cargo its own cgroup so a runaway build is
# killed instead of your desktop — see AGENTS.md. That belongs in your own
# environment, not in this script: it is a property of one machine.
#
#   REFLOW2_PROFILE=debug tools/reflow2-rebuild.sh   # the edit-compile loop
#
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo"

profile="${REFLOW2_PROFILE:-release}"
case "$profile" in
  release) cargo_profile_flag=(--release) ;;
  debug)   cargo_profile_flag=() ;;
  *)
    echo "reflow2-rebuild: REFLOW2_PROFILE must be 'release' or 'debug', got '$profile'." >&2
    exit 1
    ;;
esac

stamp="target/$profile/.reflow2-mcp.srchash"

# THE SAME HASH THE LAUNCHER COMPARES. Kept identical on purpose: if these two
# ever compute differently, the launcher warns about staleness this script
# cannot clear, or clears staleness the launcher still sees. Both are worse
# than the duplication.
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

if [ -x "target/$profile/reflow2-mcp" ] && [ "$want" = "$have" ] && [ "${1:-}" != "--force" ]; then
  echo "reflow2-rebuild: target/$profile/reflow2-mcp already matches your source (${want:0:12}). Nothing to do."
  echo "reflow2-rebuild: pass --force to rebuild anyway."
  exit 0
fi

echo "reflow2-rebuild: building reflow2-mcp, $profile profile. This can take ~34 minutes cold."
start=$SECONDS

if cargo build "${cargo_profile_flag[@]}" -p reflow2-mcp; then
  # Stamp only on success, so a failed build leaves the launcher still warning.
  printf '%s' "$want" > "$stamp"
  echo "reflow2-rebuild: ok in $(( (SECONDS - start) / 60 ))m$(( (SECONDS - start) % 60 ))s — target/$profile/reflow2-mcp is now ${want:0:12}."
  echo "reflow2-rebuild: a RUNNING server keeps serving the binary it started from."
  echo "reflow2-rebuild: restart it to pick this up — /mcp reconnect does NOT replace a running stdio server."
else
  echo "reflow2-rebuild: BUILD FAILED — the stamp is unchanged, so the launcher will go on telling you the binary is behind." >&2
  exit 1
fi
