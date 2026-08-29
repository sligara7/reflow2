#!/usr/bin/env python3
"""reflow2 loop nudge — the trigger half of the coherence loop (BL-74 rung a).

The field lesson this exists for: told to "use reflow2 extensively", an agent
under operational load kept the graph's *bookkeeping* current through the raw
write tools while the capture→detect→ask→decide loop silently stopped — and
"under load, a mood loses to whatever has a trigger." The in-band halves are
already built: write results carry a `loop_hint`, and `loop_status` is the one
cheap call that says what the loop is owed. This script is the out-of-band
trigger that fires them, wired to the harness's own events (Claude Code hooks;
the snippet lives in the kit's AGENTS.md, step 0a).

One script, three events, read from the hook's stdin JSON:

- **SessionStart** — prints the orientation line into the session's context:
  orient on the graph first, and `loop_status` is the in-flight pulse-check.
- **PostToolUse** — two things it counts per session, from the tool name:
  - reflow2 graph *writes* — a loop check (`loop_status` / `detect_gaps` /
    `detect_defects`) resets the count. Resolve steps (answering, acknowledging,
    dispositioning drift) are loop participation: neither debt nor a full check
    — ignored. *Any* reflow2 call, even a read, marks the session as having
    engaged the design brain at all.
  - harness file *edits* (`Edit` / `Write` / `MultiEdit` / `NotebookEdit`) —
    counted only to catch the session that edits code while making **zero**
    reflow2 calls: the total-bypass blind spot (BL-90). This is upstream of the
    write-nudge — the agent that ignores the design brain entirely is exactly
    the one the write count never sees.
- **Stop** — the backstop, blocking ONCE with the reason the agent needs:
  - graph writes finished with no loop check → "call loop_status", or
  - code was edited and a change recorded, and `propagate_change` was never
    called → "the ChangeEvent is bookkeeping; run impact-check" (BL-163), or
  - the session never touched reflow2 at all and edited enough files → "the
    graph was never consulted; start with loop_status, impact-check before
    further edits, link-artifacts after", or
  - the GRAPH says this session raised a debt count and left it there → "open
    gaps went 7 → 10; run detect-and-ask" (`cap:stop-nudge-asks-the-graph`).
    This one is not counted from the tally at all — see the block below.
  A second stop (`stop_hook_active`) always proceeds — a nudge that can loop
  forever is a hostage-taker, not a trigger. The cases are mutually exclusive:
  any graph write means reflow2 was touched, so the bypass case cannot also be
  armed, and the BL-163 case requires a recorded change, which is a write.

  **Order is what the middle case adds.** The other two ask whether the loop ran
  at all; that one asks whether it ran in the right ORDER — recording a change
  after editing the code satisfies "a ChangeEvent exists" while being exactly
  the bookkeeping-after the hook's own message says is not the loop.

🛑 THE GRAPH IS NOW READ, AND THIS PARAGRAPH USED TO SAY IT COULD NOT BE. The
old text — "the session's own MCP server holds the single-writer lock" — was
true before the shared server and is false now: the shared server answers
stateless MCP over the URL in `.reflow2/graph.server.json`, and a read costs
nothing but TIME. Correcting it matters more than the feature it blocked,
because for months the obstacle was read as a CAPABILITY limit when it was a
LATENCY limit, and those take opposite fixes. Measured: `loop_status` is 22.6s.
So the read happens in a DETACHED process (`tools/graph_probe.py`) whose answer
a later stop collects — never inline, because a Stop hook that blocks for 23
seconds is worse than no hook. `dec:the-stop-hook-asks-the-graph-asynchronously`.

Everything ELSE here still counts events rather than asking, and one limit of
that survives untouched: the hook cannot know which edited files are
design-relevant, so the bypass backstop stays blunt (a count threshold,
once-only) on purpose.
State is one small JSON per session under `.reflow2/loop-nudge/` (gitignored
with the rest of `.reflow2/`). A hook must never break a session: any failure
here warns on stderr and exits 0.

Stdlib only, no arguments needed. Three thresholds, all env-tunable:
`REFLOW2_LOOP_NUDGE_THRESHOLD` (default 1) — unchecked graph writes before the
Stop backstop fires; `REFLOW2_LOOP_NUDGE_EDIT_THRESHOLD` (default 3) — file
edits in a zero-reflow2 session before the bypass backstop fires;
`REFLOW2_LOOP_NUDGE_PROPAGATE_THRESHOLD` (default 1) — propagate calls that a
session with recorded changes and edited files must reach to stay silent.
"""

from __future__ import annotations

import json
import os
import sys
import time
from pathlib import Path

# Ops that ARE the loop check — seeing one clears the debt counter.
LOOP_OPS = {"loop_status", "detect_gaps", "detect_defects"}

# Graph writes beyond the add_/create_/delete_ prefixes. Unknown ops fall
# through to "ignored" — this is a backstop, not an accountant, and a missed
# count only softens the nudge, never wrongs the user.
EXTRA_WRITE_OPS = {
    "allocate", "consumes", "contain_component", "contains", "deploy_to",
    "genesis", "import_graph", "link_artifact", "part_of_flow", "pin_at_epoch",
    "precedes", "provides", "record_change", "release_includes",
    "require_resource", "satisfies", "set_capability_status",
    "set_provenance", "set_requirement_status", "set_verification_status",
    "verifies",
}

# The harness's own file-write tools — counted only for the total-bypass
# backstop (BL-90). A session that touches reflow2 at all never trips it.
EDIT_TOOLS = {"Edit", "Write", "MultiEdit", "NotebookEdit"}

# ---- cap:skill-triggers: the SHAPES, not just the count ---------------------
#
# The nudge already knew HOW MUCH happened. These let it know WHAT happened, so
# it can name the skill the situation calls for instead of saying "call
# loop_status" into every situation alike.
#
# THE HARD CONSTRAINT, and it shaped the design: this script runs on every tool
# call, so a SHAPE has to be free. It is therefore only implementable if it is
# visible in the session's own op tally, and `design_present()` is deliberately
# a directory test rather than a query. That rules out the spec's "an artifact
# checksum drift names link-artifacts" as literally written: drift is a fact
# about the graph versus the disk, and nothing on this path can see either.
# What IS visible is the progression — a change recorded but as-built never
# touched — which is the same situation one step earlier, and is what the third
# shape keys on.
#
# 🛑 THE REASON CHANGED AND THE OLD ONE IS WORTH NAMING. This comment used to
# say the script "CANNOT READ THE GRAPH — the session's own server holds the
# single-writer lock". That stopped being true when the shared server landed,
# and the correction is not a footnote: for months the obstacle was read as a
# capability limit when it was a COST limit. The graph is readable; a read is
# 22.6 seconds, which is free on a detached process and ruinous on a hook that
# runs per tool call. So the shapes below still reason from the tally — not
# because they may not ask, but because THEY cannot afford to — and the asking
# happens once per stop, detached, in `graph_probe.py`.

# EVERY SET BELOW MUST NAME THE BULK FORM BESIDE THE SINGULAR ONE. BL-153
# shipped `create_nodes`, `create_edges`, `set_artifact_checksums`,
# `acknowledge_gaps` and `gaps_to_prompts` precisely so the common path stops
# being N calls — and these sets kept only the singular names, so a session that
# did everything right *through the new tools* tallied as having done none of
# it. That is [BL-152]'s shape (two halves of the served surface disagreeing)
# landing on the trigger that judges whether the loop ran, and it gets worse
# exactly as the bulk forms succeed. When a tool gains a bulk form, add it here.

