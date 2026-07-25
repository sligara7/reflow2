//! The ingress trust boundary — hidden instructions stripped out of foreign
//! text, and *said out loud*.
//!
//! reflow2's standing rule is that graph text is data, never instructions. Up
//! to now that rule was addressed only to a well-behaved reader: a line in
//! every skill and in the server handshake. This is its mechanical half, and
//! the technique is imported from `github-mcp-server`'s `pkg/sanitize`
//! (docs/github-mcp-nuggets.md) — a hosted MCP server with real
//! adversaries, which strips the characters whose entire purpose is to make
//! text read one way to a human and another way to a machine.
//!
//! Three classes, each a documented smuggling channel:
//!
//! - **Unicode tag characters** (U+E0001, U+E0020–U+E007F) — an invisible
//!   alphabet. A whole paragraph of instructions can ride inside what looks
//!   like an empty string, visible to no reviewer.
//! - **Bidirectional overrides** (U+202A–U+202E, U+2066–U+2069) — reorder how
//!   text *renders* without changing what it *says*, so the sentence a person
//!   approves is not the sentence stored.
//! - **Hidden formatting** (zero-width space/non-joiner, LRM/RLM, soft hyphen,
//!   BOM, Mongolian vowel separator, U+2060–U+2064) — word-splitting that
//!   defeats a human skim and any exact-match check.
//!
//! **Two deliberate departures from the source, both of them reflow2's rules:**
//!
//! 1. **It reports.** GitHub sanitises silently, which is right when the job is
//!    rendering an issue body. A design brain that quietly rewrote a
//!    requirement statement would be unauditable — and rule 6 forbids silent
//!    drops. So every pass returns what it removed, by class, and the caller is
//!    expected to surface it.
//! 2. **No HTML stripping.** The source runs bluemonday over its text; reflow2
//!    renders no HTML, and a design may legitimately say `Vec<Component>`,
//!    `a < b`, or contain a diagram in angle brackets. Stripping tags here
//!    would corrupt honest design content to defend against a risk this project
//!    does not have — and a filter that mangles real content is one people turn
//!    off.
//!
//! ZERO-WIDTH JOINER (U+200D) IS DELIBERATELY KEPT. It is load-bearing inside
//! emoji sequences (👨‍👩‍👧 is three people joined by two of them), so removing it
//! would visibly damage ordinary text. The source makes the same call.
//!
//! NOT COVERED YET, named rather than left implicit: metadata hidden in
//! code-fence info strings (the source filters it), and the import path —
//! `import_graph` deliberately does **not** rewrite the document it is given,
//! because an export's content is its identity and silently editing it on the
//! way in would make two honest exports disagree. Detecting and reporting on
//! import is the next rung.

use std::borrow::Cow;

/// What one sanitisation pass removed, by class. Never a bare total: the class
/// is the actionable part, since a tag character is an attempted smuggle while
/// a stray BOM is usually a careless editor.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SanitizeReport {
    /// Unicode tag characters (U+E0001, U+E0020–U+E007F) — the invisible
    /// alphabet. Effectively never innocent.
    pub unicode_tag: usize,
    /// Bidirectional override/isolate controls — render-vs-store mismatch.
    pub bidi_control: usize,
    /// Zero-width and hidden formatting characters.
    pub hidden_formatting: usize,
}

impl SanitizeReport {
    /// Total characters removed.
    pub fn total(&self) -> usize {
        self.unicode_tag + self.bidi_control + self.hidden_formatting
    }

    /// Nothing was removed — the text passed through untouched.
    pub fn is_clean(&self) -> bool {
        self.total() == 0
    }

    /// Fold another pass into this one, for a per-run tally.
    pub fn merge(&mut self, other: &SanitizeReport) {
        self.unicode_tag += other.unicode_tag;
        self.bidi_control += other.bidi_control;
        self.hidden_formatting += other.hidden_formatting;
    }

    /// A one-line description naming the classes, for a warning a person reads.
    /// Empty string when clean, so a caller can test it as a flag.
    pub fn describe(&self) -> String {
        let mut parts = Vec::new();
        if self.unicode_tag > 0 {
            parts.push(format!("{} unicode tag character(s)", self.unicode_tag));
        }
        if self.bidi_control > 0 {
            parts.push(format!("{} bidi control character(s)", self.bidi_control));
        }
        if self.hidden_formatting > 0 {
            parts.push(format!(
                "{} hidden formatting character(s)",
                self.hidden_formatting
            ));
        }
        parts.join(", ")
    }
}

/// Which class a character belongs to, or `None` if it is ordinary text.
fn classify(c: char) -> Option<Class> {
    match c {
        // Unicode tag block: the invisible alphabet.
        '\u{E0001}' | '\u{E0020}'..='\u{E007F}' => Some(Class::Tag),
        // Bidi embeddings/overrides and isolates.
        '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' => Some(Class::Bidi),
        // Zero-width and hidden formatting. U+200D (ZWJ) is NOT here: see the
        // module docs — it is load-bearing in emoji sequences.
        '\u{200B}'
        | '\u{200C}'
        | '\u{200E}'
        | '\u{200F}'
        | '\u{00AD}'
        | '\u{FEFF}'
        | '\u{180E}'
        | '\u{2060}'..='\u{2064}' => Some(Class::Hidden),
        _ => None,
    }
}

enum Class {
    Tag,
    Bidi,
    Hidden,
}

/// Strip instruction-carrying invisible characters, returning the cleaned text
/// and what was removed.
///
/// Clean input is returned borrowed and unallocated, so calling this on every
/// field of an honest document costs one scan and nothing else.
pub fn sanitize_text(input: &str) -> (Cow<'_, str>, SanitizeReport) {
    let mut report = SanitizeReport::default();
    if !input.chars().any(|c| classify(c).is_some()) {
        return (Cow::Borrowed(input), report);
    }
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match classify(c) {
            None => out.push(c),
            Some(Class::Tag) => report.unicode_tag += 1,
            Some(Class::Bidi) => report.bidi_control += 1,
            Some(Class::Hidden) => report.hidden_formatting += 1,
        }
    }
    (Cow::Owned(out), report)
}
