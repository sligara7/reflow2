# Self-host GENESIS — reflow2 running /genesis on reflow2, 2026-07-18

**What:** the `genesis` skill driven end-to-end against reflow2's own repo, through the installed
consumer kit — the real MCP binary, the real `.mcp.json`, the graph at `./.reflow2/graph`, from
Claude Code. Not a harness: the same path a consumer gets.

**Why this is different from [the self-host probe](2026-07-18-selfhost-probe.md).** That one was
read-mostly, modelled in `/tmp` by a throwaway script we wrote, and modelled the design *backwards*
from `requirements-coverage.md` — 119 nodes, already structured. This one runs GENESIS *forwards*,
from a brief, using the shipped skill and a **shipped third-party client**. That last part is where
the one new finding comes from.

**Seeded:** 10 Requirements, 7 Capabilities, 12 SATISFIES, 17 CONTAINS, 0 Components (per the
skill). 18 nodes.

**Scope note, written after the fact.** The first draft of this document claimed five findings.
Four were already recorded in earlier trials, which I had not read before writing. They are kept
below as confirmations with attribution, not as discoveries, and one first-draft claim is retracted
outright. The honest yield of this session is **one new finding (§1) and one retraction (§6)**.
Recording the miss because the failure mode — a fresh trial re-reporting known findings as new —
inflates the evidence base that BL items are argued from.

## What it said

```
18 nodes · 3 gaps · 14 structural defects

  0.70  concept_without_design      ← artifact of seeding order (known: ophyd 1, 3dtictactoe 0+3)
  0.65  build_without_verification  ← same class, second detector (new detail)
  0.60  unsatisfied_requirement     ← on a deploy-context requirement (known: grok finding 9)

   7  orphan_node (capability not allocated)  ← known: ophyd finding 14
   3  disconnected_community                  ← known: ophyd finding 14
   3  single_point_of_failure                 ← known: BL-5
   1  orphan_node (requirement unsatisfied)   ← double-count of gap 3 (known: ophyd 15, 3dtictactoe 10)
```

Nothing in the list was actionable. See §6.

---

## 1. NEW — every `JsonValue` parameter on the tool surface is unusable from this client

The input-side twin of the bug class ophyd found on the output side (findings 10 and 22) and the
grok trial found on array responses (its finding 1). Nobody has looked at the parameters.

Five tool parameters are declared `JsonValue` in `service.rs`. `JsonValue`'s `JsonSchema` impl
emits an untyped `{}`, so the published `inputSchema` gives the client **no type to marshal
against**. Claude Code sends the object as a JSON *string*; `serde_json::from_value` then gets
`Value::String` and rejects it. Confirmed against the advertised `tools/list` — `gap` is the only
property in its schema with no `type`, every sibling has one.

All five, tested over the real MCP path:

| Tool | Param | Required | Error |
|---|---|---|---|
| `gap_to_prompt` | `gap` | yes | `invalid GapCandidate: … expected struct GapCandidate` |
| `apply_heal` | `proposal` | yes | `invalid HealProposal: … expected struct HealProposal` |
| `import_graph` | `document` | yes | `not a reflow2 export: … expected struct GraphExport` |
| `create_node` | `props` | no | `invalid props object: … expected a map` |
| `create_edge` | `props` | no | `invalid props object: … expected a map` |

From Claude Code that removes the **ask half of DETECT**, the **apply half of HEAL**, graph
**restore/migration**, and all property-setting on generic CRUD. Four of the five are named in the
skills the kit installs.

**It is client-dependent, which is the actually interesting part.** The grok trial lists
`gap_to_prompt` among tools that *work* over MCP and used it successfully — grok build marshals a
nested object, Claude Code marshals a string. Both are behaving reasonably against a schema that
declares nothing. So this is not a dead tool; it is **a tool that works on the harness we test on
and fails on the one `req:driving-agent` names first**, with no signal that anything is wrong.
`tools/smoke_mcp.py:295` passes `gap` as a Python dict and is green.

