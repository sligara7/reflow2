//! Schema discovery — what vocabulary does this graph actually accept?
//!
//! The schema is the source of truth for which node types exist, which
//! properties they require, and which edge types may join which endpoints. All
//! of it is already reachable through [`DesignGraph::schema`], and none of it
//! was reachable from a caller: an agent could only learn the vocabulary by
//! guessing at `create_edge` until something validated. The blind trial did
//! exactly that — fourteen guesses to connect a `Release` to a `Component` —
//! and its complaint is the design brief for this module: the error "told me I
//! was wrong without telling me what was right".
//!
//! # Validation is not endorsement
//!
//! [`EdgeEndpoint::accepts`] answers "would this write pass?", and it returns
//! `true` for the `*` wildcard. So the honest answer to "what connects a
//! `Release` to a `Component`?" is not a flat list — a wildcard edge type is
//! *permitted* between them without *modelling* them. That is the trap the
//! trial fell into: it settled on `DEPENDS_ON` "because it was the one that
//! validated", which is precisely the silent accommodation this project is
//! against.
//!
//! So every match here carries [`EndpointMatch`], separating an endpoint that
//! names a type from one that merely accepts everything, and
//! [`EdgeQuery::note`] says so in words. Ranking puts exact matches first and
//! never hides the wildcards — they are legal, and sometimes right.
//!
//! Deterministic and LLM-free: results are sorted by name, so repeated calls
//! are byte-identical (the schema's backing `HashMap`s have no stable order).

use dynograph_core::{DynoError, EdgeEndpoint, EdgeTypeDef, NodeTypeDef, PropertyDef};
use serde::Serialize;

use crate::graph::DesignGraph;

/// Why a node type was accepted at an edge endpoint.
///
/// The distinction is the point of this module: `Wildcard` means the endpoint
/// accepts *everything*, so a match says nothing about whether the edge type
/// means what the caller intends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EndpointMatch {
    /// The endpoint names this node type explicitly.
    Exact,
    /// The endpoint is `*`; it accepts this type because it accepts any type.
    Wildcard,
}

/// One property of a node or edge type, flattened for display.
#[derive(Debug, Clone, Serialize)]
pub struct PropertySpec {
    pub name: String,
    /// Schema type, lowercased (`string`, `enum`, `float`, …).
    pub prop_type: String,
    pub required: bool,
    /// Allowed values, for `enum` properties.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub values: Option<Vec<String>>,
    /// `[min, max]`, for numeric properties.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<(f64, f64)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl PropertySpec {
    fn new(name: &str, def: &PropertyDef) -> Self {
        Self {
            name: name.to_string(),
            prop_type: format!("{:?}", def.prop_type).to_lowercase(),
            required: def.required,
            values: def.values.clone(),
            range: def.range,
            description: def.description.clone(),
        }
    }
}

/// A node type: what it is, and what a `create_node` call must supply.
#[derive(Debug, Clone, Serialize)]
pub struct NodeTypeSpec {
    pub node_type: String,
    /// The schema's own description of when to use this type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    pub properties: Vec<PropertySpec>,
}

/// An edge type: what it joins, and what it means.
#[derive(Debug, Clone, Serialize)]
pub struct EdgeTypeSpec {
    pub edge_type: String,
    /// Legal source types; `["*"]` for the wildcard.
    pub from: Vec<String>,
    /// Legal target types; `["*"]` for the wildcard.
    pub to: Vec<String>,
    /// The schema's own description of what this edge asserts. The field that
    /// lets a caller judge *meaning* once several candidates all validate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub properties: Vec<PropertySpec>,
}

/// An edge type that accepts a specific `from`/`to` pair, and on what basis.
#[derive(Debug, Clone, Serialize)]
pub struct EdgeTypeMatch {
    #[serde(flatten)]
    pub spec: EdgeTypeSpec,
    pub from_match: EndpointMatch,
    pub to_match: EndpointMatch,
}

