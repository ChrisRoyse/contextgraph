//! Graph structure impact metrics for measuring embedder effects on knowledge graph.
//!
//! This module measures how each embedder affects:
//! - Topic formation (cluster detection)
//! - Edge creation and distribution
//! - Graph connectivity
//! - Weighted agreement scores
//!
//! Per ARCH-04 and AP-60: Temporal embedders (E2-E4) should have 0 impact on topics.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use context_graph_core::graph_linking::GraphLinkEdgeType;
use context_graph_storage::teleological::indexes::EmbedderIndex;

/// Complete graph structure impact analysis.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GraphStructureImpact {
    /// Edge type distribution per embedder.
    pub edge_type_distribution: HashMap<EmbedderIndex, EdgeTypeDistribution>,
    /// Topic formation impact analysis.
    pub topic_impact: TopicFormationImpact,
    /// Graph connectivity metrics.
    pub connectivity_metrics: GraphConnectivityMetrics,
    /// Weighted agreement analysis per embedder.
    pub weighted_agreement_analysis: WeightedAgreementAnalysis,
}

impl GraphStructureImpact {
    /// Create new graph structure impact.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if temporal embedders have zero topic impact (per AP-60).
    pub fn verify_temporal_zero_impact(&self) -> bool {
        let temporal = [
            EmbedderIndex::E2TemporalRecent,
            EmbedderIndex::E3TemporalPeriodic,
            EmbedderIndex::E4TemporalPositional,
        ];

        for embedder in temporal {
            if let Some(impact) = self.topic_impact.topic_count_without.get(&embedder) {
                // Removing temporal should not change topic count
                if *impact != self.topic_impact.full_topic_count {
                    return false;
                }
            }
        }
        true
    }
}

/// Topic formation impact showing how embedders affect clustering.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TopicFormationImpact {
    /// Topic count with all embedders.
    pub full_topic_count: usize,
    /// Topic count when each embedder is removed.
    pub topic_count_without: HashMap<EmbedderIndex, usize>,
    /// Full weighted agreement (all embedders).
    pub full_weighted_agreement: f32,
    /// Weighted agreement when each embedder is removed.
    pub weighted_agreement_without: HashMap<EmbedderIndex, f32>,
    /// Delta in topic count when each embedder is removed.
    pub topic_delta: HashMap<EmbedderIndex, i32>,
    /// Delta in weighted agreement when each embedder is removed.
    pub agreement_delta: HashMap<EmbedderIndex, f32>,
    /// Embedders essential for topic formation (removing causes >10% topic loss).
    pub essential_for_topics: Vec<EmbedderIndex>,
}

impl TopicFormationImpact {
    /// Create new topic formation impact.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set full metrics.
    pub fn set_full(&mut self, topic_count: usize, weighted_agreement: f32) {
        self.full_topic_count = topic_count;
        self.full_weighted_agreement = weighted_agreement;
    }

    /// Add ablation result for an embedder.
    pub fn add_without(
        &mut self,
        embedder: EmbedderIndex,
        topic_count: usize,
        weighted_agreement: f32,
    ) {
        self.topic_count_without.insert(embedder, topic_count);
        self.weighted_agreement_without
            .insert(embedder, weighted_agreement);

        // Compute deltas
        let topic_delta = self.full_topic_count as i32 - topic_count as i32;
        self.topic_delta.insert(embedder, topic_delta);

        let agreement_delta = self.full_weighted_agreement - weighted_agreement;
        self.agreement_delta.insert(embedder, agreement_delta);

        // Check if essential (>10% topic loss)
        if self.full_topic_count > 0 {
            let pct_loss = topic_delta as f64 / self.full_topic_count as f64;
            if pct_loss > 0.10 {
                self.essential_for_topics.push(embedder);
            }
        }
    }

    /// Get embedders sorted by topic impact (highest delta first).
    pub fn sorted_by_topic_impact(&self) -> Vec<(EmbedderIndex, i32)> {
        let mut sorted: Vec<_> = self.topic_delta.iter().map(|(&e, &d)| (e, d)).collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        sorted
    }

    /// Get embedders sorted by agreement impact (highest delta first).
    pub fn sorted_by_agreement_impact(&self) -> Vec<(EmbedderIndex, f32)> {
        let mut sorted: Vec<_> = self.agreement_delta.iter().map(|(&e, &d)| (e, d)).collect();
        sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        sorted
    }
}

