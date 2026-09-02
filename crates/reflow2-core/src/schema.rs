//! Load the reflow2 design vocabulary.
//!
//! The 11 composable schema domains in `schema/*.yaml` are the single source of
//! truth for the node/edge vocabulary (29 node types, 60 edge types). They are
//! embedded at compile time with `include_str!` so the core carries its own
//! vocabulary — no runtime file IO, no working-directory dependence, and no
//! second copy to drift out of sync. These are the exact files that
//! `tools/validate_schema.py` checks; here they load through the real
//! dynograph-core path (`Schema::from_multiple_yamls` → merge → validate).

use crate::foundation::core::{DynoError, Schema};

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

/// The parsed schema, built once per process.
///
/// # Why this is cached
///
/// The eleven domains are `include_str!`'d at compile time, so their bytes are
/// fixed for the life of the binary and parsing them twice cannot produce two
/// different answers. It was nonetheless being done on EVERY graph
/// construction, which made `open_in_memory` cost 41.3 ms against 54.7 µs for
/// an ordinary write — construction was 750× a write, and it is setup rather
/// than work (`con:graph-construction-is-setup-not-work`, the budget stated
/// before this was touched).
///
/// The `Result` is cached rather than the `Schema`, so a malformed domain still
/// fails loud on the first call and every call after it. Caching only the
/// success path would turn a broken schema into a panic at an unrelated
/// callsite.
static PARSED_SCHEMA: std::sync::LazyLock<Result<Schema, String>> =
    std::sync::LazyLock::new(|| {
        let yamls: Vec<&str> = SCHEMA_DOMAINS.iter().map(|(_, yaml)| *yaml).collect();
        Schema::from_multiple_yamls(&yamls).map_err(|e| e.to_string())
    });

/// Merge all 11 domains into one validated [`Schema`].
///
/// Fails loud (returns [`DynoError`]) if any domain fails to parse or the
/// merged schema fails validation — never a silently partial vocabulary.
///
/// Returns a CLONE of the process-wide parse. The clone is deliberate and not
/// an oversight: `StorageEngine` takes the schema by value, and two graphs in
/// one process must not share one. Cloning a parsed schema is a memcpy of a few
/// hundred small structs; parsing eleven YAML documents is not.
pub fn load_schema() -> Result<Schema, DynoError> {
    match &*PARSED_SCHEMA {
        Ok(schema) => Ok(schema.clone()),
        // The message is re-wrapped rather than the original error moved,
        // because a cached error has to be returnable more than once.
        Err(message) => Err(DynoError::Schema(message.clone())),
    }
}

/// One `default:` the schema declares, with the enum values it must belong to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredDefault {
    pub node_type: String,
    pub property: String,
    pub value: String,
    /// The property's declared `values`, empty for non-enums (bool, int).
    pub values: Vec<String>,
}

