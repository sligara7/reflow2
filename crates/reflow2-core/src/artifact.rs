//! Artifact linking — connect real deliverables back to the design (SP-6).
//!
//! The loop's closing move: the agent builds a real file (a Unity C# script, an
//! OpenAPI spec, a doc) and registers it as an [`Artifact`](crate::nodes::node::ARTIFACT)
//! that **`REALIZES`** the Capability/Component it implements, with provenance. That
//! keeps as-designed vs as-built honest and makes DETECT's `unrealized_capability`
//! gap productive (a Capability with no incoming `REALIZES` is a build gap).
//!
//! The read side already exists (DETECT/HEAL/PROPAGATE/report all expect
//! `Artifact → Capability` REALIZES); this module is the missing **write side**.
//!
//! ## Provenance
//!
//! An `Artifact` carries no `provenance` property — provenance lives on a
//! [`Fragment`](crate::nodes::node::FRAGMENT). So [`DesignGraph::link_artifact`]
//! records provenance the same way INGEST does: it creates a provenance Fragment
//! that `YIELDED` (action `created`) the Artifact. Bare [`add_artifact`] /
//! [`realizes`](DesignGraph::realizes) skip the Fragment when provenance isn't needed.
//!
//! Scope: the write side of the link, plus the drift **baseline** — an optional
//! `checksum` recorded at link time. Comparing that baseline against observed
//! reality is [`crate::drift`] (SP-6b).

use dynograph_core::DynoError;
use dynograph_storage::{StoredEdge, StoredNode};

use crate::graph::DesignGraph;
use crate::nodes::{Props, edge, node};
use crate::temporal::{ChangeAction, ChangeType};

/// Canonicalise a recorded checksum to `sha256:<hex>`.
///
/// Drift is detected by comparing the registered checksum against one observed
/// from disk, and that comparison is a **string** comparison — so `abc123…` and
/// `sha256:abc123…` are the same digest and total drift at the same time. On
/// 2026-07-25 four artifacts were registered from raw `sha256sum` output, and
/// the coherence gate reported every one of them as "the build no longer matches
/// the committed design" while the bytes matched exactly. A false red on a gate
/// whose whole job is to be believed is worse than no gate.
///
/// A bare hex digest is unambiguous, so it is canonicalised rather than refused
/// — and the stored value comes back in the returned artifact, so the caller
/// sees what was written. Anything else (a different algorithm's prefix, a
/// non-hex fingerprint) is stored verbatim: this normalises a known dialect, it
/// does not police the field.
///
/// **Applied on BOTH sides of the comparison, not only on the way in** (BL-125).
/// It lived here as a write-side fix from 2026-07-25 until 2026-08-01, while
/// `drift.rs` compared literally — so the identical false red came back through
/// the read door, and a caller who passed a bare hash to `link_artifact` and the
/// same bare hash to `reconcile_artifacts` was told every artifact of an
/// untouched tree had drifted. A normalisation that only one end of a comparison
/// performs is not a normalisation.
pub(crate) fn canonical_checksum(checksum: &str) -> String {
    let is_bare_hex = !checksum.is_empty()
        && checksum.len() <= 64
        && checksum.chars().all(|c| c.is_ascii_hexdigit());
    if is_bare_hex {
        format!("sha256:{checksum}")
    } else {
        checksum.to_string()
    }
}

