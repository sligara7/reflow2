# What storyflow learned about "are these two the same thing?"

> Part of the **Reflow 2.0** design docs — see **[overview.md](overview.md)** for the full map and
> reading order.

*A prior-art study, in the same shape as [github-mcp-nuggets.md](github-mcp-nuggets.md): what is
worth importing, ranked, and an explicit list of what is not. Read from
`services/dynograph/crates/dynograph-server/src/compat/resolution.rs` (1,112 lines),
`services/dynograph/schemas/domains/core.yaml`, and
`services/generation_plus/src/modules/extraction/graph_informed_resolve.py` on 2026-07-26. Nothing
in storyflow was modified — AGENTS.md rule 6.*

## Why this study exists

`Bob` / `Bobby` / `Robert` / `Robert, The Great` is the same character. `Auth` / `Auth Service` /
`Authentication Service` across forty markdown specs is the same component. **These are one problem**,
and storyflow has been fighting it in production for longer than reflow2 has existed. The user made
the connection; this is what the code actually does.

It bears directly on [scope-corpus-ingest.md](scope-corpus-ingest.md), whose Finding 5 flagged
reflow2's auto-merge rule as needing a decision rather than a default. storyflow has already made
that decision, and made it differently.

---

## The finding that costs nothing to act on

**reflow2's foundation already parses everything needed, and `ingest` ignores it.**

`dynograph-core` — the version reflow2 is *already pinned to* — exposes `ResolutionConfig` on every
node type, with **two** thresholds:

```rust
pub struct ResolutionConfig {
    pub fuzzy_threshold: u32,       // a candidate worth considering
    pub auto_merge_threshold: u32,  // certain enough to act without asking
}
```

`DesignGraph::schema()` is public and already used at `graph.rs:214`, so reading
`def.resolution` is a few lines, no foundation change, no new tag, no rebuild.

Meanwhile `ingest.rs` uses a hardcoded `FUZZY_MATCH_THRESHOLD: u32 = 90` — and reflow2's own schema
declares per-type thresholds of **80–88**. So the constant is not merely ignoring the config, it is
*stricter than every declared value*: **reflow2 currently merges less than its own schema asks for**,
and no reflow2 type declares an `auto_merge_threshold` at all, so the two-band model is unused.

This is the fifth instance of the shape this project keeps finding — declared, and read by nothing.
Unlike the others, the reader already exists upstream.

---

## Worth importing, ranked

### 1. Two thresholds, and a band between them where you *ask*

storyflow declares both per type, and the values genuinely differ by type — `Character` is the
loosest at `70/90` because people have the most name variants; the strictest is `85/98`.

| Band | Meaning |
|---|---|
| below `fuzzy_threshold` | not a candidate; ignore |
| between the two | **a candidate — surface it, do not act** |
| at/above `auto_merge_threshold` | certain enough to merge without asking |

This resolves scope-corpus-ingest's Finding 5 cleanly and *without* violating `dec:ask-not-repair`:
the rule "suspected duplicates are asked, never merged" applies precisely to the middle band, while
the top band is not a suspicion. It also answers BL-42's lesson — a detector that punishes correct
bulk work needs a different question, and "which band is this in?" is that question.

**Import it.** Add `auto_merge_threshold` to reflow2's schema declarations and read both.

### 2. The token-subset pass — the one that would not have been guessed

Their code says it plainly:

> *Foundation's fuzzy resolver is length-sensitive (token_sort + jaro_winkler), so it misses
> token-subset cases like "Nool" vs "Jungle of Nool" — we add a structural pass on top.*

A ratio-based score **falls as the length difference grows**, so the very case a corpus produces most
— a short name in one document and its qualified form in another — scores *below* threshold no
matter how the threshold is tuned. Tuning cannot fix it; it needs a different test:

```
is_token_subset(a, b) = a ⊂ b  and  |a| < |b|
```

For reflow2 that is exactly `Auth` ⊂ `Auth Service` ⊂ `Authentication Service`.

**And the survivor rule is the non-obvious part:** the **longer** name wins, *regardless of edge
count*, because it is the more specific and disambiguating one — the shorter folds into it as an
alias. The naive rule (keep whichever node has more edges) would collapse the specific into the
vague, which is the wrong direction and hard to undo.

**Import it.** This is the highest-value single mechanism here.

### 3. Normalise before comparing

`name_tokens`: lowercase → split on whitespace → strip non-alphanumeric edges → **drop stopwords**
(`the`, `of`, `a`, `an`, `and`, `&`) → drop empties. `canonical_name` then sorts and dedupes them.

