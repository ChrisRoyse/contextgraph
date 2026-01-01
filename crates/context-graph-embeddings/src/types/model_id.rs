//! ModelId enum identifying the 12 embedding models in the pipeline.
//!
//! Each variant maps to a specific model architecture with defined dimensions.
//! Custom models (Temporal*, Hdc) are implemented from scratch.
//! Pretrained models load weights from HuggingFace repositories.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Identifies one of the 12 embedding models in the fusion pipeline.
///
/// # Variants
///
/// | Variant | Model | Dimension | Type |
/// |---------|-------|-----------|------|
/// | Semantic | e5-large-v2 | 1024 | Pretrained |
/// | TemporalRecent | Exponential decay | 512 | Custom |
/// | TemporalPeriodic | Fourier basis | 512 | Custom |
/// | TemporalPositional | Sinusoidal PE | 512 | Custom |
/// | Causal | Longformer | 768 | Pretrained |
/// | Sparse | SPLADE | ~30K sparse | Pretrained |
/// | Code | CodeT5p | 256 embed | Pretrained |
/// | Graph | paraphrase-MiniLM | 384 | Pretrained |
/// | Hdc | Hyperdimensional | 10K-bit | Custom |
/// | Multimodal | CLIP | 768 | Pretrained |
/// | Entity | all-MiniLM | 384 | Pretrained |
/// | LateInteraction | ColBERT | 128/token | Pretrained |
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
    /// E5: Causal embedding using allenai/longformer-base-4096 (768D)
    Causal = 4,
    /// E6: Sparse lexical using naver/splade-cocondenser (~30K sparse -> 1536D projected)
    Sparse = 5,
    /// E7: Code embedding using Salesforce/codet5p-110m-embedding (256D embed, 768D internal)
    /// Note: PRD says 1536D - that's the projected dimension after learned projection layer
    Code = 6,
    /// E8: Graph/sentence using sentence-transformers/paraphrase-MiniLM-L6-v2 (384D)
    Graph = 7,
    /// E9: Hyperdimensional computing (10K-bit -> 1024D projected, custom)
    Hdc = 8,
    /// E10: Multimodal using openai/clip-vit-large-patch14 (768D)
    Multimodal = 9,
    /// E11: Entity using sentence-transformers/all-MiniLM-L6-v2 (384D)
    Entity = 10,
    /// E12: Late interaction using colbert-ir/colbertv2.0 (128D per token)
    LateInteraction = 11,
}

impl ModelId {
    /// Returns the native output dimension of this model BEFORE any projection.
    ///
    /// Note: Sparse (30K), Hdc (10K-bit), and Code (256) are projected to larger dimensions
    /// in downstream processing. This returns the raw model output size.
    ///
    /// # Returns
    /// - Semantic: 1024
    /// - Temporal*: 512 (custom implementations)
    /// - Causal: 768
    /// - Sparse: 30522 (vocab size, 5% active)
    /// - Code: 256 (CodeT5p embed_dim, projects to 768)
    /// - Graph: 384
    /// - Hdc: 10000 (bit vector)
    /// - Multimodal: 768
    /// - Entity: 384
    /// - LateInteraction: 128 (per token)
    #[must_use]
    pub const fn dimension(&self) -> usize {
        match self {
            Self::Semantic => 1024,
            Self::TemporalRecent => 512,
            Self::TemporalPeriodic => 512,
            Self::TemporalPositional => 512,
            Self::Causal => 768,
            Self::Sparse => 30522, // SPLADE vocab size
            Self::Code => 256,     // CodeT5p embed_dim (internal d_model=768)
            Self::Graph => 384,
            Self::Hdc => 10000, // 10K-bit vector
            Self::Multimodal => 768,
            Self::Entity => 384,
            Self::LateInteraction => 128, // Per-token dimension
        }
    }

