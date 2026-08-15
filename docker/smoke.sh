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