impl EdgeTypeMatch {
    /// True when both endpoints name their type explicitly — the edge type
    /// models this pair rather than merely tolerating it.
    pub fn is_exact(&self) -> bool {
        self.from_match == EndpointMatch::Exact && self.to_match == EndpointMatch::Exact
    }

    /// Order: exact pairs first, then by name. Deterministic across calls.
    ///
    /// A comparator rather than a `sort_by_key`, which would force a `String`
    /// clone of the edge type on every comparison.
    fn order(a: &Self, b: &Self) -> std::cmp::Ordering {
        (a.from_match, a.to_match)
            .cmp(&(b.from_match, b.to_match))
            .then_with(|| a.spec.edge_type.cmp(&b.spec.edge_type))
    }
}

/// The full vocabulary — every node and edge type in the merged schema.
#[derive(Debug, Clone, Serialize)]
pub struct Vocabulary {
    pub node_types: Vec<NodeTypeSpec>,
    pub edge_types: Vec<EdgeTypeSpec>,
}

/// One node type in context: its properties and the edges it can carry.
#[derive(Debug, Clone, Serialize)]
pub struct NodeTypeDetail {
    #[serde(flatten)]
    pub spec: NodeTypeSpec,
    /// Edge types that accept this type as their source.
    pub outgoing: Vec<EdgeTypeMatch>,
    /// Edge types that accept this type as their target.
    pub incoming: Vec<EdgeTypeMatch>,
}

/// The answer to "what may connect an X to a Y?".
#[derive(Debug, Clone, Serialize)]
pub struct EdgeQuery {
    pub from_type: String,
    pub to_type: String,
    /// Exact matches first, then wildcard-only ones. Never truncated.
    pub matches: Vec<EdgeTypeMatch>,
    /// How many of `matches` model this pair explicitly. Zero here with a
    /// non-empty `matches` is the case worth reading `note` for.
    pub exact_matches: usize,
    /// How many of `matches` name one endpoint exactly and are open on the
    /// other **by design** — e.g. `CHANGED`, whose source is always a
    /// ChangeEvent while its target is anything a change can touch. For its
    /// pair such an edge is the modelled fit, not a wildcard loophole; before
    /// this count existed it was presented as merely tolerating the pair
    /// (BL-50). Sorted after the exact fits, before both-sides wildcards.
    pub half_exact_matches: usize,
    /// Plain-language reading of the result, so a caller that only skims sees
    /// the wildcard caveat rather than treating any hit as a fit.
    pub note: String,
}

/// Resolve an endpoint to its list of type names.
fn endpoint_types(endpoint: &EdgeEndpoint) -> Vec<String> {
    match endpoint {
        EdgeEndpoint::Single(t) => vec![t.clone()],
        EdgeEndpoint::Multiple(ts) => {
            let mut ts = ts.clone();
            ts.sort();
            ts
        }
        // `EdgeEndpoint` is `#[non_exhaustive]`; a variant added upstream must
        // not silently read as "matches nothing".
        other => vec![format!("<unsupported endpoint: {other:?}>")],
    }
}

/// Classify how `node_type` matches `endpoint`, or `None` if it does not.
///
/// Deliberately not `accepts()` alone: that collapses "names this type" and
/// "accepts anything" into one `true`, which is the distinction callers need.
fn classify(endpoint: &EdgeEndpoint, node_type: &str) -> Option<EndpointMatch> {
    if !endpoint.accepts(node_type) {
        return None;
    }
    let named = endpoint_types(endpoint).iter().any(|t| t == node_type);
    Some(if named {
        EndpointMatch::Exact
    } else {
        EndpointMatch::Wildcard
    })
}

