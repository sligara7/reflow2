//! RocksDB-backed graph storage: schema-validated nodes and edges, column
//! families for adjacency and indexes, MessagePack values, graph isolation,
//! batch writes and iterator scans.
//!
//! Absorbed verbatim from `dynograph-storage` at v0.12.0; see [`super`] for the
//! provenance block.
//!
//! ⭐ UPSTREAM RE-EXPORTED `DynoError`, `Schema` and `Value` FROM CORE HERE, and
//! that re-export is exactly what made this increment unsplittable — see
//! [`super`]. In-tree the two live in one crate, so the re-export is no longer
//! load-bearing and callers reach the vocabulary through
//! [`crate::foundation::core`] directly.

mod backend;
mod cache;
mod engine;
mod keys;

pub use cache::{CacheConfig, ReadCache};
#[cfg(feature = "fulltext")]
pub use engine::FulltextHit;
pub use engine::{StorageEngine, StoredEdge, StoredNode};
