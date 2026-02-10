//! Type definitions and constants for the Entity embedding model.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use crate::gpu::BertWeights;
use crate::traits::SingleModelConfig;
use tokenizers::Tokenizer;

/// Native dimension for MiniLM entity embeddings (legacy).
/// Note: Production E11 uses KEPLER (768D) via ModelId::Kepler.
/// This legacy Entity model remains at 384D (all-MiniLM-L6-v2).
pub const ENTITY_DIMENSION: usize = 384;

/// Maximum tokens for KEPLER (standard BERT-family limit).
pub const ENTITY_MAX_TOKENS: usize = 512;

/// Latency budget in milliseconds (P95 target).
pub const ENTITY_LATENCY_BUDGET_MS: u64 = 2;

/// HuggingFace model repository name.
/// Note: This is the deprecated MiniLM model. Production uses ModelId::Kepler with KEPLER 768D.
pub const ENTITY_MODEL_NAME: &str = "sentence-transformers/all-MiniLM-L6-v2";

/// Internal state that varies based on feature flags.
#[allow(dead_code)]
pub(crate) enum ModelState {
    /// Unloaded - no weights in memory.
    Unloaded,

    /// Loaded with candle model and tokenizer (GPU-accelerated).
    Loaded {
        /// BERT model weights on GPU (boxed to reduce enum size).
        weights: Box<BertWeights>,
        /// HuggingFace tokenizer for text encoding (boxed to reduce enum size).
        tokenizer: Box<Tokenizer>,
    },
}

/// Entity embedding model (deprecated - use ModelId::Kepler for production).
///
/// This model produces 768D vectors optimized for named entity embeddings
/// and TransE-style knowledge graph operations.
///
/// # Architecture
///
/// Production uses KEPLER (RoBERTa-base + TransE) at 768D.
/// This EntityModel is legacy code using MiniLM - prefer KeplerModel for new code.
///
/// # Entity-Specific Features
///
/// - **encode_entity**: Encodes entity names with optional type context
/// - **encode_relation**: Encodes relation predicates for TransE operations
/// - **transe_score**: Computes TransE triple score
/// - **predict_tail**: Predicts tail entity from head + relation
/// - **predict_relation**: Predicts relation from head and tail
///
/// # Construction
///
/// ```rust,no_run
/// use context_graph_embeddings::models::EntityModel;
/// use context_graph_embeddings::traits::SingleModelConfig;
/// use context_graph_embeddings::error::EmbeddingResult;
/// use std::path::Path;
///
/// async fn example() -> EmbeddingResult<()> {
///     let model = EntityModel::new(
///         Path::new("models/entity"),
///         SingleModelConfig::default(),
///     )?;
///     model.load().await?;  // Must load before embed
///
///     // Encode an entity with type
///     let entity_text = EntityModel::encode_entity("Alice", Some("PERSON"));
///     // => "[PERSON] Alice"
///     Ok(())
/// }
/// ```
pub struct EntityModel {
    /// Model weights and inference engine.
    #[allow(dead_code)]
    pub(crate) model_state: std::sync::RwLock<ModelState>,

    /// Path to model weights directory.
    #[allow(dead_code)]
    pub(crate) model_path: PathBuf,

    /// Configuration for this model instance.
    #[allow(dead_code)]
    pub(crate) config: SingleModelConfig,

    /// Whether model weights are loaded and ready.
    pub(crate) loaded: AtomicBool,

    /// Memory used by model weights (bytes).
    #[allow(dead_code)]
    pub(crate) memory_size: usize,
}

// Implement Send and Sync manually since RwLock is involved
unsafe impl Send for EntityModel {}
unsafe impl Sync for EntityModel {}
