//! A design knows its own name, and remembers it across opens.
//!
//! `req:design-identity`, governed by `dec:identity-out-of-band` — *names are
//! assigned with zero coordination, never derived from shared state.*
//!
//! Until now every reflow2 graph answered to the same hardcoded id, so **no
//! design could tell another design from itself**. `mirror_surface` has to
//! refuse a surface whose source is the importing graph (a filtered copy of
//! your own design would overwrite the full one), and with one constant that
//! check could never pass for anybody. Composition between designs was
//! meaningless: they all had the same name.
//!
//! ## Why the id lives beside the store and not in it
//!
//! **The graph id namespaces every stored key.** Reading anything requires
//! already knowing it, so it cannot be a node inside the design — that is a
//! chicken-and-egg, and getting it wrong is silent: a graph reopened under a
//! name it was not created with finds nothing and presents as an *empty
//! design*. So identity sits in a sibling file, exactly where the version stamp
//! already sits, and is read before the design is.
//!
//! Its own file rather than a field in `<graph>.meta.json`, for the same reason
//! the sync marker got one: `check_and_stamp` rewrites that file wholesale on
//! every open, and changing its shape would make every existing graph fail to
//! open with "the version stamp is not readable".
//!
//! ## The migration is the dangerous part, so it is the explicit part
//!
//! Every graph that exists today holds its design under the old default id.
//! Minting a fresh id for those would be a catastrophe of exactly the silent
//! kind above — the design would still be on disk, and reflow2 would open a new
//! empty one beside it and report nothing wrong. So a graph that **already has
//! design data under the default id adopts that id** as its identity, forever.
//! Only a graph with nothing under it mints.
//!
//! One consequence worth stating: `graph_id` is part of the export's content
//! hash, so adoption is also what keeps every existing export, chain link and
//! committed record valid across this change.

use std::path::{Path, PathBuf};

use dynograph_core::DynoError;
use serde::{Deserialize, Serialize};

/// How a design came by its name — recorded because the two cases have very
/// different consequences, and a later reader should not have to guess.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    /// Minted for a graph that had no design in it yet.
    Minted,
    /// Kept from the era when every graph shared one id, because this graph
    /// already held a design under it.
    Adopted,
}

/// What this design is called, and what it was called by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignIdentity {
    /// The storage-scoping id. Stable for the life of the design, including
    /// across machines and copies — a copy of a design *is* that design.
    pub graph_id: String,
    /// The human-facing name. A label on top of the id, changeable at will,
    /// and never load-bearing: two designs may share a label and still be
    /// distinct.
    pub label: String,
    /// Minted or adopted.
    pub origin: Origin,
    /// Which reflow2 wrote this record.
    pub minted_by: String,
}

/// `<graph-path>.id.json` — a sibling of the store, like the version stamp.
pub fn identity_path(graph_path: &str) -> PathBuf {
    let p = Path::new(graph_path);
    match p.file_name().map(|n| n.to_string_lossy().to_string()) {
        Some(n) => p.with_file_name(format!("{n}.id.json")),
        None => PathBuf::from(format!("{graph_path}.id.json")),
    }
}

/// A name assigned with zero coordination — nothing shared is read, so nothing
/// can race, at one seat or a thousand (`dec:identity-out-of-band`).
///
/// Deliberately not a UUID crate: the inputs already make it unique by
/// construction — the nanosecond it was created, the process that created it,
/// and where it lives — and a dependency added for sixteen hex characters is a
/// dependency every consumer pays a rebuild for.
fn mint(graph_path: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let absolute = std::fs::canonicalize(graph_path)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| graph_path.to_string());
    let seed = format!("{nanos}|{}|{absolute}", std::process::id());
    format!("{:016x}", crate::nodes::fnv1a(&seed))
}

/// A friendly default: the project directory's name, not the store's.
///
/// `<project>/.reflow2/graph` should read as "project", which is what a person
/// would call it — the two path segments below it are reflow2's plumbing.
fn default_label(graph_path: &str) -> String {
    let p = std::fs::canonicalize(graph_path).unwrap_or_else(|_| PathBuf::from(graph_path));
    let mut cursor = p.as_path();
    while let Some(name) = cursor.file_name().and_then(|n| n.to_str()) {
        if name != "graph" && name != ".reflow2" {
            return name.to_string();
        }
        match cursor.parent() {
            Some(parent) => cursor = parent,
            None => break,
        }
    }
    "design".to_string()
}

