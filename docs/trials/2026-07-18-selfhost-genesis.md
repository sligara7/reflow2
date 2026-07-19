# Self-host GENESIS — reflow2 running /genesis on reflow2, 2026-07-18

**What:** the `genesis` skill driven end-to-end against reflow2's own repo, through the installed
consumer kit — the real MCP binary, the real `.mcp.json`, the graph at `./.reflow2/graph`, from
Claude Code. Not a harness: the same path a consumer gets.

**Why this is different from [the self-host probe](2026-07-18-selfhost-probe.md).** That one was
read-mostly, modelled in `/tmp` by a throwaway script we wrote, and it modelled the design
*backwards* from `requirements-coverage.md` — 119 nodes, already structured. This one runs GENESIS
*forwards*, from a brief, on a codebase that already exists, using the shipped skill and the
shipped client. Different question: not "do the detectors scale?" but "does the entry point work?"

**Confound, stated up front:** reflow2 is brownfield, and GENESIS is the greenfield entry point.
Running it here is deliberately off-label — that is the thing under test ([BL-27](../backlog.md)).
Findings 1–2 are about that mismatch and should be read as replication, not new discovery.
Findings 3–5 are not about brownfield at all and would have fired on a greenfield project too.

**Seeded:** 10 Requirements, 7 Capabilities, 12 SATISFIES, 17 CONTAINS, 0 Components (per the
skill). 18 nodes. Requirements derived from `AGENTS.md`'s stated invariants and the deployment
context the skill asks for; capabilities one per loop step plus store/artifacts/install.

## What it said

```
18 nodes · 3 gaps · 14 structural defects

  0.70  concept_without_design      ← false here
  0.65  build_without_verification  ← false here
  0.60  unsatisfied_requirement     ← the only true one, ranked last

   7  orphan_node (capability not allocated)   ← all 7 caused by following GENESIS
   3  disconnected_community
   3  single_point_of_failure
   1  orphan_node (requirement unsatisfied)
```

## Findings

### 1. Third independent replication of the BL-27 gap-ordering inversion — and it is worse than recorded

[BL-27](../backlog.md) records that in brownfield, `concept_without_design` fires at 0.7, above
the genuinely valuable gap at 0.6, so an agent working top-down does the useless thing first. That
reproduced here exactly, on a third codebase, at a third size — after ophyd-service (399 files)
and 3dtictactoe (~20). It is a property of the path, confirmed.

What is **new**: BL-27 names one detector. Two fire this way. `build_without_verification` (0.65)
says "there's a design/build, but no way to confirm any of it actually works" — of a repo with 15
integration-test files, unit tests, doctests, and a smoke test, whose AGENTS.md makes a green test
run a precondition for calling a change done. Both false gaps outrank the one true gap.

So the top **two-thirds** of the list is wrong, not the top item. A fix scoped to
`concept_without_design` alone would leave the agent's first action still useless. Both are
`scope: phase` detectors that infer project maturity from node-type census — that shared inference
is the defect, not either detector's wording or severity.

### 2. HEAL contradicts GENESIS, and check-health is the skill that surfaces it

Seven of the 14 structural defects are `Capability 'X' is not allocated to any component` — one
for every capability seeded. They exist because GENESIS step 2 says, in bold, **do not create
Components yet**, deliberately, so that `concept_without_design` fires as the productive first
question.

The two skills ship together and the natural order is genesis → check-health. Run that order and
HEAL reports seven warnings against a graph that is exactly what GENESIS instructed. An agent that
believes its tools will start allocating components to silence them — which is the guess GENESIS
stopped it from making. An agent that doesn't will learn that HEAL warnings are ignorable, which is
worse and permanent.

Not a brownfield finding: this fires on any project on day one.

### 3. `gap_to_prompt` is unusable from a real MCP client — the DETECT→ASK handshake is broken

Blocker. Calling it from Claude Code fails on both a JSON-string and an object argument:

