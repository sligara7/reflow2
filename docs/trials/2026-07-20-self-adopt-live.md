# Trial: first live self-adopt session — stub-survivor reconciliation (2026-07-20)

The session COORD parked under "Blocked / waiting": first session after the restart, running
reflow2 on its own live graph through the installed kit, from Claude Code. The plan written
there was followed exactly: **where-am-i**, then **detect-and-ask** to reconcile the
genesis-stub survivor nodes *with the user*, then **check-health**'s detectors around the
merges. Server identity checked first: `served_by 0.5.0`, `binary_mtime_unix` matching disk.

## What ran

- `where-am-i` gather: graph_report + Decisions + gaps + open_questions (0) + reviewed_gaps (0).
  11 gaps, all anchored on the four predicted stub survivors (`cap:store`, `cap:artifacts`,
  `cap:install`, `req:platform`); defects included the two predicted stub clusters. One
  duplicate the detectors never flagged: `req:past-preserved` vs `req:intent-preserved` — same
  name, byte-different statements.
- `gap_to_prompt` handshake on 5 representative gaps; questions recorded; user answered all
  four decision points (merge all three duplicate pairs; keep + wire `cap:store`; `req:platform`
  covered by `cap:kit`; storyflow trial recorded as `cap:adopt`'s proof).
- `add_change_event chg:stub-survivor-reconciliation` + 9 CHANGED edges + `propagate_change`
  **before** editing: 142 impacted, distance-1 ring exactly the expected neighborhood, 0 risk
  crossings.
- Descriptions folded into survivors first, then DUPLICATES edges, then
  `propose_heal`(balanced) → review → `apply_heal`: 3 merges, `verified: true`.
- `cap:store` wired: provenance authored, allocated to `cmp:schema` + `cmp:graph`, `ver:store`
  added and set passing against a **live** `cargo test --no-default-features` run in this
  session (exit 0), status verified. `ver:adopt-storyflow` (acceptance) added passing;
  `cap:adopt` verified. `req:platform` satisfied by `cap:kit`, accepted.
- BL-29's queued Decision node landed: `dec:merge-survivor-provenance`, GOVERNED_BY from
  `cmp:heal`.
- Re-run: **gaps 11 → 0. Defects 6 → 4** (three operational-SPOF warnings + the
  `{env:dev, rel:v040}` island). Export refreshed: 197 nodes / 370 edges, stamp 0.5.0.

## Findings (verbatim evidence, numbered; backlog ids where work was raised)

- **F1 → BL-48.** `graph_report_markdown` unusable from Claude Code: *"MCP server \"reflow2\"
  returned a malformed result that failed schema validation … path: structuredContent …
  expected record, received string"*. JSON `graph_report` was the fallback.
- **F2 → BL-46.** `create_node` on an existing id replaces the whole property object with
  supplied-props-over-schema-defaults. Folding a description into `cap:kit` reset
  `status: verified → planned`; on `req:intent-preserved` it also reset `priority: high →
  medium` and `status: accepted → proposed`. Recovered via the typed setters and a full-props
  rewrite.
- **F3 → BL-47.** First `propose_heal`(balanced) proposed keeping stub `cap:install` and
  **removing authored, verified `cap:kit`** (likewise `cap:artifacts` over
  `cap:reconcile-built`): the stubs' unset provenance defaulted to `authored`, tied, and the id
  tiebreak went alphabetical. Caught in review; fixed by `set_provenance planned` on the stubs,
  after which the re-proposal pointed all three merges the right way. The third merge had been
  right only because "intent-preserved" sorts before "past-preserved".
- **F4 → BL-49.** `propagate_change` (70,595 chars) and `export_graph` (93,324 chars) both
  overflowed the tool-result budget; read only via the harness spill file + `jq`.
- **F5 → BL-50(1).** `DUPLICATES.confidence: 1` rejected: *"expected type Float, got int"*.
  Worked around by omitting the optional property.
- **F6 → BL-47 (noted).** `apply_heal`'s edge re-point let the stub's CHANGED edge properties
  overwrite the survivor's (`action: removed` clobbered `modified`) — reported in `discarded`,
  fixed by re-asserting the edge.
- **F7 → BL-50(2).** `add_change_event` has no way to say what it changed; nine generic
  `create_edge CHANGED` calls, with `describe_schema` ranking CHANGED in the wildcard bucket.
- **F8 → BL-50(3).** The where-am-i-at-session-start ritual fired only because CLAUDE.md →
  COORD.md routed the agent there; the user expected it to be automatic. Convention, no
  mechanism.
- **Observations, no item:** `reviewed_gaps` came back empty — the earlier "cap:adopt gap is
  deliberate" judgement was never recorded (now moot: verified instead). The graph carries
  `rel:v040` but **no Release node for v0.5.0** while `req:released-eq-designed` is critical —
  the as-released model is one release behind reality; wants a session with `add_release` +
  `release_includes` + `release_report`, and wiring `{env:dev, rel:v040}` while there.
  Confirmation ledger: 25/25 claims unexamined — the ledger has never been exercised on this
  graph. Release run 29785834848 still `queued` on GitHub at session time; the re-dispatch
  follow-up stays parked.

## Verdict

The loop held on its own graph: detect → ask → answer → record → heal → re-detect reached zero
open gaps with every decision on the record, and the survivor rule + review gate caught the one
proposal that would have destroyed authored work. The instrument found five real defects in
itself while doing it — which is the point.