/// Read this design's identity, establishing it on first open.
///
/// `holds_default_design` is asked only when there is no identity file yet, and
/// answers the migration question: does this store already contain a design
/// under the old shared id? If it does, that id is adopted rather than
/// replaced — see the module docs for why the alternative is silent data loss.
pub fn resolve(
    graph_path: &str,
    default_id: &str,
    holds_default_design: impl FnOnce() -> bool,
) -> Result<DesignIdentity, DynoError> {
    let path = identity_path(graph_path);
    match std::fs::read_to_string(&path) {
        Ok(text) => {
            // Refused rather than defaulted: an unreadable identity file is the
            // one thing we must not paper over, because "default it" means
            // opening a different design under the same path and finding it
            // empty (req:design-identity).
            return serde_json::from_str(&text).map_err(|e| {
                DynoError::Serialization(format!(
                    "the design identity at {} is not readable ({e}). It records which design \
                     this store holds, and reflow2 will not guess: fix the file, or move it aside \
                     to have a new identity established.",
                    path.display()
                ))
            });
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            return Err(DynoError::Storage(format!(
                "cannot read the design identity at {}: {e}",
                path.display()
            )));
        }
    }

    let identity = if holds_default_design() {
        DesignIdentity {
            graph_id: default_id.to_string(),
            label: default_label(graph_path),
            origin: Origin::Adopted,
            minted_by: env!("CARGO_PKG_VERSION").to_string(),
        }
    } else {
        DesignIdentity {
            graph_id: mint(graph_path),
            label: default_label(graph_path),
            origin: Origin::Minted,
            minted_by: env!("CARGO_PKG_VERSION").to_string(),
        }
    };
    write(graph_path, &identity)?;
    Ok(identity)
}

/// Has a RocksDB store ever held data at this path?
///
/// Asked BEFORE the store is opened, because opening creates the directory and
/// erases the distinction. Deliberately "has ever held data" rather than "exists":
/// a store that was created and never written carries `CURRENT`, `MANIFEST-*` and
/// `OPTIONS-*` from the open alone, and treating those as content would refuse a
/// path somebody merely touched. Data lands either in an SST or, before it is
/// flushed, in a non-empty write-ahead log.
///
/// A read failure answers `false`. This feeds a REFUSAL, and a directory we
/// cannot list is not evidence that a design is in it — the conservative answer
/// for a guard is the one that does not invent a reason to refuse.
pub fn store_has_content(graph_path: &str) -> bool {
    let Ok(entries) = std::fs::read_dir(graph_path) else {
        return false;
    };
    entries.flatten().any(|e| {
        let name = e.file_name();
        let name = name.to_string_lossy();
        if name.ends_with(".sst") {
            return true;
        }
        // The WAL: data written and not yet flushed is here and nowhere else.
        name.ends_with(".log") && e.metadata().map(|m| m.len() > 0).unwrap_or(false)
    })
}

