//! Pipeline types, configuration, and result structures.
//!
//! This module contains all type definitions for the 4-stage retrieval pipeline.

use uuid::Uuid;

use super::super::super::indexes::EmbedderIndex;
use super::super::error::SearchError;
use context_graph_core::graph_linking::GraphLinkEdgeType;

// ============================================================================
// PIPELINE ERRORS
// ============================================================================

/// Pipeline-specific errors. FAIL FAST - no recovery.
#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    /// Stage execution error.
    #[error("FAIL FAST: Stage {stage:?} error: {error}")]
    Stage { stage: PipelineStage, error: String },

    /// Stage exceeded maximum latency.
    #[error("FAIL FAST: Stage {stage:?} timeout after {elapsed_ms}ms (max: {max_ms}ms)")]
    Timeout {
        stage: PipelineStage,
        elapsed_ms: u64,
        max_ms: u64,
    },

    /// Required query missing for stage.
    #[error("FAIL FAST: Missing query for stage {stage:?}")]
    MissingQuery { stage: PipelineStage },

    /// Empty candidates at stage (when not expected).
    #[error("FAIL FAST: Empty candidates at stage {stage:?}")]
    EmptyCandidates { stage: PipelineStage },

    /// Wrapped search error.
    #[error("FAIL FAST: Search error: {0}")]
    Search(#[from] SearchError),
}

// ============================================================================
// STAGE CONFIGURATION
// ============================================================================

/// Configuration for a single pipeline stage.
#[derive(Debug, Clone)]
pub struct StageConfig {
    /// Whether this stage is enabled.
    pub enabled: bool,
    /// Candidate multiplier: target = k * multiplier.
    pub candidate_multiplier: f32,
    /// Minimum score threshold to pass this stage.
    pub min_score_threshold: f32,
    /// Maximum allowed latency in milliseconds.
    pub max_latency_ms: u64,
}

impl Default for StageConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            candidate_multiplier: 3.0,
            min_score_threshold: 0.3,
            max_latency_ms: 20,
        }
    }
}

// ============================================================================
// GRAPH EXPANSION CONFIGURATION
// ============================================================================

/// Edge type routing strategy for graph expansion.
///
/// Determines which edge types to follow during expansion.
#[derive(Debug, Clone, Default)]
pub enum EdgeTypeRouting {
    /// Follow all edge types (default).
    #[default]
    All,
    /// Follow only specific edge types.
    Custom(Vec<GraphLinkEdgeType>),
}

impl EdgeTypeRouting {
    /// Check if an edge type should be expanded.
    pub fn should_expand(&self, edge_type: GraphLinkEdgeType) -> bool {
        match self {
            Self::All => true,
            Self::Custom(types) => types.contains(&edge_type),
        }
    }
}

/// Configuration for graph expansion stage (Stage 3.5).
///
/// Controls how candidates are expanded via pre-computed K-NN graph edges.
#[derive(Debug, Clone)]
pub struct GraphExpansionConfig {
    /// Whether graph expansion is enabled.
    pub enabled: bool,
    /// Maximum neighbors to expand per candidate node.
    pub max_expansion_per_node: usize,
    /// Minimum edge weight threshold to follow an edge.
    pub min_edge_weight: f32,
    /// Decay factor applied to expanded node scores (0.0-1.0).
    /// Expanded nodes get score = parent_score * edge_weight * expansion_decay.
    pub expansion_decay: f32,
    /// Edge type routing strategy.
    pub edge_type_routing: EdgeTypeRouting,
    /// Maximum total expanded candidates (to prevent explosion).
    pub max_total_expanded: usize,
    /// Maximum latency in milliseconds.
    pub max_latency_ms: u64,
}

impl Default for GraphExpansionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_expansion_per_node: 5,
            min_edge_weight: 0.4,
            expansion_decay: 0.8,
            edge_type_routing: EdgeTypeRouting::default(),
            max_total_expanded: 150,
            max_latency_ms: 10,
        }
    }
}

impl GraphExpansionConfig {
    /// Create a disabled configuration.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }
}

// ============================================================================
// GNN ENHANCEMENT CONFIGURATION
// ============================================================================

