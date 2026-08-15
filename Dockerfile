# reflow2-mcp as a container — `req:reflow2-consumable-as-an-image`.
#
# WHAT THIS IS FOR: letting a consumer (flo2, and anyone who is not building
# reflow2) run the design server without a Rust toolchain, pinned to a version
# rather than chasing `latest`.
#
# ⭐ THE BINARY IS COPIED IN, NOT BUILT HERE, AND THAT IS DELIBERATE.
# `release.yml`'s `binaries` job has already produced the linux-x86_64 binary by
# the time this image is built, so the container job downloads that artifact and
# this Dockerfile just wraps it. Two reasons, both load-bearing:
#   1. A multi-stage Rust build would recompile RocksDB — ~14 minutes, per the
#      note at the top of release.yml — for a binary that already exists.
#   2. More importantly, it guarantees THE IMAGE SHIPS THE EXACT BINARY THE
#      RELEASE SHIPS. A separately-compiled binary could differ from the one
#      users download, and nothing would report it.
# The cost, stated: this Dockerfile cannot be built from a bare checkout. It
# needs `reflow2-mcp` present in the build context. `docker/build.sh` in this
# repo does that for a local build; CI does it from the release artifact.
#
# BASE IMAGE: ubuntu:22.04, matching the `binaries` job's runner EXACTLY. That
# job pins ubuntu-22.04 precisely because the binary links the runner's glibc,
# so the runtime image must not be older. Matching rather than merely
# "new enough" means the glibc the binary was linked against is the glibc it
# runs on, and a base bump is then a deliberate act rather than a silent one.
#
# ⚠️ THE BASE IS AN ARG BECAUSE THE COUPLING IS SHARP AND WAS HIT IMMEDIATELY.
# The first local test of this file staged a binary built on an Ubuntu 24.04
# host (glibc 2.39) into this 22.04 base (glibc 2.35). The image built fine and
# the container died on start with `version 'GLIBC_2.38' not found` — a runtime
# failure with no build-time signal at all. CI never sees this, because it
# stages the binary the ubuntu-22.04 runner produced; a CONTRIBUTOR building
# locally on a modern distro sees it every time. `docker/build.sh` therefore
# picks a base matching whatever binary it just built, and says so.
# THE DEFAULT IS THE RELEASE BASE: overriding it produces a dev image, not the
# image users get.
ARG BASE=ubuntu:22.04
FROM ${BASE}

# ca-certificates only. reflow2 makes no outbound calls of its own, but a TLS
# trust store is what a sidecar/proxy in front of it will expect to exist, and
# omitting it is the kind of thing that fails at 2am rather than at build time.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# ⭐ NON-ROOT, AND THE UID IS FIXED ON PURPOSE.
# The commonest way a first container run fails is a mounted volume the runtime
# user cannot write. Pinning uid/gid 1000 means `chown -R 1000:1000 <volume>` on
# the host is a complete instruction, rather than "find out what uid the image
# happens to use".
# ⚠️ CREATED ONLY IF ABSENT, because bases disagree about uid 1000. ubuntu:22.04
# has no user there; ubuntu:24.04 ships one (`ubuntu`) and a bare
# `useradd --uid 1000` exits 4 on it. Hit on the second local build, immediately
# after the base became an ARG — the two changes interact, and hardcoding the
# creation would have made the ARG a trap rather than an escape hatch.
# What matters is the RUNTIME UID, not who owns the name: 1000 either way, so
# `chown -R 1000:1000 <volume>` stays a complete instruction on any base.
RUN set -eux; \
    if ! getent group 1000 >/dev/null; then groupadd --gid 1000 reflow2; fi; \
    if ! getent passwd 1000 >/dev/null; then \
      useradd --uid 1000 --gid 1000 --create-home --shell /usr/sbin/nologin reflow2; \
    fi

COPY --chown=root:root reflow2-mcp /usr/local/bin/reflow2-mcp
RUN chmod 0755 /usr/local/bin/reflow2-mcp