    /// Returns the projected dimension used after normalization (for FuseMoE input).
    ///
    /// All models are normalized to these dimensions before concatenation:
    /// - Most models: native dimension (no projection needed)
    /// - Sparse: 1536 (projected from 30K sparse)
    /// - Code: 768 (projected from 256 embed_dim)
    /// - Hdc: 1024 (projected from 10K-bit)
    /// - LateInteraction: pooled to single 128D vector
    #[must_use]
    pub const fn projected_dimension(&self) -> usize {
        match self {
            Self::Sparse => 1536,  // 30K -> 1536 via learned projection
            Self::Code => 768,     // 256 embed -> 768 via projection (CodeT5p internal dim)
            Self::Hdc => 1024,     // 10K-bit -> 1024 via projection
            _ => self.dimension(), // No projection needed
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

    /// Returns the HuggingFace repository name for pretrained models.
    ///
    /// # Returns
    /// - `Some("repo/name")` for pretrained models
    /// - `None` for custom implementations
    #[must_use]
    pub const fn model_repo(&self) -> Option<&'static str> {
        match self {
            Self::Semantic => Some("intfloat/e5-large-v2"),
            Self::Causal => Some("allenai/longformer-base-4096"),
            Self::Sparse => Some("naver/splade-cocondenser-ensembledistil"),
            Self::Code => Some("Salesforce/codet5p-110m-embedding"),
            Self::Graph => Some("sentence-transformers/paraphrase-MiniLM-L6-v2"),
            Self::Multimodal => Some("openai/clip-vit-large-patch14"),
            Self::Entity => Some("sentence-transformers/all-MiniLM-L6-v2"),
            Self::LateInteraction => Some("colbert-ir/colbertv2.0"),
            Self::TemporalRecent
            | Self::TemporalPeriodic
            | Self::TemporalPositional
            | Self::Hdc => None,
        }
    }

    /// Returns the local directory name for this model's files.
    ///
    /// Maps to the subdirectory under the models base path.
    #[must_use]
    pub const fn directory_name(&self) -> &'static str {
        match self {
            Self::Semantic => "semantic",
            Self::TemporalRecent | Self::TemporalPeriodic | Self::TemporalPositional => "temporal",
            Self::Causal => "causal",
            Self::Sparse => "sparse",
            Self::Code => "code",
            Self::Graph => "graph",
            Self::Hdc => "hdc",
            Self::Multimodal => "multimodal",
            Self::Entity => "entity",
            Self::LateInteraction => "late-interaction",
        }
    }

    /// Constructs the full path to this model's directory.
    ///
    /// # Arguments
    /// * `base_dir` - Base directory containing all model subdirectories
    ///
    /// # Example
    /// ```rust
    /// use std::path::Path;
    /// use context_graph_embeddings::types::ModelId;
    ///
    /// let path = ModelId::Semantic.model_path(Path::new("/models"));
    /// assert_eq!(path, Path::new("/models/semantic"));
    /// ```
    #[must_use]
    pub fn model_path(&self, base_dir: &Path) -> PathBuf {
        base_dir.join(self.directory_name())
    }

    /// Returns the maximum input token count for this model.
    ///
    /// # Returns
    /// - Causal (Longformer): 4096 tokens
    /// - CLIP (Multimodal): 77 tokens
    /// - Most others: 512 tokens
    /// - Custom models: effectively unlimited (no tokenization)
    #[must_use]
    pub const fn max_tokens(&self) -> usize {
        match self {
            Self::Causal => 4096,      // Longformer's extended context
            Self::Multimodal => 77,    // CLIP text encoder limit
            Self::TemporalRecent
            | Self::TemporalPeriodic
            | Self::TemporalPositional
            | Self::Hdc => usize::MAX, // Custom models: no token limit
            _ => 512,                  // Standard BERT-family limit
        }
    }

    /// Returns the tokenizer family for shared tokenizer caching.
    ///
    /// Models using the same tokenizer family can share tokenization results.
    /// See M03-L29 (TokenizationManager) for usage.
    #[must_use]
    pub const fn tokenizer_family(&self) -> TokenizerFamily {
        match self {
            Self::Semantic => TokenizerFamily::BertWordpiece,    // e5 uses BERT tokenizer
            Self::Causal => TokenizerFamily::RobertaBpe,         // Longformer uses RoBERTa
            Self::Sparse => TokenizerFamily::BertWordpiece,      // SPLADE uses BERT
            Self::Code => TokenizerFamily::SentencePieceBpe,     // CodeT5p uses SentencePiece
            Self::Graph => TokenizerFamily::BertWordpiece,       // MiniLM uses BERT
            Self::Multimodal => TokenizerFamily::ClipBpe,        // CLIP has its own BPE
            Self::Entity => TokenizerFamily::BertWordpiece,      // all-MiniLM uses BERT
            Self::LateInteraction => TokenizerFamily::BertWordpiece, // ColBERT uses BERT
            Self::TemporalRecent
            | Self::TemporalPeriodic
            | Self::TemporalPositional
            | Self::Hdc => TokenizerFamily::None, // Custom: no tokenizer
        }
    }

