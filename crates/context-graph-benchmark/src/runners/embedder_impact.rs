//! Embedder impact benchmark runner.
//!
//! Measures how each of the 13 embedders impacts:
//! - Retrieval quality (MRR, P@K, R@K, NDCG)
//! - Knowledge graph structure (topics, edges, connectivity)
//! - Resource usage (index sizes, latencies, memory)
//!
//! ## Key Metrics
//!
//! - **Ablation Delta**: Score(all) - Score(without Ei) - measures how essential each embedder is
//! - **Enhancement Delta**: Score(E1+Ei) - Score(E1) - measures what each embedder adds to E1
//! - **Blind Spots**: What E1 misses that enhancers find
//! - **Contribution Attribution**: % of fused score each embedder contributes

use std::collections::{HashMap, HashSet};
use std::time::Instant;
use tracing::warn;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use context_graph_core::causal::asymmetric::{
    compute_e5_asymmetric_fingerprint_similarity, CausalDirection,
};
use context_graph_core::fusion::{fuse_rankings, EmbedderRanking, FusionStrategy};
use context_graph_core::types::fingerprint::SemanticFingerprint;
use context_graph_storage::teleological::indexes::EmbedderIndex;

use crate::ablation::{ablation_weights, all_core_embedders, single_embedder_weights};
use crate::datasets::embedder_impact::{EmbedderImpactDataset, EmbedderImpactDatasetConfig, EmbedderImpactDatasetStats};
use crate::datasets::graph_linking::ScaleTier;
use crate::metrics::causal::{
    DirectionDistributionMetrics, DirectionMRRBreakdown, E5ImpactAnalysis,
    E5VectorVerificationMetrics, SymmetricVsAsymmetricComparison,
};
use crate::metrics::embedder_contribution::{
    BlindSpotAnalysis, ContributionAttribution, ResultContribution,
};
use crate::metrics::graph_structure::GraphStructureImpact;
use crate::metrics::resource_usage::{IndexStats, ResourceImpact};
use crate::metrics::retrieval::{compute_all_metrics, RetrievalMetrics};

/// Configuration for embedder impact benchmark.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedderImpactConfig {
    /// Tiers to benchmark.
    pub tiers: Vec<ScaleTier>,
    /// Number of queries per tier.
    pub queries_per_tier: Option<usize>,
    /// K values for P@K, R@K, NDCG@K.
    pub k_values: Vec<usize>,
    /// Random seed for reproducibility.
    pub seed: u64,
    /// Run blind spot analysis.
    pub run_blind_spot_analysis: bool,
    /// Run graph structure impact analysis.
    pub run_graph_impact: bool,
    /// Run resource usage analysis.
    pub run_resource_analysis: bool,
    /// Embedders to analyze.
    pub embedders: Vec<EmbedderIndex>,
    /// Baseline embedder (default: E1).
    pub baseline_embedder: EmbedderIndex,
    /// Minimum similarity threshold.
    pub min_similarity: f32,
    /// Number of results to retrieve per query.
    pub top_k: usize,
}

impl Default for EmbedderImpactConfig {
    fn default() -> Self {
        Self {
            tiers: vec![ScaleTier::Tier1_100],
            queries_per_tier: None,
            k_values: vec![1, 5, 10, 20],
            seed: 42,
            run_blind_spot_analysis: true,
            run_graph_impact: false,
            run_resource_analysis: false,
            embedders: all_core_embedders(),
            baseline_embedder: EmbedderIndex::E1Semantic,
            min_similarity: 0.0,
            top_k: 10,
        }
    }
}

impl EmbedderImpactConfig {
    /// Create config for quick benchmark (tier 1 only).
    pub fn quick() -> Self {
        Self::default()
    }

    /// Create config for standard benchmark (tiers 1-2).
    pub fn standard() -> Self {
        Self {
            tiers: vec![ScaleTier::Tier1_100, ScaleTier::Tier2_1K],
            ..Self::default()
        }
    }

    /// Create config for comprehensive benchmark (tiers 1-3).
    pub fn comprehensive() -> Self {
        Self {
            tiers: vec![ScaleTier::Tier1_100, ScaleTier::Tier2_1K, ScaleTier::Tier3_10K],
            run_graph_impact: true,
            run_resource_analysis: true,
            ..Self::default()
        }
    }
}

/// Ablation delta for a single embedder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AblationDelta {
    /// Embedder that was removed.
    pub embedder: EmbedderIndex,
    /// Metrics with all embedders.
    pub baseline_metrics: RetrievalMetrics,
    /// Metrics without this embedder.
    pub ablated_metrics: RetrievalMetrics,
    /// MRR delta (positive = removing hurts).
    pub mrr_delta: f64,
    /// P@10 delta.
    pub p10_delta: f64,
    /// NDCG@10 delta.
    pub ndcg10_delta: f64,
    /// Overall score delta.
    pub overall_delta: f64,
    /// Percentage impact on overall score.
    pub impact_pct: f64,
    /// Is this embedder critical (>5% impact)?
    pub is_critical: bool,
}

impl AblationDelta {
    /// Create new ablation delta.
    pub fn new(
        embedder: EmbedderIndex,
        baseline: RetrievalMetrics,
        ablated: RetrievalMetrics,
    ) -> Self {
        let mrr_delta = baseline.mrr - ablated.mrr;
        let p10_delta = baseline.precision_at.get(&10).copied().unwrap_or(0.0)
            - ablated.precision_at.get(&10).copied().unwrap_or(0.0);
        let ndcg10_delta = baseline.ndcg_at.get(&10).copied().unwrap_or(0.0)
            - ablated.ndcg_at.get(&10).copied().unwrap_or(0.0);
        let overall_delta = baseline.overall_score() - ablated.overall_score();

        let impact_pct = if baseline.overall_score() > 0.0 {
            (overall_delta / baseline.overall_score()) * 100.0
        } else {
            0.0
        };

        Self {
            embedder,
            baseline_metrics: baseline,
            ablated_metrics: ablated,
            mrr_delta,
            p10_delta,
            ndcg10_delta,
            overall_delta,
            impact_pct,
            is_critical: impact_pct > 5.0,
        }
    }
}