fn edge_spec(name: &str, def: &EdgeTypeDef) -> EdgeTypeSpec {
    let mut properties: Vec<PropertySpec> = def
        .properties
        .iter()
        .map(|(n, d)| PropertySpec::new(n, d))
        .collect();
    properties.sort_by(|a, b| a.name.cmp(&b.name));
    EdgeTypeSpec {
        edge_type: name.to_string(),
        from: endpoint_types(&def.from),
        to: endpoint_types(&def.to),
        hint: def.extraction_hint.as_ref().map(|h| h.trim().to_string()),
        properties,
    }
}

fn node_spec(name: &str, def: &NodeTypeDef) -> NodeTypeSpec {
    let mut properties: Vec<PropertySpec> = def
        .properties
        .iter()
        .map(|(n, d)| PropertySpec::new(n, d))
        .collect();
    // Required first, then alphabetical: a caller reading top-down sees what
    // it must supply before what it may.
    properties.sort_by(|a, b| b.required.cmp(&a.required).then(a.name.cmp(&b.name)));
    NodeTypeSpec {
        node_type: name.to_string(),
        hint: def.extraction_hint.as_ref().map(|h| h.trim().to_string()),
        properties,
    }
}

impl DesignGraph {
    /// Every node and edge type in the merged schema, sorted by name.
    pub fn describe_vocabulary(&self) -> Vocabulary {
        let schema = self.schema();

        let mut node_types: Vec<NodeTypeSpec> = schema
            .node_types
            .iter()
            .map(|(n, d)| node_spec(n, d))
            .collect();
        node_types.sort_by(|a, b| a.node_type.cmp(&b.node_type));

        let mut edge_types: Vec<EdgeTypeSpec> = schema
            .edge_types
            .iter()
            .map(|(n, d)| edge_spec(n, d))
            .collect();
        edge_types.sort_by(|a, b| a.edge_type.cmp(&b.edge_type));

        Vocabulary {
            node_types,
            edge_types,
        }
    }

    /// Fail loud on a node type the schema does not define, listing near
    /// misses. An unknown type must not read as "exists, but has no edges".
    fn require_node_type(&self, node_type: &str) -> Result<&NodeTypeDef, DynoError> {
        self.schema()
            .node_types
            .get(node_type)
            .ok_or_else(|| DynoError::UnknownNodeType(node_type.to_string()))
    }

    /// One node type's properties, plus every edge type that can leave or
    /// arrive at it.
    pub fn describe_node_type(&self, node_type: &str) -> Result<NodeTypeDetail, DynoError> {
        let def = self.require_node_type(node_type)?;
        let spec = node_spec(node_type, def);

        let mut outgoing = Vec::new();
        let mut incoming = Vec::new();
        for (name, edge_def) in &self.schema().edge_types {
            if let Some(from_match) = classify(&edge_def.from, node_type) {
                // The far endpoint is unconstrained by this query, so report
                // how it would match *itself* — i.e. whether it is a wildcard.
                let to_match = wildcard_or_exact(&edge_def.to);
                outgoing.push(EdgeTypeMatch {
                    spec: edge_spec(name, edge_def),
                    from_match,
                    to_match,
                });
            }
            if let Some(to_match) = classify(&edge_def.to, node_type) {
                let from_match = wildcard_or_exact(&edge_def.from);
                incoming.push(EdgeTypeMatch {
                    spec: edge_spec(name, edge_def),
                    from_match,
                    to_match,
                });
            }
        }
        outgoing.sort_by(EdgeTypeMatch::order);
        incoming.sort_by(EdgeTypeMatch::order);

        Ok(NodeTypeDetail {
            spec,
            outgoing,
            incoming,
        })
    }

