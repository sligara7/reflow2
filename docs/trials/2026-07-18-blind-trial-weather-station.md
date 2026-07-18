<!-- Preserved verbatim. Primary evidence, not a summary — do not tidy. -->

# Blind trial, 2026-07-18 — backyard weather station

**Method.** A general-purpose agent was given a clean consumer repo (`AGENTS.md`, the skills, a
one-shot CLI onto the graph) and a user's plain-English brief. It had **no access to reflow2's
source** and was not told it was being evaluated. It was asked to do the design work the way
`AGENTS.md` says, start building, and then write down what fought it.

**Outcome.** A design (16 requirements, 14 capabilities, 9 components, 6 interfaces, 6 decisions)
plus ~1,300 lines of working Python, 27 tests, and 13 artifacts linked with checksums. Six of the
findings below became fixes the same day; the rest are in [../backlog.md](../backlog.md).

Its own verdict is the last section, and worth reading first.

---

# NOTES — running the reflow2 loop on this brief

Blunt version, as asked.

## What I did

**GENESIS.** Empty graph, so `genesis` first, then seeded the brief into 15 Requirements and 13
Capabilities with `satisfies` links, no Components — as the skill insists. That restraint paid
off: the first `detect_gaps` returned `concept_without_design`, which is exactly the question I'd
otherwise have answered silently by inventing a component tree in my head.

**DETECT.** Ran the `gap_to_prompt` handshake on both opening gaps. Wrote the resulting questions
into `QUESTIONS.md` (7 of them), answered each with my best judgement, and recorded every
assumption in the graph rather than only in the code.

**Structure.** 9 Components, 6 Interfaces with both sides recorded, 13 allocations, 6 Decisions
with rationale, 2 Releases / 2 Environments / 4 Resources.

**Build.** ~1100 lines of Python (stdlib only) + a single-file phone client. 27 tests, all
passing. Verified end-to-end for real, not just by unit test: ran the collector, pushed 40
readings through the UDP uplink from the simulated outdoor unit, confirmed the API and HTML
served, then killed the collector, sampled 5 readings during the "outage", restarted, and watched
all 5 arrive with no sequence gaps. That last one is the only test of `req:no-dropped-readings`
that means anything.

**As-built.** 13 artifacts linked with sha256 checksums; `reconcile_artifacts` reports 13
unchanged, 0 drift. 5 Verifications, 2 passing, 3 planned (the ones needing a physical unit).

**Final state:** 0 structural defects, 13 gaps — 10 of which are the `unexpected_coupling` false
positive described below, and 3 of which are correct ("nothing builds the sensors / power /
enclosure" — true, that's hardware).

## Design decisions I'd want you to check

- **I refused to pick the radio (Q3).** Instead I made `ifc:reading-uplink` a transport-agnostic
  frame and put the UDP implementation behind it. I then ran `propagate_from` on that interface to
  check the isolation was real, and at distance 1 it touches only `cmp:outdoor-controller`,
  `cmp:collector-service`, and `reading.py`. That's the whole cost of deciding later. Genuinely
  useful — I would not have bothered to confirm this without the tool.
- **The rain accumulator is cumulative, not per-interval.** If a frame is lost, cumulative
  self-heals and a delta loses that rain forever. This is the one place where the brief's "don't
  drop readings" translated into a specific wire-format choice.
- **`req:local-only` and `req:rain-alert` genuinely conflict.** A phone will not wake for a
  LAN-only web page. I did not paper over it — `cmp:notifier` is a plug, the default backend
  writes to a local file (deliberately useless for waking a phone, so the unanswered question
  stays visible), and Q4 asks you to choose.
- **HEAL surfaced something real.** Among nine single-point-of-failure warnings, eight were the
  inevitable shape of a one-of-everything personal system and I recorded a Decision declining
  redundancy. But `cmp:reading-store` was different in kind — its failure destroys data that
  cannot be re-collected — so I added `req:store-backup`, `cap:backup-history`, an online SQLite
  backup, and a test that the backup is actually restorable. **That requirement is not in your
  brief and I would not have thought of it unprompted.** Best single moment of the loop.

## What fought me

**`detect_defects` "single point of failure" is noisy on a small design, and then goes silent for
the wrong reason.** Nine SPOF warnings on a nine-component system, most of them meaningless
("the power subsystem is a single point of failure" — yes, there is one solar panel). Worse:
after I connected the Release nodes to their Components, **all 15 defects vanished at once**,
including every SPOF. Nothing about the design's actual fragility changed — I added bookkeeping
edges. If adding two edges between admin nodes can take a design from 15 warnings to 0, the
detector is measuring graph connectivity, not engineering risk, and I don't trust either the 15
or the 0. AGENTS.md says structural detectors are "selective"; they didn't feel selective here.

**`unexpected_coupling` fires on correct architecture, and can't be silenced.** Ten of my
thirteen remaining gaps are "component X and interface Y sit in separate parts of the design yet
are directly coupled". That is the definition of an interface. The community detector clusters
each component with the capabilities it owns, so any Interface — which by construction joins two
clusters — always looks surprising. AGENTS.md tells me to model an Interface *every time* two
components talk, and then the detector penalises me for every one I model. I recorded a Decision
saying these are intentional; **the gaps did not go away.** There appears to be no way to
acknowledge a gap. So the gap list can never reach zero, which trains you to ignore it — the
exact failure mode the tool exists to prevent. A "reviewed / accepted" state, or `governed_by` a
Decision suppressing the gap, would fix this.

**No way to discover the schema.** `create_edge` takes any edge type but nothing lists them. I
needed to connect a Release to the Components it packages and had to *brute-force fourteen
guesses* to find out that `PACKAGES`, `CONTAINS`, `REALIZES`, `DEPLOYED_TO`, `INCLUDES`,
`RELEASES`, `PACKAGED_AS`, `DELIVERS` and others are all invalid, `PRODUCES` exists but can't
connect those two types, and `DEPENDS_ON` works. The error "Unknown edge type: PACKAGES" tells me
I'm wrong without telling me what's right. **A `list_schema` / `--describe schema` tool would have
saved me twenty minutes of guessing.** And I'm still not confident `DEPENDS_ON` is semantically
what I meant — I used it because it was the one that validated, which is precisely the kind of
silent accommodation this project says it's against.

**`set_verification_status` isn't in the AGENTS.md tool list** (it's in `--list`). Minor, but I
only found it because I ran `--list` first.