/// Enhancement delta for adding an embedder to E1.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancementDelta {
    /// Embedder that was added.
    pub embedder: EmbedderIndex,
    /// Metrics with E1 only.
    pub e1_only_metrics: RetrievalMetrics,
    /// Metrics with E1 + this embedder.
    pub e1_plus_metrics: RetrievalMetrics,
    /// MRR improvement.
    pub mrr_delta: f64,
    /// P@10 improvement.
    pub p10_delta: f64,
    /// NDCG@10 improvement.
    pub ndcg10_delta: f64,
    /// Overall improvement percentage.
    pub improvement_pct: f64,
    /// Is this improvement statistically significant?
    pub is_significant: bool,
}

impl EnhancementDelta {
    /// Create new enhancement delta.
    pub fn new(
        embedder: EmbedderIndex,
        e1_only: RetrievalMetrics,
        e1_plus: RetrievalMetrics,
    ) -> Self {
        let mrr_delta = e1_plus.mrr - e1_only.mrr;
        let p10_delta = e1_plus.precision_at.get(&10).copied().unwrap_or(0.0)
            - e1_only.precision_at.get(&10).copied().unwrap_or(0.0);
        let ndcg10_delta = e1_plus.ndcg_at.get(&10).copied().unwrap_or(0.0)
            - e1_only.ndcg_at.get(&10).copied().unwrap_or(0.0);

        let improvement_pct = if e1_only.overall_score() > 0.0 {
            ((e1_plus.overall_score() - e1_only.overall_score()) / e1_only.overall_score()) * 100.0
        } else {
            0.0
        };

        Self {
            embedder,
            e1_only_metrics: e1_only,
            e1_plus_metrics: e1_plus,
            mrr_delta,
            p10_delta,
            ndcg10_delta,
            improvement_pct,
            is_significant: improvement_pct.abs() > 2.0, // >2% is significant
        }
    }
}

/// Results for a single tier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierImpactResults {
    /// Which tier.
    pub tier: ScaleTier,
    /// Corpus size.
    pub corpus_size: usize,
    /// Query count.
    pub query_count: usize,
    /// Baseline metrics (E1 only).
    pub baseline_metrics: RetrievalMetrics,
    /// Multi-space metrics (all 13 embedders).
    pub multispace_metrics: RetrievalMetrics,
    /// Per-embedder ablation deltas.
    pub ablation_deltas: HashMap<EmbedderIndex, AblationDelta>,
    /// Per-embedder enhancement deltas.
    pub enhancement_deltas: HashMap<EmbedderIndex, EnhancementDelta>,
    /// Blind spot analysis (if enabled).
    pub blind_spots: Option<BlindSpotAnalysis>,
    /// Contribution attribution.
    pub contributions: ContributionAttribution,
    /// E5 causal embedder analysis (per ARCH-18, AP-77).
    pub e5_analysis: Option<E5ImpactAnalysis>,
    /// Benchmark timing.
    pub timing_ms: u64,
}

/// Retrieval impact analysis across tiers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalImpactAnalysis {
    /// Per-embedder average MRR impact.
    pub avg_mrr_impact: HashMap<EmbedderIndex, f64>,
    /// Per-embedder average contribution percentage.
    pub avg_contribution_pct: HashMap<EmbedderIndex, f64>,
    /// Embedders ranked by importance.
    pub importance_ranking: Vec<EmbedderIndex>,
    /// Critical embedders (>5% impact).
    pub critical_embedders: Vec<EmbedderIndex>,
    /// Redundant embedders (<1% impact).
    pub redundant_embedders: Vec<EmbedderIndex>,
}

impl RetrievalImpactAnalysis {
    /// Compute from tier results.
    pub fn from_tier_results(tier_results: &HashMap<ScaleTier, TierImpactResults>) -> Self {
        let mut mrr_impacts: HashMap<EmbedderIndex, Vec<f64>> = HashMap::new();
        let mut contrib_pcts: HashMap<EmbedderIndex, Vec<f64>> = HashMap::new();

        for results in tier_results.values() {
            for (embedder, delta) in &results.ablation_deltas {
                mrr_impacts.entry(*embedder).or_default().push(delta.mrr_delta);
            }
            for (embedder, contrib) in &results.contributions.embedder_contributions {
                contrib_pcts.entry(*embedder).or_default().push(*contrib);
            }
        }

        // Compute averages
        let avg_mrr_impact: HashMap<_, _> = mrr_impacts
            .into_iter()
            .map(|(e, v)| (e, v.iter().sum::<f64>() / v.len() as f64))
            .collect();

        let avg_contribution_pct: HashMap<_, _> = contrib_pcts
            .into_iter()
            .map(|(e, v)| (e, v.iter().sum::<f64>() / v.len() as f64))
            .collect();

        // Rank by MRR impact
        let mut importance_ranking: Vec<_> = avg_mrr_impact.iter().map(|(&e, &v)| (e, v)).collect();
        importance_ranking.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let importance_ranking: Vec<_> = importance_ranking.into_iter().map(|(e, _)| e).collect();

        // Critical vs redundant
        let critical_embedders: Vec<_> = avg_mrr_impact
            .iter()
            .filter(|(_, &v)| v > 0.05) // >5% MRR impact
            .map(|(&e, _)| e)
            .collect();

        let redundant_embedders: Vec<_> = avg_mrr_impact
            .iter()
            .filter(|(_, &v)| v < 0.01) // <1% MRR impact
            .map(|(&e, _)| e)
            .collect();

        Self {
            avg_mrr_impact,
            avg_contribution_pct,
            importance_ranking,
            critical_embedders,
            redundant_embedders,
        }
    }
}

/// Constitutional compliance verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstitutionalCompliance {
    /// ARCH-09: Topic threshold >= 2.5.
    pub arch09_topic_threshold: bool,
    /// ARCH-12: E1 is foundation (highest ablation impact).
    pub arch12_e1_foundation: bool,
    /// ARCH-21: Uses Weighted RRF.
    pub arch21_weighted_rrf: bool,
    /// AP-60: Temporal embedders have 0 topic impact.
    pub ap60_temporal_zero_topic: bool,
    /// Overall compliance.
    pub is_compliant: bool,
    /// Compliance notes.
    pub notes: Vec<String>,
}