# Recording that something moved, on the record, before it moves.
CHANGE_OPS = {"record_change", "add_change_event"}

# LOOKING at what the recorded change reaches. The distinction between this set
# and CHANGE_OPS is the whole of BL-163, and it is worth stating plainly because
# the two read as synonyms and are not: `add_change_event` and `record_change`
# are RECORDING ops — they say a thing moved — while `propagate_change` and
# `propagate_from` are the act of asking what the movement touches. impact-check
# is both, in that order.
#
# THE DEFECT THIS SET FIXES: the impact-check shape used to key on
# `edits > 0 and changes == 0` — it fired only when a session recorded NOTHING.
# So a session that edited code and then wrote its ChangeEvents up afterwards
# had `changes > 0` and the trigger stayed silent, while every one of those
# events was bookkeeping-after. The nudge's own message says "Bookkeeping is not
# the loop"; the trigger shipped beside it could not tell the two orders apart,
# because no op set counted looking at all. It checked a ChangeEvent's PRESENCE
# where it meant its PRECEDENCE.
#
# WHY PRESENCE-OF-PROPAGATE AND NOT LITERAL ORDERING. The honest key would be
# "propagated before the first edit", and that is deliberately NOT what this
# implements. A session legitimately works several items — editing for item 1
# while propagating for item 2 — and a strict ordering rule would fire on that,
# which is exactly the fire-on-correct-work failure BL-23 and BL-42 name and
# this family exists to avoid. Presence is the conservative proxy: it cannot
# catch an agent who propagates late, and it never accuses one who did the work.
# That limit is real and is stated rather than hidden.
PROPAGATE_OPS = {"propagate_change", "propagate_from"}

# Telling the design what is now on disk.
ARTIFACT_OPS = {
    "link_artifact", "set_artifact_checksum", "set_artifact_checksums",
    "reconcile_artifacts",
}

# Capturing intent — the ops that add new design that nobody has yet asked the
# gaps of. Deliberately NOT every write: bookkeeping (release_includes,
# pin_at_epoch, set_*_status) is not a capture and must not trip this.
CAPTURE_OPS = {
    "add_requirement", "add_capability", "add_component", "add_interface",
    "add_constraint", "add_actor", "add_flow", "add_resource", "add_environment",
    "genesis", "import_graph", "ingest_step",
    # The bulk forms. `create_nodes` carries whatever node types the caller
    # passed, so it counts as a capture on the same argument as `import_graph`:
    # new design arrived and nobody has asked the gaps of it yet.
    "create_nodes",
}

# The op that IS a gap pass. `loop_status` is the cheap pulse-check and is
# deliberately NOT enough here: it reports debt, it does not ask the user
# anything, and this shape exists because captured intent needs questions put.
# `gaps_to_prompts` is the bulk form of the handshake and counts for the same
# reason `detect_gaps` does — it is the step that puts the questions.
GAP_PASS_OPS = {"detect_gaps", "gaps_to_prompts"}

# ---- cap:session-artifacts --------------------------------------------------
#
# A diagram or mockup drawn during a session is the visual half of a Decision's
# rationale, and today it is thrown away when the session ends. The capability's
# own text says capture must be TRIGGERED rather than remembered — "a rule
# depending on the agent choosing to store its own diagram decays exactly as
# req:skill-use-survives-a-long-session measures" — and that it "belongs with
# the trigger work rather than beside it". So it is a fourth shape here.
#
# WHAT THIS CAN AND CANNOT DO, stated because the difference matters. The hook
# sees that a rendering was WRITTEN; it cannot see whether any Decision points
# at it, because that is a fact about the graph. So the hook supplies the
# TRIGGER and the agent supplies the FILTER — and the filter is the whole rule
# (`dec:` the link is the filter, Anthony 2026-07-31): a rendering is kept when
# a Decision or Capability actually points at it, so the store holds what
# someone will look at again rather than every intermediate. The nudge therefore
# names BOTH halves, including the instruction NOT to store an orphan.
RENDERING_SUFFIXES = (
    ".svg", ".png", ".jpg", ".jpeg", ".drawio", ".mmd", ".puml", ".dot", ".excalidraw",
)


# Loading a skill (`cap:skill-loads-are-counted`). `get_skill` only: it is the
# call that puts a skill's text in front of the agent. `list_skills` is
# discovery — knowing the menu exists is not reading the recipe, and counting it
# here would let a session tick the box without opening anything.
#
# THE COUNT IS SESSION STATE, NOT GRAPH STATE, and that is the whole of the
# `dec:adoption-is-reported-the-loop-still-computes` line. It lives in the tally
# file beside the other per-session counters, is never written into the design,
# and is read by nothing that computes loop debt. `dec:loop-status-state-not-history`
# governs what the LOOP may reason from; this is the KIT reporting a fact about
# itself, and the two stay separate because the data never crosses over.
SKILL_OPS = {"get_skill"}

# Deliberately says reflow2 is INSTALLED here, never that a design EXISTS here.
# The hook cannot know: it runs before the server is reachable, and the graph
# meta file carries no node counts. The old text asserted a design graph and
# sent the agent to where-am-i — which, in a project set up minutes ago, is a
# constant stating something nobody measured, and the skill's own text says to
# use genesis instead when the graph is empty. So the line names BOTH doors and
# lets one cheap call decide which one this is.
# ⭐⭐ THE `link-artifacts` CLAUSE IS A RUNNING EXPERIMENT — DO NOT DELETE IT AS
# CLUTTER, AND DO NOT READ IT AS SETTLED DOCTRINE.
#
# MEASURED 2026-08-29 across 91 sessions of a real reflow2 project
# (dev-storyflow, 2026-07-29..2026-08-29, 5240 reflow2 tool calls): what
# predicts whether a skill gets loaded is being named in THIS STRING, not being
# in the served catalogue. The MCP instructions name all 23 skills and usage
# across them runs 42..0. This hook named three, and one of them —
# `where-am-i` — is the most-loaded skill by a factor of two, pulled 8.4x more
# often than the user asks for it (42 loads, 5 slash commands).
#
# 🛑 THAT RESULT IS CONFOUNDED: where-am-i is also the most generally useful
# skill in the set, so "named here" and "intrinsically useful" cannot be
# separated from observational data. This clause is the discriminating
# experiment — ONE skill added deliberately, with a baseline taken first.
#
#     BASELINE, dev-storyflow, before this line existed:
#       sessions calling add_artifact  : 27
#       of those, loaded link-artifacts: 6  (22%)
#
# Read it again in a few weeks. If the rate moves, an always-injected surface
# moves behaviour and the budget question below is the real design problem. If
# it does not, hook-presence was never the cause and where-am-i is simply
# useful — which is worth knowing and is why the baseline is written down here
# rather than recalled.
#
# ⚠️ PAIRED, ON PURPOSE, WITH A DIFFERENT ARM ON A DIFFERENT SKILL. `impact-check`
# is being demanded from the `record_change` TOOL DESCRIPTION over the same
# period (baseline 19%, 16 sessions), enforced by skill_lint's `demanded_by`
# check. Two channels, two skills, so neither result can be claimed by the
# other. Keep them apart.
#
# ⚠️ AND THIS STRING IS A SCARCE RESOURCE, which is the finding that makes the
# whole question hard: it is read by every session at every start, so naming all
# 23 skills here would recreate the catalogue that demonstrably predicts
# nothing. The same budget bit the tool-side arm — `add_change_event` sits at
# 1494 chars against a 1500 limit and physically could not carry its demand.
# Whatever mechanism wins, it wins inside a budget.
SESSION_START_TEXT = (
    "reflow2: this project has reflow2 installed. Orient first, before touching "
    "code: call open_questions — if this design is empty, start it with the "
    "genesis skill (new project) or the adopt skill (code that already exists); "
    "if it is not, read it back with the where-am-i skill. Skills are SERVED, "
    "not installed: get_skill fetches one, list_skills names them all. When you "
    "write or change a file the design should know about, the link-artifacts "
    "skill registers it against the capability it realizes. While "
    "you work, loop_status is the one cheap call that says what the coherence "
    "loop is owed; the Stop hook will nudge if graph writes finish without one."
)


