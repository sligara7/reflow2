# reflow2 feedback

Friction encountered while modelling the US federal government with reflow2.
Newest session first. Each item records what was attempted, what happened, and
what would have worked better.

---

## Session 1 — 2026-08-15 — genesis, US government three-branch model

Context: seeding a descriptive systems model of an existing institution
(three branches, ~93 requirements drawn from the Constitution, landmark case
law and structural statutes, 38 capabilities, 17 inter-branch interfaces).
Domain is governance, not software. Project mode `flexible`.

### 1. `add_component` takes `description`; `create_node` on Component requires `purpose`

**Severity: low, but it costs a full round trip on every bulk write.**

`add_component`'s schema documents a `description` parameter ("What this part
is for"). `create_nodes` with `node_type: "Component"` rejects `description`
and demands `purpose`:

```
Validation error on Component.purpose: required property is missing
```

Because `create_nodes` is all-or-nothing, this failed a six-node batch on four
items. Nothing in `describe_schema {node_type: "Component", required_only:
true}` was consulted first — but the point is that the *helper* and the
*generic* path disagree on the field name for the same concept, so reading one
does not prepare you for the other.

Every other type checked was consistent: Capability takes `description` in
both paths, Requirement takes `statement` in both.

**Suggested fix:** either accept `description` as an alias on Component, or
rename the helper's parameter to `purpose`. Failing that, mention the mismatch
in `add_component`'s description.

### 2. `Interface.medium` has no value for an institutional or legal contract

**Severity: low. Workaround exists and is reasonable.**

Guessed `medium: "process"` for "presentment and veto". Rejected:

```
invalid enum value 'process', expected one of ["REST", "gRPC", "event",
"graphql", "cli", "library", "data", "mechanical", "electrical", "human",
"unspecified"]
```

`human` turned out to be a defensible fit for all seventeen check-and-balance
interfaces, so this is not blocking. But the enum reads as software-plus-
hardware with `human` as an afterthought, and reflow2's own pitch is "design
anything". A constitutional procedure is not really a human-factors interface;
it is a *procedural* or *legal* contract between institutions. Worth
considering `procedural` / `contractual` as first-class values.

Credit where due: the error message listing every valid value is exactly
right, and made the fix a single retry.

### 3. The big one: requirements that *forbid* have nowhere to live

**Severity: high for this domain. This is a modelling-vocabulary gap, not a bug.**

The `unsatisfied_requirement` detector assumes every requirement is discharged
by a Capability — something the system *does*. That holds well for powers
("Congress shall have power to lay and collect taxes" ← `cap:levy-taxes-and-
borrow`). It breaks completely for the large class of constitutional
requirements that constrain what the system *is* or *may not do*:

- "Congress shall pass no bill of attainder and no ex post facto law"
- "The privilege of the writ of habeas corpus shall not be suspended..."
- "No title of nobility shall be granted by the United States"
- "The powers not delegated... are reserved to the states respectively"
- "No person except a natural born citizen is eligible to the office of President"
- Speech or Debate immunity

Nothing delivers these. They are boundaries on everything, owned by nobody.
Left as Requirements they are reported as gaps forever, which is precisely the
failure mode AGENTS.md warns about — an open list that can never reach zero
gets skimmed.

**The available node types do not fit:**

| type | why not |
|---|---|
| `Constraint` | requires `quantity` / `limit` / `direction`. Numeric budgets only. "No ex post facto law" has no unit. |
| `DesignRule` | structurally perfect — `name` + `statement`, and `CONSTRAINS` reaches any node. But its own hint says *"a rule you picked, not a limit imposed on you"*, which is the exact opposite of a constitutional prohibition. Using it would make the graph say something false about where the rule came from. |
| `Requirement` | semantically right, but guarantees a permanent gap. |

**What was done:** left them as Requirements and raised the question with the
user through the `gap_to_prompt` handshake rather than silencing it, on the
principle that reshaping the design until the complaint stops is how a graph
bends into fiction. Eleven such gaps are still open and deliberately so.

**Suggested fix:** either
(a) relax `Constraint` so `quantity`/`limit` are optional, making it a general
    "limit imposed on you" node alongside `DesignRule`'s "rule you picked"; or
(b) broaden `DesignRule`'s hint to cover imposed rules, and add a property
    distinguishing chosen from imposed; or
(c) add a `polarity` / `modality` property to `Requirement`
    (`shall` vs `shall not`) and teach `unsatisfied_requirement` that a
    prohibition is satisfied by the *absence* of a violating capability rather
    than the presence of a delivering one.

(c) is the most interesting: a prohibition really is checkable against a
design — you look for something that VIOLATES it. reflow2 already has a
`VIOLATES` edge type and does not appear to use it for this.

### 4. `Capability.status` conflates "exists in the world" with "we built it"

