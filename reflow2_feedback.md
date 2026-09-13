## reflow2 feedback — 2026-09-12, window: since the ledger began (no previous report; one day, 10:53–15:49 local)

### Environment
reflow2 0.59.0 · linux/x86_64 · harness claude-code 2.1.269 · MCP 2025-11-25 · design 4356 nodes
model: claude-fable-5-1 (self-reported). Note for a maintainer reading the ledger by seat: the harness's own attribution named Opus 5 for the first part of this same session and Fable 5.1 for the rest, so one session can span two models and the ledger cannot see the change.

### Usage
168 calls across 46 tools; top: loop_status ×37, record_finding ×22, pin_at_epoch ×10, export_graph ×8, add_verification ×6, set_artifact_checksums ×6, add_capability ×5, scan_nodes ×5
clients: claude-code ×135, rmcp ×26 (the Stop/PostToolUse probe), steps-setter ×6, ledger-peek ×1 · 49 seats
skills fetched: where-am-i ×1 — BUT three more were used this session (optimize, root-cause, feedback) through the harness's local slash-command copies, which never touch get_skill. The tally undercounts skill use whenever the kit's commands are installed locally; the ledger has no way to see that.
never called: 133 of 181 served tools. One day of release-cut then optimisation work — the capture surface (add_component, allocate, satisfies, gap_to_prompt…) had no reason to be touched. Nothing wanted and not found. find_tools was not called once, for what that is worth after a session spent making every tool findable through it.

### Refusals and errors
No errors. 8 refusals, 7 of them in class `other` — the class table does not know two phrasings that appeared today, and that is the main finding of this table.

| tool | class | count | disposition | note |
| --- | --- | --- | --- | --- |
| record_finding | other | 1 | guard working (class wrong) | A list-typed parameter (`steps`, new in 0.59.0) arrived as a comma-joined string because the harness caches tool schemas per session; the server refused with a serde "expected a sequence". Correct refusal. Lands in `other` because "failed to deserialize parameters" is not a known class — this is the `invalid_argument` class the last report said was owed. |
| record_finding | other | 4 | unexplained | Previous session's rows; not witnessed here. The prior report already noted these four landed in `other`. |
| add_epoch | other | 1 | guard working (class wrong) | Create without `sequence`; the refusal named the next integer and why. Correct and helpful. In substance a `missing_argument`, but the wording "X is required to CREATE" does not match the class table, so it lands in `other`. |
| add_design_rule | other | 1 | unexplained | Previous session's row. |
| export_graph | refused | 1 | unexplained | Previous session's row. |

Shape of the fix the table is asking for: two more class patterns — the serde deserialize failure → `invalid_argument`, and "is required to CREATE" → `missing_argument`. With those, 2 of today's 8 rows leave `other`; the other 5 belong to a session that would need to disposition them itself.

### What the ledger cannot see
- Every hook fired TWICE all day: `.claude/settings.json` and `.claude/settings.local.json` both registered the same four hooks, and the probe's in-flight guard was check-then-write, so both racers spawned. Visible in the ledger only as 14 same-second loop_status pairs from `rmcp` with durations within 1% — 211.8 s, 25% of all tool time today. Fixed (duplicate registration removed; guard made atomic; pinned by a test that failed 4≠1 first).
- The harness-side half of the record_finding row above: a per-session schema cache means any NEW list parameter arrives as a string until the harness restarts. reflow2 can only refuse it; it cannot cause or cure it.
- My own mistake, no reflow2 defect: a throwaway test server launched with `--export-to` pointed at the real committed export wrote 8 probe nodes into docs/design/reflow2.json. The loop hint caught it; restored from HEAD. Nothing could have refused it — the copy had adopted the live design's identity, so the target was legitimately "its" record.
- Machine load: sar shows load 6–8.5 with 12 blocked during a cargo release build; the ledger stamped the calls in that window at 3–6× their quiet cost and has no column to say why.
