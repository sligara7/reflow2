//! CORPUS — a folder of documents becomes one design ([BL-186], `cap:corpus-ingest`).
//!
//! The last piece of `req:corpus-ingest`. Single-document extraction already
//! works ([`ingest`](crate::ingest)); what a corpus needs and one file does not
//! is a **run**: one epoch for the whole folder, one provenance `Fragment` per
//! document, the ambiguous merge band gathered into ONE question instead of
//! hundreds, and a report that names every document it could not read.
//!
//! **reflow2 performs NO FILE I/O, and that is doctrine rather than a gap**
//! (`dec:agent-navigates-content`). The *agent* walks the directory and hands
//! over each document's text; the graph records what came of it and where it
//! came from. So "folder driver" means the run, not the walk — this module never
//! sees a path it did not receive as a string.
//!
//! ## Why the handshake batches, and why that is nearly free
//!
//! [`ingest_step`](DesignGraph::ingest_step) is a three-round handshake per
//! document. Run naively over 1,124 documents that is ~3,400 agent round trips.
//! This driver collects the outstanding prompts for **every** document in one
//! round, so a corpus costs the same ~3 rounds a single document does. The
//! document text still passes through the agent's context once per prompt — that
//! cost is irreducible and is not claimed to be solved here — but the round trips
//! collapse from `3N` to `3`.
//!
//! The mechanism that makes this safe is already in [`agent`](crate::agent) and
//! needed no change: a prompt's id is an FNV-1a hash of its *semantic content*,
//! so prompts from different documents are naturally distinct and one shared
//! answer pool cannot cross-feed them. It also means two byte-identical
//! documents in a corpus — boilerplate headers, a template filled in twice —
//! produce one prompt and are answered once.
//!
//! ## Resume is derived, never bookmarked
//!
//! There is no cursor and no progress file. A document whose `fragment_id`
//! already exists is [`Skipped`](DocumentStatus::Skipped), because `ingest`
//! refuses to reuse a Fragment ([BL-58] — it would reopen that run's epoch and
//! overwrite its snapshots). So **re-running a corpus is idempotent and picks up
//! where it stopped**, and "what is left" is computed by asking the graph rather
//! than by trusting a note someone wrote. That is the same shape
//! `coverage_report` uses for unclaimed regions.
//!
//! Skipped and failed are deliberately different states: one means *already
//! done*, the other means *could not be read*. Collapsing them is how a corpus
//! run that understood half its input reports the same thing as one that
//! understood all of it.

use std::collections::HashSet;

use dynograph_core::DynoError;

use crate::agent::{AgentAnswer, AgentPrompt, PartialBackend};
use crate::graph::DesignGraph;
use crate::ingest::{IngestOptions, IngestReport, IngestStatus, MergeCandidate};
use crate::nodes::node;
use crate::temporal::ChangeType;

/// One document handed to a corpus run.
///
/// The agent supplies `text` because reflow2 does no file I/O; `source` is an
/// opaque locator it may set to whatever suits the medium — a path, a URL, a
/// page number — and which reflow2 stores and never parses
/// (`dec:agent-navigates-content`).
#[derive(Debug, Clone)]
pub struct CorpusDocument {
    /// Provenance `Fragment` id for this document. Must be unique within the
    /// run and must not already exist in the graph.
    pub fragment_id: String,
    /// Human title — normally the file name.
    pub title: String,
    /// The document's text.
    pub text: String,
    /// Opaque locator back to the source, stored verbatim. `None` when the
    /// caller has nothing better than the title.
    pub source: Option<String>,
}

/// Options for a whole corpus run.
#[derive(Debug, Clone)]
pub struct CorpusOptions {
    /// The ONE epoch every document in this run pins to. Required, and that is
    /// the point: left to itself `ingest` opens `epoch:{fragment_id}` per
    /// document, so 500 documents would open 500 epochs and the history would
    /// read as five hundred unrelated events instead of one ingest.
    pub epoch_id: String,
    /// How this content entered the graph. A corpus of documents somebody else
    /// wrote is `imported`; the caller decides, because only the caller knows.
    pub provenance: String,
    /// The change type recorded for every matched-evolved node in this run.
    pub change_type: ChangeType,
}

impl Default for CorpusOptions {
    fn default() -> Self {
        Self {
            epoch_id: "epoch:corpus-ingest".to_string(),
            provenance: "imported".to_string(),
            change_type: ChangeType::ScopeChange,
        }
    }
}