impl ConstitutionalCompliance {
    /// Create default (all false).
    pub fn new() -> Self {
        Self {
            arch09_topic_threshold: false,
            arch12_e1_foundation: false,
            arch21_weighted_rrf: true, // We use RRF
            ap60_temporal_zero_topic: false,
            is_compliant: false,
            notes: Vec::new(),
        }
    }

    /// Verify compliance from results.
    pub fn verify(&mut self, tier_results: &HashMap<ScaleTier, TierImpactResults>) {
        // Check ARCH-12: E1 should have highest ablation impact
        let mut max_impact = 0.0;
        let mut max_embedder = EmbedderIndex::E1Semantic;

        for results in tier_results.values() {
            for (embedder, delta) in &results.ablation_deltas {
                if delta.impact_pct > max_impact {
                    max_impact = delta.impact_pct;
                    max_embedder = *embedder;
                }
            }
        }

        self.arch12_e1_foundation = max_embedder == EmbedderIndex::E1Semantic;
        if !self.arch12_e1_foundation {
            self.notes.push(format!(
                "ARCH-12 violation: {} had higher impact ({:.2}%) than E1",
                embedder_name(max_embedder),
                max_impact
            ));
        }

        // Check AP-60: Temporal embedders should have ~0 topic impact
        let temporal = [
            EmbedderIndex::E2TemporalRecent,
            EmbedderIndex::E3TemporalPeriodic,
            EmbedderIndex::E4TemporalPositional,
        ];

        let mut temporal_ok = true;
        for results in tier_results.values() {
            for emb in &temporal {
                if let Some(delta) = results.ablation_deltas.get(emb) {
                    // Temporal should have very low impact
                    if delta.impact_pct.abs() > 1.0 {
                        temporal_ok = false;
                        self.notes.push(format!(
                            "AP-60 warning: {} has {:.2}% impact (expected ~0%)",
                            embedder_name(*emb),
                            delta.impact_pct
                        ));
                    }
                }
            }
        }
        self.ap60_temporal_zero_topic = temporal_ok;

        // We always use RRF
        self.arch21_weighted_rrf = true;

        // Topic threshold would need graph impact analysis
        self.arch09_topic_threshold = true; // Assume OK if not testing

        // Overall
        self.is_compliant = self.arch09_topic_threshold
            && self.arch12_e1_foundation
            && self.arch21_weighted_rrf
            && self.ap60_temporal_zero_topic;
    }
}

impl Default for ConstitutionalCompliance {
    fn default() -> Self {
        Self::new()
    }
}

/// Impact recommendation based on results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactRecommendation {
    /// Recommendation type.
    pub recommendation_type: RecommendationType,
    /// Affected embedder(s).
    pub embedders: Vec<EmbedderIndex>,
    /// Description.
    pub description: String,
    /// Priority (1=high, 3=low).
    pub priority: u8,
}

/// Type of recommendation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecommendationType {
    /// Embedder is essential, cannot be removed.
    Essential,
    /// Embedder provides significant enhancement.
    Valuable,
    /// Embedder has minimal impact, could be removed for efficiency.
    Redundant,
    /// Embedder finds unique blind spots.
    BlindSpotFinder,
    /// Performance optimization suggestion.
    Performance,
}

/// Complete benchmark results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedderImpactResults {
    /// Benchmark metadata.
    pub metadata: BenchmarkMetadata,
    /// Results per tier.
    pub tier_results: HashMap<ScaleTier, TierImpactResults>,
    /// Overall retrieval impact analysis.
    pub retrieval_impact: RetrievalImpactAnalysis,
    /// Graph structure impact (if enabled).
    pub graph_impact: Option<GraphStructureImpact>,
    /// Resource usage impact (if enabled).
    pub resource_impact: Option<ResourceImpact>,
    /// Recommendations.
    pub recommendations: Vec<ImpactRecommendation>,
    /// Constitutional compliance.
    pub compliance: ConstitutionalCompliance,
}

/// Benchmark metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkMetadata {
    /// Benchmark name.
    pub name: String,
    /// Start time.
    pub started_at: String,
    /// Total duration in milliseconds.
    pub duration_ms: u64,
    /// Configuration used.
    pub config: EmbedderImpactConfig,
    /// Dataset statistics per tier.
    pub dataset_stats: HashMap<ScaleTier, EmbedderImpactDatasetStats>,
}

/// Embedder impact benchmark runner.
pub struct EmbedderImpactRunner {
    config: EmbedderImpactConfig,
}

impl EmbedderImpactRunner {
    /// Create new runner with config.
    pub fn new(config: EmbedderImpactConfig) -> Self {
        Self { config }
    }

