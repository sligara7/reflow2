#!/usr/bin/env python3
"""Build reflow2's own functional design graph, and analyse it with reflow2.

This is dogfooding in the strong sense. Every other trial in `tools/` builds a
throwaway graph to *test* reflow2; this one builds the real, durable design of
reflow2 itself and then turns reflow2's whole analysis surface on it —
detect_gaps, detect_defects, possible_duplicate, hierarchy_issues,
surprising_connections, evaluate_allocation.

The graph is committed as a deterministic export (`docs/design/reflow2.json`)
rather than as a RocksDB directory: exports are sorted and byte-identical for an
unchanged design, so the design becomes reviewable and diffable in git, and any
working copy is `import_graph` away. `.reflow2/` is gitignored, so the export is
the durable artifact — the RocksDB store is a local cache of it.

Granularity is deliberately coarse (BL-23): one Component per module, one
Artifact per module, one Verification per test file — never one Artifact per
source file, which made 22 of 25 gaps noise the last time this repo was modelled.

Run:  python3 tools/build_design_graph.py [--analyse-only]
"""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from smoke_mcp import Server  # noqa: E402

REPO = pathlib.Path(__file__).resolve().parent.parent
EXPORT = REPO / "docs/design/reflow2.json"

# ---- P0 · Intent ----------------------------------------------------------
REQUIREMENTS = {
    "req:coherence": ("Design stays coherent across its lifecycle",
                      "When anything changes in any phase, the ripples are found and surfaced.",
                      "critical"),
    "req:released-eq-designed": ("Released equals designed",
                                 "At release the design must describe what was actually released.",
                                 "critical"),
    "req:no-silent-fallback": ("No silent fallbacks or drops",
                               "Failures and skipped work are surfaced loudly, never swallowed.",
                               "critical"),
    "req:golden-thread": ("Every artifact traces to the intent it serves",
                          "Traceability runs from concept through operations without a break.",
                          "high"),
    "req:no-se-knowledge": ("The user never needs systems engineering",
                            "The graph carries the discipline; the user answers plain questions.",
                            "high"),
    "req:design-anything": ("Design anything, not only software",
                            "Software, hardware, a document, a process — the vocabulary stays neutral.",
                            "high"),
    "req:agent-native": ("An agent can drive the whole loop",
                         "Every capability is reachable from a coding agent over one surface.",
                         "high"),
    "req:survives-upgrade": ("A design survives a reflow2 upgrade",
                             "An existing graph opens, or is refused loudly with what to do.",
                             "high"),
    "req:intent-preserved": ("The past is never overwritten",
                             "Updating the design to match the build must not erase what was intended.",
                             "high"),
    "req:adopt-existing": ("A system that already exists can be brought under design control",
                           "Requirements, functions and structure can be recovered from a running system.",
                           "medium"),
}

