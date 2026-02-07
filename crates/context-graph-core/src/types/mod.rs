//! Core domain types for the Context Graph system.

pub mod audit;
pub mod causal_relationship;
pub mod code_entity;
pub mod file_index;
pub mod fingerprint;
mod graph_edge;
mod memory_node;
mod source_metadata;

pub use causal_relationship::{
    CausalRelationship, CausalSearchResult, CausalSourceSpan, LLMProvenance, MultiEmbedderConfig,
};
pub use code_entity::{
    CodeEntity, CodeEntityType, CodeFileIndexEntry, CodeGitMetadata, CodeLanguage, CodeStats,
    Visibility,
};
pub use file_index::{FileIndexEntry, FileWatcherStats};
pub use fingerprint::{
    // SemanticFingerprint and embedding types
    EmbeddingSlice,
    SemanticFingerprint,
    // SparseVector types
    SparseVector,
    SparseVectorError,
    E10_DIM,
    E11_DIM,
    E12_TOKEN_DIM,
    E1_DIM,
    E2_DIM,
    E3_DIM,
    E4_DIM,
    E5_DIM,
    E6_SPARSE_VOCAB,
    E7_DIM,
    E8_DIM,
    E9_DIM,
    MAX_SPARSE_ACTIVE,
    SPARSE_VOCAB_SIZE,
    TOTAL_DENSE_DIMS,
};
pub use audit::{
    AuditOperation, AuditRecord, AuditResult, ConsolidationCandidate, ConsolidationRecommendation,
    EmbeddingVersionRecord, HookExecutionRecord, ImportanceChangeRecord, MergeRecord,
    RecommendationStatus,
};
pub use graph_edge::*;
pub use memory_node::*;
pub use source_metadata::{SourceMetadata, SourceType};
