# Scope: reading a design somebody else is holding

> Part of the **Reflow 2.0** design docs — see **[overview.md](overview.md)** for the full map and
> reading order.

*Scoping only — nothing here is built. `req:read-while-held` is accepted work, and the mechanism
choice is open as `dec:read-while-held-mechanism`. Written 2026-07-26 by reading the code in
`dynograph-foundation` and `tantivy`, not from memory.*

## What we are trying to buy

Today the store is single-writer. One session holds the graph; every other session gets the degraded
server — a handshake, an explanation, and one tool. That is honest, and it is still five of six
sessions with no design brain.

`req:read-while-held`: **a session that cannot take the write lock should still be able to read** —
orientation, detectors, reports, export. Not write. One seat would still own capture.

## Finding 1 — it is three crates, not one

The assumption going in was "expose `open_as_secondary` in `dynograph-storage`". That is necessary
and not sufficient.

| Layer | What blocks a second reader | Where |
|---|---|---|
| **RocksDB** | `RocksBackend::open` takes RocksDB's exclusive directory lock | `dynograph-storage/src/backend.rs:376` |
| **Tantivy** | `TextIndex::open` eagerly builds an `IndexWriter`, which takes `.tantivy-writer.lock` | `dynograph-text/src/lib.rs:160` |
| **reflow2** | `reindex_search()` runs on **every** server start — a write | `reflow2-mcp/src/service.rs:1552` |

The Tantivy layer is the one that would have been discovered the hard way: the full-text index lives
at `<graph-path>/fulltext`, a subdirectory *inside* the store, and reflow2 builds with the `fulltext`
feature on. RocksDB's lock simply fires first, so nothing has ever reached the second lock to notice
it.

**The good news is in Tantivy's own source.** `INDEX_WRITER_LOCK` exists to allow exactly one
*writer*; the neighbouring `META_LOCK` is documented as making it "possible for another process to
safely consume our index in-writing". A reader in a second process is a supported pattern, so the
change is a `TextIndex::open_read_only` that skips the writer — small, and idiomatic rather than a
workaround.

The reflow2 layer needs `reindex_search` skipped in read-only mode. Worth noting two smaller writes
that happen at open and are *outside* RocksDB, so they would still succeed and should be suppressed
deliberately rather than by luck: the provenance stamp refresh (`<graph>.meta.json`) and establishing
identity (`<graph>.id.json`). A read-only seat should not be recording that it "held" the graph.

## Finding 2 — read-only and secondary are genuinely different, and it is a real choice

RocksDB offers two, and the difference matters more here than it looks:

**`open_cf_descriptors_read_only`** — a consistent view as of open. No lock, no extra directory,
nothing to clean up. A long session goes stale and cannot refresh; seeing new writes means
reopening.

**`open_cf_as_secondary`** — also read-only, but `try_catch_up_with_primary()` refreshes it on
demand. Needs its **own** secondary directory for its private state, which is a second temp path to
create, manage and remove — the same class of thing as `--export-snapshot`'s scratch dir, which has
already bitten twice (a leaked sidecar, and a residue test that stripped only the suffix it knew).

The interesting interaction is with `req:stale-seat-knows`. A read-only seat that silently drifts
from the primary is precisely the "complete but older" trap we just closed for exports, arriving
from a new direction. Read-only-at-open makes staleness *bounded and knowable* (it is exactly as old
as the open). Secondary makes it *fixable* but introduces "how stale am I right now?" as a live
question the seat has to keep answering.

Recorded as `dec:read-while-held-mechanism` with that trade written down.

## Finding 3 — the tool surface splits cleanly

- **35** tools are annotated `read_only_hint = true`
- **69** handlers take `write_lock()`

So a read-only server serves about a third of the surface and refuses the rest. The refusal already
has a model to copy: the degraded server's `reflow2_unavailable` payload — reason, remedies, and an
explicit *do not conclude the design is missing*. Here it becomes *do not conclude the design is
read-only forever; another session holds the write lock, and here is which one* (`claim_report` now
knows seats, so that sentence can name it).

## What this buys, and what it does not

**Buys:** five of six fleet seats get orientation, `detect_gaps`, `detect_defects`, `loop_status`,
`graph_report`, `compare_designs` and `export_graph` — which is most of what a reading seat did by
hand through `--export-snapshot` on 2026-07-25, but live and without the copy.

**Does not buy:** parallel *writing*. One seat still owns capture. The answer for parallel work
remains a worktree per seat with its own graph, reconciled through git — unchanged by this.

Worth being clear that this is a **different use case** from the fleet's, not a replacement for it:
a boss reading the design while a worker writes it, rather than several workers writing.

## Cost

A new foundation tag means a full `librocksdb-sys` C++ rebuild — **~10 minutes on every machine that
pulls**, including Alex's, on top of the v0.13.0 update he has not taken yet. AGENTS.md's standing
rule is to bump the pin only when reflow2 genuinely needs something new, which is satisfied here;
the rule's *other* half is that it costs collaborators, which is why the sequencing below puts it
last.

Two crates change upstream, so the foundation's own suite has to cover the new open paths — a read-
only backend that refuses writes, and a reader-only text index — before reflow2 can rely on either.

## Sequence

1. **`dynograph-text`: `open_read_only`** — smallest, independently testable, and the blocker nobody
   had noticed. Prove two processes can hold readers on one index while a third writes.
2. **`dynograph-storage`: a read-only backend** behind the `KvBackend` trait, whose `put` / `delete` /
   `commit_batch` **fail loud** rather than silently no-op. The trait already funnels every write
   through six methods, so this is a contained addition.
3. **Foundation tag**, once both are proven upstream.
4. **reflow2**: read-only mode in the open path — skip `reindex_search`, suppress the stamp and
   identity writes, serve the read tools, refuse the write tools with a message that names the
   holding seat.
5. **Extend `tools/test_degraded_server.py`** so a locked-out seat is proven to *read* a real held
   graph, not merely to explain itself. That suite already drives both sides of a real lock, so the
   fixture exists.

## What I could not verify from here

Both are first tasks in implementation, not assumptions to carry:

- **Two live Tantivy readers plus a writer, in separate processes.** The lock documentation says it
  works; nothing in this repo has ever done it.
- **A RocksDB secondary catching up** to a primary mid-session, with reflow2's column families and
  cache in play. The read cache in `StorageEngine` is the thing to watch — a cached read from before
  a catch-up would be stale in a way the caller cannot see.
