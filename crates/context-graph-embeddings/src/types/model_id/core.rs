//! Core ModelId enum definition and primary methods.

use serde::{Deserialize, Serialize};

use super::tokenizer::TokenizerFamily;

/// Identifies one of the 13 embedding models in the Multi-Array Storage pipeline.
///
/// # Variants
///
/// | Variant | Model | Dimension | Type |
/// |---------|-------|-----------|------|
/// | Semantic | e5-large-v2 | 1024 | Pretrained |
/// | TemporalRecent | Exponential decay | 512 | Custom |
/// | TemporalPeriodic | Fourier basis | 512 | Custom |
/// | TemporalPositional | Sinusoidal PE | 512 | Custom |
/// | Causal | nomic-embed-v1.5 | 768 | Pretrained |
/// | Sparse | SPLADE | ~30K sparse | Pretrained |
/// | Code | Qodo-Embed-1-1.5B | 1536 | Pretrained |
/// | Graph | e5-large-v2 | 1024 | Pretrained |
/// | Hdc | Hyperdimensional | 10K-bit | Custom |
/// | Multimodal | CLIP | 768 | Pretrained |
/// | Entity | KEPLER | 768 | Pretrained (DEPRECATED - use Kepler) |
/// | Kepler | KEPLER (RoBERTa + TransE) | 768 | Pretrained |
/// | LateInteraction | ColBERT | 128/token | Pretrained |
/// | Splade | SPLADE v3 | ~30K sparse | Pretrained |
///
/// # Example
///
/// ```rust
/// use context_graph_embeddings::types::ModelId;
///
/// let model = ModelId::Semantic;
/// assert_eq!(model.dimension(), 1024);
/// assert!(model.is_pretrained());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum ModelId {
    /// E1: Semantic embedding using intfloat/e5-large-v2 (1024D)
    Semantic = 0,
    /// E2: Temporal recency using exponential decay (512D, custom)
    TemporalRecent = 1,
    /// E3: Temporal periodicity using Fourier basis (512D, custom)
    TemporalPeriodic = 2,
    /// E4: Temporal position using sinusoidal encoding (512D, custom)
    TemporalPositional = 3,
    /// E5: Causal embedding using nomic-ai/nomic-embed-text-v1.5 (768D)
    Causal = 4,
    /// E6: Sparse lexical using naver/splade-cocondenser (~30K sparse -> 1536D projected)
    Sparse = 5,
    /// E7: Code embedding using Qodo-Embed-1-1.5B (1536D native)
    Code = 6,
    /// E8: Graph/sentence using intfloat/e5-large-v2 (1024D, upgraded from MiniLM 384D)
    Graph = 7,
    /// E9: Hyperdimensional computing (10K-bit -> 1024D projected, custom)
    Hdc = 8,
    /// E10: Multimodal using openai/clip-vit-large-patch14 (768D)
    Multimodal = 9,
    /// E11: Entity using KEPLER (768D, upgraded from MiniLM 384D)
    /// DEPRECATED: Use Kepler ModelId instead for meaningful TransE operations.
    Entity = 10,
    /// E12: Late interaction using colbert-ir/colbertv2.0 (128D per token)
    LateInteraction = 11,
    /// E13: SPLADE v3 sparse embedding for Stage 1 recall in 5-stage pipeline
    Splade = 12,
    /// E11 (new): KEPLER - RoBERTa-base trained with TransE on Wikidata5M (768D)
    /// Replaces Entity (all-MiniLM-L6-v2) for meaningful knowledge graph operations.
    Kepler = 13,
}

impl ModelId {
    /// Returns the native output dimension of this model BEFORE any projection.
    ///
    /// Note: Sparse (30K) and Hdc (10K-bit) are projected to smaller dimensions
    /// in downstream processing. This returns the raw model output size.
    #[must_use]
    pub const fn dimension(&self) -> usize {
        match self {
            Self::Semantic => 1024,
            Self::TemporalRecent => 512,
            Self::TemporalPeriodic => 512,
            Self::TemporalPositional => 512,
            Self::Causal => 768,
            Self::Sparse => 30522, // SPLADE vocab size
            Self::Code => 1536,    // Qodo-Embed-1-1.5B native dimension
            Self::Graph => 1024,   // e5-large-v2 (upgraded from MiniLM 384D)
            Self::Hdc => 10000,    // 10K-bit vector
            Self::Multimodal => 768,
            Self::Entity => 384,   // Legacy MiniLM-L6-v2 (production E11 uses Kepler 768D)
            Self::LateInteraction => 128, // Per-token dimension
            Self::Splade => 30522,        // SPLADE v3 vocab size
            Self::Kepler => 768,          // KEPLER (RoBERTa-base)
        }
    }

    /// Returns the projected dimension used after normalization (for multi-array storage input).
    ///
    /// All models are normalized to these dimensions before concatenation:
    /// - Most models: native dimension (no projection needed)
    /// - Sparse: 1536 (projected from 30K sparse)
    /// - Hdc: 1024 (projected from 10K-bit)
    /// - LateInteraction: pooled to single 128D vector
    /// - Splade: 30K sparse -> 1536D projected
    #[must_use]
    pub const fn projected_dimension(&self) -> usize {
        match self {
            Self::Sparse => 1536,  // 30K -> 1536 via learned projection
            Self::Hdc => 1024,     // 10K-bit -> 1024 via projection
            Self::Splade => 1536,  // 30K -> 1536 via learned projection (same as E6)
            Self::Kepler => 768,   // No projection needed
            _ => self.dimension(), // No projection needed (Code is now native 1536D)
        }
    }

