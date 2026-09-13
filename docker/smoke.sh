#!/usr/bin/env bash
#
# Start the image and prove it SERVES. Not that it builds, not that it pushes,
# not that the manifest resolves — that it comes up and answers.
#
# Usage:
#   docker/smoke.sh reflow2-mcp:dev          # after docker/build.sh
#   docker/smoke.sh ghcr.io/…/reflow2-mcp:0.31.0
#
# ⭐ WHY THIS EXISTS, because a check nobody understands gets deleted.
#
# v0.27.0 withdrew the content store and its `--content-path` flag. The
# ENTRYPOINT kept passing it. clap exits 2 on an unknown argument, so the
# container died before it opened the graph — and v0.27.0, v0.28.0, v0.29.0,
# v0.30.0 and v0.31.0 EACH PUBLISHED THAT IMAGE. Five releases.
#
# `release.yml` was not negligent; it verified the wrong property. It checked
# that the image was PULLABLE (`imagetools inspect`), which was true the whole
# time. A pull is not a start. The image's own HEALTHCHECK could not report it
# either, because the process was gone before the port was ever bound — a
# readiness probe cannot fail on behalf of something that never started.
#
# ⚠️ THE ORDERING IS THE POINT. Run this BEFORE the push, not after. A smoke
# test on an image that is already public tells you what you shipped; this one
# is here to stop you shipping it.
set -euo pipefail

image="${1:?usage: docker/smoke.sh <image[:tag]>}"
name="reflow2-smoke-$$"
data="$(mktemp -d)"
# The runtime user is uid/gid 1000 (fixed in the Dockerfile precisely so this
# instruction is complete). A volume it cannot write is the commonest way a
# first run fails, and it would fail here as a timeout rather than as itself.
chown -R 1000:1000 "$data" 2>/dev/null || sudo chown -R 1000:1000 "$data"

cleanup() {
  docker rm -f "$name" >/dev/null 2>&1 || true
  rm -rf "$data" 2>/dev/null || sudo rm -rf "$data" || true
}
trap cleanup EXIT

fail() {
  echo "" >&2
  echo "SMOKE FAILED: $*" >&2
  echo "--- docker logs ---" >&2
  docker logs "$name" 2>&1 | tail -40 >&2 || echo "(no logs — the container may never have started)" >&2
  echo "--- state ---" >&2
  docker inspect "$name" --format 'running={{.State.Running}} exit={{.State.ExitCode}} health={{if .State.Health}}{{.State.Health.Status}}{{else}}none{{end}}' >&2 || true
  exit 1
}

echo "smoke: starting $image"
docker run -d --name "$name" -v "$data:/data" -p 127.0.0.1:18080:8080 "$image" >/dev/null

# THE FIRST THING CHECKED IS THAT IT IS STILL ALIVE, and it is checked before
# anything about readiness. The failure this exists for is an immediate exit,
# and waiting 60s for a health probe to go unhealthy would report it as a
# timeout — true, but it would not name the cause, and the cause is one line of
# stderr the container printed on its way out.
sleep 3
if [ "$(docker inspect "$name" --format '{{.State.Running}}')" != "true" ]; then
  code="$(docker inspect "$name" --format '{{.State.ExitCode}}')"
  fail "the container exited immediately with code ${code}. An unknown CLI flag exits 2 — check that every flag in the ENTRYPOINT still exists in this binary."
fi

# Then readiness, on the image's OWN healthcheck rather than a second opinion
# invented here. HEALTHCHECK has --start-period=60s, so `starting` is not a
# problem until it stops being `starting`.
deadline=$(( SECONDS + 180 ))
while [ "$SECONDS" -lt "$deadline" ]; do
  status="$(docker inspect "$name" --format '{{if .State.Health}}{{.State.Health.Status}}{{else}}none{{end}}')"
  running="$(docker inspect "$name" --format '{{.State.Running}}')"
  [ "$running" = "true" ] || fail "the container exited while coming up (code $(docker inspect "$name" --format '{{.State.ExitCode}}'))."
  case "$status" in
    healthy) echo "smoke: healthy after ~$SECONDS s"; break ;;
    unhealthy) fail "the healthcheck went unhealthy — it started and did not serve." ;;
    none) fail "this image declares no HEALTHCHECK, so 'it serves' cannot be asserted. That is a change to the Dockerfile, not to this script." ;;
  esac
  sleep 3
done
[ "${status:-}" = "healthy" ] || fail "still '${status:-unknown}' after 180s."

# One request from OUTSIDE the container, because the healthcheck runs INSIDE it
# and therefore cannot catch a bind to loopback instead of 0.0.0.0 — a plausible
# regression that would leave every in-container probe perfectly green.
curl -fsS -o /dev/null -m 10 "http://127.0.0.1:18080/" 2>/dev/null \
  || curl -sS -o /dev/null -m 10 "http://127.0.0.1:18080/" 2>/dev/null \
  || fail "the published port did not answer from outside the container — check the bind address is routable, not loopback."

echo "smoke: OK — $image starts, reports healthy, and answers on its published port"