/// Read this design's identity on a durable open, refusing the one case
/// [`resolve`] cannot tell apart from a new design.
///
/// **The case.** The identity sidecar sits BESIDE the store, so the two can be
/// parted — a partial restore, a snapshot taken mid-write, a sync tool that skips
/// dotfiles, or (the one the Dockerfile warns about in capitals at its `/data`
/// layout) a container started with `-v` scoped to `.../graph` instead of its
/// parent. When they are parted, `resolve` finds no file and asks
/// `holds_default_design`, which probes only for a design under the OLD SHARED
/// id. That probe is the legacy migration path and **cannot see a minted design
/// by construction** — every design created since identity landed is under a
/// minted id. So it answers false, a new id is minted, and the mint is written
/// over the missing sidecar. The design is still on disk, now unreachable because
/// the id namespaces every stored key, and the id needed to reach it is gone.
/// Nothing errors; the design presents as empty and healthy.
///
/// `cap:hosted-state-on-a-volume` states the required behaviour directly — "a
/// store without its identity sidecar is refused rather than opened empty" — and
/// the same rule already holds one case over: an UNREADABLE sidecar is refused
/// with "reflow2 will not guess". Corrupt was refused and ABSENT was not, which
/// is the wrong way round, because absent is the likelier of the two on a volume.
///
/// **Why the guard is `store_has_content` and not a scan.** The honest check
/// would be "does this store hold a design under ANY id" — but ids namespace
/// every key and the storage engine offers no enumeration of them
/// (`count_nodes` needs the id you are trying to discover), so that question
/// cannot be asked without a change to the pinned foundation. "The store already
/// held data, and it is not the legacy design" answers the same question from the
/// outside, using only what a filesystem can see.
///
/// The three outcomes, and why each is what it is:
///
/// - **sidecar present** — nothing to guard; `resolve` reads it, corrupt included.
/// - **no sidecar, no prior content** — a genuinely new store. Mint, as before.
/// - **no sidecar, prior content, holds the legacy design** — the migration case.
///   Adopt, as before, or every pre-identity graph would be refused.
/// - **no sidecar, prior content, no legacy design** — REFUSE. Something was here
///   and we cannot name it.
///
/// Refusing costs an operator one error naming the file to put back. Opening
/// costs them the design, silently — and `dec:two-sided-accept` ("silent
/// drift-accept does not exist") is the same principle one layer up.
pub fn resolve_on_open(
    graph_path: &str,
    default_id: &str,
    store_had_content: bool,
    holds_default_design: impl FnOnce() -> bool,
) -> Result<DesignIdentity, DynoError> {
    // A sidecar that exists is `resolve`'s business either way — it reads it, and
    // refuses it if it is unreadable. The closure is never reached on that path.
    if identity_path(graph_path).exists() {
        return resolve(graph_path, default_id, || false);
    }

    if !store_had_content {
        // Nothing was ever stored here, so there is no design to lose. This is
        // every first open, and it must stay cheap and silent.
        return resolve(graph_path, default_id, holds_default_design);
    }

    if holds_default_design() {
        // A pre-identity graph: its data really is under the shared id, and
        // adopting it is what keeps it readable. Refusing here would lock every
        // graph that predates identity out of its own design.
        return resolve(graph_path, default_id, || true);
    }

    Err(DynoError::Storage(format!(
        "the design at {graph_path} has lost its identity file ({}), and reflow2 will not guess.\n\
         This store already holds data, but not under the shared id every pre-identity design \
         used — so it belongs to a design whose name lived only in that file. Opening anyway \
         would mint a NEW name, write it over the missing one, and present the design as empty \
         while it is still on disk and no longer reachable.\n\
         The identity file is a SIBLING of the store, not inside it. If this is a container, the \
         usual cause is a volume mounted at the store directory instead of its parent — mount the \
         parent. Otherwise restore {} from a backup, alongside the store it belongs to.",
        identity_path(graph_path).display(),
        identity_path(graph_path).display(),
    )))
}

/// Persist an identity beside the store.
pub fn write(graph_path: &str, identity: &DesignIdentity) -> Result<(), DynoError> {
    let path = identity_path(graph_path);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let json = serde_json::to_string_pretty(identity).map_err(|e| {
        DynoError::Serialization(format!("cannot serialize the design identity: {e}"))
    })?;
    std::fs::write(&path, json + "\n").map_err(|e| {
        DynoError::Storage(format!(
            "cannot write the design identity at {}: {e}",
            path.display()
        ))
    })
}

