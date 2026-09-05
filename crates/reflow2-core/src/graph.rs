//! [`DesignGraph`] — the reflow2 handle over a schema-configured graph store.
//!
//! Thin, deterministic, LLM-free (docs/interaction-surfaces.md, "deterministic
//! ops"). It wraps a dynograph-foundation [`StorageEngine`] already configured
//! with the full reflow2 [`Schema`], scopes every call to one logical graph id,
//! and exposes both generic schema-validated CRUD and typed convenience
//! constructors for the golden-thread node/edge types.
//!
//! Every write goes through the engine's `validate_node` / `validate_edge`, so a
//! bad node type, a missing required property, or an edge with the wrong
//! endpoints fails loud here (rule 4 in AGENTS.md: no silent fallbacks).

use crate::foundation::core::{DynoError, PropertyDef, PropertyType, Schema, Value};
use crate::foundation::store::{StorageEngine, StoredEdge, StoredNode};

use crate::nodes::{Props, edge, node};

/// The canonical content hash of a node's properties — keys sorted, compact
/// separators, so the same properties hash identically regardless of map order.
///
/// IT LIVES IN THE CORE SO THERE IS ONLY ONE OF IT. The MCP surface computed
/// this itself for the `revision` block's `prior_content_hash`, and the moment
/// `upsert_node_if_unchanged` started COMPARING against that value, two
/// independent implementations of one hash became a way for a caller's
/// expectation to be rejected by an engine that agreed with them. Two
/// implementations of one number is the shape of defect this project keeps
/// finding in other people's code; there is no reason to seed one here.
pub fn node_content_hash(props: &std::collections::HashMap<String, Value>) -> String {
    use sha2::{Digest, Sha256};
    let ordered: std::collections::BTreeMap<&String, &Value> = props.iter().collect();
    let text = serde_json::to_string(&ordered).unwrap_or_default();
    format!("sha256:{:x}", Sha256::digest(text.as_bytes()))
}

/// Widen integer literals to floats for properties the schema declares
/// `float`, in place. JSON has one number type, so every client writes
/// `confidence: 1` — and the store validates [`Value`] variants strictly, so
/// the bare integer was refused ("expected Float, got int", BL-50). The
/// coercion lives here, at the schema-aware seam every write passes through,
/// and only when exact: an integer a `f64` cannot represent is left alone to
/// fail loud, and a property the schema does not declare float is never
/// touched.
fn widen_ints_for_float_props(
    defs: &std::collections::HashMap<String, PropertyDef>,
    props: &mut std::collections::HashMap<String, Value>,
) {
    for (name, value) in props.iter_mut() {
        if let Value::Int(i) = value
            && defs
                .get(name)
                .is_some_and(|d| d.prop_type == PropertyType::Float)
            // Only widen when `f64` represents the integer EXACTLY. The
            // `(i as f64) as i64` round-trip is not a sufficient check: near
            // i64::MAX the float rounds up to 2^63 and the float→int cast
            // saturates back to i64::MAX, so a lossy value passed the test
            // (BL-58). Bound by 2^53, the largest integer f64 holds losslessly.
            && i.unsigned_abs() <= (1u64 << 53)
        {
            *value = Value::Float(*i as f64);
        }
    }
}

/// Default logical graph id inside the storage instance. One design lives in
/// one graph; the id is just a stable name to scope keys.
pub const DEFAULT_GRAPH_ID: &str = "reflow2";

/// A design graph: a [`StorageEngine`] scoped to a single graph id.
pub struct DesignGraph {
    engine: StorageEngine,
    /// `pub(crate)` because each coherence-loop step is its own module of
    /// `impl DesignGraph`, and `import_graph` has to be able to ADOPT a
    /// restored design's name rather than write it under the receiver's
    /// (BL-169). Read it through [`graph_id`](Self::graph_id) from outside.
    pub(crate) graph_id: String,
    /// Where the store lives on disk, when it lives on disk at all.
    ///
    /// Carried so the graph can reach its own identity sidecar
    /// (`<path>.id.json`), which is what lets [`import_graph`](Self::import_graph)
    /// adopt a restored design's name instead of silently renaming it (BL-169).
    /// `None` for an in-memory graph, which has no sidecar and nothing to adopt.
    pub(crate) store_path: Option<String>,
    /// Derived results that every orientation rollup used to recompute —
    /// `open_defects` (~5 s) and `detect_gaps` (~1.5 s) — remembered per write
    /// generation. Measured 2026-09-05: loop_status, graph_report and
    /// debt_since each re-ran the defect scan to print a COUNT, and the read
    /// path ran it again after every write; one orientation pass paid the same
    /// five seconds three times. A memo keyed on `engine.write_generation()`
    /// pays it once per graph state (`dec:derived-scans-are-memoised-per-write-
    /// generation`). Interior mutability because the scans take `&self`.
    pub(crate) derived: std::sync::Mutex<DerivedMemo>,
}

/// What [`DesignGraph::derived`] holds. `generation` is the engine write
/// generation the entries were computed at; a mismatch discards them all.
/// `recomputes` counts actual scans so a test can assert STRUCTURE — once per
/// write, not N times per rollup — rather than a duration that measures load.
#[derive(Debug, Default)]
pub struct DerivedMemo {
    pub generation: Option<u64>,
    pub defects: Option<(Vec<crate::heal::HealIssue>, crate::heal::Suppressed)>,
    pub gaps: Option<Vec<crate::detect::GapCandidate>>,
    pub recomputes: u64,
}

/// Memo access lives in its own ungated impl: the rollups that use it compile
/// with and without the `rocksdb` feature, so it must too.
impl DesignGraph {
    /// Lock the derived memo, discarding its entries if the store has been
    /// written since they were computed. Callers then hit or fill ONE entry.
    pub(crate) fn derived_at_current_generation(&self) -> std::sync::MutexGuard<'_, DerivedMemo> {
        let now = self.engine.write_generation();
        let mut memo = self.derived.lock().expect("derived memo poisoned");
        if memo.generation != Some(now) {
            memo.generation = Some(now);
            memo.defects = None;
            memo.gaps = None;
        }
        memo
    }

    /// How many times a memoised derived scan has actually RUN in this
    /// graph's lifetime. The structural assertion behind the memo: across N
    /// rollups with no write in between this moves by one, not N.
    pub fn derived_recomputes(&self) -> u64 {
        self.derived
            .lock()
            .expect("derived memo poisoned")
            .recomputes
    }
}

impl DesignGraph {
    /// Open an in-memory design graph configured with the full reflow2 schema.
    ///
    /// The in-memory backend needs no cargo feature and no disk — ideal for
    /// tests and dev iteration. Fails only if the embedded schema fails to
    /// merge/validate (a build-time-embedded bug, surfaced at open).
    pub fn open_in_memory() -> Result<Self, DynoError> {
        Self::open_in_memory_as(DEFAULT_GRAPH_ID)
    }

    /// Open an in-memory graph that knows its own name.
    ///
    /// Every node and edge carries a `graph_id`, and until federation it was
    /// pinned to one constant — so every reflow2 design claimed to BE the same
    /// design. That is harmless while designs never meet and load-bearing the
    /// moment they do: `mirror_surface` has to tell "somebody else's design" from
    /// "mine", and it can only do that if the two have different names
    /// (`dec:nested-graphs` option (c); `rule:no-foreclosure` item 5, which said
    /// the foreclosure is code that ASSUMES the constant rather than reads the
    /// field).
    ///
    /// **In-memory only, deliberately.** The id namespaces every stored key, so
    /// an on-disk graph reopened under a different name would find nothing —
    /// which means a durable design's identity has to be remembered beside the
    /// store, not passed on each open. That is real work and it is named as a gap
    /// rather than half-done here.
    pub fn open_in_memory_as(graph_id: &str) -> Result<Self, DynoError> {
        let schema = crate::schema::load_schema()?;
        Ok(Self {
            engine: StorageEngine::new_in_memory(schema),
            graph_id: graph_id.to_string(),
            store_path: None,
            derived: Default::default(),
        })
    }

    /// Open an on-disk design graph backed by RocksDB at `path`, configured
    /// with the full reflow2 schema. This is the persistent surface backend:
    /// the design survives across agent sessions (surface-plan.md, step 1),
    /// where the in-memory backend is dev/test only.
    ///
    /// Delegates to the foundation's [`StorageEngine::new_rocksdb`], which is
    /// present in the API regardless of the `rocksdb` feature: with the feature
    /// off it returns a fail-loud error (no silent fallback to memory — AGENTS.md
    /// rule 4), and the C++ `librocksdb-sys` compile stays opt-in. Also fails if
    /// the embedded schema fails to merge or the store cannot be opened.
    pub fn open_rocksdb(path: &str) -> Result<Self, DynoError> {
        Ok(Self::open_rocksdb_with_provenance(path)?.0)
    }