# ─────────────────────────────────────────────────────────────────────────────
# PHASE 2 — TWO DESIGNS IN THE IMAGE, AND THEY STAY APART
#
# ⭐ WHY THIS IS HERE AND NOT ONLY IN THE CRATE. `sessions_cannot_cross_designs`
# and `two_designs_stay_apart_over_the_transport` prove isolation against the
# BUILD. This proves it against the ARTEFACT — the image a consumer pulls, with
# its own entrypoint, its own env, its own unprivileged user and its own libc.
# flo2 asked for exactly this and it was unbuildable until 2026-09-13, because
# nothing in the binary called `Registry::discover`.
#
# 🛑 THE PROPERTY FAILS SILENTLY. A handler that reaches the wrong design
# corrupts it rather than erroring, so a green start says nothing about it.
# ⚠️ AND A CHECK THAT CANNOT FAIL IS WORSE THAN NONE: this asserts a WRITE into
# one design is ABSENT from the other, rather than merely that both answer. Two
# empty designs would pass the weaker check forever.
echo "smoke: phase 2 — two designs under one registry root"

reg="$(mktemp -d)"
chown -R 1000:1000 "$reg" 2>/dev/null || sudo chown -R 1000:1000 "$reg"
cleanup2() { docker rm -f "$name-a" "$name-b" "$name-reg" >/dev/null 2>&1 || true
             rm -rf "$reg" 2>/dev/null || sudo rm -rf "$reg" || true; }
trap 'cleanup; cleanup2' EXIT

# Mint two designs AT THE REGISTRY SHAPE (<root>/<name>/.reflow2/graph). A store
# at any other path is not discovered, which is a convention worth failing on
# loudly here rather than in a consumer's deployment.
for d in alpha beta; do
  docker run -d --name "$name-$d" -v "$reg:/data" \
    -e REFLOW2_GRAPH_PATH="/data/$d/.reflow2/graph" "$image" >/dev/null
  for _ in $(seq 1 40); do
    [ "$(docker inspect "$name-$d" --format '{{.State.Running}}')" = "true" ] || break
    docker exec "$name-$d" test -d "/data/$d/.reflow2/graph" 2>/dev/null && break
    sleep 1
  done
  docker rm -f "$name-$d" >/dev/null 2>&1 || true
done
[ -d "$reg/alpha/.reflow2/graph" ] && [ -d "$reg/beta/.reflow2/graph" ] \
  || fail "could not mint two designs under the registry root — nothing to isolate."

docker run -d --name "$name-reg" -v "$reg:/data" -p 127.0.0.1:18081:8080 \
  -e REFLOW2_REGISTRY_ROOT=/data "$image" >/dev/null
sleep 3
[ "$(docker inspect "$name-reg" --format '{{.State.Running}}')" = "true" ] \
  || { echo "--- registry container logs ---" >&2
       docker logs "$name-reg" 2>&1 | tail -20 >&2
       fail "the container exited immediately in registry mode (code $(docker inspect "$name-reg" --format '{{.State.ExitCode}}')). Every flag in the ENTRYPOINT must exist in this binary — and note that \${VAR:-...} substitutes the VALUE when VAR is set, which is how this broke the first time."; }

# The ids are the durable names; read them from what the image itself wrote.
ids="$(docker logs "$name-reg" 2>&1 | grep -oE '[0-9a-f]{16}' | sort -u | head -2)"
A="$(echo "$ids" | sed -n 1p)"; B="$(echo "$ids" | sed -n 2p)"
[ -n "$A" ] && [ -n "$B" ] || fail "the registry server did not report two designs; it logged:
$(docker logs "$name-reg" 2>&1 | tail -20)"
echo "smoke: registry serving $A and $B"

mcp() { # mcp <prefix> <json> [session]
  curl -sS -m 30 -D /tmp/h.$$ -H 'Content-Type: application/json' \
    -H 'Accept: application/json, text/event-stream' \
    ${3:+-H "mcp-session-id: $3"} -d "$2" "http://127.0.0.1:18081$1"
}
sid_of() { grep -i '^mcp-session-id' /tmp/h.$$ | tr -d '\r' | cut -d' ' -f2; }
INIT='{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"smoke","version":"1"}}}'

mcp "/g/$A/" "$INIT" >/dev/null || fail "design $A did not answer in the image"
SA="$(sid_of)"
mcp "/g/$B/" "$INIT" >/dev/null || fail "design $B did not answer in the image"
SB="$(sid_of)"
[ -n "$SA" ] && [ -n "$SB" ] || fail "the image served no session id for one of the designs"

W='{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"add_requirement","arguments":{"id":"req:only-in-a","name":"only in a","statement":"this belongs to the first design"}}}'
R='{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"get_node","arguments":{"id":"req:only-in-a"}}}'
mcp "/g/$A/" "$W" "$SA" >/dev/null
mcp "/g/$A/" "$R" "$SA" | grep -q "this belongs to the first design" \
  || fail "the write did not land in its own design — isolation cannot be asserted over a write that never happened."
if mcp "/g/$B/" "$R" "$SB" | grep -q "this belongs to the first design"; then
  fail "ISOLATION BREACH IN THE PUBLISHED IMAGE: design $B returned design $A's node."
fi
rm -f /tmp/h.$$

echo "smoke: OK — two designs in $image stay apart, and a write into one is absent from the other"
