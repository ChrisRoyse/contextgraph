//! Meaning extraction types for teleological fusion.
//!
//! From teleoplan.md Section 3.1 Per-Embedding Semantic Extraction:
//!
//! 1. ATTENTION FOCUSING - Top-k dimensions with highest activation
//! 2. CONTRAST ENHANCEMENT - Amplify dimensions that deviate from corpus mean
//! 3. SPARSIFICATION - Zero out low-signal dimensions
//! 4. NORMALIZATION - L2 normalize to unit sphere
//!
//! Section 3.2 Cross-Embedding Meaning Amplification:
//!
//! 1. AGREEMENT DETECTION - Find dimensions where multiple embeddings agree
//! 2. DISAGREEMENT EXPLOITATION - Dimensions where embeddings disagree = rich information
//! 3. PERSPECTIVE TRIANGULATION - Use 3+ embeddings to triangulate meaning

use serde::{Deserialize, Serialize};

use super::types::{EMBEDDING_DIM, NUM_EMBEDDERS};

/// Configuration for meaning extraction from embeddings.
///
/// Controls the parameters for attention focusing, contrast enhancement,
/// sparsification, and cross-embedding analysis.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MeaningExtractionConfig {
    /// Number of top dimensions to focus attention on.
    /// These represent the "semantic focus" of each embedding.
    /// Default: 128 (from teleoplan.md)
    pub attention_top_k: usize,

    /// Contrast enhancement factor (alpha).
    /// Enhanced[i] = Ei + alpha * (Ei - corpus_mean[i])
    /// Default: 0.2
    pub contrast_alpha: f32,

    /// Threshold below which dimensions are zeroed (sparsification).
    /// Default: 0.1
    pub sparsity_threshold: f32,

    /// Minimum number of embeddings that must agree for a dimension
    /// to be considered high-agreement.
    /// Default: 6 (from teleoplan.md: "Agreement > 6")
    pub min_agreement: usize,

    /// Variance threshold for disagreement detection.
    /// Dimensions with variance above this are flagged as high-disagreement.
    /// Default: 0.3
    pub disagreement_threshold: f32,

    /// Enable L2 normalization after extraction.
    /// Default: true
    pub normalize: bool,

    /// Agreement value threshold (dimensions above this count as "agreeing").
    /// Default: 0.5 (from teleoplan.md: "Ei[k] > 0.5")
    pub agreement_value_threshold: f32,
}

impl Default for MeaningExtractionConfig {
    fn default() -> Self {
        Self {
            attention_top_k: 128,
            contrast_alpha: 0.2,
            sparsity_threshold: 0.1,
            min_agreement: 6,
            disagreement_threshold: 0.3,
            normalize: true,
            agreement_value_threshold: 0.5,
        }
    }
}

impl MeaningExtractionConfig {
    /// Create a config optimized for code search.
    pub fn code_search() -> Self {
        Self {
            attention_top_k: 256,      // More focused attention
            contrast_alpha: 0.3,       // Higher contrast
            sparsity_threshold: 0.15,  // More aggressive sparsification
            min_agreement: 4,          // Lower agreement threshold (code-specific)
            disagreement_threshold: 0.25,
            normalize: true,
            agreement_value_threshold: 0.4,
        }
    }

    /// Create a config optimized for semantic search.
    pub fn semantic_search() -> Self {
        Self {
            attention_top_k: 128,
            contrast_alpha: 0.15,
            sparsity_threshold: 0.05,  // Less aggressive (preserve nuance)
            min_agreement: 7,          // Higher agreement for semantic
            disagreement_threshold: 0.35,
            normalize: true,
            agreement_value_threshold: 0.5,
        }
    }

    /// Create a config optimized for high precision.
    pub fn high_precision() -> Self {
        Self {
            attention_top_k: 64,        // Very focused
            contrast_alpha: 0.4,        // High contrast
            sparsity_threshold: 0.2,    // Aggressive sparsification
            min_agreement: 8,           // High agreement required
            disagreement_threshold: 0.2,
            normalize: true,
            agreement_value_threshold: 0.6,
        }
    }