    /// Returns all 12 model variants in pipeline order.
    ///
    /// Order matches the E1-E12 specification in constitution.yaml.
    #[must_use]
    pub const fn all() -> &'static [ModelId] {
        &[
            Self::Semantic,          // E1
            Self::TemporalRecent,    // E2
            Self::TemporalPeriodic,  // E3
            Self::TemporalPositional, // E4
            Self::Causal,            // E5
            Self::Sparse,            // E6
            Self::Code,              // E7
            Self::Graph,             // E8
            Self::Hdc,               // E9
            Self::Multimodal,        // E10
            Self::Entity,            // E11
            Self::LateInteraction,   // E12
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
            Self::LateInteraction => 8,
        }
    }

    /// Returns the string representation of the model ID for configuration.
    ///
    /// These names are used in config files (e.g., `preload_models = ["semantic", "code"]`).
    ///
    /// # Example
    /// ```rust
    /// use context_graph_embeddings::types::ModelId;
    ///
    /// assert_eq!(ModelId::Semantic.as_str(), "semantic");
    /// assert_eq!(ModelId::LateInteraction.as_str(), "late_interaction");
    /// ```
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
            Self::LateInteraction => "late_interaction",
        }
    }
}

impl std::fmt::Display for ModelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::Semantic => "Semantic (E1)",
            Self::TemporalRecent => "TemporalRecent (E2)",
            Self::TemporalPeriodic => "TemporalPeriodic (E3)",
            Self::TemporalPositional => "TemporalPositional (E4)",
            Self::Causal => "Causal (E5)",
            Self::Sparse => "Sparse (E6)",
            Self::Code => "Code (E7)",
            Self::Graph => "Graph (E8)",
            Self::Hdc => "Hdc (E9)",
            Self::Multimodal => "Multimodal (E10)",
            Self::Entity => "Entity (E11)",
            Self::LateInteraction => "LateInteraction (E12)",
        };
        write!(f, "{name}")
    }
}

impl TryFrom<u8> for ModelId {
    type Error = &'static str;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Semantic),
            1 => Ok(Self::TemporalRecent),
            2 => Ok(Self::TemporalPeriodic),
            3 => Ok(Self::TemporalPositional),
            4 => Ok(Self::Causal),
            5 => Ok(Self::Sparse),
            6 => Ok(Self::Code),
            7 => Ok(Self::Graph),
            8 => Ok(Self::Hdc),
            9 => Ok(Self::Multimodal),
            10 => Ok(Self::Entity),
            11 => Ok(Self::LateInteraction),
            _ => Err("Invalid ModelId: must be 0-11"),
        }
    }
}