    /// Open on disk, and report which reflow2 wrote the graph.
    ///
    /// The stamp lives beside the store and is refreshed on the way through.
    /// Only one difference is fatal — a graph written by a reflow2 that knew
    /// *more* of the schema than this one, which cannot be read in full. See
    /// [`crate::provenance`] for why every other difference opens.
    pub fn open_rocksdb_with_provenance(
        path: &str,
    ) -> Result<(Self, crate::provenance::Provenance), DynoError> {
        let schema = crate::schema::load_schema()?;
        // Ask BEFORE opening whether a store has ever held data here: opening
        // creates the directory, and after that "a design was here" and "nothing
        // was ever here" look identical. It is the only evidence that survives a
        // lost identity sidecar, and it is gone one line later.
        let store_had_content = crate::identity::store_has_content(path);
        // Open the store FIRST. A build without the `rocksdb` feature fails loud
        // here (naming the feature — AGENTS.md rule 4), and it must do so BEFORE
        // the provenance stamp touches disk: check_and_stamp writes
        // `<path>.meta.json` for any non-refused path, so stamping before a failed
        // open leaves a stray stamp that poisons the next open across a schema
        // change (a stale higher-count stamp then reads as "knows more of the
        // schema than you"). Opening is content-agnostic — no design data is
        // interpreted — so the "knows more" refusal check_and_stamp raises next is
        // unchanged for a real on-disk graph.
        let engine = StorageEngine::new_rocksdb(schema.clone(), path)?;
        let provenance = crate::provenance::check_and_stamp(path, &schema)?;
        // Who is this design? (req:design-identity.) Established on first open
        // and read on every one after, from a sibling file — the id namespaces
        // every stored key, so it has to be known before the design can be.
        //
        // The closure is the migration, and it only runs when there is no
        // identity file yet: a store that ALREADY holds a design under the old
        // shared id keeps that id. Minting one instead would leave the design on
        // disk and open a new empty one beside it, reporting nothing wrong.
        //
        // `resolve_on_open` rather than `resolve`: the probe below can only see a
        // design under the OLD SHARED id, so on a store whose sidecar has been
        // parted from it — the mounted-volume case `cap:hosted-state-on-a-volume`
        // is about — it answers "no design here" for a design that is very much
        // here, and the mint that follows overwrites the only record of its name.
        let identity =
            crate::identity::resolve_on_open(path, DEFAULT_GRAPH_ID, store_had_content, || {
                Self::holds_a_design_probe(&engine, DEFAULT_GRAPH_ID)
            })?;
        Ok((
            Self {
                engine,
                graph_id: identity.graph_id,
                store_path: Some(path.to_string()),
                derived: Default::default(),
            },
            provenance,
        ))
    }

    /// Does this store already hold a design under `graph_id`?
    ///
    /// Asked once, on the open that establishes identity, and deliberately
    /// cheap: a handful of the types every real design has. A false negative
    /// would mint a new id for an existing design and hide it, so the list
    /// errs wide rather than narrow — anything that has ever been captured
    /// puts at least one of these in the store.
    fn holds_a_design_probe(engine: &StorageEngine, graph_id: &str) -> bool {
        [
            node::PROJECT,
            node::REQUIREMENT,
            node::CAPABILITY,
            node::COMPONENT,
            node::DECISION,
            node::ARTIFACT,
            node::VERIFICATION,
            node::FRAGMENT,
        ]
        .iter()
        .any(|t| engine.count_nodes(graph_id, t).unwrap_or(0) > 0)
    }

    /// Does this graph hold a design at all? The public form of the probe that
    /// decides whether an id is minted or adopted — asked again on import,
    /// where an empty store takes the name of the design it restores.
    pub fn holds_a_design(&self) -> bool {
        Self::holds_a_design_probe(&self.engine, &self.graph_id)
    }

    /// Use a non-default logical graph id (e.g. to host several designs in one
    /// storage instance). Chainable off a constructor.
    #[must_use]
    pub fn with_graph_id(mut self, id: impl Into<String>) -> Self {
        self.graph_id = id.into();
        self
    }

    /// The graph id every operation is scoped to.
    pub fn graph_id(&self) -> &str {
        &self.graph_id
    }

    /// Crate-internal access to the storage engine, for modules that wrap an
    /// engine capability not already surfaced as a `DesignGraph` method
    /// (currently only `search`). Feature-gated with its one user so the
    /// default build stays warning-free.
    #[cfg(feature = "fulltext")]
    pub(crate) fn engine(&self) -> &StorageEngine {
        &self.engine
    }

    /// The merged schema backing this graph.
    pub fn schema(&self) -> &Schema {
        self.engine.schema()
    }

    // ---- Generic, schema-validated CRUD -----------------------------------

    /// Create (or replace) a node of `node_type` with `id` and `props`.
    /// Which of `props` the schema does not declare for `node_type`, sorted.
    ///
    /// ⚠️ THIS REPORTS; IT NEVER REFUSES, and the distinction is the whole
    /// design. The store is a property BAG on purpose — open-world properties
    /// are what let a project record something reflow2 never anticipated — so
    /// rejecting unknown keys would break a capability rather than fix a bug.
    /// What was missing is that a caller could not tell an EXTENSION from a
    /// TYPO, because the reply was identical either way.
    ///
    /// MEASURED 2026-08-16: `enforcement: "advisory"` was written to a
    /// DesignRule, accepted, stored and echoed back. The schema declares no
    /// such property — the real field is `enforced`, a bool — so the write
    /// succeeded and meant nothing, and only an unrelated gap firing exposed
    /// it. THE ASYMMETRY IS WHY THIS EXISTS: an unknown TOOL ARGUMENT is
    /// refused in 134 places across the served surface, and an EDGE to a
    /// missing node is refused through sixteen typed helpers — while the props
    /// bag, which is exactly where a property name lands, said nothing at all.
    ///
    /// An unknown `node_type` yields an empty list rather than every key:
    /// that case is already refused loudly by the write itself, and answering
    /// "all of them are undeclared" would bury the real error under noise.
    pub fn undeclared_properties(
        &self,
        node_type: &str,
        props: &std::collections::HashMap<String, Value>,
    ) -> Vec<String> {
        let Some(def) = self.schema().node_types.get(node_type) else {
            return Vec::new();
        };
        let mut out: Vec<String> = props
            .keys()
            .filter(|k| !def.properties.contains_key(*k))
            .cloned()
            .collect();
        out.sort();
        out
    }

    /// Validates against the schema; unknown type or missing required property
    /// is an error, not a silent skip.
    pub fn create_node(
        &mut self,
        node_type: &str,
        id: &str,
        props: impl Into<std::collections::HashMap<String, Value>>,
    ) -> Result<StoredNode, DynoError> {
        let mut props = props.into();
        if let Some(def) = self.schema().node_types.get(node_type) {
            widen_ints_for_float_props(&def.properties, &mut props);
        }
        self.refuse_dangling_node_refs(node_type, &props)?;
        self.engine
            .create_node(&self.graph_id, node_type, id, props)
    }

    /// Refuse a write whose property NAMES a node that does not exist.
    ///
    /// The sibling of `create_edge`'s endpoint guard, and it exists because the
    /// same failure was reachable through the other shape of reference:
    /// `fact:defect-a-property-naming-a-node-is-unguarded-while-edges-are-not`
    /// measured a TemporalFact written with `subject_id` naming a capability
    /// that had never existed. It was stored, echoed back without complaint,
    /// and caught only because somebody read the write back by habit.
    ///
    /// REFUSED AT WRITE rather than reported later, for the reason `create_edge`
    /// already gives: a dangling reference is not a judgement the user might
    /// reasonably disagree with, so `dec:report-dont-judge` does not apply — it
    /// is a graph that cannot be walked.
    ///
    /// ⚠️ A BARE ID CARRIES NO TYPE, which is the one way this differs from the
    /// edge guard. An edge declares its endpoint types; `subject_id` holds only
    /// `"cap:foo"`, and the store has no type-free lookup. So the id is resolved
    /// by asking each declared node type in turn — the same walk
    /// `count_all_nodes` makes. That is a handful of point lookups on a write
    /// path, paid only by nodes that actually declare a reference.
    ///
    /// 🛑 WHAT THIS CANNOT REACH, measured on this design's own graph: of nine
    /// dangling values, roughly a third are typos this refuses. The rest were
    /// VALID WHEN WRITTEN and dangled later when the target was renamed. No
    /// write-time check can catch those; they need detection, which is a
    /// separate cause with a separate fix.
    fn refuse_dangling_node_refs(
        &self,
        node_type: &str,
        props: &std::collections::HashMap<String, Value>,
    ) -> Result<(), DynoError> {
        let refs = self.declared_node_refs(node_type, props);
        if refs.is_empty() {
            return Ok(());
        }
        let types: Vec<String> = self.schema().node_types.keys().cloned().collect();
        for (prop, target) in refs {
            let mut found = false;
            for t in &types {
                if self.get_node(t, &target)?.is_some() {
                    found = true;
                    break;
                }
            }
            if !found {
                return Err(Self::dangling_node_ref_error(node_type, &prop, &target));
            }
        }
        Ok(())
    }

    /// The node-reference properties `node_type` declares, paired with the
    /// value this node carries for each.
    ///
    /// ⭐ THE ONLY READER OF `node_ref` IN THE CRATE, and that is the point.
    /// Two paths have to agree about what counts as a reference — the write-time
    /// guard above, and the pass `import_graph` runs once a whole document is
    /// staged — and a second copy of "which properties are references, and how
    /// is one read" is exactly the hand-maintained duplicate that drifts.
    ///
    /// An absent or empty value is not returned: an unset optional reference is
    /// not a dangling one.
    pub(crate) fn declared_node_refs(
        &self,
        node_type: &str,
        props: &std::collections::HashMap<String, Value>,
    ) -> Vec<(String, String)> {
        let Some(def) = self.schema().node_types.get(node_type) else {
            return Vec::new();
        };
        let mut out: Vec<(String, String)> = def
            .properties
            .iter()
            .filter(|(_, d)| d.node_ref)
            .filter_map(|(name, _)| match props.get(name) {
                Some(Value::String(target)) if !target.is_empty() => {
                    Some((name.clone(), target.clone()))
                }
                _ => None,
            })
            .collect();
        out.sort();
        out
    }

    /// The refusal a dangling node reference gets, worded in one place so the
    /// write path and the import path cannot come to say it differently.
    pub(crate) fn dangling_node_ref_error(node_type: &str, prop: &str, target: &str) -> DynoError {
        DynoError::Validation {
            node_type: node_type.to_string(),
            property: prop.to_string(),
            message: format!(
                "'{target}' names no node in this design. This property is declared \
                 a node reference, so a value that resolves to nothing would be a \
                 record about something the design does not have — the same refusal \
                 an edge to a missing node already gets. Check the id, or create the \
                 node first."
            ),
        }
    }