# ---- P1 · Function. The coherence loop, plus what serves it. --------------
# (id, name, description, status, satisfies[])
CAPABILITIES = [
    ("cap:change", "Record a change", "Snapshot prior state at an epoch, then edit — axis Z.",
     "verified", ["req:coherence", "req:intent-preserved"]),
    ("cap:propagate", "Propagate impact", "Walk the golden thread to compute a blast radius.",
     "verified", ["req:coherence", "req:golden-thread"]),
    ("cap:detect", "Detect gaps", "Find what the design has not decided yet, ranked.",
     "verified", ["req:coherence", "req:no-se-knowledge"]),
    ("cap:surface", "Ask the user a plain question", "Turn a gap into a question in the user's words.",
     "verified", ["req:no-se-knowledge"]),
    ("cap:heal", "Repair structure", "Propose content-free structural fixes, then apply atomically.",
     "verified", ["req:coherence", "req:no-silent-fallback"]),
    ("cap:reconcile-built", "Reconcile against what was built",
     "Compare registered artifacts against observed files and report divergence.",
     "verified", ["req:released-eq-designed", "req:golden-thread"]),
    # Both were `planned` until 2026-07-20, when the self-adopt run found them
    # shipped, MCP-exposed and tested — 15 of the 16 gaps that model produced
    # were pointing at the model, not the system. Ruled per sharpening.md §2.
    ("cap:reconcile-verified", "Reconcile against what was proven",
     "Compare recorded verification status against a real test run.",
     "verified", ["req:released-eq-designed"]),
    ("cap:reconcile-deployed", "Reconcile against what is running",
     "Compare the design against what is actually deployed.",
     "verified", ["req:released-eq-designed"]),
    ("cap:budget", "Roll up a budget against a constraint",
     "Sum contributions along the thread; refuse a total it cannot honestly compute.",
     "verified", ["req:no-silent-fallback", "req:golden-thread"]),
    ("cap:genesis", "Bootstrap a design from a brief",
     "Seed a project, its requirements and capabilities, refusing to clobber an existing graph.",
     "verified", ["req:no-se-knowledge"]),
    ("cap:vocabulary", "Describe the vocabulary", "Tell a client what types exist and what may join them.",
     "verified", ["req:agent-native", "req:no-silent-fallback"]),
    # 2026-07-20: the schema carried `fulltext:` flags from day one and the
    # foundation implemented the index; nothing enabled or served it —
    # recurring-lesson instance #17. Finding by content is the graph's job
    # (partnership.md), not the LLM's.
    ("cap:search", "Find design nodes by what they say",
     "BM25 keyword search over every fulltext property, ranked, type-scopable.",
     "verified", ["req:agent-native", "req:no-se-knowledge"]),
    ("cap:portability", "Export and import a design", "Move a design across machines and versions.",
     "verified", ["req:survives-upgrade"]),
    ("cap:stamp", "Say which reflow2 wrote a graph", "Stamp and check, refusing a graph from the future.",
     "verified", ["req:survives-upgrade"]),
    ("cap:allocate", "Analyse and propose allocation", "Score function-to-structure allocation; cluster coupling.",
     "verified", ["req:golden-thread"]),
    ("cap:hierarchy", "Check decomposition levels", "Find missing intermediates and level mismatches — axis Y.",
     "verified", ["req:coherence"]),
    # Both moved realized -> verified 2026-07-20 (user-directed sweep): their
    # suites were run live (dimensions 6/6, ingest 16/16) with VERIFIES edges
    # already in place.
    ("cap:dimensions", "Track quality over time", "Assess nodes on dimensions and detect decline.",
     "verified", ["req:coherence"]),
    ("cap:ingest", "Extract a design from freeform text", "Multi-pass LLM extraction with provenance.",
     "verified", ["req:no-se-knowledge"]),
    ("cap:questions", "Remember what was asked", "Questions outlive the session; answers stay visible.",
     "verified", ["req:no-se-knowledge"]),
    ("cap:report", "Say where the design stands", "One rollup answering 'what should I look at?'.",
     "verified", ["req:no-se-knowledge"]),
    ("cap:mcp-surface", "Serve the loop over MCP", "Every capability as a typed tool for an agent.",
     "verified", ["req:agent-native"]),
    # `realized` from 2026-07-19 (status_contradiction caught the original
    # `verified` claim — a lie in our own committed model) until 2026-07-20,
    # when tools/test_init.py gave the claim something real to stand on.
    ("cap:kit", "Install into a consumer project", "One command sets up or refreshes the design environment.",
     "verified", ["req:agent-native"]),
    # `realized`, not `verified`: the skill exists and was exercised once (the
    # 2026-07-20 storyflow trial), but nothing automated checks it.
    ("cap:adopt", "Recover a design from an existing system",
     "Read requirements, functions and structure back out of a running system.",
     "realized", ["req:adopt-existing"]),
    ("cap:model-process", "Model a process, not only a product",
     "Represent an ordered flow of activities with roles on its edges.",
     "verified", ["req:design-anything"]),
    ("cap:freshness", "Say when a claim was last confirmed",
     "Distinguish a design that matches reality from one nobody has checked.",
     "verified", ["req:released-eq-designed", "req:coherence"]),
    # BL-71 rung c (2026-07-21): the reconcile family's design-vs-design
    # sibling — the three reconcile tools compare design against reality;
    # nothing compared two as-designed records until the curated rebuild
    # clobbered the accumulated live layer and only a node count noticed.
    ("cap:compare-designs", "Compare two as-designed records",
     "Diff two export documents (or the live graph against one) into added / removed / changed "
     "relative to a named base, banded into design content vs bookkeeping, property-level on "
     "changes.",
     "verified", ["req:intent-preserved"]),
]

