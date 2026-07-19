# Phase coverage — does the golden thread still carry weight after P2?

**What:** reflow2's own design carried through P3 (realization), P4 (verification) and P5 (deploy
and operate), with divergences injected on purpose at each phase and scored on whether the graph
noticed. Driven over the real MCP binary by [`tools/phase_trial.py`](../../tools/phase_trial.py),
which is committed so the result is reproducible and re-runnable.

**Why:** the user's account of the original reflow — *"it did really well in the initial phases,
concept to functional decomposition to system allocation. That set up development well, but after
that, the development and testing and deployment phases, you might as well have ignored the first
phases."* The question was how to test whether reflow2 repeats that.

**Method — deliberate probes, not observation.** The two highest-value findings in the whole trial
record ([ophyd 9](2026-07-18-brownfield-ophyd-service.md),
[3dtictactoe 4](2026-07-18-brownfield-3dtictactoe.md)) came from seeding a defect on purpose and
checking whether the tool noticed. Waiting for divergence to occur naturally inside a trial does not
work; you inject it. A probe scores CAUGHT only if the graph names the problem **without being told
where to look**.

## The bias this was written to correct

Every previous trial stops at or before P2:

| Trial | Reached |
|---|---|
| weather station (blind) | 16 Req, 14 Cap, 9 Cmp, 6 Ifc, 6 Dec — P2 |
| grok weather station | P0–P2 |
| aidrone | P0–P1 |
| 3dtictactoe | 9 Req, 12 Cap, 12 Cmp — P2 |
| ophyd | "P0/P1/P2 seeded" |
| self-host genesis | P0–P1, 0 Components |

So the entire evidence base — every BL item argued from it — is drawn from the phases reflow1 was
*already good at*. That is worth stating plainly: we had no data on the phases that failed.

> **Correction, added after the [erosion trial](2026-07-19-erosion.md).** The P3 score below is
> misleading and the probes were too weak. They test whether the graph notices a drift *event* —
> one file, changed once, detected once. The failure that actually sank the original reflow is
> cumulative: N rounds of test → fix → accept, after which the code is the truth and the design is
> fiction, with no single step wrong. Detecting "this file changed" barely helps, because the answer
> is always *"yes, I know, I fixed a bug."* Read P3 as **4/4 at detecting events, 0/2 at retaining
> coherence across them** — the erosion trial scores that second axis, and the graph reports **zero
> gaps** after full erosion and a release.

## Result

```
  P3 (realization)   4/4 caught
  P4 (verification)  1/4 caught
  P5 (deploy/operate) 0/2 caught
  thread (traceability, both directions)  3/3 caught
```

The shape was predicted before the run, from one structural fact: **there is exactly one reconciler
in the system, `reconcile_artifacts`, and it serves P3.** P4 and P5 have write sides — `add_verification`,
`verifies`, `set_verification_status`, `add_release`, `add_environment`, `deploy_to` — and nothing
that compares what the graph says against what is true.

That is the mechanism, and it is not a discipline problem. In P0–P2 the graph **is** the artifact:
the requirement, the capability and the allocation exist nowhere else, so they cannot drift. From P3
on, reality lives somewhere else — in files, in CI, in production — and the graph is a mirror. A
mirror with no reconciler quietly becomes a painting.

---

## 1. The headline — a failing test silences the gap that asked for it

**Verified three times, twice in isolation from the harness.** `build_without_verification` fires
when a capability has no `Verification`. Attach one and set its status to `failing`, and the gap
goes away:

```
no verification at all      : ['build_without_verification', 'no_deploy_operate']
a verification that FAILS   : ['no_deploy_operate']
```

The gap's own question is *"How will you confirm `<Capability>` actually works?"* It is answered, and
closed, by a test that proves it does not.

And the failure itself is invisible everywhere else. With the status written correctly as `failing`,
`detect_gaps`, `detect_defects` and `graph_report` are **identical to the `passing` case**:

```
--- status written = 'passing'      --- status written = 'failing'
  gaps    : [design_without_build,    gaps    : [design_without_build,
             no_deploy_operate]                  no_deploy_operate]
  defects : []                        defects : []
  report mentions failing?  False     report mentions failing?  False
```

This is the reflow1 failure reproduced in miniature and mechanically. The later-phase checks measure
**the presence of bookkeeping, not the state of reality** — "is there a Verification node?" rather
than "does the thing work?". A design that counts test *nodes* and ignores test *results* is exactly
a design you might as well have ignored once building started.

## 2. P5 has no feedback at all

Both probes missed. A Component allocated a capability and present in no `Release` is not reported;
there is no way to ask whether what is deployed matches what the design says is deployed.
[BL-9](../backlog.md) already names `reconcile_deployment` as a sibling of `reconcile_artifacts`;
this is its first piece of execution evidence rather than inference.

## 3. A `status` field is a claim nothing checks

`Capability.status` was set to `verified` on a capability with no `VERIFIES` edge. Nothing noticed
the contradiction. `unverified_capability` fires on it, but it fires either way — being told "this is
unverified" is not the same as being told **the design contradicts itself**, and only the second is a
coherence failure. The same applies to `Requirement.status = met` with nothing satisfying it.

Sharpened by this session's own work: `add_capability(status)` and `set_capability_status` made these
fields writable (BL-27), so the claims are now easy to make and still unchecked.

## 4. The good news — the thread itself is alive, in both directions

All three traceability probes passed, and this matters as much as the failures:

- a file changed after registration yields `propagation_seeds` (`['cap:detect']`),
- propagating from those seeds **reaches the Requirement behind the code** (17 nodes),
- and a change to a Requirement reaches the **Artifacts** that implement it.

So the spine is not the problem. Impact propagation crosses the P0↔P3 boundary correctly in both
directions. What is missing is anything that *feeds reality into* the spine after P3. That is a
narrower and much more tractable diagnosis than "the later phases don't work".

## 5. Live-session binary skew — found by accident, worth its own item

The trial was nearly run against the wrong binary. The MCP server this session was talking to had
been started at session open; every tool added since — `set_capability_status`, `set_provenance` —
did not exist on it, and `graph_report` still ranked gaps by the pre-fix rule. Nothing indicated the
server was stale.

This is **not** [BL-18](../backlog.md), which compares an installed kit against the remote HEAD. It
is narrower and hits developers and agents mid-session: rebuild the binary, and the running server
keeps serving the old surface silently until it restarts. `tools/smoke_mcp.py` structurally cannot
catch it, because it spawns a fresh binary on every run — the fourth instance of "a client we wrote
agreeing with itself."

## What this says to do

The diagnosis is narrow: **the golden thread works; nothing feeds it reality after P3.** So the fix
is reconcilers and result-aware detectors, not a rework of the spine.

Raised from this trial: **BL-30** (P4 has no reality feedback — the headline, and the
gap-silencing half is **S**), **BL-31** (status fields are unchecked claims), **BL-32** (live-session
binary skew). **BL-9** gains its first execution evidence.

`tools/phase_trial.py` exits non-zero today, on purpose — it records a known-failing baseline of
5 missed probes. When BL-30 and BL-9 land it should reach 10/10, and it is then a standing gate
against the whole class regressing.