/// Whether two canonicalised checksums are the **same digest** — including when
/// they were written at different LENGTHS (BL-160).
///
/// A design registers whatever its tooling produced. reflow2's own
/// `tools/build_design_graph.py` writes `hexdigest()[:16]`; a caller running
/// `sha256sum` supplies all 64. Both describe the same bytes, and comparing
/// them as strings says the file changed. On 2026-08-01 that reported **51
/// phantom drifts on a provably clean tree** in the same minute the coherence
/// gate said the design and the build agreed — because `reflow2_check.py`
/// carried a Python workaround truncating the observation to the registered
/// length, and nothing else did. This is [`canonical_checksum`]'s own bug in a
/// second form, with the same verdict: *a false red on a gate whose whole job
/// is to be believed is worse than no gate*. Putting the rule here answers it
/// for every consumer, not just the one that knew.
///
/// **What is required is a real prefix relationship, never truncate-both-to-N.**
/// Two full digests that happen to share sixteen characters are two different
/// digests and the file really did move. And prefix tolerance is a fact about
/// hex digests of one algorithm — where a truncation genuinely is a prefix of
/// the whole — so it applies to the `sha256:` dialect only; letting it loose on
/// an arbitrary fingerprint would make `blake3:zz` and `blake3:zzzz` agree,
/// which is the massage-everything-into-equality failure this pair of functions
/// exists to avoid.
///
/// No minimum length is imposed. A short baseline is a weak baseline, but its
/// strength is decided when it is registered — the write side takes a 16-char
/// digest without complaint, and a read side that then refused to honour it
/// would be the same write/read disagreement all over again.
pub(crate) fn checksums_agree(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let (Some(a_hex), Some(b_hex)) = (a.strip_prefix("sha256:"), b.strip_prefix("sha256:")) else {
        return false;
    };
    // Both sides non-empty: `"anything".starts_with("")` is true, so a bare
    // `sha256:` with no digest behind it would otherwise agree with everything.
    !a_hex.is_empty() && !b_hex.is_empty() && (a_hex.starts_with(b_hex) || b_hex.starts_with(a_hex))
}

/// Which way an accepted drift went — the answer to the **second question**.
///
/// The code moved; that much is observed. Accepting the new baseline is a
/// decision about what the movement *meant*, and the erosion trial showed why
/// it cannot be silent: five legitimate fix cycles, each accepted with no
/// question asked, left a design describing a system that no longer existed —
/// while reporting zero gaps. The third option, "accept the file, leave the
/// design alone, say nothing", is the one that erodes, so it does not exist:
/// every accept states which of these it is, and the claim goes on axis Z.
#[derive(Debug, Clone)]
pub enum DriftDisposition<'a> {
    /// The change carries no design meaning — a refactor, a cosmetic fix, a
    /// bug fix restoring intended behaviour. This is itself a recorded claim:
    /// a `ChangeEvent` is written saying the design was judged to still hold
    /// against this checksum. The claim can be wrong, but it cannot be silent,
    /// and it is dated — which is exactly what a later freshness check reads.
    DesignHolds {
        /// Why the code moved (usually [`ChangeType::TestFailureFix`] or
        /// [`ChangeType::Refactor`]).
        change_type: ChangeType,
    },
    /// The behaviour moved and the design moved with it. References the
    /// `ChangeEvent` recorded (via
    /// [`record_change`](DesignGraph::record_change)) when the design was
    /// updated — the same event is linked to the artifact, so the code accept
    /// and the design edit are one change on axis Z, not two coincidences.
    DesignUpdated {
        /// The existing `ChangeEvent` from the design-side update. Verified to
        /// exist; a dangling reference is refused rather than recorded.
        change_event_id: &'a str,
    },
    /// **Nothing moved.** This artifact was registered with no checksum and is
    /// getting its first one, so there is no divergence to take a position on
    /// (BL-157).
    ///
    /// The other two both presuppose a baseline: `DesignHolds` claims *the code
    /// moved and the change carried no design meaning*, `DesignUpdated` claims
    /// *behaviour moved and the design moved with it*. Neither is true of a
    /// first baseline, and forcing one anyway is not a harmless approximation —
    /// it writes a `refactor` of a file nobody touched into the very ledger that
    /// exists to stop the design accumulating fiction. Found the only way it
    /// could be: by having to do it (`art:detect`, 2026-08-01).
    ///
    /// Carries no `change_type` because the `ChangeEvent` it writes names
    /// `baseline_established` — the record moved and the code did not.
    ///
    /// **This is not a way around the two-sided accept.** Establishing a
    /// baseline over an artifact that already has one is refused, because that
    /// is exactly the shape a real drift would take if you wanted to launder it
    /// past the disposition question.
    BaselineEstablished,
}