# ---- Decisions. The distillate of the sessions that shaped the design. ----
#
# The graph is the distillate, not the tape: a session transcript is an
# artifact outside the graph (every commit carries its Claude-Session URL, so
# the raw context is one link from git log). What belongs IN the graph is what
# was decided and why — where-am-i's "what's settled" reads exactly this, and
# until 2026-07-19 it had nothing to read.
SESSION = "https://claude.ai/code/session_013aumVdHrRHu24cLirn6Cam"
DECISIONS = [
    ("dec:ask-not-repair", "Suspected duplicates are asked, never merged",
     "possible_duplicate is a DETECT gap; HEAL merges only on a human-drawn DUPLICATES edge.",
     "apply_heal deletes a node with no undo, so merge is safe only because the endpoints were "
     "asserted; a heuristic must not drive it. A gap can be acknowledged, a HEAL defect cannot "
     f"be dismissed. Decided 2026-07-19 ({SESSION}); evidence: 3dtictactoe trial, BL-27/BL-29.",
     ["cap:detect", "cap:heal"]),
    ("dec:anchored-first", "A gap that names nodes outranks a phase nudge",
     "detect_gaps sorts anchored gaps before project-level phase nudges, severity within each band.",
     "A named gap says something is wrong NOW; a nudge says what comes next. Ranking 'next' above "
     "'broken' made three brownfield trials do the useless thing first. Nudges are demoted, never "
     f"suppressed. Decided 2026-07-19 ({SESSION}); evidence: gap-surfacing disciplines 3 and 8.",
     ["cap:detect"]),
    ("dec:operational-spof", "Only things that operate can be single points of failure",
     "single_point_of_failure candidates are Components, Interfaces, Resources, Environments.",
     "The suggested fix is add_redundancy, and redundancy is only coherent for running parts; a "
     "golden thread converges on intent by design, so intent hubs are the thread working. 22 of "
     f"22 false positives cleared, 4 true survivors. Decided 2026-07-19 ({SESSION}); BL-5.",
     ["cap:heal"]),
    ("dec:two-sided-accept", "Silent drift-accept does not exist",
     "set_artifact_checksum requires a disposition: design_holds (a dated claim) or "
     "design_updated (naming the design-side ChangeEvent, linked to the artifact).",
     "'Accept the file, leave the design alone, say nothing' is how a design erodes into fiction "
     "over N legitimate fix cycles while reporting zero gaps — the failure that sank the original "
     f"reflow. Decided 2026-07-19 ({SESSION}); evidence: erosion trials, BL-33.",
     ["cap:reconcile-built", "cap:change"]),
    ("dec:passing-is-verified", "Verified means a check that passes, not one that exists",
     "verification_coverage counts passing checks; a failing check is its own 0.8 gap.",
     "A failing test used to satisfy the gap that asked for a test, and passing vs failing were "
     "byte-identical to every diagnostic — counting test nodes while ignoring test results. "
     f"Decided 2026-07-19 ({SESSION}); evidence: phase-coverage trial, BL-30.",
     ["cap:detect"]),
    ("dec:report-dont-judge", "The confirmation ledger reports claim history, never judges a claim",
     "Per capability: drifting / confirmed / unexamined, with the disposition counts visible.",
     "Five design_holds claims with zero design edits is the erosion signature and the ledger "
     "makes it legible — but judging a specific claim false is semantic, and a deterministic "
     "detector would fire on every stable design with cosmetic churn (the unexpected_coupling "
     f"lesson). Decided 2026-07-19 ({SESSION}); BL-35.",
     ["cap:freshness"]),
    ("dec:views-are-projections", "A view is a projection of the graph; renderer fill-ins are defects",
     "Renderers may only emit what the graph states; everything else is confessed as a finding.",
     "UAF/DoDAF doctrine from the author: the graph stores all design detail, the agent only "
     "renders viewpoints. If rendering requires extrapolation, something is missing inside "
     f"reflow2 — almost always. Decided 2026-07-19 ({SESSION}); render_views.py, BL-40.",
     ["cap:report"]),
    ("dec:three-party-checks", "The LLM speaks, the graph remembers and counts, the human decides",
     "Every capability is designed as a check one party places on another; none does another's job.",
     "The parties are not interchangeable: the LLM cannot be trusted with arithmetic, memory or "
     "its own confidence; the graph cannot judge meaning; the human cannot remember everything or "
     "notice slow drift. The deterministic core computes, the graph carries claims and their "
     "audit trail, questions route judgment to the human. docs/partnership.md maps each known LLM "
     f"failure mode to its mechanism. Decided 2026-07-19 ({SESSION}).",
     ["cap:detect", "cap:surface", "cap:change"]),
    ("dec:repo-file-embedded", "The graph lives as a repo file, embedded — not a service",
     "RocksDB directory beside the repo, exports as the durable, diffable record.",
     "The service's strongest argument (concurrency) is hypothetical while there is one writer; "
     "it would put the user's design on a machine they do not control and is permanent "
     "operational cost. Reopening conditions are written down. Decided 2026-07-18 "
     "(surface-plan.md); BL-12/BL-15 carry the consequences.",
     ["cap:portability"]),
    ("dec:design-diff-vocabulary",
     "Design-vs-design comparison is directional, banded, and lives in the core",
     "A design-vs-design diff (compare_designs) exists as a core op exposed as both an MCP tool "
     "and a CLI flag. It compares two export documents, or the live graph against one document. "
     "Findings are directional — added / removed / changed relative to an explicitly named base "
     "— with property-level detail on changed nodes, grouped into two bands: design content vs "
     "the supporting/bookkeeping layer (ChangeEvents, DriftEvents, Fragments, Questions). It "
     "reports divergence between two as-designed records and never judges which side is right; "
     "the word 'drift' stays reserved for design-vs-reality.",
     "User decided all four axes 2026-07-21 (BL-71 rung c vocabulary session). Directional "
     "because all three consumers have a real base — the committed record (the BL-71 clobber "
     "tripwire), the main branch (BL-70 branch-by-file comparison), the state a claim was made "
     "against (BL-12 merge) — while AoA can still read the report neutrally or run it both "
     "ways. Banded because the real 2026-07-21 divergence was 3 Decisions and 8 Requirements "
     "buried under ~20 bookkeeping nodes; a flat list hides exactly what matters. Core op (not "
     "a kit script) because deterministic design computation belongs in the tested core; CLI "
     "flag so CI and no-server contexts reach it, like --import/--export. It is the read side "
     "of the rung a+b upsert-layering, the sibling of the reconcile family — which compares "
     "design against reality, hence the separate word: divergence, not drift.",
     ["cap:compare-designs"]),
]