    /// Validate configuration values.
    ///
    /// # Panics
    ///
    /// Panics if any values are invalid (FAIL FAST).
    pub fn validate(&self) {
        assert!(
            self.attention_top_k > 0 && self.attention_top_k <= EMBEDDING_DIM,
            "FAIL FAST: attention_top_k must be in (0, {}], got {}",
            EMBEDDING_DIM,
            self.attention_top_k
        );
        assert!(
            self.contrast_alpha >= 0.0 && self.contrast_alpha <= 1.0,
            "FAIL FAST: contrast_alpha must be in [0, 1], got {}",
            self.contrast_alpha
        );
        assert!(
            self.sparsity_threshold >= 0.0 && self.sparsity_threshold <= 1.0,
            "FAIL FAST: sparsity_threshold must be in [0, 1], got {}",
            self.sparsity_threshold
        );
        assert!(
            self.min_agreement > 0 && self.min_agreement <= NUM_EMBEDDERS,
            "FAIL FAST: min_agreement must be in (0, {}], got {}",
            NUM_EMBEDDERS,
            self.min_agreement
        );
        assert!(
            self.disagreement_threshold >= 0.0,
            "FAIL FAST: disagreement_threshold must be >= 0, got {}",
            self.disagreement_threshold
        );
        assert!(
            self.agreement_value_threshold >= 0.0 && self.agreement_value_threshold <= 1.0,
            "FAIL FAST: agreement_value_threshold must be in [0, 1], got {}",
            self.agreement_value_threshold
        );
    }
}

/// Result of meaning extraction from embeddings.
///
/// Contains the extracted semantic focus, enhanced/sparse representations,
/// and cross-embedding analysis results.
#[derive(Clone, Debug)]
pub struct ExtractedMeaning {
    /// Top-k dimensions with highest activation (index, value) pairs.
    /// Sorted by value descending.
    pub focus_dimensions: Vec<(usize, f32)>,

    /// Contrast-enhanced embedding.
    pub enhanced: Vec<f32>,

    /// Sparsified embedding (low values zeroed).
    pub sparse: Vec<f32>,

    /// Dimension indices where embeddings agree (above min_agreement threshold).
    pub agreement_dimensions: Vec<usize>,

    /// Dimension indices where embeddings disagree (high variance).
    pub disagreement_dimensions: Vec<usize>,

    /// Per-dimension agreement counts (how many embeddings agree on this dim).
    pub agreement_counts: Vec<usize>,

    /// Per-dimension variance across embeddings.
    pub dimension_variances: Vec<f32>,
}

impl ExtractedMeaning {
    /// Create an empty ExtractedMeaning.
    pub fn empty() -> Self {
        Self {
            focus_dimensions: Vec::new(),
            enhanced: Vec::new(),
            sparse: Vec::new(),
            agreement_dimensions: Vec::new(),
            disagreement_dimensions: Vec::new(),
            agreement_counts: Vec::new(),
            dimension_variances: Vec::new(),
        }
    }

    /// Number of high-agreement dimensions.
    pub fn agreement_count(&self) -> usize {
        self.agreement_dimensions.len()
    }

    /// Number of high-disagreement dimensions.
    pub fn disagreement_count(&self) -> usize {
        self.disagreement_dimensions.len()
    }

    /// Sparsity ratio (proportion of zero values in sparse embedding).
    pub fn sparsity_ratio(&self) -> f32 {
        if self.sparse.is_empty() {
            return 0.0;
        }
        let zero_count = self.sparse.iter().filter(|&&v| v.abs() < f32::EPSILON).count();
        zero_count as f32 / self.sparse.len() as f32
    }

    /// Get the top N focus dimensions.
    pub fn top_focus(&self, n: usize) -> &[(usize, f32)] {
        let len = n.min(self.focus_dimensions.len());
        &self.focus_dimensions[..len]
    }

    /// Check if a dimension is high-agreement.
    pub fn is_agreement_dimension(&self, dim: usize) -> bool {
        self.agreement_dimensions.contains(&dim)
    }

    /// Check if a dimension is high-disagreement.
    pub fn is_disagreement_dimension(&self, dim: usize) -> bool {
        self.disagreement_dimensions.contains(&dim)
    }
}

/// Cross-embedding analysis for meaning triangulation.
///
/// From teleoplan.md Section 3.2:
/// - Perspective triangulation using 3+ embeddings
/// - If E1, E4, E11 (semantic, causal, abstract) agree -> high confidence
/// - If only E10 (emotional) disagrees -> emotional nuance detected
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrossEmbeddingAnalysis {
    /// Per-dimension agreement matrix: which embeddings agree on each dimension.
    /// Outer index: dimension, Inner: set of agreeing embedder indices.
    pub dimension_agreement: Vec<Vec<usize>>,

    /// Dimensions with triangulated confidence (3+ embeddings agree).
    pub triangulated_dimensions: Vec<usize>,

    /// Dimensions with detected nuance (one embedder disagrees).
    pub nuance_dimensions: Vec<NuanceDimension>,

    /// Overall cross-embedding coherence score [0, 1].
    pub coherence_score: f32,
}