/// Inputs for [`DesignGraph::link_artifact`] — register a real file against the
/// design with provenance. Serializable: it crosses the MCP boundary.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LinkArtifactOptions {
    /// Stable Artifact id (e.g. `art:ball-physics`).
    pub artifact_id: String,
    /// Artifact name.
    pub name: String,
    /// Where the artifact lives (path / URI / content-hash). Points outside the graph.
    #[serde(default)]
    pub location: Option<String>,
    /// `code` (default) / `spec` / `document` / `diagram` / `model` / …
    #[serde(default)]
    pub artifact_type: Option<String>,
    /// Node type the artifact realizes (e.g. `Capability`, `Component`).
    pub target_type: String,
    /// Node id the artifact realizes.
    pub target_id: String,
    /// REALIZES completeness: `stub` / `partial` / `complete`.
    #[serde(default)]
    pub completeness: Option<String>,
    /// Provenance stamped on the Fragment (default `authored`).
    #[serde(default)]
    pub provenance: Option<String>,
    /// Provenance Fragment id (default `frag:<artifact_id>`).
    #[serde(default)]
    pub fragment_id: Option<String>,
    /// Content hash of the file as registered — the baseline
    /// [`reconcile_artifacts`](DesignGraph::reconcile_artifacts) compares
    /// against later. Opaque to reflow2; the caller picks the algorithm. Without
    /// it the artifact can still be checked for existence, but a content change
    /// is reported as `no_baseline` rather than passing silently.
    #[serde(default)]
    pub checksum: Option<String>,
}

/// What [`DesignGraph::link_artifact`] created.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ArtifactLink {
    /// The Artifact node id.
    pub artifact_id: String,
    /// The realized target node id.
    pub target_id: String,
    /// The provenance Fragment id that `YIELDED` the Artifact.
    pub fragment_id: String,
    /// The REALIZES completeness recorded (as stored).
    pub completeness: String,
    /// The provenance recorded on the Fragment (as stored).
    pub provenance: String,
}

impl DesignGraph {
    /// Create an `Artifact` node — a deliverable that lives outside the graph.
    /// `name` is required; `artifact_type` (default `code`) and `location` are
    /// optional (omitted rather than blank when absent).
    pub fn add_artifact(
        &mut self,
        id: &str,
        name: &str,
        artifact_type: Option<&str>,
        location: Option<&str>,
    ) -> Result<StoredNode, DynoError> {
        self.upsert_node(
            node::ARTIFACT,
            id,
            Props::new()
                .set("name", name)
                .set_opt("artifact_type", artifact_type)
                .set_opt("location", location),
        )
    }