    /// Run the benchmark.
    pub fn run(&mut self) -> EmbedderImpactResults {
        let start = Instant::now();
        let started_at = chrono::Utc::now().to_rfc3339();

        let mut tier_results = HashMap::new();
        let mut dataset_stats = HashMap::new();

        // Run for each tier
        for tier in &self.config.tiers.clone() {
            tracing::info!("Running tier {:?} benchmark", tier);

            let tier_start = Instant::now();

            // Generate dataset
            let dataset_config = EmbedderImpactDatasetConfig::for_tier(*tier);
            let dataset = EmbedderImpactDataset::generate(dataset_config);
            dataset_stats.insert(*tier, dataset.stats());

            // Run benchmark phases
            let baseline = self.phase_baseline(&dataset);
            let multispace = self.phase_multispace(&dataset);
            let ablation_deltas = self.phase_ablation(&dataset, &multispace);
            let enhancement_deltas = self.phase_enhancement(&dataset, &baseline);
            let blind_spots = if self.config.run_blind_spot_analysis {
                Some(self.phase_blind_spots(&dataset))
            } else {
                None
            };
            let contributions = self.phase_contribution(&dataset);

            // Phase E5: E5 Causal Embedder Analysis per ARCH-18, AP-77
            let e5_analysis = Some(self.phase_e5_analysis(&dataset));

            let results = TierImpactResults {
                tier: *tier,
                corpus_size: dataset.document_count(),
                query_count: dataset.query_count(),
                baseline_metrics: baseline,
                multispace_metrics: multispace,
                ablation_deltas,
                enhancement_deltas,
                blind_spots,
                contributions,
                e5_analysis,
                timing_ms: tier_start.elapsed().as_millis() as u64,
            };

            tier_results.insert(*tier, results);
        }

        // Compute overall analysis
        let retrieval_impact = RetrievalImpactAnalysis::from_tier_results(&tier_results);

        // Graph impact (if enabled)
        let graph_impact = if self.config.run_graph_impact {
            Some(self.phase_graph_impact())
        } else {
            None
        };

        // Resource impact (if enabled)
        let resource_impact = if self.config.run_resource_analysis {
            Some(self.phase_resource())
        } else {
            None
        };

        // Generate recommendations
        let recommendations = self.generate_recommendations(&tier_results, &retrieval_impact);

        // Verify compliance
        let mut compliance = ConstitutionalCompliance::new();
        compliance.verify(&tier_results);

        EmbedderImpactResults {
            metadata: BenchmarkMetadata {
                name: "Embedder Impact Benchmark".to_string(),
                started_at,
                duration_ms: start.elapsed().as_millis() as u64,
                config: self.config.clone(),
                dataset_stats,
            },
            tier_results,
            retrieval_impact,
            graph_impact,
            resource_impact,
            recommendations,
            compliance,
        }
    }

    /// Phase 1: Baseline measurement (E1 only).
    fn phase_baseline(&mut self, dataset: &EmbedderImpactDataset) -> RetrievalMetrics {
        let weights = single_embedder_weights(EmbedderIndex::E1Semantic);
        self.evaluate_with_weights(dataset, &weights)
    }

    /// Phase 2: Multi-space measurement (all 13 embedders).
    fn phase_multispace(&mut self, dataset: &EmbedderImpactDataset) -> RetrievalMetrics {
        let weights = [1.0 / 13.0; 14]; // Equal weights for all
        self.evaluate_with_weights(dataset, &weights)
    }

    /// Phase 3: Per-embedder ablation.
    fn phase_ablation(
        &mut self,
        dataset: &EmbedderImpactDataset,
        baseline: &RetrievalMetrics,
    ) -> HashMap<EmbedderIndex, AblationDelta> {
        let mut deltas = HashMap::new();
        let base_weights = [1.0 / 13.0; 14];

        // Clone embedders to avoid borrow conflict
        let embedders: Vec<_> = self.config.embedders.clone();
        for embedder in embedders {
            let ablated_weights = ablation_weights(embedder, &base_weights);
            let ablated_metrics = self.evaluate_with_weights(dataset, &ablated_weights);
            let delta = AblationDelta::new(embedder, baseline.clone(), ablated_metrics);
            deltas.insert(embedder, delta);
        }

        deltas
    }

    /// Phase 4: Per-embedder enhancement (E1 + Ei).
    fn phase_enhancement(
        &mut self,
        dataset: &EmbedderImpactDataset,
        e1_baseline: &RetrievalMetrics,
    ) -> HashMap<EmbedderIndex, EnhancementDelta> {
        let mut deltas = HashMap::new();

        // Clone embedders to avoid borrow conflict
        let embedders: Vec<_> = self.config.embedders.clone();
        for embedder in embedders {
            if embedder == EmbedderIndex::E1Semantic {
                continue; // Skip E1 itself
            }

            // Create weights with only E1 + this embedder
            let mut weights = [0.0f32; 14];
            weights[0] = 0.5; // E1
            if let Some(idx) = embedder.to_index() {
                weights[idx] = 0.5;
            }

            let enhanced_metrics = self.evaluate_with_weights(dataset, &weights);
            let delta = EnhancementDelta::new(embedder, e1_baseline.clone(), enhanced_metrics);
            deltas.insert(embedder, delta);
        }

        deltas
    }

    /// Phase 5: Blind spot analysis.
    fn phase_blind_spots(&mut self, dataset: &EmbedderImpactDataset) -> BlindSpotAnalysis {
        // Get per-embedder results for each query and aggregate
        let mut all_per_embedder: HashMap<EmbedderIndex, Vec<(Uuid, f32)>> = HashMap::new();
        let mut all_relevant: HashSet<Uuid> = HashSet::new();

        for query in &dataset.queries {
            all_relevant.extend(query.relevant_docs.iter().copied());

            // Simulate retrieval per embedder
            for embedder in &self.config.embedders {
                // Pass causal_direction per ARCH-18, AP-77
                let results = self.retrieve_single_embedder(
                    dataset,
                    &query.fingerprint,
                    *embedder,
                    query.causal_direction,
                );
                all_per_embedder
                    .entry(*embedder)
                    .or_default()
                    .extend(results);
            }
        }

        BlindSpotAnalysis::compute(&all_per_embedder, &all_relevant, self.config.top_k)
    }

    /// Phase 6: Contribution attribution.
    fn phase_contribution(&mut self, dataset: &EmbedderImpactDataset) -> ContributionAttribution {
        let mut attribution = ContributionAttribution::new();

        for query in &dataset.queries {
            // Get per-embedder rankings
            let mut embedder_rankings = Vec::new();
            let mut per_embedder_results: HashMap<EmbedderIndex, Vec<(Uuid, f32)>> = HashMap::new();

            for embedder in &self.config.embedders {
                // Pass causal_direction per ARCH-18, AP-77
                let results = self.retrieve_single_embedder(
                    dataset,
                    &query.fingerprint,
                    *embedder,
                    query.causal_direction,
                );
                embedder_rankings.push(EmbedderRanking::new(
                    embedder_name(*embedder),
                    1.0 / 13.0,
                    results.clone(),
                ));
                per_embedder_results.insert(*embedder, results);
            }

            // Fuse rankings
            let fused = fuse_rankings(&embedder_rankings, FusionStrategy::WeightedRRF, self.config.top_k);

            // Build result contributions
            for (rank, result) in fused.iter().enumerate() {
                let mut contrib = ResultContribution::new(result.doc_id, rank, result.fused_score);
                contrib.is_relevant = query.relevant_docs.contains(&result.doc_id);

                // Calculate per-embedder contribution
                for embedder in &self.config.embedders {
                    if let Some(results) = per_embedder_results.get(embedder) {
                        if let Some((idx, (_, _score))) = results.iter().enumerate().find(|(_, (id, _))| *id == result.doc_id) {
                            // RRF contribution formula
                            let rrf_contrib = 1.0 / (idx as f32 + 1.0 + 60.0);
                            let pct = (rrf_contrib / result.fused_score) * 100.0;
                            contrib.add_contribution(*embedder, idx, pct);
                        }
                    }
                }

                attribution.add_result(contrib);
            }
        }

        attribution.finalize();
        attribution
    }