/// Every `default:` the schema declares, parsed from the embedded YAML.
///
/// # Why this exists at all
///
/// **The schema declared 81 defaults and nothing read a single one of them**
/// (measured 2026-08-10, `fact:the-schema-declares-81-defaults-and-nothing-reads-them`).
/// `crate::foundation::core::PropertySpec` has no `default` field, so the key parses to
/// nobody and is silently dropped; nothing in this crate mentioned the word; and
/// `describe_schema` surfaced `required` and `values` but never a default. They
/// were documentation wearing the costume of behaviour — the declared-but-unread
/// class this project keeps finding, arrived at from a new direction.
///
/// The values that DO appear in the data came from the typed constructors
/// injecting them unconditionally, which is why 3,207 stored values across 59%
/// of reflow2's own nodes equal a declared default.
///
/// # Why a hand parser rather than a YAML crate
///
/// `reflow2-core` carries no YAML dependency — dynograph does that parse — and
/// adding one to read four tokens per line would be a dependency bought for a
/// regex. The property lines are uniform by construction and
/// `tools/validate_schema.py` already fails the build if they are not.
///
/// # What this deliberately does NOT do
///
/// It does not apply anything. Making a default *readable* is the precondition
/// for BL-198 (stop storing a value nobody chose, so an absent property honestly
/// means "nobody said"), and that change belongs at the WRITE surface — stripping
/// `value == default` at export cannot work, because the store cannot tell an
/// explicitly-chosen value from an injected one.
pub fn declared_defaults() -> Vec<DeclaredDefault> {
    let mut out = Vec::new();
    for (_, yaml) in SCHEMA_DOMAINS {
        // `    NodeType:` opens a type; its properties are indented further and
        // each is a one-line inline map.
        let mut node_type: Option<String> = None;
        for line in yaml.lines() {
            let indent = line.len() - line.trim_start().len();
            let trimmed = line.trim_end();
            if indent == 4
                && let Some(name) = trimmed.trim().strip_suffix(':')
                && !name.contains(' ')
                && name.chars().next().is_some_and(char::is_uppercase)
            {
                node_type = Some(name.to_string());
                continue;
            }
            let Some(ref nt) = node_type else { continue };
            let Some((key, rest)) = trimmed.trim().split_once(':') else {
                continue;
            };
            if !rest.trim_start().starts_with('{') {
                continue;
            }
            let Some(value) = field(rest, "default") else {
                continue;
            };
            out.push(DeclaredDefault {
                node_type: nt.clone(),
                property: key.trim().to_string(),
                value,
                values: field(rest, "values")
                    .map(|v| {
                        v.trim_matches(['[', ']'])
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect()
                    })
                    .unwrap_or_default(),
            });
        }
    }
    out
}

/// Pull `name: <value>` out of an inline YAML map, honouring `[a, b]` lists.
fn field(line: &str, name: &str) -> Option<String> {
    let at = line.find(&format!("{name}:"))? + name.len() + 1;
    let rest = line[at..].trim_start();
    if let Some(close) = rest.strip_prefix('[').and_then(|r| r.find(']')) {
        return Some(rest[..close + 2].to_string());
    }
    let end = rest
        .find([',', '}'])
        .unwrap_or_else(|| rest.trim_end().len());
    let v = rest[..end].trim();
    (!v.is_empty()).then(|| v.to_string())
}

/// The value the schema declares for a property when nobody said otherwise.
pub fn schema_default(node_type: &str, property: &str) -> Option<String> {
    declared_defaults()
        .into_iter()
        .find(|d| d.node_type == node_type && d.property == property)
        .map(|d| d.value)
}

/// Which node and edge types each schema DOMAIN declares.
///
/// The eleven domains are the schema's own carve-up, and grouping by them is
/// deliberate: `vocabulary_coverage` needs an axis it did not invent. The
/// two-arm trial that produced that feature found the unused vocabulary
/// clustering into whole subsystems — flow, dimensions and readiness, quality
/// gates, governance and risk — and the clusters turned out to BE these
/// domains, so the grouping is a finding rather than a convenience.
///
/// Parses each domain separately rather than reading the merged schema, which
/// is the only way to tell which domain a type came from: the merge is
/// deliberately flat.
/// The node and edge type names one schema domain declares.
pub type DomainTypes = (Vec<String>, Vec<String>);