    /// Accept a drifted artifact's new content hash as the baseline — a
    /// **two-sided decision**, never a silent update (BL-33).
    ///
    /// Updating the baseline answers "which state do we compare against next
    /// time?". The [`DriftDisposition`] answers the question that used to go
    /// unasked: *the code moved — did the design move too, or did it not need
    /// to?* Both answers leave a `ChangeEvent` `CHANGED`-linked to the
    /// artifact, so the accept is on axis Z either way:
    ///
    /// - [`DriftDisposition::DesignHolds`] writes a new event recording the
    ///   claim that this change carried no design meaning, at a deterministic
    ///   id (`chg:accept-…` hashed from artifact + checksum) so re-accepting
    ///   the same state is idempotent.
    /// - [`DriftDisposition::DesignUpdated`] links the **existing** event from
    ///   the design-side [`record_change`](DesignGraph::record_change) to the
    ///   artifact — one change, both sides. A `change_event_id` that does not
    ///   exist is refused loudly: a phantom reference would let the claim
    ///   "the design was updated" stand with nothing behind it.
    ///
    /// Returns the updated artifact and the id of the change event the accept
    /// now hangs off.
    pub fn set_artifact_checksum(
        &mut self,
        artifact_id: &str,
        checksum: &str,
        disposition: DriftDisposition<'_>,
        note: Option<&str>,
        at: Option<&str>,
    ) -> Result<(StoredNode, String), DynoError> {
        let Some(existing) = self.get_node(node::ARTIFACT, artifact_id)? else {
            return Err(DynoError::NodeNotFound {
                node_type: node::ARTIFACT.to_string(),
                node_id: artifact_id.to_string(),
            });
        };
        // Canonicalise before the accept id is hashed, so re-accepting the same
        // state stays idempotent whichever dialect the caller typed.
        let mut checksum = canonical_checksum(checksum);

        // Which of the three answers is even available is a FACT about the
        // artifact, not a preference: a first baseline compares against nothing,
        // and an accept has nothing to accept without one. Both directions are
        // refused rather than reported, because each wrong way round writes a
        // specific fiction into the ledger (BL-157) — and the guard is what stops
        // `baseline_established` becoming a way to launder a real drift past the
        // two-sided decision.
        let recorded = existing
            .properties
            .get("checksum")
            .and_then(|v| v.as_str())
            .filter(|c| !c.is_empty())
            .map(canonical_checksum);
        let had_baseline = recorded.is_some();
        // Re-establishing the SAME baseline is a no-op and stays idempotent, so
        // re-running a sweep is safe. What is refused is a `baseline_established`
        // that would MOVE one — which is the laundering case, and the only one
        // the guard is for.
        //
        // "The same" is [`checksums_agree`], not `==`: a bulk BL-157 sweep that
        // hashes with `sha256sum` and meets a 16-char baseline is re-stating the
        // digest, not moving it, and refusing that would fail the sweep on every
        // short-registered artifact for a change that never happened (BL-160).
        let would_move_baseline = recorded
            .as_deref()
            .is_some_and(|r| !checksums_agree(r, &checksum));
        // When the two dialects agree, the LONGER digest is what stays on the
        // record: it contradicts nothing the shorter one said, carries more of
        // the evidence, and makes the accept idempotent ACROSS dialects, since
        // the event id is hashed from the value that is stored.
        if let Some(r) = recorded.as_deref()
            && !would_move_baseline
            && r.len() > checksum.len()
        {
            checksum = r.to_string();
        }
        let checksum = &checksum;
        match (&disposition, had_baseline) {
            (DriftDisposition::BaselineEstablished, true) if would_move_baseline => {
                return Err(DynoError::Validation {
                    node_type: node::ARTIFACT.into(),
                    property: "checksum".into(),
                    message: format!(
                        "'{artifact_id}' already has a baseline and this would move it, so it \
                         is not a first one. `baseline_established` says NOTHING MOVED; using \
                         it here would accept a real change without answering what the change \
                         meant, which is the silent accept `dec:two-sided-accept` exists to \
                         prevent. Pass `design_holds` (the change carries no design meaning) or \
                         `design_updated` (the design moved with it)"
                    ),
                });
            }
            (
                DriftDisposition::DesignHolds { .. } | DriftDisposition::DesignUpdated { .. },
                false,
            ) => {
                return Err(DynoError::Validation {
                    node_type: node::ARTIFACT.into(),
                    property: "checksum".into(),
                    message: format!(
                        "'{artifact_id}' has no recorded checksum, so there is no baseline to \
                         accept a change against — both `design_holds` and `design_updated` \
                         would be claiming something about a movement nobody observed. This is a \
                         FIRST baseline: pass `baseline_established`"
                    ),
                });
            }
            _ => {}
        }

        let event_id = match disposition {
            DriftDisposition::DesignHolds { change_type } => {
                let event_id = format!(
                    "chg:accept-{:016x}",
                    crate::nodes::fnv1a(&format!("{artifact_id}|{checksum}"))
                );
                if self.get_node(node::CHANGE_EVENT, &event_id)?.is_none() {
                    self.add_change_event(
                        &event_id,
                        note.unwrap_or(
                            "Accepted a new baseline: the change carries no design meaning",
                        ),
                        change_type,
                    )?;
                    if let Some(at) = at {
                        // The claim is worth more dated. Read-modify-write so the
                        // event keeps its name and type.
                        let ev = self
                            .get_node(node::CHANGE_EVENT, &event_id)?
                            .expect("just created");
                        let mut props = Props::new().set("detected_at", at);
                        for (k, v) in &ev.properties {
                            if k != "detected_at" {
                                props = props.set(k, v.clone());
                            }
                        }
                        self.create_node(node::CHANGE_EVENT, &event_id, props)?;
                    }
                    self.accept_changed_edge(&event_id, artifact_id)?;
                }
                event_id
            }
            DriftDisposition::DesignUpdated { change_event_id } => {
                if self
                    .get_node(node::CHANGE_EVENT, change_event_id)?
                    .is_none()
                {
                    return Err(DynoError::NodeNotFound {
                        node_type: node::CHANGE_EVENT.to_string(),
                        node_id: change_event_id.to_string(),
                    });
                }
                self.accept_changed_edge(change_event_id, artifact_id)?;
                change_event_id.to_string()
            }
            DriftDisposition::BaselineEstablished => {
                // Keyed the same way as a `design_holds` accept — artifact plus
                // checksum — so re-establishing the same first baseline is
                // idempotent rather than piling up identical claims. The `chg:`
                // prefix differs so the two can never collide on one artifact.
                let event_id = format!(
                    "chg:baseline-{:016x}",
                    crate::nodes::fnv1a(&format!("{artifact_id}|{checksum}"))
                );
                if self.get_node(node::CHANGE_EVENT, &event_id)?.is_none() {
                    self.add_change_event(
                        &event_id,
                        note.unwrap_or(
                            "First baseline recorded: the artifact was registered without a \
                             checksum and nothing was compared",
                        ),
                        ChangeType::BaselineEstablished,
                    )?;
                    if let Some(at) = at {
                        let ev = self
                            .get_node(node::CHANGE_EVENT, &event_id)?
                            .expect("just created");
                        let mut props = Props::new().set("detected_at", at);
                        for (k, v) in &ev.properties {
                            if k != "detected_at" {
                                props = props.set(k, v.clone());
                            }
                        }
                        self.create_node(node::CHANGE_EVENT, &event_id, props)?;
                    }
                    self.accept_changed_edge(&event_id, artifact_id)?;
                }
                event_id
            }
        };

        // The accept answers the open drift on this artifact, so the schema's
        // own lifecycle flag says so: `DriftEvent.resolved` was declared with
        // `default: false` and, until BL-35, nothing ever wrote it — recorded
        // divergences stayed "open" forever no matter what happened next.
        for e in self.incoming(artifact_id, Some(edge::DEPENDS_ON))? {
            let Some(ev) = self.get_node(node::DRIFT_EVENT, &e.from_id)? else {
                continue; // DEPENDS_ON from something that isn't a DriftEvent
            };
            let already = ev
                .properties
                .get("resolved")
                .and_then(dynograph_core::Value::as_bool)
                .unwrap_or(false);
            if already {
                continue;
            }
            let mut props = Props::new().set("resolved", true);
            for (k, v) in &ev.properties {
                if k != "resolved" {
                    props = props.set(k, v.clone());
                }
            }
            self.create_node(node::DRIFT_EVENT, &ev.node_id, props)?;
        }

        let mut props = Props::new().set("checksum", checksum.as_str());
        for (k, v) in &existing.properties {
            if k != "checksum" {
                props = props.set(k, v.clone());
            }
        }
        let artifact = self.create_node(node::ARTIFACT, artifact_id, props)?;
        Ok((artifact, event_id))
    }

