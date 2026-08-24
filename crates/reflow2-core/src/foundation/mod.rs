//! The substrate everything else in reflow2 is built on: the schema and value
//! vocabulary, the RocksDB-backed graph store, and the full-text index.
//!
//! # Provenance
//!
//! ABSORBED FROM `dynograph-foundation` at tag **v0.12.0**, increment 4 of
//! `dec:absorb-the-foundation-subset-and-end-the-dependency` — the increment
//! that ENDS the dependency. Recorded here because a later reader has no other
//! way to find where this came from:
//!
//! ```text
//! repo     https://github.com/sligara7/dynograph-foundation
//! tag      v0.12.0
//! core     crates/dynograph-core/src/{schema,value,error}.rs      @ 91ac47780867cd9a3368021e3c0f99f4085762c1
//! store    crates/dynograph-storage/src/{backend,cache,keys}.rs
//!          crates/dynograph-storage/src/engine/*.rs               @ b5a9f7416eb00fa99b36923cbbe3045fb7f22012
//! text     crates/dynograph-text/src/lib.rs                       @ b5a9f7416eb00fa99b36923cbbe3045fb7f22012
//! taken    verbatim, with their tests
//! licence  MIT, Copyright (c) 2026 Anthony Sligar
//! ```
//!
//! The header is a REQUIREMENT of that decision rather than a courtesy: the
//! recorded objection to absorbing anything is that vendoring converts a
//! visible dependency into an INVISIBLE one — the version pin carried a written
//! reason for every bump, and in-tree code has no successor to that record.
//! This block is that successor.
//!
//! # 🛑 Why this increment could not be split, though the plan said it could
//!
//! The plan promised five increments, "each independently shippable". That was
//! true of the first three and FALSE here, and the correction is
//! `chg:increments-4-and-5-are-one-increment`. `dynograph-storage` re-exported
//! `DynoError`, `Schema` and `Value` from `dynograph-core`, and
//! `StoredNode.properties` is a `HashMap<String, Value>` of that very type. Take
//! core in-tree while storage stays external and reflow2 has TWO types called
//! `Value` and TWO called `DynoError` — 33 files naming one, 48 naming the
//! other, and nothing compiles.
//!
//! ⭐ **A dependency graph tells you what needs what to BUILD; it does not tell
//! you what breaks if you take one and not the other.** The scoping measured
//! sizes, import counts and dependency direction, and never asked whether two
//! crates share types across the boundary. The signal was two import lines wide.
//!
//! # Why this module is PUBLIC, unlike the other absorbed ones
//!
//! `stats`, `fuzzy` and `graphalg` are all `pub(crate)`, on the ground that
//! `ifc:core-api` already records 277 public functions growing by default and
//! absorbing code is no reason to widen a surface already too wide.
//!
//! **That argument does not reach here, because these types are ALREADY in the
//! public surface.** `lib.rs` re-exported `DynoError`, `Schema`, `Value`,
//! `StoredNode` and `StoredEdge` before this change, and `reflow2-mcp` names
//! `DynoError` 35 times and `StoredNode` 21 times. Making them private would be
//! a breaking change dressed as tidiness.
//!
//! # The external crates this brought with it
//!
//! `rocksdb`, `tantivy`, `rmp-serde`, `lru`, `serde_yaml`, `uuid`, `thiserror`.
//! ⚠️ `rocksdb` is pinned at **0.24, absorbed verbatim** — it is the
//! historically-unmaintained crate, and `rust-rocksdb` is the maintained one.
//! Switching is deliberately NOT part of this change:
//! `dec:absorb-rocksdb-024-unchanged-then-switch-separately` keeps the
//! migration to one variable so a failure has one cause.

pub mod core;
pub mod store;
#[cfg(feature = "fulltext")]
pub mod text;