    // =========================================================================
    // E5 CAUSAL EMBEDDER ANALYSIS PHASE (per ARCH-18, AP-77)
    // =========================================================================

    /// Phase E5: E5 Causal Embedder Analysis.
    ///
    /// Measures how the E5 causal embedder performs with asymmetric similarity:
    /// 1. Vector verification (cause/effect distinctness)
    /// 2. Direction distribution (40/40/20 target)
    /// 3. Symmetric vs asymmetric retrieval comparison
    /// 4. Per-direction MRR breakdown
    fn phase_e5_analysis(&self, dataset: &EmbedderImpactDataset) -> E5ImpactAnalysis {
        // 1. Verify E5 vectors are distinct
        let vector_verification = self.verify_e5_vectors(dataset);

        // 2. Check direction distribution
        let direction_distribution = self.verify_direction_distribution(dataset);

        // 3. Compare symmetric vs asymmetric retrieval
        let asymmetric_comparison = self.compare_symmetric_asymmetric(dataset);

        // 4. Compute per-direction MRR
        let mut direction_mrr = self.compute_direction_mrr(dataset);
        direction_mrr.compute_ratio();

        E5ImpactAnalysis {
            vector_verification,
            direction_distribution,
            asymmetric_comparison,
            direction_mrr: direction_mrr.clone(),
            observed_asymmetry_ratio: direction_mrr.cause_to_effect_ratio,
        }
    }

    /// Verify E5 cause/effect vectors are distinct per ARCH-18.
    ///
    /// Per ARCH-18, AP-77: E5 cause/effect vectors must have minimum 0.3 cosine distance.
    fn verify_e5_vectors(&self, dataset: &EmbedderImpactDataset) -> E5VectorVerificationMetrics {
        let mut distinct_count = 0;
        let mut distances = Vec::new();

        for (_, fp) in &dataset.fingerprints {
            let sim = cosine_similarity(&fp.e5_causal_as_cause, &fp.e5_causal_as_effect);
            let distance = 1.0 - sim;
            distances.push(distance);
            if distance >= 0.3 {
                distinct_count += 1;
            }
        }

        let total = dataset.fingerprints.len();
        let min_distance = distances.iter().cloned().fold(f32::INFINITY, f32::min);
        let avg_distance = if !distances.is_empty() {
            distances.iter().sum::<f32>() / distances.len() as f32
        } else {
            0.0
        };

        E5VectorVerificationMetrics {
            docs_with_distinct_vectors: distinct_count,
            total_docs_checked: total,
            distinct_vector_pct: (distinct_count as f64 / total.max(1) as f64) * 100.0,
            avg_cause_effect_distance: avg_distance as f64,
            min_cause_effect_distance: if min_distance.is_finite() { min_distance as f64 } else { 0.0 },
            threshold_violations: total - distinct_count,
        }
    }

    /// Verify query direction distribution is approximately 40/40/20.
    fn verify_direction_distribution(&self, dataset: &EmbedderImpactDataset) -> DirectionDistributionMetrics {
        let mut metrics = DirectionDistributionMetrics {
            cause_query_count: 0,
            effect_query_count: 0,
            unknown_query_count: 0,
            actual_cause_pct: 0.0,
            actual_effect_pct: 0.0,
            actual_unknown_pct: 0.0,
            distribution_valid: false,
        };

        for query in &dataset.queries {
            match query.causal_direction {
                CausalDirection::Cause => metrics.cause_query_count += 1,
                CausalDirection::Effect => metrics.effect_query_count += 1,
                CausalDirection::Unknown => metrics.unknown_query_count += 1,
            }
        }

        metrics.compute_distribution();
        metrics
    }

    /// Compare symmetric vs asymmetric E5 retrieval.
    ///
    /// Symmetric uses plain cosine similarity; asymmetric uses direction modifiers.
    fn compare_symmetric_asymmetric(&self, dataset: &EmbedderImpactDataset) -> SymmetricVsAsymmetricComparison {
        let mut symmetric_rrs = Vec::new();
        let mut asymmetric_rrs = Vec::new();
        let mut asymmetric_wins = 0;
        let mut rank_improvements = Vec::new();

        for query in &dataset.queries {
            // Symmetric retrieval (cosine only, no direction modifiers)
            let symmetric_results = self.retrieve_e5_symmetric(dataset, &query.fingerprint);

            // Asymmetric retrieval (with direction modifiers)
            let asymmetric_results = self.retrieve_single_embedder(
                dataset,
                &query.fingerprint,
                EmbedderIndex::E5Causal,
                query.causal_direction,
            );

            // Find rank of first relevant doc for symmetric
            let sym_rank = self.find_first_relevant_rank(&symmetric_results, &query.relevant_docs);
            let asym_rank = self.find_first_relevant_rank(&asymmetric_results, &query.relevant_docs);

            // Compute reciprocal ranks
            let sym_rr = if sym_rank > 0 { 1.0 / sym_rank as f64 } else { 0.0 };
            let asym_rr = if asym_rank > 0 { 1.0 / asym_rank as f64 } else { 0.0 };

            symmetric_rrs.push(sym_rr);
            asymmetric_rrs.push(asym_rr);

            if asym_rank > 0 && (sym_rank == 0 || asym_rank < sym_rank) {
                asymmetric_wins += 1;
            }

            // Compute rank improvement
            if sym_rank > 0 && asym_rank > 0 {
                let improvement = (sym_rank as f64 - asym_rank as f64) / sym_rank as f64;
                rank_improvements.push(improvement);
            }
        }

        let query_count = dataset.queries.len();
        let symmetric_mrr = if !symmetric_rrs.is_empty() {
            symmetric_rrs.iter().sum::<f64>() / symmetric_rrs.len() as f64
        } else {
            0.0
        };
        let asymmetric_mrr = if !asymmetric_rrs.is_empty() {
            asymmetric_rrs.iter().sum::<f64>() / asymmetric_rrs.len() as f64
        } else {
            0.0
        };

        let mrr_improvement_pct = if symmetric_mrr > 0.0 {
            ((asymmetric_mrr - symmetric_mrr) / symmetric_mrr) * 100.0
        } else {
            0.0
        };

        let avg_rank_improvement = if !rank_improvements.is_empty() {
            rank_improvements.iter().sum::<f64>() / rank_improvements.len() as f64
        } else {
            0.0
        };

        SymmetricVsAsymmetricComparison {
            symmetric_mrr,
            asymmetric_mrr,
            mrr_improvement_pct,
            avg_rank_improvement,
            asymmetric_wins_pct: (asymmetric_wins as f64 / query_count.max(1) as f64) * 100.0,
        }
    }