    /// [`create_node`](Self::create_node) with the node-reference guard NOT run
    /// here — reserved for [`import_graph`](Self::import_graph), which runs the
    /// equivalent check itself once every node in the document is staged.
    ///
    /// 🛑 THE GUARD IS NOT WEAKENED, IT IS MOVED — and it had to be, because the
    /// write-time guard resolves a reference against the store AS IT STANDS,
    /// which during an import is "however far down the document the walk has
    /// got". An export is ordered by node type, never by dependency, so a
    /// TemporalFact whose `subject_id` names a Verification is refused for no
    /// reason but the alphabet.
    ///
    /// MEASURED, 2026-08-29, on this design's own export: importing it with the
    /// guard on the walk failed with 23 faults and wrote nothing. **Only nine
    /// were real** — five instances of the `prj:reflow2` typo for `proj:reflow2`
    /// and four capabilities renamed after the fact was written. Three were
    /// nodes PRESENT IN THE SAME DOCUMENT at indices 3342, 3360 and 3169,
    /// referenced from 2854, 3167 and 3063; one more was a node refused above
    /// cascading onto its referrer. A coherent design with no dangling
    /// references at all would still have failed this way.
    ///
    /// ⭐ THE UNIT OF VALIDATION MUST MATCH THE UNIT OF ATOMICITY. The import is
    /// all-or-nothing (`dec:bulk-is-all-or-nothing-with-per-item-findings`), so
    /// the document is what becomes true or does not — and a reference has to be
    /// judged against that, not against a partial state no caller ever observes.
    /// This is the same shape the edge guard already gets for free by running
    /// after every node is written.
    pub(crate) fn create_node_refs_checked_later(
        &mut self,
        node_type: &str,
        id: &str,
        props: impl Into<std::collections::HashMap<String, Value>>,
    ) -> Result<StoredNode, DynoError> {
        let mut props = props.into();
        if let Some(def) = self.schema().node_types.get(node_type) {
            widen_ints_for_float_props(&def.properties, &mut props);
        }
        self.engine
            .create_node(&self.graph_id, node_type, id, props)
    }

    /// [`create_node`](Self::create_node), REFUSED when the node has moved since
    /// the caller read it — a compare-and-swap, and the prevention half of
    /// `req:a-write-cannot-silently-lose-someone-elses-work`.
    ///
    /// # The failure this exists to stop
    ///
    /// Measured from BOTH SIDES of one collision on a shared graph: a worker
    /// read a node, ninety seconds later another attached session wrote it, and
    /// the write returned a normal success with the full node body and nothing
    /// unusual. **THE WINNER WAS NEVER TOLD.** The loser found out only because
    /// `record_change` happens to return the snapshot it took — a diagnostic
    /// side-effect of an unrelated tool, not a guard. On any other write path
    /// both would have believed they made the change and one would have been
    /// wrong.
    ///
    /// # Why this rather than more reporting
    ///
    /// The `revision` block already REPORTS what a write replaced, and that is
    /// detection: it tells the loser afterwards and tells the winner nothing.
    /// `rule:fix-it-properly-while-it-is-still-cheap` is why this is a refusal
    /// instead of a fifth report — the requirement's own words are that the
    /// revision block's hash "is exactly the raw material a compare-and-swap
    /// needs; nothing consumes it yet". This consumes it.
    ///
    /// # What it does NOT do
    ///
    /// It does not lock, it does not retry, and it does not merge for you. A
    /// refusal means *your copy is stale* — re-read, decide what to keep, and
    /// write again. Deciding that is a judgement (`dec:ask-not-repair`), and an
    /// automatic merge here would be the silent overwrite wearing a seatbelt.
    ///
    /// It is also OPT-IN, and that bound is worth stating plainly: a caller who
    /// passes no expectation gets the old behaviour. Making it mandatory would
    /// break every existing writer, and — more to the point — a caller who
    /// never read the node has no honest expectation to state.
    /// It guards the UPSERT rather than the raw create, because the upsert is
    /// the merge path every surface actually writes through — a guard on a path
    /// nobody calls is a guard that reports success and defends nothing, which
    /// is the failure this whole increment is about.
    pub fn upsert_node_if_unchanged(
        &mut self,
        node_type: &str,
        id: &str,
        props: impl Into<std::collections::HashMap<String, Value>>,
        expected_content_hash: &str,
    ) -> Result<StoredNode, DynoError> {
        self.refuse_if_moved(node_type, id, expected_content_hash)?;
        self.upsert_node(node_type, id, props)
    }

    /// The precondition on its own: `Ok(())` when the node still holds what the
    /// caller read, an error naming both hashes when it does not.
    fn refuse_if_moved(
        &self,
        node_type: &str,
        id: &str,
        expected_content_hash: &str,
    ) -> Result<(), DynoError> {
        let found = self.get_node(node_type, id)?;
        let actual = match &found {
            Some(node) => node_content_hash(&node.properties),
            // ABSENT IS A MISMATCH, NOT A CREATE. A caller stating an
            // expectation has read something; if it is gone, somebody deleted
            // it while they were deciding, and silently creating it back would
            // resurrect a node whose removal was somebody's decision.
            None => {
                return Err(DynoError::Validation {
                    node_type: node_type.to_string(),
                    property: "expected_content_hash".into(),
                    message: format!(
                        "refusing the write: '{id}' does not exist, and you passed an \
                         expected content hash, which means you read it and it has since \
                         been DELETED. Creating it again would undo somebody's removal. \
                         Re-read, decide whether it should come back, and write without an \
                         expectation if it should."
                    ),
                });
            }
        };
        if actual != expected_content_hash {
            return Err(DynoError::Validation {
                node_type: node_type.to_string(),
                property: "expected_content_hash".into(),
                message: format!(
                    "refusing the write: '{id}' has changed since you read it. You expected \
                     {expected_content_hash} and it now holds {actual}. Somebody else's work \
                     is in there. Re-read the node, merge your change on top of what it says \
                     NOW, and write again — this refusal is the only thing standing between \
                     that work and a silent overwrite."
                ),
            });
        }
        Ok(())
    }

    /// Merge `props` onto `id` if it exists, or create it. The supplied
    /// properties overwrite; every stored property the caller does not name
    /// survives.
    ///
    /// This is the update half of generic CRUD — the contract the
    /// revise-design skill states. [`create_node`](Self::create_node) alone is
    /// create-or-*replace*, and replacing re-materializes schema defaults over
    /// everything omitted, which is how a partial "edit one property" call
    /// once silently reset a verified capability to `planned` (BL-46, the
    /// 2026-07-20 self-adopt session). The typed setters remain the right
    /// call when one exists: they refuse a missing node instead of creating it.
    pub fn upsert_node(
        &mut self,
        node_type: &str,
        id: &str,
        props: impl Into<std::collections::HashMap<String, Value>>,
    ) -> Result<StoredNode, DynoError> {
        let supplied = props.into();
        match self.get_node(node_type, id)? {
            Some(existing) => {
                let mut merged = existing.properties;
                merged.extend(supplied);
                self.create_node(node_type, id, merged)
            }
            None => self.create_node(node_type, id, supplied),
        }
    }

    /// Fetch a node by type and id. `Ok(None)` when it does not exist.
    pub fn get_node(&self, node_type: &str, id: &str) -> Result<Option<StoredNode>, DynoError> {
        self.engine.get_node(&self.graph_id, node_type, id)
    }

    /// Count nodes of a type.
    pub fn count_nodes(&self, node_type: &str) -> Result<usize, DynoError> {
        self.engine.count_nodes(&self.graph_id, node_type)
    }

    /// How many nodes this graph holds, across every schema type.
    ///
    /// One counted scan per node type and no adjacency walk, so this is cheap
    /// enough for an orientation call — unlike an edge total, which would have
    /// to visit every node's outgoing set and therefore cost what
    /// `export_graph` costs.
    pub fn count_all_nodes(&self) -> Result<usize, DynoError> {
        let types: Vec<String> = self.schema().node_types.keys().cloned().collect();
        let mut total = 0;
        for t in types {
            total += self.count_nodes(&t)?;
        }
        Ok(total)
    }

