//! Memory node representing a stored memory unit in the knowledge graph.
//!
//! This module provides the core types for representing knowledge in the Context Graph:
//! - [`MemoryNode`] - The primary knowledge unit with embedding, content, and metadata
//! - [`NodeMetadata`] - Rich metadata container for supplementary information
//! - [`ValidationError`] - Errors that occur during node validation
//!
//! # Constitution Compliance
//! - AP-009: All f32 fields validated (no NaN/Infinity)
//! - SEC-06: Soft delete via metadata.deleted flag
//! - Naming: snake_case fields per constitution.yaml
//!
//! # Example
//! ```
//! use context_graph_core::types::MemoryNode;
//!
//! let embedding = vec![0.1f32; 1536];
//! let mut node = MemoryNode::new("knowledge content".to_string(), embedding);
//! node.metadata.add_tag("important");
//! assert!(node.metadata.tags.contains(&"important".to_string()));
//! ```

mod metadata;
mod node;
mod validation;

#[cfg(test)]
mod tests_serialization;
#[cfg(test)]
mod tests_metadata;
#[cfg(test)]
mod tests_node;

// Re-export all public types for backwards compatibility
pub use metadata::NodeMetadata;
pub use node::MemoryNode;
pub use validation::ValidationError;

use uuid::Uuid;

/// Unique identifier for memory nodes
pub type NodeId = Uuid;

/// Embedding vector type (1536 dimensions for OpenAI-compatible)
pub type EmbeddingVector = Vec<f32>;

/// Default embedding dimension (OpenAI text-embedding-3-large compatible).
/// Per constitution.yaml: embeddings.models.E7_Code = 1536D
pub const DEFAULT_EMBEDDING_DIM: usize = 1536;

/// Maximum content size in bytes (1MB).
/// Per constitution.yaml: perf.memory constraints
pub const MAX_CONTENT_SIZE: usize = 1_048_576;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_embedding_dim_constant() {
        assert_eq!(DEFAULT_EMBEDDING_DIM, 1536);
    }

    #[test]
    fn test_max_content_size_constant() {
        assert_eq!(MAX_CONTENT_SIZE, 1_048_576);
        assert_eq!(MAX_CONTENT_SIZE, 1024 * 1024); // 1MB
    }

    #[test]
    fn test_type_aliases_compile() {
        let _id: NodeId = Uuid::new_v4();
        let _embedding: EmbeddingVector = vec![0.0; 10];
    }
}