**Severity: medium for descriptive models. Cosmetic for greenfield software.**

The status ladder is `planned` / `in_progress` / `realized` / `verified`. For a
model of an institution that has operated since 1789, `planned` is plainly
false. `realized` was used — but `realized` in reflow2 means *an Artifact
realizes it*, and there are no artifacts and never will be for this phase.

So both readings are wrong in different directions, and the `build_without_
verification` and `design_without_build` gaps fire on a design that is
complete and correct for its phase. Both were acknowledged with reasons, which
is the right escape hatch and worked cleanly.

The `adopt` skill presumably hits a softer version of this — recording a
running codebase — but there at least the artifacts exist. A model whose
subject is real but whose *implementation* is not this project's to build has
no honest value on this ladder.

**Suggested fix:** a `descriptive` or `observed` status, or a project-level
mode meaning "this design describes something that exists rather than
specifying something to build", which would suppress the build/deploy/verify
phase detectors wholesale rather than one acknowledgement at a time.

Follow-on: acknowledging the `build_without_verification` gap moved it out of
`detect_gaps`, but `loop_status` still reports *"38 capability(ies) claim
realized/verified with no passing check"* and so never goes `clean`. The two
are different debt classes computed independently, which is defensible — but
the practical effect is that a design in this shape can never reach a quiet
`loop_status`, and the AGENTS.md argument for keeping lists at zero applies
just as much here. `acknowledge_gap` has no counterpart for this debt class.

### 5. `detect_defects` rates the design's central mechanism a *critical* defect

**Severity: high. The most interesting finding of the session.**

With the three branches and their seventeen check-and-balance interfaces
modelled, `detect_defects` reported:

```
severity: critical
category: circular_dependency
circular dependency: cmp:executive-branch → cmp:judicial-branch →
cmp:executive-branch — every hop is a contract via ifc:clemency,
ifc:judicial-appointment, ifc:judicial-review-of-executive-action;
no DEPENDS_ON edge is involved
suggested_fix_type: break_cycle
```

That cycle is not a defect. It is the entire point. The executive appoints
judges and pardons those the courts convict; the courts review executive
action. Madison designed the mutual dependence deliberately so that ambition
would counteract ambition. `break_cycle` here would mean deleting the
separation of powers.

**reflow2 gets two things right and one thing wrong:**

- Right: it notices no `DEPENDS_ON` edge is involved and says so unprompted.
  That is a genuinely useful distinction — contract cycles and dependency
  cycles are different animals.
- Right: `acknowledge_defect` exists, takes a reason, stores it as a real
  Decision, and expires the review if the shape changes. The escape hatch is
  well built.
- Wrong: having established that every hop is a contract rather than a
  dependency, it still rates the finding **critical** and still suggests
  **break_cycle**. Severity and suggested fix are computed as though the
  distinction it just drew did not matter.

**Suggested fix:** downgrade contract-only cycles below dependency cycles by
default, and offer a fix type other than `break_cycle` for them — something
like `confirm_intentional`. A "critical" that the correct response is always
to accept trains people to skim criticals.

The same applies to the four `contradiction` warnings, all four of which were
edges deliberately authored to record live, unresolved constitutional
conflicts (War Powers Resolution vs Commander in Chief; nondelegation;
removal power vs civil service tenure; pardon vs finality of judgment).
`CONTRADICTS` carries an `alignment` property and an `evidence` string — a
contradiction authored *with* evidence and a sub-1.0 confidence is a claim
somebody made on purpose, and reads differently from one a heuristic inferred.
`DUPLICATES` already draws exactly this distinction with its `basis`
(`asserted` vs `suspected`) and reflow2 is rightly proud of it. `CONTRADICTS`
would benefit from the same field.

### 6. Small things that went right, recorded so they don't get regressed

- Batch failure reporting. `create_nodes` listing *every* failure rather than
  the first, and writing nothing, made a 32-node batch safe to attempt.
- `describe_schema {from, to}` ranking edges that genuinely model a pair above
  ones accepting it via wildcard. `ALLOCATED_TO` was found in one call.
- `Requirement.source` plus `provenance` being separate fields is exactly the
  distinction this project needed: `source` carries the citation
  ("US Const. art. I, sec. 7, cl. 2"), `provenance` carries whether a human
  stated it or a model inferred it. The user explicitly asked to be able to
  tell those apart later, and the schema already supported it.
- `CONTRADICTS` with `alignment: opposing|supporting` let the model record
  genuine live constitutional tensions (War Powers Resolution vs Commander in
  Chief; nondelegation) as first-class content instead of prose buried in a
  description. This is the single most valuable thing reflow2 gave this model.
- The `gap_to_prompt` two-step handshake produced better questions than were
  put in, and records them so a later session does not re-ask.
