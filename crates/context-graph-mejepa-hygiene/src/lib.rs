// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).
//! ME-JEPA Phase 6 hygiene operations.
//!
//! The crate owns fail-closed storage hygiene: tier transitions, quota
//! enforcement, old witness-chain Merkle compression, and nightly GC reports.

pub mod categories;
pub mod entry;
pub mod error;
pub mod gc;
pub mod mcp;
pub mod ops;
pub mod quota;
pub mod quota_types;
pub mod reports;
pub mod retention;
pub mod storage;
pub mod tier;
pub mod tier_frozen;
pub mod tier_ops;
pub mod witness_compress;

pub use categories::*;
pub use entry::*;
pub use error::*;
pub use gc::*;
pub use mcp::*;
pub use ops::*;
pub use quota::*;
pub use quota_types::*;
pub use reports::*;
pub use retention::*;
pub use storage::*;
pub use tier::*;
pub use tier_frozen::*;
pub use tier_ops::*;
pub use witness_compress::*;