    /// Create an edge of `edge_type` between typed endpoints. Endpoint types
    /// are validated against the edge's declared `from`/`to`, and both endpoint
    /// nodes must already exist — a dangling edge is refused before anything is
    /// written, never stored and reported later.
    pub fn create_edge(
        &mut self,
        edge_type: &str,
        from_type: &str,
        from_id: &str,
        to_type: &str,
        to_id: &str,
        props: impl Into<std::collections::HashMap<String, Value>>,
    ) -> Result<StoredEdge, DynoError> {
        // BOTH ENDPOINTS MUST EXIST. The schema validates the endpoint TYPES
        // against the edge's declared `from`/`to`, which is a different question
        // from whether the nodes are there — so until 2026-07-28 a `DEPENDS_ON`
        // pointing at a capability that had never been created was accepted,
        // stored, and reported by NOTHING: `detect_defects` returned zero,
        // `detect_gaps` returned only unrelated phase gaps, and the target
        // resolved to `None`. One mistyped id produced a dependency that read as
        // fine and pointed into nothing, and every rollup that walks the golden
        // thread walked off the end of it.
        //
        // Found by comparing reflow2 against the original reflow's
        // `system_of_systems_graph_v2.py`, whose functional-mode gap detector
        // has an `unmet_dependencies` bucket for exactly this. reflow2 had
        // neither the prevention nor the detection.
        //
        // REFUSED AT WRITE rather than reported later, which is the choice
        // `snapshot_node` and `add_change_event` already made ("every entry must
        // name an existing node — the whole call is refused before anything is
        // written"). A dangling edge is not a judgement call the user might
        // reasonably disagree with, so `dec:report-dont-judge` does not apply:
        // it is a graph that cannot be walked. Refusing keeps the store's
        // invariant true for every reader, instead of asking each one to
        // tolerate an endpoint that is not there.
        // ORDERING: only for an edge type the schema knows. An unrecognised
        // edge type is the more fundamental error and carries the better
        // rejection — `edge_error` names the types that DO accept this pair and
        // points at `describe_schema`, which is the fix for "the error tells me
        // I'm wrong without telling me what's right" (the blind trial's
        // complaint, after fourteen guesses at joining a Release to a
        // Component). Checking endpoints first buried that: a caller guessing
        // `PACKAGES` between two ids that don't exist yet was told only that
        // `rel:1` was missing, never that `PACKAGES` is not an edge type at
        // all — so they would create the nodes and guess wrong again.
        // Vocabulary errors before instance errors.
        if self.schema().edge_types.contains_key(edge_type) {
            for (kind, id) in [(from_type, from_id), (to_type, to_id)] {
                if self.get_node(kind, id)?.is_none() {
                    return Err(DynoError::NodeNotFound {
                        node_type: kind.to_string(),
                        node_id: id.to_string(),
                    });
                }
            }
        }

        let mut props = props.into();
        if let Some(def) = self.schema().edge_types.get(edge_type) {
            widen_ints_for_float_props(&def.properties, &mut props);
        }
        self.engine.create_edge(
            &self.graph_id,
            edge_type,
            from_type,
            from_id,
            to_type,
            to_id,
            props,
        )
    }

    /// Outgoing edges from `from_id`, optionally filtered to one edge type.
    /// This is the primitive the golden-thread walk (PROPAGATE) builds on.
    pub fn outgoing(
        &self,
        from_id: &str,
        edge_type: Option<&str>,
    ) -> Result<Vec<StoredEdge>, DynoError> {
        self.engine
            .scan_outgoing_edges(&self.graph_id, from_id, edge_type)
    }

    /// Incoming edges to `to_id`, optionally filtered to one edge type. The
    /// reverse-direction companion to [`outgoing`](Self::outgoing) — PROPAGATE
    /// needs both, because impact flows along an edge in whichever direction the
    /// edge's semantics carry it (e.g. a Requirement's realizers are reached via
    /// *incoming* SATISFIES).
    pub fn incoming(
        &self,
        to_id: &str,
        edge_type: Option<&str>,
    ) -> Result<Vec<StoredEdge>, DynoError> {
        self.engine
            .scan_incoming_edges(&self.graph_id, to_id, edge_type)
    }

    /// All nodes of a type. Used by PROPAGATE to build an id→type index (edge
    /// adjacency stores only endpoint ids, not their types).
    pub fn scan_nodes(&self, node_type: &str) -> Result<Vec<StoredNode>, DynoError> {
        self.engine.scan_nodes(&self.graph_id, node_type)
    }

    /// Build an id→type index over the whole project subgraph. Edge adjacency
    /// carries only endpoint ids; this resolves a node's type (and confirms it
    /// exists — e.g. dangling edges to absent nodes are excluded from a blast
    /// radius). Shared plumbing for `propagate`, `structure`, `heal`, `export`.
    ///
    /// Assumes node ids are unique across types within a graph (reflow2's typed-
    /// prefix id convention, e.g. `req:`, `cap:`); on a collision the first
    /// type scanned wins.
    pub(crate) fn node_type_index(
        &self,
    ) -> Result<std::collections::HashMap<String, String>, DynoError> {
        let mut index = std::collections::HashMap::new();
        // Sorted so that on an id collision across types (a convention
        // violation, but writable) "first type scanned wins" is deterministic
        // across processes rather than following HashMap iteration order
        // (BL-58). The schema's `node_types` is a HashMap with per-process key
        // order.
        let mut types: Vec<String> = self.schema().node_types.keys().cloned().collect();
        types.sort_unstable();
        for node_type in types {
            for node in self.scan_nodes(&node_type)? {
                index
                    .entry(node.node_id)
                    .or_insert_with(|| node_type.clone());
            }
        }
        Ok(index)
    }

    /// Delete a node and every edge attached to it. Returns whether it existed.
    ///
    /// This takes every attached edge with it, which is a second door onto the
    /// same loss `delete_edge` guards — retiring a scheduled requirement is
    /// precisely how something gets discontinued, and it would otherwise walk
    /// straight past the refusal. Both directions are checked: the item being
    /// deleted carries commitments away with it, and deleting the moment itself
    /// destroys the whole plan pointed at it.
    pub fn delete_node(&mut self, node_type: &str, id: &str) -> Result<bool, DynoError> {
        for e in self.outgoing(id, Some(edge::SCHEDULED_FOR))? {
            self.guard_schedule_loss(
                &e.to_id,
                &format!("deleting '{id}', which is scheduled for it"),
            )?;
        }
        if !self.incoming(id, Some(edge::SCHEDULED_FOR))?.is_empty() {
            self.guard_schedule_loss(id, &format!("deleting '{id}' itself"))?;
        }
        self.engine.delete_node(&self.graph_id, node_type, id)
    }

    /// Delete a single edge. Returns whether it existed.
    ///
    /// Removing a `SCHEDULED_FOR` is how a plan slips or is dropped, so it is
    /// refused while the plan it belongs to is unrecorded — see
    /// [`guard_schedule_loss`](Self::guard_schedule_loss).
    pub fn delete_edge(
        &mut self,
        edge_type: &str,
        from_id: &str,
        to_id: &str,
    ) -> Result<bool, DynoError> {
        if edge_type == edge::SCHEDULED_FOR {
            self.guard_schedule_loss(to_id, &format!("un-scheduling '{from_id}' from it"))?;
        }
        self.engine
            .delete_edge(&self.graph_id, edge_type, from_id, to_id)
    }

    // ---- Atomic batches (used by HEAL's apply step) -----------------------

    /// Begin buffering writes; nothing hits the store until [`commit_batch`].
    ///
    /// [`commit_batch`]: Self::commit_batch
    pub(crate) fn begin_batch(&mut self) {
        self.engine.begin_batch();
    }

    /// Flush all buffered writes atomically.
    pub(crate) fn commit_batch(&mut self) -> Result<usize, DynoError> {
        self.engine.commit_batch()
    }

    /// Drop all buffered writes without applying them.
    pub(crate) fn discard_batch(&mut self) {
        self.engine.discard_batch();
    }

    // ---- Typed golden-thread constructors ---------------------------------
    //
    // Convenience over `create_node` for the four spine node types, supplying
    // only their required properties. Richer properties can still go through
    // `create_node` with a full `Props`.

    /// P0 · Intent — the top-level thing being designed. `name` is required.
    pub fn add_project(&mut self, id: &str, name: &str) -> Result<StoredNode, DynoError> {
        self.upsert_node(node::PROJECT, id, Props::new().set("name", name))
    }

    /// P0 · Intent — a stated need. `name` and `statement` are required.
    pub fn add_requirement(
        &mut self,
        id: &str,
        name: &str,
        statement: &str,
    ) -> Result<StoredNode, DynoError> {
        self.upsert_node(
            node::REQUIREMENT,
            id,
            Props::new().set("name", name).set("statement", statement),
        )
    }

    /// Record a [`Contributor`](node::CONTRIBUTOR) — who authors and decides the
    /// design (a person, an automated agent, or an organization). Distinct from
    /// an Actor, who the designed system serves. The seed of the identity thread:
    /// design nodes then point at it with [`authored_by`](Self::authored_by).
    pub fn add_contributor(
        &mut self,
        id: &str,
        name: &str,
        kind: Option<&str>,
        handle: Option<&str>,
        description: Option<&str>,
    ) -> Result<StoredNode, DynoError> {
        self.upsert_node(
            node::CONTRIBUTOR,
            id,
            Props::new()
                .set("name", name)
                .set_opt("kind", kind)
                .set_opt("handle", handle)
                .set_opt("description", description),
        )
    }

    /// P1 · Function — something the design can do. `name` and `description`
    /// are required.
    ///
    /// `status` ∈ `planned` (the default) / `in_progress` / `realized` /
    /// `verified`. Optional at creation, and optional for a reason: on the
    /// greenfield path a capability genuinely starts planned, so the default is
    /// right and the caller should not have to say so.
    ///
    /// It is settable *at creation* because the brownfield path cannot use the
    /// default at all. Adopting a system that already exists means recording
    /// capabilities that already ship, and a graph that calls them all `planned`
    /// asserts that a production system is entirely unbuilt — ophyd's 15 shipped,
    /// under-test capabilities landed exactly that way. Correcting them
    /// afterwards through [`set_capability_status`](Self::set_capability_status)
    /// is two writes per node with no bulk tool, which is what an adoption pass
    /// does least well.
    pub fn add_capability(
        &mut self,
        id: &str,
        name: &str,
        description: &str,
        status: Option<&str>,
    ) -> Result<StoredNode, DynoError> {
        self.upsert_node(
            node::CAPABILITY,
            id,
            Props::new()
                .set("name", name)
                .set("description", description)
                .set_opt("status", status),
        )
    }

    /// Set a `Capability`'s lifecycle status, preserving its other properties.
    /// `status` ∈ `planned` (the default) / `in_progress` / `realized` /
    /// `verified`.
    ///
    /// The sibling of [`set_requirement_status`](Self::set_requirement_status)
    /// and [`set_verification_status`](crate::DesignGraph::set_verification_status),
    /// and it exists for the same reason: a capability's standing changes far
    /// more often than its description, and re-stating the description to move
    /// it would invite drift between the two.
    pub fn set_capability_status(
        &mut self,
        capability_id: &str,
        status: &str,
    ) -> Result<StoredNode, DynoError> {
        let Some(existing) = self.get_node(node::CAPABILITY, capability_id)? else {
            return Err(DynoError::NodeNotFound {
                node_type: node::CAPABILITY.to_string(),
                node_id: capability_id.to_string(),
            });
        };
        let mut props = Props::new().set("status", status);
        for (k, v) in &existing.properties {
            if k != "status" {
                props = props.set(k, v.clone());
            }
        }
        self.create_node(node::CAPABILITY, capability_id, props)
    }

