# Backlog — what's open, and why

> Part of the **Reflow 2.0** design docs — see **[overview.md](overview.md)** for the map.

Three records, three questions:

| Record | Answers |
|---|---|
| [requirements-coverage.md](requirements-coverage.md) | *Are we meeting the docs?* — requirement → module → test |
| [../CHANGELOG.md](../CHANGELOG.md) | *What changed, and when?* |
| **this file** | *What should we do next, and on what evidence?* |

Items point at their source rather than restating it. Sizes are rough: **S** ≈ an afternoon,
**M** ≈ a day or two, **L** ≈ a real project.

Each item has a stable id (**BL-n**). Claim one on the board in
[../COORD.md](../COORD.md) before starting, so two people don't build the same thing.

## Evidence base

Nine independent sources, which is why several items appear on more than one list:

- **Blind trial, 2026-07-18** — an agent with no knowledge of reflow2's source designed and
  built a weather station through the consumer kit. Its friction log is the single richest
  source of findings we have; quotes below are its words.
- **Grok via opencode, 2026-07-18** — a second blind trial, different model *and* harness. Found
  the `structuredContent` array bug that three home-grown test layers all missed, because every
  one of them was a client we wrote. Notes: [trials/](trials/).
- **macOS / grok build, 2026-07-18** — first real external user. Reached the design loop and
  asked for things the trial agent could not (it had no continuity across sessions to miss).
- **Self-host probe, 2026-07-18** — reflow2's own design (119 nodes) pushed into a reflow2 graph
  and interrogated. The first test above fixture scale, and the only one where we know the right
  answer. Notes: [trials/2026-07-18-selfhost-probe.md](trials/2026-07-18-selfhost-probe.md).
- **Brownfield trials, 2026-07-18** — reflow2 pointed at two systems that already existed:
  `ophyd-service` (private trial record) (399 files, ~110k LOC, requirements
  inferred backward from code) and `3dtictactoe` (private trial record)
  (~20 files, no spec at all — the pure-inference case). The only source for BL-27, and the two
  independently reproduce the same entry-point finding at a 20× size difference.
- **Self-host genesis, 2026-07-18** — `/genesis` run on reflow2 itself through the installed kit,
  from **Claude Code** rather than grok build. The only source for BL-28: the harness difference is
  what exposed it. Otherwise mostly a replication of findings above, and its
  [notes](trials/2026-07-18-selfhost-genesis.md) mark which is which.
- **Self-host functional design, 2026-07-19** — the first *durable* design graph of reflow2, 96 nodes,
  committed as a deterministic export at [design/reflow2.json](design/reflow2.json) and analysed with
  reflow2's own surface. Independently rediscovered five open backlog items, and found two detector
  defects. Notes:
  [trials/2026-07-19-selfhost-functional-design.md](trials/2026-07-19-selfhost-functional-design.md);
  re-runnable via `tools/build_design_graph.py --analyse-only`.
- **Erosion trial, 2026-07-19** — the sharper half of the one below, and the closest thing we have
  to a reproduction of how the original reflow failed. Five rounds of *test fails → fix code →
  accept drift* on a coherent thread, then a release: afterwards the design describes a system that
  no longer exists and reports **zero gaps**. The only source for BL-33/34. Notes:
  [trials/2026-07-19-erosion.md](trials/2026-07-19-erosion.md); re-runnable via
  `tools/erosion_trial.py`.
- **Phase-coverage trial, 2026-07-19** — the first trial to go past P2. reflow2's own design carried
  through realization, verification and deploy, with divergences injected on purpose at each phase.
  Scored **P3 4/4, P4 1/4, P5 0/2, traceability 3/3**. The only source for BL-30/31/32 and the first
  execution evidence for BL-9. Notes: [trials/2026-07-19-phase-coverage.md](trials/2026-07-19-phase-coverage.md);
  re-runnable via `tools/phase_trial.py`.
- **[reflow-audit.md](reflow-audit.md)** — the original Reflow's workflows and tools, with
  adopt/obsolete verdicts.

- **Adopt trial on storyflow, 2026-07-20** — the first real exercise of the `adopt` skill, and the
  largest system reflow2 has been pointed at: 2,643 source files across 8 services, with a
  *separate* 979-note design corpus (`~/dev_storyflow`) as the intent source — the exact division
  reflow2's doctrine assumes and no trial had ever tested. The first trial to perform **dynamic
  analysis** (suites really run; a genuinely failing test found). Produced five true findings about
  storyflow its own notes do not state, and four about reflow2 — including the two that became
  BL-42 and BL-43. Notes:
  `2026-07-20-adopt-storyflow.md` (private trial record); the resulting
  graph is committed at `2026-07-20-storyflow-adopted.json` (private trial record).
- **Coherent-erosion trial, 2026-07-19** — the constructive counterpart: the same five fix cycles run
  *with* axis-Z discipline, the design following the build backwards. `designed == released` is
  reachable today and the original intent survives in a Snapshot — but reflow2 returns the **same
  verdict** for this graph and the eroded one. The only source for BL-35/36. Notes:
  [trials/2026-07-19-coherent-erosion.md](trials/2026-07-19-coherent-erosion.md); re-runnable via
  `tools/coherent_erosion_trial.py`.

> **How to weigh any of this: [sharpening.md](sharpening.md).** It records where findings actually
> come from (reflow2's own output contributed to 2 of the 12 items raised on 2026-07-19, and both
> required already knowing the answer), and the failure mode that would quietly invalidate the whole
> evidence base — shaping the model until the tool goes quiet.
>
> **A bias worth naming.** Every source above except the three 2026-07-19 trials stops at or before
> **P2**. The blind trials, both brownfield trials and both self-host runs all end at structure and
> allocation — so until 2026-07-19 the entire evidence base came from the phases the original reflow
> was *already good at*, and none from the phases where it failed. Weigh accordingly when an item
> cites "three independent trials": that usually means three independent trials *of the front half*.

## Next up

| ID | Item | Why | Size |
|---|---|---|---|
| **BL-114** | **A detector's message claims more than the detector checked** | *Found 2026-07-31 by an adopt pass over a real design-review document in another organisation — a pre-construction design with no code — and independently reproduced in reflow2's own graph the same day. Redacted of all source specifics.* **Two detectors report a narrow check in wide words.** `disconnected_component` said *"component is not connected to anything"* about a component that had a `CONTAINS` parent and siblings in the same subsystem; it meant *no runtime path*. `orphan_node` said a document Artifact *"realizes nothing"* — true and irrelevant: it is a specification, it `DOCUMENTS` the project, and `REALIZES` is the only edge the detector counts. A 63-node cluster was likewise reported *"disconnected from the rest of the design"* while its parent subsystems carried `DEPENDS_ON` edges outward and everything shared one Project. **In every case the machinery was right and the sentence was wrong.** **Reproduced here the same day:** `disconnected_community` fired on a Requirement that had just gained an acknowledgement Decision, reporting an island for two nodes sharing the Project — the [orphan-requirements-hide-from-heal] shape from the other side. **This is not a detector bug; it is the class `req:schema-prose-is-checked` already names** — prose that promises breadth the code does not implement, which is exactly why `Project.mode`'s description was fixed at the source rather than annotated. Same defect, different surface: there it was schema prose, here it is finding prose. **Fix shape:** every finding states the edge kinds it considered — *"no runtime (PROVIDES/CONSUMES/DEPENDS_ON) path; containment edges exist but do not count"*. Cheap, changes no computation, and it is the difference between a finding a user acts on and one they learn to distrust. **The `orphan_node` half may additionally be a real rule gap:** `DOCUMENTS` arguably *is* ownership for `artifact_type` of `document`/`spec`/`diagram`, and if so the detector should say so rather than every design document reading as an orphan forever. Decide that separately from the wording. **THE CONTROL EXPERIMENT THIS ITEM LACKED, supplied 2026-07-31 by a case study designed end to end through reflow2:** `propagate_change` returned `impacted: []` for a change that materially activated a regulatory Constraint sitting three nodes away — correct given the edges, badly wrong about the world, because the CONSTRAINS edge had never been drawn. Later in the SAME session, same tool, ~16 nodes both times, with edges present, it produced a genuinely valuable answer with correct upstream hop chains that caught two requirements about to be orphaned. **Edge density, not graph size, decided whether the tool was useful — and the output is identical for "nothing depends on this" and "you never drew the edges", which is the overwhelmingly common case early in a design's life, i.e. exactly when users decide whether to trust the feature.** Same disease as this item: the machinery was right and the sentence claimed more than it checked. **Fix, one line:** when few or no nodes are impacted, report the seeds' connectivity — *"0 impacted; note these seeds carry 2 edges total, so this design may be under-linked rather than genuinely isolated"*. **The data already existed:** `detect_defects` had ALREADY flagged that exact node set as a disconnected community in the same session. Two detectors knew and neither told the tool that mattered. **INDEPENDENT CORROBORATION OF THIS ITEM'S PROPOSED FIX, arrived at from the other end (2026-07-31, same case study):** `disconnected_community`'s SUGGESTION does not know about the detector's own SELECTIVITY. Connectivity is measured over traceability edges; `MITIGATES`/`RISKS`/`CONTRADICTS` do not participate. So a user told to bridge the island draws a TRUE edge of a type that does not count, re-runs, and gets **nothing back** — not "closer", not "that edge does not count", nothing. The only edge types that would clear the finding are ones that would be FALSE for that cluster. **The advice's only correct implementations are excluded from the measurement.** The underlying position is right — an unselective topology detector fires everywhere and means nothing, and `structure.rs` says so — but the message never mentions it. This is exactly the fix already proposed above, reached independently: *state the edge kinds considered*. **Plus a refinement worth taking:** if the island already carries non-traceability edges outward, say so — *"semantically connected to the design but structurally isolated"* is a completely different finding from *"isolated"*, and in that project it was the true one. Size **S** | S |
| **BL-115** | **`unverified_capability` and `unrealized_capability` are unusable on a pre-construction design** | *Found 2026-07-31 by the same external adopt pass — 23 of 51 gaps were `unverified_capability` and another 23 `unrealized_capability`, on a baseline where NOTHING is built yet and that is the honest state. Independently confirmed in reflow2's own graph the same day, where four `unverified_capability` gaps were the ONLY notes on the gate and Anthony accepted all four as noise in one judgement.* **Two sessions, different designs, same verdict — that is a defect, not a preference.** The detector raises one alarm per capability because its verification is `status=planned` rather than `passing`, which is correct and useless: 23 identical alarms drown the four gaps that actually need a human, and a gap list that can never approach zero gets skimmed. `dec:passing-is-verified` is right and must not be weakened — a check that has not passed is not proof — but *"nothing proves this"* and *"a proof is written and has not run yet"* are different states, and today they render identically. **Fix shapes, none chosen:** (a) a third reported state, *planned verification exists, not yet run*, which is honest and costs no new vocabulary; (b) a ROLLUP — *"23 capabilities await their first verification run"* — with the same aggregate keying `dec:aggregate-gap-keyed-on-rule` already built for `unvalidated_capability`, which is the nearest working sibling and was built for precisely this churn; (c) severity scaled by whether a check exists at all. **The rollup route has a known trap:** an aggregate acknowledgement covers capabilities added later without a fresh look, which is what *standing* means and was accepted deliberately once — accept it again knowingly or not at all. Size **S–M** | S–M |
| **BL-116** | **`kpp_contradicted` arrives as one uncheckable blob** | *Found 2026-07-31 by the same external adopt pass.* A single KPP's check listed **12 accepted decisions** in one gap — *"confirm each still leaves it intact"* — with no way to work through them or to record *"checked, intact"* per decision. As one item it cannot be ticked off, so the only available actions are accept-the-whole-thing or leave it open forever. **Same root as [BL-115] one level down: the unit the detector REPORTS in is not the unit a human ACTS in.** The KPP machinery is right — an inviolable requirement genuinely is threatened by any of twelve decisions and saying so is the point — but a finding a human cannot discharge incrementally is a finding they discharge wholesale, which is the opposite of what a KPP deserves. **Fix shape:** per-decision sub-items, or a recorded per-decision disposition so the gap shrinks as it is worked. Note the tension with `dec:aggregate-gap-keyed-on-rule`, which moved in the other direction for a different reason — aggregates whose affected set IS the population want one key; this one wants twelve. Size **S** | S |
| **BL-117** | **The import/export document shape is undiscoverable without reverse-engineering it** | *Found 2026-07-31 by the same external adopt pass, which had to create a throwaway node and edge and re-export the graph purely to learn the encoding.* An export of an EMPTY graph shows the envelope and nothing about per-node or per-edge shape, so an agent building a document by hand — which the `adopt` skill explicitly recommends, *"build one export document and `import_graph` it once"* — cannot learn `{node_id, node_type, properties}` / `{edge_type, from_id, to_id, properties}` from any served surface. **The skill recommends a path the tools do not document.** Two wasted round trips, and worse for anyone without a scratch graph to burn. **Fix shape:** a document-format section in `describe_schema`, or in `import_graph`'s own description — which is also `cap:tool-carries-convention`'s exact argument, a convention an agent would never guess arriving with the call. Size **XS** | XS |
| **BL-118** | **`import_graph` validation is fail-fast, one error per attempt** | *Found 2026-07-31 by the same external adopt pass: FOUR consecutive imports each failed on the first error found — a missing stamp field, then three different enum violations — over a hand-authored 9,000-line document.* Every attempt is a full edit-retry cycle for one error, so a document with N faults costs N round trips. **Atomicity is not the problem and must not be touched** — the same session named it as one of the things that worked notably well, nothing half-loaded across four failures. The problem is that validation stops at the first fault when it could collect them all. **Fix shape:** validate the whole document and report every violation in one response, keeping the all-or-nothing write. Four round trips become one. Size **S** | S |
| **BL-119** | **The stamp requirement is wrong for a hand-authored import document** | *Found 2026-07-31 by the same external adopt pass: rejected with "missing field `node_types` at line 9001".* For a round-tripped export, demanding the full type-name inventory is right — it is what `dec:stamp` exists for, and BL-19/BL-94's lockout depends on it. For a document an AGENT builds, it means copying boilerplate the server already knows, and the `adopt` skill recommends exactly that path. **The two cases are genuinely different and the code does not distinguish them.** **Fix shape:** make the stamp optional on import, or accept a minimal one (`schema_version` alone), and validate the content against the LIVE schema — which is the stronger check anyway, since a hand-copied inventory proves nothing about the document beneath it. **Constraint that must hold:** a document that DOES carry a full stamp must still be refused when it disagrees, or the lockout guarantee is gone. Size **S** | S |
| **BL-120** | **Enum vocabularies do not cover adoption or schedule semantics** | *Found 2026-07-31 by the same external adopt pass; three concrete gaps, each with a workaround that misstates the record.* **(a)** `YIELDED.action` allows `created|updated|merged` only. An adopt pass wants **recovered** — the extraction read it out of an existing artifact, it was not created — so `created` had to be written, and recovered content now claims authorship it does not have. That is the same honesty line `dec:certainty-derived` draws for requirement status, unpoliced one layer down. **(b)** `DEPENDS_ON.dependency_type` allows `function_call|data_flow|control_flow|error_flow|physical` — all runtime. There is no way to type a SCHEDULE predecessor (a success criterion depending on a milestone), so those edges were left untyped. A `temporal`/`schedule` value would fit, though note `PRECEDES` already exists for DesignEpoch→DesignEpoch and widening THAT may be the better answer — decide which, do not add both. **(c)** `DEPENDS_ON` has **no `note` property** while `CONTAINS`, `GOVERNED_BY` and `ALLOCATED_TO` all do, so the stated NATURE of each dependency had nowhere to live. That last one is an inconsistency rather than a design position — but it is still a schema change and still moves the stamp, so it wants an increment, not a drive-by. **(d) ADDED 2026-07-31, the fourth instance and the most recurrent friction in a whole case study (hit in four separate entries): `provenance` has no value for "imposed by an external authority".** The values are `authored / planned / inferred / healed / reconciled / imported`. Four requirements read out of a DARPA solicitation were never said by the user and are not negotiable; they were recorded as `inferred`, whose own documentation says such a requirement is *"satisfied by construction and cannot contradict anything"* — the exact opposite of true for a compliance requirement. **So the graph records something false in order to record something important.** Neither adjacent field fits: `designation` is internal/published, `lineage` is where the need arose. **This is not niche — it is every regulated, government, safety-critical or contract-driven project, which is a large share of the work that most needs a design graph.** A `mandated`/`external` value also carries distinct semantics worth having: it cannot be dropped unilaterally and should probably be immune to healing. **Separable sub-case, same root:** there is no way to record *"the user agreed to MY phrasing"* versus *"the user said this"* — both land as `authored`, and three of that project's requirements are the former with no way for a reader to tell. Size **S** | S |
| **BL-121** | **`add_component` silently defaults `provenance` to `authored`** | *Found 2026-07-31 by the same external adopt pass.* For an ADOPTION, `inferred`/`imported` is the honest default, and a silent `authored` lets recovered content masquerade as stakeholder-stated — the precise confusion `dec:certainty-derived` exists to prevent for requirements, arriving through a component-shaped door. **Fix shapes:** require `provenance` explicitly, or default it to `inferred` when the project was created by an adopt flow. **The second is more convenient and more dangerous** — it makes the honest answer depend on ambient state rather than on what the caller said, which is the shape of a silent fallback. Prefer the explicit demand unless someone argues the other way on the record. **Relates to [BL-120](a)** — same failure, different property, and worth fixing together so provenance honesty is one change rather than two. Size **XS** | XS |
| **BL-122** | **Nothing detects a Release with no epoch** | *Found by me, 2026-07-31, while building `cap:changelog-view` — and the release it was found on had been cut four hours earlier in the same session.* `rel:v0190` had no `AT_EPOCH` edge to `epoch:v0190`. The epoch node existed, the naming convention matched, and **nothing anywhere reported the absence**, so the changelog window for the most recent release — the first anyone would ask about — had no lower bound and silently widened to the beginning of the design. **16 of 20 releases carried the edge; v0.17.0 is still missing it to this day**, and `v0.18.0`'s own commit message boasts that *"v0.17.0 was cut without an epoch or deploy/retire bookkeeping, which this cut does not repeat"* — a claim nothing verified, made about a fault nothing detects, immediately before the same fault recurred. **The invisibility is the finding:** a matching name and an existing epoch node make a missing edge look exactly like a present one to every reader, human or otherwise. **Fix shape:** a gap or defect for a `Release` with no `AT_EPOCH` — cheap, and `changelog_view` already has to report the condition, so the computation exists and only the detector rung is missing. Same recurring shape as [BL-107] and [BL-110]: a designed dependency that nothing validates. Size **XS** | XS |
| **BL-123** | **The `/mcp` reconnect is the one step of an autonomous adopt that an agent cannot take** | *Found 2026-07-31 by the same external adopt pass, and already known here as a recurring trap.* After `reflow2_start_design`, the design tools appear only once the USER manually reconnects — documented and explained well, and still the single point where an otherwise autonomous flow stalls on a human. **Worth recording precisely because it may not be reflow2's to fix:** whether a stdio server can signal a harness to refresh its tool list is a property of the harness, not of this codebase, and the honest answer may be that the start tool should hot-serve the surface in-process instead. Related to [BL-113], which is the same stale-server problem in its silent form. **Do not close this by documenting it better** — it is already documented well, and the session that hit it said so. Size **M, mostly unknown** | M |
| **BL-124** | **`acknowledge_defect` can never close a `disconnected_community`, and each attempt writes a junk Decision** | *Found 2026-07-31 by a case-study project designed end-to-end through reflow2 (`quantum_benchmarking_initiative`), confirmed in this codebase and reproduced live in that graph — the cluster has grown 8 -> 9 -> 10 nodes across attempts.* **The two documented behaviours are individually right and jointly fatal.** `acknowledge_defect` (`crates/reflow2-core/src/heal.rs:490`) creates the review Decision and then draws a `GOVERNED_BY` edge from *every* affected node to it, deliberately, so "the review is reachable from the design". `disconnected_community` (`heal.rs:~734`) computes its id by hashing the category with the affected set. For most categories that is fine, because the acknowledgement does not alter what the detector measures. **For this one it measures precisely what the acknowledgement modifies: cluster membership.** The Decision joins the island it acknowledges, enlarging it by one, minting a new id, resurrecting the defect one node larger — without limit. **So an entire category is permanently unclosable, and the failure is the exact one `acknowledge_defect` exists to prevent** — its own docs say the point is that "a list that can never reach zero gets skimmed". The case study ended three sessions running with `loop_status` reporting "1 structural defect outstanding" for a defect that had been reviewed, understood and accepted, while `generate_bridge` was correctly *refused* because nothing in the design models the relationship. **The honest disposition is the one the bug blocks.** **Fix shapes, any one sufficient:** (1) exclude acknowledgement Decisions from `design_network()` — an acknowledgement is a statement *about* the design, not a participant in it, and this also stops them distorting centrality, SPOF and coupling wherever a review has been recorded; (2) compute the defect id from the affected set MINUS acknowledgement nodes; (3) store the affected ids as a property rather than as edges. **(1) is the most correct and the widest, and the caller list settles it:** `design_network()` (`crates/reflow2-core/src/structure.rs:220`) has **three** callers, not one — `heal.rs:732` (`disconnected_community`, the reported symptom), `propagate.rs:389` (betweenness / centrality) and `surprises.rs:88` (`surprising_connections`). **So acknowledgement Decisions are silently perturbing centrality and surprise detection too, in every design where a review has ever been recorded** — unreported distortion in two more places that nobody has looked at. Option (2) would fix the visible symptom at the hash and leave the other two callers still wrong. *Caller list contributed by the case study and verified here.* **THE PRACTICAL ARGUMENT FOR PRIORITY, from the same source:** that project has now spent FOUR sessions unable to close a defect it correctly understood and correctly judged should be accepted. It tried the acknowledgement route — blocked by this livelock. It tried the bridge route — blocked by the selectivity problem folded into [BL-114], which advises an edge whose only correct forms do not count. **Both correct routes are shut, so the honest disposition is unreachable by any path**, and `loop_status` reports outstanding debt that is actually a reviewed and settled judgement. That is the skimmed-list failure the acknowledge tools exist to prevent, produced by the tool built to prevent it. Relates to [BL-114] (same detector, message half) and to the review-expiry tension already recorded on `req:review-expires-when-what-it-judged-changes`: that one expires too *late* on content, this one expires too *eagerly* on membership, and both come from keying a review to a hash of a set. **BUILT 2026-08-01 — fix shape (1), at `design_network()`.** A new `is_review_record` id rule excludes the `decision:ack:` prefix that both acknowledgement paths already mint and already strip to recognise their own records, so it is the codebase's existing marker rather than a new convention. `ver:acknowledgement-not-structure` passing, 5 cases, **written before the fix**: the two bug cases failed and all three counterweights passed. **THE MEASUREMENT SETTLED WHAT THIS ITEM ONLY ASSERTED, and corrected it in both directions** (`tools/bl124_instrument.py`, run on reflow2's own graph — 125 review records carrying 610 edges): *centrality* was distorted badly — **four of the eight most central nodes were acknowledgements**, outranking every Component and Capability, and are now none, with real nodes rising into their place (`rel:v0170` +75%, `cmp:detect` +41%). *Surprises* moved the OPPOSITE way from the prediction: zero acknowledgements ever appeared in that list before or after, but the count went **16 → 32** — the bookkeeping edges were **suppressing half the real surprises** by tying communities together, rather than polluting the list. *Islands* could not be measured here at all, because reflow2's own graph has none; the unit test and the field reproduction carry that consumer. **A review remains recorded, reasoned and reachable** — excluded from the NETWORK, not from the GRAPH — and still appears in a blast radius, because a review genuinely is affected when what it reviewed changes. Size **S** | S |
| **BL-125** | **The checksum canonicaliser is applied when writing and never when comparing, so the first honest `reconcile_artifacts` reports 100% false drift — DONE 2026-08-01** | *Found 2026-07-31 by the same case study, on its first build round; verified here in source.* `canonical_checksum` (`crates/reflow2-core/src/artifact.rs:46`) turns a bare hex digest into `sha256:<hex>`, and its docstring records exactly why it exists: on 2026-07-25 four artifacts registered from raw `sha256sum` output made the coherence gate report every one as drifted while the bytes matched — *"a false red on a gate whose whole job is to be believed is worse than no gate."* **That fix was applied to the two WRITE sites (`artifact.rs:199`, `:437`) and never to the READ side: `drift.rs` contains zero references to it,** and the function is private, so the compare path could not call it as written. A caller who passes a bare hash to `link_artifact` and the same bare hash to `reconcile_artifacts` — the natural reading of both docstrings, and `reconcile`'s actively encourages it by saying "compute the hashes yourself (any algorithm, used consistently)" — gets `checksum_change` on **every** artifact on a tree nobody touched. **Why this is worse than an ordinary format mismatch:** nothing errors. It is a plausible, well-formed report with correct `realizes` edges and correct `propagation_seeds`, saying everything drifted; the natural response is to re-register everything, which overwrites the baselines and hides it for another cycle. **The mechanism underneath is sound** — the same case study re-ran it with prefixes on both sides and got 7 unchanged / 5 genuinely edited, with correct seeds — **which raises the severity rather than lowering it: a correct and valuable mechanism is hidden behind a silent format asymmetry that makes it look broken on first use, and first use is when users decide whether to trust it.** **Fix:** canonicalise the observed side too (make the function `pub(crate)` and call it in the drift comparison). **Second, independently:** a format mismatch should be its own finding kind — "you gave me a bare hash and I hold a prefixed one" is a different fact from "this file changed", and conflating them is the whole bug. Size **XS** for the canonicalisation, **S** with the distinct finding kind. **BUILT 2026-08-01 ([PR #13](https://github.com/sligara7/reflow2/pull/13), squashed to `d75b900`, CI green both jobs) — the canonicalisation half.** `canonical_checksum` is `pub(crate)` and runs on both sides; the OBSERVED value is canonicalised rather than merely compared canonically, because it is part of a `checksum_change` event's identity — leaving the raw form filed one divergence under two ids depending on the dialect. `ver:checksum-dialect` passing, 5 cases, **written before the fix**: the three bug cases failed and both counterweights passed, which IS the mutation check — a fix that normalised everything into equality would have passed the bug cases and destroyed the detector. **Measured at the real MCP surface both ways rather than inferred:** before `unchanged: 0` with a `checksum_change` and `propagation_seeds` seeded from a file nobody edited; after `unchanged: 1`, no findings, no seeds. **STILL OPEN, deliberately: the distinct finding kind.** *"You gave me a bare hash and I hold a prefixed one"* is a different fact from *"this file changed"* — and now that the common case is handled, the remaining mismatches are genuinely foreign dialects, which is a smaller and different problem than the one that was hurting. **Note the length dimension is untouched and should stay so:** artifacts here carry a mix of 16-hex and full digests, and a truncated hash really is a different string — canonicalising the ALGORITHM prefix does not and must not paper over that. | XS |
| **BL-126** | **A verification is a boolean, so a check that only ever ran at ONE point of the space it claims to cover reads as full coverage — [BL-106]'s second axis** | *Found 2026-07-31 by an EXTERNAL review of the QBI case study: a real defect the designing session could not see, in code it had written an hour earlier.* **[BL-106] already names the root — "verification is modelled as a boolean per capability, and a capability is not one behaviour" — and its instance was the TIME axis:** a check that drove stdio only, while the capability widened to HTTP and nothing said the evidence had stopped covering the claim. **This is the same blindness one axis over: the INPUT axis.** A capability's own design declared a parameter arbitrary ("change it deliberately when you want to check that a conclusion is not an artefact of one sample path"); every check in the suite pinned that parameter to one value; the regression guard written specifically to protect the project's headline term asserted an exact identity that holds only at that value. **Six of eight alternative values broke it.** The design stated the invariant and then tested only its fixed point, and reflow2 reported `status: passing` — correctly, and blindly, exactly as in [BL-106]. **The discovery move is mechanical and is what should be built: VARY WHAT THE DESIGN DECLARES IRRELEVANT, and see whether the answer moves.** Nuisance parameters — seeds, orderings, timezones, locales, hostnames, float reduction order — are where defects live *because* every check pins them, by the same instinct that made them nuisance parameters. **Fix shapes, cheapest first, all graph-shaped.** (1) **`Verification` records what it was run AGAINST.** It carries `name/description/kind/last_run_at/level/location/method/status` — everything about the check except its coverage. One property naming parameters SWEPT versus PINNED makes "31 checks, all at one value" a visible graph fact, and the detector is then trivial: *a capability whose design claims independence from X, proven only at one X.* Same move that turned `last_run_at` from write-only into [BL-106]'s signal. (2) **Extend `ver:evidence-fidelity` from WHERE to OVER WHAT.** It already encodes that a capability proven only on a rig must say so — that evidence has a scope, and the scope is a fact about the claim. Proven-only-on-a-rig and proven-only-at-one-seed are the same sentence rotated. (3) **The invariants were in DOCSTRINGS and reflow2 never saw them** — "treat as ordinal, not cardinal", "the true spread is wider than shown", "change the seed to check the conclusion is not an artefact". Every one is load-bearing and checkable; not one is a node. That is [BL-95]'s family with a sharper and mechanically greppable target than "unmodelled subsystems": **modal language in artifact prose is where a design states its own invariants**, and a detector saying *"this artifact claims a property and nothing in the graph holds it"* would have raised this finding before a human read a line. **DELIBERATELY NOT IN SCOPE: do not make reflow2 execute or fuzz anything.** `dec:loop-status-state-not-history` and "looking is not writing" both point away from it, and a design brain that becomes a test runner is a worse design brain. All three shapes make the NARROWNESS OF THE EVIDENCE a visible fact and leave the varying to whoever runs the tests. **The framing worth keeping:** the guard was written in the same hour, by the same session, to protect a bug it had just found — so it encoded the belief instead of checking it. That is the structural limit of self-review, not carelessness, and it is the sharpest argument yet for what the graph is actually FOR: not that it remembers, but that it lets a different reader audit a claim instead of re-deriving the belief that produced it. Size **S** for (1), **S** for (2), **M** for (3) | S |
| **BL-127** | **A record whose applicability ENDED has nowhere to say so — Constraint has no lifecycle, and neither does an abandoned ChangeEvent** | *Two instances of one shape, found 2026-07-31; split them if they want different fixes.* **(a)** `Requirement` has `proposed/accepted/deferred/dropped/met` and `dropped` preserves it as history — the retraction model this project gets right. **`Constraint` has no status property at all.** When a submission-window constraint stopped applying, the only options were to DELETE the node, destroying the record that it ever bound, or leave it in place implying it still binds. It was deleted; the history survived only because a ChangeEvent happened to record the removal — by luck, not by design. Regulatory and schedule constraints lapse constantly. **(b)** The prescribed loop order is `add_change_event` then `propagate_change` then edit. Followed correctly, and then the blast radius (or a collision, as here) says do not make the change — **and there is no way to mark the ChangeEvent as recorded-then-abandoned.** The only choices are to leave a record asserting nodes were modified when they were not, or delete it. Deleting was right in that instance (never true, so a mistake rather than history) but *the loop's own prescribed sequence routinely produces a state the vocabulary cannot express* — and talking yourself out of a change via the blast radius is the tool WORKING. **Fix:** give `Constraint` the Requirement lifecycle; give `ChangeEvent` a way to say the change was not made. Both are schema changes and move the stamp. Size **S** | S |
| **BL-128** | **`brainstorm` ships as a skill and the vocabulary cannot hold its output — there is no node type for an idea** | *Found 2026-07-31; the skill was followed exactly and had nowhere to put the result.* The skill's contract is *"record the ideas as ideas… do not turn any of them into requirements or capabilities"*. **Of 28 node types, none is an idea, hypothesis or candidate.** The two near-fits both fail for stated reasons: `Question` is reserved by its own schema for the DETECT→PROMPT loop (*"not extracted from prose — authored by the loop"*), and `register_alternative` requires each alternative to be an Artifact with a `location` pointing at where that alternative's design export lives on disk (`service.rs:1812-1820`, non-optional) — for six paragraphs of thinking with no export, that writes six pointers to files that do not exist. Six unexplored directions were recorded as `Decision` nodes at `proposed`, which the schema does describe as an undecided fork, and it works — but **a reader six months out sees seven Decision nodes and reasonably assumes seven things were decided.** **Brainstorming is where most design work starts.** **Fix, either:** a lightweight `Idea`/`Candidate` type, OR make `location` optional on `register_alternative` — an alternative that is still just a thought is the normal case at the moment you most want to record it. The second is much cheaper and probably sufficient. Size **S** | S |
| **BL-129** | **`Interface.medium` is unreachable from BOTH tools that create and specify Interfaces, and its default silently manufactures false SPOF warnings** | *Found 2026-07-31 by following the obvious path.* `add_interface` accepts only `id` and `name` (`graph.rs:720`). `set_interface_spec` fills paradigm, payload format, schema, endpoint, operations, auth, transport security and error model (`service.rs:4784-4800`) — **and not `medium`.** So the one property AGENTS.md explicitly warns about — *"mark a shared package `library`, because a library linked into its callers cannot fail on its own, and the structural detectors need to know that to avoid calling it a single point of failure"* — is reachable only through `create_node`. **A user who follows the obvious path leaves every interface at `unspecified` and collects false SPOF warnings later, having done nothing wrong**, which is the punishing-correct-work shape of [BL-23]. Note the seam work already had to fix `medium` defaulting to a flattering value (`ver:seam-incompatibility`: *"two silent boundaries agreed on a value neither had chosen"*) — the honest default is in place and the field is simply unwritable from where people stand. **Fix:** accept `medium` on `add_interface`, or on `set_interface_spec`. Size **XS** | XS |
| **BL-130** | **A Component cannot be placed in an Environment until a Release exists, so `no_deploy_operate` asks a question the vocabulary cannot answer at the phase it fires** | *Found 2026-07-31 at exactly the phase the gap is designed for.* The gap asks where this will run. `add_environment` models precisely that. But `deploy_to` links a **Release** to an Environment and there is no Component→Environment edge — and at concept phase there is no Release, so inventing one to answer the question would be fiction. The honest move was to leave the gap open. **"This runs on one local workstation" is a real and useful early design fact with nowhere to live except prose in a Requirement.** Related to [BL-115] (`unverified_capability` / `unrealized_capability` unusable on a pre-construction design): same family — a detector that is right about the question and arrives before the vocabulary can hold the answer. **Fix shapes:** a Component→Environment edge for the pre-release case, or let the gap name the Release it would need so the user can see WHY it cannot be answered yet rather than reading it as unanswered work. Size **S** | S |
| **BL-131** | **The loop nudge is optional setup, and the one time it fired it caught a real hole** | *Found 2026-07-31 from both sides in one session.* `loop_status` volunteers that no session-end hook is installed and that therefore *"nothing will remind you when the coherence loop is owed something"* — good self-awareness, and the instructions carry the exact settings.json. **Then the hook fired and earned it immediately:** it caught a session about to end after 14 graph writes with no defect check, holding six brainstormed options islanded from the requirement they exist to serve. The session's own words: *"left to my own devices I would have ended the session with that hole in the graph"* — the precise failure the instructions warn about, being committed by an agent that had run `detect_gaps` diligently after every batch and `detect_defects` not once in twelve writes. **The product documents a known failure mode of itself and then leaves the fix as manual setup the user has to notice and perform.** The design's own commentary — *"a discipline that depends on being remembered loses to urgency every time; fire it on a trigger, not on virtue"* — is validated, and applies to installing the trigger as much as to the trigger. **Fix:** offer to install the hook during genesis. Relates to [BL-111] and [BL-112] (the nudge's own defects) — this is about whether it is there at all. Size **S** | S |
| **BL-132** | **Error-message quality is wildly inconsistent, and the bad half cost a false bug report against reflow2** | *Found 2026-07-31, and reproduced independently by two sessions within the same hour.* Same product, same session: `artifact_type: "source"` returns *"invalid enum value 'source', expected one of [code, config, document, …]"* and is fixed in one retry with no guessing; `change_type` returns bare *"unknown change type: <value>"* with no list, no hint and no pointer. One session guessed `refactor` and happened to be right; the reviewing session hit the identical wall on `defect_fix`. The valid values DO exist in prose — `test_failure_fix` appears in `set_artifact_checksum`'s docstring — **so the vocabulary is documented everywhere except where the error rejects you.** **Same family, and this one has a measured cost:** `rephrase_degraded: true` says THAT the gap→question rephrase degraded and not WHY, which caused a case study to file a bug against reflow2 that it had to retract two entries later — the real cause was its own malformed payload, and one line of reason would have shown that immediately. **A tool that degrades gracefully and says only "degraded" makes the caller suspect the tool.** **Fix:** make the enum-error format universal, and give `rephrase_degraded` a reason. The good version already exists in the codebase — this is consistency work, not new work. Size **S** | S |
| **BL-133** | **The edit path exists and is invisible: two independent readings of the surface concluded statements are write-once** | *Found 2026-07-31. Filed as a DISCOVERABILITY defect, not a missing capability — the reported bug is wrong and the underlying problem is real.* A case study reported three times that Requirement and Capability statements cannot be edited over MCP, that the only path is drop-and-recreate under a new id (inflating the graph with near-duplicates, which is the opposite of what `dropped` is for), and that its Project `objective` was therefore *"permanently stale"*. **All of that is false.** `create_node` calls `upsert_node` (`graph.rs:232`) and the merge is literal — `merged.extend(supplied)` — so passing `{statement: …}` with an existing id edits the statement and every omitted property survives. The tool's own description says so. **What makes this worth an item rather than a correction:** that same session USED the upsert an hour earlier to set `Interface.medium`, and still did not generalise it; and the belief was stable enough to survive into a standalone consolidated fix list, from which it would have propagated as fact. **The `add_*` tools never say where to go to CHANGE one.** Fix: one sentence in the `add_*` descriptions naming `create_node` as the edit path. Size **XS** | XS |
| **BL-134** | **A clean `reconcile_artifacts` cannot clear its own loop debt, so the diligent operator and the negligent one see identical output** | *Found 2026-07-31 after reconciling every artifact with `unchanged: 9, findings: []`.* `loop_status` went on reporting *"N built capability(ies) never checked against reality (reconcile_artifacts)"*. **Both underlying principles are right and neither should be given up:** reconcile records nothing unless it finds divergence (*"looking is not writing"*), and `loop_status` computes from graph state rather than run history (`dec:loop-status-state-not-history`, straight out of the BL-74 fleet trial). **The consequence is that this debt item can only be cleared by FINDING drift** — do the right thing, find nothing wrong, and the to-do list is unchanged. Same disease as [BL-124]: a list that cannot reach zero is a list people stop reading, which is exactly what the acknowledge tools exist to prevent. **Fix shape:** record a lightweight *"checked, clean at <timestamp>"* observation. That is still a fact ABOUT the design rather than run history, so it does not breach `dec:loop-status-state-not-history`, and it is the fact `loop_status` actually needs. **Distinct from the confirmation-ledger item** (the second `BL-108`, in Bigger threads — note that id is used twice and wants resolving) which is about a `planned` capability inheriting its component's artifact; this one is about a clean check leaving no trace. Size **S** | S |
| **BL-135** | **State the FAILURE MODE, not the rule — the guardrail principle, evidenced by a blind trial, and written nowhere** | *The strongest general finding from the 2026-07-31 case study, and the only one that is about how this project WRITES rather than what it computes.* An agent designed a whole project through reflow2 without knowing the guardrail docstrings were deliberate behavioural interventions. It read them as ordinary API prose, formed its own view of what they were for, complied, and praised the phrasing in its log without suspecting it was aimed at it — **a blind trial, and they worked.** It then ranked the eight that changed its behaviour, most effective first: `set_artifact_checksum`'s *"silent accept does not exist: it is how a design erodes into fiction over N fix cycles while reporting zero gaps"* (named the single strongest intervention in the surface — it produced a named ChangeEvent for all nine artifact updates that would otherwise have been silent re-registrations); genesis's *"do not invent a brief on my behalf"* (held through THREE blank briefs with a detailed solicitation sitting in the directory, preventing the largest failure available — fabricated requirements phrased in the customer's own language, which are unusually hard to spot later); `add_constraint`'s *"criticality is a claim about consequence, so ask the user first"*; `set_requirement_status`'s *"promoting it yourself forges their signature"*; `delete_edge`'s *"a link that WAS true and stopped being true is design history, not an error"*; `describe_schema`'s *"validating is not the same as meaning what you intended"*; `set_project_mode`'s *"the default records that nobody has chosen"*; and the stop hook's *"bookkeeping is not the loop"*. **The extractable rule, in its words: it complied more readily where it was told what goes wrong if it did not, because that let it evaluate the reasoning rather than obey it — and agree.** *"Ask the user first"* alone would have been weaker than the version naming the consequence. **Equally informative: the interventions that did NOT need to exist** — nothing had to stop it over-modelling or creating Components at genesis, because the skills sequenced the work so those temptations arrived when the right move was already obvious. **The work:** write the rule where descriptions are written (AGENTS.md), with this ranked evidence, so it is a standard rather than a habit — and consider a `skill_lint` check that a guardrail sentence names a consequence. **The counterexample is already filed:** [BL-124] shows a guardrail whose reasoning is the same quality as the rest and whose IMPLEMENTATION contradicts its intent, which the case study called the highest-value fix in its document. Good wording does not survive a mechanism that defeats it. **CORROBORATED FROM INSIDE THE OTHER PROJECT, which is the strongest form the evidence could take:** that session adopted the same pattern in its own code without being told to, and it caught a real error — a guardrail written two entries earlier asserted that all calibration anchors miss in the same direction, and when the same agent later fitted a coefficient to one anchor, the assertion failed with its own text: *"anchors now disagree in direction; the models were probably tuned to fit one of them."* **Written before the mistake, and it named the cause.** The principle is not a style preference about API prose; it transfers to any assertion a person will read at the moment they are getting something wrong. Their honest division of labour is worth carrying into the docs alongside it: *reflow2 is where reasoning persists; tests are what stop you being wrong* — which is also the argument for the out-of-scope line on [BL-126]. Size **S** | S |
| **BL-136** | **A value FITTED to a piece of evidence can still be cited AS VALIDATED BY that evidence — circular validation is invisible, and it is [BL-106]'s third axis** | *Found 2026-07-31 by a project that hit it, invented its own fix, and then had that fix catch the same author committing the error again one entry later.* **The shape:** a design node whose value was DERIVED from an anchor should not be able to count as VERIFIED by that anchor. reflow2 has `Verification` nodes and provenance values for how a node ENTERED the graph, and nothing that expresses *"this check is circular with respect to that parameter"*. **Concretely, in that project:** a layout coefficient was fitted to one published anchor; agreement with that anchor immediately stopped being a test and became a fit; the framework's only footprint check was consumed and every status still read green. They added a `CALIBRATED` provenance carrying `calibrated_against`, and made the report print the anchor as *[CONSUMED — a fit, not a test]*. **Their own diagnosis of why it matters, which generalises past quantum computing:** in ANY empirically-calibrated design — control systems, simulation models, anything with fitted constants — this is the standard way a design becomes fiction while nothing reports a gap. **This completes a family, and the framing should match the other two:** [BL-106] is a check gone stale along the TIME axis; [BL-126] is a check narrow along the INPUT axis; this is a check that is not INDEPENDENT of what it validates. All three are the same underlying hole — **the quality of the evidence is not a fact the graph holds**, only its existence and its status. **The closest existing thing is `unvalidated_capability`** ("built right, but the right thing?"), which that project named the single most valuable prompt of its session — and which cannot see this, because a circular check is a check. **Fix shapes:** a `calibrated_against` relation from the fitted node to the evidence it consumed, and a detector that reports a `VERIFIES` edge whose target was calibrated against the same evidence. Adjacent to [BL-120](d) — both are provenance being unable to say where a number's AUTHORITY comes from. Size **M** | M |
| **BL-113** | **The server can serve the latent surface for a whole session while the agent works in a design directory, and nothing says so** | *Found by me, 2026-07-29, over the course of an entire session, and then reproduced under observation when Anthony ran `/mcp` and asked whether reflow2 was among the servers that came back. It was not.* **Nothing here is broken, which is what makes it hard to see.** The machine-wide registration `install.sh` writes carries `--graph-path .reflow2/graph --shared --only-if-present`, and that path is **relative** — it resolves against the server process's working directory. Started with cwd `/home/ajs7`, the server looked for `/home/ajs7/.reflow2/graph`, correctly found none, and `--only-if-present` did exactly what it promises: served the latent surface, one tool, `reflow2_start_design`. Every component behaved as designed. **The failure is that nobody was told.** For four hours an agent read that repo's export, edited its files and committed to it while the design brain for that repo was pointed somewhere else, and the only symptom was the absence of tools nobody had reason to look for. **Evidence:** PID 92257, started 09:09:03, cwd `/home/ajs7`, unchanged across a reconnect that reported *"Reconnected 3 of 3"* — Gmail, Calendar and Drive; reflow2 was not among them — with the tool surface still exactly one tool afterwards. **The diagnostic already exists and is not computation:** the `reflow2-mcp-launch-wrapper` memory records the correction in as many words — *"Only a full agent restart reliably replaces a running stdio server. Check the process, not the reconnect message: `readlink /proc/<pid>/cwd`"* — which is knowledge a human must remember, the same shape as [BL-107]. **What it cost, concretely:** `capture-intent`, `loop_status`, `impact-check`, `link-artifacts` and `detect_gaps` were all unavailable, so [BL-110], [BL-111], [BL-112] and this row had to be staged in a markdown table instead of captured in the graph where they belong; and the loop nudge fired **five times** demanding a loop check that could not be performed, since `touched` only clears on a successful `mcp__reflow2__*` call. That is the case [BL-111] predicts turns into someone switching the hook off. **THE CONSTRAINT THAT MAKES THIS NON-TRIVIAL, and it must not be lost:** a directory with no design is the NORMAL, intended state — the server's own instructions say most directories should stay that way, and a warning that fires in every such directory would be noise that gets the whole surface ignored. The signal is not *"no design here"*; it is the **MISMATCH** — the agent is working in a directory that HAS a design while the server serves a different one, or none. **Fix shapes, none chosen:** (a) the latent surface's text already says *"no design has been started HERE"*, and the word HERE is doing work nobody can see — naming the resolved absolute directory it means costs nothing and changes no behaviour; (b) `tools/reflow2-mcp-launch.sh` could report the graph path it resolved on stderr at startup, where diagnostics already go; (c) something could compare the harness's project directory against the server's and speak only on mismatch, which is the honest signal but couples two components that are currently independent. **What must NOT be done:** hardcode an absolute `--graph-path` into the global registration. That would make every directory on the machine serve one project's graph and destroy the once-per-machine, any-project property `install.sh` exists to provide. **Shares a root cause with [BL-112]** — a relative path resolved against an ambient working directory — and is filed separately because the fix sites and the blast radii are nothing alike: one silently miscounts edits, this one silently disables the design brain for an entire session. Size **S** | S |
| **BL-112** | **The loop nudge counts file edits made outside the project** | *Found by me, 2026-07-29, in the same session as [BL-111] and by the same route. Not yet in the graph — that session had the MCP server bound to the wrong directory, so `capture-intent` could not run; this row is a staging post and the item should be promoted properly when the graph is reachable.* `loop_nudge.py:202` is `if tool in EDIT_TOOLS: state["edits"] += 1` — it matches on the **tool name only** (`EDIT_TOOLS = {"Edit", "Write", "MultiEdit", "NotebookEdit"}`, so Bash-driven changes are invisible to it), with no check that the edited path lies inside the project. In the session that found it the tally reached 5 while only 4 Edit/Write calls had touched repo files, so at least one edit to a path outside the project was counted; the surplus came from scratch files written under `/tmp`. **A second and probably deeper cause sits underneath:** `state_dir()` at `:98` returns a **relative** `Path(".reflow2") / "loop-nudge"`, so which project's tally an edit lands in is decided by the hook process's working directory at the moment it runs, not by where the edited file lives — and `write_state` does `mkdir(parents=True)`, so a hook that runs anywhere else silently starts a fresh tally in a new directory rather than failing. Both faults point the same way and a path filter alone would not fix the second. **The consequence is that the number in the message does not mean what the message says it means:** a session that edits four files in an entirely different repository, and this design not at all, trips a nudge asserting that *this* design's as-built record is at risk. That is worse than the noise the threshold exists to bound, because the nudge is not merely frequent but wrong — and a warning that is wrong in a checkable way is the thing that teaches people to ignore warnings. **The fix is a path filter, not relevance detection.** The bluntness is deliberate and the comment at `loop_nudge.py:226` says so — *"the hook cannot know which files are design-relevant, so a count threshold and the once-only rule bound the noise"* — and that judgement should stand; scoping the count to the project keeps it exactly as blunt while making it true. `CLAUDE_PROJECT_DIR` is already in hand: `reflow2_install.py:168` uses it to build the hook command, so the value is available at the point the count is taken. **Honest subtlety:** the tool event's path field differs by tool (`Write`, `Edit`, `NotebookEdit`), and an edit whose path cannot be determined should count rather than be dropped — silently under-counting would turn a noisy nudge into an absent one, which is the same failure inverted. Size **XS** | XS |
| **BL-111** | **`loop_nudge` promises "this nudge fires once" and nothing records that it fired** | *Found by me, 2026-07-29, only because the nudge fired twice in one session and the second firing contradicted its own text.* The message ends *"(This nudge fires once; stopping again proceeds.)"* — and the guarantee rests entirely on `event.get("stop_hook_active")` at `loop_nudge.py:207`, a flag the **harness** sets when it re-invokes within a single stop cycle. It is never persisted. The session state file carries `{"writes": 0, "edits": 4, "touched": false}` and **has no `nudged` key at all**, so once the cycle ends the hook has no memory of having spoken. The rule it actually implements is *fires once per stop cycle*; the rule it advertises is *fires once per session*. Observed directly: it blocked at `edits: 3`, the session continued and did more work, and a fresh Stop arrived with `stop_hook_active` false again and blocked a second time at `edits: 4`. **The case where this bites hardest is the one where the nudge cannot be satisfied.** `state["touched"]` only ever clears on a successful `mcp__reflow2__*` call, so a session whose server is unreachable — bound to the wrong cwd, or serving the latent surface only — gets nudged at *every* stop for the rest of its life with no action available that would stop it. That is precisely the session in which the user is least able to comply and most likely to disable the hook. **Fix shape:** set `state["nudged"] = True` beside the `print` and test it alongside `stop_hook_active`; both blocking branches need it, since the graph-writes branch at `:214` makes the same promise on the same footing. This does not change the design intent — the once-only rule is deliberate noise-bounding from BL-90 — it makes the stated intent true, which is the same class of fix as [BL-107]: a designed guarantee with no computation behind it. Size **XS** | XS |
| **BL-110** | **Nothing computes whether the design ever reached git, and git is where the design lives** | *Found by me, 2026-07-29, answering Anthony's question — "is reflow2 git aware? when an agent calls a reflow2 skill, does it remind the agent to commit and push?" The honest answer is that reflow2 is git-aware everywhere except the surface that would act on it. Not yet in the graph; belongs in `capture-intent` when the server is reachable, likely as a requirement rather than a defect.* **Where the awareness is real:** `.reflow2/graph` is gitignored, machine-local and single-writer, so the **committed export is the only copy anyone else can see**; `merge.rs` ships a driver git itself invokes, taking its ancestor from `git merge-base`; `reflow2_check.py` reads git to validate the export hash chain against `HEAD~1` ([BL-107]); and `sync.rs:91` hands an agent literal recovery steps — *"This is not a merge conflict and git will not catch it… 1. `git pull --rebase`"* — on a stale-export refusal. **Where it is absent is the loop.** `report.rs`, where loop debt is computed, contains **zero** occurrences of `git` or `export`, so `loop_status` is structurally incapable of reporting that the graph is ahead of the committed file. The nudge strings never mention committing. The Rust core never invokes git at all — no `Command::new("git")` anywhere in `crates/`. **So an agent can design productively for an entire session, never export, never commit, and nothing in the loop notices that the work exists on exactly one machine** — the same failure mode as a stale seat, one layer up, and the one the gitignored-graph architecture makes possible by construction. **This is load-bearing for work now in flight:** `req:graph-indexes-snapshots` and `dec:epoch-roadmap-storage`'s (c-refined) shape both divide labour as *git stores the snapshots, the graph owns the index* ([roadmap-and-planned-epochs.md](roadmap-and-planned-epochs.md)) — a division with nothing verifying that git received its half, before the first planned epoch is ever written. **Fix shape, and it is cheap:** the graph knows its write generation and the export carries `content_hash`; comparing the live graph's hash against the committed export's answers *"is there undeposited work"* without any new vocabulary, and `reflow2_check.py` already proves the git side is reachable from the Python tier. **Two constraints that must hold.** First, git may be absent and its absence is not an error — the answer is skipped, never guessed, exactly as BL-107's gate already does outside a working tree. Second, **VCS neutrality is a deliberate property and must not regress**: every integration today is plain git semantics — driver, refs, `pull --rebase`, `merge-base` — with no forge APIs anywhere, which is why GitLab, Gitea and Bitbucket work unchanged, and why the GitHub MCP server was declined. (`tools/install.sh` does use `gh` and GitHub release URLs, but that distributes *reflow2 itself* and ties nobody's *design* to GitHub; the distinction is worth keeping.) Same recurring shape as [BL-107] — a designed dependency that nothing validates. Size **S–M** | S–M |
| **BL-109** | **Link three real repos and find where their interfaces disagree — the composition proof of concept** | *Anthony, 2026-07-27, captured while fresh rather than worked. Graph elements are the record: `dec:linked-repos-poc` (**proposed**), `req:interface-spec-complete` and `req:seam-incompatibility` (both **proposed**); this row is the pointer.* Build a reflow2 graph inside **storyflow** (adopt under way, unfinished) and another inside **dynograph-foundation**, then link them through their published surfaces and identify incompatibilities. **The case is well chosen and not a toy:** reflow2 AND storyflow both depend on dynograph-foundation, so it is a real three-way seam between three real repositories — built by the same author at different times against a foundation that moved, which is exactly how interface drift happens. **The linking half already exists** (`dec:nested-graphs` decided, `cap:mirror-surface` and `cap:publish-surface` built); **the comparing half does not**, and the reason is concrete. Measured against the six characteristics an interface spec needs — protocol *and* sync/async paradigm; payload format *and* field-level schema; endpoints *and* permitted methods; authentication and data protection; status vocabulary and parseable failure structure; rate limits, concurrency and timeouts — reflow2 structures roughly **one and a half**: `Interface.medium` and `designation`. Everything else is one free-text `spec` string or absent. **So two graphs can be linked today and still not be comparable**, and `unprovided_interface` only ever answers the wiring question (does the contract have both sides), never the one that bites in SoS work: both sides exist, both are confident, and they disagree. One note in reflow2's favour — #6 may need wiring rather than vocabulary, since `Constraint` already carries `quantity`/`limit`/`direction` and `CONSTRAINS` already rolls up, so rate limits and timeouts may be expressible now. **Sequencing is the open choice** in the decision: structure the spec first, or run the PoC against today's thin spec precisely to find out what it cannot say. Second payoff either way — it would be the first time reflow2 is pointed at a design it did not author, twice, which is the adopt path Anthony says his real work almost always starts from. **PINNING, added by Anthony the same day** as `req:design-dependencies-declared` (**proposed**): the consumer of a dependency should declare WHICH version/commit/tag of it they depend on, in a checked-in manifest — a `reflow2.toml` in the spirit of Cargo.toml / pyproject.toml / pixi.toml — so that when the two evolve separately you know what yours was built against. **Half of this exists and the distinction is the point:** `cap:mirror-surface` already records `mirror_content_hash` (the content hash of the surface a mirror was taken from) and `mirrored_at`, deliberately as a *dated claim* rather than live truth — that is what you GOT. A manifest is what you MEANT: human-meaningful (`v0.11.0`, not 64 hex characters), authored not derived, reviewable in a diff, readable before any graph is opened. Having both is what makes two checks possible that neither half supports alone — *declared vs mirrored* (am I composing against the version I said?) and *declared vs upstream* (has it moved since?). Must also handle a declared dependency that cannot currently be resolved, since that is a normal state and silence about it would be the failure, and must **record which reflow2 wrote it** — the same discipline the export stamp already follows, because a manifest read by an older or newer binary that cannot say which vocabulary it was written against forces the reader to guess. **THE MECHANISM, added by Anthony the same day and better than what this row first implied** (`req:composed-analysis`, **proposed**): checking whether two projects line up should be **IMPORTING one graph into the other** and running reflow2's ordinary checks — `detect_gaps`, `check-health` — over the combined design, so seam problems surface as the gaps they already are rather than needing a bespoke comparator nobody else benefits from. Same principle this project keeps rediscovering: make the existing computation SEE more rather than write a new one. **What blocks it is concrete:** `import_graph` writes every node under its ORIGINAL id with upsert semantics, so importing Z into A means Z's `cap:store` silently overwrites A's — it was built for layering a design onto *itself*. The other two mechanisms do not do this either: `mirror_surface` imports only the published surface and keeps it foreign; `merge_designs` is a three-way merge of two versions of the SAME design. So a **fourth** thing is needed — a namespaced import where both designs coexist and stay separately attributable. Two things not to lose: an export of A must never start shipping Z's internals, and project-level detectors would otherwise show a consumer its dependency's gaps — noise that gets a feature switched off, for which `cap:scoped-analysis` is the likely answer since it already narrows and reports what it left out. Note the tension with `dec:nested-graphs` (a graph per ownership boundary, because edges cannot cross stores): this does not overturn it — ownership and release stay separate — but it does add a THIRD, analysis-only composition. Size **L** | L |
| **BL-108** | **Design the simulator, and test there before the real thing** | *Anthony, 2026-07-27, offered explicitly as a future idea rather than current work — captured as `req:design-the-simulator` (**proposed**, wants his word), and this row is the pointer.* The argument: a design has to operate in an environment, and where a simulated environment or a digital twin is reasonable to build, **exercising the design there first removes risk while it is still cheap** — an issue found in simulation costs a rebuild, the same issue found in the field costs a deployment. So the simulator is something to be **designed**, not an implementation detail that shows up later, and the progression simulation → progressively more realistic tests → fielding is part of the design process rather than an afterthought. **The caveat is part of the requirement:** not every design admits a meaningful simulation, and a tool that insisted would generate elaborate fictions for the ones that do not — so *is a simulator feasible and worth it for THIS design* is asked, never assumed, and a design that answers no says so on the record without being nagged. **What already exists to build on** (checked, not assumed): `Verification.method` accepts `simulation`, so a check CAN already be recorded as having been run in one; `Environment` and `EnvironmentRule` hold deployment and platform context. **What is genuinely missing:** nothing treats the simulator as a designed thing with its own requirements and fidelity claims; nothing expresses that a check run in simulation is weaker evidence than the same check run for real, which is the whole point of the progression; and there is no place to record the feasibility judgement. Note the fidelity axis reflow2 already has — as-designed / as-built / as-fielded — is the natural home for 'as-simulated' sitting before as-built, and that is probably the shape rather than a new subsystem. Size **M–L** | M–L |
| **BL-107** | **The export hash chain can be broken silently, and was — six commits in a row — DONE 2026-07-27** | *Found by me, 2026-07-27, and only because an unrelated verification step compared two exports byte for byte.* `dec:export-hash-chain` makes each committed export link to its predecessor's `content_hash`, so the design has a lineage independent of git. `export_graph --path` builds that link from **whatever file is already at the path** — so exporting to a fresh scratch path and then copying the result into `docs/design/reflow2.json` produces `prev_content_hash: null` and severs the chain. That is exactly what I did for six consecutive commits (`cf7492b`..`0812492`) while working around the version-stamp lag; the chain was intact for every commit before that (`436aafe` → `df9a5f8` → `476ae9a` → …) and is intact again after. **The finding is not the mistake, it is that nothing noticed.** The design gate passed 0 notes every time, `loop_status` stayed clean, and `detect_gaps` stayed zero, because no check reads the chain — a designed lineage feature with no computation validating it, which is now the *sixth* instance of this project's recurring shape (after BL-70, BL-96, BL-35, BL-106, and the resolution thresholds). **Fix shape:** `reflow2_check.py` already reads the committed export and already has git available; comparing `prev_content_hash` against the previous commit's `content_hash` is a few lines and turns a silent severance into a red build. Two honest subtleties: the chain legitimately does not advance when content is unchanged, and a legitimate first export has no predecessor — both must be distinguished from a break rather than lumped with it. Note the six broken links **cannot be repaired retroactively** without rewriting published history, and should not be; the gate starts from the next commit. **DONE 2026-07-27:** `reflow2_check.py` now compares the export against the one it replaced and fails loud on a break. Two contexts, one rule — before a commit the working file's predecessor is HEAD's version; in CI the working file *is* HEAD's version, so the pair checked is HEAD against HEAD~1. Both subtleties are honoured and tested: unchanged content is not a break (the chain is not meant to advance), and a first export has no predecessor. Outside a git working tree the question is skipped rather than guessed, so a consumer without git can still run the gate — and the path is resolved against the git root, because an absolute `--export` would otherwise have skipped silently, which is the same failure in miniature. 5 cases, MUTATION-CHECKED: against a gate with the check removed, the severed-chain and wrong-link cases both fail. **A second silence found while doing it** — the new tests were appended after `unittest.main()`, so they were defined, never collected, and the suite reported OK on 6 tests while claiming to cover 11 | ~~S~~ |
| **BL-106** | **A capability can claim `verified` on a check older than the code it covers, and nothing says so** | *Anthony, 2026-07-26, from [BL-105]: "something showing as green while failing is a failure of reflow2? is this something we should address?"* **The honest answer is that reflow2 reported nothing false and was still blind.** `cap:degraded-surface` was `verified` with a passing check; that check drove stdio only; adding the HTTP transport widened what *"the server explains its own absence"* had to mean without widening anything that tested it. `detect_gaps` returned zero throughout, correctly — verification is modelled as a **boolean per capability**, and a capability is not one behaviour. **What makes it actionable is that the evidence was already in the graph:** `art:main` drifted twice (`chg:shared-sessions`, `chg:remote-sessions`) while `art:test-degraded-server` never moved, and **`Verification.last_run_at` is written by `verify.rs:115` and read by nothing anywhere in the core** — the same shape this project keeps finding (the temporal axis before [BL-70], the entire inviolable-intent vocabulary before [BL-96]), and the same question AGENTS.md says pays best: *what does the core already do that nothing can reach?* **Shape, decided by Anthony rather than assumed:** a **freshness FACT on the confirmation ledger**, not a gap (`dec:verification-freshness-not-a-gap`). A stale-looking check is a standing property of a claim, not an event; it would fire on every legitimate refactor as readily as on a real hole; and an open list that can never reach zero gets skimmed, which is the failure the gap workflow exists to prevent ([BL-23]'s lesson — when a detector punishes correct work, the answer is a different question, not a tuned threshold). So `ClaimConfirmation` gains the verification side of what it already computes for accepts: the newest dated `last_run_at` across a capability's passing `VERIFIES` checks, beside the dates it already has. **Three constraints the core imposes and this must honour:** it takes no clock, so this compares timestamps supplied by callers and never asks the system the time; undated events are not comparable, which `confirm.rs` already states; and where either side is undated the answer is an explicit reported `unknown`, never a silent pass — an unanswerable freshness question presented as freshness is the same lie in a new place. **The check owed at build time:** an accept dated after a verification's `last_run_at` reports stale, the reverse does not, and a missing date reports `unknown`. Deliberately NOT the wider question of what a check actually *covers*, which is unknowable from the graph and belongs to [BL-95]. Captured as `req:verification-keeps-up` (accepted) + `cap:verification-freshness` (planned, allocated to `cmp:confirm`, governed by `dec:report-dont-judge`) | S–M |
| **BL-105** | **The degraded surface ignores `--http`, so the outage it exists to explain is silent again on the new transport — shipped in v0.14.0. FIXED 2026-07-26, same day** | *Found by hand, 2026-07-26, while setting up the shared-server recipe on a real machine — not by any detector, which is the second finding.* `main.rs`'s `Ok` arm honours `cli.http` (line 678); the **`Err` arm is hardcoded to `serve(stdio())` and never reads it** (line 700). So when the graph is already held **and** `--http` was asked for, reflow2 serves its one-tool explanation on stdin/stdout while every session that was told to dial `http://…:8787/` gets connection-refused. That is precisely the failure `req:never-silently-absent` (accepted, field-reported by the StoryFlow fleet) exists to prevent — *"exiting at startup makes the outage indistinguishable from reflow2 was never configured"* — reintroduced on the transport added two commits later. An operator running it by hand fares no better: with stdin not a client it dies as `failed to start the degraded MCP server: connection closed: initialize request`, which names neither the lock nor the remedy, and `cap:degraded-surface`'s existing check stayed green throughout because it only ever drove stdio. **Fix shape (small, and the same shape as the `Ok` arm):** branch the `Err` arm on `cli.http` too — serve `DegradedService` over `serve_http` when an address was given, stdio otherwise. Both arms then answer on the transport that was requested, which is the invariant worth naming in the code. **Second half, and the part that keeps it fixed:** `tools/test_degraded_server.py` has *no* occurrence of `http` — hold the graph, start a server with `--http`, and assert an HTTP client still handshakes and still receives `reflow2_unavailable` with the reason. Recorded as `ver:degraded-follows-transport` (**failing**, observed by hand, not yet automated) against `cap:degraded-surface`, whose stdio check `ver:degraded-surface` remains honestly green. **The generalisable lesson is the one BL-95 keeps making:** a capability with a passing check is only proven on the paths that check drives, and adding a transport silently widened what "the server explains itself" had to mean without widening anything that tested it. **FIXED 2026-07-26:** `serve_http` is now generic over the service it carries, so both arms hand rmcp a factory and both answer on the transport that was requested; the startup line distinguishes the two surfaces, because a degraded server looks like a working one from outside and an operator who reads "serving over HTTP" and walks away has been misled. 4 new cases in `tools/test_degraded_server.py`, and **they were mutation-checked rather than assumed**: reverted against the v0.14.0 behaviour all four fail with *"nothing ever listened on the port the caller asked for"*, which is the whole reason to write them. `ver:degraded-follows-transport` passing | ~~S~~ |
| **BL-104** | **Derive `met` from the golden thread — "done" is computed, never asserted — DONE 2026-07-25** ⟵ *PARITY-CHECK ENTRY ONLY* | **The record for this item is the graph** — `req:completion-computed` (proposed) + `cap:derived-completion` (planned, allocated to `cmp:report`, governed by `dec:certainty-derived` and `dec:passing-is-verified`), captured via `chg:bl104-derived-completion`. This row exists **only so the two can be compared** until backlog items are demonstrably first-class in the graph (Anthony, 2026-07-24); it is deliberately thin, and if it and the graph ever disagree, **the graph is right**. Ask the graph, not this table. One-line summary for comparison: delivery state should be derived from evidence (satisfied AND the satisfying capability realized/verified with a passing check), not read from a hand-set field — BL-85 already states this ("done is COMPUTED from the golden thread … the anti-erosion property for free") and nothing implements it. Measured 2026-07-24: **0 of 28 requirements carry `met`** though several are plainly shipped; 3 of 5 status values (`deferred`/`dropped`/`met`) have never been used; `EVOLVES_INTO` and `OBSOLETES` have zero instances; `TemporalFact` and `Snapshot` are at zero. Full scope, the `inferred`-provenance trap, and the superseded/dropped/evolved distinction live in the capability description, not here | S–M |
| **BL-96** | **KPPs — inviolable intent as its own vocabulary, and something that actually computes a violation — DONE 2026-07-25, both halves** | *Anthony + his brother, 2026-07-24 (`/home/ajs7/Documents/reflow2_idea.txt`): "strategic intent/end goal that must be preserved/maintained no-matter what other design decision is made… hard requirements with no wiggle room — or KPPs (key performance parameters)." **Anthony's call: KPP is its own thing**, not `Constraint` + `priority: critical`.* **The finding that sizes this: most of the machinery exists and has never once been used.** `Constraint` is already defined as "a limit or rule the design MUST respect (vs. a Requirement, which is a goal to achieve)", carries `priority: critical`, and carries a real threshold triple — `quantity` (unit-bearing, e.g. `latency_ms`), `limit` (the number), `direction` (maximum/minimum) — with contributions on `CONSTRAINS` edges and `budget_report` already rolling them up. `DesignRule` carries `enforced: bool` ("whether violations are gate-blocking"). `QualityGate` carries pass/fail `criteria`. `VIOLATES_RULE` and `VIOLATES` exist as edge types. And reflow2's own design holds **22 Requirements, 0 Constraints, 0 DesignRules, 0 QualityGates** — the entire inviolable-intent vocabulary is unexercised, including by reflow2 on itself. Same shape as the BL-70 temporal finding: schema present, practice absent, nothing noticing. **So the work is mostly wiring, plus the one genuinely missing piece: nothing COMPUTES a violation.** `VIOLATES_RULE` is a vocabulary item no detector reads, so a design change that breaks a KPP is silent — which is the whole point of a KPP. Needs: (a) a KPP concept distinct from both Requirement (a goal) and Constraint (a limit) — a **threshold that, if missed, fails the program**, with the DoD/DoDAF sense Anthony means (threshold vs objective value); (b) a detector that fires when a KPP is unsatisfied, unallocated, or contradicted by an accepted Decision, at severity above ordinary gaps; (c) `propagate_change` marking a blast radius that touches a KPP as a **risk crossing**, so impact-check surfaces it before the edit, not after. **Open sub-axis for Anthony at build time (do NOT decide silently):** whether KPP lands as a **new node type** — cleanest semantics, but a schema change, which means a minor bump, an upgrade doc, the foundation-migration checklist, and (per BL-94) real pain for anyone on an older stamp — or as a **discriminated Constraint subtype** (`category: kpp` + threshold/objective fields), which is additive and upgrade-safe but overloads a type whose extraction hint says something subtly different. `dec:edge-orthogonality`'s rule applies either way: **a vocabulary distinction earns its keep only if a computation reads it** — so (b) is not optional garnish, it is what makes the distinction legitimate. Captured as `req:inviolable-intent` + `cap:kpp`. **BUILT 2026-07-25 as a discriminated `Constraint` (`category: kpp`) on Anthony's call** — upgrade-safe where a new node type would refuse older binaries — with `objective` beside `limit` for the acquisition threshold/objective pair, and three ranked violations computed rather than remembered: `kpp_unbound` (0.90), `kpp_breached` (0.95, reusing `budget_report`), `kpp_contradicted` (0.85, review not verdict; proposed decisions excluded). 9 tests. **The capture half followed the same day** as `cap:kpp-proposal`: the kpp-proposal skill + `/kpp`, and `add_constraint` gained the `objective` parameter it had never had on the MCP surface — without which a confirmed KPP could only be recorded through the generic `create_node`, which is how the violation half's own fixtures are built. 3 further tests. The record is the graph; ask it, not this row | ~~M~~ |
| **BL-97** | **Point at a folder of specs and ingest the lot — the missing half is identity resolution ACROSS documents** | *Idea file, line 1: "somebody has an obscene amount of markdown files documenting specs of an application. He'd like to point at a folder and ingest all files… adds its graph components and the attached markdown (or pointers)."* Single-document extraction already exists and is good: `ingest` does multi-pass LLM extraction, and every claim keeps a provenance `Fragment` pointing back at the source text (`YIELDED`), which is exactly the "or pointers" half of the ask. **What is missing is the folder driver and, underneath it, the hard part: cross-document identity.** Forty specs mentioning the same service must converge on ONE node, not forty near-duplicates — and the primitive for that is already declared per node type in the schema (`resolution: { strategy: fuzzy_then_vector, fuzzy_threshold: 80–85 }`), so this is wiring an existing mechanism at batch scale rather than inventing one. Watch for: ordering effects (whichever file is read first wins the canonical name — needs to be deliberate, not incidental), the `possible_duplicate` gap firing N² times on a large corpus (BL-42's noise-floor lesson: a detector that punishes correct bulk work needs a different question, not a tuned threshold), and `dec:ask-not-repair` — suspected duplicates are **asked, never merged**, which at 500 files means the asking has to be batched or it is unusable. Sequencing note: **BL-95's coverage sweep is the natural companion** — ingest a folder, then measure what the ingest did not claim. **SCOPED 2026-07-26 ([scope-corpus-ingest.md](scope-corpus-ingest.md)), and the reading moved the work in BOTH directions — this row's own framing was wrong in two places.** (1) **Cross-document identity is largely BUILT, not missing:** `fuzzy_match` (`ingest.rs:916`) resolves each extracted node against *every existing node of that type in the whole graph*, so ingesting forty specs already converges — merges reported in `IngestReport.fuzzy_merges`, prior states snapshotted with a `ChangeEvent` rather than clobbered. What is missing is **cost**: `scan_nodes` is a full type scan run once per extracted node, so a corpus is quadratic. An indexing problem, not a research one. (2) **The gap nobody had named is coverage of the ask.** `ingest` creates exactly eight node types — Project, Requirement, Constraint, Capability, Component, Interface, plus Fragment and DesignEpoch. **`Verification` and `Decision` have no extraction pass.** The request was *"requirements, test results, designer intent"*: reflow2 extracts the first and neither of the others, so a corpus run today returns a design with **no test evidence and no recorded rationale**, from documents containing both, and nothing says so. `Decision` is the pass that makes an old corpus worth ingesting at all — *why* it was built that way is exactly what is lost when the people leave. (3) **The real blocker is reachability:** `ingest` takes an `LlmBackend`, reflow2 ships no provider, and **no ingest tool exists among the 108 served** — so core ingest cannot be called from a session at all, and the only working path today is the `adopt` skill. The fork is recorded as **`dec:corpus-ingest-mechanism` (proposed, wants Anthony)**: an agent-driven skill, or the SP-3b handshake with the agent as the backend. (4) Two smaller findings: this row's claim that the schema's per-type `resolution: { fuzzy_threshold }` is the primitive is **wrong — nothing reads those declarations**; ingest uses a hardcoded `FUZZY_MATCH_THRESHOLD: u32 = 90` (the fifth instance of declared-but-never-read, after BL-70, BL-96, BL-35 and BL-106). And auto-merging at 90 across a corpus is in tension with `dec:ask-not-repair` in a directional way — *"Auth Service"* and *"Auth Service v2"* score high. **Named user waiting** (a colleague of the author's brother, years of work markdown). Size **M–L** | M–L |
| **BL-98** | **Fifteen developers on one reflow2 — can it identify overlapping work as conflicts?** | *Idea file, line 3.* Half the answer exists and is genuinely good: the BL-80 three-way merge computes a typed per-node/per-property case table against the common ancestor and mints each conflict as a **Question with a deterministic id** — no `<<<<<<<` line markers, typed values instead of lines, and rerere replay for the N near-identical conflicts a team generates. That is better than git at this specific job. **The half that does not exist is the architecture.** `dec:repo-file-embedded` chose a repo file over a service, and its rationale reads: *"the service's strongest argument (concurrency) is hypothetical **while there is one writer**."* Fifteen developers is that hypothetical arriving, which makes this **the first field-sourced trigger to re-open a settled decision** — exactly the signal `cap:revise-trigger` (BL-70 rung 3) exists to surface, arrived by conversation instead of by detector. Per `dec:reopen-supersedes` the move is to mint a NEW proposed Decision holding the alternatives (repo-file + merge-on-pull vs hosted service vs hybrid), **not** to flip `dec:repo-file-embedded` back to proposed. Prerequisites already identified elsewhere: BL-44 (cluster claims — the scoping primitive that lets two people work without colliding), BL-41 (identity/authorship, partly landed as `Contributor`), and the reflow2-native merge-base that BL-70 decided to take as a coordinate rather than a ref layer. **Do not start building this before the decision is minted** — the architecture question is upstream of every feature under it. **UPDATE 2026-07-25:** the decision was minted and DECIDED as `dec:multi-writer-architecture` (repo file + merge on pull, no hosted service), and then RE-OPENED on a narrower axis the same day by `dec:central-host` (`proposed`) when Anthony raised two-coast collaboration and several projects per person — the earlier rationale answered "how do two writers avoid clobbering each other" and not "how does someone reach a design whose repo they never cloned". The graph carries both, plus the two `proposed` requirements waiting on it; ask it rather than this row. Size **L** | L |
| **BL-99** | **Real-time collaborative planning on a shared MCP** | *Idea file, line 5: "If people used the same centralized MCP — could multiple people collaborate on planning almost real-time?"* Distinct from [BL-98] (which is about detecting conflicts in work already done) — this is about several people editing one design *concurrently and seeing each other*, which is a different feature with the same blocker. Today the answer is a flat no, and visibly so: the store is single-writer by architecture and the server says as much (`another process already has the design graph open`). Beyond the store, real-time collaboration needs things reflow2 has no notion of — presence, per-user views of an in-flight change, and conflict resolution at interactive latency rather than at merge time. **Worth recording the honest counter-argument** before anyone falls in love with it: reflow2's whole loop is *ask the human, record the decision* — a design brain optimised for deliberate, attributable choices, not for simultaneous typing. Real-time may be the wrong shape for the product even if it is technically reachable; the thing teams actually seem to need (from the idea file itself) is **non-colliding parallel work plus good merges**, which is [BL-98] + [BL-44], not live cursors. Blocked on the architecture decision — now specifically on `dec:central-host` (`proposed`), which holds git-as-transport vs a hosted MCP vs a hybrid, and records the honest counter-argument that this tool's loop is deliberate and attributable rather than simultaneous. Size **L** | L |
| **BL-100** | **"Rubber-ducking" mode — think it through before committing anything to the graph** | *Idea file, line 7: "work through planning and idea realization before committing concepts to graph… agent-assisted thinking process."* **This collides head-on with a recorded position, which is why it should be decided rather than drifted into:** the git prior-art study explicitly listed the index/staging area under **explicitly do NOT import** — *"continuous-capture doctrine deliberately rejects a staging gate."* Rubber-duck mode is a staging area wearing a friendlier name. That is not a reason to drop it; the original rejection was aimed at a gate that sits between *every* edit and the graph, whereas this is an opt-in mode for a specific activity (early, exploratory thinking where most ideas are meant to be discarded). Note what already works today: a thinking session that simply makes no write calls **is** rubber-ducking, and the read tools (`search_design`, `graph_report`, `propagate_from`, `alternatives_for`) are exactly the ducking aids. So the real feature is narrower than it first looks — **capturing the outcome of such a session in one deliberate act**, with the discarded branches either dropped or preserved as unchosen alternatives (which is what the [BL-70] fork layer just built the vocabulary for: `register_alternative` under a proposed Decision). Design it as *"end the session by choosing what survives"*, not as a staging buffer in front of every tool. Size **M** | M |
| **BL-101** | **The inherited-project onboarding path: "where and how do I add this feature?"** | *Idea file, line 9: "assume you inherit a mature project and your boss says, hey, we need to add a new feature. As the new worker, you don't know where to start."* Half of this exists and is good — `where-am-i` answers *what is this, what has been decided and why, what is still open*, which is the "summarize what it does" clause. **Nothing answers the second and harder clause: where does this new thing GO.** The pieces are all present and unassembled: `search_design` maps the user's words onto the components that already exist; `propagate_from` shows the blast radius of touching each candidate; `alternatives_for` / the Decision layer says what already governs that area and must not be casually contradicted; `hierarchy_issues` and the allocation view say which component *should* own a new capability; [BL-96]'s KPPs say what it must not break. A skill that runs that sequence and hands a newcomer "here is where this belongs, here is what it touches, here are the three decisions you must not violate, here is who authored them" is **arguably reflow2's single strongest demo** — it is the moment the graph pays for itself to someone who did not build it. Cheap relative to its value: a skill over existing tools, no core change. Pairs naturally with [BL-95] (a newcomer must be told which parts of the codebase the graph does NOT cover, or they will trust it too far). Size **S–M** | S–M |
| **BL-102** | **The README does not explain what this is or how it runs on your machine** | *Idea file, line 13: "Needs a simple explanation of the architecture in the main README.md. How it runs on your computer. Very simple how to put the AGENT.md. Need some simple use cases. Probably need to create some cool images."* Fair and checkable: the README currently runs Vision → What this is → the hallucination question → the design vocabulary → Layout → three structural axes → phases → status → heritage → license, with the entire "how do I run this" reduced to **one line at the top pointing at `getting-started/SETUP.md`**. A reader who lands on the repo cold learns the philosophy before they learn that it is an MCP server that keeps a graph in a file in their repo. Wants: a one-paragraph plain-language architecture ("a binary you run locally, a graph in a file in your repo, your coding agent talks to it over MCP"), the AGENTS.md placement in two lines, two or three concrete use cases (greenfield capture, brownfield adopt, "what breaks if I change this"), and diagrams. Adoption-critical and cheap — it is the same `req:frictionless-update` family as BL-51, and the first thing a new user or Anthony's brother's colleagues actually hit. The images are the only expensive part; do the prose first and do not let it block on them. Size **S** (prose) **+ M** (diagrams) | S+M |
| **BL-103** | **Read LangChain / LangSmith for anything worth importing** — *the same exercise was run on `github-mcp-server` first (2026-07-25, [github-mcp-nuggets.md](github-mcp-nuggets.md)); LangSmith is still open* | *Idea file, line 15: "Check out langchain / langsmith — is there something we can take from that?"* The precedent says this pays: the git prior-art study (2026-07-22) is what produced the entire merge thread — [BL-80], `dec:merge-three-way`, `dec:merge-conflict-semantics`, rerere — from one focused read of another system's model. **Scope it the same way, and carry one caveat in from the start:** LangSmith's centre of gravity is *run tracing* — recording what an agent did, step by step — and reflow2 has explicitly decided against reasoning from run history (`dec:loop-status-state-not-history`: "loop debt is computed from graph state, never from run history", straight out of the BL-74 fleet trial). So the tracing core is something reflow2 has already considered and rejected, and a study that comes back recommending it has not read the decision. The parts likely to pay: **evaluation datasets and feedback loops** (directly relevant to the trials programme, which is this project's entire validation mechanism per the standing `unvalidated_capability` disposition, and to [BL-95]'s coverage measurement), and LangChain's **retrieval/document-loader** patterns for [BL-97]'s folder ingest. Produce the same artefact the git study did: applicable imports ranked most-valuable-first, and an explicit **"explicitly do NOT import"** list with reasons. Size **S** (the study) | S |
| **BL-95** | **The design cannot see what it was never told about — no detector takes a file on disk as its subject** | *Anthony, 2026-07-24, from the storyflow adopt pass: "it did kind of a superficial job… my assumption is that the agent should fill in more and more detail as I develop."* All **26** gap sources reason about nodes **already in the graph** (unsatisfied requirement, unrealized capability, unverified capability, …); not one takes an unmodelled file, module or subsystem as its subject. So a graph covering 30% of a codebase reports the same **"0 open gaps"** as one covering 100% — and the unmodelled fraction is largest exactly where the codebase is largest, i.e. where a design brain is worth most. reflow2 will nag forever about a capability it knows is unverified and say nothing at all about ten subsystems it has never heard of. The only existing counterpart is `reconcile_artifacts`, which does report files "present but unknown to the design" — but only for paths the **agent** hands it, so it is discipline, not a detector; nothing fires on its own. **Evidence it does not self-correct in practice:** merge.rs + alternatives.rs (1,886 lines, *shipped in v0.10.0*) sat unmodelled for two days inside reflow2's own repo and nothing fired (BL-70 session, found only by looking); the whole temporal axis went unused for weeks, equally silent; and BL-74 — from Anthony's own StoryFlow fleet — is the same failure at the loop level (skills silently stop under load). The graph deepens where something forces the issue and stays flat everywhere else. **Design, and the trap to avoid:** the measure must NOT be a file-count ratio — that would punish exactly the modelling the adopt skill mandates ("one Artifact per meaningful unit, not per file; a vendored or generated mass = one opaque Component; granularity tracks distinct contracts, not LOC"). Measure **claimed regions, not files**: for each directory subtree in an agent-supplied sweep, does *any* node claim it (a `location` inside it)? Report unclaimed regions **ranked by mass**, so the biggest silences sort first, and one opaque Component legitimately claims the vendored mass beneath it. **Shape:** a new op `coverage_report(observed_paths, exclusions)` — the sibling of `reconcile_artifacts`, in `drift.rs`, same contract (**reflow2 performs no file I/O**; the agent sweeps and supplies). Reports; never scores, never blocks (`dec:report-dont-judge`). Exclusions (build output, vendored, generated) are **named as excluded**, never silently dropped (rule 6). Because `detect_gaps` takes no arguments and must stay state-derived (`dec:loop-status-state-not-history`), the sweep optionally **records** its result as a node the way reconcile records a `DriftEvent` — then a gap can be raised from graph state, and the report must say **when the sweep was taken** so a stale coverage claim is never presented as current (the `cap:freshness` precedent). **Adopt should end with a sweep**, so a thin pass is *measured* rather than felt. **DONE 2026-07-27** — `coverage_report` on the core and the MCP surface, and wired into the adopt skill's Phase 4 so the sweep actually happens rather than being available. Unclaimed paths roll up to the shallowest wholly-unclaimed directory and rank by mass, so a vendored tree arrives as one finding instead of 900 nobody reads. **The trap is pinned by its own test**: 900 files under one registered directory report ZERO unclaimed, because a file-count ratio would have scored correct coarse modelling as 1-of-901 and called it failure. Exclusions come back named with the rule that excluded them; a registered location the sweep never mentioned is named too, so "my sweep was narrower than the design" is distinguishable from "the file is gone". 7 cases. **Deliberately NOT built, recorded rather than half-done:** the sweep is not persisted, so `detect_gaps` cannot yet raise coverage from graph state — that needs a node to record a sweep in (a schema change) and a decision on how stale a recorded sweep may be before its claim expires (the `cap:freshness` precedent). Until then coverage is something a person asks for, and adopt asks | ~~M~~ |
| **BL-94** | **A graph stamped by reflow2 ≤ v0.9.0 cannot be opened by v0.10.x at all — and the recovery paths the refusal names can both be unavailable** | *Anthony, 2026-07-24, from a real project on the current release.* v0.9.0 declared 54 edge types; v0.10.0 retired some to 53 (the edge-orthogonality cut), and every stamp written before BL-86 is **count-only**, so `check_and_stamp` sees an excess it cannot attribute and returns `Err` — from `DesignGraph::open` (`graph.rs:105`). Because *every* entry point opens the graph first, **`--export` goes down with everything else**, which is what makes this more than a message problem: the graph is sealed, not merely refused. BL-86 (shipped in v0.10.1) fixed the *wording* — it now names both recovery paths instead of wrongly saying "rebuild" — but it did not make the graph openable, and **both paths it names have holes**: "export it with the reflow2 that wrote it" needs the old binary that `curl \| sh` already replaced (`req:frictionless-update` defeating the migration path), and "import a committed export" needs the project to have committed one. The undocumented escape is deleting the sidecar `.reflow2/graph.meta.json`, which opens the graph as unstamped — but that silently drops any genuinely-retired-type edges, i.e. it performs exactly the loss the refusal existed to prevent, without naming it. **Fix shape: a `--migrate` path** that opens past the stamp check, drops retired types, **names what it dropped**, and re-stamps — one step, no old binary, nothing silent. **BL-86's entry explicitly declared this option "moot" once the message became unambiguous; the field disproves that** — an unambiguous message you cannot act on is still a wall. Bears directly on `req:survives-upgrade` ("an existing graph opens, or is refused loudly with what to do") — today the second clause holds and the first does not. Update BL-86 and the requirement when it lands | S–M |
| **BL-42** | **The adopt pass has a noise floor: two defects produce half its output — DONE 2026-07-20** | From the `storyflow trial` (private trial record), the first real `adopt` run (2,643 files, 122 nodes). **(a)** `unrealized_capability` fires for every capability whose artifacts were modelled coarsely — 13 of 51 gaps — so the skill's own granularity instruction is punished by a detector; BL-23 fixed this exact shape for `unverified_artifact` by counting rather than asking, and the same is owed here. **(b)** The DETECT/HEAL double-count, reproduced a **fourth** time and now dominant: 20 of 31 defects are `orphan_node` on requirements `detect_gaps` already reports as `unsatisfied_requirement`. Together they were ~40% of the pass's output. **Both fixed, and re-measured on the same 122-node storyflow graph: gaps 51 → 38, defects 31 → 19, total output 82 → 57, with every true finding preserved** (12 unsatisfied requirements, 16 unmotivated capabilities, the generation↔media cycle). **(a)** `unrealized_capability` now reads a claim the modeller already made rather than guessing from topology: a component marked `realized` asserts it exists, so an absent artifact describes the artifact layer's coverage, not a hole in the design — while `planned`/`in_progress` still gets the forward-looking question. The number is kept as `graph_report.realization` (BL-23's bargain: drop the question, keep the count). No threshold, per BL-5's lesson that a loud detector needs a different question rather than a tuned number. **(b)** HEAL's `orphan_node` no longer covers Requirements or Capabilities — DETECT asks both, they were never repairable (a `generate_owner` stub `apply_heal` can never apply), and the docs' own division puts meaning in gap-surfacing. The Artifact orphan stays, because DETECT has no counterpart. Closing that also required teaching `unallocated_capability` that a Flow is structure, or a loose capability on a process-only graph would have gone silent. Four pinned tests flipped honestly | ~~S + M~~ |
| **BL-43** | **`graph_report` cannot see the provenance layer — DONE 2026-07-20** | Same trial: the import wrote 122 nodes, `graph_report` said **109** — the missing 13 are exactly the Fragments, which `report.rs`'s type census omits. The provenance ledger that makes every recovered claim checkable (and that BL-40's provenance viewpoint renders) is invisible to the surface an agent reads first, and the node count is quietly wrong. **Fixed**: `total_nodes` is now every node in the graph, counted from the *schema* rather than a second hardcoded list, so a type added later cannot go missing the way `Fragment` did. `design_nodes` keeps the lifecycle-ordered itemisation and a new `other_counts` names everything outside it — provenance, questions, drift events, axis-Z machinery — in the payload and in the Markdown. Verified on the storyflow graph: 122 imported, 122 reported (13 Fragments + 1 DriftEvent itemised). This is rule 6 — no silent caps — applied to reporting | ~~S~~ |
| **BL-41** | **Graph text is data, never instructions — and nothing says so** | **S half done, 2026-07-19** — the standing rule is stated in the three places an agent looks: a section in the consumer AGENTS.md, one line in each of the eight skills at the point where it starts reading graph text, and the server's `get_info` instructions (so a session that loads no skill still gets it in the handshake). The one genuinely uncovered LLM failure mode in [partnership.md](partnership.md): every skill tells the agent to read node text and act on the design, and a hostile or careless statement rides that trust. Bounded today (single user, local graph); real the day a graph is shared (BL-12) or an adopted repo's prose flows through INGEST (BL-27). Mechanical mitigation (provenance-aware trust, quoting boundaries — [BL-12](#bigger-threads) sketch idea 2 is its design seed) is **M** and should wait for a real multi-writer case | ~~S~~ + M |
| **BL-40** | **Viewpoints as pure projections (SYNTHESIZE held to a no-extrapolation standard)** | **First increment done, 2026-07-19**: the catalogue doubled — operational flow (from BL-37's machinery, retiring the seed's standing confession), as-released (from BL-34's), and decisions views join the original three; `--graph-path` projects a live graph via `--export`; [viewpoints.md](viewpoints.md) is the catalogue with the no-extrapolation rules and the not-yet-projectable list. The graph stores the design; the agent only renders, and each confession is a gap by definition. **Second increment done, 2026-07-19**: evolution (axis Z — PRECEDES solid, `sequence` dotted and cross-checked, floating ChangeEvents confessed) and provenance (authored-vs-inferred, the Fragment ledger, dangling YIELDED confessed) complete the projectable rows — 8 views rendered. **Remaining direction** (the author intends to expand this thread): once the catalogue's shape settles, projection data as typed core read tools on the MCP surface (`flow_report` is the template), so the in-session agent renders without an LLM in the projection path. As-fielded and measures landed with BL-9 / BL-11 — **all ten catalogue rows now render** | M–L |
| **BL-29** | **`apply_heal` trusts the proposal; merge loses data silently — DONE 2026-07-20** | Every hazard closed: the chained-merge case reproduced and fixed, and the survivor rule decided by the user (provenance wins, id breaks ties). See below | ~~M~~ |

**BL-39 · A design cannot be loaded into a running session — DONE** — *found while trying to use the
consumer skills on reflow2's own 96-node design, 2026-07-19.*

Three facts compose into a dead end, each reasonable alone:

1. `reflow2-mcp` takes an **exclusive RocksDB lock** on the graph while serving ([BL-12](#bigger-threads),
   single writer) — verified: `--export` against a served graph fails with `LOCK: Resource
   temporarily unavailable`.
2. The binary has **`--export` but no `--import`**, so a script can read a design out without
   speaking MCP and cannot write one back.
3. The only bulk write path is the `import_graph` **tool**, which takes the entire document as one
   argument.

So a design produced by any means other than the live session — a script, another machine, a
committed export, a backup — can only be loaded by passing the whole document through the tool
boundary. For reflow2's own design that is a 42 KB argument. The practical effect is that the
consumer skills (`where-am-i`, `check-health`, `detect-and-ask`) can only ever see a graph the
session itself built, which is exactly backwards for a tool whose selling point is that a design
outlives the session.

**Done.** `reflow2-mcp --graph-path <dir> --import <file>` is the sibling of `--export`, and takes
`-` for stdin so an export on one machine pipes into an import on another. Upsert, matching the
tool. It reports what landed *and what did not* — an import that quietly skipped half a design would
be the worst kind of success — so `skipped_edges` is printed by name.

The lock stays, because single-writer is the storage model rather than an oversight, but it is no
longer a mystery: the raw RocksDB error ("Resource temporarily unavailable") is translated into
*"another process already has the design graph open… stop that server and run this again."* That was
the actual friction — the failure gave neither the cause nor the fix.

Verified end to end in `smoke_mcp.py`: reflow2's own 116-node design imports from a file and from
stdin, the CLI round trip is byte-identical, a held graph is refused with the explanation, and a
document that is not an export is refused by name.

*What it unblocks.* The consumer skills (`where-am-i`, `check-health`, `detect-and-ask`) run against
the live graph, so before this they could only ever see a design the session itself built. A
committed export, a backup, or a design built elsewhere is now one command away from being the graph
the skills read — which is the point of a design that outlives the session.

**BL-38 · The golden thread has two valid shapes at P3 and the detector accepts one — DONE** —
*[self-host functional design](trials/2026-07-19-selfhost-functional-design.md), 2026-07-19.*

Verified in isolation. `REALIZES` is declared `from: Artifact, to: "*"`, so both of these are
schema-valid, and `link_artifact` invites either by taking any `target_type`:

```
Artifact REALIZES Component  : capability reported unrealized?  True
…plus    REALIZES Capability : capability reported unrealized?  False
```

Modelling *the file realizes the module* — which is how code is actually organised — makes every
capability report `unrealized_capability`, 11 of 33 gaps on reflow2's own design, for capabilities
shipping in the binary that reported them. The connecting path exists and is not walked:
`art:detect -REALIZES-> cmp:detect <-ALLOCATED_TO- cap:detect`. `detect_unrealized_capabilities` asked
only for `incoming(cap, REALIZES)`. **Fixed**: a capability also counts as realized when an artifact
realizes a Component it is allocated to — the indirect form is the coarser claim (the file builds
the part that owns the capability), which is exactly the granularity BL-23 pushes designs toward.
Measured on the design graph: **33 gaps → 16**, and every surviving `unrealized_capability` is one
of the five genuinely unbuilt capabilities — the graph now reports exactly the open backlog with
zero noise. The true case is pinned: artifacts elsewhere, nothing realizing this capability or its
component, still reported.

Same trial, same item, also **fixed**: `dead_end` no longer fires on a subsystem whose only edges
are `CONTAINS`. The design network's CONTAINS-exclusion stands (*decomposition is not
traceability*); the exemption is scoped to **assemblies** — a component containing other components
speaks through its children, which are flagged individually if disconnected. A contained *leaf*
hosting nothing is the true case and still fires; there is a test for each direction. Defects on the
design graph: **36 → 34**.

**BL-5, second pass · `single_point_of_failure` above fixture scale — DONE** — *the
[self-host functional design trial](trials/2026-07-19-selfhost-functional-design.md).* 22 of 36
defects on a 96-node design, post-first-fix: nearly every requirement and mid-level capability named.
The [original fix](#closed) asked whether removal *increases* the count of non-trivial components,
which is the right question about *topology* — and a golden thread is a tree, so most internal nodes
still pass it. It turned out to need a different question, not a threshold: **only things that
operate can fail.** The suggested fix is literally `add_redundancy`, and redundancy is only coherent
for running parts — a second copy of a sentence adds no resilience, and a capability's failure *is*
its component's failure, already reported there. An intent node being an articulation point is the
golden thread working: every Requirement is *supposed* to be the hub of what satisfies it. SPOF
candidates are now scoped to `Component` / `Interface` / `Resource` / `Environment`, on top of the
existing separation test.

Measured: **22 → 4**, and the survivors are exactly the ones judged plausible before the change —
`cmp:service` (all agent access through one surface), `cmp:init` (the only installer), `cmp:export`
and `ifc:graph-export` (the sole core→kit bridge). With this, **every one of the instrument's 16
gaps and 14 defects is true** — the first instrument at zero known-false output. Two of the
surviving defects are themselves worth noting: the `rel:v020 + env:dev` island is reflow2
independently reporting [BL-34](#next-up)'s consequence, and the `cmp:verify` / `cmp:operate`
islands found a genuine omission in the committed design model (the P4/P5 write side has no stated
capability — fix the model, on the record, per [sharpening.md](sharpening.md) §2).

**BL-37 · reflow2 cannot model a process — DONE** — *modelling the coherence loop itself, 2026-07-19
(`tools/model_the_loop.py`, exported to [loop-model.json](loop-model.json), drawn in
[loop-dag.html](loop-dag.html)).*

**Done, to two decisions taken 2026-07-19.** The write side is `add_flow` + `part_of_flow`
(`step_order` was already in the schema); `TRIGGERS` gains a free-form `role` property — a
backward-compatible property addition, counts stay 27/54 — so *feeds* and *forces a resync* are
distinguishable, which was the entire subject of the model. The cycle question was decided as
**report, don't judge**: `flow_report` states a flow's cycles as facts of the process (one
representative per strongly-connected cluster, deterministic), and `circular_dependency` stays
scoped to `DEPENDS_ON` and contracts, where a cycle really is a defect. Two diagnostics stopped
assuming the subject is a product: `concept_without_design` counts a Flow as structure, and
HEAL's `orphan_node` counts flow membership as an anchor. Anything unstated is confessed —
including a `PART_OF_FLOW` edge to a capability that does not exist, which the smoke layer caught
the report silently tolerating (the storage engine accepts dangling edges; only the published
surface shows it). Measured: the loop model's 4 frictions → **0** (`model_the_loop.py` is now the
fifth instrument, non-zero on regression), its defects 10 → 4 with every survivor true — the
remainder is the recorded A14 day-one shape on [BL-27](#bigger-threads), and wider process-aware
diagnostics stay with [BL-16](#bigger-threads). The other four instruments are unchanged. *Original
entry:*

Distinct from the self-host trials, which modelled reflow2's **product** — it has a detect
capability, a component per module. This modelled reflow2's **process**: the DAG of how the phases
feed each other, including the backward edges where the build teaches the design what it is. Its own
operating model is a design, and *design anything* is the claim. Four things got in the way, all
verified by attempting it:

| Friction | Detail |
|---|---|
| **`Flow` has no write side** | Fully specified in `functional.yaml` — `flow_type: process/control_flow/decision_flow`, `entry_point`, `exit_point`, with `PART_OF_FLOW` running Capability → Flow — and there is no constructor in core and no MCP tool. `node::FLOW` appears only in `report.rs` (counted) and edge classification. **The one type meant for "an ordered process that links Capabilities end to end" cannot be created**, and this model is its exact use case. Recurring lesson, eleventh instance |
| **Edge roles are lost** | Forward *feeds* and backward *forces a resync* both had to become `TRIGGERS`, declared `* → *` with no role property. The backward edges are the entire subject of the model and the graph cannot tell them from the forward ones. `PART_OF_FLOW` would not fix this either — it carries membership, not order or direction of influence |
| **Cycles are invisible** | The loops are the point, and `circular_dependency` does not fire: it walks `DEPENDS_ON` and contracts, not `TRIGGERS`. A process model's cycles are its most important feature and nothing reads them. Note the tension — in a *product* a cycle is a defect, in a *process* it is the design, so this is not simply "add TRIGGERS to the walk" |
| **The diagnostics are product-shaped** | `concept_without_design` fired on zero Components. A process has no Components, so the phase detectors assume the subject is a product. Execution evidence for [BL-16](#bigger-threads), which asks whether "design anything" survives contact with a non-software domain — here it does not survive contact with a non-*product* one |

Size **M**: a `Flow` constructor and tool are **S**, but edge roles and process-aware diagnostics are
the real content, and the cycle question needs a decision before code.

**BL-35 · A design claim has no last-confirmed date — DONE** — *[coherent-erosion
trial](trials/2026-07-19-coherent-erosion.md), 2026-07-19. The deepest of the phase-coherence items.*

*The good news first, because it changes what the rest of these are for.* The
[coherent-erosion trial](trials/2026-07-19-coherent-erosion.md) ran the same five fix cycles with
axis-Z discipline — every fix a `record_change` at its own epoch, and the behaviour-changing fix also
updating the **P1 capability**, the design following the build backwards. It works: the design ends
describing what was actually built, **the original intent is still recoverable** from a Snapshot
pinned to the baseline epoch, and every fix is on the record. `designed == released` is reachable
today, and letting the build teach the design costs no intent because Z keeps the past. The
vocabulary was already waiting for this loop — `ChangeType::TestFailureFix` is documented as *"a fix
forced by a failed verification"* and `ChangeType::Resync` as *"a re-sync back to coherence."*

*And the problem.* Run both versions and reflow2 returns **the same verdict**:

| | eroded run | coherent run |
|---|---|---|
| design describes what shipped | no — fiction | yes |
| `detect_gaps` | `[]` | quiet |
| reflow2's verdict | **coherent** | **coherent** |

The entire difference is developer virtue, which is exactly what does not survive cycle 40 of a
release crunch — and exactly what the original reflow was relying on without knowing it.

*The missing concept is a date on the design's own claims.* Structural completeness is all that is
measured — is there a Capability, does something satisfy the Requirement, does an Artifact realize
it — and every one of those is true in the eroded graph. What is absent is **when a claim was last
confirmed against reality**. A description written at the baseline epoch and never revisited while
its artifact drifted five times is a different and worse state than the same description confirmed at
the release epoch, and nothing tells them apart.

**Done — as the confirmation ledger** (`confirm.rs`, `confirmation_ledger` on the surface, a
rollup in `graph_report`). Per capability with built artifacts, three states that were previously
one: **`drifting`** (an observed divergence is unanswered — also a persistent 0.75 gap,
`unresolved_drift`, because the session that reconciled may not be the session that answers),
**`confirmed`** (examined, with the claim history visible: design_holds vs design_updated counts,
design edits, `last_claim_at` from dated claims), and **`unexamined`** (nobody has ever looked —
*no longer the same as confirmed*, which was the whole point). Two supporting facts landed with it:
`DriftEvent.resolved` — declared in the schema with `default: false` and never written by anything,
the **twelfth** recurring-lesson instance — is now flipped by the accept that answers the drift; and
an accept's `CHANGED` edge is marked `accepted_baseline: true`, so a disposition claim is
distinguishable from ordinary change history on the same artifact.

Deliberately *not* built: lie detection. Five `design_holds` claims with zero design edits is the
erosion signature and the ledger makes it legible — but judging whether a specific claim was false
is semantic, and a deterministic detector would fire on every stable design with cosmetic churn
(the `unexpected_coupling` lesson). The ledger reports; the human judges.

Measured: erosion **5/8** ("the design reports how the code moved and how each move was answered" —
the signature line reads *5 drifts, 5 claims, 0 edits*), coherent-erosion **6/9** ("which fix moved
the design" — *1 design-updating accept vs 4 design-holds, cycle 4 is the one, and the ledger says
so*), smoke green with the full drift → gap → answer → ledger loop over the real binary.

**BL-36 · `precedes` is unreachable, so the epoch chain cannot be drawn — DONE** — the `precedes`
tool orders one epoch after another, and the coherent-erosion trial now draws the chain cycle by
cycle and walks it back out of the export: `baseline → fix1 … fix5 → release`. With it the coherent
instrument reached **9/9 — the first instrument fully green**, every YES a genuine read. (Its probe
was nearly shipped as a hardcoded `True` and caught in review — the instrument-accommodation trap
from [sharpening.md](sharpening.md) §4, live again.)

**BL-33 · Accepting drift is one-sided; the drift record overwrites itself — DONE** — *[erosion
trial](trials/2026-07-19-erosion.md), 2026-07-19. The mechanism behind the user's account of
reflow1, and the load-bearing item of the three.*

*The failure is not an event.* It is `write → test → fix → test → fix → … → release`, where every
step is legitimate — a test failed, someone fixed the code — and nobody ever decides to diverge.
Detecting "this file changed" barely helps, because the answer is always *"yes, I know, I fixed a
bug."* **Verified:** five fix cycles on a coherent thread, the fourth quietly widening an
idempotency window from 24h to 7 days, then a release. Afterwards `detect_gaps` returns **`[]`** —
the design describes a system that no longer exists and reports perfect coherence, because what is
measured is whether the bookkeeping is complete, never whether it is true.

*Two halves.*

**Accept is one-sided (M).** Each cycle ends at `set_artifact_checksum` — "an accepted change is the
new baseline" — which updates the code-side baseline and **asks nothing about the design**. That is
locally reasonable and globally fatal. Nothing ever poses the second half: *the code moved, should
the design move too, or was the code wrong?* **Done, to the coherent-erosion trial's specification.** `set_artifact_checksum` now requires a
`DriftDisposition`: `design_holds` (the change carries no design meaning — recorded as a dated
`ChangeEvent` claim, deterministic id so re-accepting the same state is idempotent) or
`design_updated` (naming the `record_change` event from the design-side edit, which is then
`CHANGED`-linked to the artifact — **one change, both sides**, the first `ChangeEvent` in the
codebase originating from the build). A phantom `design_change_event_id` is refused before the
baseline moves — and the refusal caught the coherent trial itself accepting before recording, live.
The claim can still be wrong (the erosion trial's careless actor claims `design_holds` five times,
including the lie), but it can no longer be silent: it is dated, typed and auditable, which is
exactly what BL-35's freshness check reads. Measured: erosion 3/7 → **4/8** (new probe: every
accept answers the second question), coherent-erosion 4/9 → **5/9** ("anything prompted the update"
is now genuinely yes — the tool poses the question at the moment it matters). The principle it was
meant to embody — the capability description is updated to match what was built, or the
divergence is marked a defect in the code. The third option, "accept the file, leave the design
alone, say nothing," is the one that erodes and should not exist. Note this is the first thing in
the codebase that would make a `ChangeEvent` originate from the *build* side rather than the design
side, which is the right shape: a fix is a change, and CHANGE is a first-class axis.

**The record overwrites itself (S) — done.** The mechanism was subtler than first recorded: the id
(`artifact | kind`, no discriminator) didn't overwrite, it **skipped** — `write_drift_event` returns
early when the node exists, intended as dedup for re-observing the *same* unresolved divergence. The
defect was that a **new** drift hashed to the same id and was silently dropped: five drifts left one
event. Fixed by making the observed checksum part of a `checksum_change` event's identity — the
event *is* "the artifact became X while the design believed Y", so re-observing the same X dedups
and a later drift to X′ is a new event. State-shaped kinds (`missing_artifact`,
`undocumented_addition`) stay keyed on artifact + kind: "still missing" re-observed is the same
unresolved divergence. Measured: the erosion trial retains **5 events for 5 drifts**, and its probe
was tightened from `> 0` (which one surviving event weakly satisfied) to an exact count. Axis Z's
*never overwrite the past* now holds on the as-built side; "drifted once" and "drifted N times" are
different graphs, which is the data BL-35's freshness computation needs.

**BL-34 · There is no as-released view, and no vocabulary for one — DONE** — *same trial.* Checked two
ways: **`DEPLOYED_TO` (Release → Environment) is the only edge in the schema involving `Release`.**
Nothing links a Release to the Artifacts or Components it shipped, though `Release`'s own
extraction hint says *"A packaged, operable version of some Components/Artifacts"* — the intent is
prose with no edge to carry it. So *"does what we released match what we designed?"* is not an
unimplemented query; it is inexpressible. reflow2 has as-designed and a partial as-built, and the
third view — the one the user actually lives with — has no structure at all.

**Done.** `INCLUDES` (`Release → [Artifact, Component]`) is edge type **54** — the first edge-type
addition since the stamp existed, so the BL-19 mechanism now applies for real: a graph written by
this schema is **refused by older binaries**, loudly, with what wrote it. The upgrade order in
SETUP.md matters for the first time. `as_checksum` on the edge freezes the artifact's hash *as
shipped*, because the artifact node's own checksum is the live drift baseline and moves with every
accept — without the frozen copy a past release's manifest would quietly rewrite itself, the axis-Z
sin again. Write side `release_includes`; read side `release_report` — shipped artifacts with
cut-time checksums, capabilities covered (both P3 shapes), **`built_capabilities_not_covered` as
the as-released diff**, deployments. A new gap, `unreleased_component` (0.5), fires for a built
component no release includes — double-gated on releases existing *and* contents being modelled,
so day one of the first Release node is not a flood (the ophyd-A14 lesson). `pin_at_epoch` is also
on the surface now (thirteenth recurring-lesson instance: `AT_EPOCH` is `from: "*"` and the core
fn existed with no tool), so a Release joins its `release_cut` epoch. Three pinned history tests
flipped honestly: BL-1's own example pair — *"nothing models Release → Component"* — now has its
exact fit, which is the trial's question answered two items later. Measured: phase **10/13** (P5
1/2), erosion **7/8**, coherent-erosion **8/9** — the single remaining coherent miss is BL-36's
`precedes`.

**BL-30 · The later phases measure bookkeeping, not reality — DONE** — *[phase-coverage
trial](trials/2026-07-19-phase-coverage.md), 2026-07-19. The direct answer to "how do we know
reflow2 doesn't repeat reflow1?"*

**Verified, three times, twice in isolation from the harness.** `build_without_verification` fires
when a capability has no `Verification`. Attach one and set its status to `failing`, and the gap
**closes**:

```
no verification at all      : ['build_without_verification', 'no_deploy_operate']
a verification that FAILS   : ['no_deploy_operate']
```

The gap asks *"How will you confirm `<Capability>` actually works?"* — and is answered by a test
proving it does not. The failure is invisible everywhere else too: with status written as `failing`,
`detect_gaps`, `detect_defects` and `graph_report` are byte-identical to the `passing` case.

The general form, and the reason this is the thread's most important item: **the P4/P5 detectors ask
whether a node exists, never what it says.** A design that counts test nodes and ignores test results
is precisely one you may as well have ignored once building started.

Two pieces. **S — done.** A `failing` verification now raises `failing_verification` at severity
0.8 — above every absence-shaped gap, because a requirement nothing satisfies is work not started
while a failing check is work *proven broken* — anchored to both the check and what it checks.
`build_without_verification` still closes when the check exists (the "how will you confirm this?"
question *is* answered); the difference is the silence is now filled with the right signal instead
of nothing. And `verification_coverage` counts a check that **passes**, not one that exists —
`planned`, `failing`, `skipped` and `blocked` all mean "not currently confirmed". Measured:
`phase_trial` P4 1/4 → 2/4, `erosion_trial` 2/7 → 3/7 (whose coverage probe also went from a
hardcoded fail to a genuine check). Passing and failing graphs are no longer byte-identical to
DETECT, which was the headline miss. **M — done, 2026-07-19** —
`reconcile_verification` (`verify.rs`), the P4 sibling completing the reconcile family: the agent
supplies what the run actually reported (`passed`/`failed`/`skipped` per check; anything else
rejected by name, the batch survives) and the graph names each divergence from what it believed.
"Recorded `passing`, run reported `failed`" — believed proven, actually broken, the reflow1
failure in miniature — sorts first and records at severity high. Divergences are persistent
`unresolved_drift` gaps with P4-appropriate advice, auto-resolved when a later run agrees; the
event identity is the (declared, observed) pair, so flapping history stays visible per axis Z.
A partial run is never read as absence; `exhaustive` names the passing/failing claims the run
did not cover. Measured: **phase trial 13/13 — the first fully-green run of the instrument that
exists to measure the failure that sank the original reflow**, and its P4 probe now injects the
divergence rather than checking a tool exists. With BL-9, all three feedback loops (P3/P4/P5)
now close; this is also adoption's dynamic-analysis receptor (see the RE-lifecycle mapping under
[BL-27](#bigger-threads)).

**BL-31 · A `status` field is a claim nothing checks — DONE** — `status_contradiction` (0.70,
self-contradiction family: below reality-contradiction at 0.75/0.8, above absence). Scoped to the
two unambiguous cases — a Capability `verified` that no *passing* check verifies, and a Requirement
`met` that nothing satisfies. The second matters doubly: `met` silences `unsatisfied_requirement`
by design, so before this a lying `met` was invisible to everything. Deliberately not extended to
`realized`-without-artifact, which is already an absence gap — double-reporting would be the
DETECT/HEAL double-count in a new costume. **Its first catch was our own model**: `cap:kit` claimed
`verified` in the committed design graph and nothing automated checks the installer — ruled per
[sharpening.md](sharpening.md) §2 (the status was wrong, downgraded to `realized` on the record),
the second true self-report and the first lie caught in our own graph. Measured: phase **11/13**
(P4 3/4). *Original entry:* — *same trial.* `Capability.status` set to
`verified` on a capability with no `VERIFIES` edge raises nothing. `unverified_capability` fires, but
it fires either way — "this is unverified" is not "the design contradicts itself", and only the
second is a coherence failure. Same for `Requirement.status = met` with nothing satisfying it, and
`Component.status = realized` with no Artifact. Sharpened by [BL-27](#bigger-threads), which made
these fields easy to write for the first time. A `status_contradicts_structure` detector, **S**, and
it belongs in DETECT — the answer is either "fix the status" or "fix the structure", which is a
question for the user.

**BL-32 · A running MCP server silently serves a stale surface — DONE** — *same trial, found by nearly
running it against the wrong binary.* Rebuild `reflow2-mcp` mid-session and the already-running
server keeps serving the surface it started with: tools added since are absent, detectors keep the
old behaviour, and nothing says so. Distinct from [BL-18](#bigger-threads), which compares an
installed kit against the remote HEAD — this is process-lifetime skew and hits agents and developers
mid-session. `tools/smoke_mcp.py` cannot catch it by construction, since it spawns a fresh binary per
run — the fourth "a client we wrote agreed with itself" in this repo's history. **Done.** `graph_report` carries `served_by` — the crate version compiled in, plus the binary's
mtime (best-effort, `None` over a guess) — so a session can see it is talking to a binary older
than the code around it, and the upgrade doc's step 4 makes checking it the post-restart ritual.
The consistency check that pins it (handshake version == report version) immediately caught a
pre-existing bug: `Implementation::from_build_env()` expands in **rmcp's** build env, so the server
had introduced itself as the MCP *library's* version ("2.2.0") since the surface existed — the one
field a client could see, wrong all along. It now reports its own name and version.


**BL-29 · `apply_heal` trusts the proposal, and merge loses data silently** — *found 2026-07-19
while scoping [BL-27](#bigger-threads)'s duplicate detection; the reason `possible_duplicate` is a
DETECT gap and not a HEAL defect.*

**The headline was verified by running it, and is fixed.** A hand-crafted `HealProposal` — a made-up
`issue_id`, a `Merge` naming two capabilities with no `DUPLICATES` edge, which `detect_defects`
reported only as `OrphanNode` — was accepted and applied: `applied=true, operations_applied=1`, node
gone. `ApplyHealReq` deserializes caller JSON straight off the MCP surface, so any client could do
it, and a merge has no snapshot and no undo. `apply_heal` now re-derives what HEAL would propose for
the graph as it stands and refuses anything that does not match, **before any write**. The
issue→operation mapping is shared by propose and apply so the two cannot drift.

Related and worth remembering: `requires_human_review` is computed per-*proposal* and `apply_heal`
has never read it. It reports that generative stubs exist; it is not and never was a gate on
applying the structural half.

| Hazard | Status |
|---|---|
| A proposal HEAL never made is applied verbatim | ✅ fixed — refused before any write; stale proposals fail the same way |
| `remove`'s node properties are discarded entirely, so name/description/status vanish with no report | ✅ fixed — reported in `HealReport.discarded` |
| Edges to nodes absent from the index are dropped with no report | ✅ fixed — reported in `discarded` |
| `create_edge` is an upsert on `(graph, type, from, to)`, so where both nodes had the same triple, `remove`'s edge properties overwrite `keep`'s | ✅ **reported**, not prevented — `discarded` names the collision. Preventing it means deciding which side wins, which is a merge-policy question, not a bug fix |
| `DUPLICATES` declared `from: "*" to: "*"` yields a schema-valid cross-type merge | ✅ fixed in code — refused at proposal time with a reason. **The schema is still `*`/`*`**; narrowing it is the tighter fix and was left alone, since it would reject edges existing graphs may already hold |
| The node-type index is built once before the loop, so chained merges (a↔b, b↔c) re-point onto a node that no longer exists | ✅ **reproduced, then fixed** (2026-07-20). Worse than code-read suggested: both merges are individually sanctioned, the dangling edge is *accepted* by the storage layer, and the report said `applied=2, verified=true` over a corrupt graph — silent corruption with a green verification, one hash-ordering away from `propose_heal`'s own output. Now: propose emits one merge per chain (rest deferred with the reason stated), apply refuses any proposal whose merges share a node before a single write, and a third-party `DUPLICATES` edge is re-pointed onto the survivor so the chain's unresolved claim survives and the loop converges one round per link. Two silent drops found in the same repro also fixed: the chain claim vanishing with the merged node, and a real pair-joining edge dying without a `discarded` entry |
| Atomicity is per-operation, not per-proposal: a three-merge proposal failing on the second leaves the first committed | ⬜ open, code-read — but **no known trigger remains**: the shared-node refusal makes a proposal's merges node-disjoint, the cross-type guard makes every re-pointed edge type-valid, and unknown endpoints are discarded rather than errored, so a mid-proposal failure now needs a storage-level error. Closing it fully means one batch per proposal, which would make mid-batch reads stale — trading a real correctness property for a theoretical one. Revisit only with a reproduction |
| The survivor is chosen by lexicographic id (`canonical_pair`), not by connectivity or completeness — the better-connected node may be the one deleted | ✅ **decided by the user and built** (2026-07-20): **provenance wins, id breaks ties.** A merge keeps only the survivor's properties, so the choice decides whose words are kept — and the old rule let an `inferred` stub delete an `authored` node's text on id order alone. The rank follows how directly a human stands behind the text (`authored` > `planned` > `imported` > `reconciled` > `inferred` > `healed`); equal rank falls back to the smaller id, so the choice stays fully deterministic and pre-provenance graphs behave exactly as before (absent property = the schema default, `authored`). Connectivity/completeness rules were considered and rejected as unstable — one bookkeeping edge would flip the winner. Pinned in three directions: authored beats inferred against the id order, the graded order (inferred > healed), and the tie fallback |

With that, every hazard on this item is closed — the last one by the user's survivor-rule
decision (option 2 of the alternatives put to them). **BL-29 is done.** The decision itself
belongs in the design graph as a Decision node; add it in the first live-server session
alongside the stub-survivor reconciliation.

## Closed

Kept as a short pointer so a stable id never dangles; the detail is in the CHANGELOG.

- **BL-28 · Every `JsonValue` tool parameter was unusable from Claude Code** — done. Six params
  (`gap_to_prompt.gap`, `apply_heal.proposal`, `import_graph.document`, `create_node.props`,
  `create_edge.props`, `reconcile_artifacts.observed[]`) published an untyped schema, so each
  client guessed: grok build sent an object, Claude Code sent a string, the string was rejected.
  Now declared as JSON objects; a stringified object is still refused rather than accepted, since
  taking both shapes would be the silent fallback rule 4 forbids. The regression guard asserts the
  published schema (no advertised property without a type) — the behavioural layers were all green
  while the bug was live. Detail: [trial](trials/2026-07-18-selfhost-genesis.md) §1.
- **BL-22 · Skills are not reliably discoverable** — done. The kit installed `.grok/skills/`
  alone, the narrowest-reach of four harnesses, so a project opened in Claude Code had an
  AGENTS.md naming seven skills the agent could not load. `reflow2_init.py` now installs to
  `.claude/skills/` (read by Claude Code, OpenCode and Copilot) and `.grok/skills/`, and writes
  `.mcp.json`, `opencode.json` and `.vscode/mcp.json` from one generator. Configs are merged, not
  overwritten — which also fixed a silent failure where a project that already had any MCP server
  never got reflow2 installed while the run reported success. Tables and the reasoning:
  [skills/README.md](skills/README.md).
- **BL-20 · Graph export / import** — done. `export_graph` / `import_graph` in core and on the
  surface. Deterministic throughout — node types, ids, edges and property keys all sorted, which
  is why the exported types use `BTreeMap` rather than the store's `HashMap` — so two exports of
  an unchanged graph are byte-identical and a backup directory under git shows *what changed in
  the design* rather than a fresh blob each run. Import is upsert and atomic: a document that
  fails validation leaves the graph untouched, and an edge whose endpoints are missing is named
  rather than dropped. The document carries a `GraphStamp`, so it says which reflow2 wrote it.
  This is the migration mechanism BL-19 wanted: export with the old build, import with the new.
- **BL-21 · The agent can report its own friction** — done. A `report-friction` skill plus the
  trigger in the consumer AGENTS.md, since a skill alone is not reliably found (BL-22). Redaction
  is the load-bearing part: a friction report naturally quotes the graph, and the graph is the
  user's design, so the skill reports reflow2-shaped facts — tool, argument *shapes*, node
  *types*, counts, masked errors — and asks before including anything of theirs. It never files
  without asking, searches for duplicates first, and degrades to a local file when `gh` is absent
  or the repo is unreachable, **which is the normal case: the repo is private**. Also folded skill
  frontmatter validation into `reflow2_init.py`, because a malformed `name` makes a skill fail to
  load with no error anywhere.
- **BL-25 · An answered question stays visible while its gap is open** — done. `open_questions`
  now returns two kinds: `asked` (still waiting) and `answered` **whose gap is still open**, with
  the reply attached. Answering settles nothing by itself — either the answer gets written into
  the design and the gap closes, or the gap is acknowledged; until one happens there is something
  outstanding and the list says so. A question whose gap has closed or been acknowledged drops out
  of the list but stays in the graph. Verified on reflow2's own design: the third session now sees
  the question and the reply, and acknowledging takes it to **0 gaps, 0 outstanding, 1 reviewed**.
- **BL-4 · Asked questions outlive the session** — done. `gap_to_prompt` was the only tool that
  never touched the graph: it phrased a question, returned it, and forgot, so the next session
  re-derived the same gap and asked again. Its serve pass now records a `Question` node at a
  derived id (`question:{gap hash}`), `ASKS_ABOUT` the nodes the gap concerned, with the wording
  the user actually saw. `open_questions` / `answer_question` / `withdraw_question` are on the
  surface, and `where-am-i` reads them first. **New node type** — 27 node types, 53 edge types —
  purely additive, so per BL-19 it is safe for existing graphs. Re-asking updates the wording but
  cannot reopen an answered question; there is a test for that.
- **BL-5 · `single_point_of_failure` measured against the baseline** — done. Not the cause the
  self-host probe guessed (it blamed the `≥2` threshold, by analogy with `surprises.rs`).
  Reproducing the shape showed the real one: the test asked whether ≥2 non-trivial components
  exist *after* removal, which assumes a connected design. One unrelated island already satisfies
  that, so every articulation point elsewhere reported — and attaching the island cleared them all
  at once, which is exactly the trial's *"15 defects vanished when I added two bookkeeping
  edges."* It now asks whether removal **increases** the count. reflow2's own design: 8 defects → 2,
  both true.
- **BL-24 · A Component the Project contains is not floating** — done. `orphan_level` only
  recognised a *Component* parent, and the Project carries no `Component.level` because it sits
  above all of them — so the shape the tools lead you to (a Project holding a few subsystems)
  reported one false gap per subsystem. The Project now counts as a parent; a component nothing
  contains is still an orphan, and there is a test for each direction. Together with BL-23 this
  took reflow2's own design from **25 gaps to 1**, and that one is true.
- **BL-23 · Per-file verification coverage is counted, not asked** — done. One `VERIFIES` edge
  per source file was 22 of 25 gaps on reflow2's own design, on a crate whose capabilities are all
  tested. The rule was not wrong, it was loud, and volume is what makes a list get skimmed.
  `graph_report` now carries a `Verification coverage` line and the gap is gone. Measured on the
  same 119-node graph: **25 gaps → 3**, of which one is true and two are BL-24.
- **BL-6b · `unexpected_coupling` demoted to a signal** — done. The decisive fact was not the
  trials but the spec: [gap-surfacing.md](gap-surfacing.md) names `orphan_node`, `dead_end`,
  `disconnected_cluster` and `single_point_of_failure` as the structural gaps — this was never
  among them, having been volunteered by the graph-analysis work. It is now reported by
  `graph_report` under its own heading, which already existed, so no information was lost. Two
  earlier rounds of tightening had not stopped it firing on correct architecture; an `Interface`
  bridges two clusters by construction, so modelling contracts as instructed made the detector
  penalise every one. `reviewed_gaps` now reports acknowledgements whose detector has been
  retired rather than dropping them, since a trial had already accepted one.
- **BL-2 · Expose `contain_component`** and **BL-3 · `Requirement.status` reachable** — done
  `9ab3da3`. Both needed more than the entry said. BL-2 also had to expose `Component.level`:
  shipping the containment alone would have flagged a false `level_mismatch` on every nesting,
  since everything defaults to `component` — worse than the silence it replaced. BL-3 also had to
  fix HEAL, which unlike DETECT ignored a `dropped` requirement, so marking one would have
  silenced half the system while the other half kept nagging. Recorded as **WS-7**/**WS-8**.
- **BL-6 · Split `unverified_capability`** — done `9ab3da3`. Artifacts now report as
  `unverified_artifact` with wording of their own; detection is unchanged, because proving a
  capability works still does not prove *this file* delivers it. The capability key is frozen
  deliberately: gap ids hash it and acknowledgements are stored under the resulting id, so a
  rename would silently expire every acknowledgement and orphan the Decision where neither
  `detect_gaps` nor `reviewed_gaps` looks. A test pins both keys.
- **BL-1 · Schema discovery tool** — done `9440929`, consumer kit `f00fac7`. `describe_schema`
  plus rejections that name the alternatives. The design turned on one detail worth remembering:
  `EdgeEndpoint::accepts()` returns true for the `*` wildcard, so the naive answer to the trial's
  question would have been `DEPENDS_ON` — the very edge it chose and distrusted. Matches are
  labelled exact vs wildcard for that reason. Recorded as **WS-6** in the coverage matrix.

## Bigger threads

**BL-27 · Adopting a system that already exists** — *user, 2026-07-18.* "reflow2 was designed for
greenfield projects... hoping a `/reverse-engineer` skill would allow you to fill in the graph
based on what's already there." Two sub-problems named with it: codebases with no requirements
documentation, and codebases too large to model in one pass.

All three brownfield trials —
`ophyd-service` (private trial record) (399 files, ~110k LOC),
`3dtictactoe` (private trial record) (~20 files) and
[reflow2 on itself](trials/2026-07-18-selfhost-genesis.md) — had to run GENESIS
backwards, and each recorded the same entry-point finding independently. Call the skill **`adopt`**
rather than `reverse-engineer`: producing the graph is one output, but the job is bringing an
existing system under design control, and it is the sibling of `genesis`, not of a code tool.

*The seeding order inverts, and the gap ranking assumed it hadn't.* **Fixed.** GENESIS deliberately
stops before P2 so `concept_without_design` fires as the productive first gap ("how should this be
structured?"). In brownfield the Components are the only thing that indisputably exists, so that
detector fired at severity **0.7 — above the genuinely valuable gap at 0.6** — and an agent working
the list top-down did the useless thing first. It reproduced on a 20-file project as well as a
110k-LOC one, so it is a property of the path, not of scale. The
[self-host run](trials/2026-07-18-selfhost-genesis.md) added `build_without_verification` (0.65)
firing the same way — "no way to confirm any of it actually works" of a repo with 15 test files and
a smoke test — so the top **two** gaps outranked the third.

*The fix was not the one this entry originally proposed*, and the difference is worth keeping.
The entry blamed the shared maturity inference — both are `scope: phase` detectors reading a
node-type census — and called that the thing to fix. But the inference is *correct about the
graph*: `components == 0` is true, and the `aidrone trial` (private trial record)
recorded the greenfield behaviour as **worth not regressing** ("the skill and the detector agree,
the gap arrives as a question rather than a complaint"). Suppressing the detector would have broken
a case a trial called correct.

The real defect was comparing two incommensurable numbers. Phase nudges carry fixed literals;
`unsatisfied_requirement` computes `0.5 + priority_bump`, which for the default `medium` is exactly
the 0.60 the trials saw — and until [BL-28](#closed) no client on one major harness could write
`priority` at all, so the losing number was a default nobody chose.
[gap-surfacing.md](gap-surfacing.md) already had the distinction: discipline 8 names *retroactive*
(gap-driven) versus *proactive* ("here's what comes next") and puts phase-coverage in the proactive
group, and discipline 3 says concrete beats abstract. So the sort now bands on **anchoring**: a gap
naming nodes describes something wrong *now* and outranks a project-level nudge about what comes
*next*, with severity ordering within each band. Greenfield/brownfield-neutral, and the nudge is
demoted rather than suppressed — with nothing anchored to report it is still the first thing asked.
Pinned in both directions by `tests/detect.rs` and over the real MCP path in `smoke_mcp.py`.

*And the phase problem is not brownfield-only.* Ophyd A14 already reports HEAL emitting maximum
noise on a mid-construction graph, and proposes suppressing allocation-orphan defects when
Component count is 0. The self-host run reproduces that on the **greenfield** path at 18 nodes —
following GENESIS's "do not create Components yet" yields one `orphan_node` per seeded capability,
so genesis → check-health flags a graph that is exactly what genesis prescribed. So A14's fix
should not be scoped to an `adopt` mode; it fires on any project on day one. Related: on that graph
`propose_heal` returns 0 mechanical operations and 14 awaiting generation, so `check-health` has
nothing to apply at all until the LLM backends land.

*Requirements must not be inferred from the implementation.* A requirement backed out of the code
that implements it is satisfied by construction, and a graph of those can never say anything.
3dtictactoe is the controlled proof in the other direction: its one high-value finding —
`game_mode='level_assigned'` validated, stored, and **never read again** — came from
`description.txt`, a source *outside* the code, and turned on the discipline *do not create a
`satisfies` edge you cannot point at code for*. That gives the division of labour:

| Layer | Source | Note |
|---|---|---|
| Capability, Component, Interface | the code | satisfied-by-definition is fine — this is the *as-built* view ([reflow-v3-nuggets.md](reflow-v3-nuggets.md)) |
| Requirement | anything **but** the implementation | the user; tests (a test is a written-down expectation); READMEs and spec files; issues and commit messages; config and deployment; and error handling, validation, retries and locking, where the unwritten NFRs live |

Ophyd is the caution against trusting a found document: its traceability matrix was another org's
PDR, 7 of 25 rows out of scope, and it **omitted device locking — arguably the system's central
correctness property**. An agent seeding only the matrix produces a graph whose most important
invariant is absent. A second caution from the same trial: inferring component *identity* from
source comments produced a phantom external system, because stale naming outlives stale code.
Structure from imports and calls; never from prose.

*Scale is a granularity problem, not a context problem.* Neither trial ran out of context. Ophyd's
~110k LOC modelled as **~78 nodes**: 124 REST endpoints → 9 Interfaces (one per OpenAPI contract,
*not* per endpoint), 1,573 test functions → 8 Verifications, and the vendored queueserver fork —
75k of the 110k LOC — deliberately left as **one opaque Component**. [BL-23](#closed) is why: one
Artifact per source file made 22 of 25 gaps `unverified_artifact`, 88% noise from a *complete*
model. The user's instinct to explore incrementally is right, but the first pass should be
**breadth at deliberately coarse granularity over the whole repo**, because the payoff findings in
both trials were structural and came from breadth, not depth — a *critical* `circular_dependency`
between two ophyd services that the project's own architecture docs never name, surfaced only
because both sides of two Interfaces were recorded, and 3dtictactoe's absent `satisfies` edge.
Then deepen **on demand** — the subtree the user is actually working in — rather than by rotation,
so coverage tracks value and there is a natural stopping point.

*Incremental adoption is blocked until the frontier is modelled.* A partial graph emits gaps
indistinguishable from real ones. Ophyd finding 6 states the general form: the tool "cannot yet
tell 'no capability delivers this' from 'nobody has drawn the edge yet'." Finding 14 adds that the
detectors have no notion of a graph mid-construction — following `check-health` literally would
have fabricated Components over a graph whose real structure had simply not been entered yet, and
the operator declined to run `apply_heal`. Marking unexplored regions so detectors stay quiet
there is a **precondition** for the deepening stage, not a refinement of it. The
opaque-Component treatment of the vendored fork is the existing precedent.

*The orphan-Capability fix, and two things it deliberately left alone.* `unmotivated_capability`
is the mirror of `unsatisfied_requirement`, and its severity reads `Capability.provenance` — 0.55
authored, 0.70 `inferred`. Ophyd asked for it to outrank `unsatisfied_requirement` *"on a
brownfield graph"*, and a fixed number cannot honour that qualifier: the same structure means a
half-finished thought on one path and a feature in production nobody asked for on the other.
Provenance is exactly what separates them, which is the first thing to consume that property.

1. **HEAL was not given the symmetric check**, though it is blind in the same direction. Two
   reasons, and they should be revisited together rather than piecemeal. There is no mechanical
   repair for "no requirement asked for this" — the proposal would be one more
   `requires_human_review` stub on a graph where `propose_heal` already returns 0 applicable
   operations and 14 awaiting generation. And DETECT/HEAL double-counting is *already* a recorded
   complaint (ophyd 15 / 3dtictactoe 10, reproduced a third time in the self-host run); adding a
   fifth pair would deepen it. This is the docs' own division — *HEAL fills structure; Gap
   Surfacing elicits meaning* — and a missing requirement is meaning. If the double-count is fixed
   first, revisit.
2. ~~**A graph with capabilities and zero requirements reports nothing**~~ — **built,
   2026-07-19**: `design_without_intent`, the fifth phase-coverage nudge, at 0.72 — the top
   nudge on an adopted graph, exactly ophyd finding 1's ask (*"the first gap should be about
   missing intent, not missing structure"*). One project-level nudge, never one per
   capability; it yields the moment a requirement exists; the wording directs intent to
   sources **outside** the implementation, per this thread's core discipline. Verified over
   the live binary: on a capabilities-plus-component graph with zero requirements the gap
   list leads with the anchored gap, then this at the top of the nudge band.

*Duplicate detection: HEAL's rule computed nothing.* **Fixed**, and the root cause is a fresh
variant of the recurring lesson — not *unreachable on the surface* but **reachable and hollow**.
`heal.rs` iterated existing `DUPLICATES` **edges**, so it reported a conclusion somebody had already
reached and recorded, and could never fire on a duplicate nobody had found — which is every
duplicate an adoption pass exists to discover. That is
[gap-surfacing.md](gap-surfacing.md) discipline 1 verbatim, the trap it names as storyflow's
biggest: *detectors read computed signals, not raw edge-name filters* — "the detector was DEAD on
live data while looking correct."

The computed half is `possible_duplicate`, and it landed in **DETECT, not HEAL**. Three reasons,
and the first is the serious one: `HealCategory::Duplicate` maps to an *applicable* `HealOp::Merge`
that `apply_heal` executes — deleting a node and re-pointing its edges, with no snapshot and no
undo. Merge is content-free and safe only *because a human asserted the endpoints*; feeding a
heuristic into that path would let the machine delete a component it merely suspects. Second, a
HEAL issue cannot be dismissed — gaps can be acknowledged, defects cannot — and `unexpected_coupling`
([BL-6b](#closed)) is the cautionary tale of a detector firing on correct architecture with no way
to make it stop. Third, "are these the same thing?" is meaning, and the docs' own division is that
HEAL fills structure while gap-surfacing elicits meaning.

So they compose instead of overlapping: DETECT asks, the user confirms by drawing the `DUPLICATES`
edge, and HEAL's existing merge — whose "endpoints known" precondition now genuinely holds —
repairs it. A pair already carrying the edge is skipped, so nothing is double-counted.

The rule is structural (≥2 shared capabilities, Jaccard ≥ 0.8 over allocation sets), which needs
nothing deferred. [heal-process.md](heal-process.md) plans duplicate detection on
`resolution: fuzzy_then_vector`; that needs the deferred `EmbeddingBackend` and finds a different
population — things *described* alike, where this finds things *wired* alike. Complements, not
rivals. Scoped to Components deliberately: two Capabilities satisfying one Requirement is
decomposition, the normal case, and a rule there would fire on almost every correct design.

*A skill alone would ship a graph that lies.* Five fixes gate it, and each is the recurring lesson
below again:

| Blocker | Evidence | Size |
|---|---|---|
| ~~`add_capability` hardcodes `status: "planned"`~~ — **done** | ophyd's 15 shipped, under-test capabilities made the graph "assert that a production system is entirely unbuilt". Optional `status` at creation plus `set_capability_status`; nothing hardcoded it, the constructor never set the property and took the schema default | S |
| ~~`detect_gaps` walks Requirement→Capability only, so an **orphan Capability is never reported**~~ — **done (DETECT)** | "in greenfield that direction is rare… in brownfield it is the dominant direction of error" — a feature in production no requirement justifies is exactly what an adoption exercise is for. Now `unmotivated_capability`; see the note below on why HEAL was deliberately left alone | M |
| ~~No duplicate detection~~ — **done** | did not fire on a textbook duplicate; "duplicate implementations are *the* characteristic brownfield defect". Now `possible_duplicate`, computed from shared allocation sets and **asked** rather than repaired — see below | M |
| ~~`concept_without_design` severity ordering~~ — **done** | above. Fixed by banding the sort rather than touching the detectors: a gap that names nodes outranks a project-level phase nudge, severity within each band | S |
| ~~Provenance has nowhere to go~~ — **done** | ophyd smuggled `[EXTERNAL — …]` into statement text, "which is not queryable" | S |

That last one had a cheap answer worth taking regardless, and it is taken. The schema's mechanism
was `Fragment.provenance` (its enum already includes `inferred`) plus a `YIELDED` edge — the
intended pattern, but 2 writes per node with no bulk tool. A `provenance` **property** on
`Requirement` / `Capability` / `Component` / `Interface`, reusing that same enum, is
backward-compatible: adding a node or edge *type* bumps `GraphStamp` and makes older binaries
refuse the graph, but adding a property does not ([BL-19](#bigger-threads)). Confirmed — the counts
stay at 27/53. `set_provenance` writes it incrementally and `import_graph` carries it at create
time, which is the bulk path this thread already points an adopt pass at.

Related, for whoever picks this up: `import_graph` is the only bulk write path and is an atomic
upsert, so an adopt pass should build the export document and import it once — 3dtictactoe spent
~60 MCP calls on 33 nodes.

*The conversion step itself, probed for real* — *2026-07-19, installing into a scratch repo shaped
like every brownfield target (own `AGENTS.md`, own `.mcp.json`, source tree).* The earlier note
here — "cannot install into a repo that already has its own AGENTS.md; needs `--skills-only`" —
is **stale and corrected**: the sidecar path works. The install lands clean: the project's
`AGENTS.md` untouched, kit instructions to `REFLOW2.md`, skills to all four harness locations,
the existing `.mcp.json` merged not overwritten. Three real defects were found — **all three
fixed 2026-07-19**, verified by re-running the probe (fresh install, second-run idempotency,
greenfield unregressed, `--check` consistent):

1. ~~**Nothing points at `REFLOW2.md`**~~ — BL-22's sibling lesson verbatim: shipping the file
   is not shipping the capability. Fixed by the same rule as the merged MCP configs: one
   marked pointer line appended to the project's own instruction file, idempotent by content,
   reported — never overwritten. **Widened 2026-07-20** after the storyflow trial found the
   first fix protected the wrong filename: the pointer now goes into *every* convention the
   project has (`AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, copilot-instructions, cursor/windsurf
   rules), because storyflow carries `CLAUDE.md` and no `AGENTS.md`, so the installer saw
   nothing to protect and left the file Claude Code reads first with no mention of reflow2.
2. ~~**`.reflow2/` is not gitignored**~~ — the installer had no `.gitignore` handling at all,
   so a converted repo started tracking a RocksDB directory. Now appended or created,
   idempotent, with the reason in the comment: the graph is machine-local state; the durable
   record is an export.
3. ~~**The closing "Next:" text is greenfield-only**~~ — now branches on **the project**
   (a bounded source-file count), not on whether reflow2 wrote a sidecar. A repo with code
   gets the `adopt` path with its evidence stated; an empty directory gets genesis; and an
   *update* whose graph is still empty gets the adopt hint too — the case that would
   otherwise repeat the failure for anyone who installed before the skill shipped.
   **Rewritten 2026-07-20**: the first version keyed off our own install artifact, so
   storyflow — 2,643 files — was told to describe what it wanted to build.

Converting a project is now: build/point at the binary (BL-15's published-binaries gap is the
remaining wall for machines without a checkout), run `reflow2_init.py`, open the agent. Then
the graph is empty and everything after that *is* this thread's skill. The first gap an
adopted graph raises is `design_without_intent` — built the same day, see below.

*The accepted reverse-engineering lifecycle, mapped* — *user, 2026-07-19, from research into
standard practice.* Across hardware and software the accepted process is two stages —
**redocumentation** (break the existing product down) and **design recovery** (deduce the
original concepts) — through five steps: information gathering → disassembly/scanning →
analysis (static *and* dynamic) → modeling & reconstruction → validation. The user's framing
of the hard case: large codebases with no requirements and no record of why choices were made —
*"you only get what you see."* The mapping onto what this thread already holds, and what it
exposes:

| RE lifecycle step | reflow2 mechanism | Status |
|---|---|---|
| **Information gathering** | The division-of-labour table above: requirements from anything *but* the implementation; found documents trusted per the ophyd caution (its PDR omitted the system's central invariant); sources recorded as Fragments with provenance — the provenance viewpoint renders exactly this ledger | mechanisms landed (BL-27 blockers, BL-40) |
| **Disassembly / scanning** | For source-available software the disassembler is the repo read: structure from imports and calls, never prose (the phantom-component caution); breadth at deliberately coarse granularity (ophyd: 110k LOC → ~78 nodes, vendored fork opaque); one `import_graph` for the bulk write | discipline recorded above; the skill must encode it |
| **Analysis — static** | allocation/coupling/`possible_duplicate`/`hierarchy_issues` over the scanned structure | landed |
| **Analysis — dynamic** | **the gap this framework exposed — now closed.** All three brownfield trials were static-only. The receptors exist end to end: run the tests → `reconcile_verification` (BL-30, done 2026-07-19 — the typed way in, divergences named and persistent); run the thing → `reconcile_deployment` (BL-9). The adopt skill's "run it and record what you saw" phase has its full machinery | landed |
| **Modeling & reconstruction** | The graph *is* the model. Design recovery deliberately terminates at the human: a requirement inferred from its implementation is satisfied by construction, so recovered intent is marked `inferred` and `unmotivated_capability` routes "why does this exist?" to the person — recovered rationale lands as Decisions, provenance-marked. *You only get what you see* becomes a property of the graph: it confesses what it cannot know instead of improvising past it | landed (the projection doctrine, BL-40) |
| **Validation** | **the second exposure — the current plan ends at "deepen on demand" with no closing step.** The validator is the reconcile family plus the detectors: checksums match (`reconcile_artifacts`), tests agree (P4), deployments agree (`reconcile_deployment`), and every gap the model fires is either true of the system or an error in the model — which is precisely how the trials scored themselves. The skill should end by running it and reporting the verdict | mechanisms landed; the skill must close the loop |

The two-stage split lands on an existing line: **redocumentation** is the as-built layer
(Capability/Component/Interface from code — satisfied-by-definition is fine there), and
**design recovery** is the intent layer, where the *"never infer a requirement from the
implementation"* discipline is exactly the stage boundary. reflow2's position, sharpened by
this mapping: redocumentation is automatable; design recovery is question-generation — the
machine drafts, marks provenance, and asks; it never fills in the why.

*The `adopt` skill itself — built, 2026-07-19.* Nine skills now ship in the kit. It is the
five-phase RE lifecycle above made operational: gather (sources as Fragments with trust
weighed), scan (breadth-first coarse, structure from imports never prose, one `import_graph`),
analyze (static detectors + the dynamic receptors — run the tests into
`reconcile_verification`, observe deployments into `reconcile_deployment`), recover (intent
only from outside the code, `design_without_intent` and `unmotivated_capability` as the
question engine, rationale as provenance-marked Decisions, found limits as budget Constraints,
found processes as Flows), validate (the reconcile family agreeing + every remaining
gap true-of-the-system or a model error). Its close states the doctrine: *adopt is done when
the graph and the system agree and every remaining gap is acknowledged or genuinely open —
a system adopted honestly usually should have open gaps.* The installer's brownfield
next-steps and the consumer AGENTS.md step 0 both point at it. **Not yet exercised on a real
system** — the next brownfield trial should run through this skill, which is also what the
still-open deepening/frontier work (below) is waiting behind.

Size **L** for the thread; the `adopt` skill itself is ~~**M** once the two **S** blockers land~~ **done**, and
the deepening stage is a separate **M** behind the frontier work.

**BL-15 · Project bootstrap and kit updates** — *from the external user, 2026-07-18.* "You should
be able to launch a project from reflow, which bootstraps everything into a new repo... And maybe
it adds a script for pulling in releases. You won't know up front what project type it is though."

Two problems, and his caveat is the design.

*Bootstrap.* Today the kit installs by three hand-run `cp`s, one of which needs the binary path
edited in, plus a hidden `.grok/` that `cp *` misses. That should be one command.

*The caveat is the product, not an obstacle.* You don't know the project type up front **because
that is a design decision the loop is supposed to make.** So bootstrap only what is type-neutral
— AGENTS.md, skills, MCP config, `.reflow2/`, `.gitignore`, a brief template — and deliberately
scaffold no `src/` layout, build file, or language choice. Those come *out* of the design, and
both blind trials produced exactly that: a structure that fitted what had been designed. A
scaffold that guessed would commit a design decision before the design existed.

*Updates are the sharper half, and currently absent.* The kit is copied, so a consumer's copy
freezes at install time — the first external user's copy is already stale by a day of skill
fixes and nothing tells him. Text (AGENTS.md + skills) is easy to refresh; the binary needs a
~10-minute RocksDB build, so it wants either published release binaries or a pinned-version
check. Bears directly on the embedded-vs-service fork: a service would make this disappear.

**Bootstrap and in-place updates: done** (`tools/reflow2_init.py`) — one command installs or
refreshes the design environment, resolves the binary path itself, records a kit version so
staleness is detectable, and leaves the graph, user files and a customised `.mcp.json` alone.
It installs no `src/`, build file or language choice, on purpose.

**Update-skew detection: done.** `reflow2_init.py` now reports whether the *binary* is older
than the source it was built from — the quiet failure where you pull, re-run init, and forget to
rebuild, leaving current instructions driving an old server. SETUP.md documents the three-step
update in the order that matters.

~~**Still open — published releases.**~~ **Built 2026-07-20** (the first release run is the
verification): `.github/workflows/release.yml` builds `reflow2-mcp` for linux-x86_64 /
macos-arm64 / macos-x86_64 on every version tag (or a dispatch naming an existing tag — the
tag must match Cargo.toml's version or the kit job refuses), packages the consumer kit as a
tarball in the same `tools/` + `getting-started/` sibling layout the init script resolves,
stamps it with `KIT_VERSION.json` (a tarball's stand-in for git metadata), and attaches
everything plus sha256 checksums to the GitHub release under **version-less asset names**, so
`tools/install.sh` needs no API parsing. The installer prefers `gh release download` (repo is
private; unauthenticated curl is the path that starts working the day it isn't), verifies
checksums or says plainly that it could not, installs to `~/.local/bin` + `~/.local/share/
reflow2/kit`, and re-running it replaces binary and kit *together* — the BL-32/BL-18 skew pair
cannot open between them. `reflow2_init.py` grew the three checkout-independences: `--binary` /
PATH fallback, `KIT_VERSION.json`, and installed-mode update advice (re-run the installer, not
git pull + cargo build). Verified end to end from a simulated tarball: install, idempotent
re-run, `--check`, brownfield/greenfield branching all correct. macOS note: unsigned binaries
are fine through the installer (curl sets no quarantine xattr); browser downloads would hit
Gatekeeper — signing stays open if that path ever matters. **Follow-up (S–M, deliberate):**
embed the kit in the binary (`include_str!` is already the schema's pattern) so `reflow2-mcp
init` replaces the Python script entirely and one artifact carries everything.

**The fork is decided** (2026-07-18, [surface-plan.md](surface-plan.md)): **repo-file, embedded**.
So this is no longer gated — build published per-platform binaries, which is the packaging answer
to a packaging problem. The service was weighed and set aside: its strongest argument
(concurrency) is hypothetical while there is one writer, it would put the user's design graph on a
machine they do not control, and it is permanent operational cost. The conditions that would
reopen it are written down; "published binaries proved insufficient" is one of them, so this item
is also the experiment that would justify revisiting.

*Distribution mechanics, distilled from a 2026-07-20 discussion (user + brother + outside
research), so the build of this item starts where the thinking stopped.* The standalone-repo
proposal blends three questions with different answers:

1. **Where the code lives — already answered.** reflow2 *is* a standalone repo; a consumer
   project carries none of its code, `.reflow2/` is gitignored, the committed export is small
   JSON. The real residue is (2) and the kit files — and the kit files are the product's UX,
   which must live where harnesses look (BL-22's lesson).
2. **Where the binary comes from — this item.** The stack's advantage: Rust+RocksDB compiles to
   a zero-dependency native binary — no Node, no Python, no toolchain. Plan: CI builds
   per-platform binaries on each tag (v0.4.0 exists to publish); a `curl | sh` installer
   (rustup/uv pattern) that detects platform and drops `reflow2-mcp` on PATH; `cargo install
   --git` as the zero-infrastructure path for Rust developers; macOS signing still an open
   question. **Embed the kit in the binary** (`include_str!` is already the schema's pattern) so
   `reflow2-mcp init` replaces the checkout-bound `reflow2_init.py` and one artifact carries
   everything — then a consumer `.mcp.json` says `"command": "reflow2-mcp"`, no checkout
   anywhere, and kit updates ride binary updates (which also simplifies BL-18's staleness story
   to one version instead of three).
3. **Where the graph lives — the only genuinely open question, deliberately NOT this item.**
   The proposal's global `~/.reflow/` + per-project thin reference. Note before the queued
   Decision conversation: the live RocksDB dir is already machine-local working state; what
   `req:persistence` actually protects is *the durable record travels with the project*, which
   a global graph dir preserves iff exports stay committed in-project. What it costs:
   discoverability and the backup-beside-the-graph story. What it does **not** buy:
   concurrency — stdio servers spawn per client and the single-writer lock is per graph either
   way; a "global server" is only a global binary. `--graph-path` already makes the thin
   pattern available today; the question is only the default. Test against
   `dec:repo-file-embedded`'s reopening conditions, on the record.

> **Unblocked 2026-07-18.** BL-18, BL-19 and BL-20 were all waiting on the embedded-vs-service
> fork. It is decided — **repo-file, embedded** ([surface-plan.md](surface-plan.md)) — so build
> them for that shape. Export/import is now the migration story rather than a stopgap until a
> service centralises it.

**BL-26 · Which files does the design depend on, and is `DOCUMENTS` traversable?** — *user,
2026-07-18.* Prompted by the question "should every document in a repo be captured in the graph —
what is the purpose of each file?"

*Not every file.* [BL-23](#closed) is the caution: modelling 22 source files as Artifacts made
them 88% of the gap list. Capturing everything is how a list becomes something people skim. The
criterion is not "is it a file" but **"would something be wrong if this drifted out of step with
the design?"** That splits a repo four ways:

| Group | Example | Today |
|---|---|---|
| Produces the design | `crates/**/src/*.rs` | ✅ `Artifact` + `REALIZES` + checksum + `reconcile_artifacts` |
| Describes the design | `docs/*.md`, README | ✅ **write side done 2026-07-20**: `documents` (core fn + MCP tool, `doc_kind` carried, both endpoints checked — the storage engine accepts dangling edges, so the fail-loud check is the only one there is) |
| Instructs agents | `AGENTS.md`, `COORD.md`, `.github/copilot-instructions.md` | ✅ same mechanism: `artifact_type=document`, `doc_kind=agent_instructions`; the link-artifacts skill states the criterion and the boundary against `SPECIFIES` |
| No design meaning | `Cargo.lock`, `target/`, generated output | should stay out — this is where the noise would come from |

*The founding evidence is a failure reflow2 should have caught.* In one session on 2026-07-18:
AGENTS.md's build command was found wrong and fixed; hours later, by accident,
`.github/copilot-instructions.md` was found carrying **the same stale command**; and
`docs/backlog.md` grew a duplicated section that nothing noticed until someone went looking. Two
instruction files disagreeing about how to build the project is a coherence failure, and catching
coherence failures is the entire point — it was missed because neither file is in any graph.

*This is more than modelling more files.* Two things stand in the way:

1. ~~**`DOCUMENTS` has no write side.**~~ **Done 2026-07-20** — `documents` core fn + MCP tool
   (78th on the surface), endpoints fail-loud, `doc_kind` carried; pinned in core, over the
   tool surface, and the ghost-endpoint refusal in both. Was the recurring lesson's ninth
   instance, now closed.
2. **PROPAGATE does not traverse it — still open, and it is the M half.** `propagate.rs` lists `SPECIFIES`/`DOCUMENTS` as
   *"intentionally not traversed in this increment"*, so even fully wired a change would not
   ripple to the documents describing it. Making docs coherence-checked means **deciding
   `DOCUMENTS` is traversable**, and deciding what that implies for blast radius — a change to a
   Component reaching every doc that mentions it could be useful or could be the next flood.
   Weigh it against BL-23 before switching it on.

*The self-referential case is the best test available.* reflow2's own records — CHANGELOG,
backlog, requirements-coverage, COORD — are a hand-maintained golden thread, and four separate
lapses in one session went uncaught. The self-host probe already models
`requirements-coverage.md`'s **contents** as 72 Requirements but not the **file** as an Artifact
documenting them: the graph knows the requirements and not the document that is supposed to track
them. Extending the probe to the instruction and record files would test this before any of it
ships.

Size: ~~**S**~~ the write side is **done** (2026-07-20) — recording which files matter is now
possible, and the self-referential test (this repo's own records as DOCUMENTS artifacts) is
unblocked. **M remains**: the traversal decision — whether a change ripples to every doc that
mentions it — weighed against BL-23's flood lesson, and it wants the user.

**BL-19 · The graph must survive an upgrade** — *user, 2026-07-18.* **Blocks BL-18**: an
"you're out of date" nudge shipped before this exists drives users into an upgrade path with no
migration story.

*What is actually true today* (verified against dynograph-foundation v0.10.0). The schema lives
in the **binary** — reflow2 embeds the ten YAMLs via `include_str!` and re-merges them on every
open — while the RocksDB directory holds only nodes and edges. `new_rocksdb(schema, path)` takes
the schema from the caller and **stamps nothing on disk**: not a schema version, not a foundation
version. Validation runs on write, never on read.

So the reassuring half first: **upgrading reflow2 does not delete anyone's graph.** The feared
catastrophe is not the failure mode.

*The quieter hazard is real, though.* The foundation's own test (`engine/tests.rs:1325`) pins the
behaviour: add a required property with a default, and the default is applied **on create, not
backfilled**. Existing nodes keep the old shape. A schema change therefore leaves mixed-vintage
nodes with no error and no marker — detectors read `None` on old ones and a value on new ones.
That is a silent drop, which AGENTS.md rule 4 forbids everywhere else in this codebase.

*And the destructive case has no guard at all.* If dynograph-foundation changes its key encoding
(`keys.rs`) or value serialization, an existing store may be misread — and because nothing stamps
a version on the graph directory, there is no way to **detect** that a store predates the format,
let alone refuse to open it.

**The stamp and the check are done.** A `GraphStamp` — reflow2 version, schema version, node and
edge type counts — is written to `<graph>.meta.json`, a *sibling* of the store rather than a file
inside a directory RocksDB owns. `open_rocksdb` reads it, compares, refreshes it, and the MCP
server reports any difference on stderr and in the log.

*What it refuses, and deliberately does not.* Refusing on any mismatch would be worse than the
problem: schema growth here is additive, so a graph written before a type existed reads perfectly,
and refusing would lock someone out of their own design over a change that cannot hurt them. The
line is drawn at **a graph from the future** — one written by a reflow2 whose schema knew *more*
than the running one. That graph can hold nodes this binary has no vocabulary for, so reading it
means silently seeing less than is there. Refused loudly, with what wrote it and what to do.
An unreadable stamp is reported and never overwritten; it may be the only record of what wrote the
graph.

The declared schema `version` was not usable as the signal — it is 1 in every domain and has never
been bumped. Type counts are what actually move, and they caught the 26→27 change from BL-4.

**Backup-before-upgrade is done.** `reflow2_init.py` exports the design to
`.reflow2/backups/design-<utc>.json` before it changes anything — beside the graph, never `/tmp`,
which systemd-tmpfiles clears. A failed export is reported and does not abort the update: the
update may be exactly what fixes the binary that could not read the graph. `reflow2-mcp --export`
prints the document to stdout, so a script can take a backup without speaking MCP.

**Backfill is done, and it needed no new code.** Importing applies the *current* schema's
defaults, so a document written before a property existed comes back carrying it. That is why
export/import is the migration path rather than bespoke per-change code: export with the old
build, import with the new, and mixed-vintage nodes resolve themselves. Pinned by
`importing_an_old_document_backfills_new_defaults`.

**BL-18 · Am I running the current reflow2? — DONE** — *user, 2026-07-18.* Extends the update half of
BL-15, whose local machinery is already built and whose remaining gap this names precisely.

`reflow2_init.py` stamps `.reflow2/kit-version.json` with `reflow2_version`, the short `commit`
and `committed_at`, and `binary_is_stale()` compares source mtime against binary mtime. Every one
of those checks is **local**: a consumer copy can tell that its binary predates its source, but
never that its source predates upstream. That is the one an installed copy actually needs —
the first external user's kit went stale in a day of skill fixes and nothing told him.

The check is cheap because the stamp already exists: `git ls-remote` for the remote HEAD, compare
against the stamped commit. No clone, no auth, one round-trip.

**Done.** `reflow2_init.py` reports it on `--check` and after every install: `git ls-remote`
against the remote HEAD, compared to the stamped commit. No clone, no fetch.

*It fires where someone deliberately asks*, not on every server start. A network call per MCP
session would be intrusive and would hang offline, and this script *is* the act of asking. Any
failure — offline, no access, no git, slow network — reports "could not check" rather than
silence, because **"I could not check" must never look like "you are up to date"**. It never
blocks an install.

When behind, it prints the three-step update in the order that matters, because doing them out of
order leaves current instructions driving an old server.

*What it must not promise.* Unlike `claude update`, there is nothing to pull: the binary needs a
~10-minute RocksDB build, so the check can only report staleness, not resolve it. A real
`reflow2 update` needs published per-platform binaries — BL-15's still-open half, and a decision
that belongs with the embedded-vs-service fork. Keep the two apart: this item is **S** and
useful now; that one is **M–L** and gated on the fork.

**BL-16 · Domain-appropriate artifacts — the non-coding design problem** — *user, 2026-07-18.*

Coding is the *natural* domain here because agents are trained on it, so "design and build
anything" is quietly load-tested only on its easiest case. Ask for a rocket and the question
"what are the artifacts?" has no obvious answer. Ask for a 3D-printed object and one artifact
should probably be an `.stl` — but nothing in reflow2 knows that, and the agent may not either.

The gap is not the `Artifact` type, which is domain-neutral already. It is that **nothing helps
the agent decide what set of artifacts a given design concept actually calls for.** For software
it free-associates correctly from training; for a rocket it may need retrieval to find that the
answer involves things like a mass budget, a trajectory sim, drawings, a test plan — and for
hardware some artifacts are *physical*, which the as-built/drift machinery (`reconcile_artifacts`
checksums files) has no notion of.

Bears on P3 realization, on `unverified_capability` (what counts as verifying a weld?), and on
BL-9's as-fielded view. Likely wants a per-domain artifact-kind prompt or a retrieval step at
GENESIS, not a hardcoded taxonomy — the whole point is that the project type is a design output.
Size **M–L**, and it is the sharpest test of the "design anything" claim we have.

**BL-17 · Engineering principles as a separate, design-general file** — *user, 2026-07-18.*
Ported from `~/dev_storyflow/PROTOCOL.md`, whose "⭐ Engineering Principles" section is the
generalizable part (the fleet/bus/worker-pool/LEDGER/docker machinery around it is storyflow
infrastructure with no analogue here — reflow2 has COORD.md and two people).

Two of the seven are already reflow2 invariants: *no silent fallbacks* is AGENTS.md rule 4, and
*no silent caps/truncation* is implemented as `truncated_beyond_depth` / `skipped_operations`.
The four not yet written down here are **root-cause-before-fix** (name the mechanism, never
pattern-match a fix onto a symptom), **done = end-to-end** (merged ≠ done), **verify your own
claims by execution before reporting**, and **modular, no monoliths**.

Keep them in their own file rather than inlining into AGENTS.md, exactly as suggested: they need
tailoring away from coding. `PROTOCOL.md` phrases them for a web stack — "lens-exposed",
Playwright on the real surface, `npm run check`, unit-vs-live tests. For a rocket or a document,
"end-to-end" and "the real path" mean something else, and verification is a `Verification` node
rather than a test run. A separate file can generalize; a section buried in AGENTS.md will drift
back toward code. AGENTS.md then points at it. Size **S**.

| ID | Item | Why | Size |
|---|---|---|---|
| **BL-7** | **`ingest` over MCP** (SP-3b) — **DONE 2026-07-27** | The multi-pass extraction pipeline was unreachable agent-native, so provenance, fuzzy dedup and time-aware resolution never ran. **Built as `ingest_step`**, the repeated-round collect-then-serve handshake `agent.rs` has specified since SP-2 and nothing had driven: call with no answers, get prompts, answer, call again with everything so far, until `done`. Usually 3–4 rounds, because phase-2 passes are gated on the discovery classifier and threaded with phase-1 rosters and so genuinely cannot be asked up front. **The 'transactional prepare pass' this row asked for turned out to need no transaction:** every prompt is issued before INGEST's integrate phase begins, so the prepare rounds replay the whole pipeline against a throwaway in-memory graph and an abandoned handshake writes literally nothing — pinned by its own test. It also holds **no server-side session state**: each call is self-contained, so it survives a restart, works across seats sharing one server, and cannot leak an abandoned run. New `PartialBackend` (serves what is answered, collects what is not) is the one primitive that was missing between `PromptCollector` and `AgentBackend`. 4 Rust cases plus an end-to-end run over real MCP. | ~~L~~ |
| **BL-8** | **Session state / multi-project** | Select a graph per project; give agents memory across sessions. Core already supports `graph_id`; nothing exposes it. See the memory note and [reflow-audit.md](reflow-audit.md). *Partial precedent, 2026-07-19:* the design graph now carries the session **distillate** — 8 Decision nodes with rationale, each linking the session transcript URL (which every commit also carries as a `Claude-Session:` trailer). The doctrine: the graph holds decisions, not tape; a transcript is an artifact outside the graph, one link away. The remaining BL-8 work is the live memory (questions, working state) across sessions, and the `Fragment`/`YIELDED`/`TemporalFact` layer is the schema-complete, zero-write-side machinery for it | L |
| **BL-9** | **As-fielded view — DONE 2026-07-19** | `reconcile_deployment` (`fielded.rs`), the P5 sibling of `reconcile_artifacts`: per-environment observations vs `DEPLOYED_TO`, three divergence kinds, unknown ids reported, partial observation never read as absence. The library-plugin false positive is impossible by construction — only Releases run, only Environments host (the audit's caution, honored by shape rather than a flag). Recorded divergences are persistent `unresolved_drift` gaps that an agreeing observation auto-resolves; the design-side answer is `deploy_to` with the true status. Measured: **P5 2/2, phase trial 12/13** — the probe now injects a divergence instead of checking the tool exists. The as-fielded viewpoint renders. The last of the three feedback loops is BL-30's `reconcile_verification` | ~~M~~ |
| **BL-10** | **Root-cause classification of drift** | `drift.rs` detects divergence with no notion of *why*, so no notion of which side is wrong. Reflow's seven-category taxonomy ends in a decision rule. Needs a scalar coherence score to gate on. | M |
| **BL-11** | **Path-cumulative budget analysis — DONE 2026-07-19** | `budget.rs`: a budget is a `Constraint` (`quantity`/`limit`/`direction` — new backward-compatible properties) spent through `CONSTRAINS` edges (`contribution`/`basis`). `budget_report` gives the stated total, basis coverage, the worst dependency path (contracts collapsed), and an honest verdict — `incomplete` whenever a contribution is unstated (listed, never zeroed: the graph-analysis discipline), `ungated` without a limit, and a cycle refuses the path claim by name. `Constraint` had no write side at all — the fourteenth recurring-lesson instance; `add_constraint`/`constrains` close it. The measures viewpoint (≈ SV-7) renders, closing the catalogue's last row. Not built: a budget-exceeded DETECT gap — `budget_report` reports it, and whether an exceeded budget should *nag* like a contradiction is a decision for a real use first | ~~M~~ |
| **BL-12** | **Concurrent multi-agent / team access** — ⚠️ **THE SINGLE-MACHINE RUNG IS DONE, 2026-07-27** | **`--shared` closes concurrent read AND write for every session on one machine, as the default** (see the `--shared` entry below, and the Unreleased CHANGELOG). This row's original framing — *"agents take turns; that is only a real cost once a second writer actually exists, which it does not"* — **was overtaken by events twice over and should not be read as current**: second writers did exist (a StoryFlow fleet of 3 leads + a worker pool), and the mechanism to serve them had already shipped in v0.14.0 as `--http`. What remains under BL-12 is genuinely bigger and unchanged: **several machines**, authentication, claims/trust at org scale, and reconciling designs that diverged. The read-only-secondary rung is **no longer the cheapest next increment** — a session that attaches to the shared server gets full read/write, so secondaries now only serve the case where somebody deliberately runs a private store. **First design sketch below** — four consensus-mechanism ideas that survived a 2026-07-19 thought exercise, still the right material for the multi-machine question. | ~~L~~ → L (multi-machine only) |
| **BL-12b** | **A wedged shared server stalls an attached session for **335.7s measured** (`CALL_TIMEOUT` 300s + the re-election attempt on top), which is indistinguishable from a hang** | Raised by `w-74c2989e` (2026-07-27) while proving the timeout fix, and **explicitly flagged by them as unmeasured reasoning, not a result** — they did not wait the five minutes out. Session *startup* is not affected and that part IS measured: `initialize`/`tools/list`/the probe use the 5s bound, so a fresh session against a `SIGSTOP`ped server degrades at **35.8s** (control: 0.0s against a healthy one). **⚠️ The figure was first reported as 300s by quoting the CONSTANT rather than the PATH; `w-74c2989e` measured the real number at 335.7s — the bound, then a re-election attempt. A constant is not a measurement.** The bound applies only to an in-flight `tools/call`, and is deliberately generous because a bound that fires on legitimate detector work would turn a slow answer into a false failure — with **no measurement of a large design** to pick a tighter number from. The tension is real and worth recording rather than settling by guess: *the fix's whole argument is that a hang is worse than an outage, and a five-minute stall reads as a hang to the agent waiting.* Candidate answers, theirs: a progress/heartbeat line while a call is in flight, or a per-method bound. Wants a measurement of detector latency on a big design first. | S |
| **BL-13** | **Advanced testing tiers** | Comprehension (partly answered by the blind trial), scale (all fixtures are 3–10 nodes), messy input, longitudinal. | M |
| **BL-14** | **`tools/` sweep follow-ups** | The remaining adopt-list items in [reflow-audit.md](reflow-audit.md): typed gap resolution strategies, abstraction-gap → strategy, document round-trip, MCP resources/prompts. | M |

**BL-8 · addendum — WHEN context enters the graph, not just WHAT** — *user, 2026-07-22: "if I
exit the session before the LLM writes its summary, do I lose the opportunity?"* The sharp version
of the session-continuity question, and it exposes that the vulnerability is the **timing**, not
the content. The wrong design — the LLM distills a summary at session end — is the exact BL-74
failure mode ("a discipline that depends on being remembered loses to operational urgency"): exit
is the moment context is most exhausted and most likely to be abandoned, so a summary written on
the way out is the one the busy session never writes. The right design decouples two things BL-8's
existing doctrine already separates and this pins to a **three-part safety net**, each part
already doctrine or already built:

1. **During — continuous capture.** Design decisions are written *when made* via the normal loop
   tools (`add_decision`, `answer_question`, `add_change_event`), never deferred to a summary. By
   exit there is nothing pending to lose because there was never a pending summary. The
   just-shipped BL-79 skill guidance ("attribute nodes when captured, not at exit") is this rule
   applied to authorship; it generalizes.
2. **On exit — a trigger, not a virtue.** A **Stop hook** distills anything not yet captured —
   the exact mechanism BL-74's `loop_nudge.py` already installs (SessionStart / PostToolUse /
   Stop). Triggers beat exhortation.
3. **If the Stop hook is missed (hard kill / crash) — retroactive recovery.** The coding agent
   persists the full transcript under a session **UUID** independent of the LLM's cooperation
   (Claude Code: `~/.claude/projects/.../<uuid>.jsonl`; the commit trailer `Claude-Session:` and
   the transcript-URL-on-Decision precedent already link it). So the next session's SessionStart
   hook can detect an **un-distilled prior transcript and offer to ingest it** (the INGEST /
   SP-3b pipeline, BL-7). Worst case is *deferral*, never *loss* — the no-silent-drops invariant
   applied to context.

This third rung — next-session recovery of an orphaned transcript — is the piece **not yet
written down anywhere**, and it is what makes the honest answer to the user's question "no, exit
timing cannot lose it." Consistent with `views-are-projections`: the goal is to extract the
*decisions* from the transcript, never to embalm the conversation. Prerequisites: the Stop-hook
recipe (BL-74, shipped) and the INGEST-over-MCP handshake (BL-7, still L). Size **S** for the
recovery-offer hook once BL-7 lands; the doctrine is the value and it is now recorded.

**BL-12 · design notes — what crypto consensus mechanisms lend the multi-writer future** —
*thought exercise, 2026-07-19, from the author's question: could XRP / Hedera hashgraph /
Proof-of-Stake trust machinery extrapolate to reflow2?* These mechanisms all answer one question —
*how do parties who cannot fully trust each other maintain a shared ledger without a referee?* —
which is BL-12's question with different nouns: a human, an LLM, and a second human+LLM pair on one
design. Four ideas survived contact with the analogy; the rest was vocabulary tourism.

1. **Claims reference what the claimant had seen** (hashgraph's gossip-about-gossip). Hashgraph's
   core move: never vote on truth — record who-knew-what-when as a DAG (each event hashes what its
   author had already seen) and *compute* consensus deterministically. The confirmation ledger
   already computes trust states from a claim DAG the same way. The extrapolation: every accept or
   design edit carries the **export hash of the graph state it was made against**. Then the
   question that kills shared designs becomes computable: was a conflicting claim made *in
   ignorance of* mine (both honest on stale views — merge mechanically) or *in defiance of* it (a
   real disagreement — route to the humans as a question)? COORD.md answers this socially today
   ("pull before you claim"); the graph could answer it structurally. The best single idea here.
2. **Trust topology per claim type** (XRP's Unique Node List). The supermajority half is
   meaningless at n=2–4, but explicit per-claim-type trust maps cleanly, and exists in embryo:
   `unmotivated_capability` already weights `inferred` (0.70) differently from `authored` (0.55).
   Extrapolated: *who may assert what* — "verification status is only credible from a CI run,
   never from the agent"; "a requirement moves to `accepted` only on the human's say-so". This is
   also [BL-41](#next-up)'s mechanical half: text from a party not authorized for a claim type
   simply does not count as that claim.
3. **A finality boundary at release cuts** (ledger close). BL-34's frozen manifest is finality
   without the word — a past release's contents cannot be rewritten. The extrapolation with teeth:
   nothing before the last `release_cut` epoch may be *mutated*, only superseded. `temporal.rs`
   snapshots the past but does not yet enforce its immutability.
4. **Computed track records, shown not enforced** (Proof-of-Stake slashing, inverted). Nobody
   bonds capital, but every party accrues a record the graph already holds — *five `design_holds`
   claims, zero design edits* is a legible signature. Slashing softens to reputation: a party
   whose claims keep being overturned has that history computed and displayed, never auto-punished
   — judging a claim false is semantic, and automated slashing would seat the graph in the
   judgment chair that `dec:report-dont-judge` reserves for the human.

*Where the analogy breaks, so nobody rebuilds it wrong:* BFT is built for many anonymous
adversaries; BL-12 has two-to-four named collaborators whose threat model is **error and drift,
not malice**. Staking economics need a scarce resource nobody here has. And consensus mechanisms
exist to *automate agreement*, while reflow2's philosophy routes disagreement to humans as
questions — auto-resolving a design conflict by quorum would be sycophancy-by-majority, the wrong
party in the judgment seat ([partnership.md](partnership.md)). Borrow from the *evidence* side
(what was seen, who may claim, what is final); never from the *verdict* side.

**BL-12 · 2026-07-21 addendum (user)** — the reopening condition `dec:repo-file-embedded` wrote
down has now been *asked for in so many words*: "could multiple sessions (same machine or
remote) all use a common MCP — a common/centrally hosted MCP server?" That is the second writer
materializing as a request rather than a hypothesis, which is what the embedded decision said
would reopen the fork. Shape of the answer when picked up: **(a)** stdio MCP is 1:1 by
construction, so a shared server means the streamable-HTTP/SSE transport in front of the same
core — the surface-neutral seam exists for exactly this; **(b)** the cheap first rung is still
the recorded one — RocksDB read-only secondaries if the need is "let me look while you work",
a full service only for true concurrent *writers*; **(c)** a central host puts the design on a
machine the user doesn't control — the decision's strongest objection — so self-hosted-first;
**(d)** the moment two writers are real, BL-44's claims, BL-41's mechanical trust half, and
sketch idea 1 (claims reference the graph state they saw) stop being future work and become the
write-path prerequisites. Sequencing note: BL-71's design-vs-design diff is also the merge
primitive a shared graph needs the day two writers disagree.

**BL-12 · 2026-07-21 field confirmation (StoryFlow fleet trial)** — the lock is now a
*measured* cost, not a hypothesis: in a Boss + workers fleet all under one repo, the Boss's
stdio server holds `.reflow2/graph/LOCK` for its whole lifetime and **workers cannot even
READ the live graph** — "the Boss drives it, everyone else is blind" is forced by the lock,
not chosen (docs/trials-private/2026-07-21-storyflow-fleet-improvement-log.md). The recorded
first rung — **RocksDB read-only secondaries** (`--read-only` open) — now has its concrete
consumer: workers running `where-am-i` / `detect_gaps` / `scan_nodes` against a graph the
single writer holds live. That rung is now the cheapest high-value BL-12 increment and could
be pulled ahead of any service work. (Their "at minimum, a clear lock-conflict error"
suggestion is already built — `explain_open_failure`, BL-57 — but the trial saw a bare
open failure, so verify the message actually *surfaces* through an MCP-spawned server's
stderr in the client, not only on a hand-run CLI.)

**Same day, the user named the destination**: set this up "for a whole organization to access
simultaneously … a great resource for a whole organization to do all their planning and design
with." That is the north star that makes the pieces one product rather than features: a shared
graph (this item) partitioned by claims at cluster granularity (BL-44 — Alex takes this
cluster, Bobby that node), alternatives held open as parallel branches until a decision point
(BL-70 — an AoA the whole org can see), readiness-derived roadmaps (BL-68) computed *from* the
same graph the engineers are editing, cross-project dependencies between one org's many designs
(BL-45), and the trust layer (BL-41 + sketch 2's who-may-assert-what) doing at org scale what
COORD.md does socially for two people. Commercial note: an org-wide deployment is exactly the
paid tier of the licensing direction the user is considering (free personal/school, paid
commercial/government) — the multi-writer service, if built, is the natural thing the licence
sells. Not scheduled; recorded so the increments above are walked in an order that keeps this
reachable.

**BL-12 · AT Protocol design notes (2026-07-21)** — *from the user's question "anything we can
take from atproto.com/docs?" Same discipline as the crypto-consensus notes above: what survived
contact, and where the analogy breaks. AT proto answers "many parties, one shared data layer,
without a referee" — BL-12's question with different nouns — but its repos are deliberately
single-writer-per-identity, so it solves attribution, portability and trust-layering AROUND
single-writer stores, never collaborative mutation of one document. Borrow accordingly.*

1. **Self-authenticating records** (signed commits over a content-addressed root). Every AT
   repo write hashes up to a signed root, so anyone verifies state without trusting the host.
   reflow2's export is already deterministic, so a content hash is already well-defined — it
   was just never computed or carried. **Built same day** (export content hash + prev-hash
   lineage chain + `compare_designs` ancestry — see CHANGELOG); the *per-claim* half — every
   accept/edit carrying the hash of the state it saw, sketch idea 1 above made mechanical —
   stays open here as the two-writer prerequisite.
2. **Identity decoupled from hosting** (DID + keys are yours; the host is a replaceable
   service that cannot forge signed content). Dissolves `dec:repo-file-embedded`'s strongest
   objection to a hosted graph: the host holds bytes it cannot falsify. The shape for the
   shared-MCP future: writers sign, the server stores and relays, anyone audits — "we host
   it" stops meaning "we have custody of the truth". Pairs with the licensing direction (the
   paid org tier sells hosting, not custody).
3. **Labels: assertion as overlay, not mutation** (speech/reach separation; labelers publish
   their own subscribable records ABOUT content, never touching it; consumers pick whom to
   trust). Maps onto report-dont-judge and BL-41's who-may-assert-what — and adds a move we
   lack: a reviewer's "this requirement is risky" as a signed annotation from their identity
   living BESIDE the graph, not a property write inside it. Most of what a second
   collaborator wants early is annotation, not mutation — an overlay layer gives N parties a
   voice on one design with zero write contention, deferring the hard merge to genuine
   structural edits. Candidate first rung for multi-writer, cheaper than any locking scheme.
4. **Per-writer repos + computed merge as a BL-12 shape.** Since AT proto never lets two
   writers share a repo, its architecture suggests the alternative to one shared mutable
   graph: each collaborator appends to their own record and the baseline is a computed merge
   — the git model — with `compare_designs` + the survivor/provenance rules as the merge
   machinery and BL-44's claims as "which subtree I am authoring". Named here so the
   shared-graph and per-writer-repo shapes get weighed deliberately when BL-12 opens.

*Where it breaks:* DID:PLC infrastructure, blobs, firehose scale are for millions of anonymous
parties; the threat model here stays error-and-drift among named collaborators. Lexicon's
publish-forever immutability is for a network that controls neither end; reflow2 controls both
and has export/import migration — the additive-only discipline (already policy) is the right
dose. Take the evidence machinery (hashes, signatures, namespaces); skip the infrastructure.

**BL-12 · github-mcp-server notes (2026-07-22)** — *from the user's question "anything we can
learn from how they set up their MCP?" (clone read at ~/project/github-mcp-server). GitHub's
official server is a live case study in this item's open questions; what survived contact:*

1. **The hosted shape is validated.** Their remote server (githubcopilot.com) is the same
   open-source repo consumed as a library, bound into their infrastructure, serving
   *stateless streamable HTTP* with a fresh server per request. reflow2's core is already
   surface-neutral by decision, so the hosted mode is a streamable-HTTP front over the same
   service layer — with the one structural difference that reflow2's daemon would hold the
   single-writer lock once, centrally: exactly this item's "one server owns the lock,
   session shims are thin clients" option, now with an existence proof.
2. **Read-only is two layers, and they shipped the surface layer.** Per-tool
   `ReadOnlyHint` annotations + filtered registration under `--read-only`, with a CI check
   that *fails the build if any tool omits the hint* (no silent default). For reflow2 the
   full rung is annotation-filtered tools (BL-76) PLUS a RocksDB secondary open — the
   annotation alone would not release the lock.
3. **Sketch idea 2 ships in production**: tools declare required token scopes and the
   server hides what your token cannot do — trust topology per claim type, real. And their
   `--lockdown-mode` (hide content authored by users without push access, as prompt-injection
   mitigation) is [BL-41]'s mechanical half proven in the wild: authority-keyed
   who-may-assert-what. Both are the shapes to reach for when the second writer arrives.
4. **Identity**: local = token/OAuth (official binaries bake in a client id for zero-config
   login); remote = the client brings a bearer token and the server never authenticates —
   attribution is simply "the server acts as that token's user". Composes with the AT-proto
   notes above: signer-side identity, custody-free hosting.

*Where it breaks:* they are stateless over someone else's API — the durable local graph, the
merge problem, and everything temporal stays reflow2's own. Their removal of runtime
dynamic-toolset discovery (selection is static per session) is a data point for BL-77.

**BL-76 · Tool-surface hardening from the github-mcp-server comparison: ReadOnlyHint + toolsnaps
— DONE 2026-07-22** — *2026-07-22.* Size **S + S**; both mechanical, no vocabulary decision
needed. **(a)** Every reflow2 tool declares the MCP `readOnlyHint` annotation (true for the
read/analysis family, false for writes) — clients use it for approval prompts today, and it is
the surface half of the BL-12 read-only rung. With it, the explicitness gate: smoke asserts
every served tool carries the annotation, so a new tool cannot ship unclassified (their
AST-check idea, done reflow2-style over the real stdio surface). **(b)** Toolsnaps: one
committed golden JSON schema per tool, CI-diffed — the BL-28/BL-32/BL-48 bug family ("the
surface changed and nothing noticed") made into a mechanical tripwire; regenerate deliberately,
never silently.

**Done:** all 80 tools annotated (`annotations(read_only_hint = …)` on each `#[tool]`), the
classification derived from the graph borrow itself (`let g` = read, `let mut g` = write) rather
than the name — so `gap_to_prompt` and the `reconcile_*` family, which read like queries but
record, are correctly `false`; 26 read-only, 54 write. `smoke_mcp.py` gained the explicitness
gate plus a both-poles correctness spot-check (present is not enough — an inverted hint would
pass a presence-only gate). `tools/toolsnap.py` freezes each tool's served schema as a committed
golden under `tools/toolsnaps/` (80 files), CI-diffs them, and regenerates on `--update`; wired
into the `full` CI job and the AGENTS.md done-gates. Surface half of BL-12 delivered; the
read-only *transaction mode* (a session that physically cannot write) stays with BL-12. **Next
per the S+S sequencing note above:** the RocksDB secondary-open rung of BL-12 is the natural
follow-on now that the surface declares intent.

**BL-77 · Surface scale: toolsets / verb-multiplexing — parked until it pinches** —
*2026-07-22.* Size **M**; deliberately not now. github-mcp-server holds ~100 tools down with
verb-multiplexed tools (`issue_read` + a `method` enum), ~26 flag-selectable toolsets, and an
allow/exclude list — and notably *removed* runtime dynamic-toolset discovery (static per
session won). reflow2 is at ~80 tools; the fleet's cold-start/deferred-tools friction is the
early symptom. When it pinches harder, the choice is grouping (core-loop / analysis /
release / temporal) vs multiplexing vs both — a vocabulary decision for the user, informed by
which clients actually struggle. Multiplexing collides with skill_lint's tool-name contract
and every skill's tool references; cost that honestly when weighing.

**BL-78 · External-dependency freshness & obsolescence as a first-class coherence check** —
*user, 2026-07-22.* Size **M–L**; a vocabulary decision, not mechanical. The user's framing: "you
build something and find out all your external dependencies are outdated — and maybe worse, now
obsolete." They want reflow2 to notice, and explicitly **not just for software packages** — this
is the "design anything" generalisation. A design depends on things that live *outside* it and
move on their own: a Rust crate or npm package, but equally a referenced standard (a MIL-STD
revised, a spec withdrawn), a COTS hardware part going end-of-life, a discontinued supplier, a
cited document superseded. "Your design rests on something the world has moved past" is a
universal design failure mode, and it is exactly reflow2's territory.

The shape already exists — this points the reconcile family *outward*. `reconcile_artifacts`
compares observed files on disk against the design's checksums and raises drift → gap; a
`reconcile_dependencies` would compare an **observed upstream latest/status** against the design's
**pinned/current** version and raise the same drift, with two distinct severities: *stale* (a
newer version exists) and the sharper *obsolete/EOL/yanked/withdrawn* (the pinned thing is gone
or unsupported — a real gap, not a nudge). The observation stays external (an agent, a CI job, a
`git ls-remote`/registry query supplies "latest" — the core never reaches the network, per
req:deterministic-core); reflow2 records, compares, and surfaces.

**The vocabulary question for the user** (why this is M–L, not S): does an external dependency
extend the existing **Resource** node (add_resource already models "a database, a queue" the
built thing needs — add `current_version` / `pinned_version` / `lifecycle_status`), or is it a
new node type (an `ExternalDependency`, so a crate and a queue aren't conflated)? And what does
"latest/obsolete" mean uniformly across a crate, a standard, and a part — a version string, a
lifecycle enum, or both? Decide deliberately, not additively (the BL-73/BL-75 lesson).

**Load-bearing constraint, already on the record:** notify, do **not** auto-bump. AGENTS.md is
explicit that bumping the dynograph-foundation pin as housekeeping is forbidden — every bump
forces a ~10-min RocksDB C++ rebuild on every consumer, and a bump is a data-migration question
(BL-19). So the feature's output is a *gap the human dispositions*, never an automatic edit —
which is precisely how the rest of the loop already works. Near-term, before the in-graph
feature: a scheduled GitHub Action doing `cargo outdated` (crates.io deps) + `git ls-remote
--tags` against dynograph-foundation, opening an issue on divergence, would cover reflow2's own
deps today (checked 2026-07-22: dynograph pinned v0.10.0 == latest tag; rmcp pinned "2" →
2.2.0). That script is the S down-payment; the in-graph generalisation is this item.

**Note 2026-07-31 — the RATE is the finding, not the version.** rmcp released **3.1.0** while the
repo sat on 3.0.1 (pin is `"3"`, so `cargo update -p rmcp` takes it today; nothing is blocking
it). The user's point is the durable half: **v3 was still beta days earlier**, so by the time this
item is revisited the number here will be wrong — which is precisely the failure BL-78 describes,
demonstrated on BL-78's own note. Do not read "3.1.0" as current; read it as *this dependency moves
faster than the backlog does*, which is the argument for a computed check rather than a written one.
Two specifics worth carrying anyway: 3.1.0 touches **protocol version negotiation** (`honor
supported_protocol_versions`) and **stateless protocol metadata validation** — the exact area where
the v3 upgrade broke seat identity *silently while every gate stayed green*, because no test client
spoke the newer protocol version. `tools/stateless_seat_probe.py` is now a CI gate in the `full`
job, so that class of breakage would be caught this time; it exists because of that incident. When
the bump is taken, `dec:rmcp-v3-upgrade`'s own method applies — upgrade and **measure**, and treat
the probe as the evidence rather than the suite.

**BL-108 · `loop_status` calls a PLANNED capability "a built capability never checked against
reality"** — *found 2026-07-31 by following the nudge it produced.* `confirmation_ledger` attributes
a capability's artifacts through its **component**, not only through its own `REALIZES` edges. So
`cap:skill-triggers` — `status: planned`, zero artifacts realizing it, allocated to `cmp:nudge` —
inherits `art:nudge-detect` and lands in `unexamined`, and `loop_status` reports *"1 built
capability(ies) never checked against reality"* about something nobody has built. Size **S**.
**Why it matters beyond the one node:** this is a detector punishing correct work — capturing a
planned capability and allocating it properly is exactly what capture-intent asks for, and doing it
raises loop debt. That is [BL-23]'s lesson ("when a detector punishes correct work, the answer is a
different question, not a tuned threshold") and the same shape as [BL-42]'s noise floor. **Fix
shape:** the ledger's own description says "for every capability **with built artifacts**" — so
either exclude capabilities whose status is `planned`, or attribute only artifacts that `REALIZES`
the capability directly rather than reaching through `ALLOCATED_TO`. The second is probably right,
because a component's artifact is evidence about the component, not about every capability allocated
to it — but it is a behaviour change to a shipped reading and wants deciding rather than assuming.
**Watch for the counterweight:** a capability legitimately realized only through its component's
artifact would then read as unexamined-because-invisible instead of unexamined-because-unchecked, so
whichever way it goes needs a test pinning both directions.

**BL-79 · The identity keystone: who authors the design — RUNG 1 (authorship seed) DONE
2026-07-22** — *user-chosen as the next most important, from the recurring "the schema has no
notion of who" thread the backlog kept naming.* The missing keystone under four threads: BL-44
(claim a node/cluster for parallel work), BL-70 (an AoA alternative needs an author), BL-41 (the
*mechanical* half of requirement-certainty — "the user's word" is enforced only culturally
today), and BL-12 sketch 2 (who may assert what, when the second writer arrives). The backlog's
own warning made it one item, not four: *"one identity mechanism should serve all three; building
it for claims alone would be the recurring lesson in reverse."*

**User decisions (2026-07-22):** a **new `Contributor` node type**, kept separate from the
existing `Actor` — `Actor` is *who the designed system serves* (boundary-facing domain content),
`Contributor` is *who authors and decides the design itself* (person / automated_agent /
organization); conflating them blurs two lifecycles a UAF model keeps apart. First rung =
**authorship seed**: `AUTHORED_BY` (design node → Contributor), the structured *who* behind
provenance's *how*, replacing the free-text `Requirement.source`. First-class node (not a
property) because a claim expires and authorship belongs on axis-Z — a bare property is invisible
to history.

**Rung 1 DONE:** schema += `Contributor` + `AUTHORED_BY` (core.yaml; 28 node / 55 edge types);
`AUTHORED_BY` deliberately NOT in `structural_rule`, so authorship is metadata and never enlarges
a blast radius (smoke asserts it). `nodes.rs` name constants; `add_contributor` + `authored_by`
core constructors and MCP tools (readOnlyHint=false, 82 tools, 2 new toolsnaps); capture-intent
skill records the driver once per session and attributes nodes *when captured, not at exit*.
Schema counts updated in schema.rs/vocabulary.rs/smoke; all gates green. **BL-19 note:** additive,
so a new-binary graph is refused loudly by a pre-Contributor binary (count-based provenance
check) — no `schema_version` bump needed.

**Remaining rungs (need the user on vocabulary each):** **(2)** `CLAIMS` — a Contributor claims a
node or a computable cluster (`propagate_from`/community), advisory-first per `dec:report-dont-judge`
(this is BL-44). **(3)** `ACTS_FOR` — agent-acts-for-person: git's **author vs committer** split
(Anthony authored, Claude Code committed) — the git exploration ratified this exact shape. **(4)**
alternative-authorship for BL-70. **(5)** signing the export `content_hash` with a Contributor's
key = BL-41's mechanical half + `dec:export-hash-chain`'s deferred signing, "when a second writer
is real." Deliberately no "unauthored node" detector yet — it would N-alarm on every existing
node (the BL-23/42 lesson).

**BL-80 · Git's merge model is the missing half of reflow2's git-like history** — *from the
user's question "we do a few git-like operations — anything in /home/ajs7/project/git we could
apply?" (2026-07-22, explored against git's object/merge model and
`Documentation/technical/{trivial-merge,rerere,sparse-checkout,partial-clone}.adoc`).*
**#1+#2 BUILT 2026-07-22 (propose + apply)** — `crates/reflow2-core/src/merge.rs`: `merge_designs`
computes the three-way case table (pure/deterministic proposal, writes nothing), `resolve_merge`
materialises the merged design from the human's per-conflict decisions, and `apply_merge` commits
it into the live graph atomically — **refusing until every conflict is decided** (and on a
resolution that names no conflict). 21 tests. Exposed as the `merge_designs` (read-only) and
`apply_merge` (write) MCP tools + the `--merge` CLI. Decisions on the record: `dec:merge-three-way`
(explicit inputs over git's DAG; reports a proposal, never auto-applies) and
`dec:merge-conflict-semantics` (delete/modify → retain-and-ask; edges symmetric; conflicts are
Questions with deterministic ids, rerere-ready). Realizes + verifies `cap:merge-designs`. This
**closes the core of BL-12's merge**. #2's ancestor-retrieval is git's for now (merge-base); the
reflow2-native DAG/ref layer (so reflow2 finds its own merge-base) stays with BL-70.
**#5 rerere BUILT 2026-07-22** (`cap:merge-rerere`, `dec:merge-rerere`): each conflict carries a
`resolution_key` — a **content** fingerprint (values + property, node-independent, git's model, NOT
the ancestor-hash conflict id); `apply_merge` records every applied property/edge-property
resolution as an answered Question whose id is the key (travels in the export, no schema change);
`recall_resolutions` returns recorded decisions and `apply_merge use_recorded` reuses them —
**advisory**, the human opts in (`dec:report-dont-judge`). Resolve the shape once, apply across all
N near-identical conflicts (the BL-73 pain). 25 tests.
**#3 file-pure apply BUILT 2026-07-24** (`crates/reflow2-mcp/src/main.rs`): the `--merge-apply` CLI
completes the git-file workflow — `--merge base ours theirs` prints the conflicts and their ids, a
JSON decisions file maps each id to base/ours/theirs, and `--merge-apply base ours theirs
--resolutions FILE` runs `resolve_merge` and prints the merged export document. It opens no graph
(runs while a server holds the lock) and, unlike `--merge`, *refuses* — non-zero exit, no document —
until every conflict is decided; records no rerere memory (that lives in the graph). 4 CLI tests;
`art:main` accepted `design_updated` via `chg:bl80-merge-apply-cli`, `cap:merge-designs` surface
refreshed. Still open on BL-80: node-type/delete-modify rerere keys (no clean value triple) and
rerere memory aging/pruning. The
finding that reframes the whole multi-writer thread: **reflow2 already built the content-addressed
*history* half of git** — hash-chained exports (`dec:export-hash-chain`) = commits, DesignEpochs +
Snapshots = immutable history, `compare_designs` ancestry = merge-base *reporting*. What git has
that reflow2 doesn't is the **merge + branch** half, which is exactly BL-12 / BL-70 / BL-44 / BL-41.
Git is the canonical prior art, and reflow2's typed-node model makes several ideas *cleaner* than
in git. Applicable imports, most-valuable-first:

1. **Three-way typed-node merge — specifies BL-12's merge problem.** ⭐ highest value. Git merges
   per path against the common ancestor: only one side changed → take it; both changed
   differently → conflict (the `trivial-merge.adoc` case table literally). Run the SAME algorithm
   per **node** and per **property** against the common-ancestor export — and `compare_designs`
   already computes that typed diff, so it is the input. A both-sides conflict becomes a
   structured **Question/gap** ("node X.prop Y: base=A, ours=B, theirs=C — which?"), which *is*
   `dec:report-dont-judge` / human-decides. Superior to git: no `<<<<<<<` line markers, typed
   values not lines, and the ask-the-human machinery exists. Turns BL-12's merge from open to
   specified.
2. **merge-base / retain the common ancestor** — prerequisite for #1. The chain is linear
   (`prev_content_hash`) and must become a **DAG** to represent branches; the ancestor export
   must be retrievable (content, not just its hash — which `compare_designs` already reports).
3. **Branches = named mutable pointers over immutable epochs → BL-70.** A git branch is just a
   named ref to an immutable commit. Epochs + `PRECEDES` are already the commit chain; add the
   **ref layer** (a name → an epoch/hash, permission for >1 head) and AoA alternatives largely
   fall out. BL-44's cluster-claims and BL-70's alternatives "want the same scoping primitive" —
   git says that primitive is a ref + a merge-base.
4. **author vs committer + signed commits → ratifies BL-79 + mechanizes BL-41.** Git records
   author (who wrote it) *and* committer (who applied it) distinctly, and signs commits over the
   object hash. The author/committer split **is** the deferred `ACTS_FOR` rung (person authored,
   agent committed); signing the export `content_hash` with a Contributor key is BL-41's
   mechanical half + `dec:export-hash-chain`'s deferred signing, "when a second writer is real."
   No design invention — copy the proven split.
5. **rerere — reuse recorded conflict resolutions** (later rung). Record how a human resolved a
   node/property conflict, keyed by a normalized conflict id, and auto-replay it when the same
   divergence recurs. reflow2 already mints deterministic gap ids (FNV over source+sorted-affected)
   — the same normalization instinct.
6. **Merkle-per-node hashes** (defer) — the export hash is a single flat hash, not a tree of
   per-node hashes; per-node hashes would make diff/merge O(changed) and enable partial sync, but
   it is an optimization at ~300-node scale, not a correctness need.
7. **sparse-checkout / worktrees as the *model* for BL-44** — a contributor scoped to a computable
   subgraph (`propagate_from` / community). Borrow the cone/worktree concept, not git's
   filesystem mechanism — reflow2's claim layer is advisory (`dec:report-dont-judge`).

**Explicitly do NOT import** (reflow2 is a design graph, not a filesystem): line-based diff/merge
*representation* (the 3-way *algorithm* transfers, the line orientation is strictly worse than
typed diffs — the key "graph ≠ files" caveat); packfiles / delta compression; the smart
fetch/push transport (whole-file export/import is the right-sized `git bundle` analogue); the
index/staging area (continuous-capture doctrine deliberately rejects a staging gate). Sizes: #1+#2
together are the **L** core of BL-12; #3 is **M** and mostly BL-70; #4 near-term **S** (adopt the
split) then later signing; #5–#7 later rungs.

**BL-81 · reflow2 as a navigable decision-MDP — the roads not taken, held forkable** — *user's
idea on a walk home, 2026-07-22, developed live across three framings: choose-your-own-adventure,
analysis of alternatives, and a reinforcement-learning gridworld. Size **L**. Advances [BL-70] and
unifies it with [BL-80] and [BL-44]; captured as graph nodes with scope decided.* The one-line
frame in the user's words: **the graph is the world-model, defects and field trials are the
reward, branches are the replay, and the human does the credit assignment.**

Extends [BL-70]'s "AoA branches held open until a decision point" from a held-open *superposition*
into a *navigable decision tree over time* — any past Decision can be re-opened at its epoch and a
different branch taken from that point, the "return to the choice page" of a choose-your-own-
adventure book. Captured 2026-07-22: `req:roads-not-taken` (accepted — extends `req:intent-preserved`
from "the past is never overwritten" to "the roads not taken are never lost either"),
`dec:alternatives-unranked-forkable` (accepted), `cap:fork-alternatives` (planned, deliberately
**unallocated** — its owning component is BL-70's branch/history-navigation module, still to design).

**Scope decided — answers BL-70's open ranked-vs-not question: unranked.** Alternatives are
forkable siblings, not a scored trade-study runner-up; the human judges in hindsight
(`dec:report-dont-judge`, `dec:three-party-checks`). A stored ranking would be the system asserting
a preference it cannot honestly hold, and design episodes are too few, too expensive and too
subjective for a stationary reward. This tempers BL-70's "run the comparison machinery per branch":
the machinery (`budget_report`, the dimension assessments, [BL-68]'s readiness scores) still
*informs* the human per branch, but it never *ranks for* them.

The RL frame, and where it is load-bearing versus where it would mislead:
- **Model-based planning, not model-free crashing** — the graph is a world-model:
  `propagate_change` / `detect_gaps` / `compare_designs` evaluate a fork's consequences *before*
  anything is built, so you simulate the hole instead of falling in it. This is the whole reason a
  design brain earns its keep.
- **Two reward channels** — the structural detectors are *in-model* holes (SPOF, cycles, gaps),
  computable a priori; **field trials are *out-of-model* holes only a real episode reveals** —
  [BL-74]'s loop-on-virtue collapse was invisible to every detector until the StoryFlow fleet fell
  in. Both feed the replay buffer.
- **The cliff: no auto-optimize.** reflow2 supplies the replay buffer and the world-model; the
  **human is the reward function** and does the credit assignment / policy update. Replay-and-score
  to auto-pick a design would manufacture exactly the silent default the whole system forbids
  (`dec:three-party-checks`).

**Architecture stance (`dec:alternatives-unranked-forkable`): store the fork, compute the
consequence.** Keep the unchosen option + its rationale cheaply at the decision's epoch — this
upgrades BL-70's "`Decision.alternatives` = the losers' obituary" from post-hoc prose into a live,
forkable sibling — and *compute* the alternative's consequence design on demand by forking that
epoch and replaying; never eagerly maintain N full parallel designs, which combinatorially explode
and **rot** as the main line moves on (a non-stationary MDP). Which fork to expand follows the
reward: **expand where the surprise is** — a fresh defect, a failed trial, or a rationale that
already flags doubt points back to a decision (high "TD error") — not the whole tree. That is
MCTS-style selective deepening, and it is the selection rule BL-70's cheapest increment (one export
per alternative) lacked. Precedent that the mechanism already works by hand:
`dec:merge-survivor-provenance` carries a "reconsider" note tracing a near-bad merge back to its own
fork, and the 2026-07-22 hindsight pass over all 19 Decisions found it by exactly this trace.

How it unifies the threads: **[BL-70]** supplies the branch/ref scope (the fork layer); **[BL-80]**'s
three-way merge (`dec:merge-three-way` / `dec:merge-conflict-semantics`) is *how a forked road comes
home* — taking the other path and bringing its consequences into the baseline is a three-way merge
against the fork's epoch; **[BL-71]** rung c (`compare_designs`) shows the difference between the
taken and untaken path; **[BL-44]**'s cluster claims are the same scoping primitive at
work-parallelism scale (BL-70 already noted the two share it).

Decisions still the user's: the schema attachment for an alternative on a Decision — node-set tag
vs sub-graph vs graph-per-branch, shared with BL-70's first open question and BL-44; and the
genuinely new one this frame surfaces — **the "when has an episode taught us enough to revise?"
trigger**: what counts as enough reward signal (how many defects, which severity, a failed trial)
to *re-open* a fork rather than let it lie. That threshold is the credit-assignment rule, and it is
the open conceptual thread this item leaves on the table.

**BL-82 · Vocabulary hygiene — orthogonality and usage** — *user, 2026-07-22, from the storyflow
lesson that edge types an agent chooses between must be orthogonal (near-synonyms → inconsistent
extraction).* **Orthogonality pass DONE 2026-07-22** (`dec:edge-orthogonality`): the standing rule
is now "a distinction earns its keep only if a computation reads the two sides differently."
Retired `VALIDATES` (orphan + confusable with `VERIFIES`; moved to `Verification.kind` + the
`unvalidated_capability` gap) and `ENABLES` (folded into `CAUSES`); kept `TRIGGERS` (its `role`
drives Flow). 55 → 53 edges.

**Usage sweep, OPEN** — a companion audit (same "does anything read/write it?" test) found the
schema carries **dormant subsystems**: scaffolded vocabulary with *no code path either way*, distinct
from the delete-worthy `VALIDATES` case because they are **not redundant — just unbuilt**. Found:
(a) the **bitemporal layer** — `TemporalFact` node + `HAS_TEMPORAL_FACT`/`ABOUT_ENTITY`/`VALID_FROM`/
`VALID_TO` (no constructor, no reader); (b) **environment compliance** — `EnvironmentRule` node +
`IMPOSES`/`COMPLIES_WITH`/`VIOLATES_RULE` (none written or read); (c) **actor interaction** —
`Actor` + `INTERACTS_WITH`/`OPERATES_IN` (rendered in reports, never constructed via a typed path);
(d) lone dormant nodes **`QualityGate`** and **`Anchor`** (zero references anywhere);
(e) `CONTAINS_EPOCH` (epoch nesting, unbuilt). The **inference "why" edges** (`CAUSES`, `EVOLVES_INTO`,
`MITIGATES`, `MASKS`, `ANTICIPATES`, `SUPPLEMENTS`, `ANNOTATES`, `SPECIFIES`, `PRODUCES`, …) are a
*third* category — reachable only via generic `create_edge`, nothing computes on them: intentional
agent-populated vocabulary + PROPAGATE traversal fodder, **not** dead. Disposition per dormant item
is the user's: a real planned capability (keep the scaffold, mark it deferred in the coverage
matrix) or abandoned (retire the vocabulary). Test to carry forward: **unused AND redundant →
delete (VALIDATES); unused AND not-redundant → deferred, not dead.**

**BL-83 · The self-model decomposes by file, not by function** — *user, 2026-07-22, the sharpest
self-application finding of the session: "if I designed a car, reflow2 should identify engine /
frame / drivetrain / cabin as the systems; for a graph tool, the systems are nodes and edges — did
it identify that?"* Size **L** (the re-derivation is a real design exercise). **The answer was no,
and the *way* it is no is the diagnosis.**

> **DONE 2026-07-23 — all three moves.** (a) genesis re-derived reflow2's seven functional subsystems
> WITH the user (`dec:bl83a-functional-decomposition`); (b) `adopt` recovered a clean as-built module
> model on a copy (dogfood: no vacuity, caught its own over-claim); (c) `compare_designs` measured the
> divergence — `117 added / 186 removed / 18 changed`, 1 shared Component of ~42, capability count
> converged 33 = 33. Thesis confirmed and quantified: from the artifact adopt re-derives *modules*;
> the *functions* are a design layer the code doesn't carry, and they relocate to Flow/Interface when
> recovered. Full write-up: `docs/trials/2026-07-23-bl83b-adopt-dogfood.md`. Details in the three move
> bullets below; spawned BL-84/86/87/88/89.

reflow2's own 38 Components are the **Rust module list** (`graph`, `schema`, `detect`, `export`,
`service`, `dto`, `main`, …) — every one `kind: module`, every purpose literally *"The X module."*.
The domain primitives a design-graph product fundamentally IS — **node types and edge types** — appear
**nowhere** in the graph (searched: zero hits for the vocabulary as a subject). In the car analogy:
the self-model is the *factory's parts bins labeled by aisle*, not *engine / frame / drivetrain*.

**Root cause:** the self-model was **recovered backwards from source** (`build_design_graph.py` walks
the file tree) plus session accretion — so it is the **as-built** decomposition (the file layout),
never the **as-designed** functional one. A genesis exercise from the brief *"a system that captures
any design in a graph"* would surface *vocabulary / store / coherence-loop / golden-thread / question
channel* as the systems, with the Rust modules re-parented **under** them as implementation. That
exercise was never run on reflow2 itself. This is **the `VALIDATES`/BL-82 gap one level up**: the
vocabulary rot was invisible to the detectors *because the vocabulary is not in the graph*, and it is
not in the graph *because the decomposition axis is implementation, not function*. Two symptoms, one
cause.

**What DID work (fair credit):** given a decomposition, reflow2 finds what is load-bearing —
articulation-point SPOF analysis correctly named `cmp:graph`, the export chain, and `cmp:service` as
genuine cut vertices (accepted as `dec:*-spof-accepted`); modularity/coupling/surprises all computed
and true. *Originating* the right carving is deliberately not the graph's job (`dec:three-party-checks`
— the human designs, the graph remembers and counts).

**But one detectable thing was missed, and it is fair to hold:** every component purpose restating
its name (*"The X module."*) is **zero recorded intent** — `design_without_intent` at component
granularity, mechanically checkable (purpose ≈ name → nothing captured), and nothing fired.

**Three moves, increasing depth:**
1. **`vacuous_purpose` DETECT gap** (**S**) — a purpose/statement that merely restates the node's
   name is recorded structure with no recorded intent. Would flag all 38 of our own modules today.
2. **Schema-as-design** (**M**, = BL-82's fix, now motivated) — model the node/edge types as
   first-class design content (elements something SATISFIES and code REALIZES), so the next
   `VALIDATES` (unmotivated + duplicate) and the next dormant type (orphan/disconnected) are
   *detected gaps*, run by the detectors reflow2 already has, not a hand `grep`. The recursion is
   clean: the graph tool modeling its own type-graph, linted by its own graph detectors.
3. **Re-derive reflow2's functional decomposition** (**L**, a session *with the user*) — run genesis's
   question fresh on reflow2 itself: *what are the systems of a thing that captures any design in a
   graph?* Almost certainly vocabulary / store / loop / thread / question-channel, with the 38 Rust
   modules re-parented as the implementation layer. The engine/frame/drivetrain view the self-model
   never had — and exactly the kind of exercise reflow2 exists to run. Best done in a **fresh
   session** (clean context + it needs the rebuilt binary; the running server predates the 28/53
   schema change).

   **Method — as-designed vs as-built, reconciled by compare (recommended, from the 2026-07-22
   "should we just re-adopt?" exchange).** The tempting shortcut — wipe the graph and re-run
   **adopt** on reflow2 to "rebuild it from scratch" — is the wrong primary move, and *why* is the
   whole finding: **adopt is reverse-engineering; it recovers from the artifact, and the artifact
   (the code) is organized by module, so adopt re-derives a module decomposition — it reproduces
   this finding more politely, it does not fix it.** The functional carving (nodes/edges as systems)
   comes from the product's *purpose*, a genesis input, not from the code; and it *cuts across*
   module boundaries (the "vocabulary system" is `schema.rs` + `nodes.rs` + part of `graph.rs` + the
   YAML — no single module), a cut adopt structurally cannot see. Worse, wiping would strand the
   irreplaceable layer adopt **cannot** recover: the ~24 Decisions + rationale, Contributors,
   authorship, requirement certainty, gap acknowledgements — the *why*, the "what nobody can know
   from the artifact alone." So:
   - **(a) genesis (as-designed): DONE 2026-07-23** (`dec:bl83a-functional-decomposition`). Ran
     genesis's own question on reflow2 WITH the user: the seven functional subsystems a
     design-graph tool IS — **Vocabulary** (node/edge types — the domain primitive that appeared
     nowhere), **Design Store**, **Coherence Loop**, **Human Channel**, **Time & History**,
     **Intake**, **Agent Surface** — with the 35 Rust modules re-parented under them and the three
     as-built crate components (`cmp:core/mcp/kit`) retired on record. `level: subsystem` (reflow2
     is the system, `proj:reflow2` its root), so the hierarchy stays one level at a time. Recorded
     durably in `tools/build_design_graph.py` (SYSTEMS + SYSTEM_OF + retirement), not hand-edited;
     self-model 309→315 nodes. Measured: **gaps unchanged (4→4, all pre-existing** — the
     `cap:fork-alternatives` hole and the orthogonality `unvalidated_capability`); **defects +2**,
     both from 3a: the `ifc:core-api` SPOF (accepted, `dec:core-api-spof-accepted` — the DesignGraph
     API is the sole Store↔Surface bridge, like the graph handle and MCP service) and an 8-node
     "subsystem island" `disconnected_community` — a **detector false positive** raised as **BL-84**
     (the recursion working: the graph tool linted by its own detectors caught a BL-69-family blind
     spot). The purpose text of each subsystem is functional, not `"The X module."` — the recorded
     intent the module layer lacked. Remaining: **(b)** adopt on a copy, **(c)** compare.
   - **(b) adopt on a COPY (as-built, additive): DONE 2026-07-23.** Ran `adopt` against a stripped
     copy (`/home/ajs7/project/reflow2-bl83b/` — code + schema + kit, with reflow2's own design
     record and self-study tooling removed so the design had to be *recovered*, not read). Recovered
     143 nodes / 238 edges; **adopt worked** — no `"The X module."` vacuity (descriptions from
     signatures/imports/contracts), 17 unmotivated capabilities surfaced as real questions (author
     confirmed 7 as intended features, +7 authored requirements; 15 requirements recovered from
     README/comments promoted to accepted with provenance kept `inferred`), and
     `status_contradiction` **caught the adopter over-claiming `verified`** — the anti-erosion value
     prop demonstrated on a live recovery. Ended at the correct adopt state: 7 gaps deliberately
     open. Full evidence: `docs/trials/2026-07-23-bl83b-adopt-dogfood.md`. Also surfaced (and this
     session fixed) two real bugs — the stamp-ordering bug (`f4109cb`) and the misleading refusal
     message (`dc9bf96`, BL-86) — plus BL-87 (import_graph stamp), BL-88 (untested CI gate), BL-89
     (adopt-doctrine tweaks). Caution recorded: adopt found a real failing test but *confabulated* a
     nonexistent "committed fixture" as its cause — findings need verification gating.
   - **(c) `compare_designs` (a) against (b): DONE 2026-07-23 — the finding, measured.**
     `--diff docs/design/reflow2.json reflow2-asbuilt.json` → `design_added 117, design_removed 186,
     design_changed 18`; the two models barely overlap. **Structure diverges almost completely** (1
     shared Component of ~42): as-built recovered a *module/cluster* decomposition from the artifact
     and did **not** re-derive the 7 functional `sys:*` subsystems — BL-83's thesis confirmed and
     measured. **The functional concepts relocated rather than vanished** — the coherence loop came
     back as a `Flow` (`flow:coherence-loop`), vocabulary as an Interface — onto different node types,
     not structural subsystems. **The capability count converged (33 = 33, 9 ids shared)**: *what the
     system does* is intrinsic to the artifact; *how it is decomposed* is a modeling choice two honest
     passes make differently. And the **"why" layer is absent in (b)** (0 of a's 28 ChangeEvents / 37
     DriftEvents / 5 Questions / 2 Contributors / 3 Epochs) — the irreplaceable layer adopt cannot
     recover, now visible as `supporting_removed: 111`. Reproduced independently in the real-repo
     session. **BL-83 is DONE.**
   Never wipe the live graph; adopt runs on a copy, genesis extends the real one.

**BL-84 · The community/SPOF analysis flags pure-decomposition nodes as an island — DONE
2026-07-23** — *surfaced by BL-83a on reflow2's own self-model, 2026-07-23. Size **S**, BL-69
family. Fixed both halves. Community: `disconnected_community` now skips an island reachable from
the main body through `CONTAINS` (the cluster-level twin of `dead_end`'s assembly exemption); the
8-node subsystem island cleared on the real self-model (detect_defects 9→8), while the genuinely
disconnected clusters — fork-alternatives and the parked BL-91 intent — correctly still fire. SPOF
sibling: `couples_only_as_a_library` now also spares `data` medium, and a new `interface_is_foundation`
twin spares an Interface node whose own medium is `library`/`data`. `is_foundation_medium` is the
shared predicate; silence still earned by an explicit medium, REST stays a candidate. 3 new tests in
`crates/reflow2-core/tests/structural.rs` pin both directions; the two live SPOF interfaces are REST
so the accepted architectural SPOFs are untouched. The finding is kept as evidence — the graph
tool's own detectors catching a blind spot in the graph tool.* When the functional
subsystems were seeded (`level: subsystem`, connected downward only by `CONTAINS`), `detect_defects`
reported the seven of them plus their governing Decision as an 8-node `disconnected_community`. The
cause is the same one [BL-69](#closed) fixed for `single_point_of_failure`: the structural detectors
run on the **as-built operational network** and correctly exclude `CONTAINS` intent edges — but a
subsystem is a *pure decomposition node with no operational edges by design*, so excluding its only
edges leaves it a false island. BL-69 taught the SPOF candidate enumeration to skip non-operational
nodes; the **community/modularity detector never got the same lesson**, because until BL-83a the
self-model carried no pure-grouping components (the retired crates each had an operational edge —
`cmp:core` *provided* the DesignGraph API). Fix: exclude nodes whose only edges are `CONTAINS`
(decomposition) from the operational-network community analysis, the same way `single_point_of_failure`
already scopes its candidates. Pin both directions: a subsystem grouping does not island; a genuinely
disconnected *operational* cluster still fires. The finding is worth keeping as evidence — it is the
clean recursion BL-83 predicted, the graph tool's own detectors catching a blind spot in the graph
tool.

*Sibling, same family, from the BL-83b adopt dogfood (2026-07-23, C.2):* `single_point_of_failure`
still fires on `library`/`data`-medium hubs (`ifc:schema-vocab`, `ifc:graph-persist`, `cmp:core-store`
in the recovered graph) where "add redundancy" is meaningless — a foundation everything links against
isn't a failure point in the SPOF sense. BL-6/F6 already taught SPOF to skip *components* coupled only
by a library contract; the gap is that a `medium: library|data` **Interface** hub still fires. Fold
into this item's fix: the structural detectors should down-rank or skip nodes whose only contracts are
`library`/`data` medium, the same way this item skips `CONTAINS`-only nodes.

**BL-85 · The backlog is reflow2's own requirements stream, not a monolithic file** — *user,
2026-07-23, following BL-83a.* Size **L** (a vocabulary + view design and a practice change;
keystone-adjacent). The smell: `docs/backlog.md` is growing long, and a *design-coherence* tool
keeping its own todos in a side markdown file is the same shape BL-83 named one level up — state
that should live *in the graph* living *outside* it. The user's framing is the spine: **treat each
new BL as a new user requirement to reflow2.** It has already happened once — BL-27 ("adopting a
system that already exists") became `req:adopt-existing`, `accepted`, in the self-model now. The
Requirement lifecycle already **is** the backlog lifecycle: `status` (proposed → accepted →
deferred / dropped → met) = raised → agreed → parked / won't-do → done; `priority` = urgency;
`provenance` = who raised it (user vs a trial inferred it); BL-75 **certainty** = is-it-real; and
**"done" is COMPUTED from the golden thread** (a requirement is `met` when something satisfies it and
the capability is realized / verified), so % progress falls out *derived*, never asserted — the
anti-erosion property (BL-35) for free. `detect_gaps`'s `unsatisfied_requirement` is already backlog
triage; `loop_status` (BL-74) is already a proto-backlog view.

The refinement that keeps it honest — **not every BL is a requirement**, and collapsing them all
loses signal: a *defect* item (BL-42, "a detector punishes correct modelling") is a **gap / failing
verification against an existing requirement** (`req:no-silent-fallback`), not a new one; an
*open-choice* item (BL-29, the survivor rule) is a `Decision` / `Question`. So the move is not a new
`Task` node type — it is **running the backlog through reflow2's own `capture-intent`**, classifying
each raised item into the vocabulary the tool already has (mostly Requirement, some gaps, some
Decisions). The deepest dogfood there is: "improve reflow2" going through reflow2's own
capture → detect → decide loop.

What is genuinely NEW (the residue after the mapping): **effort / size (S/M/L)** — a
build-management attribute, not a design property, arguably out of scope and folding into readiness
(BL-68); **sequencing** beyond priority; **risk-to-completion**, which is exactly BL-68's thesis
(*"the roadmap is a risk-burndown schedule"*, TRL/MRL); and the **rich narrative** (some BL items
are three paragraphs of reasoning) which stays as `documents`-linked Artifacts (BL-26), lossless,
rather than crushed into a property. So: tracking-state in the graph, essays as linked docs, the
whole rendered by a `backlog_report` / roadmap **projection** (SYNTHESIZE, BL-40 — a view, never a
second source of truth).

The load-bearing caution, on the record: the self-model has **19** requirements today, all
`accepted`, load-bearing. Pouring ~84 backlog items in as requirements 5×'s that layer with
mostly-`proposed`, churny, superseded work — and the *core intent* ("the things reflow2 must be")
can drown in the *work-stream*. The mitigation is reflow2's own doctrine: items enter `proposed`,
and only the user's word promotes to `accepted` (BL-75), so accepted + high-certainty stays
separable from the raw stream, and met / dropped are axis-Z history, not cruft. **It works iff the
certainty discipline holds; it fails the day everything is rubber-stamped `accepted` on entry.**
Connects to BL-68 (risk-burndown roadmap), BL-65 (risk), BL-40 (the projection), BL-26 (narrative as
documents), BL-74 (`loop_status` = the proto-view). BL-83(b)/(c) is a miniature of the same
principle (reflow2's own as-built state in the graph, not a side doc), so this item is well-timed to
follow it.
> **2026-07-24 addendum — communities, not human-labelled families (this item's payoff, and how to
> de-risk it).** Once each BL is a node, the *families* a person intuits ("the loop-signal-quality
> thread: BL-74/90/91/93") should fall out of the graph's **own structure** via community detection —
> the same Leiden reflow2 already runs in `propose_allocation` and the modularity /
> `disconnected_community` detectors, pointed at the requirement graph instead of asserted by an
> agent reading prose. That moves "family" from LLM extrapolation (which `partnership.md` distrusts)
> to a computed **projection** (BL-40): derived, never asserted — when *I* say "family" it is
> pattern-matched prose; when Leiden says it, it is the topology. The honest caveat is the design
> content: **communities come from edges, not nodes.** A requirement clusters only through the
> capabilities/components it shares, so emergent communities skew toward *subsystem* ("everything
> routed through `cmp:service`") over *theme* ("loop signal quality"), and a `proposed` requirement
> with no satisfier yet is a **singleton** until it is built. Where the graph's communities *diverge*
> from a human's intuited family is itself the finding — it says whether the theme is real structure
> or narrative. **De-risk before the L migration**: capture one cluster as requirements and run
> community detection — grouped ⇒ greenlight; scattered ⇒ the requirement graph needs thematic edges
> (or a different projection) first.
>
> **Result (2026-07-24) — the clean hypothesis is refuted, and the refutation is the point.**
> reflow2's own Leiden (`propose_allocation`) returns **34 singletons, modularity 0**: capabilities
> carry *no sibling edges* (they link up to Requirements and down to Components, never cap-to-cap),
> so the capability layer has no community structure to find. Louvain over the golden-thread
> projection (176 nodes / 217 edges; a networkx proxy for Leiden) **scatters the 5-requirement loop
> family across 4 communities** — `loop-fires-on-triggers` + `nudge-covers-bypass` group *only*
> because they share the exact same capability (`cap:loop-status`); `read-surfaces-debt` (a different
> capability) splits off, `agent-native` splits off, and the unbuilt `disposition-accepted-defects`
> is a singleton (no satisfier — confirming a `proposed` requirement doesn't cluster until built).
> Adding `GOVERNED_BY` collapses to 14 communities but merges into subsystem/platform grab-bags, not
> the theme. **Conclusion:** community detection over the golden thread yields *delivery* families
> ("these requirements ship through the same capability / part" — genuinely useful; it *is* the
> operational roadmap) but **not** the cross-cutting *theme* a human names ("loop signal quality").
> The theme is narrative, not current structure; encoding it would need an explicit theme edge (back
> to human labelling, which defeats the point) or a different signal (citation edges between BLs;
> text similarity — but that is the LLM again). So BL-85's roadmap projection should promise the
> **delivery** view and treat thematic families as a separate, harder question — the experiment saved
> the L migration from over-promising. The graph checked the agent's "family" story against its own
> wiring and disagreed: exactly the property this whole tool exists for.

**BL-86 · The provenance stamp is count-based, so a schema *removal* breaks the upgrade direction**
— *user, 2026-07-23 (real graphs: storyflow, @bro's projects, written by pre-orthogonality
binaries); DONE 2026-07-24 — message half + the set-based real fix.* Size **S–M**. `req:survives-upgrade` promises "an existing
graph opens, or is refused loudly with what to do." The BL-19 stamp records **counts** (`node_types`,
`edge_types`), and the refusal fires when the graph's counts exceed the running binary's ("knows more
of the schema"). That is exactly right for the **additive** case (an old binary meeting a newer
graph). But the edge-orthogonality change **removed** two edge types (55 → 53) without bumping
`reflow2_version` (`0.9.0` both sides) or `schema_version` (`1` both sides) — so a graph written
*before* the removal has a **higher** count and is refused *by a current binary*, and the count alone
**cannot distinguish** "this graph uses 2 types I removed → migrate the graph" from "this graph uses
2 types I don't have yet → update my binary." The old message assumed the latter and told the user to
`cargo build` — useless for the removal case, and doubly wrong for a `curl | sh` consumer with no
checkout (`req:frictionless-update`). **DONE this session — the message half:** the refusal now names
*both* recovery paths (update the binary; or migrate the graph — import a committed export into a
fresh graph, or export-with-the-old-binary → import-here, retired types dropped and named), and drops
the wrong single assumption. Verified: `--import` of an older-stamped export document is *not*
refused (it was the migration path used for the self-model this session), so the recipe works today.

**Still open (the real fix):** the count can't self-diagnose, so the message has to hedge. The
principled resolution is a **set-based stamp** — record *which* types the schema (or the graph
actually uses) carries, not just how many. Then the binary can say precisely "this graph uses edge
type `VALIDATES`, which this reflow2 retired — safe to migrate" vs "this graph uses `X`, which this
reflow2 has never heard of — you are behind," and the refusal becomes unambiguous without hedging.
Cheaper interim options to weigh first: (a) a `--migrate` / re-stamp path that drops retired types and
re-stamps in one step (today it is a manual export→import); (b) bump `schema_version` on any *removal*
so the direction is at least detectable from the version, not only the count; (c) the per-release
`upgrading-to-vX.md` for the edge-orthogonality cut carries the migration recipe, with a one-line
pointer in the consumer AGENTS.md "if reflow2 gets in your way" section (both deferred to when the
change is released — it is still `[Unreleased]`). Connects to BL-19 (the stamp), BL-51 (frictionless
update), and `req:survives-upgrade`.
> **Built 2026-07-24 (the set-based stamp).** `GraphStamp` gains `node_type_names` / `edge_type_names`
> (sorted, `serde(default)` so legacy count-only stamps still parse), populated from
> `schema.{node,edge}_types.keys()`. A `RETIRED_NODE_TYPES` / `RETIRED_EDGE_TYPES` registry
> (`&["VALIDATES","ENABLES"]`) lets the new `unreadable_by` partition the types a graph names but the
> binary lacks into *retired* (→ migrate) and *unknown* (→ update reflow2), naming each. Legacy
> count-only stamps fall back to the count check but get a sharpened message: an excess the retired
> types fully explain leads with migration. `provenance.rs` only (`art:provenance`, `chg:bl86`,
> reconciled design_updated); 6 new unit tests + 1 real-schema integration test (a graph naming
> `VALIDATES` is told to migrate, not to update). Full core suite + clippy + fmt green.
> **Interim options (a)/(b)/(c) from above are now moot** — the set-based stamp is unambiguous, so no
> `schema_version`-bump-on-removal or `--migrate` shortcut is needed; the per-release upgrade doc
> still helps a human but the binary no longer hedges.

**BL-87 · `import_graph` requires `document.stamp` but the published schema doesn't say so — DONE
2026-07-23** — *BL-83b adopt dogfood, 2026-07-23 (E.2). Size **S**, BL-57/BL-28 family. Fixed via
option (b), on Anthony's call: the stamp is now the sibling of `content_hash` — `GraphExport.stamp`
is `Option<GraphStamp>`, a stampless (hand-authored / third-party) document imports and the
`ImportReport` carries a `provenance_note` (loud, not silent; the upgrade-direction check can't run
on it). `import_graph` never gated on the stamp, so requiring it was pure deserialization friction.
`compare_designs`/`merge_designs` read a new `reflow2_version()` accessor (`"unstamped"` when absent).
The tool input schema is unchanged (still a free object), so no toolsnap churn; chosen over (a)
publish-the-shape-and-keep-it-required because refusing a stampless document contradicts the
content_hash precedent (`absence reported, never an error`). `chg:bl87`; 43 core+mcp suites green.* The first `import_graph`
call in the trial failed with a bare `missing field \`stamp\`` and no hint about the shape; the
adopter recovered the envelope by exporting the empty graph first. Verified: `GraphExport.stamp` is a
required field (no serde default), but the tool's published input schema declares `document` as a
bare `{"type": "object", "additionalProperties": true}` with no inner structure — so a client cannot
know `stamp` is required, exactly the under-typed-parameter shape BL-28 fixed for the `JsonValue`
params. Fix, either: (a) publish the `GraphExport` shape in the tool schema (name `stamp` / `nodes` /
`edges`, the BL-28 approach), or (b) default the stamp on import — accept a stampless document as
`Unstamped` rather than erroring, which is friendlier for a hand-authored or third-party document.
(a) keeps the "no silent acceptance" line; (b) trades it for ergonomics — a design call. The
toolsnap freezes the current under-specified schema, so fixing it updates a golden.

**BL-88 · reflow2's own CI gate and view renderer have no automated test** — *BL-83b adopt dogfood,
2026-07-23 (C.1, confirmed against the real repo). Size **S–M**.* `unverified_capability` fired on
`cap:ci-gate` and `cap:render-views` during the recovery, and it is true: `tools/reflow2_check.py`
(275 LOC — *the consumer CI coherence gate*, BL-66) and `tools/render_views.py` (928 LOC) have **no
test suite** in the tree (only `test_init.py` and `test_loop_nudge.py` exist) and **neither is
invoked in `ci.yml`**. `reflow2_check.py` was three-way verified by hand when BL-66 landed, but
nothing pins it since — a change could silently break the gate that is supposed to fail the build,
and reflow2's own gate is the one thing with no regression test. Ironic against the "unexamined is a
visible state" ethos, and doubly so because `render_views` is now a *confirmed intended feature*
(the author settled it in the dogfood, so it has a requirement but still no test). Add a hermetic
suite for each (a doctored-export-fails / clean-passes / missing-export-refuses trio for the gate,
mirroring its BL-66 manual check) and wire both into `ci.yml`. **PARTIAL 2026-07-23:** `reflow2_check`
is now wired into `ci.yml`'s `core` job as the design-coherence gate — run against reflow2's own
committed self-model on every push, so it is at least *exercised* in CI (and it immediately earned its
keep: it caught this session's `art:graph`/`art:provenance` drift). Still open: a hermetic *unit*
suite for the gate's own logic (the CI run only exercises the happy path on the real self-model), and
`render_views.py` remains wholly untested. This is reflow2 finally dogfooding its own `ci-gate` skill.
**DONE 2026-07-24:** both hermetic suites landed and wired. `tools/test_reflow2_check.py` (CI `full`
job, drives the real binary) pins the gate's exit-code contract on the doctored-fails / clean-passes /
missing-refuses trio + both drift shapes + no_baseline-is-a-note; `tools/test_render_views.py` (CI
`core` job, pure-Python file form) pins the projection/confession doctrine. **The gate suite caught a
real bug:** the gate only failed on reconcile kind `"missing"`, but reconcile emits `missing_artifact`
(severity high) — so a *vanished* registered artifact was silently a note, not a red build, despite the
gate's docstring promising "changed or vanished." Fixed. Also modeled `render_views.py` (`art:render-views`
→ `cap:report`, governed by `dec:views-are-projections`) and registered both suites as passing
`Verification`s (`ver:gate-suite`, `ver:views-suite`). `chg:bl88`; 13 new hermetic cases; gate + skill_lint
green, live==committed.

**BL-89 · Adopt-doctrine tweaks from the BL-83b dogfood — DONE 2026-07-23** — *2026-07-23 (B.1,
B.3, E.1). Size **S** each, batched. All three landed: (B.1) the adopt skill's granularity guidance
now keys off distinct contracts/capabilities not LOC (all three skill copies — kit, .claude, .grok —
synced); (B.3) `describe_schema {required_only: true}` returns only required properties and drops the
edge lists (new `describe_node_type_required` in core; toolsnap regenerated, +1 param); (E.1) the
`unreleased_component` detector expands the shipped set down `CONTAINS`, so a Release including a
subsystem covers its modules (the "assembly speaks through its children" rule again). 2 new tests
(unreleased-through-subsystem, required_only-is-compact); `chg:bl89`, 4 artifacts reconciled; 43
core+mcp suites + clippy + fmt + toolsnap + skill_lint green.* Minor refinements the largest-ever adopt run surfaced, none blocking: **(B.1)** the
scale-granularity heuristic ("~78 nodes for 110k LOC") keys off **LOC**, but reflow2 is ~34k LOC with
93 tools / 28 node types — *feature* density far above *line* density, so an honest coarse model
still lands near ~100 nodes. Granularity guidance should key off **distinct contracts / capabilities**,
which is what actually drives node count. **(B.3)** `describe_schema` returns a very large payload per
type, so the adopter fell back to reading `schema/*.yaml` for "what's required" — add a
`describe_schema {required_only: true}` compact mode. **(E.1)** wiring the operate layer spawned 11
`unreleased_component` gaps until every leaf was `INCLUDES`-wired into the Release; a Release that
`INCLUDES` a subsystem could optionally imply its `CONTAINS`-children ship, instead of one explicit
edge per leaf. All three are ergonomics, not correctness.

**BL-93 · Accepted structural defects can't be dispositioned, so `loop_status` never reads clean**
— *found 2026-07-24 running reflow2's own gaps/health analysis on itself (dogfood); observed on the
live self-model — `detect_defects` reports 6 warnings, all accepted, and `loop_status` counts every
one as outstanding.* Size **S–M**; a vocabulary + loop-computation decision, BL-74/90/91 family.

The asymmetry: reflow2 has `acknowledge_gap` — move a gap the user has judged fine out of
`detect_gaps` into `reviewed_gaps`, recorded as a Decision, re-opening if the affected nodes change.
There is **no equivalent for a `detect_defects` defect.** A structural defect the user has explicitly
accepted still fires as a warning on every run: reflow2's own 5 SPOFs carry `dec:*-spof-accepted`
governing decisions, and the `cap:fork-alternatives` disconnected community is a known BL-70 draft
marker — yet `loop_status.structural_defects` counts all 6 forever, so `loop_status.clean` is
**unreachable on any design that has a single legitimately-accepted SPOF.** reflow2's own loop
reports "6 structural defect(s) outstanding" on every check, permanently.

Why it matters (the BL-74 spine): a signal that never goes quiet gets tuned out — the exact skimming
failure BL-90/91 were built to prevent, one layer over. `loop_status` today conflates *"undecided
defect → act on it"* with *"decided-and-accepted defect → standing fact,"* and a permanently non-zero
count trains the reader to ignore the number, so a genuinely **new** defect no longer stands out. The
irony is on the record: the tool's own coherence loop can never report itself clean, for a reason it
gives the user no way to resolve.

Options to weigh:
- **(a) `acknowledge_defect`** — the defect sibling of `acknowledge_gap`: record why a defect is
  accepted (a Decision), drop it from the `loop_status` debt count into a reviewed-defects view, and
  re-open it if the affected nodes change. The clean vocabulary match — the reviewed-gap ledger
  already models exactly "kept, not suppressed, re-opens on change" (BL-84's `island_attached_by_containment`
  and the retired `ack:695` show the lifecycle works).
- **(b) `loop_status` nets out** defects whose affected nodes carry a `*-spof-accepted` (or
  equivalent) governing decision — no new tool, but couples the loop count to a decision-naming
  convention and only handles the SPOF case, not a generic accepted community/cycle.
- **(c) Split the count** — `detect_defects` stays fully verbose (every defect visible, honest), but
  `loop_status.structural_defects` reports `undecided` vs `accepted`, and `clean` keys on undecided
  only. The smallest change if the goal is to keep every defect loud while fixing only the loop's
  clean/debt signal.

Lean **(a)** or **(c)**. The requirement underneath, either way: *the coherence loop must let the
user distinguish a defect they have decided to live with from one they have not, so `loop_status` can
reach clean and a new defect actually stands out.* Connects to BL-74 (the loop fires on triggers, not
virtue), BL-90/91 (skimming / signal quality), and BL-84/BL-69 (the SPOF & community detector tuning
that leaves these accepted-but-real warnings). Per BL-85, this is a **new requirement to reflow2**,
not a defect against an existing one — capture it through the tool's own capture-intent when raised.

**BL-92 · The critical detect↔verify circular dependency — DONE 2026-07-23** — *the one critical
structural defect on reflow2's own self-model, dispositioned on Anthony's direction to fix it
(`dec:fnv1a-foundational`). Size **S**, BL-83a-surfaced.* The cycle was genuine but spurious:
`detect→verify` is a real domain dependency (gap detection reads `crate::verify::CapabilityVerification`),
but `verify→detect` existed ONLY because `verify` borrowed `crate::detect::fnv1a` to mint a
deterministic id — the FNV-1a hash homed in `detect.rs` since gap-id hashing first needed it, and
reached by eight modules. **Broke it by relocating `fnv1a` to `nodes.rs`** (the vocabulary/identity
leaf everything already sits above; minting a derived node's id is an identity concern, and the leaf
gains no coupling so no new cycle). This removed the cycle plus five more fnv1a-only false couplings
on `cmp:detect` (agent, artifact, drift, fielded, heal); `report` keeps its real `GapCandidate`
dependency, `detect→verify` stays. Self-model reconciled (−6 `DEPENDS_ON→cmp:detect`, +1
`cmp:agent→cmp:nodes`) to match the corrected source; the build script derives deps from source so a
rebuild reproduces it. Verified on the real self-model via `--analyse-only`: `detect_defects` now
**zero critical** (7 warnings — 5 accepted SPOFs + fork-alternatives + parked BL-91). Also
reconciled the artifact drift BL-84's `structure.rs`/`heal.rs` edits had left (`chg:bl84`). BREAK
over ACCEPT because the cycle was a homeless-utility artifact, not a real mutual dependency — see the
decision's rationale.

**BL-91 · A read reminds the agent of loop debt at the moment of attention (read-side loop_hint) —
DONE 2026-07-24** — *user idea, 2026-07-23, raised while reviewing the BL-90
nudge. Size **S–M**, BL-74 family. `req:read-surfaces-debt` ACCEPTED and `dec:read-hint-shape`
collapsed to **option C** on Anthony's word (2026-07-24): a read carries a loop-debt pointer ONLY
when `loop_status` would report real debt (never static-every-read — the BL-90 boilerplate
anti-pattern); it rides the **orientation reads** (graph_report / scan_nodes / search_design /
get_node) and fires **only when the owed-set changes** (fire-on-change), which also bounds the
per-read cost. Read-side sibling of the write tools' `loop_hint`. Build target: the MCP read tools'
result path (with_loop_hint-style, computed from loop_status, throttled on change). gap `e20d0909`
[0.60] stays the honest unbuilt state until it lands.* The write tools carry
a static `loop_hint` at the next loop step (`dec:loop-status-state-not-history`); reads carry
nothing. The idea: a read that returns while the loop is owed something surfaces it — the mid-session
trigger between SessionStart (fires once) and the Stop nudge (fires at the end), landing at the
agent's most frequent action. **Decision to make, two axes** (`dec:read-hint-shape`): (1) *shape* —
(A) do nothing, the three existing triggers suffice; (B) a static reminder on every read; (C) a
state-derived conditional hint that fires only when `loop_status` would report real debt. (2)
*scope/throttle* — which reads, and how often the debt is recomputed. Leaning **C**: (B) is the exact
anti-pattern the user rejected in BL-90 ("aggressive wording across ~90 tools cancels out at scale" —
boilerplate trains skimming), and reads differ from writes in the way that makes static wrong here —
a write always advances the loop so its static hint is always relevant, a read creates no debt so a
constant reminder is noise most of the time. C respects `dec:loop-status-state-not-history` (debt is
computed, never remembered) and `dec:anchored-first` (a real problem outranks a generic nudge); its
cost is coupling reads to a debt traversal, which axis 2's throttle bounds. Sibling of the write-side
`loop_hint`; extends BL-74, complements BL-90.
> **Built 2026-07-24 (option C, exactly as decided).** `graph_report`, `graph_report_markdown`,
> `scan_nodes`, `search_design`, `get_node` attach `loop_hint` only on real debt, fire-on-change.
> Axis 2's throttle is a service **write-generation counter** (`write_lock` bumps it; the 61 write
> handlers route through it): the owed-set can move only on a write, so within one generation the
> first orientation read computes `loop_status` once and later reads add nothing — the debt traversal
> the decision worried about is paid at most once per write, not per read. `cap:read-loop-hint`
> (SATISFIES `req:read-surfaces-debt`, ALLOCATED_TO `cmp:service`, REALIZED `art:service`, VERIFIED
> `ver:read-loop-hint` — new `tools.rs` cases); gap `e20d0909` closed, read-hint disconnected
> community dissolved. Surfaced + fixed a real bug of its own: `reflow2_check.py`/`reflow2_cli.py`/
> `smoke_mcp.py` unwrapped the `{count, items}` list envelope by exact key set, so the new
> `loop_hint` broke the unwrap (crashed the gate) — now presence-matched. `chg:bl91`; art:service
> (design_updated) + art:check (design_holds) reconciled; all gates green, live == committed 340n/927e.

**BL-90 · loop_nudge has a total-bypass blind spot: a session that never touches the graph is never
nudged — DONE 2026-07-23** — *user, 2026-07-23, the one survivor from a review of external "force
the agent to use tools" advice. Size **S**, BL-74 family. Built exactly to the fix shape below: a
second `PostToolUse` matcher (`Edit|Write|MultiEdit|NotebookEdit`) counts file edits, and the Stop
hook blocks **once** when a session edited ≥`REFLOW2_LOOP_NUDGE_EDIT_THRESHOLD` (default 3) files
and made zero reflow2 calls — any single reflow2 call, even a read, disarms it. Never reads the
graph; the two backstops are mutually exclusive (a graph write means reflow2 was touched). Closes
`req:nudge-covers-bypass` via `cap:loop-status` (the nudge `art:loop-nudge` already realizes it);
`ver:loop-nudge` covers it with 6 new cases. `chg:bl90` on the record; live == committed.* `tools/loop_nudge.py` arms only on reflow2 **write**
activity: the PostToolUse hook matches `mcp__reflow2__.*`, counts graph writes, and the Stop hook
blocks once when writes finish unchecked. A session that edits code while making **zero** reflow2
calls generates zero counted writes, so the Stop hook passes silently — the agent that ignores the
design brain entirely is exactly the one the nudge never sees. This is the bypass *upstream* of the
one BL-74 was built from (fleet agents kept *adding nodes* while the check→ask loop stopped); same
operational-load conditions, one step earlier. Fix shape: a second PostToolUse matcher on the
harness's file-write tools (`Edit|Write|MultiEdit|NotebookEdit`) counts code edits in a session with
no reflow2 engagement at all, and the Stop hook names that debt once ("N files edited, the graph was
never consulted — start with `loop_status`; impact-check before further edits, link-artifacts
after"). Same contract as the existing nudge: blocks once, never twice, never reads the graph (the
single-writer lock constraint stands — which also means the hook *cannot* know which files are
design-relevant). Two cautions on the record, both from the review that raised this: **(1) false
positives** — a typo fix or a docs pass shouldn't arm it; since the hook can't consult the graph for
scope, start blunt: a count threshold (`REFLOW2_LOOP_NUDGE_EDIT_THRESHOLD`, default ~3) and the
once-only rule bound the annoyance. **(2) ritual compliance** — a hard gate trains an agent to fire
a token `loop_status` to silence it, so this stays a *nudge that names what is owed*, never a wall;
the honest backstop for a session that bypasses anyway is the reconcile layer (BL-66's CI gate
catches drift on **registered** artifacts at commit time — but a file *never registered* is
invisible to it, which is precisely why the session hook is the right layer for this). Rejected from
the same review, for the record: blanket MUST-language (loses to urgency; REFLOW2.md's own
trigger-not-virtue doctrine), `tool_choice` forcing (a host-side API knob no MCP server can reach —
hooks are the reachable enforcement layer), aggressive wording across ~90 tool descriptions (cancels
out at that scale and distorts deferred-tool retrieval), and transcript few-shots in the server
instructions (the skills are already the load-on-demand few-shot layer). Connects to BL-74 (the
ladder this extends) and BL-88 (the gate that backstops it).

**BL-72 · Namespaced schema packs — a domain vocabulary composes, it doesn't fork** — *from
the AT-proto comparison (Lexicon NSIDs), 2026-07-21. Size **M**; concept until a real second
vocabulary wants in.* Lexicon namespaces schemas reverse-DNS (`app.bsky.feed.post`) so
organizations extend the vocabulary without collisions, under a published-constraints-never-
change discipline (breaking = new name). reflow2's ten YAML domains already merge through
`Schema::from_multiple_yamls`, so the composition mechanism exists; what's missing is the
naming discipline and a pack convention — a UAF/TRL pack, a DoDAF viewpoint pack, an org's
own types under `org.<name>.*` beside the core 27, shippable and installable without touching
core. This is the reflow-v3 "framework packs" heritage idea with a proven governance model
attached. Prerequisite thinking: how the kit installs a pack, how `describe_schema` reports
provenance of a type, and what the CI gate does with types it doesn't know. Connects to
[BL-68] (readiness vocabularies are the first obvious pack) and the org-scale thread above.

**BL-73 · Verification at component granularity must be expressible, honestly — DONE
2026-07-22** — *first extensive field trial (the user's own StoryFlow fleet, another machine,
2026-07-21, docs/trials-private/2026-07-21-storyflow-fleet-improvement-log.md). Size
**S–M**.* User decided all three axes (`dec:component-verified-computed`): a **computed third
state** — `component_verified`, derived at read time from a passing `Verification` on an
allocated component, never written as manufactured capability-level edges; the coverage line
reports it as its own clause; and detect folds the N per-capability alarms into **one
`component_granularity_verification` gap per carrying component** at 0.35, listing the riding
capabilities, acknowledgeable once. `status_contradiction` accepts component-granularity
proof; `loop_status` counts it as proven; a failing suite carries nothing. The write side
needed nothing (`VERIFIES` → `*` all along, with an unused `coverage` enum — recurring-lesson
shaped: the read side was blind, not the vocabulary missing). The adopt skill now teaches
registering each real suite on its Component. 7-case suite replays the trial's exact shape;
the 20-gaps-21-acknowledges pile is now a handful of one-time questions. The remaining depth
(per-capability `VERIFIES` where behaviour deserves its own proof) is exactly what the
residual gap asks for, per component, once.* A brownfield adopt of a system with real coverage
(per-service unit suites + a 137-file integration suite) read as **"0/20 capabilities
verified"**, and recording the honest state cost **21 near-identical acknowledge/decision
writes**. Two defects in one: the *write side* has no way to say "verified at component
granularity" in one move, and the *report* renders that state as indistinguishable from
untested. Field-suggested shapes, decision the user's: (a) a `Verification` on a Component
**cascades** (as a weaker, labelled claim) to the capabilities that component realizes;
(b) a bulk attest operation; (c) `detect_gaps` folds the N per-capability gaps into one
"verified only at component granularity" note when the owning component carries a passing
`Verification`. Whatever is chosen, the coverage line must say the true thing: "verified at
component granularity" is neither "verified" nor "unverified" and deserves its own word.

**BL-74 · The loop fires on triggers, not virtue — adoption-critical** — *the most important
finding of the first extensive field trial (same log — the user's own fleet), self-reported by
the driving agent and caught by its user.* Size **M**. **Rungs c+b DONE 2026-07-21**
(`dec:loop-status-state-not-history`): `loop_status` is live — one cheap call returning the
loop's outstanding debt as an ordered to-do list, computed from **state, never run history**
(the core takes no clock and looking-is-not-writing is doctrine, so "no detect_gaps since
Tuesday" is not an honest computation — "3 anchored gaps never put to the user" is, and it's
the actionable one); phase nudges excluded (guidance, not debt). The five capture/structural
write tools carry a static `loop_hint` in their own results. Skills updated (capture-intent
step 6, detect-and-ask pulse-check). **Rung a DONE 2026-07-22 — CLOSING BL-74**: the kit
ships `tools/loop_nudge.py`, one stdlib script wired to three harness events (SessionStart
orientation, PostToolUse write-counting with loop-check reset, Stop blocking **once** with
what to run). It honours the lock constraint by never reading the graph — the hook counts
events, `loop_status` answers what is owed in-session — and honours the never-hostage rule
via `stop_hook_active`. Snippet in the kit AGENTS.md step 0a (which also absorbs the
cold-start orientation note), hermetic suite (`test_loop_nudge.py`, 9 cases) in CI, ships in
the kit tarball. The ladder is complete: hints where results land (b), the cheap computation
(c), the trigger that fires it (a). What remains is *evidence*: the fleet trial that raised
this should run with the hook armed and report whether the loop actually stays alive under
load — that verdict belongs in the trial log, not here. Told to "use the reflow2 skills extensively," the agent
under multi-hour operational load kept the graph's *bookkeeping* current via raw tools
(`add_capability`, `link_artifact`, …) but **dropped the loop skills** (`detect-and-ask`,
`check-health`, `impact-check`) — capture continued, the capture→detect→ask→decide loop
silently stopped. Root causes, verbatim from the field: the tools give the result without the
discipline; nothing *fires* the skills (their compaction hook works because it fires on an
event — "under load, a mood loses to whatever has a trigger"); skill round-trips cost context
mid-flow; and the lock concentrates all of it on the busiest session. Fixes, best first, and
they compose as one ladder: **(c→b→a)** — build a cheap **`loop_status`** core op + tool
("3 captures and 2 structural edits since the last health check; 1 realized capability with no
Verification; 1 open question unanswered" — the un-run loop steps, computed from what the
graph already records); thread **next-step nudges into the write tools' own results**
(`add_capability` → "run detect_gaps when you finish capturing"; zero extra round-trip to see
it); then ship the **kit hook recipe** that fires `loop_status` on the client's own trigger
(SessionStart/Stop-hook), which is (a) — triggers beat exhortation, and the enabler is (c).
The trial's meta-lesson is the item's bar: *a design-discipline tool that depends on being
remembered will lose to operational urgency every time.* Also fold in: session cold-start
warm-up (their MCP tools were deferred at first use) belongs to the same hook recipe, not to
the server.

**BL-75 · A Requirement's certainty is a state, not a caveat — DONE 2026-07-22** — *same
trial log; the last of the trio it raised.* User decided all three axes
(`dec:certainty-derived`), and the backlog's own hunch was right: **derived, not a third
axis** — status × provenance already spans the space once the doctrine is settled. The
mapping: `accepted`/`met` → user-confirmed; `proposed`+`inferred` → recovered, awaiting the
user; `proposed`+`authored` → asserted, awaiting the user; `deferred`/`dropped` → settled
out (their word too). The doctrine with teeth: **every move off `proposed` records the
USER's word** — an agent captures at proposed and only the user's answer moves it (BL-12
sketch idea 2's example made real, culturally for now; the mechanical half stays with
BL-41). `graph_report`'s snapshot renders the breakdown ("Requirement certainty: 15
user-confirmed · 2 asserted…"); where-am-i reads the line instead of caveating;
capture-intent and adopt state the rule at the point of capture; the
`set_requirement_status` description says it to every session ("promoting it yourself
forges their signature"). No schema change — the second item this week (BL-73) where the
vocabulary was sufficient and the read side was blind.

**BL-44 · Node-level claims — parallel work on one design** — *user, 2026-07-20. Concept-only by
their own framing; the details are the work.*

The idea in the user's words: an agent **claims the nodes in the graph it is working**. A task
strictly internal to a node claims only that node; work on an interface — an edge between two or
more nodes — claims **all affected nodes**. Anyone else may work any unclaimed node, and
"theoretically, their work should not interfere."

What this is: [COORD.md](../COORD.md)'s claim board moved *into the graph*, at node granularity.
COORD coordinates two humans over files, socially ("commit the claim line before the work");
this coordinates N agents over design nodes, and the graph can compute what COORD relies on
people to do. It is BL-12's first concrete write-side mechanism sketch, and it composes with the
consensus notes above rather than replacing them.

Tensions to resolve before building — the flesh-out list, so whoever picks this up starts where
the thinking stopped:

1. **Claims partition intent, not I/O.** The store is single-writer (BL-12), so simultaneous
   *writes* still take turns whatever the claims say. The claim layer is what would make
   fast-alternating writers *safe*; whether the alternation itself is acceptable (one server,
   short turns) or needs read-only secondaries is still BL-12's storage question, unchanged.
2. **"Should not interfere" is a claim PROPAGATE exists to check.** A change internal to a node
   can still ripple — blast radius is `propagate_change`'s whole subject, and
   `surprising_connections` exists because the graph's edges under-state real coupling. So a
   node claim may need to be *sized* by the blast radius at claim time (claim the propagation
   frontier, not the node), or at least validated against it — and the interference the graph
   has not drawn an edge for yet is precisely what it cannot protect anyone from. The
   edge-work rule (claim all endpoint nodes) is the user's own instance of this at depth 1.
3. **A claim needs an identity to belong to, and the graph has none.** No Person/Agent node
   type exists. Whatever carries it — a `Claim` node with `CLAIMS` edges (first-class,
   axis-Z-recordable, expirable) over a bare property on the claimed node (invisible to
   history) — this introduces *who* into the schema for the first time, which is the same
   missing piece BL-12's sketch idea 2 (who may assert what) and BL-41's mechanical half both
   need. One identity mechanism should serve all three; building it for claims alone would be
   the recurring lesson in reverse.
4. **Stale claims.** COORD's rule is "a week with no commits, anyone may take it." The graph
   equivalent wants to be *computable*: a claim that records the epoch or export-hash it was
   made against (sketch idea 1 above) goes stale by fact, not by calendar — the graph moved
   under it, or its holder stopped moving the graph.
5. **Advisory or enforced?** Refusing a write to a node another agent claims is tempting and is
   also the graph seating itself in the judgment chair. The repo's own doctrine
   (`dec:report-dont-judge`, disagreement routed to humans as questions) argues for advisory
   first: the write lands, the claim violation is *reported* — loudly, to both holders — and
   enforcement waits for evidence that reporting wasn't enough.

Prior art to mine: COORD.md itself (claim-before-work, one line per claim, merge=union — each
has a graph analogue), and the ophyd/storyflow adoption discipline of marking unexplored
frontier (a claim is "mine, in progress"; a frontier mark is "nobody's, not yet real" — the two
partitions should not be confused, and both quiet the detectors differently).

Size **M** for an advisory claim layer once the identity decision is made; the decisions —
identity (3), granularity vs blast radius (2), advisory-first (5) — are the real content, and
they want a session with the user, not a patch.

**2026-07-21 addendum (user)** — two sharpenings from a second pass on the same idea. **(a)** The
claim unit may be a **cluster**, not a node: "Alex is working that cluster — Bobby is working that
node." The regions are computable with machinery the graph already runs — a blast radius
(`propagate_from`) or a community (the allocation clustering) — which promotes tension 2 from
*validation* to *granularity*: claim the island, not the node list. **(b)** A claim licenses
**design authority**, not just edit intent: the holder "goes off and makes design choices for
their area", so Decisions recorded inside a claimed cluster belong to its holder — identity
(tension 3) joined to BL-12 sketch 2's who-may-assert-what, applied to a region instead of a
claim type. And a checkout that *explores an option* rather than progressing the baseline is
[BL-70](#bigger-threads)'s fork wearing work clothes — the claim layer and the alternatives
layer likely want the same scoping primitive, and should not be built twice.

**BL-45 · System-of-systems: external dependencies between reflow2 projects** — *user,
2026-07-20. Explicitly a thought exercise; the mechanics are the open question.*

The idea in the user's words, compressed: a reflow2 project should be able to declare
**external dependencies** on *other reflow2 projects* — possibly in different repos, owned by
different people or organizations — the way `pyproject.toml`/`pixi.toml` declare software
dependencies (and noting that in non-software domains the analogue is not obvious). One
project "interfaces" with another; groups focus internally on their own project but link
outward, building **system-of-systems architectures** — and when two or more systems interface
through the same contract, *standards between systems can come into existence*. A project
would publish an **external-facing interface spec** — "something synonymous to OpenAPI
specs/docs".

This is the package-manager shape applied to the oldest SE artifact there is: the published
surface is an **ICD**, and the graph already holds most of the vocabulary —

| Piece | Already exists | The SoS gap |
|---|---|---|
| The contract itself | `Interface` nodes; `SPECIFIES` (an OpenAPI/protobuf artifact IS the machine-readable contract, with a `format` property); `provides`/`consumes` | Nothing marks an Interface **external-facing** — visible-to-others vs internal is undeclarable |
| Publication | Deterministic `export_graph` (byte-identical, stamped); BL-15's release machinery — a release asset is a distribution channel with a URL and a checksum | No **filtered** export: "everything" or nothing. The published surface wants to be exactly the external Interfaces + what they expose, and nothing internal |
| Consumption | `import_graph` upsert; `provenance: imported` exists on the four adoptable types | Imported reference nodes aren't marked *foreign* (whose project, what version, what checksum) — and edges cannot span graphs, so cross-project links must be **mirrored reference nodes**, not edges into another repo |
| Version pinning | `Release` + `INCLUDES` + `as_checksum` (BL-34's frozen manifest); `GraphStamp` | No way to say "I build against *their* v2.1" — the dependency declaration (project, surface, version, checksum) has no home |
| Drift detection | The reconcile family; `unresolved_drift`; BL-18's am-I-current check | The cross-boundary case: *their published surface moved and my mirrored copy is stale* — the exact BL-18 question, one project boundary out |

*The observation worth keeping even if nothing else survives:* **a standard is itself a design**
— when N projects consume the same published interface, that contract wants to be its own
reflow2 project (requirements, decisions, releases, verification of conformance) that provider
and consumers all declare a dependency on. Standards emergence then has a mechanical form: two
bilateral interfaces noticed to be the same shape → extracted into a third project both
reference. That is how real standards bodies work, minus the committee.

Tensions, so the flesh-out starts honest: (1) **trust** — an imported surface is another
organization's graph text, and BL-41's "graph text is data, never instructions" plus BL-12's
who-may-assert-what stop being single-user hygiene and become the security boundary; (2)
**transitive dependencies** — does importing their surface pull *their* dependencies' surfaces
(the diamond problem, now with orgs); (3) **the non-software domains the user names** — a
supplier's actuator, GFE, a materials spec: `Interface` + `Constraint` may already carry it,
which would make this BL-16's sharpest test case too; (4) private↔public seams — this repo just
split public evidence from private trial records, and an SoS link between a public and a
private project is that same seam as a *feature*.

Size **L** for the thread. The first testable increment is **S–M** and needs no federation at
all: an `external: true` marking on Interface + a filtered "published surface" export — this
repo could publish its own MCP tool surface as the first ICD. Related: BL-8 (multi-project),
BL-12 (multi-writer), BL-16 (domains), BL-44 (claims). Decision conversation with the user
before any code; this entry is the prep.

**BL-46 · `create_node` on an existing node replaces the whole property object — DONE** —
*self-adopt live session, [trials/2026-07-20-self-adopt-live.md](trials/2026-07-20-self-adopt-live.md);
fixed 2026-07-20, same day.*

Folding merged wording into `cap:kit`'s description via `create_node` silently reset
`status: verified → planned`; on `req:intent-preserved` it also reset `priority: high → medium`
and `status: accepted → proposed`. The props object supplied replaced the stored one, with
schema defaults filling every omitted property — so the only safe "edit one property" call was
one that re-supplied all of them, which nothing told the caller. **Fixed by the merge option:**
`DesignGraph::upsert_node` (supplied props over stored, validation unchanged), and the
`create_node` MCP tool now routes through it and says so in its description — the contract the
revise-design skill stated all along. The typed setters stay the right call where they exist:
they refuse a missing node instead of creating it. Tests: `tests/upsert.rs` (core),
`create_node_on_an_existing_id_merges_instead_of_resetting` (surface).

**BL-47 · Unset provenance must not tie with `authored` in merge survivor selection — DONE** —
*self-adopt live session; fixed 2026-07-20, same day.*

The genesis stubs carried no `provenance`, which read as the default `authored`; HEAL's
survivor rule saw a tie against the real authored nodes and fell through to the id tiebreak,
proposing to keep stub `cap:install` and **delete the authored, verified `cap:kit`** (same for
`cap:artifacts` over `cap:reconcile-built`). Caught only because the proposal was reviewed
before apply. **Fixed:** `provenance_rank` now takes an `Option` and slots `None` (a
pre-provenance vintage — defaults materialize on create, so nothing newer lacks the property)
strictly between explicit `authored` and everything else. A vintage pair still ties and falls
to the id, so pre-provenance graphs are unchanged; an explicit `authored` now beats its
vintage twin outright, and the machine provenances still never delete probable human words.
The related clobber is fixed too: **a colliding edge is no longer re-pointed** — the
survivor's edge and its properties are kept, the drop reported in `discarded` (previously the
removed node's `action: removed` overwrote the survivor's `modified` after being "reported").
Semantics pinned on `dec:merge-survivor-provenance`; the unset slot is unit-pinned at the
`provenance_rank` seam because today's API cannot build a vintage node.

**BL-48 · `graph_report_markdown` returns malformed `structuredContent` — DONE 2026-07-20** —
*self-adopt live session.* Size **S**.

From Claude Code the tool failed client-side schema validation: `structuredContent` arrived as a
string where the MCP result contract wants a record — the fifteenth recurring-lesson instance
(the capability exists and one harness cannot reach it), and the same response-side shape as the
original array `structuredContent` bug. **Fixed at both layers**: the tool now returns the
report as plain text content with no `structuredContent` (a prose document has no structure to
declare — `ok_markdown`), and `ok_json` — the choke point every other tool returns through —
wraps any remaining scalar as `{value}` so no future tool can leak one. `smoke_mcp.py` now
asserts the envelope on **every** call it makes (`structuredContent`, when present, must be an
object) and fetches the Markdown report over the real wire — the check that would have caught
this the day it shipped. Reproduced live in this session before fixing (the tool failed as the
first call of a where-am-i pass).

**BL-49 · Unbounded read-tool results overflow the agent boundary — DONE 2026-07-20** —
*self-adopt live session.* Size **M**.

`propagate_change` returned 70k chars (142 impacted nodes), `export_graph` 93k — both
overflowed the tool-result budget and were readable only through the harness's spill-to-file
fallback plus `jq`. A blast radius nobody can read inside the loop is a blast radius that
doesn't get read. **Both propagate tools now answer with a summary by default** — counts by
distance (every impacted node in a band, `total_impacted` checked against the full walk in
tests), the distance-1 ring with the edge that reached each node, risk crossings at any
distance, `unknown_seeds`/`truncated_beyond_depth` carried through — with the full per-node
`via` dump behind `full: true`. The summary is computed in core (`BlastRadius::summarize`), not
shaped at the surface. **`export_graph` takes an optional `path`**: it writes the document as
deterministic sorted-key JSON (byte-identical on an unchanged graph — pinned in tests) and
returns a `{path, bytes, nodes, edges, stamp}` receipt instead of the payload. The impact-check
skill teaches the summary-first contract. `max_nodes` was not added: the summary removes the
size driver (hop chains), and a cap on the ring would be a silent truncation with extra steps.

**BL-50 · Tool-boundary paper cuts from the self-adopt live session — DONE 2026-07-20** —
grouped, each **S**.

(1) `DUPLICATES.confidence: 1` was rejected with "expected Float, got int" — every LLM writes
`1`, JSON has one number type. **Integer literals now widen losslessly to floats at the core
write seam** (`create_node`/`create_edge`, schema-aware, so it covers every surface), and only
there: a non-exact integer still fails loud, the range check still applies after widening, and
a property the schema does not declare float is never touched. The foundation stays pinned —
the coercion is reflow2's, not a validator change. (2) **`add_change_event` takes an
`affected` list** and draws its CHANGED edges in the same call — validated whole before
anything is written (storage accepts dangling edges, so the tool's check is the only one), so
a bad entry refuses the event rather than leaving a partial record; the result names each edge
and its action. And **`describe_schema` now counts half-exact matches**: CHANGED names its
from-side and is open on the to-side *by design*, and the note now calls such an edge the
modelled fit instead of lumping it with both-sides wildcards. Bonus from the same envelope
discipline: `delete_node`/`delete_edge` returned bare booleans (the BL-48 defect shape); they
now return `{deleted}`. (3) The kit's **SessionStart hook recipe** is documented in
getting-started/AGENTS.md step 0a — the where-am-i ritual lands in the session's context at
startup on harnesses with hooks; the rest keep the written convention. Not auto-installed:
writing into a consumer's `settings.json` is not a thing the installer gets to do.

**BL-51 · Frictionless install and update — the Claude Code model** — *user, 2026-07-20.*
Size **S + M**, priority deliberately low ("may not be important right now... can we get there
eventually").

The user named the target explicitly: Claude Code installs with one `curl -fsSL <url> | bash`
from a stable public URL, updates with a single `claude update`, and ships frequent, very minor
versions so updating is routine rather than an event. "I like frequent and very minor updates
(it is very iterative)." Recorded as `req:frictionless-update` (proposed, low), partially
satisfied by the install capability with the delta on the edge's evidence. The concrete gaps,
now that the repo is public and BL-15's machinery exists:

- **(S) A stable public one-liner.** `install.sh` is already checksum-verified and pulls
  published binaries; what is missing is the documented, tested
  `curl -fsSL https://raw.githubusercontent.com/sligara7/reflow2/main/install.sh | bash`
  path (or a short redirect domain later) in SETUP.md and the README, exercised by a probe.
- **(M) One-word update.** Today updating is "rerun the installer" or
  `reflow2_init.py <project>` in place. Wants `reflow2-mcp update` (or a thin `reflow2`
  wrapper verb) that re-runs the checksum-verified fetch and swaps binary + kit together —
  the staleness detection half already exists (`served_by`/BL-32, `KIT_VERSION.json`/BL-18),
  so this is the acting half.
- **(cadence, no code) Frequent minor cuts.** release.yml makes a cut cheap; the practice is
  cutting often and keeping CHANGELOG sections small. Nothing to build — but the one-word
  update is what makes a frequent cadence tolerable to consumers, so it gates the practice.
- **(2026-07-22 addendum, from the github-mcp-server read)** Two cheap reach-wideners when
  this thread is picked up: an MCP-registry manifest (`server.json`) so clients discover
  reflow2 the way they discover github-mcp-server, and possibly a Docker image (their
  recommended end-user path). Both distribution polish, neither urgent.

**BL-52 · CI enforces the gates; skills get contract lint — DONE 2026-07-20** — *user asked
"do we have legitimate CI tests for the skills?"; the answer was that there was no CI at all.*
Size **S + M**.

`.github/workflows/ci.yml` (the repo's first CI): a fast core job — core tests on the
in-memory backend, clippy `-D warnings` both crates, fmt, schema validation, the installer
suite, skill lint — and a full job that pays the cached RocksDB build for `cargo test
--workspace`, then drives the REAL binary: `smoke_mcp`, `phase_trial` (13/13 gate),
`model_the_loop`, `coherent_erosion_trial`. `erosion_trial` deliberately excluded (non-zero by
design until the ledger-judgement decision changes). `tools/skill_lint.py` checks what a skill
has that IS mechanically checkable — its contract with the surface: every backtick tool name
resolves against the served `#[tool]` set (with a committed, both-ways-enforced allowlist for
field/gap/enum terms, so a tool rename leaving prose behind fails loudly and the list cannot
rot), mirrors byte-identical to `getting-started/skills/` (the recurring "stale mirrors"
chore, now a gate), frontmatter valid, and BL-41's standing rule present in all 11 skills.
**Deliberately NOT built: LLM-driven skill evals** — a synthetic eval is another client we
write (the three-agreeing-clients lesson); semantic skill quality stays evidenced by real-use
trials per sharpening.md. Verified: all checks green on the current tree, negative test
confirms a bogus tool ref fails with exit 1, and the first live CI run on GitHub is the
end-to-end proof.

**BL-53 · A self-loop DUPLICATES edge makes HEAL delete the node — DONE 2026-07-21** —
*deep review (verified in source).* Size **S**, was **critical**. Fixed: equal endpoints
are refused in `merge_op_for` with a reason naming delete_edge as the correction — one
guard covers propose and apply, which both derive through it; regression test pins the
node's survival end to end.
`merge_op_for` (heal.rs) guards unresolvable endpoints and cross-type merges but not
`keep == remove`. `x DUPLICATES x` is schema-valid (`*→*`); the merge repoints nothing
(every edge "already points at the survivor") and then `delete_node(x)` removes the survivor
and all its edges, while the report says applied/verified. Merges have no snapshot and no
undo. Fix: refuse equal endpoints in `merge_op_for` (covers propose AND apply, which both
derive through it) + a regression test.

**BL-54 · The installer can destroy user content and die mid-run — DONE 2026-07-21** —
*deep review.* Size **M**. All four fixed: install now records a per-file sha256 manifest
in the stamp; ownership is proven by hash (edited kit files are LEFT ALONE with a report,
never overwritten; delete the file to accept the kit copy); the sidecar obeys the same
rule; files the kit no longer ships are pruned only when untouched; non-dict server values
report left-alone instead of crashing, and --check agrees with the run. Pre-manifest
installs keep the old heading heuristic for exactly one update, then the manifest closes
the window. Three regression tests. Four related defects in `reflow2_init.py`: (a) kit-file ownership is judged by
first-heading match, so a consumer's edits to an installed AGENTS.md or skill are clobbered
on update and reported as a routine refresh; (b) the `REFLOW2.md` sidecar is itself written
with no ownership check; (c) files removed from the kit are never pruned downstream, so
stale skills load forever; (d) a non-dict `mcpServers` value raises AttributeError mid-run,
leaving a partial install — and `--check` promises a write the real run refuses. Fix: a
per-file hash manifest recorded in the install stamp — "ours to refresh" = hash matches what
we installed; anything else sidecars with a report; the manifest also enables pruning; plus
the type-check `--check` already has.

**BL-55 · First-contact integrity: install.sh and the release flow — DONE 2026-07-21** —
*deep review (mechanism verified live).* Size **S + S**. Fixed: `try_download` returns
instead of exiting, so a missing checksums.txt reaches the honest-skip message; a binary
that cannot execute now fails loudly with the build-from-source recipe instead of printing
success; release.yml creates a draft, uploads, asserts all five assets present, then
publishes — a partial upload can no longer become `releases/latest`. (a) `install.sh`: a missing
`checksums.txt` silently kills the whole install — `download()`'s `fail` exits the script
even inside an `if`, and the call site's `2>/dev/null` swallows the message; the "checksums
NOT verified" honest-skip branch is unreachable. Also a binary that cannot execute still
prints "installed:". (b) `release.yml` creates the release live before uploading assets, so
a partial upload leaves `releases/latest` with checksums and no binaries. Fix: a
non-exiting `try_download` for the optional asset + a loud warn when `--version` fails;
draft → upload → assert four assets → publish.

**BL-56 · Destructive and leaky defaults in the test harnesses** — *deep review.* Size
**S + S**. **(a) DONE 2026-07-21**: `--graph-path` now refuses an existing directory
unless `--wipe` is passed. (b) orphaned servers + undrained stderr pipe still open. (a) `smoke_mcp.py --graph-path` rmtree's whatever directory it is given, before
any prompt — pointing it at a live `.reflow2/graph` destroys a real design. Wants
refuse-unless-`--wipe`. (b) On any mid-run failure the spawned servers are orphaned
(no try/finally around `Server`), and stderr is a never-drained PIPE that can deadlock the
test under a warn-storm; all four trial harnesses inherit both via the shared class. Wants a
context-manager Server that drains stderr and kills the child.

**BL-57 · Tool-boundary honesty batch — DONE 2026-07-21** — *deep review.* Size **M**. All
seven fixed: (a) `dyno_err` is variant-aware at the one choke point — caller-shaped errors
(NodeNotFound/Unknown*/Validation/EdgeValidation/InvalidEdge/InvalidKeySegment/EdgeNotFound)
→ invalid_params, genuine faults → internal_error; ~60 tools stop blaming the server for a
caller's typo. (b) Every request struct (65) carries `deny_unknown_fields`, so a typo'd
optional param is rejected — schemars now publishes additionalProperties:false, and a smoke
check asserts none regress; it immediately caught a real latent bug (the smoke suite passed
`at` to reconcile_artifacts, silently ignored — the field is `detected_at`). (c) export_graph
refuses to overwrite an existing file without `overwrite:true`, uses invalid_params for an
unwritable caller path, and reports the canonicalized path. (d) The serve path now gets
`explain_open_failure`, so the everyday two-session lock collision reads plainly. (e) get_node
returns one named `{node: <obj|null>}` shape both ways (was bare-object vs {value:null}) —
strengthening smoke checks that were previously always-true. (f) resolved by category:
"remove-if-present" tools (delete_*, both withdraws) report a boolean — withdraw_gap_ack
aligned from `was_reviewed` to `withdrawn`; answer_question correctly errors (a silent
{answered:false} would be the drop the project forbids), now documented in its description. (a) `dyno_err` maps
every core error to `internal_error`; ~60 of 78 tools report caller typos as server faults —
make it variant-aware at the one choke point (`NodeNotFound`/`Unknown*`/`Validation`/
`InvalidEdge` → invalid_params). (b) No request struct declares `deny_unknown_fields`, so a
typo'd optional param (`ful`, `record_events`, `path`) is silently swallowed and the tool
quietly does something else — add it everywhere, and a smoke check that every published
inputSchema carries `additionalProperties: false`. (c) `export_graph path:` writes/overwrites
any path with no guard — require `overwrite: true` or refuse non-export targets. (d) The
serve path bypasses `explain_open_failure`, so the everyday two-session lock collision gets a
raw RocksDB error. (e) `get_node` absent returns `{value: null}` vs a bare object when
present — one named shape both ways. (f) Sibling tools disagree on missing records
(error vs `{withdrawn:false}` vs `{was_reviewed:false}`) — pick the boolean-report style.
(g) `parse_enum` and the ~17 typed edge tools reject without naming what would have worked.

**BL-58 · Core silent-failure batch — DONE 2026-07-21** — *deep review.* Size **M–L** (each
piece S). All twelve items fixed with tests: (a) ingest re-ingest merges via `upsert_node`
instead of resetting; (b) snapshots serialize sorted (BTreeMap) for byte-stable exports;
(c) `propagate_change` errors on a missing event instead of returning empty; (d) `apply_heal`
is one atomic batch across all operations (merge_nodes made batch-free); (e) swallowed
edge-creation errors in acknowledge_gap / record_asked_question / ingest provenance /
ensure_epoch / fielded now surface; (f) budget rejects non-finite contributions at the write
seam and reports a provable overrun instead of Incomplete (max_by uses total_cmp);
(g) integer widening rejects the i64::MAX saturation edge (bound at 2^53); (h)
`truncated_beyond_depth` documented honestly as the one-hop frontier lower bound; (i) drift
skips the dangling DEPENDS_ON for undocumented additions; (j) missing-intermediate gaps get
distinct ids per producing edge (relation folded into the hash); (k) a reused ingest
fragment_id is refused up front; (l) node_type_index scans in sorted order. Old body:
(a) ingest matched-evolved uses `create_node` replace — the BL-46 reset failure, still live;
route through `upsert_node` merge. (b) `snapshot_node` serializes a HashMap — snapshot bytes
are process-random, breaking byte-identical exports of identical history; BTreeMap it.
(c) `propagate_change` on a nonexistent ChangeEvent returns an empty radius
indistinguishable from "impacts nothing" — check existence. (d) `apply_heal` batches per-op,
so a mid-proposal failure commits earlier merges while the error implies nothing happened —
one batch, or a partial-application report. (e) `let _ =`/`.is_ok()` swallow edge-creation
failures in `acknowledge_gap`, `record_asked_question`, ingest provenance edges, and
`ensure_epoch` treats a read *error* as "exists" — swallow only already-exists.
(f) budget: a provable Exceeded is masked as Incomplete when any contribution is unstated;
NaN contributions can panic `max_by` — reject non-finite at the write seam and decide the
provable side first. (g) `widen_ints_for_float_props` i64::MAX saturation edge.
(h) `truncated_beyond_depth` counts one ring, docs claim all — make the number or the doc
honest. (i) drift's `undocumented_addition` writes a dangling DEPENDS_ON to a node that
doesn't exist. (j) duplicate gap ids when CONTAINS and DEPENDS_ON level-skips share a pair —
fold edge type into the hash. (k) `IngestOptions::default()` reuses `frag:ingest`, letting a
second run overwrite the first's snapshots — make fragment_id required. (l) sort
`node_type_index` type order; surface id collisions.

**BL-59 · Analysis-pass efficiency at adopt scale** — *deep review.* Size **M**. The SPOF
check rebuilds the full design network (with a whole-graph type scan) twice per articulation
candidate and recomputes the invariant baseline per candidate; `graph_report` runs detectors
redundantly (`dimension_drifts` twice); every `propagate_from` recomputes betweenness
centrality (O(V·E)). Storyflow-scale adopt (2,643 files) is where this bites. Fix: one
`AnalysisContext { node_type_index, design_network }` threaded through a detect pass;
centrality lazy or cached on a mutation counter. Also: paging (`limit` echoed in result) on
`scan_nodes` / `detect_gaps` / `confirmation_ledger`, the BL-49 convention extended.

**BL-60 · Docs truth pass — DONE 2026-07-21** — *deep review.* Size **M** (writing only),
was **critical for new readers**. Fixed across AGENTS.md (Current state rewritten to v0.5.0
reality — surface shipped, full module list, GENESIS built, two crates, v0.10.0 pin, 54
edges, INCLUDES in the traceability set), README (27 types + Question, layout tree shows the
real repo, path fix), requirements-coverage (IS-5/6/7 → ✅, preamble + deferral list
refreshed, tool/test/schema numerals), surface-plan + interaction-surfaces (superseded
banners), overview (routing + private-repo delinking + heritage table), SETUP (public repo,
commit-an-export story), getting-started/README (all 11 skills), and three skill
contradictions (link-artifacts full:true, detect-and-ask → retire-from-design, check-health
apply gate). skill_lint allowlist gained blocked_by_mode. All gates green. AGENTS.md "Current
state" still says no surface/service/LLM wiring exists and the interaction surface is an
open decision (78 tools ship); the module list omits two-thirds of src/ and calls GENESIS
unbuilt; the foundation pin is quoted v0.9.4 vs the manifest's v0.10.0; "53 edge types"
survives in four places vs the schema's 54; coverage matrix IS-7 says "not started" vs SP-3
✅ in the same file; `interaction-surfaces.md` carries no superseded label and overview.md
still routes to it as a live decision; README says 26 node types (omits Question), its
layout tree shows a docs-and-schema repo, and `../tools/` link is broken from root; SETUP.md
still says the repo is private and tells users to commit the graph the installer
force-gitignores (pick one story); getting-started/README lists 8 of 11 skills; skills:
link-artifacts step 6 needs `full: true`, detect-and-ask's dead-capability branch should
route through retire-from-design, check-health's apply gate self-contradicts; "(180 nodes)"
in three places vs 212; upgrading-to-v0.2.0 docs lack the breadcrumb.

**BL-61 · skill_lint is blind to single-word tool names — DONE 2026-07-21** — *deep review,
same day the lint shipped.* Size **S**. The `"_" in term` filter exempted `allocate`,
`satisfies`, `genesis`, `documents`, `precedes`, `provides`, `realizes`, `verifies`,
`consumes`, `contains`, `constrains` — 11 served tools, 10 referenced in skills, none checked.
Filter dropped; the allowlist gained the ~58 legitimate single-word non-tool terms (statuses,
enum values, field names, CLI/format words), the both-ways unused-guard keeping it exact.
Negative-tested: a renamed single-word tool now fails the lint (exit 1). The `"_" in term` filter means `allocate`, `satisfies`, `genesis`,
`documents`, `precedes`… are never checked — the rename-leaves-prose-behind case the lint
exists for. Drop the filter; extend the allowlist with legitimate single-word terms.

**BL-62 · Surface test-coverage gaps — DONE 2026-07-21** — *deep review.* Size **M**. 14 of 78 tools have no
coverage in tests/tools.rs or smoke_mcp.py (add_epoch, add_resource, delete_node,
dimension_drift(s), evaluate_allocation, pin_at_epoch, precedes, propose_allocation,
realizes, record_change, require_resource, surprising_connections, withdraw_question); plus
untested behaviors: get_node absent shape, and create_node/scan_nodes/search_design over real
stdio. **All 14 now covered**: two tests/tools.rs tests (a temporal/resource/realization/
analysis/delete walk + an ask→withdraw question round trip) and a smoke_mcp `§9c` section that
drives create_node/scan_nodes/search_design/delete_node/get_node over the real stdio boundary
— the blind spot smoke exists for. (export_graph overwrite guard is BL-57's, tested there when
it lands; get_node's absent shape is pinned to today's `{value:null}` with a BL-57 pointer.)

**BL-63 · Snapshots capture properties but not edges, so a re-allocation loses its history —
DONE 2026-07-21** — *user question + live demo, 2026-07-21 (promoted from BL-58 idea I4).*
Size ~~M~~.

**Built**: `snapshot_node` captures the node's design edges into a new optional
`Snapshot.edges` property beside `state` — direction, edge type, other endpoint (id and type),
and the edge's properties, sorted for byte-stable exports (the BL-58 discipline). Bookkeeping
neighbours (Snapshot/ChangeEvent/DesignEpoch/TemporalFact/dimensions/Fragment/DriftEvent/
Question) are excluded — a snapshot captures design structure, not the audit trail, and would
otherwise grow with each prior snapshot of the same node. `parse_snapshot_edges` +
`SnapshotEdge` join the core API; a pre-BL-63 snapshot reads as an empty capture, never an
error. **One deliberate deviation from the entry's lean**: full capture (bookkeeping excluded)
rather than changed-edges scope — at snapshot time the caller cannot know which edges the
coming edit will touch, capture is cheap at design scale (a hub is a few KB), and the exclusion
list bounds the noise; simpler than an opt-in split and loses nothing. Tests pin the demo shape
end-to-end (lazy reallocation: the snapshot alone now proves "A once owned Z"), bookkeeping
exclusion + deterministic order, and old-snapshot tolerance. The revise-design links guidance
dropped its pre-BL-63 workaround ("leave a formerly-true edge") for record-first-then-delete,
and both CRUD skills say edges are captured. Schema: `Snapshot.edges` optional → next cut is
minor per the versioning policy.

`snapshot_node` serializes a node's **properties** into `Snapshot.state`; it captures none of
the node's **edges**. Axis Z's promise is that the past is recoverable, and for a node whose
*properties* changed that holds — but a large class of design change is an **edge** move, not a
property edit, and those lose their history unless the modeller deliberately records a Decision.

Demonstrated end-to-end (docs/trials, reallocation demo): "Service A does X, Y, Z" → later
"Reconcile (Z) moves to Service B." The right-way sequence (impact-check → record_change →
delete the old `ALLOCATED_TO`, add the new one → a superseding Decision) worked, and the
Decision chain (`dec:own-v2 OBSOLETES dec:own-v1`, v1 marked `superseded`) preserved the
ownership history perfectly. **But** the snapshot of `cap:z` held only its properties
(name/status/…), not the `ALLOCATED_TO cmp:a` edge it lost — so the *only* durable record that
"A once owned Z" was the hand-authored Decision. A lazy reallocation (delete_edge + allocate,
no Decision) would leave Z on B with **no trace** it was ever on A. This is exactly the
long-lived-design case (storyflow, 8–9 months of shifting allocations) where it bites.

**The fix**: capture the affected node's edges into the snapshot alongside its properties, so a
`Modified`/`Removed` `record_change` preserves the link structure, not just the text. Design
decisions to make:
- **Scope** — snapshot *all* of a node's edges (complete but noisy/expensive for a hub like the
  Project), or only the edges the change actually touched (cheap, but needs the change to name
  them — pairs with BL-50's `affected` list and the `field`-scoped `CHANGED` edge). Lean toward
  the changed-edges scope with a full-capture opt-in.
- **Storage** — extend `Snapshot.state` (today a JSON string of props) with an `edges` section,
  serialized sorted for byte-stability (same discipline as BL-58's property fix). Update
  `parse_snapshot_state` and any reader.
- **Honesty in the meantime** — until built, say so loudly on `snapshot_node`'s docs and in the
  revise-design / retire-from-design skills: "a reallocation's history lives in the Decision you
  record, not the snapshot — model it as a Decision." (Cheap, do first.)

Not a silent-drop fix like BL-58 — the current behaviour is honest, just incomplete; this
completes axis-Z coverage for the edge dimension of change.

**BL-64 · The lifecycle stops at Operation — no disposal / retirement phase** — *user, UAF
lifecycle-breadth analysis, 2026-07-21.* Concept; size **M–L**; needs the user on vocabulary.

reflow2's phase spine is P0 Intent → P5 Operation, full stop. UAF's sixth phase — decommission:
retirement timelines, data-migration pathways, unwinding dependencies, sunsetting the
capabilities a system provided to *others* — has no representation (node-type probe: no
`Disposal`/`Retirement`/lifecycle-state construct). Do not confuse with the `retire-from-design`
skill: that retires something from the *model*; this is about modelling the *system's* end of
life.

The insight that makes this cheap: **reflow2 already has the retirement-impact engine** —
`propagate_from` answers "what breaks if we remove X." What's missing is (a) the *vocabulary*
to say "this Component/Capability/Release is planned for retirement" (a `lifecycle_state`:
`active` / `sunsetting` / `retired` / `disposed`, or a first-class phase), (b) a *detector* —
"a node marked `sunsetting` still has active dependents / consumers who were never told" (the
retirement analog of `unsatisfied_requirement`), reusing the detect-and-ask loop, and (c)
modelling the *replacement/migration* (which capability supersedes it, where the data goes) as
Decisions + `EVOLVES_INTO`/`OBSOLETES` — the same pattern BL-63 showed is how ownership history
should be recorded. Interim mitigation (cheap, do first): document that the removal blast radius
(`propagate_from` with a removal framing) *is* the retirement impact-check today.

**BL-65 · Risk & security are inference edges, not a lifecycle-spanning concern** — *user, UAF
+ DevSecOps analysis, 2026-07-21.* Concept; size **L**; needs the user on vocabulary.

Two commercial/defense lineages converge on the same gap. **UAF** embeds Security & Risk
viewpoints at *every* phase (concept → design → field), never bolted on. **DevSecOps** makes
that continuous and automated — SAST/SCA on every commit, security as a shift-left gate, not an
end-of-line compliance check. reflow2 has neither shape: risk exists only as inference *edges*
(`RISKS`, `MITIGATES`, `BLOCKS`), there is no `Risk` / `Threat` / `Control` / `SecurityAsset`
node (node-type probe confirms), and — the load-bearing gap — **the coherence loop has no
detector for the *absence* of a risk/security assessment.** It flags an unsatisfied requirement;
it never flags "this capability crosses a trust boundary or handles sensitive data and no risk
was assessed here." The seed exists: `EnvironmentRule` + `COMPLIES_WITH` / `VIOLATES_RULE` is a
compliance layer, and `cap:freshness`'s confirmation ledger is the pattern for "a claim nobody
has re-checked."

Fix, three layers: (a) **a first-class `Risk` node** (likelihood / impact / status), linked via
`RISKS` / `MITIGATES` to what it threatens and `CONSTRAINS` to what bounds it — optionally
`Threat` / `Control` for a fuller security model. (b) **A cross-cutting detector** —
`unassessed_risk`: a node past a phase gate, crossing a boundary or marked sensitive, with no
linked risk assessment, fires a gap through the *existing* detect-and-ask loop (this is the UAF
"every phase" principle expressed as reflow2's native "detect the silence" move). (c) **Continuous
automated governance** (the DevSecOps angle): compliance/security is reconciled the same way
artifacts are — a caller (CI, a scanner) supplies observations, reflow2 reports drift, and a
**security-debt ledger** (mirroring the confirmation ledger) shows what is `assessed` /
`drifting` / `unexamined` per node. Interim: document that `RISKS`/`MITIGATES` + `EnvironmentRule`
are today's tools and that reflow2 does not yet detect their *absence*.

Both BL-64 and BL-65 deliberately reuse propagate + detect-and-ask rather than inventing
subsystems; the genuinely new part in each is *vocabulary* (a lifecycle state; a Risk node),
which is a design decision for the user — hence "concept, needs the user."

**BL-66 · Design coherence as a consumer CI gate (shift-left the golden thread) — DONE
2026-07-21** — *user, commercial-practice analysis (DevOps/shift-left), 2026-07-21.* Size ~~S–M~~.

**Built**: `tools/reflow2_check.py` — stdlib-only, self-contained (embeds the reflow2_cli stdio
client so it ships alone in the kit tarball; release.yml carries it). Imports the **committed
export** into a temp graph (decision made by evidence, not preference: `.reflow2/` is
gitignored so CI *cannot* open it, and the committed export is the design the team actually
reviewed), rehashes every registered Artifact from the working tree — truncating sha256 to each
registered checksum's own length, so any registration dialect works — reconciles, runs
`detect_gaps`. **Fails (exit 1)** on unaccepted `checksum_change`/`missing` (an accepted drift
updates the export, so red = the two-sided accept was skipped) and on open **anchored** gaps at
severity ≥ 0.8 (`--gap-threshold`); `acknowledge_gap` is the sanctioned way to go green without
fixing, so the gate inherits DETECT's own review mechanism instead of inventing a mute flag.
Phase nudges and `no_baseline` print as notes, never gate; exit 2 (cannot run) is loud, never a
silent pass. Verified three-way on reflow2 itself: clean tree passes, a doctored `budget.rs`
fails with the named artifact, a missing export refuses with instructions. Shipped as the
**ci-gate** skill (setup, GitHub Actions snippet, and the honest ways to turn red green —
including the two launderings it names and forbids) plus a SETUP.md pointer. There is
deliberately no flag to skip the drift check.

DevOps' deepest principle is that verification runs on **every commit**, not at a milestone
gate. reflow2 gave *itself* CI (BL-52), but a **consumer** project has no documented, one-step
way to run reflow2's detectors as a build gate on its own commits — so the golden thread is
checked periodically (a session), not continuously. Every piece already exists: the CLI
(`reflow2-mcp --import/--export`, `reflow2_cli.py`), `reconcile_artifacts` (caller supplies the
observed hashes — a CI step computes them), `detect_gaps`, the two-sided drift accept. What is
missing is the *assembly*: a single `reflow2 check` verb (or a documented pipeline step) that
(a) recomputes artifact hashes from disk, (b) reconciles against the committed export, (c) runs
`detect_gaps`, and (d) **exits non-zero** when a design drifts from its build with no two-sided
accept, or a new critical/anchored gap appears — fail-loud, never a silent pass. Ship it as a
consumer skill + a copy-paste CI snippet (the SessionStart-hook pattern from BL-50 (3) is the
model). Decisions: what severity fails the build (unaccepted `checksum_change`? a new critical
gap? a regressed `unrealized_capability`?), and read-from-committed-export vs open the live
`.reflow2/graph` (single-writer) in CI. This makes everything reflow2 already does
*continuous*, which is the whole point of the frictionless-cadence thread ([BL-51], [BL-15]).

**BL-67 · Requirements as live measured objectives — SLO/SLI reconciliation (as-operating)** —
*user, commercial-practice analysis (SRE), 2026-07-21.* Concept; size **M–L**; needs the user
on the vocabulary call.

SRE's move is that a spec is not a static statement (MTBF) but a **live objective** (SLO)
measured by **live indicators** (SLIs) against an **error budget**. This is the one commercial
practice that genuinely *extends what reflow2 can be* — from design ↔ built ↔ fielded, to design
↔ **running reality**: *is the deployed system meeting its measured objectives right now?* And
it reuses reflow2's own architecture almost entirely:
- **The SLO is a `Verification(method=measurement)`** with a target — the schema already has
  `method: measurement` and a `passing`/`failing` status. No new node type strictly required.
- **A new reconcile-family op, `reconcile_operating`** — the caller (a monitoring system,
  Prometheus, a CI probe) supplies observed SLI values exactly the way `reconcile_artifacts`
  supplies checksums (the "core does no I/O, the surface observes" seam); reflow2 compares
  against the SLO target, sets the Verification `passing`/`failing`, records the divergence, and
  `propagation_seeds` walk **up** the thread to the Capability/Requirement — the as-fielded
  pattern, one axis further.
- **The error budget is a `Constraint`** (`direction: maximum`, `quantity` = the budgeted
  metric) whose contribution is the live-consumed budget — `budget_report` already rolls this up.
- **`dimension_drift`** already detects an SLI *trend* declining over time, and `cap:freshness`'s
  confirmation ledger is already SRE-adjacent ("a claim nobody re-checked is stale"). An
  **as-operating** viewpoint would join the as-designed/built/fielded/verified set in
  `render_views`.

So the vocabulary decision for the user is small — "is an SLO a measurement-Verification with a
target, or does it deserve its own node?" — and most of the machinery (reconcile seam, Constraint
budget, dimensions, freshness) is already there. Closes the loop from intent all the way to the
telemetry of the running system.

**BL-68 · Readiness-driven roadmapping — derive the delivery timeline, don't declare it**
(keystone) — *user, Space Force acquisitions/SE, 2026-07-21.* Concept; size **L**; the most
ambitious item on the board. Needs the user on vocabulary. Unifies and gives purpose to
[BL-64] (lifecycle phasing), [BL-65] (risk), and [BL-67] (the modelled future).

The problem, in the user's words: on real programs, "people didn't understand which *epoch* a
design would be delivered on." Roadmaps are drawn as slides, disconnected from the actual
maturity of the enabling technology, so the delivery timeline is an assertion nobody can defend.
Meanwhile a design is not static under incremental development — **Version A is achievable today
because its enabling tech is at acceptable TRL/MRL; Version X is a decade out because a key
technology is immature now and expected to mature later.** The LLM's job is to help the user say
"*here is what we can build today, and here is the improved version 10 years out — that is the
roadmap*," and to make that claim traceable.

Three parts:

- **(1) Readiness and the -ilities as first-class scored risk factors that GATE achievability.**
  TRL, MRL, affordability, maintainability, reliability — all *risk factors in a design choice*,
  same family as BL-65 (a low TRL *is* a risk). reflow2 already scores `maturity`/`reliability`/
  `maintainability` as `DimensionAssessment`s, but a dimension only *trends*; it does not *gate*.
  New: a readiness assessment (TRL/MRL 1–9) whose being below threshold marks a design increment
  **not buildable yet** — and a **forecast** of that score over time (TRL 3 now, 7 expected 2035)
  so the timeline can be computed forward.
- **(2) Design increments/alternatives as comparable first-class entities.** reflow2 models
  *one* coherent design; source selection and incremental development need a *family* of
  candidates (Version A vs X) that share requirements but differ in what is achievable when,
  each scored on TRL/MRL/-ilities. Today the only trace of this is `Decision.alternatives` — an
  opaque prose string ("options considered and why they lost"). New: increments/options as
  living nodes you can score, propagate through, and pin to epochs.
- **(3) The derived roadmap — the insight that is reflow2's to claim.** Because the golden thread
  runs capability → component → enabling technology, and each technology carries a readiness
  score with a forecast, **the epoch an increment can deliver on is *computable*, not declared**:
  it is the epoch by which every enabling technology on its thread reaches acceptable TRL/MRL.
  `propagate` already walks that thread; feeding it readiness turns "which epoch delivers which
  design" from an opinion into a traceable output — *"this increment is 2036 because THIS
  technology is TRL 3 today, projected 7 in 2035, and the capability cannot close without it."*
  That is exactly the legibility the user's programs lacked.

**The spine (state this in any design of the feature): the roadmap is a risk-burndown schedule.**
Each epoch is the point where enough readiness risk has retired to make the next increment
achievable; readiness maturing *is* the risk clearing. This single framing unifies BL-65 (risk),
BL-67 (the future), and this item: the roadmap is *when the risk clears*.

Worked example (the user's): "refuel a satellite by laser" → capabilities {high-power lasing,
beam pointing/tracking, power→light conversion, thermal management} → each traces to components
and technologies with a TRL. Today's design = the increment whose whole thread is mature now;
the 10-year design = the increment gated on (e.g.) high-efficiency power→laser conversion
maturing TRL 3 → 7. reflow2 propagates the gate and can name why the later increment is later.

Seeded vs new — **seeded**: the `maturity`/`reliability`/`maintainability` dimensions, the
temporal axis (epochs, `ANTICIPATES`, `EVOLVES_INTO`), propagate + the thread, the
`Decision.alternatives` prose. **New**: TRL/MRL as a *gating* readiness assessment; a readiness
*forecast* over time (the quantitative form of BL-67's "model the future"); increments/
alternatives as first-class comparable nodes; and the derived-roadmap computation.

Vocabulary decisions that are the user's to make (why this is concept, not spec):
1. Is an increment/alternative a **new node type**, a variant of `Release` (Releases are
   as-*built*; these are as-*planned* candidates — lean: new node), or a scoped sub-graph?
2. Is readiness a **new assessment kind** with gate-semantics, or a `trl`/`mrl` addition to the
   dimension enum? (Lean: its own construct, precisely because it *gates* rather than *trends*.)
3. How is a readiness **forecast over time** modelled so the roadmap computes forward
   (a TemporalFact series on the technology? a projected `DimensionObservation` per epoch?).

Why it matters: no roadmapping tool today *derives and defends* the delivery timeline from the
real readiness of the technology — they assert it. reflow2's thread + propagate makes derivation
possible, which is a capability, not a viewpoint.

**BL-69 · `single_point_of_failure` measured connectivity on the wrong graph — DONE 2026-07-21** —
*self-host review, 2026-07-21, while dispositioning the two SPOF warnings on reflow2's own graph.*
Size **S–M**. ~~S–M~~

Raised because `detect_defects` flagged `cmp:flow` and `cmp:service`, while an independent
articulation-point (Tarjan) analysis of the operational dependency graph said the true cut
vertices were `cmp:service`, `cmp:export`, `cmp:graph` — wrong in both directions at once. The
entry's first diagnosis (community bridges) was wrong about the mechanism; reading the source
corrected it: the detector already ran a genuine removal-splits-the-graph test, baseline-relative
(BL-5 pass 1) with operational candidates (pass 2) and the library filter (pass 3, F6) — but it
measured connectivity on the **full design network**, where intent edges are wrong in both
directions at once:

- **They donate mass.** Removing `cmp:flow` strands its own intent cluster (`cap:model-process` +
  `art:flow` + verification) — ≥2 nodes, so the non-trivial filter passed and a healthily-modelled
  leaf module fired. The severed "subsystem" was made of sentences.
- **They donate phantom connectivity.** Removing `cmp:export` severs `cmp:init`+`ifc:graph-export`
  operationally, but the design network kept them "connected" through
  `art:init REALIZES cap:kit SATISFIES …` — a path that carries nothing at run time — so a real
  cut vertex stayed silent.

**The fix (the fourth pass at this detector, and the same selectivity lesson one level deeper):**
connectivity and candidate enumeration both moved to the **as-built operational network** —
Components/Interfaces/Resources/Environments plus the Artifacts realizing them, joined by the
traceability edges that hold between such nodes. Intent nodes not only must not be *flagged*
(pass 2); they must not *participate in the connectivity being measured*. Artifacts are members
(a stranded part with its file is a real severed subsystem — the fixture for the pinned
interface-bridge test had already padded its subsystems with artifacts to pass the non-trivial
filter, so this codifies what the doctrine already practiced) but never candidates. Every prior
lesson kept: baseline-relative, non-trivial ≥2, library exclusion.

**Measured on reflow2's own graph** (`build_design_graph.py --analyse-only`, before → after):
SPOF `{cmp:flow, cmp:service}` → `{cmp:graph, cmp:export, ifc:graph-export, cmp:service}`.
`cmp:flow` stops firing; its community-bridge signal stays in `surprising_connections` under its
accurate name. Three findings are new-and-true: `cmp:export` and `ifc:graph-export` are the only
route from the kit's converter (`cmp:init`) to the design, and `cmp:graph` genuinely strands
`schema`/`search`/`vocabulary` (each with its file) plus the whole export chain. All four are now
answered on the record: `cmp:service` by `dec:service-spof-accepted`, `cmp:graph` by
`dec:graph-spof-accepted` (the single store handle is the architecture, and a second one would be
two write paths to one store), and the `cmp:export`+`ifc:graph-export` chain by
`dec:export-door-spof-accepted` (one canonical, deterministic portability format is the feature;
a second export path would be a second source of truth). The defect
count *rising* 2 → 4 while the false positive leaves is the fix: the count was previously wrong
in both directions. Two regression tests pin the two shapes (intent-cluster stranding must not
fire; a cut vertex hidden by intent edges must); the island-immunity fixture was rebuilt with
operational subsystems, preserving its lesson. All 14 structural tests, workspace suites, and the
instruments (phase 13/13, erosion 7/8, coherent 9/9, model_the_loop, smoke) at their baselines.

**BL-70 · Parallel alternatives — AoA branches held open until a decision point** — *user,
source-selection practice (analysis of alternatives / DOTMLPF-P), 2026-07-21.* Concept; size
**L**; the vocabulary decisions are the user's (and shared with [BL-68]'s question 1).
**v1 BUILT 2026-07-22** (`cap:compare-alternatives`, `dec:parallel-alternatives`, user decided both
axes): alternatives are **branch-by-file** (separate exports, reusing BL-80's compare/merge/retire
whole — NOT world-tags inside one graph, so no detector goes world-aware), and they are design
**space** (siblings CONTRADICT, held under a proposed Decision), distinct from **time** (epochs /
EVOLVES_INTO). `analyze_alternatives(paths)` (MCP tool + `alternatives.rs`, 4 tests) loads N
alternative exports and lays their measures side by side (graph_report snapshot — nodes, gaps,
defects, modularity, verification) plus each branch's `compare_designs` divergence from the
baseline. Collapse = `merge_designs`/`apply_merge` winner into baseline + retire losers (built).
**Rung 2 BUILT 2026-07-22** (`cap:decision-point`): a *proposed* Decision is a decision point with
teeth. `register_alternative` hangs a lightweight Artifact pointer (its export, branch-by-file)
under it, `GOVERNED_BY` the Decision and `CONTRADICTS` its siblings — refused unless the Decision is
proposed. `alternatives_for` lists them (feed to `analyze_alternatives`). `collapse_decision`
chooses a winner: Decision → accepted, losers `OBSOLETES`-superseded (retired on the record, not
deleted), outcome + rationale written into the Decision's own `alternatives` field (the ADR obituary
the fork upgrades from prose to structure). Winner's content merged separately via `apply_merge`.
Ops `set_decision_status` / `register_alternative` / `alternatives_for` / `collapse_decision`; 8
tests. **The teeth are complete:** the `undecided_decision_point` DETECT gap BUILT 2026-07-22 — a
proposed Decision holding ≥2 alternatives is surfaced by `detect_gaps` as an open fork ("which do
you choose?"), anchored on the Decision + its alternatives so it clears the moment the decision
collapses (a fork of one road is not a choice, so ≥2 is the threshold). **Still open on BL-70:**
DETECT question #2 (is a cross-branch absence — "satisfied in only one alternative" — itself a
gap?), per-branch readiness (BL-68), and world-scope (decision #1's other option) if simultaneous
in-one-graph AoA ever proves needed. The core of BL-70 — compare, hold, collapse, and now surface —
is done.

> **The fork layer DESIGNED 2026-07-24 (graph-only; no Rust yet).** Anthony decided all three open
> axes. **(1) `dec:fork-point-address` — a fork point is a coordinate, not a copy.** "The design as
> it stood at decision D" resolves as Decision `AT_EPOCH` → DesignEpoch → that epoch's existing
> `checksum` (the export's `content_hash`) → the committed document, which git already stores.
> *Rejected: a reflow2-native ref/branch/DAG layer* — it duplicates git (and `dec:repo-file-embedded`
> put the design in the repo precisely so repo tooling carries file-shaped concerns), and a mutable
> pointer is a foreign object in a graph whose axis-Z doctrine is that history is never overwritten.
> Bringing a road home is then the existing three-way merge with the fork point as base — the
> merge-base `dec:merge-three-way` currently borrows from git, now derivable from reflow2's own
> chain. **(2) `dec:reopen-supersedes` — re-opening mints a NEW proposed Decision that `OBSOLETES`
> the original**, which stays `accepted` forever. *Rejected: flipping the original back to
> `proposed`* — it erases that the question was ever settled, which is exactly the strongest evidence
> for re-opening it. **(3) `dec:temporal-backfill-from-releases` — the epoch chain is backfilled from
> real shipped release tags or not at all.** *Rejected: forward-only* (a fork layer that can only
> fork decisions made after it was built has nothing to fork) *and full reconstruction* (a fabricated
> epoch is worse than an absent one — it looks like evidence).
>
> **The finding that reshaped the work: reflow2's own temporal axis was barely used.** Zero
> Snapshots, 8 of 42 ChangeEvents pinned, **no Decision anchored to any epoch at all**, and the chain
> stopped 2026-07-20 at three epochs. "Re-open a past decision at its epoch" had nothing to stand on
> — the fork layer's foundation was missing, not merely its ref layer. Root cause is a practice one:
> `add_change_event` (cheap, epoch-free) had been used throughout where `record_change` (which pins
> *and* snapshots) was meant. Anything that cuts an epoch should prefer `record_change`.
>
> **Built into the graph this session:** a 12-epoch spine (`epoch:genesis` → `v040` → `v050-cut` →
> `v050-hardening` → `v060` → `v061` → `v070` → `v080` → `v090` → `v0100` → `v0101` →
> `bl70-fork-layer`), `PRECEDES`-chained, sequences spaced by 10 to leave insertion room; `checksum`
> set on `v090`/`v0100`/`v0101` — the only three tags whose export carries an embedded
> `content_hash`, since `dec:export-hash-chain` shipped in v0.9.0; earlier epochs say so in their
> description rather than claiming a hash. All 34 Decisions anchored to the epoch of the earliest
> release whose *committed export actually contains them* (v0.4.0 → the 9 founding doctrine
> decisions, v0.6.0 → 1, v0.7.0 → the 3 SPOF-accepted, v0.8.0 → 1, v0.9.0 → 2, v0.10.0 → 18); v0.4.0
> is the earliest tag carrying an export, so for those 9 it is earliest *evidence*, not origin, and
> the epoch description says so. All 9 Releases pinned to their epochs.
>
> **Also modelled — the code that existed but the design didn't know about:** `cmp:merge` and
> `cmp:alternatives` under `sys:time-history` (whose stated purpose already read "hold and compare
> alternatives, diff and merge divergent designs"), with `cap:merge-designs`/`cap:merge-rerere` and
> `cap:compare-alternatives`/`cap:decision-point` moved off the `cmp:compare` stand-in they had been
> lumped under; `art:merge` (merge.rs, 1531 lines) and `art:alternatives` (alternatives.rs, 355)
> registered with checksums; `cap:fork-alternatives` finally allocated. **This dissolved the
> disconnected-community defect (6 → 5 structural defects, the remaining 5 being the accepted
> SPOFs).** New `cap:revise-trigger` captures rung 3. `chg:bl70-fork-layer` recorded and pinned.
> Export 346n/1017e → **365n/1100e**, gate green.
>
> **Still to build (Rust, next rung):** the read-only `fork_point(decision_id)` — resolve a Decision
> to its epoch, checksum and the blast radius it governs, and report what has changed since; that
> output is the base a three-way merge takes. Then `cap:revise-trigger`'s detector. **Deliberately
> left open:** the revise threshold ("how much evidence counts as enough to re-open a road") is not
> fixed until the detector can be calibrated against real history — a number chosen now would be a
> guess wearing a decision's clothes. **Detector-honesty note for whoever picks this up:** the
> `unrealized_capability` gap on `cap:fork-alternatives` retired itself the moment the capability
> became allocated to a component that *some* artifact realizes, even though the fork point is
> unbuilt. That is detector generosity, not progress — its silence means nothing here.

The idea in the user's words: an undecided design choice could hold **forks** — option A and
option B (and more) as live sub-designs — and, from military source selection, an analysis of
alternatives keeps two or more parallel designs *viable until some decision is made* — "almost
like a decision point."

More is seeded than expected:

- **`Decision.status = proposed` is a decision point in embryo** — the node can exist *before*
  the choice is made, with `GOVERNED_BY` edges already saying which parts of the design hang on
  it. Nothing today makes a `proposed` Decision *gate* anything; that is the missing teeth.
- **`Decision.alternatives` is the losers' obituary** — prose, post-hoc, written on the winner.
  The fork idea upgrades exactly this field: alternatives as live sub-graphs while the choice is
  open, collapsing into that record when it closes — real history instead of reconstruction.
- The edge vocabulary mostly exists: `CONTRADICTS` (opposing), `EVOLVES_INTO`, `OBSOLETES`,
  `ANTICIPATES` can wire branches to each other; `retire-from-design` is the losing branch's
  exit (superseded — genuine history retired on the record, not a mistake deleted).
- **The comparison machinery an AoA needs is the machinery that already exists** —
  `budget_report`, the dimension assessments, [BL-68]'s readiness scores — run *per branch*,
  they make alternatives comparable on the same measures instead of on advocacy. BL-68's
  vocabulary question 1 (increments/alternatives as first-class nodes) is this same question at
  roadmap scale; one answer should serve both.

What is genuinely missing is one primitive: **the graph is single-world.** Two Components both
`SATISFIES`-ing one Requirement *on purpose* is indistinguishable from the incoherence the
detectors hunt (`possible_duplicate`, allocation defects) — a second viable design held in the
same graph would be punished for existing. The need is a **scope**: nodes and edges tagged to an
alternative ("world"), DETECT running *within* a world, reports comparable *across* worlds, and
the decision point collapsing the superposition — winner merges into the baseline, loser retired
with its rationale. Cheapest first increment, no schema change: **one exported graph per
alternative plus a cross-export comparison report** — export/import already round-trips a whole
design deterministically, so branch-by-file works today and teaches what the real scoping
primitive must preserve.

**DOTMLPF-P is the breadth discipline for *generating* branches**: the alternative to a materiel
Component may be non-materiel — doctrine, organization, training, a process change. reflow2 is
unusually placed to hold that honestly: `req:design-anything` + `Flow` ([BL-37]) mean one branch
can be a process satisfying the requirement while a sibling branch is a product — the same
decision point gating a materiel and a non-materiel solution in one graph, which is exactly the
comparison a source selection is supposed to make and rarely gets tool support for.

Decisions that are the user's to make: branch as node-set tag vs sub-graph vs graph-per-branch;
does DETECT run per-world only, or is a cross-world absence itself a gap ("this requirement is
satisfied in only one alternative")?; and where the line falls between an alternative (design
space, `CONTRADICTS`) and an epoch (time, `EVOLVES_INTO`) — the AoA that keeps both branches is
describing space, not history, and the vocabulary should not conflate them. Connects to
[BL-44]: a cluster checkout that explores an option rather than progressing the baseline is this
item's fork — the two likely share the scoping primitive (see BL-44's 2026-07-21 addendum).

**BL-71 · Two models of one design: the curated rebuild clobbers the accumulated live graph** —
*found 2026-07-21 while modelling the v0.6/v0.7 Release nodes.* Size **M**. **CLOSED
2026-07-21: all three rungs done** (a+b layering/tripwire, c the design-vs-design diff below).

`tools/build_design_graph.py` (full run) rebuilds the curated self-model from source — 184
nodes — and writes it to `docs/design/reflow2.json`, the same path the live sessions export the
**accumulated** graph to (247 nodes at the time: everything the curated model has *plus* the
session-written layer — freshness-claim ChangeEvents, the SPOF-acceptance Decisions, Questions,
the BL-63/66/69 change events, `art:check`). The full rebuild therefore silently **discards the
live layer from the committed record**; it happened live and was caught only because the node
count dropped, then restored from git. The two writers disagree about what the file *is*: the
rebuild treats it as a projection of source, the sessions treat it as the durable record of the
graph (SETUP.md's own doctrine: "the export is the durable record").

Sharper statement: the curated model and the live graph have **diverged as designs** — 18 vs 10
Requirements, 10 vs 9 Decisions — and nothing detects or reconciles that. This is drift between
two as-designed records, a case none of the three reconcile tools covers (they all compare
design against *reality* — disk, deployment, test runs — never design against design).

**Rungs (a) and (b) DONE 2026-07-21.** The rebuild now **imports the committed export first
and layers the curated pass onto it** (import is upsert; curated wins on shared ids, so a
deliberate curated update — a release status — takes, while the session-written layer
survives). Genesis is skipped when a prior export was imported — it refuses to clobber an
existing graph, correctly. And the tool **refuses to write a shrinking export** (fewer nodes
than the committed file), naming BL-71 — a regression tripwire that layering should make
unreachable, which is exactly when a tripwire earns its keep. Verified: the union export is 250
nodes (18 Requirements, 13 Decisions, all ChangeEvents/Questions/Fragments/DriftEvents, plus
the 5 Release nodes with v0.7.0 active), a second full run is idempotent at 250, gaps 0,
reconcile-vs-filesystem clean, and `reflow2_check.py` passes against it. The interim
do-not-run rule is retired. Note: the live `.reflow2` graph learns the release nodes on its
next `--import` of the committed export (the session server holds the lock and predates 0.7.0).

**Rung (c) DONE 2026-07-21** — the vocabulary session happened and the user decided all four
axes (`dec:design-diff-vocabulary`): the diff exists as a **core op** (`compare.rs`,
`compare_designs` / `compare_with_base`), exposed as the `compare_designs` MCP tool and
`reflow2-mcp --diff BASE [OTHER]`; it compares **two export documents or the live graph
against one**; findings are **directional** — added / removed / changed relative to a named
base, property-level with absent-vs-present distinguished — because every real consumer has a
base (committed record / main branch / the state a claim saw), while AoA can read it neutrally
or run it both ways; and the report is **banded** into design content vs the supporting layer,
because the divergence that motivated this item was 3 Decisions and 8 Requirements buried
under ~20 bookkeeping nodes. Vocabulary line drawn on the record: this reports **divergence**
between two as-designed records; "drift" stays reserved for design-vs-reality. Verified by 8
core tests + a tool test + 5 smoke checks (including the CLI diffing two files *while the
server holds the lock* — the two-file form never opens the graph); first live act was
confirming its own landing (committed export vs live graph: identical, 260/616). The where-am-i
skill now opens with it. Remaining consumers when picked up: BL-70's per-branch comparison
report and BL-12's was-it-ignorance-or-defiance merge question both read this diff.

## Deliberate deferrals

Not gaps — decisions, recorded so they aren't rediscovered as bugs.

- **WS-4 `EnvironmentRule` / WS-5 `QualityGate`** — nothing reads or asks for either type, so a
  constructor would build the mirror image of the problem WS-1..3 fixed. Each lands with its
  detector.
- **Real LLM provider backends** — unnecessary agent-native; the ambient agent is the LLM.
- **`EmbeddingBackend`** — semantic dedup and retrieval. The audit has prior art on shape
  (local MiniLM/384-dim, normalized inner product, hash-gated rebuild) and one caution: retrieval
  thresholds are not identity thresholds.
- **Generative HEAL content** — proposals stay review-gated stubs.
- **Bayesian architecture optimization** — assessed and dropped; see the audit's do-not-port list.

## Recurring lesson

A capability exists in core and is unreachable or unadvertised on the surface. Fourteen instances so
far: `Interface`, HEAL's skill, the `Verification`/operate write side, `contain_component`,
`graph_id`, `Requirement.status`, `graph_report` as an answer to "where am I", the whole
`TemporalFact` / `ABOUT_ENTITY` / `VALID_FROM` / `VALID_TO` layer (schema-complete, zero Rust
API), `DOCUMENTS` (declared, named in `nodes.rs`, no constructor and no tool — closed 2026-07-20
by BL-26's write side), and `precedes`
(implemented in `temporal.rs`, no tool, so the epoch chain axis Z exists to record cannot be drawn by
any client — BL-36), and `Flow` (fully specified with its own edge `PART_OF_FLOW`, no constructor,
no tool — so no process could be modelled at all; BL-37 built the write side and `flow_report`), and `DriftEvent.resolved` (declared with
`default: false`, written by nothing — every recorded divergence stayed "open" forever no matter
what happened next; BL-35 made the accept flip it), and `pin_at_epoch` (generic in core since the
temporal module landed, `AT_EPOCH` declared `from: "*"` — and no tool, so nothing could pin a
Release to its own `release_cut` epoch; BL-34 exposed it), and `Constraint` (fourteenth: named in
`nodes.rs`, fully specified in the schema with a `budget` category — no constructor, no tool, so
no limit could ever be recorded; BL-11 built `add_constraint`/`constrains`).

Before building something new, the higher-yield question is usually: **what does the core
already do that nothing can reach?**

The sibling lesson, learned the same way: a capability can also be unreachable because nothing
*points at it*. The consumer kit's skills were installed where three of four harnesses never look
(BL-22), and `describe_schema` would have been invisible to the people who needed it had the kit
not been updated in the same change (BL-1). Shipping the code is not shipping the capability.

Third variant, from the [self-host genesis trial](trials/2026-07-18-selfhost-genesis.md): a
capability can be unreachable **on one harness only**. BL-28's untyped `JsonValue` parameters worked
from grok build and fail from Claude Code, because a schema that declares no type leaves
marshalling to the client and the two clients choose differently. The same shape appears on the
response side (the array `structuredContent` bug, `delete_node`, `graph_report_markdown`). The
generalisation: **anywhere the tool surface declines to state a type, a client is free to guess,
and our test client's guess is not evidence.** `tools/smoke_mcp.py` is green on all five broken
params. Asserting the *schema* — no advertised property without a type — is a different check from
asserting behaviour through a client we wrote, and it is the one that catches this class.
