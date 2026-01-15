// crates/context-graph-core/src/gwt/session_identity/mod.rs
//! Session Identity Persistence
//!
//! Provides cross-session identity continuity via SessionIdentitySnapshot.
//!
//! # Architecture
//! - SessionIdentitySnapshot: Core serializable struct (<30KB)
//! - IdentityCache: Thread-safe singleton for PreToolUse hot path
//! - Persists to RocksDB CF_SESSION_IDENTITY (TASK-SESSION-04)
//! - Used by CLI hooks for identity restore/persist (TASK-SESSION-12, TASK-SESSION-13)
//!
//! # Constitution Reference
//! - IDENTITY-002: IC thresholds
//! - GWT-003: Identity continuity
//! - AP-25: 13 oscillators

mod cache;
mod types;

pub use cache::{clear_cache, update_cache, IdentityCache};
pub use types::{SessionIdentitySnapshot, KURAMOTO_N, MAX_TRAJECTORY_LEN};