    /// Set a `Requirement`'s lifecycle status, preserving its other properties.
    /// `status` ∈ `proposed` (the default) / `accepted` / `deferred` /
    /// `dropped` / `met`.
    ///
    /// Kept separate from creation, like
    /// [`set_verification_status`](crate::DesignGraph::set_verification_status):
    /// a requirement's standing changes far more often than its wording, and
    /// re-stating the statement to move it would invite drift between the two.
    ///
    /// This is what a blind trial reached for and could not find — it wrote the
    /// word "ASSUMED" into the statement text instead, because status was in
    /// the schema but nothing on the surface could set it. DETECT already reads
    /// it: a `dropped` or `met` requirement stops raising
    /// `unsatisfied_requirement`.
    pub fn set_requirement_status(
        &mut self,
        requirement_id: &str,
        status: &str,
    ) -> Result<StoredNode, DynoError> {
        let Some(existing) = self.get_node(node::REQUIREMENT, requirement_id)? else {
            return Err(DynoError::NodeNotFound {
                node_type: node::REQUIREMENT.to_string(),
                node_id: requirement_id.to_string(),
            });
        };
        let mut props = Props::new().set("status", status);
        for (k, v) in &existing.properties {
            if k != "status" {
                props = props.set(k, v.clone());
            }
        }
        self.create_node(node::REQUIREMENT, requirement_id, props)
    }

    /// Choose a project's governance `mode`, preserving everything else about it.
    ///
    /// `flexible` (the schema default) lets `apply_heal` apply structural repairs;
    /// `rigid` makes it propose them and stop, so a human decides
    /// (`heal.rs::apply_heal`). That is the ONLY behaviour the mode currently
    /// changes, and the schema description says so rather than promising more.
    ///
    /// This exists because the mode was previously settable only at `genesis`,
    /// so every design ever made carried `flexible` by default and could never
    /// move off it — a governance choice asserted by a default that nobody made
    /// and nobody could revisit (`req:mode-is-chosen-and-changeable`). A default
    /// may record an absence; it must not decide how a project is governed.
    ///
    /// The value is validated by the schema on write, so an unknown mode fails
    /// loud rather than silently leaving the old one in place.
    pub fn set_project_mode(
        &mut self,
        project_id: &str,
        mode: &str,
    ) -> Result<StoredNode, DynoError> {
        let Some(existing) = self.get_node(node::PROJECT, project_id)? else {
            return Err(DynoError::NodeNotFound {
                node_type: node::PROJECT.to_string(),
                node_id: project_id.to_string(),
            });
        };
        let mut props = Props::new().set("mode", mode);
        for (k, v) in &existing.properties {
            if k != "mode" {
                props = props.set(k, v.clone());
            }
        }
        self.create_node(node::PROJECT, project_id, props)
    }

    /// Record how a node entered the graph, preserving its other properties.
    /// `provenance` ∈ `authored` (the default) / `planned` / `inferred` /
    /// `healed` / `reconciled` / `imported` — the same vocabulary as
    /// `Fragment.provenance`, deliberately, so there is one word for one idea.
    ///
    /// Accepted on `Requirement`, `Capability`, `Component` and `Interface`:
    /// the four types an adoption pass reads back out of a system that already
    /// exists. Any other type fails loud rather than silently doing nothing.
    ///
    /// `inferred` is the value that earns this property. A Requirement backed
    /// out of the code that implements it is satisfied by construction, so it
    /// can never contradict anything and a graph full of them says nothing —
    /// but only if you can *tell*. Ophyd had nowhere to put that fact and wrote
    /// `[EXTERNAL — …]` into the statement text, which is not queryable.
    ///
    /// For bulk adoption prefer [`import_graph`](Self::import_graph): it is the
    /// one bulk write path, it carries arbitrary properties including this one,
    /// and it applies them at create time rather than as a second write per node.
    pub fn set_provenance(
        &mut self,
        node_type: &str,
        node_id: &str,
        provenance: &str,
    ) -> Result<StoredNode, DynoError> {
        const ACCEPTS_PROVENANCE: [&str; 4] = [
            node::REQUIREMENT,
            node::CAPABILITY,
            node::COMPONENT,
            node::INTERFACE,
        ];
        if !ACCEPTS_PROVENANCE.contains(&node_type) {
            return Err(DynoError::Validation {
                node_type: node_type.to_string(),
                property: "provenance".to_string(),
                message: format!(
                    "no such property on `{node_type}`; it is declared on {}",
                    ACCEPTS_PROVENANCE.join(", ")
                ),
            });
        }
        let Some(existing) = self.get_node(node_type, node_id)? else {
            return Err(DynoError::NodeNotFound {
                node_type: node_type.to_string(),
                node_id: node_id.to_string(),
            });
        };
        let mut props = Props::new().set("provenance", provenance);
        for (k, v) in &existing.properties {
            if k != "provenance" {
                props = props.set(k, v.clone());
            }
        }
        self.create_node(node_type, node_id, props)
    }

    /// P2 · Structure — a buildable part. `name` and `purpose` are required.
    ///
    /// `level` is the axis-Y decomposition rank (matryoshka) — `component`,
    /// `subsystem`, `system`, `system_of_systems`, `enterprise` — and defaults
    /// to `component`. It is optional but load-bearing: [`hierarchy_issues`]
    /// compares the levels either side of a `CONTAINS` edge, so a design whose
    /// components all sit at the default has no hierarchy to check, and one
    /// that nests same-level components reports a `level_mismatch` for every
    /// edge. Set it whenever a part is genuinely an assembly.
    ///
    /// `kind` still takes its schema default (`module`).
    ///
    /// [`hierarchy_issues`]: crate::DesignGraph::hierarchy_issues
    pub fn add_component(
        &mut self,
        id: &str,
        name: &str,
        purpose: &str,
        level: Option<&str>,
    ) -> Result<StoredNode, DynoError> {
        self.upsert_node(
            node::COMPONENT,
            id,
            Props::new()
                .set("name", name)
                .set("purpose", purpose)
                .set_opt("level", level),
        )
    }

    /// P2 · Structure — a contract between parts. `name` is required; `medium`
    /// takes its schema default (`REST`).
    ///
    /// An Interface is the seam PROPAGATE crosses to reach the *other* side of
    /// a change: one Component [`provides`](Self::provides) it, others
    /// [`consume`](Self::consumes) it.
    pub fn add_interface(&mut self, id: &str, name: &str) -> Result<StoredNode, DynoError> {
        self.upsert_node(node::INTERFACE, id, Props::new().set("name", name))
    }