impl CrossEmbeddingAnalysis {
    /// Create empty analysis.
    pub fn empty() -> Self {
        Self {
            dimension_agreement: Vec::new(),
            triangulated_dimensions: Vec::new(),
            nuance_dimensions: Vec::new(),
            coherence_score: 0.0,
        }
    }

    /// Number of triangulated (high-confidence) dimensions.
    pub fn triangulated_count(&self) -> usize {
        self.triangulated_dimensions.len()
    }

    /// Number of dimensions with detected nuance.
    pub fn nuance_count(&self) -> usize {
        self.nuance_dimensions.len()
    }
}

impl Default for CrossEmbeddingAnalysis {
    fn default() -> Self {
        Self::empty()
    }
}

/// A dimension where nuance is detected (one embedder disagrees).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NuanceDimension {
    /// The dimension index.
    pub dimension: usize,

    /// The embedder index that disagrees.
    pub dissenting_embedder: usize,

    /// Dissenting value.
    pub dissenting_value: f32,

    /// Consensus value from agreeing embedders.
    pub consensus_value: f32,

    /// Magnitude of disagreement.
    pub disagreement_magnitude: f32,
}

impl NuanceDimension {
    /// Create a new NuanceDimension.
    pub fn new(
        dimension: usize,
        dissenting_embedder: usize,
        dissenting_value: f32,
        consensus_value: f32,
    ) -> Self {
        Self {
            dimension,
            dissenting_embedder,
            dissenting_value,
            consensus_value,
            disagreement_magnitude: (dissenting_value - consensus_value).abs(),
        }
    }

    /// Check if the dissenter is the emotional embedder (E10, index 9).
    pub fn is_emotional_nuance(&self) -> bool {
        self.dissenting_embedder == 9
    }

    /// Check if the dissenter is the code embedder (E6, index 5).
    pub fn is_code_nuance(&self) -> bool {
        self.dissenting_embedder == 5
    }

    /// Check if the dissenter is the social embedder (E9, index 8).
    pub fn is_social_nuance(&self) -> bool {
        self.dissenting_embedder == 8
    }
}

/// Fusion method for combining extracted meanings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum FusionMethod {
    /// Simple weighted average.
    #[default]
    WeightedAverage,

    /// Attention-weighted combination.
    Attention,

    /// Use only focus dimensions.
    FocusOnly,

    /// Hierarchical group fusion.
    Hierarchical,

    /// Use agreement dimensions only (high consensus).
    ConsensusOnly,
}


impl FusionMethod {
    /// All available fusion methods.
    pub const ALL: [FusionMethod; 5] = [
        FusionMethod::WeightedAverage,
        FusionMethod::Attention,
        FusionMethod::FocusOnly,
        FusionMethod::Hierarchical,
        FusionMethod::ConsensusOnly,
    ];