/// Edge type distribution for a single embedder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeTypeDistribution {
    /// Which embedder this is for.
    pub embedder: EmbedderIndex,
    /// Count of edges by type.
    pub type_counts: HashMap<GraphLinkEdgeType, usize>,
    /// Total edges this embedder contributes to.
    pub total_edges: usize,
    /// Percentage of total edges in graph this embedder influences.
    pub edge_coverage: f64,
    /// Average edge weight for edges this embedder contributes to.
    pub avg_edge_weight: f32,
    /// Edges where this embedder is the primary contributor.
    pub primary_contributor_count: usize,
}

impl EdgeTypeDistribution {
    /// Create new edge type distribution for an embedder.
    pub fn new(embedder: EmbedderIndex) -> Self {
        Self {
            embedder,
            type_counts: HashMap::new(),
            total_edges: 0,
            edge_coverage: 0.0,
            avg_edge_weight: 0.0,
            primary_contributor_count: 0,
        }
    }

    /// Add an edge.
    pub fn add_edge(&mut self, edge_type: GraphLinkEdgeType, weight: f32, is_primary: bool) {
        *self.type_counts.entry(edge_type).or_default() += 1;
        self.total_edges += 1;
        self.avg_edge_weight += weight;
        if is_primary {
            self.primary_contributor_count += 1;
        }
    }

    /// Finalize calculations.
    pub fn finalize(&mut self, total_graph_edges: usize) {
        if self.total_edges > 0 {
            self.avg_edge_weight /= self.total_edges as f32;
        }
        if total_graph_edges > 0 {
            self.edge_coverage = (self.total_edges as f64 / total_graph_edges as f64) * 100.0;
        }
    }

    /// Get dominant edge type (most frequent).
    pub fn dominant_type(&self) -> Option<GraphLinkEdgeType> {
        self.type_counts
            .iter()
            .max_by_key(|(_, &count)| count)
            .map(|(&t, _)| t)
    }
}

/// Connectivity metrics for a single embedder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedderConnectivity {
    /// Which embedder this is for.
    pub embedder: EmbedderIndex,
    /// Average out-degree for nodes this embedder connects.
    pub avg_out_degree: f32,
    /// Average in-degree for nodes this embedder connects.
    pub avg_in_degree: f32,
    /// Number of isolated nodes when only this embedder's edges are considered.
    pub isolated_nodes: usize,
    /// Largest connected component size.
    pub largest_component_size: usize,
    /// Number of connected components.
    pub component_count: usize,
}

impl EmbedderConnectivity {
    /// Create new embedder connectivity.
    pub fn new(embedder: EmbedderIndex) -> Self {
        Self {
            embedder,
            avg_out_degree: 0.0,
            avg_in_degree: 0.0,
            isolated_nodes: 0,
            largest_component_size: 0,
            component_count: 0,
        }
    }
}

/// Graph-wide connectivity metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GraphConnectivityMetrics {
    /// Average degree (in + out) per node.
    pub avg_degree: f32,
    /// Global clustering coefficient.
    pub clustering_coefficient: f32,
    /// Density (edges / possible edges).
    pub density: f64,
    /// Number of nodes.
    pub node_count: usize,
    /// Number of edges.
    pub edge_count: usize,
    /// Per-embedder connectivity metrics.
    pub embedder_connectivity: HashMap<EmbedderIndex, EmbedderConnectivity>,
    /// Connectivity when each embedder is removed.
    pub connectivity_without: HashMap<EmbedderIndex, ConnectivitySnapshot>,
}

impl GraphConnectivityMetrics {
    /// Create new connectivity metrics.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set graph-wide metrics.
    pub fn set_global(&mut self, node_count: usize, edge_count: usize) {
        self.node_count = node_count;
        self.edge_count = edge_count;
        if node_count > 0 {
            self.avg_degree = (2.0 * edge_count as f32) / node_count as f32;
        }
        if node_count > 1 {
            let possible_edges = node_count * (node_count - 1);
            self.density = edge_count as f64 / possible_edges as f64;
        }
    }

    /// Add embedder connectivity.
    pub fn add_embedder_connectivity(&mut self, connectivity: EmbedderConnectivity) {
        self.embedder_connectivity
            .insert(connectivity.embedder, connectivity);
    }

