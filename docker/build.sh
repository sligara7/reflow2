#!/usr/bin/env bash
#
# Build the reflow2-mcp container image LOCALLY.
#
# The Dockerfile deliberately does not compile anything — it wraps an existing
# `reflow2-mcp` binary, so that the published image ships byte-for-byte what the
# release ships rather than a separately-compiled twin (see the Dockerfile's own
# comments). CI gets that binary from the release artifact; this script builds it
# from your checkout instead, so a local image is a real test of the packaging
# without waiting for a tag.
#
# Usage:
#   docker/build.sh                 # build, tag as reflow2-mcp:dev
#   docker/build.sh v0.24.0         # build, tag as reflow2-mcp:v0.24.0
#
# Then run it — note the volume, which is where every design lives:
#   mkdir -p /srv/reflow2-data && sudo chown -R 1000:1000 /srv/reflow2-data
#   docker run --rm -p 8080:8080 -v /srv/reflow2-data:/data reflow2-mcp:dev
#
# ⚠️ NO AUTHENTICATION. Bind it to a private network, never the open internet.
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo"

tag="${1:-dev}"

echo "docker/build.sh: building reflow2-mcp (release profile) — this compiles RocksDB on a cold cache and is slow." >&2
cargo build --release -p reflow2-mcp

# The Dockerfile COPYs `reflow2-mcp` from the build context root, exactly as CI
# stages it from the release tarball. Staging it here keeps the one Dockerfile
# honest for both paths rather than growing a build-arg that only CI sets.
staged="$repo/reflow2-mcp"
cleanup() { rm -f "$staged"; }
trap cleanup EXIT
cp "target/release/reflow2-mcp" "$staged"

# ⭐ PICK A BASE THE BINARY CAN ACTUALLY RUN ON, AND PRE-FLIGHT IT.
#
# The release image's base is ubuntu:22.04 because CI builds the binary on an
# ubuntu-22.04 runner. A contributor's box is usually newer, and a binary linked
# against a newer glibc fails at CONTAINER START, not at build time:
#     /usr/local/bin/reflow2-mcp: /lib/.../libc.so.6: version `GLIBC_2.38' not found
# The image builds green and the container exits 1. That was hit on the very
# first local build of this file, which is why this check exists.
base="${REFLOW2_DOCKER_BASE:-ubuntu:22.04}"
need="$(objdump -T "$staged" 2>/dev/null | grep -o 'GLIBC_[0-9.]*' | sort -uV | tail -1 || true)"
echo "docker/build.sh: binary needs up to ${need:-unknown}; trying base ${base}" >&2

preflight() {
  docker run --rm --entrypoint /bin/sh \
    -v "$staged":/probe/reflow2-mcp:ro "$1" \
    -c '/probe/reflow2-mcp --help >/dev/null 2>&1 || exit 1' >/dev/null 2>&1
}

if ! preflight "$base"; then
  echo "docker/build.sh: the binary cannot run on ${base} (glibc too old for ${need:-it})." >&2
  if [ -z "${REFLOW2_DOCKER_BASE:-}" ] && preflight "ubuntu:24.04"; then
    base="ubuntu:24.04"
    echo "docker/build.sh: falling back to ${base} for this DEV image." >&2
    echo "docker/build.sh: ⚠️  this is NOT the base the release image uses (ubuntu:22.04)." >&2
    echo "docker/build.sh:    it exercises the packaging — volume layout, non-root uid, entrypoint," >&2
    echo "docker/build.sh:    readiness — but it is not the artifact consumers receive." >&2
  else
    echo "docker/build.sh: set REFLOW2_DOCKER_BASE to an image whose glibc is new enough, or" >&2
    echo "docker/build.sh: build the binary on the release platform. Refusing to produce an" >&2
    echo "docker/build.sh: image that builds green and dies on start." >&2
    exit 1
  fi
fi

docker build --build-arg "BASE=${base}" --tag "reflow2-mcp:${tag}" .

cat >&2 <<EOF
docker/build.sh: built reflow2-mcp:${tag}

Remember the volume layout — the sidecars live BESIDE the store, and a store
opened without its identity sidecar presents as an EMPTY design rather than
erroring:

  /data/graphs/<design>/graph            the store
  /data/graphs/<design>/graph.*.json     identity, version, sync  <- mount the PARENT
  /data/content                          blobs

Mount the parent directory, not the store itself, and use a real block device or
local volume — never NFS, where RocksDB's exclusive lock is not reliable.
EOF