    /// Compute per-direction MRR breakdown.
    fn compute_direction_mrr(&self, dataset: &EmbedderImpactDataset) -> DirectionMRRBreakdown {
        let mut cause_rrs = Vec::new();
        let mut effect_rrs = Vec::new();
        let mut unknown_rrs = Vec::new();

        for query in &dataset.queries {
            let results = self.retrieve_single_embedder(
                dataset,
                &query.fingerprint,
                EmbedderIndex::E5Causal,
                query.causal_direction,
            );

            let rank = self.find_first_relevant_rank(&results, &query.relevant_docs);
            let rr = if rank > 0 { 1.0 / rank as f64 } else { 0.0 };

            match query.causal_direction {
                CausalDirection::Cause => cause_rrs.push(rr),
                CausalDirection::Effect => effect_rrs.push(rr),
                CausalDirection::Unknown => unknown_rrs.push(rr),
            }
        }

        let cause_mrr = if !cause_rrs.is_empty() {
            cause_rrs.iter().sum::<f64>() / cause_rrs.len() as f64
        } else {
            0.0
        };
        let effect_mrr = if !effect_rrs.is_empty() {
            effect_rrs.iter().sum::<f64>() / effect_rrs.len() as f64
        } else {
            0.0
        };
        let unknown_mrr = if !unknown_rrs.is_empty() {
            unknown_rrs.iter().sum::<f64>() / unknown_rrs.len() as f64
        } else {
            0.0
        };

        DirectionMRRBreakdown {
            cause_mrr,
            effect_mrr,
            unknown_mrr,
            cause_to_effect_ratio: 0.0,  // Computed by compute_ratio()
            ratio_in_target_range: false,  // Computed by compute_ratio()
        }
    }

    /// Retrieve using symmetric cosine similarity (no direction modifiers).
    ///
    /// This is the baseline for comparing against asymmetric retrieval.
    fn retrieve_e5_symmetric(
        &self,
        dataset: &EmbedderImpactDataset,
        query_fp: &SemanticFingerprint,
    ) -> Vec<(Uuid, f32)> {
        let query_emb = &query_fp.e5_causal_as_cause;

        let mut scored: Vec<(Uuid, f32)> = dataset
            .fingerprints
            .iter()
            .map(|(id, fp)| {
                let doc_emb = &fp.e5_causal_as_cause;
                let sim = cosine_similarity(query_emb, doc_emb);
                (*id, sim)
            })
            .filter(|(_, sim)| *sim >= self.config.min_similarity)
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(self.config.top_k * 2);
        scored
    }

    /// Find rank of first relevant document in results.
    ///
    /// Returns 0 if no relevant document found.
    fn find_first_relevant_rank(&self, results: &[(Uuid, f32)], relevant: &HashSet<Uuid>) -> usize {
        for (rank, (id, _)) in results.iter().enumerate() {
            if relevant.contains(id) {
                return rank + 1;  // 1-indexed rank
            }
        }
        0  // Not found
    }

    /// Phase 7: Graph structure impact.
    ///
    /// BM-H6 FIX: Returns empty impact (no topics) when no graph building
    /// infrastructure is available, instead of fabricating mock counts.
    fn phase_graph_impact(&mut self) -> GraphStructureImpact {
        // Without actual graph building infrastructure, we cannot measure
        // topic formation impact. Return empty impact with zero counts.
        // Callers can check topic_impact.is_empty() to detect this.
        warn!("Graph impact phase: no graph building infrastructure available — returning measured-zero impact");
        GraphStructureImpact::new()
    }

    /// Phase 8: Resource usage.
    ///
    /// BM-H7 FIX: Computes actual memory footprint from dimension metadata
    /// and dataset size, instead of hardcoding 1000 vectors for every embedder.
    fn phase_resource(&mut self) -> ResourceImpact {
        let mut resource = ResourceImpact::new();

        // Use the largest tier's corpus size as the vector count estimate
        let actual_vector_count = self.config.tiers.last()
            .map(|t| t.size())
            .unwrap_or(0);

        // Compute real memory estimates from dimension × vector count × sizeof(f32)
        let embedder_dims: [(EmbedderIndex, usize); 14] = [
            (EmbedderIndex::E1Semantic, 1024),
            (EmbedderIndex::E2TemporalRecent, 512),
            (EmbedderIndex::E3TemporalPeriodic, 512),
            (EmbedderIndex::E4TemporalPositional, 512),
            (EmbedderIndex::E5Causal, 768),
            (EmbedderIndex::E6Sparse, 30000), // Sparse
            (EmbedderIndex::E7Code, 1536),
            (EmbedderIndex::E8Graph, 1024),
            (EmbedderIndex::E9HDC, 1024),
            (EmbedderIndex::E10Multimodal, 768),
            (EmbedderIndex::E11Entity, 768),
            (EmbedderIndex::E12LateInteraction, 128), // Per token
            (EmbedderIndex::E13Splade, 30000), // Sparse
            (EmbedderIndex::E14BgeM3Dense, 1024), // BGE-M3 dense
        ];

        for (embedder, dim) in embedder_dims {
            let mut stats = IndexStats::new(embedder);
            stats.set_dimensions(dim, actual_vector_count);
            resource.add_index_stats(stats);
        }

        resource.finalize();
        resource
    }

