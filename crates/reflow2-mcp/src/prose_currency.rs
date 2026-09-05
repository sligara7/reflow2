//! Say when a status has advanced past the prose that describes it.
//!
//! # The finding this exists to fix
//!
//! A node's `status` and its own `description` are two claims in ONE node, and
//! nothing kept them honest with each other. Reported by dev_storyflow
//! 2026-09-02 and recorded as
//! `fact:defect-a-status-can-advance-past-its-own-prose-and-nothing-says-so`:
//! they wrote a capability whose description said, in capitals, *"THE DROPLET
//! STILL RUNS THE OLD SCRIPT"*, installed the fix TWENTY MINUTES LATER, called
//! `set_capability_status(realized)` — and the description still said the
//! droplet ran the old script. The status said delivered, the prose said not
//! started, nothing flagged it, and they caught it only by re-reading their own
//! writing.
//!
//! ⭐ THE ROT HAPPENED INSIDE THE GRAPH, INSIDE ONE NODE, INSIDE TWENTY
//! MINUTES, to someone who had read `epoch:2026_08_17_state_documents_rot_
//! silently` — an entire epoch about state-describing text going stale — THAT
//! SAME DAY. Their conclusion is the one this module is built on: vigilance is
//! not the countermeasure, something mechanical has to be.
//!
//! # Why a sentence in a reply, and not a detector
//!
//! The reporter's own proposal, and it is right: a `set_*_status` call ALREADY
//! KNOWS the description was not touched, because changing the status is the
//! whole of what it does. So this needs no gap, no nudge, no sweep and no
//! second call — one block in the reply the caller is already reading, at the
//! exact moment the divergence is created.
//!
//! A detector would find the same thing later, on a graph where the prose has
//! already been quoted back to somebody. This finds it while the author is
//! still in the room.
//!
//! # What it deliberately does NOT do
//!
//! - **It never judges the prose.** It cannot read English, and a description
//!   that is still perfectly true after a status change is the common case. It
//!   states the FACT — this prose was written while the status was X, and the
//!   status is now Y — and asks. `dec:report-dont-judge`.
//! - **It is silent when the status did not move.** Re-setting a status to what
//!   it already was creates no divergence, and a block that appears on every
//!   call is the noise `with_capture_notes`' siblings are explicitly built not
//!   to become.
//! - **It is silent when there is no prose.** Nothing to have gone stale.

use serde::Serialize;
use serde_json::{Map as JsonMap, Value as JsonValue};

/// How much of the prose to quote back. Enough to recognise what it claims,
/// short enough that it does not bury the reply it is attached to.
const EXCERPT_CHARS: usize = 240;

/// The prose fields a status can outrun, in the order they are looked for.
///
/// `description` covers Capability and DesignEpoch; `statement` covers the
/// types that carry their prose under that name. The FIRST non-empty one wins —
/// a node carrying both is describing itself twice and either is enough to make
/// the point.
const PROSE_FIELDS: [&str; 2] = ["description", "statement"];

/// Attach the currency note to a `set_*_status` reply, if there is one to make.
///
/// `prior_status` is what the node said BEFORE this call — read it before the
/// write, because after the write there is nothing left to compare against.
pub fn with_prose_currency<T: Serialize>(
    value: T,
    prior_status: Option<&str>,
) -> Result<JsonValue, serde_json::Error> {
    let mut v = serde_json::to_value(value)?;
    let Some(note) = currency_note(&v, prior_status) else {
        return Ok(v);
    };
    if let Some(obj) = v.as_object_mut() {
        obj.insert("prose_currency".into(), JsonValue::Object(note));
    }
    Ok(v)
}

/// The note itself, or `None` when nothing diverged. Split out so it is
/// testable without building a whole reply.
pub fn currency_note(
    v: &JsonValue,
    prior_status: Option<&str>,
) -> Option<JsonMap<String, JsonValue>> {
    let props = v.get("properties")?.as_object()?;
    let now = props.get("status")?.as_str()?;

    // Silent unless the status actually MOVED. Re-setting a status to what it
    // already was creates no divergence to report.
    let prior = prior_status?;
    if prior == now {
        return None;
    }

    let (field, prose) = PROSE_FIELDS.iter().find_map(|f| {
        let s = props.get(*f)?.as_str()?;
        (!s.trim().is_empty()).then_some((*f, s))
    })?;

    let mut note = JsonMap::new();
    note.insert("field".into(), JsonValue::String(field.to_string()));
    note.insert(
        "written_under_status".into(),
        JsonValue::String(prior.to_string()),
    );
    note.insert("status_now".into(), JsonValue::String(now.to_string()));
    note.insert("excerpt".into(), JsonValue::String(excerpt(prose)));
    note.insert(
        "note".into(),
        JsonValue::String(format!(
            "`status` moved {prior} -> {now}, and this call did not touch `{field}` — so that \
             prose was written while the status was {prior}. It is quoted above so you can judge \
             it here rather than in another call. DOES IT STILL READ TRUE? Nothing checks this: \
             a status and a description are two claims in one node, and only a person can say \
             whether they still agree."
        )),
    );
    Some(note)
}

/// First [`EXCERPT_CHARS`] characters, cut on a CHAR boundary and marked when
/// cut. Slicing bytes would panic on the first non-ASCII description, and this
/// project's prose is full of them.
fn excerpt(s: &str) -> String {
    let trimmed = s.trim();
    if trimmed.chars().count() <= EXCERPT_CHARS {
        return trimmed.to_string();
    }
    let head: String = trimmed.chars().take(EXCERPT_CHARS).collect();
    format!("{head}… (cut at {EXCERPT_CHARS} chars)")
}