# ---- P2 · Structure. Coarse: crate -> module. -----------------------------
SUBSYSTEMS = [
    ("cmp:core", "reflow2-core", "The deterministic, LLM-free coherence engine.", "subsystem"),
    ("cmp:mcp", "reflow2-mcp", "The agent-facing MCP surface over one graph.", "subsystem"),
    ("cmp:kit", "consumer kit", "What gets installed into a project being designed.", "subsystem"),
]
MODULES = [
    ("cmp:temporal", "temporal", "cmp:core", ["cap:change"]),
    ("cmp:propagate", "propagate", "cmp:core", ["cap:propagate"]),
    ("cmp:detect", "detect", "cmp:core", ["cap:detect", "cap:surface", "cap:questions"]),
    ("cmp:heal", "heal", "cmp:core", ["cap:heal"]),
    ("cmp:structure", "structure", "cmp:core", ["cap:heal"]),
    ("cmp:drift", "drift", "cmp:core", ["cap:reconcile-built"]),
    ("cmp:vocabulary", "vocabulary", "cmp:core", ["cap:vocabulary"]),
    ("cmp:export", "export", "cmp:core", ["cap:portability"]),
    ("cmp:provenance", "provenance", "cmp:core", ["cap:stamp"]),
    ("cmp:allocate", "allocate", "cmp:core", ["cap:allocate"]),
    ("cmp:hierarchy", "hierarchy", "cmp:core", ["cap:hierarchy"]),
    ("cmp:dimensions", "dimensions", "cmp:core", ["cap:dimensions"]),
    ("cmp:ingest", "ingest", "cmp:core", ["cap:ingest"]),
    ("cmp:report", "report", "cmp:core", ["cap:report"]),
    ("cmp:graph", "graph", "cmp:core", ["cap:portability"]),
    ("cmp:verify", "verify", "cmp:core", ["cap:reconcile-verified"]),
    ("cmp:operate", "operate", "cmp:core", []),
    # Added 2026-07-20: the self-adopt run found 15 of 33 source files carried
    # no Component and no Artifact, which is what made five shipped
    # capabilities read as unbuilt.
    ("cmp:confirm", "confirm", "cmp:core", ["cap:freshness"]),
    ("cmp:fielded", "fielded", "cmp:core", ["cap:reconcile-deployed"]),
    ("cmp:flow", "flow", "cmp:core", ["cap:model-process"]),
    ("cmp:artifact", "artifact", "cmp:core", ["cap:reconcile-built"]),
    ("cmp:budget", "budget", "cmp:core", ["cap:budget"]),
    ("cmp:genesis", "genesis", "cmp:core", ["cap:genesis"]),
    ("cmp:llm", "llm", "cmp:core", ["cap:surface"]),
    ("cmp:agent", "agent", "cmp:core", ["cap:surface"]),
    ("cmp:surprises", "surprises", "cmp:core", ["cap:report"]),
    ("cmp:schema", "schema", "cmp:core", ["cap:vocabulary"]),
    ("cmp:search", "search", "cmp:core", ["cap:search"]),
    ("cmp:compare", "compare", "cmp:core", ["cap:compare-designs"]),
    ("cmp:nodes", "nodes", "cmp:core", []),
    ("cmp:service", "service", "cmp:mcp", ["cap:mcp-surface"]),
    ("cmp:main", "main", "cmp:mcp", ["cap:mcp-surface"]),
    ("cmp:dto", "dto", "cmp:mcp", ["cap:mcp-surface"]),
    ("cmp:init", "reflow2_init", "cmp:kit", ["cap:kit"]),
    ("cmp:skills", "skills", "cmp:kit", ["cap:kit"]),
]