def state_dir() -> Path:
    return Path(".reflow2") / "loop-nudge"


def state_file(session_id: str) -> Path:
    safe = "".join(c if c.isalnum() or c in "-_" else "_" for c in session_id)
    return state_dir() / f"{safe or 'unknown'}.json"


def warn(message: str) -> None:
    """A hook must never break a session, so every failure in here is a stderr
    line and an exit 0 — but it is never SILENT. The defect this file was
    fixed for was a swallowed failure that reported success."""
    print(f"loop_nudge: {message}", file=sys.stderr)


def blank_state() -> dict:
    """A session nobody has recorded anything for. Correct ONLY when the tally
    is genuinely absent — see [`update_state`] for why that distinction is the
    whole bug this file once had."""
    return {"writes": 0, "edits": 0, "touched": False,
            "changes": 0, "propagates": 0, "artifacts": 0, "captures": 0,
            "gap_pass": 0, "renderings": 0, "skills": 0,
            # Graph writes for the WHOLE session, never reset by a loop check —
            # distinct from `writes`, which is "unchecked since the last loop
            # check" and is deliberately cleared. Only this can answer "did this
            # session do design work at all", which `writes` cannot: a session
            # that wrote and then ran loop_status has `writes == 0` and looks
            # identical to one that only ever read.
            "wrote": 0,
            # The ChangeEvent ids this session recorded, so the probe can ask
            # what they RETIRED (`unclaimed_findings`). Ids rather than a count:
            # the question is answerable only against the specific events, and
            # this hook is the only place that sees them go past. Bounded — see
            # CHANGE_ID_CAP.
            "change_ids": [],
            # Whether this tally was rebuilt after an unreadable one. A restart
            # keeps the mechanism working; this flag is what stops the restart
            # being mistaken for evidence. See `update_state`.
            "reset": False,
            # Whether a Stop nudge has already been printed for this session
            # (BL-111). The promise "this nudge fires once" used to rest
            # entirely on the harness's `stop_hook_active`, which covers one
            # stop CYCLE and is never persisted — so the rule implemented was
            # *once per stop cycle* while the rule advertised was *once per
            # session*. See `claim_nudge`.
            "nudged": False}


def parse_state(text: str) -> dict | None:
    """The stored tally, or **None** if it cannot be read.

    `None` is not the same as `blank_state()` and conflating the two is what
    made a correct session read as one that never touched reflow2 at all: the
    caller must be able to tell *nobody has recorded anything* from *something
    is recorded and I could not read it*. Older state files carried only
    `writes`; the rest default, which is a genuine absence and stays fine.
    """
    try:
        raw = json.loads(text)
        return {
            "writes": int(raw.get("writes", 0)),
            "edits": int(raw.get("edits", 0)),
            "touched": bool(raw.get("touched", False)),
            # cap:skill-triggers — the shape fields. Absent in older state
            # files, which default to zero and simply yield the generic nudge.
            "changes": int(raw.get("changes", 0)),
            # BL-163. Absent in older state files and defaulting to zero, which
            # is the SAME shape as "this session never propagated" — a state
            # file written by the previous version cannot be told from a session
            # that skipped the step. That is safe only because the branch this
            # feeds also requires `changes > 0` in the SAME tally, and a tally
            # old enough to lack this key is one no longer being written to.
            "propagates": int(raw.get("propagates", 0)),
            "artifacts": int(raw.get("artifacts", 0)),
            "captures": int(raw.get("captures", 0)),
            "gap_pass": int(raw.get("gap_pass", 0)),
            "renderings": int(raw.get("renderings", 0)),
            # cap:skill-loads-are-counted. Absent in older state files and
            # defaulting to zero — the SAME shape as "this session loaded no
            # skill", so a tally written by the previous version is
            # indistinguishable from a real zero. Safe only because the branch
            # this feeds also requires `touched` and an edit count in the same
            # tally, and a tally old enough to lack this key is one no longer
            # being written to. Same argument as `propagates` above, and it
            # holds for the same reason.
            "skills": int(raw.get("skills", 0)),
            "wrote": int(raw.get("wrote", 0)),
            "change_ids": [str(c) for c in raw.get("change_ids", [])
                           if isinstance(c, (str, int))][:CHANGE_ID_CAP],
            "reset": bool(raw.get("reset", False)),
            "nudged": bool(raw.get("nudged", False)),
        }
    except (ValueError, KeyError, TypeError, AttributeError):
        return None


def read_state(session_id: str) -> dict:
    """The tally, for READING only (the Stop backstop).

    An unreadable tally reads as blank here, which is the quiet direction: the
    thresholds are then unmet and no nudge fires. A nudge fired on a tally
    nobody could read would be the false-positive this whole file is judged on.
    Never write the result of this back — use [`update_state`].
    """
    try:
        parsed = parse_state(state_file(session_id).read_text())
    except OSError:
        return blank_state()
    return parsed if parsed is not None else blank_state()


def _lock(path: Path):
    """Exclusive advisory lock around a read-modify-write, or a no-op context
    where the platform has no `flock`. Degrades rather than failing: a hook must
    never break a session, and an unlocked update is what shipped before.
    """
    import contextlib

    @contextlib.contextmanager
    def _ctx():
        try:
            import fcntl
        except ImportError:  # pragma: no cover — non-POSIX
            yield
            return
        try:
            fh = open(path, "w")
        except OSError:
            yield
            return
        try:
            fcntl.flock(fh, fcntl.LOCK_EX)
            yield
        finally:
            try:
                fcntl.flock(fh, fcntl.LOCK_UN)
            except OSError:
                pass
            fh.close()

    return _ctx()