/// Rename the design. The label is a label: the id never moves, because
/// everything stored is keyed by it and every export ever written names it.
pub fn set_label(graph_path: &str, label: &str) -> Result<DesignIdentity, DynoError> {
    let path = identity_path(graph_path);
    let text = std::fs::read_to_string(&path).map_err(|e| {
        DynoError::Storage(format!(
            "no design identity at {} to rename ({e}) — open the graph once to establish it.",
            path.display()
        ))
    })?;
    let mut identity: DesignIdentity = serde_json::from_str(&text).map_err(|e| {
        DynoError::Serialization(format!("the design identity is not readable: {e}"))
    })?;
    identity.label = label.to_string();
    write(graph_path, &identity)?;
    Ok(identity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sidecar_sits_beside_the_store() {
        assert_eq!(
            identity_path("/p/.reflow2/graph"),
            PathBuf::from("/p/.reflow2/graph.id.json")
        );
    }

    #[test]
    fn two_designs_minted_at_once_do_not_collide() {
        // Unique by construction: same nanosecond is possible, same path is not.
        let a = mint("/tmp/one");
        let b = mint("/tmp/two");
        assert_ne!(a, b);
        assert_eq!(a.len(), 16, "a readable, fixed-width id: {a}");
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    }
}

// ---------------------------------------------------------------------------
// Seat identity — who is *working*, as opposed to what is being worked on.
// ---------------------------------------------------------------------------

/// This session's name, minted once per process.
///
/// `req:claims-have-owners`. A claim that does not say who made it cannot be
/// told from a claim nobody is working any more, and a ghost claim makes the
/// overlap report lie — which is worse than no report, because people act on it.
///
/// Same doctrine as the design's own name (`dec:identity-out-of-band`): nothing
/// shared is read, so nothing can race at one seat or fifteen. The shape is
/// `<machine>:<pid>:<mint>`, and it is chosen to make **liveness computable**
/// rather than asserted — a later reader can ask the operating system whether
/// that process still exists instead of trusting a flag somebody set.
pub fn seat_id() -> String {
    static SEAT: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    SEAT.get_or_init(mint_seat).clone()
}

/// A fresh seat, not the process-wide one.
///
/// `req:seat-per-client`. One server can hold many client sessions
/// (`req:sessions-share-a-graph`), and the process-wide seat is exactly wrong
/// there: every client would report the same owner, so every claim would name
/// the same seat and the overlap report would tell six sessions they are each
/// other. A session mints its own on connect.
///
/// **Honest limit, because it is easy to misread.** The seat carries a pid, so
/// liveness answers "is the process that made this claim still running". Under
/// one server that is the right answer about *the server*, and only a proxy for
/// the session: a client that disconnects while the server lives still reads
/// `live`. Per-session liveness needs the server's own session registry, which
/// the core cannot see — recorded rather than papered over.
pub fn mint_seat() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    // The address of a stack local disambiguates two mints in the same
    // nanosecond within one process — which is exactly the case a shared server
    // creates when two sessions connect at once.
    let here = &nanos as *const u128 as usize;
    let mint = format!(
        "{:08x}",
        crate::nodes::fnv1a(&format!("{nanos}|{here:x}")) & 0xffff_ffff
    );
    format!("{}:{}:{mint}", machine(), std::process::id())
}

/// This machine's name, for telling "their session died" from "their session is
/// on a different computer, and I cannot see it from here".
///
/// Public so tests can build a seat this machine will recognise, rather than
/// reimplementing the lookup and disagreeing with it somewhere subtle.
///
/// Best effort by design, and honest when it fails: an unknown machine makes
/// every foreign claim report as `Unknown` rather than as alive or dead, which
/// is the only truthful answer available.
pub fn machine() -> String {
    // Each source is trimmed and emptiness-checked BEFORE falling through, so
    // an empty HOSTNAME (set but blank, which happens in stripped environments)
    // still reaches /etc/hostname instead of short-circuiting to unknown.
    let non_empty = |s: String| Some(s.trim().to_string()).filter(|h| !h.is_empty());
    std::env::var("HOSTNAME")
        .ok()
        .and_then(non_empty)
        .or_else(|| {
            std::fs::read_to_string("/etc/hostname")
                .ok()
                .and_then(non_empty)
        })
        .unwrap_or_else(|| "unknown-machine".to_string())
}

/// Is the session that made a claim still running?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Liveness {
    /// The process is still there. Somebody is probably working this.
    Live,
    /// The session that made this claim has exited. The claim is a ghost —
    /// still worth reading for what it says, never worth treating as held.
    Gone,
    /// Made on another machine, by a seat with no name, or on a machine that
    /// could not identify itself. Reported as unknown rather than guessed:
    /// calling a foreign claim dead would invite somebody to take work that is
    /// actively being done.
    Unknown,
}