# ---- P3/P4 · one Artifact per module, one Verification per test file ------
ARTIFACTS = {  # component -> source path
    "cmp:temporal": "crates/reflow2-core/src/temporal.rs",
    "cmp:propagate": "crates/reflow2-core/src/propagate.rs",
    "cmp:detect": "crates/reflow2-core/src/detect.rs",
    "cmp:heal": "crates/reflow2-core/src/heal.rs",
    "cmp:structure": "crates/reflow2-core/src/structure.rs",
    "cmp:drift": "crates/reflow2-core/src/drift.rs",
    "cmp:vocabulary": "crates/reflow2-core/src/vocabulary.rs",
    "cmp:export": "crates/reflow2-core/src/export.rs",
    "cmp:provenance": "crates/reflow2-core/src/provenance.rs",
    "cmp:allocate": "crates/reflow2-core/src/allocate.rs",
    "cmp:hierarchy": "crates/reflow2-core/src/hierarchy.rs",
    "cmp:dimensions": "crates/reflow2-core/src/dimensions.rs",
    "cmp:ingest": "crates/reflow2-core/src/ingest.rs",
    "cmp:report": "crates/reflow2-core/src/report.rs",
    "cmp:graph": "crates/reflow2-core/src/graph.rs",
    "cmp:verify": "crates/reflow2-core/src/verify.rs",
    "cmp:operate": "crates/reflow2-core/src/operate.rs",
    "cmp:confirm": "crates/reflow2-core/src/confirm.rs",
    "cmp:fielded": "crates/reflow2-core/src/fielded.rs",
    "cmp:flow": "crates/reflow2-core/src/flow.rs",
    "cmp:artifact": "crates/reflow2-core/src/artifact.rs",
    "cmp:budget": "crates/reflow2-core/src/budget.rs",
    "cmp:genesis": "crates/reflow2-core/src/genesis.rs",
    "cmp:llm": "crates/reflow2-core/src/llm.rs",
    "cmp:agent": "crates/reflow2-core/src/agent.rs",
    "cmp:surprises": "crates/reflow2-core/src/surprises.rs",
    "cmp:schema": "crates/reflow2-core/src/schema.rs",
    "cmp:search": "crates/reflow2-core/src/search.rs",
    "cmp:compare": "crates/reflow2-core/src/compare.rs",
    "cmp:nodes": "crates/reflow2-core/src/nodes.rs",
    "cmp:service": "crates/reflow2-mcp/src/service.rs",
    "cmp:main": "crates/reflow2-mcp/src/main.rs",
    "cmp:dto": "crates/reflow2-mcp/src/dto.rs",
    "cmp:init": "tools/reflow2_init.py",
}
VERIFICATIONS = {  # capability -> test file
    "cap:change": "crates/reflow2-core/tests/temporal.rs",
    "cap:propagate": "crates/reflow2-core/tests/propagate.rs",
    "cap:detect": "crates/reflow2-core/tests/detect.rs",
    "cap:heal": "crates/reflow2-core/tests/heal.rs",
    "cap:reconcile-built": "crates/reflow2-core/tests/drift.rs",
    "cap:portability": "crates/reflow2-core/tests/export.rs",
    "cap:stamp": "crates/reflow2-core/tests/provenance.rs",
    "cap:allocate": "crates/reflow2-core/tests/allocate.rs",
    "cap:hierarchy": "crates/reflow2-core/tests/hierarchy.rs",
    "cap:questions": "crates/reflow2-core/tests/gap_review.rs",
    "cap:report": "crates/reflow2-core/tests/report.rs",
    "cap:vocabulary": "crates/reflow2-core/tests/write_side.rs",
    "cap:dimensions": "crates/reflow2-core/tests/dimensions.rs",
    "cap:ingest": "crates/reflow2-core/tests/ingest.rs",
    "cap:surface": "crates/reflow2-core/tests/llm.rs",
    "cap:mcp-surface": "crates/reflow2-mcp/tests/tools.rs",
    # Added 2026-07-20 with the self-adopt corrections: these suites existed
    # and passed all along; the model simply never recorded them.
    "cap:reconcile-verified": "crates/reflow2-core/tests/verify_drift.rs",
    "cap:reconcile-deployed": "crates/reflow2-core/tests/fielded.rs",
    "cap:model-process": "crates/reflow2-core/tests/flow.rs",
    "cap:budget": "crates/reflow2-core/tests/budget.rs",
    "cap:genesis": "crates/reflow2-core/tests/genesis.rs",
    "cap:freshness": "crates/reflow2-core/tests/confirm.rs",
    "cap:kit": "tools/test_init.py",
    "cap:search": "crates/reflow2-core/tests/search.rs",
    "cap:compare-designs": "crates/reflow2-core/tests/compare.rs",
}
# The one capability that ships without an automated check — the honest
# remainder the gap list SHOULD carry: cap:adopt is a skill, exercised on a
# real system once (storyflow, 2026-07-20) but checked by no machine.
# Contracts between subsystems.
INTERFACES = [
    ("ifc:core-api", "DesignGraph API", "cmp:core", ["cmp:service"]),
    ("ifc:mcp-tools", "MCP tool surface", "cmp:service", ["cmp:skills"]),
    ("ifc:graph-export", "Design export document", "cmp:export", ["cmp:init"]),
]


def sha(p: pathlib.Path) -> str:
    return "sha256:" + hashlib.sha256(p.read_bytes()).hexdigest()[:16] if p.exists() else "sha256:absent"


# ---- DEPENDS_ON, derived from source — never from prose -------------------
#
# The 2026-07-20 self-adopt run found the model carried zero DEPENDS_ON edges,
# which made circular_dependencies structurally blind: the real
# structure<->propagate cycle (both sides are `impl DesignGraph` blocks, so
# rustc never flags it) was invisible to the very detector built to find it.
#
# Two signals, because either alone under-reports:
#   1. `use crate::<module>` / `crate::<module>::` — explicit imports.
#   2. `self.<method>(` where <method> is defined in exactly one OTHER module's
#      `impl DesignGraph` block. Rust needs no `use` for inherent methods, and
#      it is precisely these that carry the cycle.
# Ambiguous method names (defined in more than one module) are skipped, and
# skipped loudly — a guessed edge is worse than a missing one.

_FN = re.compile(r"^\s*(?:pub(?:\((?:crate|super)\))?\s+)?(?:async\s+)?fn\s+([a-z_][a-z0-9_]*)", re.M)
_USE = re.compile(r"use\s+crate::([a-z_][a-z0-9_]*)")
_PATH = re.compile(r"crate::([a-z_][a-z0-9_]*)::")
_SELF = re.compile(r"self\.([a-z_][a-z0-9_]*)\s*\(")
_BLOCK = re.compile(r"/\*.*?\*/", re.S)


def _strip_comments(text: str) -> str:
    # Load-bearing: detect.rs carries a rustdoc intra-doc link
    # (`/// [HealOp::Merge]: crate::heal::HealOp::Merge`). Matched raw, that
    # line fabricates a detect<->heal cycle that does not exist in the code.
    text = _BLOCK.sub(" ", text)
    out = []
    for line in text.splitlines():
        if line.lstrip().startswith("//"):
            continue
        i = line.find("//")
        while i != -1:
            if i > 0 and line[i - 1] == ":":  # keep `https://` in literals
                i = line.find("//", i + 2)
                continue
            line = line[:i]
            break
        out.append(line)
    return "\n".join(out)