/// What became of one document in the run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentStatus {
    /// Extracted and integrated.
    Ingested,
    /// Its `Fragment` already exists, so a previous run covered it. Not an
    /// error — this is what makes a re-run resumable.
    Skipped,
    /// Could not be ingested. `error` says why, and the run continued.
    Failed,
}

/// One document's outcome, always reported — including the ones that failed,
/// because a corpus run that quietly drops what it could not read is exactly the
/// false completeness this feature exists to avoid.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DocumentOutcome {
    /// The document's provenance `Fragment` id.
    pub fragment_id: String,
    /// Its title, echoed so a reader need not join back to the input.
    pub title: String,
    /// The opaque source locator, if one was given.
    pub source: Option<String>,
    /// Ingested / skipped / failed.
    pub status: DocumentStatus,
    /// Nodes created from this document (0 unless `Ingested`).
    pub nodes_created: usize,
    /// Existing nodes this document changed.
    pub nodes_evolved: usize,
    /// Why it failed, when it did.
    pub error: Option<String>,
}

/// The outcome of a corpus run.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CorpusReport {
    /// The epoch every document pinned to.
    pub epoch_id: String,
    /// How many documents were handed in.
    pub documents_total: usize,
    /// How many were extracted and integrated this run.
    pub documents_ingested: usize,
    /// How many a previous run had already covered.
    pub documents_skipped: usize,
    /// How many could not be read. Never silently zero — see `failures`.
    pub documents_failed: usize,
    /// Nodes genuinely new across the run (includes one `Fragment` per document).
    pub nodes_created: usize,
    /// Existing nodes whose content changed — snapshotted, never overwritten.
    pub nodes_evolved: usize,
    /// Existing nodes matched with identical content.
    pub nodes_unchanged: usize,
    /// Edges created across the run.
    pub edges_created: usize,
    /// Cross-document convergences: how many times a name in one document
    /// resolved onto a node an earlier document created, instead of duplicating
    /// it. **This is the number that says whether the corpus became ONE design**
    /// rather than N disconnected ones.
    pub fuzzy_merges: usize,
    /// The ambiguous band, gathered across the WHOLE corpus and deduplicated —
    /// one question to put to a person, not one per document
    /// (`dec:ask-not-repair`, and the batching its own text demands at this
    /// scale).
    pub merge_candidates: Vec<MergeCandidate>,
    /// How many suspicions were persisted as `DUPLICATES` edges, so HEAL can
    /// collect the same question later and in any order.
    pub duplicates_recorded: usize,
    /// Every document that failed, named. The list `req:corpus-ingest` is really
    /// about: a run that cannot say what it did not understand should not ship.
    pub failures: Vec<DocumentOutcome>,
    /// Every document's outcome, in the order handed in.
    pub outcomes: Vec<DocumentOutcome>,
    /// `Partial` if any document failed or any document's own ingest degraded.
    pub status: IngestStatus,
}

/// One turn of the corpus handshake — the batched sibling of
/// [`IngestStep`](crate::ingest::IngestStep).
#[derive(Debug, serde::Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CorpusStep {
    /// Prompts across every document that still needs one. Answer them all and
    /// call again with the SAME documents and options plus every answer so far —
    /// the run replays from the top rather than resuming, which is what keeps it
    /// stateless across restarts and seats.
    NeedsLlm {
        /// Newly reachable prompts, deduplicated across documents.
        prompts: Vec<AgentPrompt>,
        /// How many documents contributed to this round, so the caller can see
        /// the batching working rather than infer it.
        documents_pending: usize,
        /// Answers no pass asked for — reported, never ignored, because a
        /// leftover answer usually means the input moved under the handshake.
        unused_answers: Vec<String>,
    },
    /// Every prompt was answered and the corpus was written.
    Done { report: Box<CorpusReport> },
}

/// A document rejected before any work began.
struct Rejection {
    outcome: DocumentOutcome,
}