def update_state(session_id: str, mutate) -> None:
    """Read-modify-write the tally under a lock, and replace it ATOMICALLY.

    Both halves are load-bearing, and this function is the fix for a defect that
    survived three sessions because it looked intermittent:

    - **The write was `write_text` — truncate, then write.** A concurrent hook
      process reading mid-write saw a partial file, `json.loads` raised, the
      old `read_state` swallowed it into an all-zero tally, and that process
      wrote the zeros back. One badly-timed read wiped `touched`, `artifacts`
      and `gap_pass` for the rest of the session, so a session that consulted
      the graph constantly was told it never had. Reproduced: seeding
      `touched=true, artifacts=4` and firing 150 concurrent edit hooks returned
      `touched=false, artifacts=0, edits=6` — 144 increments lost with it.
    - **The read-modify-write had no lock**, so even without corruption two
      concurrent hooks lost updates. PostToolUse hooks run as separate
      processes and parallel tool batches are normal, so this is the common
      case rather than the exotic one.

    A tally that exists but cannot be parsed still RESTARTS the count — the
    mechanism has to keep working, and an existing test says so in as many
    words. What changes is that the restart is **marked** (`reset`), because the
    lie was never the restart: it was a rebuilt-from-nothing tally then being
    read as proof that the graph was never consulted. A positive claim ("N
    writes went unchecked") survives a restart honestly; the negative one does
    not, and the Stop backstop drops it accordingly.
    """
    d = state_dir()
    try:
        d.mkdir(parents=True, exist_ok=True)
    except OSError as exc:
        warn(f"could not create {d}: {exc}")
        return
    path = state_file(session_id)
    with _lock(d / ".lock"):
        state = blank_state()
        if path.exists():
            try:
                parsed = parse_state(path.read_text())
            except OSError as exc:
                warn(f"could not read {path}: {exc}")
                return
            if parsed is None:
                warn(f"{path} exists but could not be parsed — restarting the "
                     f"tally and marking it `reset`, so nothing later reads it "
                     f"as proof the graph was never consulted")
                state["reset"] = True
            else:
                state = parsed
        mutate(state)
        tmp = path.with_name(path.name + ".tmp")
        try:
            tmp.write_text(json.dumps({
                "writes": int(state.get("writes", 0)),
                "edits": int(state.get("edits", 0)),
                "touched": bool(state.get("touched", False)),
                "changes": int(state.get("changes", 0)),
                # THIS LIST IS THE THIRD COPY OF THE STATE'S KEY SET
                # (`blank_state`, `parse_state`, here), and a field missing from
                # any one of them is silently dropped rather than failing. BL-163
                # was written with the other two updated and this one not, so the
                # counter incremented in memory and was thrown away on every
                # write — caught only because the new tests failed. When you add
                # a field, add it in all three.
                "propagates": int(state.get("propagates", 0)),
                "artifacts": int(state.get("artifacts", 0)),
                "captures": int(state.get("captures", 0)),
                "gap_pass": int(state.get("gap_pass", 0)),
                "renderings": int(state.get("renderings", 0)),
                # cap:skill-loads-are-counted. Added here THIRD, after the note
                # above was read and then ignored anyway: both of these were
                # incremented in memory and thrown away on every write, and the
                # new tests failed exactly as BL-163's did. The warning works;
                # it just cannot make anyone read it before writing the code.
                "skills": int(state.get("skills", 0)),
                "wrote": int(state.get("wrote", 0)),
                # THE WRITE SIDE OF `change_ids`. It was added to blank_state
                # and parse_state first and NOT here, and every read came back
                # empty while every test that only counted still passed — the
                # serialiser is an explicit field list, so a field added to the
                # shape and not to this dict is silently dropped on every write.
                "change_ids": [str(c) for c in state.get("change_ids", [])
                               ][-CHANGE_ID_CAP:],
                "reset": bool(state.get("reset", False)),
                "nudged": bool(state.get("nudged", False)),
            }))
            os.replace(tmp, path)  # atomic: a reader sees old or new, never half
        except OSError as exc:
            warn(f"could not write {path}: {exc}")
            try:
                tmp.unlink()
            except OSError:
                pass
            return
    # Opportunistic tidy-up: session files a week old are dead sessions.
    cutoff = time.time() - 7 * 24 * 3600
    for old in d.glob("*.json"):
        try:
            if old.stat().st_mtime < cutoff:
                old.unlink()
        except OSError:
            pass


def claim_nudge(session_id: str) -> bool:
    """Claim the right to nudge this session, exactly once (BL-111).

    Returns True for the FIRST caller and False for every one after it, as an
    atomic test-and-set under the same lock `update_state` uses.

    Two reasons it has to be a claim rather than a flag set after printing:

    - **The promise was never computed.** The message ends *"this nudge fires
      once"*, and that rested entirely on the harness's `stop_hook_active`,
      which covers a single stop CYCLE and is never persisted. So the rule
      implemented was *once per stop cycle* and the rule advertised was *once
      per session* — and the case where the gap bites hardest is the one where
      the nudge cannot be satisfied at all (a session whose server is
      unreachable gets nudged at every stop with no action that would stop it,
      which is exactly when someone disables the hook).
    - **The hook can legitimately be registered more than once.** reflow2
      installs machine-wide *and* a project can carry its own registration, and
      the two command spellings do not dedupe — so two processes run this on the
      same Stop. Without an atomic claim they both read `nudged: false` and both
      print, which is the doubled message BL-111 was filed from.
    """
    claimed = False

    def take(state: dict) -> None:
        nonlocal claimed
        if not state.get("nudged"):
            claimed = True
            state["nudged"] = True

    update_state(session_id, take)
    return claimed


def is_write(op: str) -> bool:
    return op.startswith(("add_", "create_", "delete_")) or op in EXTRA_WRITE_OPS


def env_threshold(name: str, default: int) -> int:
    try:
        return max(1, int(os.environ.get(name, str(default))))
    except ValueError:
        return default


def design_present() -> bool:
    """Whether this working directory has opted into being designed.

    The same test the server's latent mode makes, for the same reason and it
    must stay the same: since 2026-07-28 reflow2 can be installed ONCE per
    machine, which registers these hooks globally. A hook that nudged about
    coherence in a directory with no design would fire in every repo the user
    ever opens — and the one thing a trigger cannot survive is being noise.

    Deliberately a directory test and not a graph read: the session's own server
    holds the single-writer lock, and this script must stay cheap enough to run
    on every tool call.
    """
    return Path(".reflow2").exists()


def match_shape(state: dict) -> str | None:
    """The skill this session's SHAPE calls for, or None (`cap:skill-triggers`).

    **This never fires on its own.** It only refines a nudge the caller has
    already decided to send, so the number of nudges is unchanged and only their
    usefulness moves. That is the whole design constraint: `ver:skill-triggers`'s
    own counterweight says a trigger that fires on correct work is the failure
    BL-23 and BL-42 both name, and this capability exists to REDUCE nagging
    rather than add to it. A matcher that could arm the hook by itself would be
    adding a fourth way to be interrupted, which is the opposite.

    The shapes are mutually exclusive and ordered earliest-first, because a
    session that never recorded the change has a different next step from one
    that recorded it and stopped there.
    """
    edits = state.get("edits", 0)
    # 1a. Something on disk moved and nothing on the record says so.
    if edits > 0 and state.get("changes", 0) == 0:
        return (
            "impact-check — file(s) changed this session with no ChangeEvent "
            "recorded, so nothing computed what the change reaches"
        )
    # 1b. The change IS on the record and nobody asked what it reaches (BL-163).
    #     The half that used to be invisible: `changes > 0` silenced the shape
    #     above, so writing the ChangeEvents up after the edits satisfied the
    #     very trigger that exists to catch writing them up after the edits.
    #     Recording is not looking, and only PROPAGATE_OPS is looking.
    if edits > 0 and state.get("propagates", 0) == 0:
        return (
            "impact-check — the change is recorded but propagate_change was "
            "never called, so the ChangeEvent is bookkeeping and nothing "
            "computed the blast radius it exists to compute"
        )
    # 2. The change IS recorded, and as-built was never told. The reachable
    #    half of the spec's checksum-drift shape: the hook cannot see drift, but
    #    it can see that nobody looked.
    if edits > 0 and state.get("artifacts", 0) == 0:
        return (
            "link-artifacts — the change is on the record but no artifact was "
            "linked or re-checksummed, so as-designed and as-built have not been "
            "reconciled"
        )
    # 3. Intent captured, and nobody put the questions it raises.
    if state.get("captures", 0) > 0 and state.get("gap_pass", 0) == 0:
        return (
            "detect-and-ask — intent was captured this session and detect_gaps "
            "was never run, so the decisions it implies are still unasked"
        )
    # 4. Something was drawn and nothing was stored (cap:session-artifacts).
    #    Named LAST: it is the least urgent of the four, and a rendering is the
    #    visual half of a rationale rather than a break in the thread. The
    #    sentence carries the FILTER as well as the trigger, because the hook
    #    cannot tell an orphan from an explanation and must not imply it can.
    return None