    /// Human-readable description.
    pub fn description(self) -> &'static str {
        match self {
            FusionMethod::WeightedAverage => "Simple weighted average of all dimensions",
            FusionMethod::Attention => "Attention-weighted combination using query",
            FusionMethod::FocusOnly => "Use only top-k focus dimensions",
            FusionMethod::Hierarchical => "Hierarchical group-then-domain fusion",
            FusionMethod::ConsensusOnly => "Use only high-agreement dimensions",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== MeaningExtractionConfig Tests =====

    #[test]
    fn test_config_default() {
        let config = MeaningExtractionConfig::default();

        assert_eq!(config.attention_top_k, 128);
        assert!((config.contrast_alpha - 0.2).abs() < f32::EPSILON);
        assert!((config.sparsity_threshold - 0.1).abs() < f32::EPSILON);
        assert_eq!(config.min_agreement, 6);
        assert!((config.disagreement_threshold - 0.3).abs() < f32::EPSILON);
        assert!(config.normalize);
        assert!((config.agreement_value_threshold - 0.5).abs() < f32::EPSILON);

        println!("[PASS] MeaningExtractionConfig::default matches teleoplan.md");
    }

    #[test]
    fn test_config_code_search() {
        let config = MeaningExtractionConfig::code_search();

        assert_eq!(config.attention_top_k, 256);
        assert!(config.contrast_alpha > 0.2);
        assert!(config.sparsity_threshold > 0.1);

        println!("[PASS] code_search config has tighter parameters");
    }

    #[test]
    fn test_config_semantic_search() {
        let config = MeaningExtractionConfig::semantic_search();

        assert!(config.sparsity_threshold < 0.1); // Less aggressive
        assert!(config.min_agreement > 6);         // Higher agreement

        println!("[PASS] semantic_search config preserves nuance");
    }

    #[test]
    fn test_config_high_precision() {
        let config = MeaningExtractionConfig::high_precision();

        assert!(config.attention_top_k < 128);     // Very focused
        assert!(config.contrast_alpha > 0.3);      // High contrast
        assert!(config.sparsity_threshold > 0.15); // Aggressive

        println!("[PASS] high_precision config is aggressive");
    }

    #[test]
    fn test_config_validate_pass() {
        let config = MeaningExtractionConfig::default();
        config.validate(); // Should not panic

        println!("[PASS] Default config passes validation");
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_config_validate_invalid_top_k_zero() {
        let config = MeaningExtractionConfig {
            attention_top_k: 0,
            ..Default::default()
        };
        config.validate();
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_config_validate_invalid_top_k_too_large() {
        let config = MeaningExtractionConfig {
            attention_top_k: EMBEDDING_DIM + 1,
            ..Default::default()
        };
        config.validate();
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_config_validate_invalid_alpha() {
        let config = MeaningExtractionConfig {
            contrast_alpha: 1.5,
            ..Default::default()
        };
        config.validate();
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_config_validate_invalid_agreement() {
        let config = MeaningExtractionConfig {
            min_agreement: 0,
            ..Default::default()
        };
        config.validate();
    }

    #[test]
    fn test_config_serialization() {
        let config = MeaningExtractionConfig::code_search();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: MeaningExtractionConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.attention_top_k, deserialized.attention_top_k);

        println!("[PASS] Config serialization roundtrip works");
    }

    // ===== ExtractedMeaning Tests =====

    #[test]
    fn test_extracted_meaning_empty() {
        let em = ExtractedMeaning::empty();

        assert!(em.focus_dimensions.is_empty());
        assert!(em.enhanced.is_empty());
        assert!(em.sparse.is_empty());
        assert_eq!(em.agreement_count(), 0);
        assert_eq!(em.disagreement_count(), 0);

        println!("[PASS] ExtractedMeaning::empty creates empty structure");
    }

    #[test]
    fn test_extracted_meaning_sparsity_ratio() {
        let mut em = ExtractedMeaning::empty();

        // Empty = 0 ratio
        assert!((em.sparsity_ratio() - 0.0).abs() < f32::EPSILON);

        // 50% sparse
        em.sparse = vec![0.0, 0.5, 0.0, 0.3, 0.0, 0.1];
        assert!((em.sparsity_ratio() - 0.5).abs() < 0.001);

        println!("[PASS] sparsity_ratio computes correctly");
    }

    #[test]
    fn test_extracted_meaning_top_focus() {
        let mut em = ExtractedMeaning::empty();
        em.focus_dimensions = vec![(10, 0.9), (20, 0.8), (30, 0.7), (40, 0.6)];

        let top2 = em.top_focus(2);
        assert_eq!(top2.len(), 2);
        assert_eq!(top2[0], (10, 0.9));
        assert_eq!(top2[1], (20, 0.8));

        // Request more than available
        let top10 = em.top_focus(10);
        assert_eq!(top10.len(), 4);

        println!("[PASS] top_focus returns correct subset");
    }

    #[test]
    fn test_extracted_meaning_dimension_queries() {
        let mut em = ExtractedMeaning::empty();
        em.agreement_dimensions = vec![5, 10, 15];
        em.disagreement_dimensions = vec![20, 25];

        assert!(em.is_agreement_dimension(10));
        assert!(!em.is_agreement_dimension(20));
        assert!(em.is_disagreement_dimension(20));
        assert!(!em.is_disagreement_dimension(10));

        println!("[PASS] dimension queries work correctly");
    }

    // ===== CrossEmbeddingAnalysis Tests =====

    #[test]
    fn test_cross_embedding_analysis_empty() {
        let cea = CrossEmbeddingAnalysis::empty();

        assert!(cea.dimension_agreement.is_empty());
        assert_eq!(cea.triangulated_count(), 0);
        assert_eq!(cea.nuance_count(), 0);
        assert!((cea.coherence_score - 0.0).abs() < f32::EPSILON);

        println!("[PASS] CrossEmbeddingAnalysis::empty creates empty structure");
    }

    #[test]
    fn test_cross_embedding_analysis_default() {
        let cea = CrossEmbeddingAnalysis::default();
        assert_eq!(cea.triangulated_count(), 0);

        println!("[PASS] CrossEmbeddingAnalysis::default is empty");
    }

    #[test]
    fn test_cross_embedding_analysis_serialization() {
        let mut cea = CrossEmbeddingAnalysis::empty();
        cea.triangulated_dimensions = vec![1, 2, 3];
        cea.coherence_score = 0.85;

        let json = serde_json::to_string(&cea).unwrap();
        let deserialized: CrossEmbeddingAnalysis = serde_json::from_str(&json).unwrap();

        assert_eq!(cea.triangulated_dimensions, deserialized.triangulated_dimensions);
        assert!((cea.coherence_score - deserialized.coherence_score).abs() < f32::EPSILON);

        println!("[PASS] CrossEmbeddingAnalysis serialization works");
    }

    // ===== NuanceDimension Tests =====

    #[test]
    fn test_nuance_dimension_new() {
        let nd = NuanceDimension::new(100, 9, 0.2, 0.8);

        assert_eq!(nd.dimension, 100);
        assert_eq!(nd.dissenting_embedder, 9);
        assert!((nd.dissenting_value - 0.2).abs() < f32::EPSILON);
        assert!((nd.consensus_value - 0.8).abs() < f32::EPSILON);
        assert!((nd.disagreement_magnitude - 0.6).abs() < f32::EPSILON);

        println!("[PASS] NuanceDimension::new computes disagreement magnitude");
    }

    #[test]
    fn test_nuance_dimension_emotional() {
        let nd = NuanceDimension::new(50, 9, 0.1, 0.9);
        assert!(nd.is_emotional_nuance());

        let nd_other = NuanceDimension::new(50, 5, 0.1, 0.9);
        assert!(!nd_other.is_emotional_nuance());

        println!("[PASS] is_emotional_nuance detects E10 (index 9)");
    }

    #[test]
    fn test_nuance_dimension_code() {
        let nd = NuanceDimension::new(50, 5, 0.1, 0.9);
        assert!(nd.is_code_nuance());

        println!("[PASS] is_code_nuance detects E6 (index 5)");
    }

    #[test]
    fn test_nuance_dimension_social() {
        let nd = NuanceDimension::new(50, 8, 0.1, 0.9);
        assert!(nd.is_social_nuance());

        println!("[PASS] is_social_nuance detects E9 (index 8)");
    }

    // ===== FusionMethod Tests =====

    #[test]
    fn test_fusion_method_default() {
        assert_eq!(FusionMethod::default(), FusionMethod::WeightedAverage);

        println!("[PASS] FusionMethod::default is WeightedAverage");
    }

    #[test]
    fn test_fusion_method_all() {
        assert_eq!(FusionMethod::ALL.len(), 5);

        println!("[PASS] FusionMethod::ALL contains 5 methods");
    }

    #[test]
    fn test_fusion_method_description() {
        assert!(FusionMethod::WeightedAverage.description().contains("weighted"));
        assert!(FusionMethod::Attention.description().contains("Attention"));
        assert!(FusionMethod::FocusOnly.description().contains("focus"));
        assert!(FusionMethod::Hierarchical.description().contains("Hierarchical"));
        assert!(FusionMethod::ConsensusOnly.description().contains("agreement"));

        println!("[PASS] FusionMethod descriptions are informative");
    }

    #[test]
    fn test_fusion_method_equality() {
        assert_eq!(FusionMethod::Attention, FusionMethod::Attention);
        assert_ne!(FusionMethod::Attention, FusionMethod::FocusOnly);

        println!("[PASS] FusionMethod equality works");
    }

    #[test]
    fn test_fusion_method_serialization() {
        let method = FusionMethod::Hierarchical;
        let json = serde_json::to_string(&method).unwrap();
        let deserialized: FusionMethod = serde_json::from_str(&json).unwrap();

        assert_eq!(method, deserialized);

        println!("[PASS] FusionMethod serialization works");
    }
}
