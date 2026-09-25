//! Who a write was FOR: a caller names a contributor, and every node it writes
//! is credited to them.
//!
//! `req:a-session-names-the-person-it-writes-for-and-the-server-remembers-it`
//! (accepted, Anthony 2026-09-23; clause 1 of flo2's levied
//! `req:a-write-carries-who-made-it-and-the-server-records-that-it-happened`).
//! A caller declares a contributor once — for a session, or on one request —
//! and every node written on its behalf gets an `AUTHORED_BY` edge (role
//! `author`) to that contributor, without the caller repeating it on each
//! call. The server records which nodes a call wrote here, at the one place
//! every node write passes ([`DesignGraph::create_node`]), so nothing depends
//! on a constructor remembering to report what it touched.
//!
//! 🛑 ATTRIBUTION, NEVER AUTHORITY. The id is taken ON TRUST — reflow2 verifies
//! nothing, because the caller (flo2's gateway, or a person's own agent) is
//! what authenticated it. It is used only for WHO a change is recorded as
//! coming from. It never signs an approval: approving, accepting or settling
//! still needs the explicit `approver` path, and
//! `rule:design-intent-moves-only-on-the-owners-word` is untouched. Whether a
//! caller-asserted identity may ever carry authority stays open in
//! `dec:idea-may-a-caller-asserted-identity-be-trusted-and-for-what`.
//!
//! ⚠️ WHAT IS NOT CREDITED: `Snapshot` nodes (history the server keeps for
//! itself, not something anybody wrote) and `Contributor` nodes (a person is
//! not the author of themselves). A write that only draws EDGES touches no
//! node and credits nothing — the automatic journal of every mutating call is
//! clause 2 of the levied ask and is deliberately not built here.

use crate::DesignGraph;
use crate::foundation::core::DynoError;
use crate::nodes::node;

/// The node types a declared contributor is never credited with.
const NOT_CREDITED: [&str; 2] = [node::SNAPSHOT, node::CONTRIBUTOR];

impl DesignGraph {
    /// Start recording every node written from here on. Recording that was
    /// already on is restarted — a caller that forgot to take the last log
    /// must not have its writes credited to the next caller.
    pub fn begin_touch_log(&mut self) {
        self.touch_log = Some(Vec::new());
    }

    /// Stop recording and return what was written, in write order. Empty when
    /// nothing was recorded or recording was never started.
    pub fn take_touch_log(&mut self) -> Vec<(String, String)> {
        self.touch_log.take().unwrap_or_default()
    }

    /// Whether `contributor_id` can be written for: it must already exist as a
    /// Contributor. reflow2 never invents the person — the refusal says to
    /// `add_contributor` first, and says so differently when the id names an
    /// Actor (a wrong type, not a wrong id).
    pub fn require_writes_for(&self, contributor_id: &str) -> Result<(), DynoError> {
        self.require_contributor(contributor_id, "writes_for", "write for")
    }

    /// Credit each written node to `contributor_id` as its author, and return
    /// how many were credited.
    ///
    /// Each node is credited once however many times the call wrote it, in the
    /// order first written. A node that no longer exists (written, then
    /// deleted in the same call) is skipped, as are the types in
    /// [`NOT_CREDITED`]. An author edge the contributor already holds is kept
    /// as it is ([`DesignGraph::authored_by`] adds to a role SET).
    pub fn credit_writes(
        &mut self,
        touched: &[(String, String)],
        contributor_id: &str,
    ) -> Result<usize, DynoError> {
        self.require_writes_for(contributor_id)?;
        let mut seen = std::collections::HashSet::new();
        let mut credited = 0;
        for (node_type, id) in touched {
            if NOT_CREDITED.contains(&node_type.as_str()) || !seen.insert(id.as_str()) {
                continue;
            }
            if self.get_node(node_type, id)?.is_none() {
                continue;
            }
            self.authored_by(node_type, id, contributor_id, Some("author"), None)?;
            credited += 1;
        }
        Ok(credited)
    }
}