    /// Which edge types may join `from_type` to `to_type`, exact fits first.
    ///
    /// Both types must exist; an unknown name is an error rather than an empty
    /// list, so a typo cannot look like "nothing connects these".
    pub fn edge_types_between(
        &self,
        from_type: &str,
        to_type: &str,
    ) -> Result<EdgeQuery, DynoError> {
        self.require_node_type(from_type)?;
        self.require_node_type(to_type)?;

        let mut matches: Vec<EdgeTypeMatch> = self
            .schema()
            .edge_types
            .iter()
            .filter_map(|(name, def)| {
                let from_match = classify(&def.from, from_type)?;
                let to_match = classify(&def.to, to_type)?;
                Some(EdgeTypeMatch {
                    spec: edge_spec(name, def),
                    from_match,
                    to_match,
                })
            })
            .collect();
        matches.sort_by(EdgeTypeMatch::order);

        let exact_matches = matches.iter().filter(|m| m.is_exact()).count();
        let half_exact_matches = matches
            .iter()
            .filter(|m| {
                !m.is_exact()
                    && (m.from_match == EndpointMatch::Exact || m.to_match == EndpointMatch::Exact)
            })
            .count();
        let note = edge_query_note(
            from_type,
            to_type,
            matches.len(),
            exact_matches,
            half_exact_matches,
        );

        Ok(EdgeQuery {
            from_type: from_type.to_string(),
            to_type: to_type.to_string(),
            matches,
            exact_matches,
            half_exact_matches,
            note,
        })
    }
}

/// How an endpoint reads when the query does not constrain it.
fn wildcard_or_exact(endpoint: &EdgeEndpoint) -> EndpointMatch {
    if endpoint_types(endpoint)
        .iter()
        .any(|t| t == EdgeEndpoint::WILDCARD)
    {
        EndpointMatch::Wildcard
    } else {
        EndpointMatch::Exact
    }
}

