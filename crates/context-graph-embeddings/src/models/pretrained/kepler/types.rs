//! Type definitions and constants for the KEPLER embedding model.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use crate::gpu::BertWeights;
use crate::traits::SingleModelConfig;
use tokenizers::Tokenizer;

/// Native dimension for KEPLER (RoBERTa-base) embeddings.
/// This is 768D, double the previous MiniLM 384D.
pub const KEPLER_DIMENSION: usize = 768;

/// Maximum tokens for RoBERTa (standard BERT-family limit).
pub const KEPLER_MAX_TOKENS: usize = 512;

/// Latency budget in milliseconds (P95 target).
/// Slightly higher than MiniLM due to larger model (12 layers vs 6).
pub const KEPLER_LATENCY_BUDGET_MS: u64 = 5;

/// Model name for logging and identification.
pub const KEPLER_MODEL_NAME: &str = "KEPLER (RoBERTa-base + TransE)";

/// Internal state that varies based on loading status.
#[allow(dead_code)]
pub(crate) enum ModelState {
    /// Unloaded - no weights in memory.
    Unloaded,

    /// Loaded with candle model and tokenizer (GPU-accelerated).
    Loaded {
        /// BERT/RoBERTa model weights on GPU (boxed to reduce enum size).
        weights: Box<BertWeights>,
        /// RoBERTa tokenizer (GPT-2 BPE) for text encoding (boxed to reduce enum size).
        tokenizer: Box<Tokenizer>,
    },
}

/// KEPLER entity embedding model.
///
/// KEPLER combines RoBERTa-base with TransE training on Wikidata5M.
/// Unlike the previous all-MiniLM-L6-v2 model, KEPLER produces embeddings
/// where TransE operations (`h + r ≈ t`) are semantically meaningful.
///
/// # Architecture
///
/// - Base: RoBERTa-base (12 layers, 768D hidden, 12 heads)
/// - Training: Pre-trained on Wikipedia + BookCorpus, then fine-tuned with
///   TransE objective on Wikidata5M (4.8M entities, 20M triples)
/// - Tokenizer: GPT-2 BPE (same as RoBERTa)
///
/// # TransE Training
///
/// KEPLER was trained with the TransE objective:
/// - Positive triples (h, r, t): minimize ||h + r - t||₂
/// - Negative triples: maximize ||h + r - t||₂
///
/// This means TransE operations produce meaningful scores:
/// - Valid triple: score > -5.0 (close to 0)
/// - Invalid triple: score < -10.0 (large negative)
///
/// # Entity-Specific Features
///
/// - **encode_entity**: Encodes entity names with optional type context
/// - **encode_relation**: Encodes relation predicates for TransE operations
/// - **transe_score**: Computes TransE triple score (now meaningful!)
/// - **predict_tail**: Predicts tail entity from head + relation
/// - **predict_relation**: Predicts relation from head and tail
///
/// # Construction
///
/// ```rust,no_run
/// use context_graph_embeddings::models::kepler::KeplerModel;
/// use context_graph_embeddings::traits::SingleModelConfig;
/// use context_graph_embeddings::error::EmbeddingResult;
/// use std::path::Path;
///
/// async fn example() -> EmbeddingResult<()> {
///     let model = KeplerModel::new(
///         Path::new("models/kepler"),
///         SingleModelConfig::default(),
///     )?;
///     model.load().await?;  // Must load before embed
///
///     // Encode an entity with type
///     let entity_text = KeplerModel::encode_entity("Paris", Some("LOCATION"));
///     // => "[LOCATION] Paris"
///     Ok(())
/// }
/// ```
pub struct KeplerModel {
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
unsafe impl Send for KeplerModel {}
unsafe impl Sync for KeplerModel {}