That is a sharper version of the lesson AGENTS.md already draws from three home-grown test layers:
the smoke test is not just *a client we wrote*, it is a client whose marshalling choice happens to
match one of the four supported harnesses.

**Fix:** derive `JsonSchema` on `GapCandidate` / `HealProposal` / `GraphExport` and declare the
fields as those structs; `props` as an explicit map type. Do **not** make the server also accept a
JSON string — that is a silent fallback and violates `req:no-silent-fallback`.

**Test:** ophyd finding 22 already proposes a smoke test asserting every tool's *response* envelope
validates. That proposal does not cover this: these are request schemas, and the failure is in the
client's marshalling, not the server's output. The check that would have caught it is static —
assert no advertised `inputSchema` property lacks a type — and it needs writing separately.

Filed as **BL-28**, widened from `gap_to_prompt` alone to the whole class.

## 2. CONFIRMS ophyd finding 14 — HEAL contradicts GENESIS, now at 18 nodes

Seven of the 14 defects are `Capability 'X' is not allocated to any component`, one per seeded
capability, caused entirely by GENESIS step 2's bolded **do not create Components yet**.

[Ophyd finding 14](2026-07-18-brownfield-ophyd-service.md) already reports this precisely — 15 of
its 35 defects were the same detector restating "0 Components exist," and it already draws the
conclusion that following `check-health` literally would fabricate Components via `generate_owner`.
It proposes suppressing allocation-orphan defects when Component count is 0, or having
`graph_report` label the phase.

What this run adds is only scale and sequencing: it reproduces at **18 nodes** (ophyd was 52), and
it reproduces on the *greenfield* path, so finding 14's proposed fix should not be scoped to
brownfield. The natural skill order genesis → check-health produces it on any project, day one.

One detail ophyd finding 14 could not see, because it declined to run `propose_heal`: on this graph
`propose_heal(balanced)` returns **0 structural operations and 14 awaiting generation**, all
`requires_human_review: true`. Generation is gated on the deferred LLM backends. So
`check-health`'s promise to "apply only the mechanical fixes" has, here, nothing whatever to apply
— the HEAL loop end-to-end yields zero actionable output on a freshly-seeded graph.

## 3. CONFIRMS BL-27 — third target, and a second detector in the same class

The `concept_without_design` at 0.70 outranking the real gap at 0.60 is [BL-27](../backlog.md),
already established by [ophyd finding 1](2026-07-18-brownfield-ophyd-service.md) and [3dtictactoe
finding 3](2026-07-18-brownfield-3dtictactoe.md), which together already upgraded it from an
observation to a property of the path. A third sighting adds little; recording it only because the
target is reflow2 itself.

The one detail worth carrying: **`build_without_verification` (0.65) fires in the same way**, and
no prior trial names it in this role. It reports "no way to confirm any of it actually works" of a
repo with 15 integration-test files, unit tests, doctests, and a smoke test whose green run
AGENTS.md makes a precondition for calling a change done.

Both are `scope: phase` detectors inferring maturity from a node-type census, and both are true
about the *graph* while false about the *system* — 3dtictactoe finding 3's argument, extended one
detector across. The consequence for BL-27 is narrow but real: a fix scoped to
`concept_without_design` alone still leaves the top two-thirds of the first gap list unactionable.

## 4. CONFIRMS aidrone finding 1 and ophyd finding 11 — and closes the last workaround

[Aidrone finding 1](2026-07-18-greenfield-aidrone.md) reports "No way to set node properties at
creation" (everything lands `priority=medium, kind=functional, status=proposed`) and [ophyd finding
11](2026-07-18-brownfield-ophyd-service.md) reports `add_capability` hardcoding `status: planned`.
Both reproduced exactly: all 10 requirements landed `priority: medium`, all 7 capabilities
`planned` — for capabilities that ship in the binary the client is talking to.