# ---- cap:stop-nudge-asks-the-graph: the branch that ASKS rather than counts --
#
# Everything above this line reasons from the session's own op TALLY. That is a
# proxy for design debt and it is blind in the one direction that matters: a
# session which goes through the loop's motions correctly — write, then
# `loop_status` — trips no counting branch, whatever it actually left in the
# graph. The tally can see that the motions happened. It cannot see what they
# left behind.
#
# So this asks. `dec:the-stop-hook-asks-the-graph-asynchronously`, Anthony
# 2026-08-24, and the shape is forced by one measurement rather than by taste:
#
#     loop_status                     22.6s   (repeatably, on reflow2's own graph)
#     loop_status since_export=true   37.5s
#     claim_report                     0.04s
#
# A Stop hook that blocks for 23 seconds is worse than no hook, and nothing
# under a second answers the question. Hence: SPAWN and return, collect at a
# later stop. A session stops once per TURN, so on anything longer than a couple
# of exchanges the answer arrives while the agent can still act on it; when the
# session ends first, SessionStart reports it instead. That fallback is the
# degraded case, not the design.
#
# ⭐⭐ THE TRIGGER IS A DELTA, NOT A LEVEL, and this is the whole restraint.
# reflow2's own design carries 7 unsurfaced gaps and 60 structural defects, both
# standing for weeks. Fired on a level this would speak in every session forever
# and be a nag — the fire-on-correct-work failure `ver:skill-triggers` exists to
# prevent, and the one BL-23 and BL-42 both name. Fired on a delta against a
# baseline taken at SessionStart, it says "this session took open gaps from 7 to
# 10 and never put the three new ones to the user", which is a fact about the
# session. NO BASELINE MEANS NO NUDGE: a delta needs two readings, and a session
# whose first probe never landed has one.
#
# 🛑 `structural_defects` IS DELIBERATELY NOT A TRIGGER CLASS — see `COUNTS` in
# graph_probe.py. HEAL's count moves on edits nobody made to it, so a delta
# there would report a session for something it did not do.
#
# ⚠ THE READING CAN BE STALE, AND THE ONLY HONEST FIX IS TO SAY SO. The answer
# is whatever the last probe found, which the debounce holds at up to a couple
# of minutes old and longer if no probe has been spawned since. A session
# genuinely IN FLIGHT raises counts and then settles them — capture intent, and
# the gaps appear; wire the thread, and they close — so a verdict collected from
# a mid-session reading can name debt that is already gone. Observed on the very
# session that built this: gaps read 7 → 10 while the new nodes were unwired and
# finished at 6. The message therefore states the reading's AGE and tells the
# agent to confirm with `loop_status`, which is the one call that answers now.
# Waiting for a fresh probe at stop time is the 23-second block this whole
# design exists to avoid, so staleness is the cost that was accepted.
#
# WHAT IT STILL CANNOT DO, stated rather than implied: the graph cannot see the
# transcript. This reports what the DESIGN says changed, never the reasoning the
# conversation held and nobody wrote down. It complements the capture-session
# skill; it does not replace it, and `req:skill-use-survives-a-long-session`
# stays unmet.

# How many ChangeEvent ids one session's tally will carry. A bulk session can
# record dozens, and the question they answer is answered just as well by the
# most recent handful — while an unbounded list would grow a state file the hook
# rewrites on every tool call. The cap is stated in the probe's answer rather
# than applied silently.
CHANGE_ID_CAP = 25

PROBE_SCRIPT = Path(__file__).resolve().parent / "graph_probe.py"

# How long a probe may be in flight before the hook assumes it died and lets
# another start. Generously above the ~23s measured call, because the cost of
# guessing too soon is two probes hammering the shared server and the cost of
# guessing too late is one quiet session.
PROBE_STALE_LOCK_S = 300

# Minimum gap between probe STARTS. Turns come faster than this, and re-asking a
# 23-second question every turn would load the server that is also serving the
# session doing the work.
PROBE_MIN_INTERVAL_S = 120

# The debt classes that get a sentence, in the order a nudge should mention
# them: what the design does not yet KNOW first, what it has not yet CHECKED
# after. THE KEYS MIRROR `COUNTS` IN graph_probe.py — two stdlib-only scripts in
# two processes, so there is no shared module to hold them, and a key added
# there without a sentence here measures something the nudge cannot say.
COUNT_ADVICE = (
    ("unsurfaced_gaps",
     "open gap(s) nobody has put to the user",
     "run detect-and-ask"),
    ("unanswered_questions",
     "question(s) put to the user and still waiting",
     "follow them up — open_questions carries the wording they saw"),
    ("unwritten_answers",
     "answer(s) the user gave that nothing has written into the design",
     "write them in, or acknowledge the gap"),
    ("undispositioned_drift",
     "artifact(s) whose file no longer matches what the design recorded",
     "run link-artifacts and give each drift its OWN disposition"),
    ("unproven_capabilities",
     "capability(ies) claiming realized/verified with no passing check",
     "add or run their Verification"),
    ("unexamined_claims",
     "built capability(ies) nobody has checked against reality",
     "check them, or record why not"),
    ("unsettled_assigned_decisions",
     "Decision(s) a named person was asked to settle",
     "put them to that person"),
)


def probe_file(session_id: str) -> Path:
    return state_dir() / (state_file(session_id).stem + ".probe.json")


def probe_lock(session_id: str) -> Path:
    return state_dir() / (state_file(session_id).stem + ".probe.lock")


def read_probe(session_id: str) -> dict:
    """The last probe's answer, or an empty dict.

    Unreadable reads as absent, which is the quiet direction — every claim this
    feature makes is a positive one ("the count went up"), and a count nobody
    could read supports none of them.
    """
    try:
        data = json.loads(probe_file(session_id).read_text())
    except (OSError, ValueError):
        return {}
    return data if isinstance(data, dict) else {}


def probe_in_flight(session_id: str, now: float) -> bool:
    """Is a probe already running for this session?

    The probe removes its own lock on the way out, so a lock that is still here
    and still young means one is working. An OLD lock means a probe was killed
    before it could clean up — treated as gone rather than as running, because
    a stuck lock would silence the feature permanently and a duplicate probe
    costs one wasted read.
    """
    try:
        lock = json.loads(probe_lock(session_id).read_text())
        started = lock.get("started")
    except (OSError, ValueError):
        return False
    if not isinstance(started, (int, float)):
        return False
    return (now - started) < PROBE_STALE_LOCK_S


