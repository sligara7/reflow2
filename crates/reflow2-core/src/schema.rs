//! Load the reflow2 design vocabulary.
//!
//! The 10 composable schema domains in `schema/*.yaml` are the single source of
//! truth for the node/edge vocabulary (27 node types, 53 edge types). They are
//! embedded at compile time with `include_str!` so the core carries its own
//! vocabulary — no runtime file IO, no working-directory dependence, and no
//! second copy to drift out of sync. These are the exact files that
//! `tools/validate_schema.py` checks; here they load through the real
//! dynograph-core path (`Schema::from_multiple_yamls` → merge → validate).

use dynograph_core::{DynoError, Schema};

/// The 10 schema domains, as `(name, yaml)`, embedded at compile time.
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
];

/// Merge all 10 domains into one validated [`Schema`].
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
        let schema = load_schema().expect("the 10 domains must merge and validate");
        // The vocabulary the docs and README commit to: 27 node types, 52 edges.
        assert_eq!(schema.node_types.len(), 27, "expected 27 node types");
        assert_eq!(schema.edge_types.len(), 53, "expected 53 edge types");
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
