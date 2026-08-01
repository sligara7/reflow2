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
  - the session never touched reflow2 at all and edited enough files → "the
    graph was never consulted; start with loop_status, impact-check before
    further edits, link-artifacts after".
  A second stop (`stop_hook_active`) always proceeds — a nudge that can loop
  forever is a hostage-taker, not a trigger. The two cases are mutually
  exclusive: any graph write means reflow2 was touched, so the bypass case
  cannot also be armed.

Deliberately does NOT read the graph: the session's own MCP server holds the
single-writer lock, and the committed export can be a session stale. The hook
counts events and points at `loop_status`; the *graph* answers what is owed —
which also means the hook cannot know which edited files are design-relevant, so
the bypass backstop stays blunt (a count threshold, once-only) on purpose.
State is one small JSON per session under `.reflow2/loop-nudge/` (gitignored
with the rest of `.reflow2/`). A hook must never break a session: any failure
here warns on stderr and exits 0.

Stdlib only, no arguments needed. Two thresholds, both env-tunable:
`REFLOW2_LOOP_NUDGE_THRESHOLD` (default 1) — unchecked graph writes before the
Stop backstop fires; `REFLOW2_LOOP_NUDGE_EDIT_THRESHOLD` (default 3) — file
edits in a zero-reflow2 session before the bypass backstop fires.
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
# call and CANNOT READ THE GRAPH — the session's own server holds the
# single-writer lock, and `design_present()` is deliberately a directory test
# for that reason. So a shape is only implementable if it is visible in the
# session's own op tally. That rules out the spec's "an artifact checksum drift
# names link-artifacts" as literally written: drift is a fact about the graph
# versus the disk, and nothing here can see either. What IS visible is the
# progression — a change recorded but as-built never touched — which is the
# same situation one step earlier, and is what the third shape keys on.

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

# Putting bytes in the content store.
CONTENT_OPS = {"content_put"}

# Deliberately says reflow2 is INSTALLED here, never that a design EXISTS here.
# The hook cannot know: it runs before the server is reachable, and the graph
# meta file carries no node counts. The old text asserted a design graph and
# sent the agent to where-am-i — which, in a project set up minutes ago, is a
# constant stating something nobody measured, and the skill's own text says to
# use genesis instead when the graph is empty. So the line names BOTH doors and
# lets one cheap call decide which one this is.
SESSION_START_TEXT = (
    "reflow2: this project has reflow2 installed. Orient first, before touching "
    "code: call open_questions — if this design is empty, start it with the "
    "genesis skill (new project) or the adopt skill (code that already exists); "
    "if it is not, read it back with the where-am-i skill. Skills are SERVED, "
    "not installed: get_skill fetches one, list_skills names them all. While "
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
            "changes": 0, "artifacts": 0, "captures": 0, "gap_pass": 0,
            "renderings": 0, "content": 0,
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
            "artifacts": int(raw.get("artifacts", 0)),
            "captures": int(raw.get("captures", 0)),
            "gap_pass": int(raw.get("gap_pass", 0)),
            "renderings": int(raw.get("renderings", 0)),
            "content": int(raw.get("content", 0)),
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
                "artifacts": int(state.get("artifacts", 0)),
                "captures": int(state.get("captures", 0)),
                "gap_pass": int(state.get("gap_pass", 0)),
                "renderings": int(state.get("renderings", 0)),
                "content": int(state.get("content", 0)),
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

    The three shapes are mutually exclusive and ordered earliest-first, because a
    session that never recorded the change has a different next step from one
    that recorded it and stopped there.
    """
    edits = state.get("edits", 0)
    # 1. Something on disk moved and nothing on the record says so.
    if edits > 0 and state.get("changes", 0) == 0:
        return (
            "impact-check — file(s) changed this session with no ChangeEvent "
            "recorded, so nothing computed what the change reaches"
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
    if state.get("renderings", 0) > 0 and state.get("content", 0) == 0:
        return (
            "session-artifacts — a diagram or rendering was written this session "
            "and nothing was stored; if a Decision or Capability points at it, "
            "content_put it and link it, and if nothing points at it, do not "
            "store it"
        )
    return None


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
        return 0

    if kind == "PostToolUse":
        tool = str(event.get("tool_name") or "")
        # This project's reflow2 server; the op is the last __ segment. Any
        # call — even a read — counts as having engaged the design brain.
        if "reflow2" in tool and "__" in tool:
            op = tool.rsplit("__", 1)[-1]

            def touch(state: dict, op: str = op) -> None:
                state["touched"] = True
                if op in LOOP_OPS:
                    state["writes"] = 0
                elif is_write(op):
                    state["writes"] += 1
                # Shape tallies are cumulative and are NOT cleared by a loop
                # check: detect_gaps does not un-edit a file or un-capture
                # intent.
                if op in CHANGE_OPS:
                    state["changes"] += 1
                if op in ARTIFACT_OPS:
                    state["artifacts"] += 1
                if op in CAPTURE_OPS:
                    state["captures"] += 1
                if op in GAP_PASS_OPS:
                    state["gap_pass"] += 1
                if op in CONTENT_OPS:
                    state["content"] += 1

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
                "reason": (
                    f"reflow2: {n} graph write(s) this session and no loop check. "
                    f"{detail} Bookkeeping is not the loop. (This nudge fires "
                    f"once; stopping again proceeds.)"
                ),
            }))
            return 0

        # The upstream bypass (BL-90): code edited, the graph never consulted at
        # all. Blunt by design — the hook cannot know which files are design-
        # relevant, so a count threshold and the once-only rule bound the noise.
        #
        # THIS ONE CLAIM IS DROPPED WHEN THE TALLY WAS REBUILT. It is the only
        # NEGATIVE assertion the hook makes — "the graph was never consulted" —
        # and a tally restarted from nothing cannot support it, because the
        # calls it would have counted are exactly what was lost. The write
        # nudge above is positive ("N writes went unchecked") and survives a
        # restart honestly, which is why only this branch checks the flag.
        if state.get("reset"):
            return 0
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

    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as e:  # noqa: BLE001 — a hook must never break a session
        print(f"loop_nudge: skipped ({e})", file=sys.stderr)
        sys.exit(0)