pub fn domain_membership() -> Result<std::collections::BTreeMap<String, DomainTypes>, DynoError> {
    use std::collections::BTreeMap;

    // A DOMAIN CANNOT BE VALIDATED ALONE, which is why this reads names rather
    // than calling the loader per file: `core.yaml` declares CONTAINS, whose
    // endpoint references `Component` from `structure.yaml`, so
    // `Schema::from_multiple_yamls(&[one])` fails on every cross-domain
    // reference. The schema is deliberately merged flat and attribution is not
    // recoverable from the merged form.
    //
    // So this is a SHALLOW parse of two-space-indented keys — and it is
    // CROSS-CHECKED against the authoritative loader below, which is what
    // stops a hand-rolled reader drifting from the real one. A restructure of
    // the YAML that broke this returns an error instead of a quietly partial
    // answer.
    fn names_under(yaml: &str, section: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut inside = false;
        for line in yaml.lines() {
            let trimmed = line.trim_end();
            if trimmed.trim().is_empty() || trimmed.trim_start().starts_with('#') {
                continue;
            }
            let indent = trimmed.len() - trimmed.trim_start().len();
            if indent == 2 {
                inside = trimmed.trim() == format!("{section}:");
                continue;
            }
            if inside
                && indent == 4
                && let Some(name) = trimmed.trim().strip_suffix(':')
                && name.chars().next().is_some_and(char::is_alphabetic)
            {
                out.push(name.to_string());
            }
        }
        out.sort();
        out
    }

    let mut out: BTreeMap<String, DomainTypes> = BTreeMap::new();
    for (name, yaml) in SCHEMA_DOMAINS {
        out.insert(
            (*name).to_string(),
            (
                names_under(yaml, "node_types"),
                names_under(yaml, "edge_types"),
            ),
        );
    }

    // THE CROSS-CHECK. Every name found must exist in the merged schema, and
    // between them the domains must account for ALL of it. Either half failing
    // means this parser and the real one disagree, and a coverage report built
    // on the disagreement would under-report vocabulary as "used" simply
    // because it was never seen.
    let schema = load_schema()?;
    let mut seen_nodes: Vec<&String> = out.values().flat_map(|(n, _)| n).collect();
    let mut seen_edges: Vec<&String> = out.values().flat_map(|(_, e)| e).collect();
    seen_nodes.sort();
    seen_edges.sort();
    for n in &seen_nodes {
        if !schema.node_types.contains_key(n.as_str()) {
            return Err(DynoError::Query(format!(
                "domain_membership read node type '{n}' that the merged schema does not declare —                  the domain parser and the schema loader disagree"
            )));
        }
    }
    for e in &seen_edges {
        if !schema.edge_types.contains_key(e.as_str()) {
            return Err(DynoError::Query(format!(
                "domain_membership read edge type '{e}' that the merged schema does not declare —                  the domain parser and the schema loader disagree"
            )));
        }
    }
    if seen_nodes.len() != schema.node_types.len() || seen_edges.len() != schema.edge_types.len() {
        return Err(DynoError::Query(format!(
            "domain_membership found {} node types and {} edge types; the merged schema has {}              and {}. Every type must belong to exactly one domain or a coverage report silently              omits vocabulary.",
            seen_nodes.len(),
            seen_edges.len(),
            schema.node_types.len(),
            schema.edge_types.len(),
        )));
    }
    Ok(out)
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
        // 65 since ANSWERS (2026-09-02) — a design record names the Question it
        // answered. The schema had DESCRIBED this edge for months without it
        // existing: `Question.answer` read "the design nodes it produced are
        // linked separately" and nothing linked them. Three independent reports
        // asked for it (chama, api-boss, and the open wording question).
        // 63 since IMPLEMENTS + COMPLEMENTS (2026-08-23) — record-to-record
        // relations, which were thinner in this vocabulary than relations to
        // Requirements: a check can now name the file that RUNS it, and two
        // rules can declare that they stand beside each other and must never
        // be merged. 61 before that, since OWNED_BY (2026-08-09), the third
        // "who" axis: whose AREA this is, durable and never released, distinct
        // from AUTHORED_BY (who wrote it) and CLAIMS (who is in it now).
        //
        // ⭐ THIS ASSERTION IS THE POINT, NOT AN OBSTACLE. Two new edge types
        // move the schema stamp, and the stamp is what makes an older binary
        // REFUSE a graph it cannot read rather than fault on one edge at a
        // time. A count pinned here is what forces the author of the next edge
        // type to notice they owe an upgrade note.
        assert_eq!(schema.edge_types.len(), 65, "expected 65 edge types");
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