/// Configuration for GNN enhancement stage (Stage 3.75).
///
/// Controls R-GCN message passing for learned retrieval enhancement.
/// Uses pre-computed graph edges to propagate information between nodes.
#[derive(Debug, Clone)]
pub struct GnnEnhanceConfig {
    /// Whether GNN enhancement is enabled.
    pub enabled: bool,
    /// Path to the R-GCN model weights (SafeTensors).
    pub weights_path: Option<String>,
    /// Weight for blending GNN score with original score (0.0-1.0).
    /// final_score = original * (1 - blend_weight) + gnn_score * blend_weight
    pub blend_weight: f32,
    /// Maximum subgraph nodes to include for GNN forward pass.
    pub max_subgraph_nodes: usize,
    /// Maximum subgraph edges to include.
    pub max_subgraph_edges: usize,
    /// Minimum similarity for GNN re-scoring (skip low-similarity candidates).
    pub min_similarity: f32,
    /// Maximum latency in milliseconds.
    pub max_latency_ms: u64,
}

impl Default for GnnEnhanceConfig {
    fn default() -> Self {
        Self {
            enabled: false, // Disabled by default (requires trained model)
            weights_path: None,
            blend_weight: 0.3,
            max_subgraph_nodes: 200,
            max_subgraph_edges: 1000,
            min_similarity: 0.3,
            max_latency_ms: 20,
        }
    }
}

impl GnnEnhanceConfig {
    /// Create a disabled configuration.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    /// Create an enabled configuration with model path.
    pub fn with_model(weights_path: impl Into<String>) -> Self {
        Self {
            enabled: true,
            weights_path: Some(weights_path.into()),
            ..Default::default()
        }
    }

    /// Set blend weight.
    pub fn with_blend_weight(mut self, weight: f32) -> Self {
        self.blend_weight = weight.clamp(0.0, 1.0);
        self
    }
}

// ============================================================================
// PIPELINE STAGE ENUM
// ============================================================================

/// The 6 pipeline stages (including optional graph expansion and GNN enhancement).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PipelineStage {
    /// Stage 1: SPLADE sparse pre-filter (inverted index).
    SpladeFilter,
    /// Stage 2: Matryoshka 128D fast ANN.
    MatryoshkaAnn,
    /// Stage 3: Multi-space RRF rerank.
    RrfRerank,
    /// Stage 3.5: Graph expansion via K-NN edges.
    GraphExpansion,
    /// Stage 3.75: GNN-enhanced node embeddings (R-GCN).
    GnnEnhance,
    /// Stage 4: Late interaction MaxSim.
    MaxSimRerank,
}

impl PipelineStage {
    /// Get the stage index (0-5).
    #[inline]
    pub fn index(&self) -> usize {
        match self {
            Self::SpladeFilter => 0,
            Self::MatryoshkaAnn => 1,
            Self::RrfRerank => 2,
            Self::GraphExpansion => 3,
            Self::GnnEnhance => 4,
            Self::MaxSimRerank => 5,
        }
    }

    /// Get all stages in order.
    pub fn all() -> [Self; 6] {
        [
            Self::SpladeFilter,
            Self::MatryoshkaAnn,
            Self::RrfRerank,
            Self::GraphExpansion,
            Self::GnnEnhance,
            Self::MaxSimRerank,
        ]
    }

    /// Get core stages (excluding optional stages).
    pub fn core() -> [Self; 4] {
        [
            Self::SpladeFilter,
            Self::MatryoshkaAnn,
            Self::RrfRerank,
            Self::MaxSimRerank,
        ]
    }
}

// ============================================================================
// PIPELINE CANDIDATE
// ============================================================================

/// A candidate moving through the pipeline.
#[derive(Debug, Clone)]
pub struct PipelineCandidate {
    /// Memory ID.
    pub id: Uuid,
    /// Current aggregated score.
    pub score: f32,
    /// Stage-by-stage scores for debugging.
    pub stage_scores: Vec<(PipelineStage, f32)>,
}

impl PipelineCandidate {
    /// Create a new candidate with initial score.
    #[inline]
    pub fn new(id: Uuid, score: f32) -> Self {
        Self {
            id,
            score,
            stage_scores: Vec::with_capacity(4),
        }
    }

    /// Add a stage score.
    #[inline]
    pub fn add_stage_score(&mut self, stage: PipelineStage, score: f32) {
        self.stage_scores.push((stage, score));
        self.score = score;
    }
}