```
MCP error -32602: invalid GapCandidate: invalid type: string "{...}", expected struct GapCandidate
```

Root cause, `crates/reflow2-mcp/src/service.rs:621`:

```rust
pub gap: JsonValue,
```

`JsonValue`'s `JsonSchema` impl emits an untyped `{}` — the published tool schema declares no type
for `gap`. A client with no type to marshal against sends the nested object as a **string**;
`serde_json::from_value` at `service.rs:1504` then gets `Value::String` and rejects it. Nothing the
caller can do from the client side.

This is the single most load-bearing tool in the loop. `detect_gaps` finds gaps; `gap_to_prompt` is
how they become questions for the human — the whole of `req:human-decides`, and the entire second
half of the `detect-and-ask` skill. It has presumably never worked from Claude Code.

**And it is the exact failure AGENTS.md warns about**, four lines under "Compiling is not the finish
line": *"three home-grown test layers once agreed with each other and were all wrong because each
was a client we wrote."* `tools/smoke_mcp.py:295` passes `gap` as a Python dict, which serializes to
a JSON object, which works. The smoke test is green. It is a client we wrote.

Fix is to give `gap` a real type — derive `JsonSchema` on `GapCandidate` and declare the field as
that struct — not to make the server also accept strings, which would be a silent fallback and
violates `req:no-silent-fallback`. The smoke test cannot catch the regression; the tool schema
needs asserting directly (there is precedent — `smoke_mcp.py:148` already inspects
`reconcile_artifacts`'s `inputSchema`).

### 4. The typed constructors cannot express what GENESIS is told to capture

`add_requirement` takes only `id`, `name`, `statement`. All ten requirements landed
`priority: medium`, `status: proposed`, `concern: core`. `add_capability` takes only `id`, `name`,
`description`; all seven landed `status: planned`.

Two consequences, one per direction:

- **Forwards (greenfield):** GENESIS step 3 says to capture deployment context — platform, driving
  agent, invocation, persistence — because these "ripple into everything." They were captured as
  requirements indistinguishable in priority from anything else. `unsatisfied_requirement`'s own
  evidence string cites `priority=medium` as if it were a signal; here it is a default nobody set.
  Severity ranking reads a field the seeding tools cannot write.
- **Backwards (brownfield):** every capability reflow2 actually ships is recorded as `planned`.
  There is no way through the typed surface to say "this exists and works," which is most of what
  is true about an existing system.

### 5. Structural defects on a young graph are indistinguishable from real ones

3 `disconnected_community` and 3 `single_point_of_failure` on an 18-node graph, all `warning`.
The disconnected clusters are just requirement/capability pairs not yet joined to the rest — the
normal shape of a graph seeded an hour ago. The SPOFs are on `req:coherence` and
`req:no-silent-fallback`, which are load-bearing *because they are cross-cutting*: the topology
reading is correct and the engineering conclusion ("add redundancy") is wrong.

`structure.rs` is already documented as selective about SPOF to avoid exactly this class of noise
on tree-shaped threads. That selectivity does not extend to graph age. Related to finding 1 — both
are detectors inferring from a census without knowing what phase the design is in — but distinct:
finding 1 is ordering, this is a floor on graph size below which topology means little.

## Verdict

GENESIS's mechanics work: scaffold, seed, contain, and detect all ran clean on the shipped path,
and the golden thread came out coherent. The findings are on either side of it — what it tells the
agent to do next (1, 2, 5) and what it can record while doing it (4).

Finding 3 is the one to fix first, and it is not a brownfield finding: the ask half of the loop
does not work from Claude Code at all, and our own test client is why we did not know.

## Repro

```bash
python3 tools/reflow2_init.py .          # already done, commit 98baf40
# then, from Claude Code in this repo:
/genesis
```

Graph left in place at `./.reflow2/graph` (18 nodes) for follow-up. No crate changes made.