**`unverified_capability` fires per-Artifact, not per-Capability, despite the name.** I linked
verifications to Capabilities, re-ran detect, and got eleven new gaps named "Nothing verifies
reading.py". Nothing said Verifications also need `VERIFIES` edges to Artifacts. Not hard once
seen, but the gap name actively misled me.

**The skills are good; AGENTS.md undersells step 3.** The link-artifacts skill warns that the gap
count *rises* after the first `link_artifact`. That warning is doing a lot of work and it's buried
one level down — without it I'd have assumed I'd broken something.

**Nothing to do with reflow2:** `pkill -f 'main.py'` killed my own shell twice, because the
harness's bash wrapper has my command text in its own command line, so the pattern matched
itself. Cost me two round trips to diagnose. Worth knowing.

## What I wanted to do and couldn't

- **Acknowledge or suppress a reviewed gap.** See above. This is my biggest ask.
- **Model the outdoor unit as a hierarchy.** `contains` is Project→child only, so
  `cmp:outdoor-controller` and `cmp:power-subsystem` are siblings of `cmp:phone-client` rather
  than children of an outdoor-unit assembly. The design graph is therefore flatter than the real
  thing. `hierarchy_issues` returned `[]` throughout, which I suspect means it never had a
  hierarchy to check. CLAUDE.md mentions `Component.level` as schema-present-but-no-code, so I
  think this is a known deferral — but from inside the loop it just looks like I can't express
  an assembly.
- **Record that a Requirement is provisional.** `req:link-choice-open` and
  `req:alert-delivery` are open questions, not requirements, but the only place to say so was in
  the statement text as the word "ASSUMED" / "OPEN DECISION". A `status` property on Requirement
  would let `detect_gaps` chase unanswered questions, which feels like the tool's whole purpose.
- **Attach the QUESTIONS.md questions to their gaps.** I ran the `gap_to_prompt` handshake and
  got good questions out, then had to copy them into a Markdown file by hand. The graph has no
  memory that a question was asked and is awaiting an answer — so on my next session I'd re-derive
  the same gaps and re-ask the same questions, which is the stateless-agent problem reflow2 is
  supposed to solve.

## Honest overall

The golden thread and the interface discipline earned their keep — `propagate_from` proving the
radio decision is cheap to defer, and HEAL surfacing the backup requirement, are both things I
would not have got to alone. The detectors that fire on shape rather than meaning
(`unexpected_coupling`, `single_point_of_failure`) produced far more noise than signal at this
scale, and because acknowledged gaps never clear, the gap list is already something I'd learn to
skim rather than read. That's the thing I'd fix first.