/// Tokenizer families for shared tokenization caching.
///
/// Models using the same family can share tokenized inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenizerFamily {
    /// BERT WordPiece tokenization (e5, SPLADE, MiniLM, ColBERT)
    BertWordpiece,
    /// RoBERTa BPE tokenization (Longformer)
    RobertaBpe,
    /// SentencePiece BPE tokenization (CodeT5p)
    SentencePieceBpe,
    /// CLIP-specific BPE tokenization
    ClipBpe,
    /// Custom models with no tokenization
    None,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_returns_12_variants() {
        assert_eq!(ModelId::all().len(), 12);
    }

    #[test]
    fn test_variant_order_matches_spec() {
        let all = ModelId::all();
        assert_eq!(all[0], ModelId::Semantic);         // E1
        assert_eq!(all[4], ModelId::Causal);           // E5
        assert_eq!(all[11], ModelId::LateInteraction); // E12
    }

    #[test]
    fn test_semantic_dimension() {
        assert_eq!(ModelId::Semantic.dimension(), 1024);
    }

    #[test]
    fn test_temporal_custom_flag() {
        assert!(ModelId::TemporalRecent.is_custom());
        assert!(ModelId::TemporalPeriodic.is_custom());
        assert!(ModelId::TemporalPositional.is_custom());
        assert!(ModelId::Hdc.is_custom());
    }

    #[test]
    fn test_pretrained_repo() {
        assert_eq!(
            ModelId::Semantic.model_repo(),
            Some("intfloat/e5-large-v2")
        );
        assert_eq!(ModelId::TemporalRecent.model_repo(), None);
    }

    #[test]
    fn test_model_path() {
        let base = Path::new("/home/cabdru/contextgraph/models");
        assert_eq!(
            ModelId::Semantic.model_path(base),
            PathBuf::from("/home/cabdru/contextgraph/models/semantic")
        );
        assert_eq!(
            ModelId::LateInteraction.model_path(base),
            PathBuf::from("/home/cabdru/contextgraph/models/late-interaction")
        );
    }

    #[test]
    fn test_max_tokens() {
        assert_eq!(ModelId::Causal.max_tokens(), 4096);
        assert_eq!(ModelId::Multimodal.max_tokens(), 77);
        assert_eq!(ModelId::Semantic.max_tokens(), 512);
    }

    #[test]
    fn test_u8_round_trip() {
        for id in ModelId::all() {
            let byte = *id as u8;
            let recovered = ModelId::try_from(byte).expect("valid u8 should convert");
            assert_eq!(*id, recovered);
        }
    }

    #[test]
    fn test_serde_round_trip() {
        for id in ModelId::all() {
            let json = serde_json::to_string(id).expect("serialization should succeed");
            let recovered: ModelId =
                serde_json::from_str(&json).expect("deserialization should succeed");
            assert_eq!(*id, recovered);
        }
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", ModelId::Semantic), "Semantic (E1)");
        assert_eq!(
            format!("{}", ModelId::LateInteraction),
            "LateInteraction (E12)"
        );
    }

    #[test]
    fn test_pretrained_count() {
        let pretrained: Vec<_> = ModelId::pretrained().collect();
        assert_eq!(pretrained.len(), 8); // 12 total - 4 custom
    }

    #[test]
    fn test_custom_count() {
        let custom: Vec<_> = ModelId::custom().collect();
        assert_eq!(custom.len(), 4); // TemporalRecent, TemporalPeriodic, TemporalPositional, Hdc
    }

    #[test]
    fn test_projected_dimensions() {
        // Sparse projects from ~30K to 1536
        assert_eq!(ModelId::Sparse.dimension(), 30522);
        assert_eq!(ModelId::Sparse.projected_dimension(), 1536);

        // Code projects from 256 embed_dim to 768 (CodeT5p internal dimension)
        assert_eq!(ModelId::Code.dimension(), 256);
        assert_eq!(ModelId::Code.projected_dimension(), 768);

        // HDC projects from 10K-bit to 1024
        assert_eq!(ModelId::Hdc.dimension(), 10000);
        assert_eq!(ModelId::Hdc.projected_dimension(), 1024);

        // Others unchanged
        assert_eq!(ModelId::Semantic.projected_dimension(), 1024);
    }

    #[test]
    fn test_latency_budgets() {
        assert_eq!(ModelId::Semantic.latency_budget_ms(), 5);
        assert_eq!(ModelId::Hdc.latency_budget_ms(), 1);
        assert_eq!(ModelId::Multimodal.latency_budget_ms(), 15);
    }

    #[test]
    fn test_tokenizer_families() {
        // BERT family: Semantic, Sparse, Graph, Entity, LateInteraction
        assert_eq!(
            ModelId::Semantic.tokenizer_family(),
            TokenizerFamily::BertWordpiece
        );
        assert_eq!(
            ModelId::Sparse.tokenizer_family(),
            TokenizerFamily::BertWordpiece
        );

        // RoBERTa family: Causal
        assert_eq!(ModelId::Causal.tokenizer_family(), TokenizerFamily::RobertaBpe);

        // SentencePiece family: Code (CodeT5p)
        assert_eq!(
            ModelId::Code.tokenizer_family(),
            TokenizerFamily::SentencePieceBpe
        );

        // CLIP family
        assert_eq!(ModelId::Multimodal.tokenizer_family(), TokenizerFamily::ClipBpe);

        // Custom: no tokenizer
        assert_eq!(ModelId::TemporalRecent.tokenizer_family(), TokenizerFamily::None);
    }

    #[test]
    fn test_invalid_u8_conversion() {
        // Before: attempt conversion of invalid value
        let result = ModelId::try_from(12u8);

        // After: verify error
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Invalid ModelId: must be 0-11");
        println!("Edge Case 1 PASSED: Invalid u8 (12) correctly rejected");
    }

    #[test]
    fn test_maximum_enum_value() {
        // Before: get max valid value
        let max_valid = ModelId::LateInteraction as u8;
        println!("Before: max valid u8 = {}", max_valid);

        // After: verify round-trip
        let recovered = ModelId::try_from(max_valid).expect("max valid should convert");
        assert_eq!(recovered, ModelId::LateInteraction);
        println!("After: recovered = {:?}", recovered);
        println!("Edge Case 2 PASSED: Maximum value (11) converts correctly");
    }

    #[test]
    fn test_custom_model_no_repo() {
        // Before: check all custom models
        for model in ModelId::custom() {
            println!("Before: checking {:?}", model);
            let repo = model.model_repo();

            // After: verify None
            assert!(repo.is_none(), "Custom model {:?} should have no repo", model);
            println!("After: {:?}.model_repo() = None (correct)", model);
        }
        println!("Edge Case 3 PASSED: All 4 custom models return None for repo");
    }
}