impl DesignGraph {
    /// Ingest a whole corpus as one run ([BL-186]).
    ///
    /// Drive it exactly like [`ingest_step`](DesignGraph::ingest_step): call with
    /// no answers, answer everything that comes back, call again with the same
    /// documents and every answer so far. Repeat until [`CorpusStep::Done`].
    ///
    /// **Nothing is written until the final round.** The prepare rounds replay
    /// each document against a throwaway in-memory graph, so an abandoned corpus
    /// leaves no half-design behind — the same guarantee the single-document
    /// handshake makes, held across the whole folder.
    ///
    /// Documents are integrated **in the order given**, and that order matters:
    /// convergence resolves each document against what the graph already holds,
    /// so document 40's "Auth Service" lands on document 1's node. The merged
    /// name is settled from the two strings alone, so the result does not depend
    /// on which document arrived first.
    pub fn ingest_corpus_step(
        &mut self,
        documents: &[CorpusDocument],
        options: &CorpusOptions,
        answers: Vec<AgentAnswer>,
    ) -> Result<CorpusStep, DynoError> {
        let (pending, rejections) = self.triage(documents)?;

        // Prepare: replay every pending document against its own scratch graph,
        // sharing ONE answer pool. Prompt ids are content hashes, so the pool
        // cannot cross-feed documents and identical documents cost one prompt.
        let mut prompts: Vec<AgentPrompt> = Vec::new();
        let mut seen_prompts: HashSet<String> = HashSet::new();
        let mut documents_pending = 0usize;
        let mut unused: Vec<String> = Vec::new();
        let mut unused_seen: HashSet<String> = HashSet::new();

        for doc in &pending {
            let probe = PartialBackend::new(answers.clone());
            let mut scratch = DesignGraph::open_in_memory()?;
            // The scratch result is discarded; only what it ASKED matters. Its
            // errors are the expected consequence of stubbed answers.
            let _ = scratch.ingest(&doc.text, &ingest_options(doc, options), &probe);

            let outstanding = probe.outstanding();
            if !outstanding.is_empty() {
                documents_pending += 1;
            }
            for prompt in outstanding {
                if seen_prompts.insert(prompt.id.clone()) {
                    prompts.push(prompt);
                }
            }
            // An answer is "unused" only if NO document wanted it. Collect per
            // document and filter at the end, or every document would report
            // every other document's answers as stale.
            for id in probe.unused_answers() {
                if unused_seen.insert(id.clone()) {
                    unused.push(id);
                }
            }
        }

        if !prompts.is_empty() {
            let wanted: HashSet<&str> = prompts.iter().map(|p| p.id.as_str()).collect();
            unused.retain(|id| !wanted.contains(id.as_str()));
            return Ok(CorpusStep::NeedsLlm {
                prompts,
                documents_pending,
                unused_answers: unused,
            });
        }

        Ok(CorpusStep::Done {
            report: Box::new(self.run_corpus(&pending, rejections, options, &answers)),
        })
    }

    /// Split the input into documents worth running and ones rejected up front,
    /// before anything is written. Two rejections, both refused here rather than
    /// discovered mid-run: a `fragment_id` repeated **within** the batch (which
    /// would make the second document overwrite the first's provenance), and one
    /// that already exists in the graph (a previous run covered it).
    fn triage(
        &self,
        documents: &[CorpusDocument],
    ) -> Result<(Vec<CorpusDocument>, Vec<Rejection>), DynoError> {
        let mut pending = Vec::new();
        let mut rejections = Vec::new();
        let mut within_batch: HashSet<&str> = HashSet::new();

        for doc in documents {
            if !within_batch.insert(doc.fragment_id.as_str()) {
                rejections.push(Rejection {
                    outcome: outcome(
                        doc,
                        DocumentStatus::Failed,
                        Some(format!(
                            "duplicate fragment_id '{}' within this batch — each document \
                             needs its own, or the second would overwrite the first's \
                             provenance Fragment and reopen its epoch",
                            doc.fragment_id
                        )),
                    ),
                });
                continue;
            }
            if self.get_node(node::FRAGMENT, &doc.fragment_id)?.is_some() {
                rejections.push(Rejection {
                    outcome: outcome(doc, DocumentStatus::Skipped, None),
                });
                continue;
            }
            pending.push(doc.clone());
        }
        Ok((pending, rejections))
    }

    /// Serve: integrate every pending document against the real graph, in order.
    /// A document that fails is recorded and the run continues — never
    /// cascade-fail, the same discipline `ingest` holds across its passes, held
    /// here across documents.
    fn run_corpus(
        &mut self,
        pending: &[CorpusDocument],
        rejections: Vec<Rejection>,
        options: &CorpusOptions,
        answers: &[AgentAnswer],
    ) -> CorpusReport {
        let mut agg = Aggregate::new(rejections);

        for doc in pending {
            let backend = crate::agent::AgentBackend::from_answers(answers.to_vec());
            match self.ingest(&doc.text, &ingest_options(doc, options), &backend) {
                Ok(report) => agg.accept(doc, report),
                Err(e) => agg.reject(doc, e.to_string()),
            }
        }

        agg.finish(options, pending.len())
    }
}

