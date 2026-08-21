# What reflow2 offers: 21 skills and 155 tools

Generated from the **running server**, not from memory — the skill list came from `list_skills`,
the tool list and every read/write marking from `tools/list`, and the command mapping from
`getting-started/commands/`. If this file and the server disagree, the server is right.

As of v0.37.0 + the changes merged after it, 2026-08-21.

---

## Who calls what — the short answer

**Nothing here fires by itself.** That is worth saying plainly, because "automatic" is the natural
assumption and it is wrong in a specific way:

- **Skills are served, never auto-loaded.** They live in the server so they always match the
  running version, but no harness loads one for you. An agent reaches a skill by calling
  `get_skill`, and a person reaches one by typing its slash command.
- **Hooks emit instructions, not calls.** `tools/loop_nudge.py` is wired to three harness events
  and it *writes a line into the session* — it never calls a reflow2 tool. What it does is make
  the agent notice; the agent still decides.

So the three ways anything here gets invoked:

| how | what it looks like |
|---|---|
| **A person types a slash command** | `/gaps`, `/health`, `/optimize` — 23 commands, 21 of which name a skill |
| **The agent calls a tool or `get_skill`** | The ordinary path. Skill descriptions are written as trigger conditions so an agent can recognise its own situation |
| **A hook nudges the agent** | `loop_nudge.py` on SessionStart, PostToolUse and Stop — see below |

### What the hook actually does

`tools/loop_nudge.py` ships in the kit and the installer wires it, so a consumer project gets it
too. Three events, and it counts rather than commands:

- **SessionStart** — prints the orientation line: orient on the graph before touching code.
- **PostToolUse** — counts reflow2 *writes* since the last loop check, and counts harness file
  edits made while the design brain was never called at all. A loop check (`loop_status`,
  `detect_gaps`, `detect_defects`) resets the count.
- **Stop** — the backstop. Blocks **once**, with the reason: graph writes finished with no loop
  check, or code changed and `propagate_change` was never run.

It exists because of a measured failure: told to "use reflow2 extensively", an agent under load
kept the graph's *bookkeeping* current and let the capture→detect→ask→decide loop stop.
**Under load, a mood loses to whatever has a trigger.**

### Two commands that are not skills

`/debt` and `/decisions` call a tool directly — `loop_status` and `scan_nodes` — with no
procedure behind them. Every other command names a skill.

---

## The 21 skills

Each is a procedure, not a tool call: it says what to do, in what order, and what *not* to do.
Read one in full with `get_skill` before doing the work it covers.

