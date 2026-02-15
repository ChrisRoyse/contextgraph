//! Shared test utilities for the context-graph workspace.
//!
//! Consolidates duplicated test helpers (fingerprint generators, store factories,
//! data generation) into a single crate. ALL data is REAL — no mocks.
//!
//! # Usage
//!
//! Add to your crate's `[dev-dependencies]`:
//! ```toml
//! [dev-dependencies]
//! context-graph-test-utils = { path = "../context-graph-test-utils" }
//! ```

pub mod fingerprints;
pub mod stores;

// Re-export commonly used items at crate root for convenience
pub use fingerprints::{
    create_real_fingerprint, create_real_fingerprint_with_id, generate_real_content_hash,
    generate_real_semantic_fingerprint, generate_real_sparse_vector,
    generate_real_teleological_fingerprint, generate_real_unit_vector, hex_string,
};
pub use stores::{create_initialized_store, create_test_store};