def spawn_probe(session_id: str, reason: str) -> None:
    """Start the detached probe, or decline for a stated reason. NEVER waits.

    Every early return here is a case where asking would cost more than the
    answer is worth; none of them is an error, and none of them warns.
    """
    if not PROBE_SCRIPT.exists():
        return  # a kit install that did not carry the probe — silent by design
    if not Path(".reflow2/graph.server.json").exists():
        return  # served over stdio: there is no URL to ask, and that is normal
    data = read_probe(session_id)
    if data.get("unavailable"):
        return  # already established there is nothing to ask; do not re-ask
    now = time.time()
    last = data.get("taken_at")
    if isinstance(last, (int, float)) and (now - last) < PROBE_MIN_INTERVAL_S:
        return
    if probe_in_flight(session_id, now):
        return
    try:
        import subprocess
        state_dir().mkdir(parents=True, exist_ok=True)
        probe_lock(session_id).write_text(
            json.dumps({"started": now, "reason": reason})
        )
        # start_new_session so it outlives this hook process, and DEVNULL on
        # every stream because the hook's stdout is a CONTRACT — Claude Code
        # parses it as the hook's decision, and a stray line from a child would
        # be read as one.
        subprocess.Popen(
            [sys.executable, str(PROBE_SCRIPT), session_id],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            start_new_session=True,
        )
    except (OSError, ValueError, ImportError) as e:
        warn(f"could not start the graph probe ({e})")


def graph_verdict(session_id: str) -> str | None:
    """What the graph says THIS session raised. See [`verdict_from`]."""
    return verdict_from(read_probe(session_id))


def verdict_from(data: dict) -> str | None:
    """What a probe file says its session RAISED, or None if it says nothing.

    None covers every case where the answer would not be supportable: no probe,
    no baseline, one reading only, an unreadable file, or a probe whose counts
    went nowhere. Silence is the correct output for all of them.

    Takes the DATA rather than a session id because the same computation has to
    run over another session's file at SessionStart, and a version that could
    only reach the current session's would have been quietly copied.
    """
    counts = data.get("counts")
    baseline = data.get("baseline")
    if not isinstance(counts, dict) or not isinstance(baseline, dict):
        return None
    base = baseline.get("counts")
    base_at = baseline.get("taken_at")
    taken = data.get("counts_taken_at")
    if not isinstance(base, dict):
        return None
    if not isinstance(taken, (int, float)) or not isinstance(base_at, (int, float)):
        return None
    # The first probe IS its own baseline, so it can only ever report a delta of
    # zero. Refusing it explicitly keeps that from depending on the arithmetic
    # below happening to come out empty.
    if taken <= base_at:
        return None

    risen = []
    for key, noun, remedy in COUNT_ADVICE:
        now_value, was = counts.get(key), base.get(key)
        # Both sides must be present. A class the server did not report on one
        # of the two readings is a class nobody measured twice — usually a
        # server upgraded mid-session — and treating the absent side as zero
        # would manufacture a delta out of a version change.
        if isinstance(now_value, int) and isinstance(was, int) and now_value > was:
            risen.append(f"{noun} {was} → {now_value} (+{now_value - was}) — {remedy}")
    if not risen:
        return None

    age = int(max(0, time.time() - taken))
    caveat = ""
    served = data.get("served_by")
    if isinstance(served, dict) and served.get("stale"):
        # The trap `fact:an-agent-that-cannot-name-its-version-reports-fixed-bugs`
        # names, one layer over: every COMPUTED rollup in that answer came from
        # a binary no longer on disk. The nudge still fires — the counts are the
        # best reading there is — but it must not present them as current.
        caveat = (
            " ⚠ The server reports its own binary as STALE, so these computed "
            "counts came from code no longer on disk; refresh it with "
            "`reflow2-mcp --graph-path <path> --stop-shared` before acting on "
            "the numbers."
        )
    return (
        f"the graph itself says so — measured by loop_status against the live "
        f"graph {age}s ago, not counted from your tool calls: "
        + "; ".join(risen) + "." + caveat
    )


# How many probe files a SessionStart will look at, and how long one is kept.
# The directory holds one file per session and would otherwise grow forever.
PROBE_SCAN_LIMIT = 12
PROBE_KEEP_S = 14 * 24 * 3600


def last_unreported_verdict(current_session: str) -> str | None:
    """A verdict an EARLIER session earned and never heard.

    The async shape's admitted cost: a session whose last stop happens before
    its probe lands never gets told. This is where that answer surfaces —
    late, in the next session, attributed to the session that caused it. It is
    the fallback, not the design, and the wording at the call site says so.

    Marks the file reported before returning, because the alternative is
    repeating one session's debt at the start of every session after it.
    """
    current = probe_file(current_session)
    try:
        files = sorted(
            (f for f in state_dir().glob("*.probe.json") if f != current),
            key=lambda f: f.stat().st_mtime, reverse=True,
        )
    except OSError:
        return None

    now = time.time()
    verdict = None
    for index, path in enumerate(files):
        try:
            if now - path.stat().st_mtime > PROBE_KEEP_S:
                path.unlink()
                continue
        except OSError:
            continue
        if verdict is not None or index >= PROBE_SCAN_LIMIT:
            continue  # keep pruning, stop reading
        try:
            data = json.loads(path.read_text())
        except (OSError, ValueError):
            continue
        if not isinstance(data, dict) or data.get("reported"):
            continue
        # BOTH unheard answers, joined: the delta ("you raised debt") and the
        # retired-ask ("you may have made these false"). They are different
        # questions with the same delivery problem — a session that ended before
        # its probe landed heard neither — so they surface together rather than
        # one of them being silently dropped for arriving second.
        parts = [x for x in (verdict_from(data), retired_ask(data)) if x]
        if not parts:
            continue
        found = " ".join(parts)
        data["reported"] = True
        try:
            tmp = path.with_suffix(f".{os.getpid()}.tmp")
            tmp.write_text(json.dumps(data, indent=1))
            os.replace(tmp, path)
        except OSError:
            # Could not mark it. Say nothing rather than say it every session
            # from here on — an unstoppable reminder is the hostage-taking this
            # file's once-only rule exists to prevent.
            continue
        verdict = found
    return verdict



def retired_ask(data: dict | None = None, session_id: str | None = None) -> str | None:
    """The shortlist of observations this session's work may have made FALSE.

    ⭐ IT ASKS AND NEVER BLOCKS — Anthony's word, 2026-08-24, and the restraint
    is deliberate rather than timid. Every other branch in this file arms an
    interruption; this one only ever RIDES ALONG on a message already being
    sent, or is carried to the next SessionStart. A brand-new trigger keyed on
    a computation nobody has field-tested is exactly the thing that should not
    get to stop a session, and it can be upgraded once the shortlist has been
    seen to be right.

    🛑 THE HONEST CONSEQUENCE, stated because it decides how much this is worth:
    a Stop hook that does not block reaches the TRANSCRIPT, not the agent. The
    two paths that genuinely reach an agent are the ride-along (when some other
    branch is already blocking) and the next SessionStart, where a hook's stdout
    does enter context. If this needs to reach an agent mid-session every time,
    blocking is the only mechanism that does it.
    """
    if data is None:
        data = read_probe(session_id or "")
    found = data.get("unclaimed")
    if not isinstance(found, dict):
        return None
    rows = found.get("candidates") or []
    if not rows:
        return None
    named = []
    for row in rows[:3]:
        if not isinstance(row, dict):
            continue
        label = str(row.get("name") or row.get("finding_id") or "").strip()
        since = row.get("valid_from")
        named.append(f"{label}" + (f" (taken {since})" if since else ""))
    if not named:
        return None
    more = found.get("count", len(rows)) - len(named)
    tail = f", and {more} more" if more > 0 else ""
    return (
        f"WHAT DID THIS SESSION MAKE FALSE? The graph says your changes touched "
        f"{found.get('count')} open observation(s) nobody has closed: "
        + "; ".join(named) + tail + ". Each is a CANDIDATE, not a verdict — "
        f"close only what your work actually retired, with `invalidates`, and "
        f"never by overwriting what you are closing. Re-check with "
        f"`unclaimed_findings`."
    )