/// The seats this process is currently serving, when it is serving more than
/// itself.
///
/// # Why a registry exists at all, when the whole design says "compute, never remember"
///
/// A seat carries a pid, and asking the OS whether that pid is alive is a real
/// computation that cannot go stale. That was the right and complete answer
/// under stdio, where one process WAS one session. Under `--shared` it silently
/// stopped being an answer at all: every seat is minted BY THE DAEMON, so every
/// seat carries the daemon's pid, and the probe asks "is the server running?" —
/// to which the answer is always yes, because nothing else could have replied.
///
/// Measured in dev_storyflow 2026-08-07 (w-613d836b): `mint_seat` returned a
/// seat whose pid was the `--serve-shared` process, while their session's own
/// shell was a different pid entirely. `Gone` had become unreachable, so a
/// worker that died hours ago still read as holding its region and a peer would
/// defer to nobody. `cap:claim-liveness` was status `verified` throughout.
///
/// So the pid answers a question about the SERVER, and only the server knows
/// which SESSIONS it is holding. That knowledge cannot be computed from the
/// graph or from `/proc` — it exists solely in the process, which is why this is
/// the one thing here that is remembered rather than derived. It is kept honest
/// three ways: it lives only in the serving process (nothing is persisted, so
/// nothing survives to go stale), a session's entry is removed when its handler
/// is dropped, and **a process that never registers keeps the old behaviour
/// exactly** — `seat_liveness` consults this only when it has been populated, so
/// stdio is byte-for-byte unchanged.
/// # Why TWO sets and not one
///
/// The obvious shape — one set of currently-attached seats, consulted whenever
/// the seat's pid is ours — is wrong, and the tests caught it: it makes the
/// registry authoritative over seats it has never heard of. One `attach()`
/// anywhere in a process would flip every seat minted WITHOUT a lease from
/// `Live` to `Gone`, because absent-from-the-set was being read as departed.
/// That is the half-populated registry this type's own warning predicted, and it
/// is far too easy to reach: any caller of bare `mint_seat` in a process that
/// also serves sessions.
///
/// So the registry only answers about seats it actually issued. `ever_leased`
/// records every seat this process has handed out; `attached` records the ones
/// still held. A seat in the first and not the second is DEFINITELY gone. A seat
/// in neither is none of the registry's business, and falls through to the pid
/// probe exactly as before. Absence of evidence stays absence of evidence.
///
/// `ever_leased` grows with sessions served rather than with time, and a shared
/// daemon expires on idle, so it is bounded in practice by one daemon's
/// lifetime — noted rather than capped, because evicting an entry would
/// resurrect the very ambiguity the second set exists to remove.
#[derive(Default)]
struct SeatRegistry {
    attached: std::collections::HashSet<String>,
    ever_leased: std::collections::HashSet<String>,
}

static ATTACHED_SEATS: std::sync::OnceLock<std::sync::Mutex<SeatRegistry>> =
    std::sync::OnceLock::new();

fn registry() -> &'static std::sync::Mutex<SeatRegistry> {
    ATTACHED_SEATS.get_or_init(|| std::sync::Mutex::new(SeatRegistry::default()))
}

/// Declare that this process is serving `seat`, so liveness can answer about the
/// SESSION rather than about the server holding it.
///
/// Idempotent. Only affects seats registered through here: a seat this process
/// never issued is left to the pid probe, so registering one session cannot make
/// another process's — or an unleased — seat read as a ghost.
pub fn register_seat(seat: &str) {
    if let Ok(mut reg) = registry().lock() {
        reg.attached.insert(seat.to_string());
        reg.ever_leased.insert(seat.to_string());
    }
}

/// This session is over: its seat is no longer served, so its claims are ghosts.
///
/// Called from the handler's `Drop`, so a client that disconnects, crashes or is
/// killed releases its seat without having to say anything. Nothing is written
/// to the graph — the CLAIM stays exactly where it was, and only its liveness
/// changes, which is the property that makes a ghost claim still readable for
/// what it says while no longer counting as held.
pub fn release_seat(seat: &str) {
    let Some(lock) = ATTACHED_SEATS.get() else {
        return;
    };
    if let Ok(mut reg) = lock.lock() {
        // Removed from `attached`, KEPT in `ever_leased`: that pair is what
        // makes the next read say `gone` rather than shrugging.
        reg.attached.remove(seat);
    }
}

/// What the registry knows about `seat`, or `None` if it is not its business.
///
/// Split out so the three conditions that must ALL hold before the registry may
/// override the pid probe — it is our own pid, we are tracking, and we issued
/// this seat — read as three refusals rather than as nested ifs.
fn registry_verdict(seat: &str, pid: u32) -> Option<Liveness> {
    if pid != std::process::id() {
        return None;
    }
    let reg = ATTACHED_SEATS.get()?.lock().ok()?;
    if !reg.ever_leased.contains(seat) {
        return None;
    }
    Some(if reg.attached.contains(seat) {
        Liveness::Live
    } else {
        Liveness::Gone
    })
}

/// How many sessions this process is serving right now, or `None` if it is not
/// tracking them (a plain stdio process, which is always serving exactly itself).
///
/// `req:a-session-can-tell-it-is-not-alone`: a seat that can see `attached: 7`
/// cannot honestly report a graph-wide rollup as its own result. dev_storyflow
/// had two bosses attribute a fleet-wide movement to their own change — in the
/// flattering direction, which is the one nobody catches unaided.
pub fn attached_seat_count() -> Option<usize> {
    ATTACHED_SEATS
        .get()
        .and_then(|l| l.lock().ok())
        .map(|reg| reg.attached.len())
}

