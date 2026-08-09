//! Load the reflow2 design vocabulary.
//!
//! The 11 composable schema domains in `schema/*.yaml` are the single source of
//! truth for the node/edge vocabulary (29 node types, 60 edge types). They are
//! embedded at compile time with `include_str!` so the core carries its own
//! vocabulary — no runtime file IO, no working-directory dependence, and no
//! second copy to drift out of sync. These are the exact files that
//! `tools/validate_schema.py` checks; here they load through the real
//! dynograph-core path (`Schema::from_multiple_yamls` → merge → validate).

use dynograph_core::{DynoError, Schema};

/// The 11 schema domains, as `(name, yaml)`, embedded at compile time.
///
/// Order is not load-bearing: `from_multiple_yamls` merges additively and
/// validates once at the end, so cross-domain edge endpoints (e.g. an edge in
/// `structure` pointing at a node in `functional`) resolve regardless of order.
pub const SCHEMA_DOMAINS: &[(&str, &str)] = &[
    ("core", include_str!("../../../schema/core.yaml")),
    (
        "functional",
        include_str!("../../../schema/functional.yaml"),
    ),
    ("structure", include_str!("../../../schema/structure.yaml")),
    ("build", include_str!("../../../schema/build.yaml")),
    ("verify", include_str!("../../../schema/verify.yaml")),
    ("operate", include_str!("../../../schema/operate.yaml")),
    (
        "environment",
        include_str!("../../../schema/environment.yaml"),
    ),
    ("temporal", include_str!("../../../schema/temporal.yaml")),
    ("inference", include_str!("../../../schema/inference.yaml")),
    (
        "dimensions",
        include_str!("../../../schema/dimensions.yaml"),
    ),
    ("readiness", include_str!("../../../schema/readiness.yaml")),
];

/// Merge all 11 domains into one validated [`Schema`].
///
/// Fails loud (returns [`DynoError`]) if any domain fails to parse or the
/// merged schema fails validation — never a silently partial vocabulary.
pub fn load_schema() -> Result<Schema, DynoError> {
    let yamls: Vec<&str> = SCHEMA_DOMAINS.iter().map(|(_, yaml)| *yaml).collect();
    Schema::from_multiple_yamls(&yamls)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_domains_merge_and_validate() {
        let schema = load_schema().expect("the 11 domains must merge and validate");
        // The vocabulary the docs and README commit to. History: 26→27 nodes
        // was BL-4 (Question), 53→54 edges is BL-34 (INCLUDES — the
        // 55→56 is PERFORMED_IN (2026-07-27, req:design-the-simulator), and
        // unlike an added enum value this one MOVES THE STAMP: an older reflow2
        // refuses a graph carrying more edge types than it knows (BL-94), so it
        // needs an upgrade note rather than a shrug.
        // as-released view); 55→53 edges is the edge-orthogonality retirement of
        // VALIDATES + ENABLES (dec:edge-orthogonality — VALIDATES moved to
        // Verification.kind, ENABLES folded into CAUSES). Every bump moves
        // GraphStamp, so a graph written by this schema is refused by older
        // binaries — deliberate, loud, and worth a CHANGELOG entry each time (BL-19).
        // 56→57 is SCHEDULED_FOR (2026-07-30, req:epochs-can-be-planned second
        // increment) — the satisfaction schedule, kept separate from AT_EPOCH
        // because that one means *belongs to* over a wildcard source, and one
        // type carrying both meanings would be indistinguishable to every
        // detector (dec:schedule-is-an-edge-with-modality). Moves the stamp.
        // 57→58 is CALIBRATED_AGAINST (2026-08-01, req:a-fit-is-not-a-test) —
        // what a value was FITTED to, so the same evidence cannot count as its
        // validation. A new type rather than a property because the detector is
        // then a graph query, and because the relation carries WHAT was
        // consumed, which a provenance enum cannot (dec:calibration-is-an-edge).
        // Moves the stamp: v0.21.0 owes an upgrade note.
        // 28→29 nodes and 58→60 edges is BL-68 (2026-08-02): ReadinessAssessment
        // plus GATED_ON and HAS_READINESS — the eleventh domain, `readiness`.
        // Readiness is deliberately NOT a `dimension` enum value: a 1-9 ladder
        // only enters a 0.0-1.0 float lossily, the enum is a closed list of
        // QUALITY axes, and `maturity` already sits there meaning code maturity
        // (dec:readiness-is-an-observation-the-threshold-is-the-judgement).
        // GATED_ON carries the threshold as an EDGE property so one increment
        // can demand TRL 7 of one technology and 4 of another. Moves the stamp:
        // the next release owes an upgrade note.
        assert_eq!(schema.node_types.len(), 29, "expected 29 node types");
        // 61 since OWNED_BY (2026-08-09) — the third "who" axis: whose AREA
        // this is, durable and never released, distinct from AUTHORED_BY
        // (who wrote it) and CLAIMS (who is in it right now). Moves the
        // stamp: the next release owes an upgrade note.
        assert_eq!(schema.edge_types.len(), 61, "expected 61 edge types");
    }

    #[test]
    fn golden_thread_types_present() {
        let schema = load_schema().unwrap();
        for nt in ["Project", "Requirement", "Capability", "Component"] {
            assert!(
                schema.node_types.contains_key(nt),
                "schema must define node type {nt}"
            );
        }
        for et in ["CONTAINS", "SATISFIES", "ALLOCATED_TO"] {
            assert!(
                schema.edge_types.contains_key(et),
                "schema must define edge type {et}"
            );
        }
    }
}