    /// The `CHANGED` edge an accept writes: marked `accepted_baseline: true`,
    /// which is how the confirmation ledger tells a disposition claim from
    /// ordinary change history on the same artifact.
    fn accept_changed_edge(&mut self, event_id: &str, artifact_id: &str) -> Result<(), DynoError> {
        self.create_edge(
            edge::CHANGED,
            node::CHANGE_EVENT,
            event_id,
            node::ARTIFACT,
            artifact_id,
            Props::new()
                .set("action", ChangeAction::Modified.as_str())
                .set("accepted_baseline", true),
        )?;
        Ok(())
    }

    /// Link an `Artifact` to the entity it implements via `REALIZES`. `target_type`
    /// is required because `REALIZES` accepts any target type (`to: "*"`).
    pub fn realizes(
        &mut self,
        artifact_id: &str,
        target_type: &str,
        target_id: &str,
        completeness: Option<&str>,
    ) -> Result<StoredEdge, DynoError> {
        self.create_edge(
            edge::REALIZES,
            node::ARTIFACT,
            artifact_id,
            target_type,
            target_id,
            Props::new().set_opt("completeness", completeness),
        )
    }

    /// Link an `Artifact` to the entity it *describes* via `DOCUMENTS` — a
    /// design doc, ADR, README, instruction file or diagram that explains a
    /// node without implementing it (that would be `REALIZES`) and without
    /// being its machine-readable contract (that would be `SPECIFIES`).
    ///
    /// This is BL-26's criterion made recordable: a file belongs in the graph
    /// when something would be *wrong* if it drifted out of step with the
    /// design — two instruction files disagreeing about the build command is a
    /// coherence failure, and it went uncaught because neither file was in any
    /// graph. Not every file: modelling all 22 source files of a crate was 88%
    /// of a gap list once (BL-23), and capturing everything is how a list gets
    /// skimmed.
    ///
    /// `target_type` is required because `DOCUMENTS` accepts any target
    /// (`to: "*"`). `doc_kind` names what kind of document — `design_doc` /
    /// `adr` / `readme` / `runbook` / `agent_instructions` / a diagram kind.
    /// Both endpoints must already exist: the storage engine accepts a dangling
    /// edge without complaint, so failing loud here is the only check there is.
    /// PROPAGATE deliberately does not traverse `DOCUMENTS` yet — whether a
    /// change should ripple to every doc that mentions it is BL-26's open
    /// decision, not an oversight.
    pub fn documents(
        &mut self,
        artifact_id: &str,
        target_type: &str,
        target_id: &str,
        doc_kind: Option<&str>,
    ) -> Result<StoredEdge, DynoError> {
        if self.get_node(node::ARTIFACT, artifact_id)?.is_none() {
            return Err(DynoError::NodeNotFound {
                node_type: node::ARTIFACT.to_string(),
                node_id: artifact_id.to_string(),
            });
        }
        if self.get_node(target_type, target_id)?.is_none() {
            return Err(DynoError::NodeNotFound {
                node_type: target_type.to_string(),
                node_id: target_id.to_string(),
            });
        }
        self.create_edge(
            edge::DOCUMENTS,
            node::ARTIFACT,
            artifact_id,
            target_type,
            target_id,
            Props::new().set_opt("doc_kind", doc_kind),
        )
    }