/// One document's [`IngestOptions`], carrying the run's shared epoch.
fn ingest_options(doc: &CorpusDocument, options: &CorpusOptions) -> IngestOptions {
    IngestOptions {
        fragment_id: doc.fragment_id.clone(),
        fragment_title: doc.title.clone(),
        provenance: options.provenance.clone(),
        epoch_id: Some(options.epoch_id.clone()),
        change_type: options.change_type,
    }
}

fn outcome(doc: &CorpusDocument, status: DocumentStatus, error: Option<String>) -> DocumentOutcome {
    DocumentOutcome {
        fragment_id: doc.fragment_id.clone(),
        title: doc.title.clone(),
        source: doc.source.clone(),
        status,
        nodes_created: 0,
        nodes_evolved: 0,
        error,
    }
}

/// Running totals for a corpus run.
struct Aggregate {
    outcomes: Vec<DocumentOutcome>,
    nodes_created: usize,
    nodes_evolved: usize,
    nodes_unchanged: usize,
    edges_created: usize,
    fuzzy_merges: usize,
    duplicates_recorded: usize,
    candidates: Vec<MergeCandidate>,
    candidate_keys: HashSet<(String, String)>,
    degraded: bool,
}

impl Aggregate {
    fn new(rejections: Vec<Rejection>) -> Self {
        let outcomes: Vec<DocumentOutcome> = rejections.into_iter().map(|r| r.outcome).collect();
        let degraded = outcomes.iter().any(|o| o.status == DocumentStatus::Failed);
        Self {
            outcomes,
            nodes_created: 0,
            nodes_evolved: 0,
            nodes_unchanged: 0,
            edges_created: 0,
            fuzzy_merges: 0,
            duplicates_recorded: 0,
            candidates: Vec::new(),
            candidate_keys: HashSet::new(),
            degraded,
        }
    }

    fn accept(&mut self, doc: &CorpusDocument, report: IngestReport) {
        self.nodes_created += report.nodes_created;
        self.nodes_evolved += report.nodes_evolved;
        self.nodes_unchanged += report.nodes_unchanged;
        self.edges_created += report.edges_created;
        self.fuzzy_merges += report.fuzzy_merges.len();
        self.duplicates_recorded += report.duplicates_recorded;
        if report.status == IngestStatus::Partial {
            self.degraded = true;
        }
        // Deduplicate the ambiguous band across the corpus. The same pair
        // surfacing in six documents is ONE question, and asking it six times is
        // how a correct feature becomes an unusable one.
        for candidate in report.merge_candidates {
            let key = (
                candidate.extracted_id.clone(),
                candidate.candidate_id.clone(),
            );
            if self.candidate_keys.insert(key) {
                self.candidates.push(candidate);
            }
        }
        let mut out = outcome(doc, DocumentStatus::Ingested, None);
        out.nodes_created = report.nodes_created;
        out.nodes_evolved = report.nodes_evolved;
        self.outcomes.push(out);
    }

    fn reject(&mut self, doc: &CorpusDocument, error: String) {
        self.degraded = true;
        self.outcomes
            .push(outcome(doc, DocumentStatus::Failed, Some(error)));
    }

    fn finish(self, options: &CorpusOptions, _pending: usize) -> CorpusReport {
        let count = |s: DocumentStatus| self.outcomes.iter().filter(|o| o.status == s).count();
        let failures: Vec<DocumentOutcome> = self
            .outcomes
            .iter()
            .filter(|o| o.status == DocumentStatus::Failed)
            .cloned()
            .collect();

        CorpusReport {
            epoch_id: options.epoch_id.clone(),
            documents_total: self.outcomes.len(),
            documents_ingested: count(DocumentStatus::Ingested),
            documents_skipped: count(DocumentStatus::Skipped),
            documents_failed: failures.len(),
            nodes_created: self.nodes_created,
            nodes_evolved: self.nodes_evolved,
            nodes_unchanged: self.nodes_unchanged,
            edges_created: self.edges_created,
            fuzzy_merges: self.fuzzy_merges,
            merge_candidates: self.candidates,
            duplicates_recorded: self.duplicates_recorded,
            failures,
            outcomes: self.outcomes,
            status: if self.degraded {
                IngestStatus::Partial
            } else {
                IngestStatus::Ok
            },
        }
    }
}
