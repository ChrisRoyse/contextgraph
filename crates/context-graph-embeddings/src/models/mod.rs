//! Model management and registry module.
//!
//! This module provides the core infrastructure for managing embedding model lifecycle:
//! - `ModelRegistry`: Central manager for all 12 embedding models
//! - `MemoryTracker`: Tracks GPU memory allocation to prevent OOM
//! - `ModelRegistryConfig`: Configuration for registry behavior
//!
//! # Architecture
//!
//! ```text
//! ModelRegistry
//! ├── models: RwLock<HashMap<ModelId, Arc<dyn EmbeddingModel>>>
//! ├── memory_tracker: RwLock<MemoryTracker>
//! ├── loading_locks: HashMap<ModelId, Arc<Semaphore>> (per-model)
//! ├── factory: Arc<dyn ModelFactory>
//! └── stats: RwLock<RegistryStatsInternal>
//! ```
//!
//! # Design Principles
//!
//! - **NO FALLBACKS**: Errors propagate immediately with full context
//! - **FAIL FAST**: Invalid state = immediate EmbeddingError
//! - **NO MOCK DATA**: All tests use real ModelFactory implementations
//! - **THREAD SAFE**: All operations safe for concurrent access
//!
//! # Example
//!
//! ```rust,no_run
//! use context_graph_embeddings::models::{ModelRegistry, ModelRegistryConfig};
//! use context_graph_embeddings::traits::ModelFactory;
//! use context_graph_embeddings::error::EmbeddingResult;
//! use context_graph_embeddings::types::ModelId;
//! use std::sync::Arc;
//!
//! async fn example(factory: Arc<dyn ModelFactory>) -> EmbeddingResult<()> {
//!     let config = ModelRegistryConfig::default();
//!     let registry = ModelRegistry::new(config, factory).await?;
//!     registry.initialize().await?;
//!
//!     // Lazy load on first access
//!     let model = registry.get_model(ModelId::Semantic).await?;
//!     // model.embed() requires EmbeddingInput - see EmbeddingModel trait docs
//!
//!     Ok(())
//! }
//! ```

pub mod attention;
pub mod custom;
mod factory;
mod memory_tracker;
pub mod pretrained;
mod registry;

pub use custom::{
    periods, TemporalPeriodicModel, TemporalPositionalModel, TemporalRecentModel, DEFAULT_BASE,
    DEFAULT_DECAY_RATES, DEFAULT_PERIODS, FEATURES_PER_PERIOD, TEMPORAL_PERIODIC_DIMENSION,
    TEMPORAL_POSITIONAL_DIMENSION, TEMPORAL_RECENT_DIMENSION,
};
pub use factory::DefaultModelFactory;
pub use memory_tracker::MemoryTracker;
pub use pretrained::{
    validate_late_interaction_batch_vram_budget,
    // SparseModel (E6) - SPLADE
    validate_true_batch_vram_budget,
    // BgeM3DenseModel (E14) - BAAI/bge-m3 dense head
    BgeM3DenseModel,
    // CausalModel (E5) - nomic-embed-text-v1.5
    CausalModel,
    // CodeModel (E7) - Qodo-Embed-1-1.5B (Qwen2-based)
    CodeModel,
    // ContextualModel (E10) - intfloat/e5-base-v2
    ContextualModel,
    // GraphModel (E8) - sentence-transformers/all-MiniLM-L6-v2
    GraphModel,
    // KeplerModel (E11) - RoBERTa-base + TransE on Wikidata5M (768D)
    KeplerModel,
    LateInteractionBatchVramPlan,
    // LateInteractionModel (E12) - ColBERT
    LateInteractionModel,
    // SemanticModel (E1) - intfloat/e5-large-v2
    SemanticModel,
    SparseBatchVramPlan,
    SparseModel,
    SparseVector,
    TokenEmbeddings,
    BGE_M3_DENSE_DIMENSION,
    BGE_M3_DENSE_LATENCY_BUDGET_MS,
    BGE_M3_DENSE_MAX_TOKENS,
    CAUSAL_DIMENSION,
    CAUSAL_LATENCY_BUDGET_MS,
    CAUSAL_MAX_TOKENS,
    CODE_LATENCY_BUDGET_MS,
    CODE_MAX_TOKENS,
    CODE_MODEL_NAME,
    CODE_NATIVE_DIMENSION,
    CODE_PROJECTED_DIMENSION,
    GRAPH_DIMENSION,
    GRAPH_LATENCY_BUDGET_MS,
    GRAPH_MAX_TOKENS,
    GRAPH_MODEL_NAME,
    // KEPLER (E11) constants
    KEPLER_DIMENSION,
    KEPLER_LATENCY_BUDGET_MS,
    KEPLER_MAX_TOKENS,
    KEPLER_MODEL_NAME,
    LATE_INTERACTION_DIMENSION,
    LATE_INTERACTION_LATENCY_BUDGET_MS,
    LATE_INTERACTION_MAX_TOKENS,
    LATE_INTERACTION_MODEL_NAME,
    MAX_CONTEXT_NEIGHBORS,
    PASSAGE_PREFIX,
    QUERY_PREFIX,
    SEMANTIC_DIMENSION,
    SEMANTIC_LATENCY_BUDGET_MS,
    SEMANTIC_MAX_TOKENS,
    SPARSE_EXPECTED_SPARSITY,
    SPARSE_LATENCY_BUDGET_MS,
    SPARSE_MAX_TOKENS,
    SPARSE_MODEL_NAME,
    SPARSE_NATIVE_DIMENSION,
    SPARSE_PROJECTED_DIMENSION,
    SPARSE_VOCAB_SIZE,
};
pub use registry::{ModelRegistry, ModelRegistryConfig, RegistryStats, RegistryStatsInternal};