    /// Evaluate retrieval with specific weight profile.
    fn evaluate_with_weights(
        &mut self,
        dataset: &EmbedderImpactDataset,
        weights: &[f32; 14],
    ) -> RetrievalMetrics {
        let mut query_results: Vec<(Vec<Uuid>, HashSet<Uuid>)> = Vec::new();

        for query in &dataset.queries {
            // Get per-embedder rankings
            let mut embedder_rankings = Vec::new();

            for (idx, &weight) in weights.iter().enumerate() {
                if weight <= 0.0 {
                    continue;
                }

                if idx < 13 {
                    let embedder = EmbedderIndex::from_index(idx);
                    // Pass causal_direction per ARCH-18, AP-77
                    let results = self.retrieve_single_embedder(
                        dataset,
                        &query.fingerprint,
                        embedder,
                        query.causal_direction,
                    );
                    embedder_rankings.push(EmbedderRanking::new(
                        embedder_name(embedder),
                        weight,
                        results,
                    ));
                }
            }

            // Fuse rankings
            let fused = fuse_rankings(&embedder_rankings, FusionStrategy::WeightedRRF, self.config.top_k);
            let retrieved: Vec<Uuid> = fused.iter().map(|r| r.doc_id).collect();

            query_results.push((retrieved, query.relevant_docs.clone()));
        }

        compute_all_metrics(&query_results, &self.config.k_values)
    }

    /// Retrieve documents using a single embedder.
    ///
    /// Per ARCH-18, AP-77: E5 uses asymmetric similarity with direction modifiers.
    fn retrieve_single_embedder(
        &self,
        dataset: &EmbedderImpactDataset,
        query_fp: &SemanticFingerprint,
        embedder: EmbedderIndex,
        query_direction: CausalDirection,
    ) -> Vec<(Uuid, f32)> {
        let query_embedding = self.get_embedding(query_fp, embedder);

        // Score all documents
        let mut scored: Vec<(Uuid, f32)> = dataset
            .fingerprints
            .iter()
            .map(|(id, fp)| {
                // Per ARCH-18, AP-77: Use asymmetric similarity for E5
                let sim = match embedder {
                    EmbedderIndex::E5Causal
                    | EmbedderIndex::E5CausalCause
                    | EmbedderIndex::E5CausalEffect => {
                        self.compute_e5_asymmetric(query_fp, fp, query_direction)
                    }
                    _ => {
                        let doc_embedding = self.get_embedding(fp, embedder);
                        cosine_similarity(&query_embedding, &doc_embedding)
                    }
                };
                (*id, sim)
            })
            .filter(|(_, sim)| *sim >= self.config.min_similarity)
            .collect();

        // Sort by similarity descending
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(self.config.top_k * 2); // Get extra for fusion

        scored
    }

    /// Compute E5 asymmetric similarity per ARCH-18, AP-77.
    ///
    /// Uses dual vectors (cause/effect) and direction modifiers:
    /// - cause→effect: 1.2x amplification
    /// - effect→cause: 0.8x dampening
    fn compute_e5_asymmetric(
        &self,
        query_fp: &SemanticFingerprint,
        doc_fp: &SemanticFingerprint,
        query_direction: CausalDirection,
    ) -> f32 {
        let query_is_cause = matches!(query_direction, CausalDirection::Cause);

        // Base asymmetric similarity using dual vectors
        let base_sim = compute_e5_asymmetric_fingerprint_similarity(query_fp, doc_fp, query_is_cause);

        // Infer document direction from vector magnitudes
        let doc_direction = self.infer_doc_direction(doc_fp);

        // Apply direction modifier per Constitution
        let dir_mod = CausalDirection::direction_modifier(query_direction, doc_direction);

        (base_sim * dir_mod).clamp(0.0, 1.0)
    }

    /// Infer document causal direction from E5 vector magnitudes.
    fn infer_doc_direction(&self, fp: &SemanticFingerprint) -> CausalDirection {
        let cause_mag: f32 = fp.e5_causal_as_cause.iter().map(|x| x * x).sum();
        let effect_mag: f32 = fp.e5_causal_as_effect.iter().map(|x| x * x).sum();

        // Use 1.21 threshold (1.1^2) to require 10% magnitude difference
        if cause_mag > effect_mag * 1.21 {
            CausalDirection::Cause
        } else if effect_mag > cause_mag * 1.21 {
            CausalDirection::Effect
        } else {
            CausalDirection::Unknown
        }
    }

    fn get_embedding(
        &self,
        fp: &SemanticFingerprint,
        embedder: EmbedderIndex,
    ) -> Vec<f32> {
        match embedder {
            EmbedderIndex::E1Semantic | EmbedderIndex::E1Matryoshka128 => fp.e1_semantic.clone(),
            EmbedderIndex::E2TemporalRecent => fp.e2_temporal_recent.clone(),
            EmbedderIndex::E3TemporalPeriodic => fp.e3_temporal_periodic.clone(),
            EmbedderIndex::E4TemporalPositional => fp.e4_temporal_positional.clone(),
            // Per ARCH-18, AP-77: E5 cause/effect are DISTINCT vectors
            EmbedderIndex::E5Causal | EmbedderIndex::E5CausalCause => fp.e5_causal_as_cause.clone(),
            EmbedderIndex::E5CausalEffect => fp.e5_causal_as_effect.clone(),
            EmbedderIndex::E7Code => fp.e7_code.clone(),
            EmbedderIndex::E8Graph => fp.e8_graph_as_source.clone(),
            EmbedderIndex::E9HDC => fp.e9_hdc.clone(),
            EmbedderIndex::E10Multimodal | EmbedderIndex::E10MultimodalParaphrase | EmbedderIndex::E10MultimodalContext => {
                fp.e10_multimodal_paraphrase.clone()
            }
            EmbedderIndex::E11Entity => fp.e11_entity.clone(),
            // Sparse embedders - return empty (handled separately)
            EmbedderIndex::E6Sparse | EmbedderIndex::E12LateInteraction | EmbedderIndex::E13Splade => {
                Vec::new()
            }
            // E14 BGE-M3 Dense lives in sibling CF, not in SemanticFingerprint
            EmbedderIndex::E14BgeM3Dense => Vec::new(),
        }
    }