/// A seat held for exactly as long as the session holding it.
///
/// The registry is only as truthful as its removals, and "remember to release
/// on every exit path" is the kind of discipline that holds until the day a
/// client crashes instead of disconnecting. So the release is not a step anybody
/// has to remember: it is `Drop`. A session that panics, is killed, or simply
/// loses its socket releases its seat on the way out, because the handler owning
/// the lease is dropped either way.
///
/// Hold it behind an `Arc` and clone THAT within a session — cloning the lease
/// itself would mint a second identity, and a service is legitimately cloned in
/// a dozen places inside one session.
#[derive(Debug)]
pub struct SeatLease {
    seat: String,
}

impl SeatLease {
    /// Mint a seat and declare this process is serving it.
    pub fn attach() -> Self {
        let seat = mint_seat();
        register_seat(&seat);
        Self { seat }
    }

    /// The seat handle, for passing to anything that records who is working.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.seat
    }
}

impl Drop for SeatLease {
    fn drop(&mut self) {
        release_seat(&self.seat);
    }
}

/// Ask whether the session that made a claim is still working.
///
/// Computed, not remembered, wherever it can be: a pid probe cannot go stale.
/// Cross-machine is deliberately `Unknown` — a pid means nothing on a computer
/// that is not the one that minted it.
///
/// **The one case a pid cannot answer** is a seat minted by THIS process while
/// this process serves many sessions: the pid is trivially alive (it is us), so
/// it says nothing about the client. There the registry decides, and its absence
/// from the registry is a real `Gone` rather than a guess — the session was
/// registered when it attached and removed when it dropped.
pub fn seat_liveness(seat: &str) -> Liveness {
    let parts: Vec<&str> = seat.split(':').collect();
    let [host, pid, ..] = parts.as_slice() else {
        return Liveness::Unknown;
    };
    if *host != machine() || *host == "unknown-machine" {
        return Liveness::Unknown;
    }
    let Ok(pid) = pid.parse::<u32>() else {
        return Liveness::Unknown;
    };
    // A seat this process minted while serving many: the pid is us, so it proves
    // nothing about the client. Ask what we are actually holding.
    //
    // Note the deliberate asymmetry with the branch below: a seat carrying a
    // DIFFERENT pid is still answered by the probe, because a seat left behind
    // by a previous daemon (a `--stop-shared` bounce, a crash) has a pid that
    // genuinely no longer exists, and `Gone` is the true answer for it.
    // Only seats this process actually issued. One it never leased is none of
    // the registry's business and falls through to the probe below.
    if let Some(verdict) = registry_verdict(seat, pid) {
        return verdict;
    }
    if std::path::Path::new(&format!("/proc/{pid}")).exists() {
        return Liveness::Live;
    }
    // No /proc (macOS): ask ps. One spawn per distinct seat, on a report path
    // that runs when a person asks, never in a loop.
    if std::path::Path::new("/proc").exists() {
        // /proc exists but this pid is not in it: the process is genuinely gone.
        return Liveness::Gone;
    }
    match std::process::Command::new("ps")
        .args(["-p", &pid.to_string()])
        .output()
    {
        Ok(out) if out.status.success() => Liveness::Live,
        Ok(_) => Liveness::Gone,
        Err(_) => Liveness::Unknown,
    }
}

/// Take the identity of a design being imported into an empty store.
///
/// The case: `--import` (or `import_graph`) into a fresh graph is a *restore* —
/// same design, new store — and reflow2 says elsewhere that a copy of a design
/// **is** that design. If the empty graph kept the id it minted at open, the
/// round trip would not come back byte-identical, because `graph_id` is part of
/// the export's content hash. The project's own smoke test caught exactly that
/// the hour identity landed.
///
/// A graph that already holds a design keeps its own name, always. That is the
/// other half of the same rule, and it is what makes the stale-seat remedy safe:
/// absorbing the shared record into a working graph must never rename it.
///
/// Returns the adopted identity when it took, `None` when the graph kept its
/// own. **Call before importing** — the import writes under the current id.
pub fn adopt_on_import(
    graph_path: &str,
    document_graph_id: &str,
    holds_a_design: bool,
) -> Result<Option<DesignIdentity>, DynoError> {
    if holds_a_design || document_graph_id.is_empty() {
        return Ok(None);
    }
    let mut identity = resolve(graph_path, document_graph_id, || false)?;
    if identity.graph_id == document_graph_id {
        return Ok(None);
    }
    identity.graph_id = document_graph_id.to_string();
    identity.origin = Origin::Adopted;
    write(graph_path, &identity)?;
    Ok(Some(identity))
}