    /// Register a real file against the design **with provenance**, atomically:
    /// create the Artifact + a provenance Fragment that `YIELDED` it + the
    /// `REALIZES` edge to its target.
    ///
    /// Fails loud if the target node does not exist — no dangling REALIZES edge.
    pub fn link_artifact(&mut self, opts: LinkArtifactOptions) -> Result<ArtifactLink, DynoError> {
        // The target must exist — never author an edge into thin air.
        if self.get_node(&opts.target_type, &opts.target_id)?.is_none() {
            return Err(DynoError::NodeNotFound {
                node_type: opts.target_type.clone(),
                node_id: opts.target_id.clone(),
            });
        }

        let provenance = opts.provenance.as_deref().unwrap_or("authored");
        let completeness = opts.completeness.as_deref().unwrap_or("complete");
        let fragment_id = opts
            .fragment_id
            .clone()
            .unwrap_or_else(|| format!("frag:{}", opts.artifact_id));

        // All four writes land together or not at all — a failed one (e.g. a bad
        // enum value) leaves no half-linked Artifact behind.
        self.begin_batch();
        match self.write_artifact_link(&opts, &fragment_id, provenance, completeness) {
            Ok(()) => {
                self.commit_batch()?;
                Ok(ArtifactLink {
                    artifact_id: opts.artifact_id,
                    target_id: opts.target_id,
                    fragment_id,
                    completeness: completeness.to_string(),
                    provenance: provenance.to_string(),
                })
            }
            Err(e) => {
                self.discard_batch();
                Err(e)
            }
        }
    }