    /// Fill in what a consumer of this contract must agree with
    /// (`req:interface-spec-complete`).
    ///
    /// Separate from creation, like `set_verification_status`, because the
    /// contract usually gets named long before anyone knows its payload format —
    /// and a constructor demanding all of it would push people to guess.
    ///
    /// Every field is optional and omitting one LEAVES IT ALONE rather than
    /// clearing it, so this can be called repeatedly as a spec is filled in. The
    /// unset value is `unspecified` on the enums, never a plausible-looking
    /// default: silence about authentication must not read as "none".
    ///
    /// Rate limits, timeouts and concurrency are deliberately NOT here — they
    /// are numeric limits with a unit and a direction, which is what
    /// `Constraint` already carries and what `budget_report` already rolls up.
    /// `CONSTRAINS` accepts an Interface as its target today.
    /// Record what a Capability takes in and puts out — its functional
    /// signature, which is the black-box interface at the capability tier.
    ///
    /// # Why this is a setter and not three more parameters on `add_capability`
    ///
    /// `set_interface_spec` is the precedent and the reasoning is the same: a
    /// contract is enriched onto a node that already exists, often long after
    /// it was created. `add_capability` also has **276 call sites**, so
    /// widening it would churn the whole codebase to reach three optional
    /// fields — and the fields need backfilling onto capabilities that already
    /// exist far more often than declaring at birth.
    ///
    /// # The measurement this exists because of
    ///
    /// `Capability.capability_type`, `inputs` and `outputs` were declared in
    /// `schema/functional.yaml`, indexed, documented — and set on **0 of 170
    /// capabilities**, because `add_capability` writes only name, description
    /// and status and nothing anywhere else in either crate touched them. A
    /// capability's functional signature, what goes in and what comes out, had
    /// never once been recorded in a design that has been running for months
    /// (`fact:eighteen-declared-properties-nothing-has-ever-written`).
    ///
    /// That matters beyond tidiness: `req:recursive-black-box-decomposition`
    /// says every element of a design is a black box with inner function AND
    /// INTERFACES, nested as deep as the design needs. At the capability tier
    /// these two properties ARE that interface, and they were unwritable.
    ///
    /// # What this deliberately does NOT do
    ///
    /// **No detector fires when a capability lacks a signature.** 170 of them
    /// lack one today, so a gap per capability would put 170 findings in front
    /// of a reader overnight — the wall-of-red failure the vocabulary-coverage
    /// trial was run to avoid. Prompting for it at the right moment is the
    /// instruction leg and belongs to its own increment
    /// (`dec:idea-how-does-a-users-project-acquire-vocabulary-it-never-uses`,
    /// option (c)), not to this one.
    ///
    /// Refuses an unknown capability rather than creating one: a typo must not
    /// silently mint a capability whose only content is a signature.
    pub fn set_capability_signature(
        &mut self,
        capability_id: &str,
        capability_type: Option<&str>,
        inputs: Option<&[String]>,
        outputs: Option<&[String]>,
    ) -> Result<StoredNode, DynoError> {
        let Some(existing) = self.get_node(node::CAPABILITY, capability_id)? else {
            return Err(DynoError::NodeNotFound {
                node_type: node::CAPABILITY.to_string(),
                node_id: capability_id.to_string(),
            });
        };
        // The schema stores both as "JSON array of names/types", so the list
        // shape belongs to the caller and the serialisation belongs here —
        // otherwise every caller hand-writes JSON and one of them gets it wrong.
        let as_json = |v: Option<&[String]>| -> Option<String> {
            v.map(|items| serde_json::to_string(items).unwrap_or_else(|_| "[]".to_string()))
        };
        let inputs_json = as_json(inputs);
        let outputs_json = as_json(outputs);
        let incoming: [(&str, Option<&str>); 3] = [
            ("capability_type", capability_type),
            ("inputs", inputs_json.as_deref()),
            ("outputs", outputs_json.as_deref()),
        ];
        let mut props = Props::new();
        // Carry everything already stored, then overlay only what was supplied,
        // so recording the outputs cannot erase inputs somebody else declared.
        for (k, v) in &existing.properties {
            if !incoming
                .iter()
                .any(|(name, given)| name == k && given.is_some())
            {
                props = props.set(k, v.clone());
            }
        }
        for (name, given) in incoming {
            if let Some(value) = given {
                props = props.set(name, value);
            }
        }
        self.create_node(node::CAPABILITY, capability_id, props)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn set_interface_spec(
        &mut self,
        interface_id: &str,
        medium: Option<&str>,
        paradigm: Option<&str>,
        payload_format: Option<&str>,
        payload_schema: Option<&str>,
        endpoint: Option<&str>,
        operations: Option<&str>,
        auth: Option<&str>,
        transport_security: Option<&str>,
        error_model: Option<&str>,
    ) -> Result<StoredNode, DynoError> {
        let Some(existing) = self.get_node(node::INTERFACE, interface_id)? else {
            return Err(DynoError::NodeNotFound {
                node_type: node::INTERFACE.to_string(),
                node_id: interface_id.to_string(),
            });
        };
        let incoming = [
            // `medium` lives here rather than on `add_interface` (BL-129): it is
            // part of what a consumer must AGREE with, which is this tool's
            // subject, and the seam checker already compares it — two boundaries
            // can only be wired together if their media match. Until this it was
            // settable only through `create_node`, so anyone following the
            // obvious path left every interface at `unspecified` and collected
            // false single-point-of-failure warnings, having done nothing wrong.
            ("medium", medium),
            ("paradigm", paradigm),
            ("payload_format", payload_format),
            ("payload_schema", payload_schema),
            ("endpoint", endpoint),
            ("operations", operations),
            ("auth", auth),
            ("transport_security", transport_security),
            ("error_model", error_model),
        ];
        let mut props = Props::new();
        // Carry everything already stored, then overlay only what was supplied —
        // so a caller filling in the payload format cannot silently erase the
        // authentication somebody else recorded.
        for (k, v) in &existing.properties {
            if !incoming
                .iter()
                .any(|(name, given)| name == k && given.is_some())
            {
                props = props.set(k, v.clone());
            }
        }
        for (name, given) in incoming {
            if let Some(value) = given {
                props = props.set(name, value);
            }
        }
        self.create_node(node::INTERFACE, interface_id, props)
    }

    /// Designate a contract as a **published boundary** others may rely on, or
    /// back to **internal** plumbing the owner may change freely.
    ///
    /// The distinction MOSA's whole discipline turns on (a modular system
    /// interface, 10 U.S.C. 4401) and the one BL-45's system-of-systems thread
    /// found missing from the other direction. It earns its keep by being READ:
    /// `propagate_from` reports which published boundaries a change crosses, so
    /// "is this part severable" is computed rather than asserted
    /// (`req:key-interfaces`, `req:modularity-computed`).
    ///
    /// Publishing is a commitment, so it is always an explicit act — an
    /// Interface is `internal` until someone says otherwise.
    pub fn set_interface_designation(
        &mut self,
        interface_id: &str,
        designation: &str,
    ) -> Result<StoredNode, DynoError> {
        if !matches!(designation, "internal" | "published" | "required" | "both") {
            return Err(DynoError::Validation {
                node_type: node::INTERFACE.into(),
                property: "designation".into(),
                message: format!(
                    "'{designation}' is not an Interface designation (one of internal, published, \
                     required, both). `published` offers the contract, `required` needs one from \
                     outside, `both` does each, `internal` is plumbing nobody outside sees."
                ),
            });
        }
        let Some(existing) = self.get_node(node::INTERFACE, interface_id)? else {
            return Err(DynoError::NodeNotFound {
                node_type: node::INTERFACE.into(),
                node_id: interface_id.into(),
            });
        };
        let mut props = Props::new().set("designation", designation);
        for (k, v) in &existing.properties {
            if k != "designation" {
                props = props.set(k, v.clone());
            }
        }
        self.create_node(node::INTERFACE, interface_id, props)
    }

    /// Declare WHAT KIND of thing delivers a capability — a file, or a change
    /// to the design itself.
    ///
    /// ⭐ THIS DECLARES THE KIND, NEVER THE SUCCESS, and that distinction is
    /// what makes it safe to let an author state it. Delivery stays COMPUTED
    /// from the golden thread (`req:completion-computed`): both kinds still
    /// demand a passing check, and there is still no way to mark anything done.
    ///
    /// - `artifact` (the default): a file realizes it AND a check passes.
    /// - `model`: the deliverable IS the design change — a re-decomposition, a
    ///   retirement, a governance ruling, a correction to a model that was
    ///   wrong about the world. There is no file to point at, so the check is
    ///   the whole of the evidence.
    ///
    /// The same shape as [`set_interface_designation`](Self::set_interface_designation)
    /// and `set_artifact_intent`: the author says the one thing only they can
    /// know, and the computation earns the rest.
    pub fn set_capability_delivery(
        &mut self,
        capability_id: &str,
        delivery: &str,
    ) -> Result<StoredNode, DynoError> {
        if !matches!(delivery, "artifact" | "model") {
            return Err(DynoError::Validation {
                node_type: node::CAPABILITY.into(),
                property: "delivery".into(),
                message: format!(
                    "'{delivery}' is not a delivery kind (one of artifact, model). `artifact` \
                     means a file realizes it and delivery needs both the file and a passing \
                     check; `model` means the deliverable IS the design change, so the check is \
                     the whole of the evidence. It says what KIND delivers this, never whether \
                     it was delivered — that stays computed."
                ),
            });
        }
        let Some(existing) = self.get_node(node::CAPABILITY, capability_id)? else {
            return Err(DynoError::NodeNotFound {
                node_type: node::CAPABILITY.into(),
                node_id: capability_id.into(),
            });
        };
        let mut props = Props::new().set("delivery", delivery);
        for (k, v) in &existing.properties {
            if k != "delivery" {
                props = props.set(k, v.clone());
            }
        }
        self.create_node(node::CAPABILITY, capability_id, props)
    }

    /// Mark a requirement as a promise this design publishes, or take that back
    /// (`req:publishable-promise`).
    ///
    /// A behavioural commitment — "fails loud rather than falling back",
    /// "ordering is preserved" — lives in a Requirement, and every Requirement
    /// was withheld from `export_surface` as internal. So a design could hold a
    /// promise, a consumer could depend on it, and the published surface could
    /// not carry it between them. Found by a real trial: the promise ended up
    /// asserted in a *comment in the consumer's build file*, on the wrong side of
    /// the seam.
    ///
    /// Publishing is a commitment, so `internal` is the default and reaching
    /// `published` is a deliberate act — the same rule, for the same reason, as
    /// [`set_interface_designation`](Self::set_interface_designation).
    pub fn set_requirement_designation(
        &mut self,
        requirement_id: &str,
        designation: &str,
    ) -> Result<StoredNode, DynoError> {
        if !matches!(designation, "internal" | "published") {
            return Err(DynoError::Validation {
                node_type: node::REQUIREMENT.into(),
                property: "designation".into(),
                message: format!(
                    "'{designation}' is not a requirement designation (one of internal, published)"
                ),
            });
        }
        let Some(existing) = self.get_node(node::REQUIREMENT, requirement_id)? else {
            return Err(DynoError::NodeNotFound {
                node_type: node::REQUIREMENT.into(),
                node_id: requirement_id.into(),
            });
        };
        let mut props = Props::new().set("designation", designation);
        for (k, v) in &existing.properties {
            if k != "designation" {
                props = props.set(k, v.clone());
            }
        }
        self.create_node(node::REQUIREMENT, requirement_id, props)
    }

    /// The promises this design publishes, by id — the behavioural half of a
    /// published surface (`req:publishable-promise`).
    pub fn published_promises(&self) -> Result<std::collections::BTreeSet<String>, DynoError> {
        Ok(self
            .scan_nodes(node::REQUIREMENT)?
            .into_iter()
            .filter(|n| {
                n.properties
                    .get("designation")
                    .and_then(Value::as_str)
                    .is_some_and(|d| d == "published")
            })
            .map(|n| n.node_id)
            .collect())
    }

    /// The published boundaries in this design, by id — the set impact analysis
    /// checks a blast radius against, and the OFFER side of pairing.
    ///
    /// `both` counts: a boundary that offers and needs is still offered, and
    /// omitting it here would hide a real published surface from impact analysis
    /// the moment anyone used the value.
    pub fn published_interfaces(&self) -> Result<std::collections::BTreeSet<String>, DynoError> {
        self.interfaces_designated(&["published", "both"])
    }

    /// The boundaries this design NEEDS from outside, by id — the SUBSCRIBE side
    /// of pairing (`req:complementary-pairing`), and the half that did not exist
    /// until 2026-07-30.
    ///
    /// `both` counts here too, for the mirror of the reason above.
    pub fn required_interfaces(&self) -> Result<std::collections::BTreeSet<String>, DynoError> {
        self.interfaces_designated(&["required", "both"])
    }

    /// Boundaries carrying no external role, by id.
    ///
    /// Reported rather than silently skipped: `internal` is the DEFAULT, so it
    /// cannot distinguish "deliberately internal" from "nobody classified this",
    /// and a design that never did the labelling would otherwise pair with
    /// nothing and report a clean seam — the blindness `cap:coverage` exists to
    /// end.
    pub fn unclassified_interfaces(&self) -> Result<std::collections::BTreeSet<String>, DynoError> {
        self.interfaces_designated(&["internal"])
    }

    fn interfaces_designated(
        &self,
        roles: &[&str],
    ) -> Result<std::collections::BTreeSet<String>, DynoError> {
        Ok(self
            .scan_nodes(node::INTERFACE)?
            .into_iter()
            .filter(|n| {
                // Absent reads as the schema default rather than as "no role",
                // so a graph written before this property existed behaves as it
                // always did instead of vanishing from every set at once.
                let d = n
                    .properties
                    .get("designation")
                    .and_then(Value::as_str)
                    .unwrap_or("internal");
                roles.contains(&d)
            })
            .map(|n| n.node_id)
            .collect())
    }

    /// P2 · Structure — a recorded decision with its rationale (an ADR, in
    /// software terms). `name` and `decision` are required; `rationale` is
    /// optional but is the part worth having — HEAL raises a `contradiction`
    /// when two nodes disagree with no Decision resolving them, and a Decision
    /// without a reason does not actually resolve anything.
    pub fn add_decision(
        &mut self,
        id: &str,
        name: &str,
        decision: &str,
        rationale: Option<&str>,
    ) -> Result<StoredNode, DynoError> {
        self.upsert_node(
            node::DECISION,
            id,
            Props::new()
                .set("name", name)
                .set("decision", decision)
                .set_opt("rationale", rationale),
        )
    }

    // ---- Typed golden-thread edges ----------------------------------------

    /// `Project CONTAINS child` — the containment spine (axis Y).
    pub fn contains(
        &mut self,
        project_id: &str,
        child_type: &str,
        child_id: &str,
    ) -> Result<StoredEdge, DynoError> {
        self.create_edge(
            edge::CONTAINS,
            node::PROJECT,
            project_id,
            child_type,
            child_id,
            Props::new(),
        )
    }

    /// `parent Component CONTAINS child Component` — the component decomposition
    /// spine (axis Y / matryoshka). Parent should be exactly one `Component.level`
    /// above the child; see [`crate::hierarchy`].
    pub fn contain_component(
        &mut self,
        parent_id: &str,
        child_id: &str,
    ) -> Result<StoredEdge, DynoError> {
        self.create_edge(
            edge::CONTAINS,
            node::COMPONENT,
            parent_id,
            node::COMPONENT,
            child_id,
            Props::new(),
        )
    }

    /// `Capability SATISFIES Requirement` — the traceability link that binds
    /// WHAT back to intent (the golden thread).
    pub fn satisfies(
        &mut self,
        capability_id: &str,
        requirement_id: &str,
    ) -> Result<StoredEdge, DynoError> {
        self.create_edge(
            edge::SATISFIES,
            node::CAPABILITY,
            capability_id,
            node::REQUIREMENT,
            requirement_id,
            Props::new(),
        )
    }

    /// `child DECOMPOSES parent` — split one requirement into smaller testable
    /// pieces that add no new information, and mark the child `decomposed`.
    ///
    /// Refuses a self-loop and refuses to introduce a cycle: a decomposition
    /// that contains itself has no leaves, so delivery could never roll up
    /// through it and "satisfy every child" would be unsatisfiable by
    /// construction. Cheaper to refuse here than to detect later as a defect
    /// nobody can act on.
    ///
    /// Sets `lineage` on the child rather than leaving it to the caller: the
    /// edge and the label are the same fact, and letting them disagree would
    /// make the classification a second thing to maintain — which is how the
    /// requirement lifecycle went unused in the first place.
    pub fn decomposes(&mut self, child_id: &str, parent_id: &str) -> Result<StoredEdge, DynoError> {
        if child_id == parent_id {
            return Err(DynoError::Validation {
                node_type: node::REQUIREMENT.into(),
                property: "DECOMPOSES".into(),
                message: format!("'{child_id}' cannot decompose itself"),
            });
        }
        // Walk the parent's own ancestry; if the child is up there, this edge
        // would close a loop.
        let mut seen = std::collections::BTreeSet::new();
        let mut frontier = vec![parent_id.to_string()];
        while let Some(id) = frontier.pop() {
            if id == child_id {
                return Err(DynoError::Validation {
                    node_type: node::REQUIREMENT.into(),
                    property: "DECOMPOSES".into(),
                    message: format!(
                        "'{child_id}' already sits above '{parent_id}', so this would make the \
                         decomposition circular — a tree with no leaves can never roll up"
                    ),
                });
            }
            if !seen.insert(id.clone()) {
                continue;
            }
            for e in self.outgoing(&id, Some(edge::DECOMPOSES))? {
                frontier.push(e.to_id);
            }
        }

        let edge = self.create_edge(
            edge::DECOMPOSES,
            node::REQUIREMENT,
            child_id,
            node::REQUIREMENT,
            parent_id,
            Props::new(),
        )?;
        self.set_requirement_lineage(child_id, "decomposed")?;
        Ok(edge)
    }

    /// Set a Requirement's `lineage` — where it came from, as opposed to how it
    /// entered the graph (`provenance`). Preserves every other property.
    pub fn set_requirement_lineage(
        &mut self,
        requirement_id: &str,
        lineage: &str,
    ) -> Result<StoredNode, DynoError> {
        const LINEAGES: [&str; 3] = ["original", "decomposed", "derived"];
        if !LINEAGES.contains(&lineage) {
            return Err(DynoError::Validation {
                node_type: node::REQUIREMENT.into(),
                property: "lineage".into(),
                message: format!(
                    "'{lineage}' is not a requirement lineage (one of {})",
                    LINEAGES.join(", ")
                ),
            });
        }
        let Some(existing) = self.get_node(node::REQUIREMENT, requirement_id)? else {
            return Err(DynoError::NodeNotFound {
                node_type: node::REQUIREMENT.into(),
                node_id: requirement_id.into(),
            });
        };
        let mut props = Props::new().set("lineage", lineage);
        for (k, v) in &existing.properties {
            if k != "lineage" {
                props = props.set(k, v.clone());
            }
        }
        self.create_node(node::REQUIREMENT, requirement_id, props)
    }

    /// The requirements this one was split into — its direct children.
    pub fn decomposed_children(&self, parent_id: &str) -> Result<Vec<String>, DynoError> {
        let mut kids: Vec<String> = self
            .incoming(parent_id, Some(edge::DECOMPOSES))?
            .into_iter()
            .map(|e| e.from_id)
            .collect();
        kids.sort();
        Ok(kids)
    }

    /// `Capability ALLOCATED_TO Component` — the WHAT→WHERE binding.
    pub fn allocate(
        &mut self,
        capability_id: &str,
        component_id: &str,
    ) -> Result<StoredEdge, DynoError> {
        self.create_edge(
            edge::ALLOCATED_TO,
            node::CAPABILITY,
            capability_id,
            node::COMPONENT,
            component_id,
            Props::new(),
        )
    }

    /// `Component DEPENDS_ON Component` — the coupling the topology rules read.
    ///
    /// # Why this exists as a typed helper
    ///
    /// It is the single most common structural edge in any design, and it feeds
    /// cycle detection, single-point-of-failure analysis and the seam gap —
    /// and until 2026-09-01 it was the one edge with no helper, so recording a
    /// coupling meant a raw `create_edge` with both endpoint types spelled out.
    ///
    /// Reported by musicjug (`fact:the-commonest-structural-edge-has-no-typed-helper-and-its-name-is-taken`):
    /// searching for it found `external_dependency`, which pins a version of
    /// ANOTHER DESIGN and is a different concept entirely. That tool is now
    /// `external_dependency`, freeing this name.
    ///
    /// ⚠️ THE HELPER DOES NOT MAKE THE MODELLING HAPPEN. Measured on reflow2's
    /// own design the day this landed: 1 of 70 components carried any coupling,
    /// so modularity reported "not measurable" rather than a score. This
    /// removes the excuse, not the work.
    pub fn depends_on(
        &mut self,
        from_component_id: &str,
        to_component_id: &str,
    ) -> Result<StoredEdge, DynoError> {
        self.create_edge(
            edge::DEPENDS_ON,
            node::COMPONENT,
            from_component_id,
            node::COMPONENT,
            to_component_id,
            Props::new(),
        )
    }

    /// `node GOVERNED_BY Decision/DesignRule` — the node is shaped by a
    /// recorded decision. `from_type` and `to_type` are required: the schema
    /// allows any endpoints (`from: "*"`, `to: "*"`).
    ///
    /// `ruling` says what KIND of governance this is. `None` — the ordinary
    /// case — means the target simply shapes the source. `Some("parks")` means
    /// the ruling declares the source's unattached or unsatisfied state to be
    /// CORRECT AND DELIBERATE (`req:a-deliberate-state-is-not-a-defect`).
    ///
    /// ⚠️ THE PARAMETER EXISTS BECAUSE THE PROPERTY WOULD OTHERWISE BE
    /// UNREACHABLE FROM HERE, and that trap was measured hours earlier the same
    /// day: `Verification.description` was declared, fulltext, the embedding
    /// field — and used ONCE IN 164 NODES, because `add_verification` had no
    /// parameter for it and the only route was raw `create_node`. Declaring
    /// `ruling` without exposing it here would have been the fourth instance of
    /// one pattern in a day (after `description`, `SUPERSEDES`, and the
    /// `revision` block missing from `create_node`). A declared field nobody
    /// can reach is a declared field nobody writes to.
    ///
    /// ⭐ AND `note` IS THE SAME TRAP, CAUGHT ONE FIELD LATER. The schema
    /// declared `note` on GOVERNED_BY the whole time; this constructor could
    /// not reach it, so `describe_schema` advertised a field the typed write
    /// path refused. dev_storyflow hit it on 2026-08-23, wanted the note twice
    /// in one session, and both times fell back to raw `create_edge` — which
    /// works and abandons this path's validation for the whole call. The note
    /// is the part a later reader actually needs: WHY this ruling binds this
    /// node. An agent less inclined to run `describe_schema` simply drops the
    /// reasoning, which is the silent half of the failure.
    /// Record that this design record ANSWERED a question.
    ///
    /// The half `Question.answer` always promised — "the design nodes it
    /// produced are linked separately" — and nothing delivered until
    /// 2026-09-02. Draw it as you write the answer in, so a later session can
    /// tell an answer that reached the design from one that reached only the
    /// chat.
    pub fn answers(
        &mut self,
        from_type: &str,
        from_id: &str,
        question_id: &str,
        note: Option<&str>,
    ) -> Result<StoredEdge, DynoError> {
        self.create_edge(
            edge::ANSWERS,
            from_type,
            from_id,
            node::QUESTION,
            question_id,
            Props::new().set_opt("note", note),
        )
    }

    pub fn governed_by(
        &mut self,
        from_type: &str,
        from_id: &str,
        to_type: &str,
        to_id: &str,
        ruling: Option<&str>,
        note: Option<&str>,
    ) -> Result<StoredEdge, DynoError> {
        self.create_edge(
            edge::GOVERNED_BY,
            from_type,
            from_id,
            to_type,
            to_id,
            Props::new().set_opt("ruling", ruling).set_opt("note", note),
        )
    }

    /// Record that a design node was authored (or reviewed/approved) by a
    /// [`Contributor`](node::CONTRIBUTOR) — the structured "who" behind
    /// provenance's "how". [`AUTHORED_BY`](edge::AUTHORED_BY) is deliberately not
    /// a traceability edge, so this never enlarges a blast radius.
    pub fn authored_by(
        &mut self,
        from_type: &str,
        from_id: &str,
        contributor_id: &str,
        role: Option<&str>,
        acted_at: Option<&str>,
    ) -> Result<StoredEdge, DynoError> {
        self.require_contributor(contributor_id, "authored_by", "author this")?;
        self.create_edge(
            edge::AUTHORED_BY,
            from_type,
            from_id,
            node::CONTRIBUTOR,
            contributor_id,
            Props::new()
                .set_opt("role", role)
                .set_opt("acted_at", acted_at),
        )
    }

    /// Refuse a who-edge in a way that says what WOULD have worked.
    ///
    /// # Why this exists, and why it is worth a function rather than a message
    ///
    /// Without it, both of these render identically as
    /// `Node not found: Contributor <id>`, from `create_edge`'s endpoint rule:
    ///
    /// - the id names nothing at all (a typo), and
    /// - the id names a real node that is an **`Actor`** rather than a
    ///   `Contributor`.
    ///
    /// The second one is the trap, because the node *is* there. A caller told
    /// "not found" goes looking for a wrong id and never suspects a wrong TYPE.
    /// Reported by dev_storyflow on 2026-08-15, and reported **against the
    /// reporter itself**, which is what makes it worth acting on: rather than
    /// retry, the session wrote the user's authorship into Decision PROSE about
    /// ten times, and that is the direct reason `what_next` had nothing to show
    /// in its most important band. Its own conclusion — *"the failure mode of a
    /// rejected typed call is not an error, it is prose"* — a capable agent
    /// routes around the refusal and produces something that READS BETTER than
    /// the edge would have, while quietly dropping the structure.
    ///
    /// **This is not a new principle here, it is an existing one applied to a
    /// third case.** `claim_region` names its fix (and its comment records the
    /// same shape: an unactionable refusal became a false correction that
    /// travelled five hops through a fleet before anyone disproved it); the
    /// missing-`seat` refusal has named its fix since 2026-07-30; and
    /// `get_node` refuses an unknown node type rather than answering a
    /// confident `None`, because "no such type" and "no such node" are
    /// different facts that must not share one reply. This is that sentence,
    /// one edge along.
    ///
    /// Only `Actor` is probed for the wrong-type case. That is deliberate
    /// rather than lazy: it is the confusion actually observed, and the only
    /// other person-shaped node type, so a general cross-type scan would cost
    /// every caller a sweep to report a case that cannot arise.
    fn require_contributor(&self, id: &str, tool: &str, verb: &str) -> Result<(), DynoError> {
        if self.get_node(node::CONTRIBUTOR, id)?.is_some() {
            return Ok(());
        }
        let message = if self.get_node(node::ACTOR, id)?.is_some() {
            format!(
                "'{id}' EXISTS BUT IS AN Actor, and `{tool}` needs a Contributor — so this is a \
                 wrong TYPE, not a wrong id, and looking for a better id will not find one. The \
                 two are different on purpose: an Actor is a role the DESIGNED SYSTEM interacts \
                 with, a Contributor is a person who works on the design. To {verb}, call \
                 `add_contributor` for the human (its id may differ from '{id}'), then call \
                 `{tool}` again with that id."
            )
        } else {
            format!(
                "no Contributor '{id}' exists yet, so there is nobody to {verb}. CALL \
                 `add_contributor` FIRST with id '{id}', then call `{tool}` again. (reflow2 will \
                 not invent the person: authorship nobody declared is a name no colleague can ask \
                 about.)"
            )
        };
        Err(DynoError::Validation {
            node_type: node::CONTRIBUTOR.into(),
            property: "contributor_id".into(),
            message,
        })
    }

    /// Record that a design node is OWNED by a [`Contributor`](node::CONTRIBUTOR)
    /// — whose AREA it is, durable and never released.
    ///
    /// The third "who" axis, and the one that was missing.
    /// [`AUTHORED_BY`](edge::AUTHORED_BY) is past tense and never changes;
    /// [`CLAIMS`](edge::CLAIMS) is who is in it right now and is released at
    /// checkout; this survives every session.
    ///
    /// # Why not a claim, which was the cheap answer
    ///
    /// `claim_region` is advisory and session-scoped by its own description, the
    /// parallel-work skill says release at checkout, and on a shared server a
    /// claim reads `unknown` rather than `live` and never expires. Standing
    /// ownership claims would permanently drown that tool's actual job —
    /// showing who is ACTIVELY in your ground. `dec:ownership-reads-claims-before-adding-an-edge`
    /// set exactly this condition: decide on an edge once claims are "shown to
    /// be insufficient", naming *transient work-in-hand versus durable
    /// ownership* as the disqualifying evidence in advance.
    ///
    /// # What it does not do
    ///
    /// It is deliberately NOT a traceability edge — absent from
    /// `structural_rule`, the third of a kind after `AUTHORED_BY` and `CLAIMS` —
    /// so ownership never propagates a blast radius and a Contributor never
    /// becomes a hub. Owning something says who answers for it, not that a
    /// change to it changes them.
    ///
    /// And an unowned node is NOT a gap. Most nodes in a mature design have no
    /// owner, so absence is ordinary; whether unowned ground should be detected
    /// at all is open in `dec:idea-detect-ownership-orphans` and turns on a
    /// per-project answer to which types are even ownable.
    pub fn owned_by(
        &mut self,
        from_type: &str,
        from_id: &str,
        contributor_id: &str,
        note: Option<&str>,
        since: Option<&str>,
    ) -> Result<StoredEdge, DynoError> {
        // Same refusal as `authored_by`: `owned_by` takes a Contributor for the
        // same reasons and fails the same indistinguishable way without it.
        self.require_contributor(contributor_id, "owned_by", "own this")?;
        self.create_edge(
            edge::OWNED_BY,
            from_type,
            from_id,
            node::CONTRIBUTOR,
            contributor_id,
            Props::new().set_opt("note", note).set_opt("since", since),
        )
    }

    /// Every node this contributor owns, by id.
    ///
    /// Computed on demand rather than stored, so it follows the design.
    pub fn owned_by_contributor(
        &self,
        contributor_id: &str,
    ) -> Result<std::collections::BTreeSet<String>, DynoError> {
        Ok(self
            .incoming(contributor_id, Some(edge::OWNED_BY))?
            .into_iter()
            .map(|e| e.from_id)
            .collect())
    }

    /// `Component PROVIDES Interface` — the side of a contract that implements it.
    pub fn provides(
        &mut self,
        component_id: &str,
        interface_id: &str,
    ) -> Result<StoredEdge, DynoError> {
        self.create_edge(
            edge::PROVIDES,
            node::COMPONENT,
            component_id,
            node::INTERFACE,
            interface_id,
            Props::new(),
        )
    }

    /// `Component CONSUMES Interface` — the side of a contract that depends on it.
    ///
    /// This is the edge that makes "changed one side, forgot the other"
    /// findable: from the provider, PROPAGATE reaches every consumer laterally
    /// through the Interface.
    pub fn consumes(
        &mut self,
        consumer_id: &str,
        interface_id: &str,
    ) -> Result<StoredEdge, DynoError> {
        self.create_edge(
            edge::CONSUMES,
            node::COMPONENT,
            consumer_id,
            node::INTERFACE,
            interface_id,
            Props::new(),
        )
    }
}
