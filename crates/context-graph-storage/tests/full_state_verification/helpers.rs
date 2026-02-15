//! Helper functions for Full State Verification tests.
//!
//! Re-exports shared utilities from context-graph-test-utils (C5 deduplication).

pub use context_graph_test_utils::{
    create_test_store, generate_real_semantic_fingerprint, generate_real_sparse_vector,
    generate_real_teleological_fingerprint, generate_real_unit_vector, hex_string,
};