def ride_along(session_id: str, reason: str) -> str:
    """Append the retired-observations ask to a message already going out.

    FREE BY CONSTRUCTION: it adds no interruption, because it only ever speaks
    where one was happening anyway. That is the whole of "ask, don't block".
    """
    ask = retired_ask(session_id=session_id)
    return f"{reason} {ask}" if ask else reason


def main() -> int:
    try:
        event = json.load(sys.stdin)
    except (ValueError, OSError):
        return 0
    if not isinstance(event, dict):
        return 0
    # Silent everywhere reflow2 is not being used. Checked before anything is
    # read or counted, so a non-reflow2 project costs one stat call and leaves
    # no state behind.
    if not design_present():
        return 0
    kind = event.get("hook_event_name", "")
    session = str(event.get("session_id") or "unknown")

    if kind == "SessionStart":
        print(SESSION_START_TEXT)
        # The BASELINE. Everything the graph branch later claims is measured
        # against this reading, so it has to be taken before the session has
        # done anything — which is the one moment a hook is guaranteed to run
        # and the agent is guaranteed not to have written yet.
        #
        # THE HONEST LIMIT: the probe takes ~23s, so a session whose first turn
        # writes to the graph inside that window has those writes folded into
        # its own baseline. There is no clock on a node to fix this with. The
        # error is one-directional — the nudge UNDER-reports, never over-reports
        # — which is the right direction for a thing that interrupts people.
        spawn_probe(session, "session-start baseline")
        # A verdict the previous session earned and never heard: it stopped for
        # the last time before its probe landed. Named as the previous session's
        # so nobody reads it as a fact about this one.
        stale_verdict = last_unreported_verdict(session)
        if stale_verdict:
            print(f"reflow2: the session before this one left debt behind — "
                  f"{stale_verdict}")
        return 0

    if kind == "PostToolUse":
        tool = str(event.get("tool_name") or "")
        # This project's reflow2 server; the op is the last __ segment. Any
        # call — even a read — counts as having engaged the design brain.
        if "reflow2" in tool and "__" in tool:
            op = tool.rsplit("__", 1)[-1]

            # A ChangeEvent's id travels in the call's own input, which is the
            # only place it appears — the result is not given to a PostToolUse
            # hook in a shape this can rely on.
            change_id = ""
            ti = event.get("tool_input")
            if isinstance(ti, dict) and isinstance(ti.get("id"), str):
                change_id = ti["id"]

            def touch(state: dict, op: str = op, change_id: str = change_id) -> None:
                state["touched"] = True
                if op in LOOP_OPS:
                    state["writes"] = 0
                elif is_write(op):
                    state["writes"] += 1
                # Cumulative and never cleared — see `blank_state`. A loop check
                # settles the DEBT, it does not un-write what was written.
                if is_write(op):
                    state["wrote"] += 1
                # Shape tallies are cumulative and are NOT cleared by a loop
                # check: detect_gaps does not un-edit a file or un-capture
                # intent.
                if op in CHANGE_OPS:
                    state["changes"] += 1
                    # THE ID, not just the count. `unclaimed_findings` can only
                    # answer against the specific events, and this hook is the
                    # one place that sees them written.
                    if change_id:
                        ids = state.setdefault("change_ids", [])
                        if change_id not in ids:
                            ids.append(change_id)
                            del ids[:-CHANGE_ID_CAP]
                if op in PROPAGATE_OPS:
                    state["propagates"] += 1
                if op in ARTIFACT_OPS:
                    state["artifacts"] += 1
                if op in CAPTURE_OPS:
                    state["captures"] += 1
                if op in GAP_PASS_OPS:
                    state["gap_pass"] += 1
                if op in SKILL_OPS:
                    state["skills"] += 1

            update_state(session, touch)
            return 0
        # A harness file-write, tallied only for the total-bypass backstop.
        if tool in EDIT_TOOLS:
            # cap:session-artifacts — was it a rendering? The path is the only
            # signal available here, and a wrong guess costs at most one extra
            # sentence on a nudge that was firing anyway.
            path = ""
            ti = event.get("tool_input")
            if isinstance(ti, dict):
                path = str(ti.get("file_path") or ti.get("notebook_path") or "")
            rendering = path.lower().endswith(RENDERING_SUFFIXES)

            def edited(state: dict, rendering: bool = rendering) -> None:
                state["edits"] += 1
                if rendering:
                    state["renderings"] += 1

            update_state(session, edited)
        return 0

    if kind == "Stop":
        if event.get("stop_hook_active"):
            return 0  # already nudged once — never hold the session hostage
        state = read_state(session)

        # ASK FIRST, READ LATER, and ask BEFORE any branch below can return.
        # The question takes ~23 seconds to answer and nothing here waits for
        # it; putting the spawn above the branches means a session that trips a
        # counting nudge still has its graph answer in flight for the stop after
        # it. Only sessions that DID something are asked about — a read-only
        # session has left nothing for the graph to report.
        if state.get("wrote", 0) > 0 or state.get("edits", 0) > 0:
            spawn_probe(session, "stop")

        # Graph writes finished without a loop check (the original nudge).
        n = state["writes"]
        if n >= env_threshold("REFLOW2_LOOP_NUDGE_THRESHOLD", 1):
            # cap:skill-triggers — same trigger, better sentence. If the shape
            # is recognisable, name the skill the situation calls for instead of
            # pointing at loop_status and leaving the agent to work it out.
            shape = match_shape(state)
            # The shape REPLACES the generic advice but never the cheap entry
            # point: `loop_status` stays in every message because it is the one
            # call that says what is actually owed, and naming a skill is a
            # refinement of that answer rather than a substitute for it. An
            # existing test asserted this and caught its removal — the contract
            # was real and was nearly dropped silently.
            detail = (
                f"The shape says: {shape}. Confirm with loop_status."
                if shape
                else "Call loop_status — if its `next` list names debt, run "
                     "detect-and-ask / check-health before finishing."
            )
            # BL-111 — the promise is now computed, not merely stated. First
            # claimant prints; anyone else (a later stop, or a second hook
            # process from a duplicate registration) stays silent.
            if not claim_nudge(session):
                return 0
            print(json.dumps({
                "decision": "block",
                "reason": ride_along(
                    session,
                    f"reflow2: {n} graph write(s) this session and no loop check. "
                    f"{detail} Bookkeeping is not the loop. (This nudge fires "
                    f"once; stopping again proceeds.)"
                ),
            }))
            return 0

        # EVERY BRANCH BELOW HERE MAKES A NEGATIVE CLAIM, and a tally rebuilt
        # from nothing cannot support one. "The graph was never consulted" and
        # "propagate_change was never called" are both assertions that something
        # did NOT happen, and the calls that would refute them are exactly what
        # an unreadable tally lost (BL-161). The write nudge above is positive
        # ("N writes went unchecked") and survives a restart honestly, which is
        # why it sits on the other side of this line.
        if state.get("reset"):
            return 0

        # BL-163 — the recorded-but-never-propagated session. This is the branch
        # the row is actually about, and the reason the shape matcher alone was
        # not enough: `record_change` is a graph WRITE, so a session that
        # recorded its changes and then ran `loop_status` has `writes == 0` and
        # `touched == True`, and sails past both of the older branches. Neither
        # one can see a loop that ran in the wrong ORDER.
        #
        # THE CONJUNCTION IS THE COUNTERWEIGHT, all three clauses load-bearing:
        #   - `edits > 0`     — something on disk actually moved; a pure design
        #                       session that captures intent and never touches
        #                       code has no blast radius to compute.
        #   - `changes > 0`   — this session engaged the design brain and put a
        #                       ChangeEvent on the record. That is what makes it
        #                       a design-relevant session rather than any old
        #                       edit, and it is what keeps this from becoming a
        #                       second bypass nudge with no threshold.
        #   - `propagates==0` — and then never asked what the change reaches.
        # Drop any one of them and this fires on correct work, which BL-23 and
        # BL-42 both name as the failure this family exists to avoid.
        #
        # THE ADMITTED COST, stated because it is a real change in kind: this is
        # a NEW interruption. `cap:skill-triggers` deliberately added none — a
        # shape only refined a nudge that was already firing. This branch arms
        # one, and it does so on the argument the row makes: reflow2 fails the
        # build on undeclared drift, on a broken export chain, on unchecked
        # writes, and nothing whatsoever fails when an agent designs without
        # consulting the design. The read side had no forcing function at all.
        if (state["edits"] > 0
                and state.get("changes", 0) > 0
                and state.get("propagates", 0)
                < env_threshold("REFLOW2_LOOP_NUDGE_PROPAGATE_THRESHOLD", 1)):
            if not claim_nudge(session):
                return 0
            print(json.dumps({
                "decision": "block",
                "reason": ride_along(
                    session,
                    f"reflow2: {state['edits']} file(s) edited and "
                    f"{state['changes']} change(s) recorded this session, and "
                    f"propagate_change was never called — so the ChangeEvent is "
                    f"bookkeeping and nothing computed what the change reaches. "
                    f"Run impact-check (record the change, THEN propagate) "
                    f"before further edits; confirm with loop_status. "
                    f"Bookkeeping is not the loop. (This nudge fires once; "
                    f"stopping again proceeds.)"
                ),
            }))
            return 0

        # The upstream bypass (BL-90): code edited, the graph never consulted at
        # all. Blunt by design — the hook cannot know which files are design-
        # relevant, so a count threshold and the once-only rule bound the noise.
        if not state["touched"]:
            e = state["edits"]
            if e >= env_threshold("REFLOW2_LOOP_NUDGE_EDIT_THRESHOLD", 3):
                # Both blocking branches make the same promise on the same
                # footing, so both have to keep it (BL-111).
                if not claim_nudge(session):
                    return 0
                print(json.dumps({
                    "decision": "block",
                    "reason": (
                        f"reflow2: {e} file(s) edited this session and the design "
                        f"graph was never consulted. Start with loop_status; run "
                        f"impact-check before further edits and link-artifacts "
                        f"after, so as-built stays honest. (This nudge fires once; "
                        f"stopping again proceeds.)"
                    ),
                }))
            return 0

        # cap:skill-loads-are-counted — LAST, and last on purpose.
        #
        # The hole this closes is the one `cap:skill-triggers` structurally
        # cannot reach. Every shape that capability recognises is a shape in the
        # graph-write stream, and "no skill has been loaded" is not a graph
        # write, so nothing counted it. Twice the decay was found by a human
        # asking an agent — which `req:skill-adherence-is-measured` calls, in
        # its own words, "not a mechanism".
        #
        # THIS ARMS A NUDGE, which `cap:skill-triggers` deliberately never did,
        # so the conjunction carries the whole counterweight — all three clauses
        # load-bearing, and `ver:skill-triggers` says why: a trigger that fires
        # on correct work is the failure BL-23 and BL-42 both name.
        #   - reached here at all — every other branch declined, so the session
        #     is otherwise clean. This never adds a second interruption to a
        #     session already being interrupted; it can only speak where there
        #     would have been silence.
        #   - `wrote > 0`        — the session did DESIGN work, not merely a
        #                          read. This is the clause that keeps an
        #                          existing contract intact: "a single read
        #                          means the agent DID consult the graph — no
        #                          bypass", and one `scan_nodes` must stay
        #                          silent. It has to be the cumulative counter,
        #                          because `writes` is cleared by a loop check
        #                          and a session that wrote and then checked
        #                          would look read-only here.
        #   - `edits >= N`       — real work landed on disk. Shares the bypass
        #                          threshold, so a read-only or trivial session
        #                          is silent, which is most sessions.
        #   - `skills == 0`      — and not one skill was ever opened.
        #
        # A NEGATIVE CLAIM, so it sits below the `reset` guard above: a tally
        # rebuilt from nothing lost exactly the `get_skill` calls that would
        # refute it, and "you loaded no skills" is not something to assert from
        # an amnesiac count.
        skill_edit_floor = env_threshold("REFLOW2_LOOP_NUDGE_EDIT_THRESHOLD", 3)
        if (state.get("wrote", 0) > 0
                and state["edits"] >= skill_edit_floor
                and state.get("skills", 0) == 0):
            if not claim_nudge(session):
                return 0
            print(json.dumps({
                "decision": "block",
                "reason": ride_along(
                    session,
                    f"reflow2: {state['edits']} file(s) edited this session and "
                    f"no skill was ever loaded. The skills carry this project's "
                    f"conventions, and they are SERVED, not installed — "
                    f"list_skills names them, get_skill reads one in full. Load "
                    f"the one that covers what you just did (link-artifacts "
                    f"after writing a file, impact-check before changing a "
                    f"design, revise-design when editing what the graph already "
                    f"says) and check the work against it. Reading it afterwards "
                    f"still catches what it would have prevented. (This nudge "
                    f"fires once; stopping again proceeds.)"
                ),
            }))
            return 0

        # cap:stop-nudge-asks-the-graph — LAST, and it speaks only where every
        # counting branch stayed silent.
        #
        # THAT PLACEMENT IS THE POINT, not an ordering convenience. The session
        # this reaches is the one that did the loop's motions correctly and so
        # tripped nothing above: wrote, then checked; edited, then propagated.
        # The tally has nothing left to say about it. The graph does.
        #
        # IT ADDS NO SECOND INTERRUPTION. `claim_nudge` is one flag for the
        # whole session across every branch, so a session already being
        # interrupted is not interrupted twice — this can only speak into what
        # would otherwise have been silence, which is the same counterweight
        # `cap:skill-loads-are-counted` carries and for the same reason.
        #
        # It sits below the `reset` guard with the other negative claims by
        # position, though it does not need to: its claim is positive and comes
        # from the probe file rather than from the tally, so a rebuilt tally
        # cannot make it lie. It stays here because the branch it must not
        # pre-empt is above it.
        verdict = graph_verdict(session)
        if verdict:
            if not claim_nudge(session):
                return 0
            print(json.dumps({
                "decision": "block",
                "reason": ride_along(
                    session,
                    f"reflow2: this session ADDED design debt and left it — "
                    f"{verdict} CONFIRM WITH loop_status before acting: that "
                    f"reading is as old as it says, and a session still in "
                    f"flight settles some of what it raises. Then settle it or "
                    f"say why it stands; bookkeeping is not the loop. (This "
                    f"nudge fires once; stopping again proceeds.)"
                ),
            }))
        return 0

    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as e:  # noqa: BLE001 — a hook must never break a session
        print(f"loop_nudge: skipped ({e})", file=sys.stderr)
        sys.exit(0)
