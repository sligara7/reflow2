# Upgrading to v0.27.0

**This doc exists because the schema stamp moved.** v0.26.1 stamped 60 edge types; v0.27.0 stamps
**61**. One edge type was added — `OWNED_BY` — and none was removed. Node types are unchanged at 29,
and `schema_version` is still `1`.

## What you must do

**Nothing, for your graph.** No existing node or edge is reinterpreted, nothing is migrated, and an
export written by v0.26.1 imports into v0.27.0 unchanged. A stamp movement is not automatically a
migration; this one is purely additive.

**Something, if you call the content tools.** Four served tools were removed and calls to them now
fail — see below.

## The one thing that will break a caller

`content_put`, `content_get`, `content_exists` and `content_manifest` **no longer exist.**

If your agent or script calls any of them, the call fails with an unknown-tool error rather than
degrading. There is no shim and no deprecation window, which is a deliberate choice for a pre-1.0
tool nobody was using: across three projects and the whole retained call sample, these four tools
were invoked **zero times**.

**What to use instead:** keep the file in your repository and register it as an `Artifact` with
`add_artifact` / `link_artifact`, then let `set_artifact_checksum` and `reconcile_artifacts` tell
you when it drifts. Git is already a content-addressed store, so the design records *where the
content lives* rather than holding a second copy of it.

**`ingest_step` and `ingest_corpus_step` are NOT affected.** They lived in the same source file and
moved with its deletion to `ingest_tools.rs`; their schemas are byte-identical to v0.26.1 and the
corpus-ingest handshake is unchanged. This is worth stating explicitly because deleting that file
*did* briefly remove them during development, and the toolsnap gate is what caught it.

## The new edge: `OWNED_BY`

`OWNED_BY` records **whose area a node is** — durable, standing, and never released.

reflow2 now has three distinct "who" edges, and the distinction is the point:

| Edge | Question it answers | Lifetime |
|---|---|---|
| `AUTHORED_BY` | Who *wrote* this? | Historical; never changes |
| `CLAIMS` | Who is *in* it right now? | Transient, advisory, released at checkout |
| `OWNED_BY` | Whose *ground* is this? | Standing; survives every session |

The case it was built for is two people splitting one design — "there are areas that are his and
areas that are mine" — which is ordinary collaborative work rather than anything multi-agent.

**How to use it:** call `owned_by` with the node, the `Contributor`, an optional `note` saying what
is actually owned, and an optional `since` date. Then `loop_status(contributor_id)` reports
`gaps_on_owned_ground` — the debt that sits on your ground rather than the whole design's.

**Two deliberate non-behaviours, so absence is not mistaken for a bug:**

- **It is not a traceability edge.** Like `AUTHORED_BY` and `CLAIMS`, it is absent from the impact
  table, so ownership never propagates a blast radius and a `Contributor` never becomes a hub in
  the design network. Owning something says who answers for it — not that changing it changes them.
- **Unowned nodes are not reported as a gap.** Most nodes in a mature design legitimately have no
  owner. Whether *ownership orphans* should be detected — well-connected, realized, shipping, and
  nobody accountable — is an open question in the design, not an oversight.

## Checking you are on it

```
reflow2-mcp --version          # 0.27.0
```

The server also reports its own currency: if `served_by.stale` is true, the binary you are talking
to is no longer the code it was started from, and you should restart it before trusting anything it
says about the schema.
