//! Stub implementations for development and testing.
//!
//! These implementations provide deterministic behavior for testing
//! and development in the Ghost System phase (Phase 0).
//!
//! # Stubs
//!
//! - [`StubEmbeddingProvider`]: Deterministic single embedding generation
//! - [`StubMultiArrayProvider`]: Deterministic 13-embedding generation (TASK-F007)
//! - [`InMemoryTeleologicalStore`]: In-memory teleological storage (TASK-F008)
//! - [`InMemoryGraphIndex`]: In-memory graph index
//!
//! # Usage
//!
//! ```
//! use context_graph_core::stubs::{InMemoryTeleologicalStore, StubMultiArrayProvider};
//! use context_graph_core::traits::{TeleologicalMemoryStore, MultiArrayEmbeddingProvider};
//!
//! let store = InMemoryTeleologicalStore::new();
//! let provider = StubMultiArrayProvider::new();
//! ```

mod embedding_stub;
mod graph_index;
mod layers;
mod multi_array_stub;
mod teleological_store_stub;
mod utl_stub;

// Single embedding stub
pub use embedding_stub::StubEmbeddingProvider;

// Graph index stub
pub use graph_index::InMemoryGraphIndex;

// Nervous layer stubs
pub use layers::{
    StubCoherenceLayer, StubLearningLayer, StubMemoryLayer, StubReflexLayer, StubSensingLayer,
};

// Multi-array embedding stub (TASK-F007)
pub use multi_array_stub::StubMultiArrayProvider;

// Teleological memory store stub (TASK-F008)
pub use teleological_store_stub::InMemoryTeleologicalStore;

// UTL processor stub
pub use utl_stub::StubUtlProcessor;