| skill | command | when it fires |
|---|---|---|
| **adopt** | `/adopt` | Reflow2 is pointed at a system that ALREADY EXISTS — a codebase, a product, a device — with little or no requirements documentation |
| **brainstorm** | `/brainstorm` | The user is thinking out loud rather than deciding — "just brainstorming", "what if we", "a few options", several half-formed ideas in one breath, "I'm not sure yet", or working an idea through before committing to it |
| **capture-intent** | `/req` | Use whenever the user shares a new idea, feature, brief, or requirement for this project |
| **capture-session** | `/capture-session` | The user asks you to capture what this session produced — "capture anything important from this conversation", or at a natural break, or before a long session ends |
| **check-health** | `/health` | Use after any structural change to the design (new components, new contracts, a resync after impact) and periodically before a build push |
| **ci-gate** | `/ci-gate` | The user wants the design checked on every commit — a CI build gate, "fail the build if the design drifts", shift-left coherence |
| **detect-and-ask** | `/gaps` | Use before building, and after capturing new intent, to find gaps in the design and ask the user about them |
| **genesis** | `/genesis` | Use at the very start of a project, or whenever the reflow2 design graph is empty, to bootstrap it from the user's opening brief |
| **governance-proposal** | `/rules` | The user states a rule the project follows rather than a thing it must do — "we always branch before pushing", "never edit generated files", a review step, a house style, a stack choice |
| **impact-check** | `/impact-check` | Use BEFORE changing or removing anything in an existing design — a new feature, a tweaked requirement, "what if we add wind?" |
| **ingest-corpus** | `/ingest-corpus` | Reflow2 is pointed at a FOLDER of documents rather than one — a directory of specifications, years of accumulated notes, a handover pack, "here is everything we ever wrote about this" |
| **kpp-proposal** | `/kpp` | The user states a need that sounds like it MUST hold no matter what — a number with a unit, a "shall", something whose failure would sink the whole effort |
| **link-artifacts** | `/link-artifacts` | Use right after you create or substantially change a real source file (Unity C#, a spec, a doc), to register it in the reflow2 graph as an Artifact that REALIZES the capability it implements — with a content hash so later edits are detectable |
| **link-projects** | `/link-projects` | Two or more separate reflow2 projects need to work together — "link projectA and projectB", "how does our service talk to theirs", "make the interface between these two real" |
| **optimize** | `/optimize` | Someone wants something to be faster, smaller or cheaper — "this is slow", "can we speed this up", "reduce the memory", "the build takes forever" — or when a module has been identified as worth improving on its own |
| **parallel-work** | `/parallel-work` | Two or more people (or agents) need to work on one design at the same time without colliding — "my brother and I are both editing this", "can we split this up", "how do we avoid stepping on each other", or before starting a large change on a design someone else is also touching |
| **plan-increments** | `/plan-increments` | The user asks what to do next, in what order, or what goes in which release — "what's the plan", "what ships in v2", "do these in this order", a numbered list of upcoming work, or when work has been agreed and nothing says when it lands |
| **report-friction** | `/report-friction` | Reflow2 itself gets in your way while you are designing — a tool that fails without saying why, a gap that fires on correct work, something you cannot record, a rejection you cannot act on |
| **retire-from-design** | `/retire-from-design` | Something should LEAVE the design — a requirement the user dropped, a capability superseded by another, a component that was a modelling mistake |
| **revise-design** | `/revise-design` | The user changes their mind about something already IN the design — a requirement's wording, a capability's scope, a status, a link that points at the wrong thing |
| **where-am-i** | `/where` | The user asks where things stand, what you've concluded, what's been decided, or wants to pick up an existing design after a break — and at the start of any session on a graph that already has a Project |

### Which you will actually reach for

- **Starting a project**: `genesis` (new) or `adopt` (code that already exists)
- **Every session**: `where-am-i` to orient, `detect-and-ask` to surface and put the gaps
- **While working**: `capture-intent` for new intent, `brainstorm` for thinking that is not intent
  yet, `revise-design` when something changes, `impact-check` before editing
- **Before pushing**: `check-health`, `link-artifacts`, `ci-gate`
- **Occasional**: `plan-increments`, `kpp-proposal`, `governance-proposal`, `link-projects`,
  `parallel-work`, `ingest-corpus`, `retire-from-design`, `optimize`
- **When reflow2 itself gets in the way**: `report-friction`
- **At a break**: `capture-session`

---

## The 155 tools

`read` never changes the design. **write** does. That marking is the tool's own `readOnlyHint`
annotation, read off the served surface — 58 read, 97 write.

Descriptions here are each tool's first sentence. Every tool carries far more in its full
description, which an agent sees in the tool schema; this table is for a person scanning.

### Capture — put intent and structure into the design

| tool | | what it does |
|---|---|---|
| `add_capability` | **write** | Create a Capability node |
| `add_component` | **write** | Create a Component node |
| `add_constraint` | **write** | Create a Constraint — a limit or rule the design must respect, vs a Requirement which is a goal to achieve |
| `add_contributor` | **write** | Record a Contributor — who authors and decides the DESIGN itself: a person, an automated coding agent, or an organization |
| `add_decision` | **write** | Record a Decision and why it was made (an ADR) |
| `add_flow` | **write** | Create a Flow — an ordered process linking Capabilities end to end (a user journey, an assembly sequence, an operating loop) |
| `add_interface` | **write** | Create an Interface node — a contract between parts (an API, event, data feed, CLI, library boundary, or physical/human connection point) |
| `add_project` | **write** | Create a Project node |
| `add_requirement` | **write** | Create a Requirement node |
| `allocate` | **write** | Allocate a Capability to a Component (ALLOCATED_TO) |
| `authored_by` | **write** | Attribute a design node to a Contributor (AUTHORED_BY) — whose word this Decision/Requirement/… is |
| `budget_report` | read | Roll a budget Constraint up (BL-11): total of stated contributions vs the limit, the worst dependency path among contributors (the path-cumulative rollup — end-to-end latency, mass down a chain), basis coverage (estimated vs measured), and an honest verdict — `incomplete` when any contribution is unstated, because a partial sum passed off as a total is how budgets lie |
| `constrains` | **write** | Record that a Constraint CONSTRAINS a target, with the target's `contribution` to the budget (in the Constraint's quantity unit) and the `basis` for the number (estimated/evidence/measured) |
| `consumes` | **write** | Record that a Component CONSUMES an Interface — it is the side that depends on the contract |
| `contain_component` | **write** | Nest one Component inside another (parent CONTAINS child) — the assembly spine |
| `contains` | **write** | Link a Project to a child node it CONTAINS |
| `decomposes` | **write** | Split a Requirement into a smaller one: `from_id` DECOMPOSES `to_id` |
| `flow_report` | read | Read a Flow back as facts: steps in stated order, the TRIGGERS transitions among them with their roles, and the cycles |
| `genesis` | **write** | Bootstrap the design graph: create the Project + a genesis Epoch anchor and return a next-steps checklist |
| `governed_by` | **write** | Link a node to the Decision or DesignRule that shapes it (GOVERNED_BY) |
| `move_component` | **write** | Move a Component to a different parent on the containment spine, DETACHING every parent it already had and naming them |
| `owned_by` | **write** | Record whose AREA a node is (OWNED_BY) — durable, standing, and never released |
| `part_of_flow` | **write** | Record that a Capability is a step of a Flow (PART_OF_FLOW), with its position (`step_order`) |
| `provides` | **write** | Record that a Component PROVIDES an Interface — it is the side that implements the contract |
| `review_relations` | **write** | Record what a node relates to — or that nothing does |
| `satisfies` | **write** | Link a Capability to a Requirement it SATISFIES |
| `set_capability_delivery` | **write** | Declare WHAT KIND of thing delivers a capability — and never whether it was delivered, which stays COMPUTED from the golden thread |
| `set_capability_signature` | **write** | Record what a Capability TAKES IN and PUTS OUT — its functional signature, which is the black-box interface at that tier (`req:recursive-black-box-decomposition`: every element of a design is a black box with inner function AND INTERFACES) |
| `set_capability_status` | **write** | Set a Capability's lifecycle status: `planned` (the default) / `in_progress` / `realized` / `verified` |
| `set_interface_designation` | **write** | Give an Interface its external ROLE, which is what makes composition computable: `published` (this design OFFERS the contract and others may rely on it), `required` (this design NEEDS one of these FROM OUTSIDE), `both` (rare, and therefore meaningful), or `internal` (plumbing its owner may change freely) |
| `set_interface_spec` | **write** | Fill in what a consumer of this contract must AGREE with — the paradigm (sync/async), the payload format, the field-level schema, the endpoint and permitted operations, authentication, transport security, and the error model |
| `set_project_mode` | **write** | Choose how much this project lets a machine change its design on its own: `flexible` (apply_heal applies structural repairs) or `rigid` (apply_heal proposes them and stops, so a human decides) |
| `set_provenance` | **write** | Record how a node entered the graph: `authored` (the default, someone stated it) / `planned` / `inferred` (read back out of an existing system) / `healed` / `reconciled` / `imported` |
| `set_requirement_designation` | **write** | Designate a Requirement as a PROMISE THIS DESIGN PUBLISHES — a behavioural commitment a consumer may rely on — or back to INTERNAL intent nobody outside sees |
| `set_requirement_lineage` | **write** | Set where a Requirement came from — `original` (the stakeholder's own word), `decomposed` (a 1:1 split of a parent, normally set for you by `decomposes`), or `derived` (technical necessity nobody asked for, created by a design decision — pair it with governed_by to that Decision) |
| `set_requirement_status` | **write** | Set a Requirement's lifecycle status: `proposed` (the default) / `accepted` / `deferred` / `dropped` / `met` |

### Coherence — what the design says about itself

| tool | | what it does |
|---|---|---|
| `acknowledge_defect` | **write** | Accept a structural defect the user has judged fine, recording WHY |
| `acknowledge_gap` | **write** | Accept a gap the user has judged fine, recording WHY |
| `acknowledge_gaps` | **write** | Acknowledge MANY gaps in one call — the bulk form of acknowledge_gap |
| `apply_heal` | **write** | Apply a reviewed HealProposal atomically (rigid mode = no-op) |
| `confirmation_ledger` | read | The confirmation ledger (BL-35): for every capability with built artifacts, when was its claim last checked against reality, and what was the answer — drift events and whether each was resolved, accept claims split into design_holds vs design_updated, first baselines counted apart from both (they are not accepts), clean-reconcile confirmations with when they last happened, design edits on the record, and a state per capability: drifting (an observed divergence is unanswered), confirmed (examined, with the claim history visible), or unexamined (nobody has ever looked — NOT the same as confirmed) |
| `consumption_report` | read | What did this design BUILD that it records no consumer for? |
| `design_regions` | read | What parts this design has, so a session that holds NOTHING can pick where to stand |
| `detect_defects` | read | Detect structural defects the machine can repair (HEAL) |
| `detect_gaps` | read | Find gaps in the design to ask the human about (DETECT) |
| `dimension_drift` | read | Quality-dimension drift for one target node |
| `dimension_drifts` | read | All declining quality dimensions across the design, worst first |
| `evaluate_allocation` | read | Evaluate how capabilities are allocated across components |
| `granularity_report` | read | Does the BUILD separate what the DESIGN separates? |
| `graph_report` | read | The 'what should I look at?' rollup report (SYNTHESIZE) |
| `graph_report_markdown` | read | The graph report rendered as Markdown |
| `hierarchy_issues` | read | Decomposition/hierarchy issues (matryoshka level checks) |
| `ility_report` | read | What can this design's graph actually SAY about the quality axes — the 'ilities'? |
| `loop_status` | read | The coherence loop's outstanding debt, cheaply: what capture→detect→ask→decide steps are owed now, computed from graph state, never run history |
| `maturity_report` | read | Where does this design sit on the trajectory from FUNCTION to STRUCTURE? |
| `propagate_change` | read | Blast radius of a recorded ChangeEvent along the golden thread |
| `propagate_from` | read | Speculative blast radius from seed node ids (what would this touch?) |
| `propose_allocation` | read | Propose a capability→component allocation via Leiden clustering |
| `propose_heal` | read | Propose a HEAL plan (never mutates; review then apply_heal) |
| `reviewed_defects` | read | Structural defects that were reviewed and accepted, each with the reason given |
| `reviewed_gaps` | read | Gaps that were reviewed and accepted, each with the reason given |
| `surprising_connections` | read | Surprising cross-community couplings (mined from the graph) |
| `sync_status` | read | Has the SHARED RECORD moved since this graph last looked? |
| `vocabulary_coverage` | read | Which of the design VOCABULARY this design has ever actually used — node types, edge types, and properties on the types that have instances |
| `what_next` | read | Which decisions to settle next — a rough guide, not an ordering, for a design with more open questions than anyone can hold at once |
| `withdraw_defect_acknowledgement` | **write** | Withdraw a defect's acknowledgement, returning it to the open list |
| `withdraw_gap_acknowledgement` | **write** | Withdraw a gap's acceptance: the Decision is marked superseded (kept, not deleted) and the gap returns to the open list |

### Query — read the design back

| tool | | what it does |
|---|---|---|
| `create_edge` | **write** | Create an edge of any schema type between typed endpoints |
| `create_edges` | **write** | Create MANY edges in one call — the bulk form of create_edge, and so of every typed helper built on it: contains, contain_component, satisfies, allocate, realizes |
| `create_node` | **write** | Create a node of any schema type with a property object |
| `create_nodes` | **write** | Create or update MANY nodes in one call — the bulk form of create_node |
| `delete_edge` | **write** | Delete one edge by type and endpoint ids (true if it existed) |
| `delete_node` | **write** | Delete a node by type and id (true if it existed) |
| `describe_schema` | read | Discover the design vocabulary before writing to it: which node types exist, which properties they require, and which edge types may join two given types |
| `find_tools` | read | Find the reflow2 tool for a job you can describe but cannot name — 'how do I record that a file implements a capability?', 'what shows me the blast radius?' |
| `get_node` | read | Fetch a node by type and id — `{node: {...}}` when present, `{node: null}` when absent |
| `scan_nodes` | read | List nodes of a type |
| `search_design` | read | Find design nodes by what they say, when you don't know their ids — 'what does the design say about persistence?', 'is there already a requirement about latency?' |

### Assurance — checks, evidence and confirmation

| tool | | what it does |
|---|---|---|
| `add_verification` | **write** | Record a Verification — a check that something meets its intent |
| `calibrated_against` | **write** | Record that a value was FITTED to a piece of evidence, so that same evidence can no longer count as its validation (CALIBRATED_AGAINST) |
| `coverage_report` | read | What has the design never been told about? |
| `evidence_report` | read | Where did each capability's evidence actually come from? |
| `performed_in` | **write** | Record WHERE a check was actually carried out (PERFORMED_IN) |
| `reconcile_verification` | **write** | Compare what a real test run REPORTED against what each Verification records — the P4 reconcile, last of the three feedback loops (BL-30): reconcile_artifacts asks about the code, this about the outcomes, reconcile_deployment about what runs |
| `set_evidence_scope` | **write** | Record what a check HELD FIXED and what it VARIED for one claim — the input scope of its evidence |
| `set_verification_kind` | **write** | Set a Verification's kind: `verification` (built right — checks the spec) or `validation` (the right thing — checks the operational intent) |
| `set_verification_status` | **write** | Set a Verification's outcome (planned/passing/failing/skipped/blocked), preserving what the check is |
| `verifies` | **write** | Link a Verification to what it checks (VERIFIES) |

### Build — what exists on disk, and whether it still matches

| tool | | what it does |
|---|---|---|
| `add_artifact` | **write** | Create an Artifact node — a real deliverable (file/spec/doc) that lives outside the graph, pointed to by `location` |
| `declare_dependency` | **write** | Declare which version of ANOTHER DESIGN this one depends on — the pin a seam analysis is taken AS OF |
| `documents` | **write** | Link an Artifact to the node it DOCUMENTS (describes without implementing): a design doc, ADR, README, runbook, instruction file or diagram |
| `link_artifact` | **write** | Register a real file against the design WITH provenance, atomically: Artifact + a provenance Fragment (YIELDED) + a REALIZES edge to the Capability/Component it implements |
| `realizes` | **write** | Link an Artifact to the Capability/Component it REALIZES (implements) |
| `reconcile_artifacts` | **write** | Check the design against what was actually built |
| `reconcile_dependencies` | read | Check the declared dependencies against what the build ACTUALLY resolves, and return the reflow2.toml manifest |
| `set_artifact_checksum` | **write** | Accept an artifact's current content as the new drift baseline — a two-sided decision |
| `set_artifact_checksums` | **write** | Accept MANY drift baselines in one call — the bulk form of set_artifact_checksum, which was 244 consecutive calls across 22 sessions of recorded usage |
| `set_artifact_intent` | **write** | Declare what an Artifact node stands for and how its content behaves — the two things only its author can say |

### Time — epochs, change, and what a claim was true of

| tool | | what it does |
|---|---|---|
| `add_change_event` | **write** | Create a ChangeEvent (seed for propagate_change) |
| `add_epoch` | **write** | Create a `DesignEpoch` that HAS HAPPENED — a point on the time axis you are recording, which is what an epoch has always meant here |
| `arrival_delta` | read | What was PLANNED for an epoch or release against what was actually DELIVERED — the planned-versus-delivered delta (dec:arrival-delta) |
| `changelog_view` | read | Derive a Keep a Changelog-shaped DRAFT between two moments of THIS design — compare_designs' sibling: that one compares two as-designed records, this one compares two moments of one design and renders the difference in the format the industry already reads |
| `pin_at_epoch` | **write** | Pin any node to a DesignEpoch (AT_EPOCH) — e.g |
| `plan_epoch` | **write** | Create an Epoch that has NOT happened yet — a claim about the future rather than a record of the past, and the forward half of the time axis (req:epochs-can-be-planned) |
| `precedes` | **write** | Order one DesignEpoch after another (earlier PRECEDES later) — the chain axis Z exists to record |
| `record_change` | **write** | Record a change to a node in an epoch (snapshots the prior state) |
| `schedule_for` | **write** | Schedule a Requirement, Capability or QUESTION against the moment it is DUE — the satisfaction schedule, which is what makes a roadmap answerable (req:epochs-can-be-planned) |
| `set_epoch_status` | **write** | Move an Epoch between `planned` and `arrived` |

### Operate — releases, environments, resources, readiness

| tool | | what it does |
|---|---|---|
| `add_environment` | **write** | Record an Environment — where a Release runs: a cloud region, a lab bench, a physical site |
| `add_readiness` | **write** | Record an OBSERVED technology-readiness level (TRL or MRL, 1-9) for an enabling technology — the input fact a derived roadmap is computed from (BL-68) |
| `add_release` | **write** | Record a Release — a packaged, operable version: a container image, a published package, a manufactured build |
| `add_resource` | **write** | Record a Resource the built thing needs — a database, a queue, a secret, a GPU, power, bandwidth |
| `deploy_to` | **write** | Deploy a Release to an Environment (planned/active/rolled_back) |
| `forecast_readiness` | **write** | Record a PROJECTED readiness level valid from a future epoch — 'this converter reaches TRL 7 in 2035' — as a TemporalFact marked basis=forecast (BL-68) |
| `gate_on` | **write** | State that an increment cannot deliver until an enabling technology reaches a given readiness level — the JUDGEMENT half of BL-68, and the one reflow2 will never make for you |
| `readiness_report` | read | The DERIVED roadmap for one increment (BL-68): the earliest epoch by which every technology it is GATED_ON clears the level demanded of it, with the reason named — 'cannot deliver before 2035, because the converter is TRL 3 today, projected 7 at 2035, and this increment needs 7' |
| `reconcile_deployment` | **write** | Compare what is observed RUNNING against what DEPLOYED_TO declares — the as-fielded reconcile, sibling of reconcile_artifacts one phase later (BL-9) |
| `release_includes` | **write** | Record that a Release ships an Artifact or Component (INCLUDES) — the as-released view |
| `release_includes_all` | **write** | Derive a Release's whole INCLUDES manifest from the design in one call: every Artifact and every Component, with each artifact's current checksum frozen as shipped |
| `release_report` | read | The as-released view (BL-34): what a Release actually shipped — artifacts with their frozen cut-time checksums, components, the capabilities that build covers, the built capabilities it leaves out, and where it is deployed |
| `require_resource` | **write** | Record that a Component or Release needs a Resource, with how critical it is (optional/recommended/required) |

### Exchange — comparing, merging and linking whole designs

| tool | | what it does |
|---|---|---|
| `alternatives_for` | read | List the alternatives registered under a decision point (BL-70) — the Artifact pointers GOVERNED_BY the Decision, with their export locations |
| `analyze_alternatives` | read | Compare parallel design alternatives on the same measures — an analysis of alternatives (BL-70) |
| `apply_merge` | **write** | Apply a resolved three-way merge into the live design — the write side of merge_designs (BL-80) |
| `certify_preservation` | read | Decide whether a restructuring PRESERVED FUNCTION — compare_designs' verdict-bearing sibling |
| `collapse_decision` | **write** | Collapse a decision point (BL-70): choose the winning alternative |
| `compare_designs` | read | Compare two as-designed records — the design-vs-design sibling of the reconcile family, which only ever compares design against reality |
| `compose_and_analyse` | read | Analyse THIS design together with another one — a dependency, a partner system — and report what only shows up when both are present |
| `export_graph` | read | The whole design as one portable document — every node and edge, sorted so two exports of an unchanged graph are byte-identical |
| `export_surface` | read | Export ONLY the published surface — the contracts others are entitled to rely on, and nothing internal |
| `import_graph` | **write** | Load an exported design into this graph |
| `merge_designs` | read | Propose a three-way merge of two divergent designs against their common ancestor — compare's write-side sibling (BL-80) |
| `mirror_surface` | **write** | Mirror ANOTHER design's published surface into this graph as foreign nodes carrying the coordinate that says whose they are — which design, at what content hash, when |
| `mirrors` | read | The designs this one is composed with, and the version each was pinned to: project id, source graph, surface content hash, and when the mirror was taken |
| `pair_designs` | read | Compute the seam between this design and another by COMPLEMENTARY ROLE, instead of hand-asserting which boundaries correspond |
| `recall_resolutions` | read | Recall recorded conflict resolutions (rerere) by their content keys — the advisory half of merge (BL-80 #5) |
| `register_alternative` | **write** | Register an alternative under a proposed decision point (BL-70): a lightweight Artifact pointer that names where the alternative's design export lives (branch-by-file), GOVERNED_BY the Decision and CONTRADICTS its siblings |
| `seam_report` | read | Compare paired boundaries across a seam and say where two designs DISAGREE — the check the ordinary detectors cannot do, because they reason about structure and a contract mismatch is a comparison of PROPERTIES ACROSS A PAIR |
| `set_decision_status` | **write** | Set a Decision's lifecycle status — proposed / accepted / superseded / rejected (BL-70) |

### Ask — turning findings into questions a person answers

| tool | | what it does |
|---|---|---|
| `answer_question` | **write** | Record what the user said in reply to a question, closing it |
| `gap_to_prompt` | **write** | Phrase a gap as a plain question via the ambient agent |
| `gaps_to_prompts` | **write** | Phrase MANY gaps as plain questions in one handshake — the bulk form of gap_to_prompt, and the read half of the detect→ask→acknowledge round trip |
| `open_questions` | read | Questions already put to the user that still bear on something open, with the wording they saw |
| `withdraw_question` | **write** | Withdraw a question asked in error or overtaken by events |

### Coordination — who holds which region

| tool | | what it does |
|---|---|---|
| `claim_region` | **write** | Take a region of the design in hand so colleagues can see it is held: `contributor_id` claims everything within `depth` hops of `seed_id` |
| `claim_report` | read | Who holds what, and where two people are working the same ground |
| `mint_seat` | read | Mint a seat: a durable name for THIS session, to pass as `seat` on the tools that record who is working (claim_region) |
| `release_claim` | **write** | Let a claimed region go |

### Ingest — reading an existing corpus in

| tool | | what it does |
|---|---|---|
| `ingest_corpus_step` | **write** | Turn a WHOLE FOLDER of documents into one design, with you as the model (cap:corpus-ingest) |
| `ingest_step` | **write** | Extract a design from freeform text, with YOU as the model — no LLM provider is involved |

### Skills — reaching the served procedures

| tool | | what it does |
|---|---|---|
| `describe_designs` | read | Say what design lives at each given path, WITHOUT opening or writing anything — the sibling of design_identity, which answers only for the design THIS session is bound to |
| `design_identity` | **write** | Which design this graph holds: its durable id and its human label |
| `get_instructions` | read | How to work THIS project with reflow2: the loop, the standing rules, and what to do first on an existing design |
| `get_skill` | read | Read one reflow2 skill in full, by name (see list_skills) |
| `list_skills` | read | List the reflow2 skills this server carries — name and the full description an agent matches on to decide whether a skill applies |

---

## Honest limits of this page

- **It is a snapshot and it will drift.** Nothing regenerates it. The server is the authority:
  `list_skills` and `tools/list` are always current, and this file is only as fresh as the last
  person to touch it.
- **First sentences lose things.** Several tools carry a refusal or a precondition in their second
  paragraph that matters more than the summary here — `review_relations` refuses when given
  neither a relation nor a note, `set_capability_delivery` never records *whether* something was
  delivered. Read the full description before using one you have not used before.
- **A tool count is not a capability count.** 155 tools is a surface area, not an amount of value;
  several are bulk forms of a single idea (`acknowledge_gap` / `acknowledge_gaps`), and a few
  exist to make a distinction visible rather than to do work.