def derive_depends_on() -> tuple[list[tuple[str, str]], list[str]]:
    """(module-name pairs a->b, skipped-ambiguity notes) for reflow2-core."""
    src = REPO / "crates/reflow2-core/src"
    mods = {p.stem: _strip_comments(p.read_text())
            for p in sorted(src.glob("*.rs")) if p.stem != "lib"}
    owner: dict[str, set[str]] = {}
    for name, text in mods.items():
        for m in _FN.finditer(text):
            owner.setdefault(m.group(1), set()).add(name)
    pairs: set[tuple[str, str]] = set()
    skipped: list[str] = []
    for name, text in mods.items():
        for rx in (_USE, _PATH):
            for m in rx.finditer(text):
                if m.group(1) in mods and m.group(1) != name:
                    pairs.add((name, m.group(1)))
        for m in _SELF.finditer(text):
            owners = owner.get(m.group(1), set()) - {name}
            if len(owners) == 1:
                pairs.add((name, owners.pop()))
            elif len(owners) > 1:
                skipped.append(f"{name}.{m.group(1)}() -> {sorted(owners)}")
    return sorted(pairs), sorted(set(skipped))


def build(s: Server, fresh: bool = True) -> None:
    # Genesis refuses to clobber an existing graph — deliberately. When the
    # committed export was imported first (BL-71: the rebuild layers onto the
    # accumulated record instead of replacing it), the Project already exists
    # and the curated pass starts from create_node upserts instead.
    if fresh:
        s.call("genesis", {"project_id": "proj:reflow2", "name": "Reflow 2.0",
                           "objective": "Keep a design coherent from concept through operations",
                           "domain": "software"})
    for rid, (name, stmt, prio) in REQUIREMENTS.items():
        s.call("create_node", {"node_type": "Requirement", "id": rid,
                               "props": {"name": name, "statement": stmt,
                                         "priority": prio, "status": "accepted"}})
        s.call("contains", {"project_id": "proj:reflow2",
                            "child_type": "Requirement", "child_id": rid})
    for cid, name, desc, status, sats in CAPABILITIES:
        s.call("add_capability", {"id": cid, "name": name, "description": desc, "status": status})
        for r in sats:
            s.call("satisfies", {"from_id": cid, "to_id": r})
    for cid, name, desc, level in SUBSYSTEMS:
        s.call("add_component", {"id": cid, "name": name, "description": desc, "level": level})
        s.call("contains", {"project_id": "proj:reflow2",
                            "child_type": "Component", "child_id": cid})
    for cid, name, parent, caps in MODULES:
        s.call("add_component", {"id": cid, "name": name, "description": f"The {name} module."})
        s.call("contain_component", {"from_id": parent, "to_id": cid})
        for c in caps:
            s.call("allocate", {"from_id": c, "to_id": cid})
    # Structure from imports and calls, never from prose (adopt discipline).
    by_stem = {name: cid for cid, name, _parent, _caps in MODULES}
    pairs, skipped = derive_depends_on()
    unmapped = sorted({m for pair in pairs for m in pair if m not in by_stem})
    if unmapped:  # a module on disk the model doesn't carry — say so, loudly
        print(f"NOT MODELLED (no Component; DEPENDS_ON edges dropped): {unmapped}")
    for a, b in pairs:
        if a in by_stem and b in by_stem:
            s.call("create_edge", {
                "edge_type": "DEPENDS_ON",
                "from_type": "Component", "from_id": by_stem[a],
                "to_type": "Component", "to_id": by_stem[b],
                "props": {"dependency_type": "function_call",
                          "weight_basis": "evidence"}})
    if skipped:
        print(f"ambiguous self-calls skipped (defined in >1 module): {len(skipped)}")
        for line in skipped:
            print(f"  {line}")
    for cmp_id, path in ARTIFACTS.items():
        p = REPO / path
        s.call("link_artifact", {"artifact_id": f"art:{cmp_id.split(':')[1]}",
                                 "name": pathlib.Path(path).name, "location": path,
                                 "artifact_type": "code", "target_type": "Component",
                                 "target_id": cmp_id, "checksum": sha(p)})
    # cap:adopt is realized by a skill, not a module: the five-phase reverse-
    # engineering workflow lives in the kit and runs in the agent. Linking it
    # keeps `realized` honest; nothing automated verifies it, and that gap
    # should stay open.
    s.call("allocate", {"from_id": "cap:adopt", "to_id": "cmp:skills"})
    s.call("link_artifact", {"artifact_id": "art:adopt-skill",
                             "name": "adopt/SKILL.md",
                             "location": "getting-started/skills/adopt/SKILL.md",
                             "artifact_type": "document", "target_type": "Capability",
                             "target_id": "cap:adopt",
                             "checksum": sha(REPO / "getting-started/skills/adopt/SKILL.md")})
    for cap, path in VERIFICATIONS.items():
        vid = f"ver:{cap.split(':')[1]}"
        s.call("add_verification", {"id": vid, "name": pathlib.Path(path).name,
                                    "method": "test", "level": "integration"})
        s.call("verifies", {"verification_id": vid, "target_type": "Capability", "target_id": cap})
        s.call("set_verification_status", {"verification_id": vid, "status": "passing"})
    for iid, name, provider, consumers in INTERFACES:
        s.call("add_interface", {"id": iid, "name": name})
        s.call("provides", {"from_id": provider, "to_id": iid})
        for c in consumers:
            s.call("consumes", {"from_id": c, "to_id": iid})
    for did, name, decision, rationale, governs in DECISIONS:
        s.call("add_decision", {"id": did, "name": name,
                                "decision": decision, "rationale": rationale})
        for target in governs:
            s.call("governed_by", {"from_type": "Capability", "from_id": target,
                                   "to_type": "Decision", "to_id": did})
    # Releases are frozen at their git tags — never hashed from the working
    # tree. Two earlier versions of this block hashed the CURRENT files under a
    # tag's name, asserting content into a release that never carried it;
    # `git show tag:path` is the only honest source for "as shipped".
    def sha_at(tag: str, path: str) -> str:
        r = subprocess.run(["git", "show", f"{tag}:{path}"],
                           capture_output=True, cwd=REPO)
        if r.returncode != 0:
            raise SystemExit(
                f"{path} does not exist at tag {tag} — refusing to invent a checksum")
        return "sha256:" + hashlib.sha256(r.stdout).hexdigest()[:16]

    # A module added after a tag was cut is not in that tag's release — its
    # absence from the manifest is the truth, not a gap. Said out loud per
    # release rather than skipped silently; sha_at's refusal still guards the
    # case that matters (a file *claimed* for a release that never carried it).
    def exists_at(tag: str, path: str) -> bool:
        return subprocess.run(["git", "cat-file", "-e", f"{tag}:{path}"],
                              capture_output=True, cwd=REPO).returncode == 0

    RELEASES = [
        ("rel:v040", "v0.4.0", "0.4.0", "retired",
         "Superseded by v0.5.0 on the developer machine (deployment declaration "
         "withdrawn as rolled_back, 2026-07-20 — the binary was upgraded in "
         "place, not reverted).", []),
        ("rel:v050", "v0.5.0", "0.5.0", "retired",
         "First release cut from the public repo; first live run of release.yml "
         "(3-platform binaries + kit tarball, checksum-verified install.sh). "
         "Surface change from v0.4.0: documents tool; graph model unchanged. "
         "Never published: its release run sat stuck in the macos-x86_64 queue "
         "for 11h; superseded by v0.6.0 cut from current main (2026-07-21).",
         ["art:adopt-skill"]),
        ("rel:v060", "v0.6.0", "0.6.0", "retired",
         "The first release to actually reach a user (v0.5.0's run never "
         "published). Carried the deep-review tier-1 fixes and BL-57 tool-"
         "boundary honesty. Superseded in place by v0.6.1.",
         ["art:adopt-skill"]),
        ("rel:v061", "v0.6.1", "0.6.1", "retired",
         "Patch: the BL-58 core silent-failure batch (12 doctrine/correctness "
         "fixes). Superseded in place by v0.7.0.",
         ["art:adopt-skill"]),
        ("rel:v070", "v0.7.0", "0.7.0", "retired",
         "Minor: Snapshot.edges (BL-63, snapshots capture design edges), "
         "single_point_of_failure measured on the as-built operational network "
         "(BL-69), and the consumer CI coherence gate reflow2_check.py + "
         "ci-gate skill (BL-66) — the kit tarball's first second tool. "
         "Superseded in place by v0.8.0.",
         ["art:adopt-skill", "art:check"]),
        ("rel:v080", "v0.8.0", "0.8.0", "deployed",
         "Minor: compare_designs, the design-vs-design diff (BL-71 rung c) — "
         "the reconcile family's sibling for two as-designed records, as a "
         "core op + MCP tool + --diff CLI. No schema change. First release "
         "whose manifest carries compare.rs.",
         ["art:adopt-skill", "art:check"]),
    ]
    EXTRA_RELEASE_ARTIFACTS = {
        "art:adopt-skill": "getting-started/skills/adopt/SKILL.md",
        "art:check": "tools/reflow2_check.py",
    }
    s.call("add_environment", {"id": "env:dev", "name": "Developer machine",
                               "env_type": "development"})
    for rid, tag, version, status, description, extras in RELEASES:
        s.call("add_release", {"id": rid, "name": tag, "version": version,
                               "unit_type": "binary"})
        s.call("create_node", {"node_type": "Release", "id": rid,
                               "props": {"status": status,
                                         "description": description}})
        # The as-released view (BL-34): the manifest, frozen at the tag.
        for cmp_id, path in ARTIFACTS.items():
            if not exists_at(tag, path):
                print(f"  note: {path} absent at {tag} — not in that release's manifest")
                continue
            s.call("release_includes", {
                "release_id": rid, "target_type": "Artifact",
                "target_id": f"art:{cmp_id.split(':')[1]}",
                "as_checksum": sha_at(tag, path)})
        for art_id in extras:
            s.call("release_includes", {
                "release_id": rid, "target_type": "Artifact",
                "target_id": art_id,
                "as_checksum": sha_at(tag, EXTRA_RELEASE_ARTIFACTS[art_id])})
        # The skills tree ships too — the kit installs it into consumer
        # projects. Without this line the graph truthfully complained "built
        # but ships in nothing" (unreleased_component, first fired 2026-07-20).
        s.call("release_includes", {"release_id": rid,
                                    "target_type": "Component",
                                    "target_id": "cmp:skills"})
    # v0.4.0 ran here until v0.5.0 replaced it in place; rolled_back is the
    # sanctioned vocabulary for "the active declaration is withdrawn" (the
    # reconcile_deployment correction path uses exactly this).
    s.call("deploy_to", {"release_id": "rel:v040", "environment_id": "env:dev",
                         "status": "rolled_back"})
    for retired in ("rel:v050", "rel:v060", "rel:v061", "rel:v070"):
        s.call("deploy_to", {"release_id": retired, "environment_id": "env:dev",
                             "status": "rolled_back"})
    s.call("deploy_to", {"release_id": "rel:v080", "environment_id": "env:dev",
                         "status": "active"})


