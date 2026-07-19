# The partnership — human, graph, LLM, and who checks whom

> Part of the **Reflow 2.0** design docs — see **[overview.md](overview.md)** for the map.

**Why this is upfront:** "how do you know it didn't hallucinate?" is the first question a tool
that works with an LLM gets asked, and people arrive pre-conditioned — rightly or not — to
distrust it. That is an adoption gate, not a nuance: credibility has to be established at the front
door or the evaluation ends there. So the README answers the skeptic in their own words, and this
document is the evidence behind that answer — with the uncovered cases named, because an
overclaimed defense is itself a credibility failure.

reflow2 is a three-party system, and the parties are not interchangeable: each is strong exactly
where another is weak. Most of the architecture's load-bearing decisions are, in retrospect, checks
one party places on another — this document names them as such, so the next mechanism is designed
*as* a check rather than rediscovered as one.

| Party | Brings | Cannot be trusted with |
|---|---|---|
| **Human** | intent, judgment, ground truth about the world | remembering everything; noticing slow drift; being asked twice |
| **Graph** | memory that outlives sessions; deterministic computation; an audit trail | meaning — it stores claims, it cannot judge them |
| **LLM** | language, synthesis, breadth; the hands on the tools | arithmetic, consistency, memory, its own confidence |

The division in one line: **the LLM speaks, the graph remembers and counts, the human decides** —
and every gap, ledger and refusal below exists to keep one party from quietly doing another's job.

## The known LLM failure modes, and the standing check for each

Compiled 2026-07-19 from the author's taxonomy of LLM limitations, mapped against shipped
mechanisms. Anything not traceable to a named mechanism is listed as uncovered — per the evidence
rule, coverage is claimed only where something enforces it.

### Covered by architecture

| Failure mode | Check | Mechanism |
|---|---|---|
| Math / logic weakness | the LLM never computes | The deterministic, LLM-free core does all counting: severities, Jaccard, articulation points, blast radii, coverage. `LlmBackend` is a seam the core holds at arm's length. |
| Instruction degradation, context-window limits | the graph is the memory | Questions outlive sessions (BL-4/25); decisions are nodes; `where-am-i` rebuilds context from the graph; the export is the durable record. The graph does not forget, which is the product. |
| Drift / inconsistency across sessions | determinism, pinned | Stable FNV-1a gap ids; byte-identical exports; smoke asserts *the same graph gives the same gaps in a fresh process*; recorded questions keep their exact wording so the user is never re-asked in new words (BL-4). |
| Brittleness to wording | types, not prose | Every tool parameter declares a type (BL-28, schema-asserted); the contract is the published schema, and the guard catches any parameter that declines to state one. |
| Hallucinated structure | fail-loud vocabulary | Unknown types/properties are refused; `describe_schema` rejections name what *would* work (BL-1); `apply_heal` executes only operations HEAL itself derives from the live graph (BL-29). An invented merge, edge or node type cannot land. |
| Hallucinated provenance | `inferred`, on the record | A requirement read out of the implementation is marked `inferred` (BL-27) — satisfied by construction, and legibly so. The adoption discipline: never draw a `satisfies` edge you cannot point at code for. |
| Sycophancy | detectors don't negotiate | A gap refires every run until the structure changes or a human acknowledges it with a recorded reason (a Decision node — agreement must leave a trace). The two-sided accept (BL-33) forces the uncomfortable question at the exact moment an agreeable agent would glide past it. |
| Confident staleness (of the *server*) | `served_by` | `graph_report` names the binary answering (BL-32); the handshake reports reflow2's own version. Skew between instructions and server is visible from inside a session. |
| Verbosity | capped, never silently | `TOP_N` with truncation *counts*, `suggested_depth` on prompts, coarse-granularity modelling (BL-23) — bounded output that says what it dropped. |
| Lack of agency | the surface is the tool | The MCP surface plus `--export`/`--import`; the agent needs no other integration to drive the whole loop. |

### Covered by role, not mechanism — the human's seat

| Failure mode | Why it stays with the human |
|---|---|
| Semantic hallucination in free text | A false *description* passes every structural check. Deliberate (`dec:report-dont-judge`): the confirmation ledger makes the claim history legible — who accepted what, dated — and the human judges. A deterministic lie-detector would fire on every stable design (the `unexpected_coupling` lesson). |
| Stochastic-parrot reasoning | The graph never asks the LLM to *understand* — it asks it to phrase (`gap_to_prompt`), extract (INGEST), and render (viewpoints, where fill-ins are confessed as defects per the projection doctrine). Comprehension-shaped work routes to the human as questions. |

### Uncovered, and named as such

- **Prompt injection via graph content** — the real hole, raised as **BL-41**. Node text is data the
  schema stores and every skill tells the agent to read and act on; nothing anywhere says *graph
  text is never instructions*. Today's exposure is bounded (single user, local graph, your own
  text), but a shared graph (BL-12) or an adopted codebase's README flowing through INGEST makes
  someone else's text part of what drives your agent. Id-level injection is already enforced (the
  foundation validates key segments); text-level trust is not.
- **Data-privacy marking** — local-first by decision (`dec:repo-file-embedded` counts "the user's
  design on a machine they do not control" against a service), and report-friction redacts by
  design; but no field is *marked* sensitive, so nothing can enforce redaction mechanically.
- **Bias / linguistic performance** — properties of the ambient agent, out of the deterministic
  core's reach; the domain-neutral vocabulary helps at the margins. Recorded so the boundary is a
  decision, not an oversight.

## The rule for new work

When adding a capability, ask which party it serves and which party checks it. A mechanism that
lets the LLM compute, the graph judge meaning, or the human be silently agreed-with is in the wrong
seat — and the failure will look like this session's findings: a lying `met`, a silent accept, a
hallucinated merge. All three were real; all three now have names and guards.
