# Upgrading to v0.7.0

One schema addition, no action required.

**What changed**: `Snapshot` gained an optional `edges` property (BL-63). Snapshots taken by
v0.7.0 capture the node's design edges — direction, edge type, other endpoint, edge properties —
beside the property bag in `state`, so an edge move (a re-allocation, a retargeted `satisfies`)
keeps its history instead of surviving only in a hand-authored Decision.

**What it means for an existing graph**:

- **Your graph opens unchanged.** The property is optional; nothing is migrated, rewritten, or
  refused.
- **Old snapshots stay readable.** A snapshot taken before v0.7.0 simply has no `edges`
  property; `parse_snapshot_edges` returns an empty list for it — the edge history was not
  recorded then, and reflow2 does not invent a past.
- **New snapshots are richer from the first `record_change` after the upgrade.** No opt-in.

**One behavioural note**: the `single_point_of_failure` detector now measures connectivity on
the as-built operational network (BL-69, in the same release window). Your defect list may
change — false positives on leaf modules disappear, and genuine cut vertices that intent edges
previously hid may appear. A finding that grows the list is the detector telling the truth;
disposition real-but-intentional SPOFs with a Decision (`governed_by`), as reflow2's own design
does.

As always: export with the old build before upgrading if you want a belt-and-suspenders backup
(`reflow2-mcp --export`), though this upgrade does not need it.
