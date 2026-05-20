//! Shared test utilities for the context-graph workspace.
//!
//! # Truth in advertising (M-H2, GH #485, 2026-05-19)
//!
//! Helpers produce real Rust types (`Vec<f32>`, `SparseVector`,
//! `TeleologicalFingerprint`) with correct dimensions and non-zero L2 norms.
//! Vector CONTENT is **pseudo-random uniform noise** drawn from a SEEDED
//! SplitMix64 RNG — it is *not* produced by a real frozen-weight embedder and
//! lacks the clustering / low-rank / language-specific structure that real
//! embedder outputs have.
//!
//! ## Suitable for
//! - Storage CRUD (store / retrieve / delete)
//! - RocksDB serialization round-trip
//! - Pipeline shape verification (correct dimensions, non-zero norms)
//! - Soft-delete and ID-based lookup behavior
//! - Boundary tests that key off `id` (Uuid) rather than content
//!
//! ## NOT suitable for
//! - Similarity-ranking-quality tests (random vectors cluster nowhere)
//! - Embedder-output property tests (no semantic structure)
//! - Clustering-aware tests
//! - Any test that gates ME-JEPA model correctness
//!
//! For those, use the production embedder pipeline against the real corpus.
//!
//! # Determinism
//!
//! Generators come in two flavors: `generate_random_*` / `create_random_*`
//! call the `_with_seed` variant with `DEFAULT_TEST_SEED` for deterministic
//! re-runs. `*_with_seed(seed: u64)` accepts an explicit seed for tests that
//! need multiple distinct-but-reproducible fingerprints.
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

// Re-export commonly used items at crate root for convenience.
pub use fingerprints::{
    create_random_fingerprint, create_random_fingerprint_with_id,
    create_random_fingerprint_with_id_and_seed, generate_random_content_hash,
    generate_random_content_hash_with_seed, generate_random_semantic_fingerprint,
    generate_random_semantic_fingerprint_with_seed, generate_random_sparse_vector,
    generate_random_sparse_vector_with_seed, generate_random_teleological_fingerprint,
    generate_random_unit_vector, generate_random_unit_vector_with_seed, hex_string,
    DEFAULT_TEST_SEED,
};
pub use stores::{create_initialized_store, create_test_store};