The new part is that **the documented escape hatch is closed by §1**. `create_node` takes a `props`
map precisely so arbitrary properties can be set; it is one of the five broken params. Verified:

```
create_node(node_type="Requirement", props={"priority":"critical", …})
→ invalid props object: … expected a map
```

So from Claude Code there is **no path at all** to set a requirement's priority — not the typed
constructor, not the generic one. That matters because `unsatisfied_requirement`'s own evidence
string cites `priority=medium` as ranking input. The severity ordering reads a field no client on
this harness can write, and every value it reads is a default nobody chose.

## 5. CONFIRMS BL-5 / blind trial — structural noise on a young graph

3 `disconnected_community` and 3 `single_point_of_failure` on 18 nodes, all `warning`. The SPOFs
land on `req:coherence` and `req:no-silent-fallback` — load-bearing *because* they are
cross-cutting, so the topology is right and "add redundancy" is wrong.

This is [BL-5](../backlog.md), reported in the blind trial and diagnosed in the [self-host
probe](2026-07-18-selfhost-probe.md), whose "Outcome, same day" section supersedes the probe's own
first guess: the test asks "are there ≥2 non-trivial components *after* removal?", which one island
already satisfies, so every articulation point elsewhere reports. Nothing here refines that. Noted
only to record that it still fires post-BL-5-fix at this size.

The 1 remaining `orphan_node` (requirement satisfied by nothing) is the same fact as gap 3 in HEAL
vocabulary — [ophyd finding 15 / 3dtictactoe finding
10](2026-07-18-brownfield-3dtictactoe.md), DETECT/HEAL
double-counting, reproducing a third time.

## 6. RETRACTED — "the only true gap"

The first draft called the 0.60 `unsatisfied_requirement` on `req:platform` the one true gap, and
built the §3 headline on the contrast. That is wrong.

`req:platform` is a **deployment-context requirement**, created because GENESIS step 3 instructs
the agent to capture platform / driving agent / invocation / persistence as Requirements. [Grok
finding 9](2026-07-18-grok-trial-weather-station.md) already reports exactly this: those context
requirements "felt forced," and each immediately creates an `unsatisfied_requirement` gap until
acknowledged as meta — with a suggestion that they belong in Project properties or a distinct
context node kind.

So the gap is a known artifact of the same skill that created the node, and **all three gaps in
this round were artifacts of seeding, not observations about reflow2**. That is a stronger claim
than the draft's, and it belongs to grok finding 9 and BL-27, not to this trial.

Worth noting the tension it exposes between two prior trials: aidrone finding 1 credits
`unsatisfied_requirement` as the detector that "earned its keep — precise hit, no false positives
across three rounds," while grok finding 9 has it firing on nodes the skill just told the agent to
make. Both are right, about different node populations. Deciding whether deploy-context
requirements are Requirements at all would resolve it, and that decision is unmade.

---

## Verdict

GENESIS's mechanics work: scaffold, seed, contain and detect all ran clean on the shipped path and
the golden thread came out coherent. Of the six sections above, one is new.

**§1 is the finding to act on**, and it is not a brownfield or a genesis finding — it is a
tool-surface bug that silently removes four capabilities from one of the four supported harnesses,
including the ask half of the loop. It was invisible to every prior trial because the two that ran
over real MCP ran on grok build, whose marshalling happens to work.

The rest of this session is evidence that the existing findings reproduce. That is worth something
— ophyd finding 14 and BL-27 both gain a sighting on a third target — but it is not new
information, and the first draft of this document presented it as though it were.

## Repro

```bash
python3 tools/reflow2_init.py .          # already done, commit 98baf40
# then, from Claude Code in this repo:
/genesis
```

Graph left in place at `./.reflow2/graph` (18 nodes, unmodified by the §1 probes — every one of
them failed before mutating). No crate changes made.