# ⭐ EVERY DURABLE THING LIVES UNDER /data — `req:hosted-state-outlives-the-image`.
#
# A reflow2 design is not configuration that can be rebuilt; it is the product.
# Anything baked into the image is lost on the next deploy, so the layout is:
#
#   /data/graphs/<design>/graph            the RocksDB store
#   /data/graphs/<design>/graph.id.json    identity  ┐ SIDECARS — these live
#   /data/graphs/<design>/graph.meta.json  version   │ BESIDE the store, not
#   /data/graphs/<design>/graph.sync.json  sync      ┘ inside it
#
# ⚠️ THE SIDECARS ARE NOT OPTIONAL AND NOT SEPARABLE. A store opened without the
# identity sidecar it was created with finds nothing and PRESENTS AS AN EMPTY
# DESIGN, reporting nothing wrong. Because they sit beside the store rather than
# inside it, mounting only `.../graph` and not its parent directory is a
# plausible-looking mistake that silently loses a project. MOUNT THE PARENT.
#
# ⚠️ THE VOLUME MUST BE A REAL BLOCK DEVICE OR LOCAL VOLUME, NOT NFS. RocksDB's
# exclusive lock is a filesystem lock, and network filesystems honour those
# unreliably — a lock that silently fails to exclude is how two processes end up
# writing one store, which is the corruption the single-writer design exists to
# prevent, and it fails with no error saying so.
RUN mkdir -p /data/graphs/default \
    && chown -R 1000:1000 /data
VOLUME ["/data"]

USER 1000:1000
WORKDIR /data
EXPOSE 8080

ENV REFLOW2_GRAPH_PATH=/data/graphs/default/graph \
    REFLOW2_BIND=0.0.0.0:8080

# ⭐ READINESS: "the port is listening" is an HONEST readiness signal here, and
# that was checked rather than assumed. In `main.rs` the graph is opened FIRST
# and `serve_http` is only reached on the `Ok` arm, so the socket cannot be bound
# before the store is open and its full-text index built — which on a large
# design takes seconds, not milliseconds. A healthcheck that probes the port
# therefore cannot report ready early.
#
# Deliberately NOT the rendezvous sidecar: that file is published by the
# SHARED-server path, which this image does not run (see below).
HEALTHCHECK --interval=30s --timeout=3s --start-period=60s --retries=3 \
    CMD bash -c 'exec 3<>/dev/tcp/127.0.0.1/8080' || exit 1

# ⭐ `--http`, AND DELIBERATELY NOT `--shared` / `--serve-shared`.
# CONFIRMED IN CODE, not inherited: the 120-minute `--idle-timeout` expiry is
# implemented inside `serve_http`'s `Some(SharedServer { .. })` branch. The plain
# `--http` path passes `None`, so no idle expiry is ever armed. A container that
# exited after two quiet hours would look like a crash loop to an orchestrator;
# on this path it cannot happen, and that is a property of the code rather than
# of a flag someone must remember to pass.
#
# ⚠️ NO AUTHENTICATION. reflow2 has none, and `--http-allow-host` is Host-header
# allowlisting — DNS-rebinding protection only. THIS IMAGE MUST BE DEPLOYED ON A
# PRIVATE NETWORK, BEHIND A GATEWAY THAT DOES AUTHENTICATE. It does not defend
# itself and was never designed to. Authorization is the job of the layer in
# front; reflow2's only obligation is that a session bound to one design cannot
# address another.
#
# ⚠️ EVERY FLAG HERE MUST EXIST IN THE BINARY THIS IMAGE COPIES IN. clap exits 2
# on an unknown argument, so a flag removed from the CLI and left here is not a
# warning — it is a container that never starts, and the HEALTHCHECK below cannot
# tell you why because the process is gone before the port is bound. This
# happened: v0.27.0 withdrew the content store and its `--content-path`, this
# ENTRYPOINT kept passing it, and FIVE releases (v0.27.0 through v0.31.0) each
# published an image that exits 2. Nothing caught it because `release.yml`
# verified the image was PULLABLE, not that it RAN.
#
# ⭐ THAT HOLE IS NOW CLOSED, and you can close it yourself before pushing a
# commit: `docker/build.sh && docker/smoke.sh reflow2-mcp:dev` starts the image
# and fails in about three seconds on exactly this class of mistake. Release CI
# runs the same script against the candidate image BEFORE it publishes anything.
ENTRYPOINT ["/bin/sh", "-c", "exec /usr/local/bin/reflow2-mcp \
  --graph-path \"$REFLOW2_GRAPH_PATH\" \
  --http \"$REFLOW2_BIND\" \
  ${REFLOW2_ALLOW_HOST:+--http-allow-host \"$REFLOW2_ALLOW_HOST\"} \
  \"$@\"", "reflow2-mcp"]