    /// The mutation half of [`link_artifact`](Self::link_artifact): Artifact +
    /// provenance Fragment + `YIELDED` + `REALIZES`. Run inside a batch so it's atomic.
    fn write_artifact_link(
        &mut self,
        opts: &LinkArtifactOptions,
        fragment_id: &str,
        provenance: &str,
        completeness: &str,
    ) -> Result<(), DynoError> {
        // Provenance Fragment (invalid provenance fails loud via schema validation).
        //
        // `upsert_node`, not `create_node`: re-registering a file must not erase
        // what the design already knows about it. `create_node` is
        // create-or-REPLACE and re-materializes schema defaults over every
        // property the caller omits, and this call site names only four of
        // Artifact's nine. So a re-link silently dropped `last_confirmed_at` —
        // the dated evidence that someone actually checked the file against
        // reality, which is the whole distinction `reconcile_artifacts`'
        // `record_events` exists to draw between a clean sweep and no sweep at
        // all. `status` merely LOOKED safe, because its default (`realized`)
        // happened to equal the stored value; an Artifact at `verified` was
        // being silently downgraded on every re-link.
        //
        // Found doing BL-165, with the evidence sitting in the committed export:
        // of the 34 artifacts `tools/build_design_graph.py` re-links on every
        // run, ZERO carried a `last_confirmed_at`, while the only two in the
        // whole design that did were the two registered by hand and never
        // re-linked since. This is BL-46 (a partial edit silently resetting a
        // verified capability to `planned`) reappearing at a different call
        // site, which is why `upsert_node` was written — and `set_artifact_
        // checksum` below hand-rolls the same merge rather than calling it.
        self.upsert_node(
            node::FRAGMENT,
            fragment_id,
            Props::new()
                .set("title", format!("Registered {}", opts.name))
                .set("fragment_type", "implementation")
                .set("provenance", provenance),
        )?;
        // The Artifact itself.
        self.upsert_node(
            node::ARTIFACT,
            &opts.artifact_id,
            Props::new()
                .set("name", opts.name.as_str())
                .set_opt("artifact_type", opts.artifact_type.as_deref())
                .set_opt("location", opts.location.as_deref())
                .set_opt("checksum", opts.checksum.as_deref().map(canonical_checksum)),
        )?;
        // Fragment YIELDED the Artifact (the provenance anchor).
        self.create_edge(
            edge::YIELDED,
            node::FRAGMENT,
            fragment_id,
            node::ARTIFACT,
            &opts.artifact_id,
            Props::new().set("action", "created"),
        )?;
        // Artifact REALIZES its target.
        self.realizes(
            &opts.artifact_id,
            &opts.target_type,
            &opts.target_id,
            Some(completeness),
        )?;
        Ok(())
    }

    /// Declare what an Artifact node stands for and how its content behaves —
    /// `granularity` (BL-188) and `volatility` (BL-191).
    ///
    /// Both are statements only the author can make. No amount of looking at the
    /// tree distinguishes a settled archive from an untouched backlog, and no
    /// amount of hashing distinguishes a log that grew from a source file that
    /// was edited — which is why these are recorded rather than inferred.
    ///
    /// A separate setter rather than arguments on `add_artifact`, for the reason
    /// BL-183 made expensive: a constructor that takes a partial property set
    /// and writes the whole node erases everything the caller did not name.
    /// Passing `None` here changes nothing, and every other property is
    /// preserved.
    pub fn set_artifact_intent(
        &mut self,
        artifact_id: &str,
        granularity: Option<&str>,
        volatility: Option<&str>,
    ) -> Result<StoredNode, DynoError> {
        const GRANULARITIES: [&str; 3] = ["atomic", "opaque", "pending_expansion"];
        const VOLATILITIES: [&str; 3] = ["stable", "append_only", "living"];

        // Reject with the legal values named. An enum rejection that does not
        // list what IS allowed costs a round-trip to `describe_schema` and is
        // the single cheapest fix on the friction list (BL-192).
        if let Some(g) = granularity
            && !GRANULARITIES.contains(&g)
        {
            return Err(DynoError::Validation {
                node_type: node::ARTIFACT.into(),
                property: "granularity".into(),
                message: format!(
                    "'{g}' is not an Artifact granularity (one of {}). `atomic` is one \
                     deliverable; `opaque` claims a subtree ON PURPOSE (a settled archive, \
                     a vendored tree); `pending_expansion` is a placeholder for items that \
                     should each become their own node.",
                    GRANULARITIES.join(", ")
                ),
            });
        }
        if let Some(v) = volatility
            && !VOLATILITIES.contains(&v)
        {
            return Err(DynoError::Validation {
                node_type: node::ARTIFACT.into(),
                property: "volatility".into(),
                message: format!(
                    "'{v}' is not an Artifact volatility (one of {}). `stable` means any \
                     content change is drift; `append_only` and `living` mean a content \
                     change is expected and is reported as `expected_change` rather than \
                     recorded — absence still fires either way.",
                    VOLATILITIES.join(", ")
                ),
            });
        }

        let Some(existing) = self.get_node(node::ARTIFACT, artifact_id)? else {
            return Err(DynoError::NodeNotFound {
                node_type: node::ARTIFACT.into(),
                node_id: artifact_id.into(),
            });
        };
        let mut props = Props::new();
        for (k, v) in &existing.properties {
            props = props.set(k, v.clone());
        }
        if let Some(g) = granularity {
            props = props.set("granularity", g);
        }
        if let Some(v) = volatility {
            props = props.set("volatility", v);
        }
        self.create_node(node::ARTIFACT, artifact_id, props)
    }
}