`"The Clover"` → `clover`. `"Jungle of Nool"` → `jungle nool`.

Cheap, deterministic, no model involved, and it does most of the work before any scoring happens.
reflow2's `token_sort_ratio` sorts tokens but does **not** strip stopwords, so
`"Authentication of Users"` and `"User Authentication"` compare worse than they should.

**Import it**, with a caveat: reflow2's stopword list should be its own. Design prose is thick with
`service`, `system`, `module`, `component` — and those are exactly the tokens that would over-merge
if stripped. Strip grammar, never domain nouns.

### 4. Record *why* each merge happened

Every candidate and every merge carries a `match_kind` — `fuzzy`, `token_subset`, or `cross_type` —
in both the wire shape and the merge log. When a merge later turns out wrong, the discriminator is
what tells you whether to fix a threshold or a rule.

reflow2 already reports `IngestReport.fuzzy_merges` ("auditable, never silent"); this adds the one
field that makes the audit actionable. **Import it.**

### 5. `SAME_AS` as a link, distinct from merging

storyflow keeps both moves available: **merge** (one node survives, the other's name becomes an
alias) and **`SAME_AS`** (both nodes persist, linked, queryable in both directions). The second is
what you want when two sources describe the same thing but you are not willing to destroy either
record — which is the common case when the sources are *other people's documents*.

reflow2 has the analogous `DUPLICATES` edge and `dec:ask-not-repair` already says to draw the edge
rather than merge. What storyflow adds is the reminder that this is a **permanent, useful state**,
not a staging area on the way to a merge.

### 6. Iterate to a fixpoint, and say so

After each merge the loop restarts, because folding A into B can make B match C. storyflow does this
explicitly. Worth importing as a *stated* property — but see the do-not-import list for the cost.

---

## Explicitly do NOT import

- **`auto_resolve` merging the ambiguous band unattended.** storyflow is a single author's creative
  tool where a wrong merge is an annoyance. reflow2 holds other people's engineering records, where
  silently collapsing `Auth Service` and `Auth Service v2` destroys a distinction someone wrote down
  on purpose. Take the bands; ask in the middle one.
- **The O(n²) pairwise loop restarted after every merge.** Acceptable for one story's cast. For a
  500-document corpus it is the same quadratic wall scope-corpus-ingest already flagged in
  `fuzzy_match`, made worse. Import the *rules*; the traversal needs an index.
- **The story-scoped plumbing** (`story_id`, `list_story_nodes`, per-story entity types). reflow2
  scopes by graph, not by story.
- **The HTTP endpoint shape.** These are axum handlers behind a REST API; reflow2's surface is MCP.
- **Cross-type same-name merging, unmodified.** storyflow treats a `Character` and a `Location`
  sharing a name as a candidate. In reflow2 a `Requirement` and the `Capability` that satisfies it
  routinely share a name *and should* — that is the golden thread working, not a duplicate. If
  imported at all it must be reported, never merged.

---

## What this changes in the corpus-ingest plan

`scope-corpus-ingest.md` listed "where do the auto-merge bands sit" as an open decision needing the
user's call. storyflow answers the *shape* — two thresholds, ask in the middle, per type — which
narrows the question to the numbers. It also adds a mechanism that was not in that scope at all: the
token-subset pass, without which the most common corpus case fails no matter how the numbers are set.

Revised sequencing note for step 3 of that scope ("per-type thresholds and an index"):

1. Read `ResolutionConfig` instead of the hardcoded constant — small, and independently correct.
2. Declare `auto_merge_threshold` per type and implement the ask-band.
3. Add the token-subset pass with the longer-name-survives rule.
4. Stopword normalisation, with a reflow2-specific list that spares domain nouns.
5. `match_kind` on every reported merge.

Steps 1–5 are useful to single-document `ingest` today, before any folder driver exists.

## What I could not verify from here

- **Whether storyflow's thresholds are actually good numbers**, or just numbers that stopped
  complaints. They appear only in schema YAML and test fixtures — I found no code in storyflow
  reading `auto_merge_threshold` either, so the two-band model may be declared there and unimplemented
  as well. The *idea* is sound prior art; the tuning is not evidence.
- **How often the token-subset rule fires wrongly.** `Auth Service` ⊂ `Legacy Auth Service` would
  merge under this rule, and those may be two real, different services. A corpus trial would tell;
  an argument will not.