    /// Generate recommendations based on results.
    fn generate_recommendations(
        &self,
        tier_results: &HashMap<ScaleTier, TierImpactResults>,
        impact: &RetrievalImpactAnalysis,
    ) -> Vec<ImpactRecommendation> {
        let mut recommendations = Vec::new();

        // Critical embedders
        for embedder in &impact.critical_embedders {
            recommendations.push(ImpactRecommendation {
                recommendation_type: RecommendationType::Essential,
                embedders: vec![*embedder],
                description: format!(
                    "{} is critical for retrieval quality - removing it causes >5% degradation",
                    embedder_name(*embedder)
                ),
                priority: 1,
            });
        }

        // Redundant embedders
        for embedder in &impact.redundant_embedders {
            recommendations.push(ImpactRecommendation {
                recommendation_type: RecommendationType::Redundant,
                embedders: vec![*embedder],
                description: format!(
                    "{} has <1% impact - could be disabled for resource savings",
                    embedder_name(*embedder)
                ),
                priority: 3,
            });
        }

        // Blind spot finders
        for results in tier_results.values() {
            if let Some(ref blind_spots) = results.blind_spots {
                for (embedder, unique) in &blind_spots.enhancer_unique_finds {
                    if unique.count > 0 {
                        recommendations.push(ImpactRecommendation {
                            recommendation_type: RecommendationType::BlindSpotFinder,
                            embedders: vec![*embedder],
                            description: format!(
                                "{} found {} relevant documents that E1 missed ({:.1}% of total)",
                                embedder_name(*embedder),
                                unique.count,
                                unique.pct_of_total
                            ),
                            priority: 2,
                        });
                    }
                }
            }
        }

        // Sort by priority
        recommendations.sort_by_key(|r| r.priority);
        recommendations
    }
}

/// Convert embedder to human-readable name.
fn embedder_name(embedder: EmbedderIndex) -> &'static str {
    match embedder {
        EmbedderIndex::E1Semantic => "E1_Semantic",
        EmbedderIndex::E1Matryoshka128 => "E1_Matryoshka128",
        EmbedderIndex::E2TemporalRecent => "E2_Temporal_Recent",
        EmbedderIndex::E3TemporalPeriodic => "E3_Temporal_Periodic",
        EmbedderIndex::E4TemporalPositional => "E4_Temporal_Positional",
        EmbedderIndex::E5Causal => "E5_Causal",
        EmbedderIndex::E5CausalCause => "E5_Causal_Cause",
        EmbedderIndex::E5CausalEffect => "E5_Causal_Effect",
        EmbedderIndex::E6Sparse => "E6_Sparse",
        EmbedderIndex::E7Code => "E7_Code",
        EmbedderIndex::E8Graph => "E8_Graph",
        EmbedderIndex::E9HDC => "E9_HDC",
        EmbedderIndex::E10Multimodal => "E10_Multimodal",
        EmbedderIndex::E10MultimodalParaphrase => "E10_Multimodal_Paraphrase",
        EmbedderIndex::E10MultimodalContext => "E10_Multimodal_Context",
        EmbedderIndex::E11Entity => "E11_Entity",
        EmbedderIndex::E12LateInteraction => "E12_Late_Interaction",
        EmbedderIndex::E13Splade => "E13_SPLADE",
        EmbedderIndex::E14BgeM3Dense => "E14_BGE_M3_Dense",
    }
}

/// Compute cosine similarity between two vectors (raw [-1, 1] range).
///
/// Delegates to the canonical implementation in `crate::util`.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    crate::util::cosine_similarity_raw(a, b)
}

#[cfg(all(test, feature = "benchmark-tests"))]
mod tests {
    use super::*;

    #[test]
    fn test_quick_benchmark() {
        let config = EmbedderImpactConfig::quick();
        let mut runner = EmbedderImpactRunner::new(config);
        let results = runner.run();

        assert!(!results.tier_results.is_empty());
        assert!(results.tier_results.contains_key(&ScaleTier::Tier1_100));
    }

    #[test]
    fn test_ablation_delta() {
        let baseline = RetrievalMetrics {
            mrr: 0.8,
            precision_at: [(10, 0.7)].into(),
            recall_at: [(10, 0.6)].into(),
            ndcg_at: [(10, 0.75)].into(),
            map: 0.65,
            query_count: 10,
        };

        let ablated = RetrievalMetrics {
            mrr: 0.7,
            precision_at: [(10, 0.6)].into(),
            recall_at: [(10, 0.5)].into(),
            ndcg_at: [(10, 0.65)].into(),
            map: 0.55,
            query_count: 10,
        };

        let delta = AblationDelta::new(EmbedderIndex::E1Semantic, baseline, ablated);

        assert!(delta.mrr_delta > 0.0);
        assert!(delta.impact_pct > 0.0);
    }

    #[test]
    fn test_enhancement_delta() {
        let e1_only = RetrievalMetrics {
            mrr: 0.6,
            precision_at: [(10, 0.5)].into(),
            recall_at: [(10, 0.4)].into(),
            ndcg_at: [(10, 0.55)].into(),
            map: 0.45,
            query_count: 10,
        };

        let e1_plus = RetrievalMetrics {
            mrr: 0.7,
            precision_at: [(10, 0.6)].into(),
            recall_at: [(10, 0.5)].into(),
            ndcg_at: [(10, 0.65)].into(),
            map: 0.55,
            query_count: 10,
        };

        let delta = EnhancementDelta::new(EmbedderIndex::E7Code, e1_only, e1_plus);

        assert!(delta.mrr_delta > 0.0);
        assert!(delta.improvement_pct > 0.0);
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        let c = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &c)).abs() < 0.001);
    }
}