def analyse(s: Server) -> None:
    rep = s.call("graph_report")
    print(f"\n{'=' * 64}\n  reflow2's own functional design: {rep['total_nodes']} nodes")
    print(f"  {dict(rep['node_counts'])}")
    print(f"{'=' * 64}")

    gaps = s.call("detect_gaps")
    print(f"\n-- detect_gaps: {len(gaps)} --")
    for g in gaps:
        who = ", ".join(g["affected_ids"]) or "(project-level)"
        print(f"  {g['severity']:.2f}  {g['gap_source']:26} {who}")
        print(f"        {g['title']}")

    defects = s.call("detect_defects")
    print(f"\n-- detect_defects: {len(defects)} --")
    for d in defects:
        print(f"  {d['severity']:8} {d['category']:24} {d['message'][:70]}")

    hier = s.call("hierarchy_issues")
    print(f"\n-- hierarchy_issues: {len(hier)} --")
    for h in hier[:10]:
        print(f"  {h.get('kind')}: {h.get('message', '')[:80]}")

    surp = s.call("surprising_connections")
    print(f"\n-- surprising_connections: {len(surp)} --")
    for x in surp[:6]:
        print(f"  {json.dumps(x)[:110]}")

    alloc = s.call("evaluate_allocation")
    print(f"\n-- evaluate_allocation --\n  {json.dumps(alloc)[:400]}")

    cov = rep["verification"]
    print(f"\n-- verification coverage --\n  {cov}")

    # -- reconcile the model against the filesystem --------------------------
    # The probe the 2026-07-20 self-adopt run showed was missing: 15 of 33
    # source files had no Artifact and nothing here could say so. Sweep scope
    # is the product (both crates' src trees + the kit installer) — trial
    # scripts under tools/ are instruments, deliberately out of scope.
    swept = sorted(
        p for pat in ("crates/reflow2-core/src/*.rs", "crates/reflow2-mcp/src/*.rs")
        for p in REPO.glob(pat)
        if p.name != "lib.rs"  # pure re-export shims; not a meaningful unit
    ) + [REPO / "tools/reflow2_init.py"]
    arts = s.call("scan_nodes", {"node_type": "Artifact"})
    if isinstance(arts, dict):  # CLI path envelopes lists; smoke_mcp does not
        arts = arts["items"]
    by_loc = {a["properties"].get("location"): a["node_id"]
              for a in arts if a["properties"].get("location")}
    observed, unregistered = [], []
    for p in swept:
        loc = p.relative_to(REPO).as_posix()
        if loc in by_loc:
            observed.append({"artifact_id": by_loc[loc], "present": True,
                             "checksum": sha(p)})
        else:
            unregistered.append(loc)
            observed.append({"artifact_id": f"art:unmodelled:{loc}",
                             "present": True, "checksum": sha(p)})
    drift = s.call("reconcile_artifacts", {"observed": observed, "exhaustive": True})
    findings = drift.get("findings", [])
    print(f"\n-- reconcile vs filesystem: {len(findings)} finding(s), "
          f"{len(swept)} files swept --")
    for f in findings:
        print(f"  {f.get('kind', '?'):24} {f.get('artifact_id', '?')}")
    if not findings:
        print("  model and filesystem agree")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--analyse-only", action="store_true",
                    help="import the committed export and analyse, without rebuilding")
    args = ap.parse_args()

    tmp = tempfile.mkdtemp(prefix="reflow2-design-")
    s = Server(str(REPO / "target/debug/reflow2-mcp"), str(pathlib.Path(tmp) / "graph"))
    try:
        if args.analyse_only:
            s.call("import_graph", {"document": json.loads(EXPORT.read_text())})
        else:
            # BL-71: the committed export is the ACCUMULATED design record —
            # the curated pass layers onto it (import first, then upsert), so
            # the session-written layer (decisions, freshness claims, change
            # events) survives a rebuild. Replacing the file with the curated
            # model alone silently discarded that layer once (2026-07-21).
            prior = json.loads(EXPORT.read_text()) if EXPORT.exists() else None
            if prior is not None:
                s.call("import_graph", {"document": prior})
            build(s, fresh=prior is None)
            doc = s.call("export_graph")
            if prior is not None and len(doc["nodes"]) < len(prior["nodes"]):
                print(f"REFUSING to write {EXPORT.relative_to(REPO)}: the rebuilt "
                      f"graph has {len(doc['nodes'])} nodes, the committed export "
                      f"{len(prior['nodes'])} — a shrinking export is the "
                      f"silent-loss signature (BL-71). Nothing was written.",
                      file=sys.stderr)
                return 1
            EXPORT.parent.mkdir(parents=True, exist_ok=True)
            EXPORT.write_text(json.dumps(doc, indent=2, sort_keys=True) + "\n")
            print(f"exported {len(doc['nodes'])} nodes / {len(doc['edges'])} edges "
                  f"-> {EXPORT.relative_to(REPO)}")
        analyse(s)
    finally:
        s.close()
        shutil.rmtree(tmp, ignore_errors=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