    /// Add connectivity snapshot when embedder is removed.
    pub fn add_without(&mut self, embedder: EmbedderIndex, snapshot: ConnectivitySnapshot) {
        self.connectivity_without.insert(embedder, snapshot);
    }

    /// Get embedders sorted by connectivity impact.
    pub fn sorted_by_connectivity_impact(&self) -> Vec<(EmbedderIndex, f32)> {
        let mut impacts: Vec<_> = self
            .connectivity_without
            .iter()
            .map(|(&e, s)| (e, self.avg_degree - s.avg_degree))
            .collect();
        impacts.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        impacts
    }
}

/// Snapshot of connectivity metrics (for ablation comparison).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConnectivitySnapshot {
    /// Average degree.
    pub avg_degree: f32,
    /// Clustering coefficient.
    pub clustering_coefficient: f32,
    /// Edge count.
    pub edge_count: usize,
    /// Largest component size.
    pub largest_component_size: usize,
}

/// Weighted agreement analysis per embedder.
///
/// Per ARCH-09: Topic threshold is weighted_agreement >= 2.5.
/// This tracks how each embedder contributes to weighted agreement.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WeightedAgreementAnalysis {
    /// Average weighted agreement across all memory pairs.
    pub avg_weighted_agreement: f32,
    /// Per-embedder contribution to weighted agreement.
    pub embedder_contribution: HashMap<EmbedderIndex, f32>,
    /// Pairs that meet topic threshold (>= 2.5).
    pub pairs_meeting_threshold: usize,
    /// Total pairs analyzed.
    pub total_pairs: usize,
    /// Distribution of weighted agreement scores.
    pub agreement_distribution: AgreementDistribution,
    /// Topic weights per category.
    pub category_weights: CategoryWeights,
}

impl WeightedAgreementAnalysis {
    /// Create new weighted agreement analysis.
    pub fn new() -> Self {
        Self {
            category_weights: CategoryWeights::default(),
            ..Default::default()
        }
    }

    /// Record a pair's agreement.
    pub fn record_pair(&mut self, embedder_agreements: &[bool; 14], embedder_weights: &[f32; 14]) {
        self.total_pairs += 1;

        let mut weighted_agreement = 0.0f32;
        for (idx, (&agrees, &weight)) in embedder_agreements
            .iter()
            .zip(embedder_weights.iter())
            .enumerate()
        {
            if agrees {
                weighted_agreement += weight;
                if idx < 13 {
                    let embedder = EmbedderIndex::from_index(idx);
                    *self.embedder_contribution.entry(embedder).or_default() += weight;
                }
            }
        }

        self.avg_weighted_agreement += weighted_agreement;

        if weighted_agreement >= 2.5 {
            self.pairs_meeting_threshold += 1;
        }

        self.agreement_distribution.record(weighted_agreement);
    }

    /// Finalize calculations.
    pub fn finalize(&mut self) {
        if self.total_pairs > 0 {
            self.avg_weighted_agreement /= self.total_pairs as f32;
            for contrib in self.embedder_contribution.values_mut() {
                *contrib /= self.total_pairs as f32;
            }
        }
        self.agreement_distribution.finalize();
    }

    /// Percentage of pairs meeting topic threshold.
    pub fn threshold_rate(&self) -> f64 {
        if self.total_pairs == 0 {
            return 0.0;
        }
        (self.pairs_meeting_threshold as f64 / self.total_pairs as f64) * 100.0
    }

    /// Get embedders sorted by contribution to weighted agreement.
    pub fn sorted_contributions(&self) -> Vec<(EmbedderIndex, f32)> {
        let mut sorted: Vec<_> = self
            .embedder_contribution
            .iter()
            .map(|(&e, &c)| (e, c))
            .collect();
        sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        sorted
    }
}

/// Distribution of weighted agreement scores.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgreementDistribution {
    /// Histogram buckets: [0-0.5), [0.5-1), [1-1.5), [1.5-2), [2-2.5), [2.5-3), [3-4), [4-5), [5+).
    pub buckets: [usize; 9],
    /// Minimum observed.
    pub min: f32,
    /// Maximum observed.
    pub max: f32,
    /// Median (computed on finalize).
    pub median: f32,
    /// Standard deviation.
    pub std_dev: f32,
    /// All values (for computing median/std_dev).
    #[serde(skip)]
    values: Vec<f32>,
}