    /// Returns true if this model requires custom implementation (no pretrained weights).
    #[must_use]
    pub const fn is_custom(&self) -> bool {
        matches!(
            self,
            Self::TemporalRecent | Self::TemporalPeriodic | Self::TemporalPositional | Self::Hdc
        )
    }

    /// Returns true if this model uses pretrained weights from HuggingFace.
    #[must_use]
    pub const fn is_pretrained(&self) -> bool {
        !self.is_custom()
    }

    /// Returns the maximum input token count for this model.
    ///
    /// # Returns
    /// - Code (Qodo-Embed): 32768 tokens (Qwen2's extended context)
    /// - Causal (nomic-embed): 512 tokens (capped from 8192 for causal use)
    /// - CLIP (Multimodal): 77 tokens
    /// - Most others: 512 tokens
    /// - Custom models: effectively unlimited (no tokenization)
    #[must_use]
    pub const fn max_tokens(&self) -> usize {
        match self {
            Self::Code => 32768,    // Qodo-Embed-1-1.5B (Qwen2) supports 32K context
            Self::Causal => 512,    // nomic-embed-text-v1.5 (capped from 8192)
            Self::Multimodal => 77, // CLIP text encoder limit
            Self::TemporalRecent
            | Self::TemporalPeriodic
            | Self::TemporalPositional
            | Self::Hdc => usize::MAX, // Custom models: no token limit
            Self::Splade => 512,    // SPLADE v3 uses BERT tokenizer limit
            Self::Kepler => 512,    // KEPLER uses RoBERTa tokenizer limit
            _ => 512,               // Standard BERT-family limit
        }
    }

    /// Returns the tokenizer family for shared tokenizer caching.
    ///
    /// Models using the same tokenizer family can share tokenization results.
    /// See M03-L29 (TokenizationManager) for usage.
    #[must_use]
    pub const fn tokenizer_family(&self) -> TokenizerFamily {
        match self {
            Self::Semantic => TokenizerFamily::BertWordpiece, // e5 uses BERT tokenizer
            Self::Causal => TokenizerFamily::BertWordpiece,    // nomic-embed uses BERT tokenizer
            Self::Sparse => TokenizerFamily::BertWordpiece,   // SPLADE uses BERT
            Self::Code => TokenizerFamily::BertWordpiece,     // Qodo-Embed uses BERT tokenizer
            Self::Graph => TokenizerFamily::BertWordpiece,    // MiniLM uses BERT
            Self::Multimodal => TokenizerFamily::ClipBpe,     // CLIP has its own BPE
            Self::Entity => TokenizerFamily::BertWordpiece,   // all-MiniLM uses BERT
            Self::Kepler => TokenizerFamily::RobertaBpe,      // KEPLER uses RoBERTa/GPT-2 BPE
            Self::LateInteraction => TokenizerFamily::BertWordpiece, // ColBERT uses BERT
            Self::Splade => TokenizerFamily::BertWordpiece,   // SPLADE v3 uses BERT
            Self::TemporalRecent
            | Self::TemporalPeriodic
            | Self::TemporalPositional
            | Self::Hdc => TokenizerFamily::None, // Custom: no tokenizer
        }
    }

    /// Returns all 13 model variants in pipeline order.
    ///
    /// Order matches the E1-E13 specification in constitution.yaml.
    #[must_use]
    pub const fn all() -> &'static [ModelId] {
        &[
            Self::Semantic,           // E1
            Self::TemporalRecent,     // E2
            Self::TemporalPeriodic,   // E3
            Self::TemporalPositional, // E4
            Self::Causal,             // E5
            Self::Sparse,             // E6
            Self::Code,               // E7
            Self::Graph,              // E8
            Self::Hdc,                // E9
            Self::Multimodal,         // E10
            Self::Entity,             // E11
            Self::LateInteraction,    // E12
            Self::Splade,             // E13
        ]
    }

    /// Returns only pretrained models (require weight loading).
    #[must_use = "this returns an iterator that must be consumed"]
    pub fn pretrained() -> impl Iterator<Item = ModelId> {
        Self::all().iter().copied().filter(|m| m.is_pretrained())
    }

    /// Returns only custom models (require implementation, no weights).
    #[must_use = "this returns an iterator that must be consumed"]
    pub fn custom() -> impl Iterator<Item = ModelId> {
        Self::all().iter().copied().filter(|m| m.is_custom())
    }

    /// Latency budget in milliseconds from constitution.yaml.
    #[must_use]
    pub const fn latency_budget_ms(&self) -> u32 {
        match self {
            Self::Semantic => 5,
            Self::TemporalRecent | Self::TemporalPeriodic | Self::TemporalPositional => 2,
            Self::Causal => 8,
            Self::Sparse => 3,
            Self::Code => 10,
            Self::Graph => 5,
            Self::Hdc => 1,
            Self::Multimodal => 15,
            Self::Entity => 2,
            Self::Kepler => 5,          // Slightly slower than Entity due to larger model
            Self::LateInteraction => 8,
            Self::Splade => 3, // Similar to E6 Sparse
        }
    }

    /// Returns the string representation of the model ID for configuration.
    ///
    /// These names are used in config files (e.g., `preload_models = ["semantic", "code"]`).
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Semantic => "semantic",
            Self::TemporalRecent => "temporal_recent",
            Self::TemporalPeriodic => "temporal_periodic",
            Self::TemporalPositional => "temporal_positional",
            Self::Causal => "causal",
            Self::Sparse => "sparse",
            Self::Code => "code",
            Self::Graph => "graph",
            Self::Hdc => "hdc",
            Self::Multimodal => "multimodal",
            Self::Entity => "entity",
            Self::Kepler => "kepler",
            Self::LateInteraction => "late_interaction",
            Self::Splade => "splade",
        }
    }
}