/// The plain-language verdict. Separated out so the wording is testable and
/// shared with the `create_edge` failure path.
fn edge_query_note(
    from_type: &str,
    to_type: &str,
    total: usize,
    exact: usize,
    half: usize,
) -> String {
    match (total, exact, half) {
        (0, _, _) => format!(
            "No edge type in this schema connects {from_type} to {to_type}. \
             Either the relationship belongs somewhere else in the design, or it \
             needs a new edge type in a schema domain — do not force it through an \
             edge that means something different."
        ),
        (n, 0, 0) => format!(
            "No edge type specifically models {from_type} -> {to_type}. The {n} below \
             accept the pair only because an endpoint is the `*` wildcard: they will \
             validate, but validating is not the same as meaning what you intend. \
             Read each `hint` before choosing, and prefer leaving the edge out to \
             asserting one that is wrong."
        ),
        (n, 0, h) => format!(
            "No edge type names both {from_type} and {to_type}, but {h} of the {n} \
             below (listed first) name one side exactly and are open on the other \
             by design — for its pair such an edge is the modelled fit, not a \
             loophole; read its `hint`. The rest accept the pair only through a \
             `*` wildcard on both sides."
        ),
        (_, k, _) => format!(
            "{k} edge type(s) explicitly model {from_type} -> {to_type}; these are \
             listed first. Any others accept the pair via a `*` wildcard endpoint."
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph() -> DesignGraph {
        DesignGraph::open_in_memory().expect("schema loads")
    }

    #[test]
    fn vocabulary_covers_the_whole_schema() {
        let v = graph().describe_vocabulary();
        assert_eq!(v.node_types.len(), 27, "all node types are listed");
        assert_eq!(v.edge_types.len(), 54, "all edge types are listed");
    }

    #[test]
    fn vocabulary_is_deterministic() {
        let g = graph();
        let a = serde_json::to_string(&g.describe_vocabulary()).unwrap();
        let b = serde_json::to_string(&g.describe_vocabulary()).unwrap();
        assert_eq!(a, b, "repeated calls must be byte-identical");
    }

    #[test]
    fn exact_endpoints_are_distinguished_from_wildcards() {
        let g = graph();
        // ALLOCATED_TO is declared `Capability -> Component`: both exact.
        let q = g.edge_types_between("Capability", "Component").unwrap();
        let allocated = q
            .matches
            .iter()
            .find(|m| m.spec.edge_type == "ALLOCATED_TO")
            .expect("ALLOCATED_TO connects Capability to Component");
        assert!(allocated.is_exact());
        assert!(q.exact_matches >= 1);
        assert!(
            q.matches.first().unwrap().is_exact(),
            "exact matches rank first"
        );
    }

    /// The blind trial's actual question, and its answer has a history. The
    /// trial brute-forced fourteen guesses and settled on `DEPENDS_ON` because
    /// it validated; BL-1 made the vocabulary say plainly that *nothing models
    /// Release → Component*, and this test pinned that honest emptiness — with
    /// the note "if one is added, update this test". BL-34 added one:
    /// `INCLUDES` is the as-released containment the trial was reaching for
    /// all along, so the right answer changed from a caveat to an exact fit.
    #[test]
    fn release_to_component_reports_the_as_released_edge() {
        let g = graph();
        let q = g.edge_types_between("Release", "Component").unwrap();
        assert_eq!(
            q.exact_matches, 1,
            "INCLUDES models Release -> Component since BL-34"
        );
        assert!(
            q.matches
                .iter()
                .any(|m| m.is_exact() && m.spec.edge_type == "INCLUDES"),
            "the exact fit is INCLUDES"
        );
    }

    /// The self-adopt session's actual question (BL-50): what connects a
    /// ChangeEvent to an Artifact? The answer is CHANGED — its from-side names
    /// ChangeEvent exactly; its to-side is `*` because a change can touch
    /// anything. That is the edge type *designed* for the pair, and it used to
    /// be presented as if it merely tolerated it.
    #[test]
    fn change_event_to_artifact_reports_changed_as_the_modelled_fit() {
        let g = graph();
        let q = g.edge_types_between("ChangeEvent", "Artifact").unwrap();
        assert_eq!(q.exact_matches, 0, "nothing names both sides");
        assert!(q.half_exact_matches >= 1, "CHANGED names its from-side");
        let first = q.matches.first().unwrap();
        assert_eq!(
            first.spec.edge_type, "CHANGED",
            "the half-exact fit ranks above both-sides wildcards"
        );
        assert!(
            q.note.contains("modelled fit"),
            "the note must not call the designed edge a wildcard loophole: {}",
            q.note
        );
    }

    #[test]
    fn unknown_node_type_is_an_error_not_an_empty_list() {
        let g = graph();
        let err = g.edge_types_between("Relese", "Component").unwrap_err();
        assert!(
            matches!(err, DynoError::UnknownNodeType(t) if t == "Relese"),
            "a typo must fail loud, not look like 'nothing connects these'"
        );
        assert!(g.describe_node_type("Nope").is_err());
    }

    #[test]
    fn node_detail_lists_edges_in_both_directions() {
        let g = graph();
        let d = g.describe_node_type("Component").unwrap();
        assert_eq!(d.spec.node_type, "Component");
        assert!(
            d.incoming
                .iter()
                .any(|m| m.spec.edge_type == "ALLOCATED_TO"),
            "Capability -> Component arrives at Component"
        );
        assert!(
            d.outgoing.iter().any(|m| m.spec.edge_type == "PROVIDES"),
            "Component -> Interface leaves Component"
        );
    }

    #[test]
    fn required_properties_come_first() {
        let g = graph();
        let d = g.describe_node_type("Requirement").unwrap();
        let first_optional = d.spec.properties.iter().position(|p| !p.required);
        let last_required = d.spec.properties.iter().rposition(|p| p.required);
        if let (Some(o), Some(r)) = (first_optional, last_required) {
            assert!(r < o, "every required property precedes every optional one");
        }
        assert!(
            d.spec
                .properties
                .iter()
                .any(|p| p.name == "statement" && p.required),
            "Requirement.statement is required"
        );
    }
}