impl AgreementDistribution {
    /// Record a value.
    pub fn record(&mut self, value: f32) {
        self.values.push(value);

        // Update min/max
        if self.values.len() == 1 {
            self.min = value;
            self.max = value;
        } else {
            self.min = self.min.min(value);
            self.max = self.max.max(value);
        }

        // Update bucket
        let bucket = match value {
            v if v < 0.5 => 0,
            v if v < 1.0 => 1,
            v if v < 1.5 => 2,
            v if v < 2.0 => 3,
            v if v < 2.5 => 4,
            v if v < 3.0 => 5,
            v if v < 4.0 => 6,
            v if v < 5.0 => 7,
            _ => 8,
        };
        self.buckets[bucket] += 1;
    }

    /// Finalize calculations.
    pub fn finalize(&mut self) {
        if self.values.is_empty() {
            return;
        }

        // Compute median
        self.values
            .sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mid = self.values.len() / 2;
        self.median = if self.values.len() % 2 == 0 {
            (self.values[mid - 1] + self.values[mid]) / 2.0
        } else {
            self.values[mid]
        };

        // Compute std dev
        let mean: f32 = self.values.iter().sum::<f32>() / self.values.len() as f32;
        let variance: f32 =
            self.values.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / self.values.len() as f32;
        self.std_dev = variance.sqrt();

        // Clear values to save memory
        self.values.clear();
    }
}

/// Category weights for weighted agreement calculation.
///
/// Per Constitution v6.5:
/// - SEMANTIC (E1,E5,E6,E7,E10,E12,E13): weight 1.0
/// - RELATIONAL (E8,E11): weight 0.5
/// - STRUCTURAL (E9): weight 0.5
/// - TEMPORAL (E2,E3,E4): weight 0.0
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryWeights {
    /// Semantic embedder weights.
    pub semantic: f32,
    /// Relational embedder weights.
    pub relational: f32,
    /// Structural embedder weights.
    pub structural: f32,
    /// Temporal embedder weights (should be 0.0).
    pub temporal: f32,
}

impl Default for CategoryWeights {
    fn default() -> Self {
        Self {
            semantic: 1.0,
            relational: 0.5,
            structural: 0.5,
            temporal: 0.0,
        }
    }
}

impl CategoryWeights {
    /// Get weight for an embedder.
    pub fn weight_for(&self, embedder: EmbedderIndex) -> f32 {
        match embedder {
            // Semantic (weight 1.0)
            EmbedderIndex::E1Semantic
            | EmbedderIndex::E1Matryoshka128
            | EmbedderIndex::E5Causal
            | EmbedderIndex::E5CausalCause
            | EmbedderIndex::E5CausalEffect
            | EmbedderIndex::E6Sparse
            | EmbedderIndex::E7Code
            | EmbedderIndex::E10Multimodal
            | EmbedderIndex::E10MultimodalParaphrase
            | EmbedderIndex::E10MultimodalContext
            | EmbedderIndex::E12LateInteraction
            | EmbedderIndex::E13Splade => self.semantic,

            // Relational (weight 0.5)
            EmbedderIndex::E8Graph | EmbedderIndex::E11Entity => self.relational,

            // Structural (weight 0.5)
            EmbedderIndex::E9HDC => self.structural,

            // Temporal (weight 0.0)
            EmbedderIndex::E2TemporalRecent
            | EmbedderIndex::E3TemporalPeriodic
            | EmbedderIndex::E4TemporalPositional => self.temporal,

            // E14 BGE-M3 Dense — semantic category
            EmbedderIndex::E14BgeM3Dense => self.semantic,
        }
    }

    /// Get all 13 weights as array.
    pub fn as_array(&self) -> [f32; 14] {
        [
            self.semantic,   // E1
            self.temporal,   // E2
            self.temporal,   // E3
            self.temporal,   // E4
            self.semantic,   // E5
            self.semantic,   // E6
            self.semantic,   // E7
            self.relational, // E8
            self.structural, // E9
            self.semantic,   // E10
            self.relational, // E11
            self.semantic,   // E12
            self.semantic,   // E13
            self.semantic,   // E14 BGE-M3 Dense
        ]
    }

