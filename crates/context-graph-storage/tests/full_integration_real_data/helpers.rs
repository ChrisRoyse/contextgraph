//! Test Utilities — Re-exports from context-graph-test-utils (C5 deduplication).
//!
//! M-H2 (GH #485, 2026-05-19): renamed `generate_real_*` / `create_real_*` →
//! `generate_random_*` / `create_random_*`. See the test-utils crate-level
//! docstring for "suitable for / NOT suitable for" guidance.

pub use context_graph_test_utils::{
    create_initialized_store, create_random_fingerprint, create_random_fingerprint_with_id,
    generate_random_content_hash, generate_random_semantic_fingerprint,
    generate_random_sparse_vector, generate_random_unit_vector,
};
