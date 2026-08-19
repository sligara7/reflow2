# reflow2 feedback (from Claude Code, working the hxm_program design)

Running log from an agent using reflow2 in earnest. Dated entries; newest at the bottom
of each section. Context for all of it: the hxm_program graph, sessions of 2026-08-17
through 2026-08-19 — capture-session passes, PR-review captures, a 12-node DesignRule
batch, TemporalFacts, change events.

## What works well

**2026-08-19**

- **`create_node` merge semantics + the `revision` block.** Getting the replaced value
  echoed back with "no snapshot holds the state it replaced — this reply is the only
  place it appears" is genuinely protective: it made me stop and check that my rewrite
  was a superset of the old text. The opt-in `expected_content_hash` CAS is the right
  default-off design.
- **`loop_status` as the one cheap call.** The capture-session skill's "call loop_status
  before finishing" rhythm works; the call is fast, the counts are legible, and the
  stop-hook nudge ("fires once; stopping again proceeds") is well calibrated — it caught
  a session where I'd edited files without consulting the graph, without becoming a nag.
- **`detect_defects`'s honesty.** The `swept` block (what was examined, which rules ran)
  makes an empty result meaningful. The orphan-fragment defect saying "No mechanical
  repair — linking this node would assert a relationship nobody drew" is exactly the
  right refusal; it routed a judgement to the human instead of guessing.
- **capture-session's six tests.** "Do not weigh importance, apply the tests" is real
  judgment scaffolding — it stopped me from transcribing tutorial Q&A that was already
  deposited in repo files, three sessions in a row, while still catching the one
  chat-only fact each time (a portability verification, a review's measured findings).
- **Batch tools (`create_nodes`/`create_edges`) all-or-nothing.** Wrote 12 rules + 25
  edges in two calls; the "every failure in one round trip, nothing written on any
  failure" contract meant no partial-batch cleanup risk.
- **BM25 `search_design` for dedup-before-write.** Every capture started with a search;
  hits ranked well enough that near-duplicates were found by meaning ("check_alignment
  port PR 76") rather than exact phrasing. The `loop_hint` piggybacked on search results
  is good ambient awareness.
- **`add_change_event`'s inline `affected`.** One call to create the event AND draw the
  CHANGED edges; refusing the whole call when an affected id doesn't exist is right.
- **TemporalFact as the home for dated measurements** (with `basis: measured`) fit
  perfectly for "verified X on date Y" facts, and `ABOUT_ENTITY` hangs them on the
  artifact they concern.

## Friction / bugs

**2026-08-19**

- **Skill-name drift between the advertised list and the server.** The MCP instructions
  (and the CLI slash-command listing) advertise skills named `rules`, `req`, `gaps`,
  `decisions`, `where`, `kpp`, `health`, `debt`… but `get_skill('rules')` fails with
  "no skill named 'rules'. This server carries 20: … governance-proposal … where-am-i …
  kpp-proposal … detect-and-ask …". The user-facing aliases and the served names have
  diverged, and `get_skill` accepts only the latter. Either serve under both names or
  have the error map aliases → canonical ("'rules' is served as 'governance-proposal'").
  As an agent I recovered by proceeding from schema knowledge, but the skill I was told
  to read first was unreachable by its advertised name.
- **`description` is undeclared on ChangeEvent.** Every ChangeEvent write that carries a
  description gets `undeclared: ["description"]`. The property-bag tolerance is great,
  but when the *natural* field for a node type is undeclared, the flag fires on every
  legitimate write and trains the caller to ignore `undeclared` — which defeats its
  typo-catching purpose. Suggest declaring `description` on ChangeEvent (it's clearly
  load-bearing in practice: it's where the reasoning goes).
- **Append-only edits still get the maximum-alarm revision note.** I appended one
  sentence to a long description; the revision block warned that the prior value exists
  nowhere else, with the full prior text echoed. Correct, but a diff-aware note ("new
  value strictly extends the prior") would preserve the alarm's signal for the
  genuinely destructive case.
- **Creating advisory rules mints interrogation debt 1:1.** Writing 12 craft-of-practice
  DesignRules moved unsurfaced gaps 7 → 19 — presumably each rule now owes an
  enforcement-level question. For rules that are knowingly advisory (tricks of the
  trade, not CI gates), a way to state that at creation (e.g. an `enforcement:
  advisory` property that pre-answers the gap) would keep bulk craft-capture from
  flooding the gap queue. The gap is legitimate; the friction is that there was no way
  to answer it in the same breath as creating the node.
- **ChangeEvent `name` has no length steer.** Nothing nudges the caller toward
  short-name-plus-description, so my event names grew into paragraphs (my doing, but a
  schema hint or a soft length warning on `name` would steer better — the search-result
  listing prints the whole name, so long names degrade the listing).

## Use-case observation: reflow2 as a PROGRAM brain, not a project brain

**2026-08-19** (AJ's own framing: "kind of a strange way to be using reflow2… managing
this scattered 'project' that has multiple facets, but that are all inter-related")

The hxm_program graph isn't tracking one codebase's design — it's keeping a *program*
coherent: five repos (hxm_program, hextools, hex-ob, hex-acq-pyepics, the tutorial
repo), two people, a simulated beamline, a PR pipeline, and the owner's own ramp-up.
Three days of heavy agent use says this is less an edge case than it looks:

- **The ontology already fits.** Capabilities that outlive any repo, Contributors with
  authorship/ownership, Verifications naming a *beamline session* rather than a CI job,
  epochs/releases, TRL/MRL gates — that vocabulary is program-shaped. The single-system
  framing lives mostly in adopt/genesis's front door, not in the schema.
- **It held under cross-repo load.** Change events about another repo's PR commits,
  craft DesignRules distilled from a pairing relationship, a portability TemporalFact
  spanning three repos — all found homes, and BM25 finds them by meaning. The graph
  answered "which proposal number?", "what state is the promotion PR chain in?", and
  "what did the last review measure?" across facets in one search each.
- **Strain 1 — people-readiness has no node type.** The program's actual gate includes
  "AJ can independently build/maintain this stack" (it's why the tutorial facet
  exists), but readiness modeling stops at technologies (TRL/MRL). The learning ladder
  ended up smuggled into a TemporalFact's context sentence. A program brain wants
  contributor-readiness (or at least a blessed pattern for it).
- **Strain 2 — facet-blind interrogation.** The gap engine prices a craft-of-practice
  rule and a system requirement identically (see the 12-rule gap flood above).
  Multi-facet graphs want per-facet interrogation defaults.
- **Suggestion:** name this use case in the docs. "One graph, many repos, one program"
  is likely the common case for anyone whose job is integration — and the front-door
  skills (genesis/adopt) could ask "system or program?" up front instead of assuming
  system.

## Ideas

**2026-08-19**

- **A "sighting" mechanism for DesignRules.** The craft-pattern use case wants "this
  rule was observed again in commit X" as a cheap dated append (like a TemporalFact
  hung on the rule) rather than hand-editing the description each time. Pattern
  recurrence is evidence the rule is real; today the graph can't accumulate that
  cheaply.
- **`get_skill` batch or index-with-bodies.** Capture-session sessions read the same
  skill every time; a `get_skill` accepting several names, or an ETag-style "unchanged
  since you last read it" response, would trim the per-session overhead.
- **Scoped loop_status in the stop-hook nudge.** The nudge says "4 files edited, graph
  never consulted" — it would land even better if it could say *which region of the
  design* the edited files map to (via linked artifacts), turning "you owe the loop"
  into "you probably touched cap:X".