    /// Maximum possible weighted agreement (sum of all weights).
    pub fn max_weighted_agreement(&self) -> f32 {
        // 7 semantic (1.0 each) + 2 relational (0.5 each) + 1 structural (0.5)
        // = 7*1.0 + 2*0.5 + 1*0.5 = 8.5
        7.0 * self.semantic + 2.0 * self.relational + 1.0 * self.structural
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_category_weights() {
        let weights = CategoryWeights::default();

        // Semantic embedders should have weight 1.0
        assert_eq!(weights.weight_for(EmbedderIndex::E1Semantic), 1.0);
        assert_eq!(weights.weight_for(EmbedderIndex::E7Code), 1.0);

        // Relational embedders should have weight 0.5
        assert_eq!(weights.weight_for(EmbedderIndex::E8Graph), 0.5);
        assert_eq!(weights.weight_for(EmbedderIndex::E11Entity), 0.5);

        // Temporal embedders should have weight 0.0
        assert_eq!(weights.weight_for(EmbedderIndex::E2TemporalRecent), 0.0);
        assert_eq!(weights.weight_for(EmbedderIndex::E3TemporalPeriodic), 0.0);
        assert_eq!(weights.weight_for(EmbedderIndex::E4TemporalPositional), 0.0);

        // Max should be 8.5
        assert!((weights.max_weighted_agreement() - 8.5).abs() < 0.01);
    }

    #[test]
    fn test_topic_formation_impact() {
        let mut impact = TopicFormationImpact::new();
        impact.set_full(20, 6.5);

        // Removing E1 causes significant loss
        impact.add_without(EmbedderIndex::E1Semantic, 12, 4.0);
        assert!(impact
            .essential_for_topics
            .contains(&EmbedderIndex::E1Semantic));

        // Removing E2 (temporal) should have no effect
        impact.add_without(EmbedderIndex::E2TemporalRecent, 20, 6.5);
        assert!(!impact
            .essential_for_topics
            .contains(&EmbedderIndex::E2TemporalRecent));

        let sorted = impact.sorted_by_topic_impact();
        assert_eq!(sorted[0].0, EmbedderIndex::E1Semantic);
    }

    #[test]
    fn test_edge_type_distribution() {
        let mut dist = EdgeTypeDistribution::new(EmbedderIndex::E1Semantic);

        dist.add_edge(GraphLinkEdgeType::SemanticSimilar, 0.8, true);
        dist.add_edge(GraphLinkEdgeType::SemanticSimilar, 0.7, false);
        dist.add_edge(GraphLinkEdgeType::MultiAgreement, 0.9, true);

        dist.finalize(100);

        assert_eq!(dist.total_edges, 3);
        assert_eq!(dist.primary_contributor_count, 2);
        assert_eq!(dist.edge_coverage, 3.0);
        assert_eq!(
            dist.dominant_type(),
            Some(GraphLinkEdgeType::SemanticSimilar)
        );
    }

    #[test]
    fn test_weighted_agreement_analysis() {
        let mut analysis = WeightedAgreementAnalysis::new();
        let weights = CategoryWeights::default().as_array();

        // All semantic embedders agree
        let mut agreements = [false; 14];
        agreements[0] = true; // E1
        agreements[4] = true; // E5
        agreements[5] = true; // E6
        agreements[6] = true; // E7
        analysis.record_pair(&agreements, &weights);

        // High agreement (should be >= 2.5)
        analysis.finalize();

        assert!(analysis.avg_weighted_agreement >= 2.5);
        assert_eq!(analysis.pairs_meeting_threshold, 1);
    }

    #[test]
    fn test_temporal_zero_impact() {
        let mut impact = GraphStructureImpact::new();
        impact.topic_impact.set_full(20, 6.5);

        // Temporal embedders should not affect topic count
        impact
            .topic_impact
            .add_without(EmbedderIndex::E2TemporalRecent, 20, 6.5);
        impact
            .topic_impact
            .add_without(EmbedderIndex::E3TemporalPeriodic, 20, 6.5);
        impact
            .topic_impact
            .add_without(EmbedderIndex::E4TemporalPositional, 20, 6.5);

        assert!(impact.verify_temporal_zero_impact());
    }
}