// ============================================================================
// STAGE RESULT
// ============================================================================

/// Result from a single pipeline stage.
#[derive(Debug)]
pub struct StageResult {
    /// Candidates that passed this stage.
    pub candidates: Vec<PipelineCandidate>,
    /// Stage execution latency in microseconds.
    pub latency_us: u64,
    /// Number of candidates entering this stage.
    pub candidates_in: usize,
    /// Number of candidates exiting this stage.
    pub candidates_out: usize,
    /// Stage that produced this result.
    pub stage: PipelineStage,
}

// ============================================================================
// PIPELINE CONFIGURATION
// ============================================================================

/// Configuration for the full pipeline.
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    /// Per-stage configurations (indexed 0-3 for core stages).
    /// 0 = SPLADE, 1 = Matryoshka, 2 = RRF, 3 = MaxSim
    pub stages: [StageConfig; 4],
    /// Graph expansion configuration (Stage 3.5, between RRF and MaxSim).
    pub graph_expansion: GraphExpansionConfig,
    /// GNN enhancement configuration (Stage 3.75, after graph expansion).
    pub gnn_enhance: GnnEnhanceConfig,
    /// Final result limit.
    pub k: usize,
    /// RRF constant (default 60.0).
    pub rrf_k: f32,
    /// RRF embedders to use in Stage 3.
    pub rrf_embedders: Vec<EmbedderIndex>,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            stages: [
                StageConfig {
                    candidate_multiplier: 10.0,
                    max_latency_ms: 5,
                    ..Default::default()
                }, // Stage 1: wide
                StageConfig {
                    candidate_multiplier: 5.0,
                    max_latency_ms: 10,
                    ..Default::default()
                }, // Stage 2: narrower
                StageConfig {
                    candidate_multiplier: 3.0,
                    max_latency_ms: 20,
                    ..Default::default()
                }, // Stage 3: RRF
                StageConfig {
                    candidate_multiplier: 1.0,
                    max_latency_ms: 15,
                    ..Default::default()
                }, // Stage 4: MaxSim final
            ],
            graph_expansion: GraphExpansionConfig::default(),
            gnn_enhance: GnnEnhanceConfig::default(),
            k: 10,
            rrf_k: 60.0,
            rrf_embedders: vec![
                EmbedderIndex::E1Semantic,
                EmbedderIndex::E8Graph,
                EmbedderIndex::E5Causal,
            ],
        }
    }
}

impl PipelineConfig {
    /// Create configuration with graph expansion disabled.
    pub fn without_graph_expansion() -> Self {
        Self {
            graph_expansion: GraphExpansionConfig::disabled(),
            ..Default::default()
        }
    }

    /// Set graph expansion config.
    pub fn with_graph_expansion(mut self, config: GraphExpansionConfig) -> Self {
        self.graph_expansion = config;
        self
    }

    /// Set GNN enhancement config.
    pub fn with_gnn_enhance(mut self, config: GnnEnhanceConfig) -> Self {
        self.gnn_enhance = config;
        self
    }

    /// Create configuration with GNN enhancement enabled.
    pub fn with_gnn_model(mut self, weights_path: impl Into<String>) -> Self {
        self.gnn_enhance = GnnEnhanceConfig::with_model(weights_path);
        self
    }
}

// ============================================================================
// PIPELINE RESULT
// ============================================================================

/// Final pipeline result.
#[derive(Debug)]
pub struct PipelineResult {
    /// Final ranked results.
    pub results: Vec<PipelineCandidate>,
    /// Per-stage results for debugging and analysis.
    pub stage_results: Vec<StageResult>,
    /// Total pipeline latency in microseconds.
    pub total_latency_us: u64,
    /// Stages that were executed.
    pub stages_executed: Vec<PipelineStage>,
}

impl PipelineResult {
    /// Check if results are empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.results.is_empty()
    }

    /// Get number of results.
    #[inline]
    pub fn len(&self) -> usize {
        self.results.len()
    }

    /// Get top result.
    #[inline]
    pub fn top(&self) -> Option<&PipelineCandidate> {
        self.results.first()
    }

    /// Get total latency in milliseconds.
    #[inline]
    pub fn latency_ms(&self) -> f64 {
        self.total_latency_us as f64 / 1000.0
    }
}
