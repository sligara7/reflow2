//! Schema, value and error vocabulary — the types every other module speaks.
//!
//! Absorbed verbatim from `dynograph-core` at v0.12.0; see [`super`] for the
//! full provenance block and for why this module is public where the other
//! absorbed ones are not.

mod error;
mod schema;
mod value;

pub use error::DynoError;
pub use schema::{
    EdgeEndpoint, EdgeTypeDef, ExtractionInclude, NodeTypeDef, PropertyDef, PropertyType,
    ResolutionConfig, ResolutionStrategy, Schema,
};
pub use value::Value;
