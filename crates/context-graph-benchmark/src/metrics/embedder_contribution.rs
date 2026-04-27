//! Embedder contribution metrics for measuring individual embedder impact.
//!
//! This module provides metrics for:
//! - Per-result contribution breakdown (which embedders contributed to each result)
//! - Contribution attribution (overall % contribution per embedder)
//! - Blind spot analysis (what E1 misses that enhancers find)
//! - Agreement pattern analysis (how embedders agree/disagree on relevance)

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use context_graph_storage::teleological::indexes::EmbedderIndex;

/// Per-result contribution breakdown showing which embedders contributed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultContribution {
    /// Result document ID.
    pub result_id: Uuid,
    /// Final rank in fused results (0-indexed).
    pub final_rank: usize,
    /// Final fused score.
    pub fused_score: f32,
    /// Per-embedder contribution percentage (0-100%).
    /// Index matches EmbedderIndex::to_index().
    pub embedder_contributions: [f32; 14],
    /// List of embedders that contributed (had non-zero ranking).
    pub contributing_embedders: Vec<EmbedderIndex>,
    /// Per-embedder rank if this result appeared in that embedder's results.
    pub embedder_ranks: HashMap<EmbedderIndex, usize>,
    /// Is this result relevant according to ground truth?
    pub is_relevant: bool,
}

impl ResultContribution {
    /// Create a new result contribution.
    pub fn new(result_id: Uuid, final_rank: usize, fused_score: f32) -> Self {
        Self {
            result_id,
            final_rank,
            fused_score,
            embedder_contributions: [0.0; 14],
            contributing_embedders: Vec::new(),
            embedder_ranks: HashMap::new(),
            is_relevant: false,
        }
    }

    /// Add contribution from an embedder.
    pub fn add_contribution(
        &mut self,
        embedder: EmbedderIndex,
        rank: usize,
        contribution_pct: f32,
    ) {
        if let Some(idx) = embedder.to_index() {
            self.embedder_contributions[idx] = contribution_pct;
            if !self.contributing_embedders.contains(&embedder) {
                self.contributing_embedders.push(embedder);
            }
            self.embedder_ranks.insert(embedder, rank);
        }
    }

    /// Get contribution from a specific embedder.
    pub fn contribution_from(&self, embedder: EmbedderIndex) -> f32 {
        embedder
            .to_index()
            .map(|idx| self.embedder_contributions[idx])
            .unwrap_or(0.0)
    }

    /// Number of embedders that contributed to this result.
    pub fn contributing_count(&self) -> usize {
        self.contributing_embedders.len()
    }
}

/// RRF rank contribution statistics showing how ranks translate to scores.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RRFRankContribution {
    /// Average rank per embedder when contributing to relevant results.
    pub avg_rank_relevant: HashMap<EmbedderIndex, f32>,
    /// Average rank per embedder when contributing to irrelevant results.
    pub avg_rank_irrelevant: HashMap<EmbedderIndex, f32>,
    /// How often each embedder had rank 1 for relevant results.
    pub rank1_relevant_count: HashMap<EmbedderIndex, usize>,
    /// Total contributions per embedder across all results.
    pub total_contributions: HashMap<EmbedderIndex, usize>,
}

impl RRFRankContribution {
    /// Create empty RRF rank contribution stats.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a contribution.
    pub fn record(&mut self, embedder: EmbedderIndex, rank: usize, is_relevant: bool) {
        *self.total_contributions.entry(embedder).or_default() += 1;

        if is_relevant {
            let entry = self.avg_rank_relevant.entry(embedder).or_insert(0.0);
            *entry += rank as f32;
            if rank == 0 {
                *self.rank1_relevant_count.entry(embedder).or_default() += 1;
            }
        } else {
            let entry = self.avg_rank_irrelevant.entry(embedder).or_insert(0.0);
            *entry += rank as f32;
        }
    }

    /// Finalize averages (call after all contributions recorded).
    pub fn finalize(&mut self, relevant_count: usize, irrelevant_count: usize) {
        if relevant_count > 0 {
            for val in self.avg_rank_relevant.values_mut() {
                *val /= relevant_count as f32;
            }
        }
        if irrelevant_count > 0 {
            for val in self.avg_rank_irrelevant.values_mut() {
                *val /= irrelevant_count as f32;
            }
        }
    }
}

/// Agreement pattern analysis across embedders.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgreementPatternAnalysis {
    /// Pairwise agreement counts: how often embedders i and j both rank a doc in top-K.
    /// Key: (min(i,j), max(i,j)) to avoid duplicates.
    pub pairwise_agreement: HashMap<(usize, usize), usize>,
    /// Average agreement (fraction of embedder pairs that agree).
    pub avg_agreement: f32,
    /// Disagreement hotspots: embedder pairs that frequently disagree.
    pub disagreement_hotspots: Vec<(EmbedderIndex, EmbedderIndex, f32)>,
    /// Number of results analyzed.
    pub total_results: usize,
}

impl AgreementPatternAnalysis {
    /// Create empty agreement analysis.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record agreement for a set of contributing embedders.
    pub fn record_result(&mut self, contributors: &[EmbedderIndex]) {
        self.total_results += 1;

        // Record pairwise agreements
        for (i, &e1) in contributors.iter().enumerate() {
            for &e2 in contributors.iter().skip(i + 1) {
                let idx1 = e1.to_index().unwrap_or(0);
                let idx2 = e2.to_index().unwrap_or(0);
                let key = (idx1.min(idx2), idx1.max(idx2));
                *self.pairwise_agreement.entry(key).or_default() += 1;
            }
        }
    }

    /// Finalize analysis and compute metrics.
    pub fn finalize(&mut self) {
        if self.total_results == 0 {
            return;
        }

        // Compute average agreement
        let total_pairs = self.pairwise_agreement.len();
        if total_pairs > 0 {
            let sum: usize = self.pairwise_agreement.values().sum();
            self.avg_agreement = sum as f32 / (total_pairs as f32 * self.total_results as f32);
        }

        // Find disagreement hotspots (pairs with low agreement relative to max)
        let max_agreement = self.pairwise_agreement.values().max().copied().unwrap_or(1) as f32;
        if max_agreement > 0.0 {
            for (&(i, j), &count) in &self.pairwise_agreement {
                let agreement_ratio = count as f32 / max_agreement;
                if agreement_ratio < 0.3 && i < 13 && j < 13 {
                    // Low agreement
                    let emb_i = EmbedderIndex::from_index(i);
                    let emb_j = EmbedderIndex::from_index(j);
                    self.disagreement_hotspots
                        .push((emb_i, emb_j, agreement_ratio));
                }
            }
        }

        // Sort hotspots by disagreement (lowest agreement first)
        self.disagreement_hotspots
            .sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
    }
}

/// Contribution attribution showing overall embedder contributions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContributionAttribution {
    /// Per-embedder contribution percentage to total fused score.
    pub embedder_contributions: HashMap<EmbedderIndex, f64>,
    /// Per-result breakdown.
    pub result_breakdown: Vec<ResultContribution>,
    /// RRF rank contribution statistics.
    pub rrf_rank_stats: RRFRankContribution,
    /// Agreement pattern analysis.
    pub agreement_patterns: AgreementPatternAnalysis,
    /// Total fused score across all results.
    pub total_fused_score: f64,
    /// Number of results analyzed.
    pub result_count: usize,
    /// Number of relevant results.
    pub relevant_count: usize,
}

impl ContributionAttribution {
    /// Create empty contribution attribution.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a result's contributions.
    pub fn add_result(&mut self, result: ResultContribution) {
        self.total_fused_score += result.fused_score as f64;
        self.result_count += 1;

        if result.is_relevant {
            self.relevant_count += 1;
        }

        // Accumulate per-embedder contributions
        for (idx, &contrib) in result.embedder_contributions.iter().enumerate() {
            if contrib > 0.0 && idx < 13 {
                let embedder = EmbedderIndex::from_index(idx);
                *self.embedder_contributions.entry(embedder).or_default() += contrib as f64;
            }
        }

        // Record agreement
        self.agreement_patterns
            .record_result(&result.contributing_embedders);

        // Record RRF stats
        for embedder in &result.contributing_embedders {
            if let Some(&rank) = result.embedder_ranks.get(embedder) {
                self.rrf_rank_stats
                    .record(*embedder, rank, result.is_relevant);
            }
        }

        self.result_breakdown.push(result);
    }

    /// Finalize attribution calculations.
    pub fn finalize(&mut self) {
        // Normalize embedder contributions to percentages
        let total: f64 = self.embedder_contributions.values().sum();
        if total > 0.0 {
            for contrib in self.embedder_contributions.values_mut() {
                *contrib = (*contrib / total) * 100.0;
            }
        }

        self.rrf_rank_stats.finalize(
            self.relevant_count,
            self.result_count.saturating_sub(self.relevant_count),
        );
        self.agreement_patterns.finalize();
    }

    /// Get sorted contributions (highest first).
    pub fn sorted_contributions(&self) -> Vec<(EmbedderIndex, f64)> {
        let mut sorted: Vec<_> = self
            .embedder_contributions
            .iter()
            .map(|(&e, &c)| (e, c))
            .collect();
        sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        sorted
    }
}

/// Example of a blind spot (document E1 missed but an enhancer found).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlindSpotExample {
    /// Document ID.
    pub doc_id: Uuid,
    /// Embedder that found this document.
    pub found_by: EmbedderIndex,
    /// Rank in the finding embedder's results.
    pub rank_in_finder: usize,
    /// Score in the finding embedder.
    pub score_in_finder: f32,
    /// Relevance score (if known from ground truth).
    pub relevance_score: f32,
    /// Rank in E1 (if present, otherwise None = not in E1 top-K).
    pub e1_rank: Option<usize>,
    /// Score in E1 (if present).
    pub e1_score: Option<f32>,
}

/// Unique finds by a specific embedder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniqueFinds {
    /// Which embedder found these.
    pub embedder: EmbedderIndex,
    /// Number of documents this embedder uniquely found.
    pub count: usize,
    /// Average relevance score of uniquely found documents.
    pub avg_relevance: f32,
    /// Percentage of total relevant documents this represents.
    pub pct_of_total: f64,
    /// Example documents.
    pub examples: Vec<BlindSpotExample>,
}

impl UniqueFinds {
    /// Create new unique finds for an embedder.
    pub fn new(embedder: EmbedderIndex) -> Self {
        Self {
            embedder,
            count: 0,
            avg_relevance: 0.0,
            pct_of_total: 0.0,
            examples: Vec::new(),
        }
    }

    /// Add a unique find.
    pub fn add(&mut self, example: BlindSpotExample) {
        self.count += 1;
        self.avg_relevance += example.relevance_score;
        if self.examples.len() < 10 {
            self.examples.push(example);
        }
    }

    /// Finalize calculations.
    pub fn finalize(&mut self, total_relevant: usize) {
        if self.count > 0 {
            self.avg_relevance /= self.count as f32;
        }
        if total_relevant > 0 {
            self.pct_of_total = (self.count as f64 / total_relevant as f64) * 100.0;
        }
    }
}

/// Blind spot analysis - what E1 misses that enhancers find.
///
/// This measures the unique value each enhancer provides beyond E1.
/// Per ARCH-12: E1 is the foundation, enhancers complement it.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BlindSpotAnalysis {
    /// Documents E1 uniquely found (in E1 but not in any enhancer).
    pub e1_unique_finds: usize,
    /// Per-enhancer unique finds (found by enhancer but not E1).
    pub enhancer_unique_finds: HashMap<EmbedderIndex, UniqueFinds>,
    /// Overlap matrix: [i][j] = count of docs found by both embedder i and j.
    pub overlap_matrix: [[usize; 14]; 14],
    /// Example blind spots (first 20).
    pub blind_spot_examples: Vec<BlindSpotExample>,
    /// Total relevant documents in ground truth.
    pub total_relevant: usize,
    /// Documents found by E1 in top-K.
    pub e1_found_count: usize,
    /// Documents found by any embedder in top-K.
    pub any_found_count: usize,
    /// K value used for analysis.
    pub k: usize,
}

impl BlindSpotAnalysis {
    /// Create new blind spot analysis.
    pub fn new(k: usize) -> Self {
        Self {
            k,
            ..Default::default()
        }
    }

    /// Compute blind spot analysis from per-embedder results.
    ///
    /// # Arguments
    /// * `per_embedder_results` - Results from each embedder: (doc_id, score) sorted by score desc
    /// * `relevant_docs` - Set of relevant document IDs (ground truth)
    pub fn compute(
        per_embedder_results: &HashMap<EmbedderIndex, Vec<(Uuid, f32)>>,
        relevant_docs: &HashSet<Uuid>,
        k: usize,
    ) -> Self {
        let mut analysis = Self::new(k);
        analysis.total_relevant = relevant_docs.len();

        // Get E1 results
        let e1_results: HashSet<Uuid> = per_embedder_results
            .get(&EmbedderIndex::E1Semantic)
            .map(|r| r.iter().take(k).map(|(id, _)| *id).collect())
            .unwrap_or_default();

        let e1_result_scores: HashMap<Uuid, (usize, f32)> = per_embedder_results
            .get(&EmbedderIndex::E1Semantic)
            .map(|r| {
                r.iter()
                    .take(k)
                    .enumerate()
                    .map(|(rank, (id, score))| (*id, (rank, *score)))
                    .collect()
            })
            .unwrap_or_default();

        // Track what each embedder found
        let mut all_found: HashSet<Uuid> = HashSet::new();
        let mut per_embedder_found: HashMap<EmbedderIndex, HashSet<Uuid>> = HashMap::new();

        for (embedder, results) in per_embedder_results {
            let found: HashSet<Uuid> = results.iter().take(k).map(|(id, _)| *id).collect();
            all_found.extend(found.iter().copied());
            per_embedder_found.insert(*embedder, found);
        }

        // Count E1 found
        analysis.e1_found_count = e1_results.len();
        analysis.any_found_count = all_found.len();

        // Find E1 unique (in E1 but not in any enhancer)
        for doc in &e1_results {
            let mut found_by_enhancer = false;
            for (embedder, found) in &per_embedder_found {
                if *embedder != EmbedderIndex::E1Semantic && found.contains(doc) {
                    found_by_enhancer = true;
                    break;
                }
            }
            if !found_by_enhancer {
                analysis.e1_unique_finds += 1;
            }
        }

        // Find enhancer unique finds (found by enhancer but not by E1)
        for embedder in per_embedder_found.keys() {
            if *embedder == EmbedderIndex::E1Semantic {
                continue;
            }

            let mut unique = UniqueFinds::new(*embedder);

            for (rank, (doc_id, score)) in per_embedder_results
                .get(embedder)
                .unwrap()
                .iter()
                .take(k)
                .enumerate()
            {
                if !e1_results.contains(doc_id) {
                    let relevance = if relevant_docs.contains(doc_id) {
                        1.0
                    } else {
                        0.0
                    };
                    let example = BlindSpotExample {
                        doc_id: *doc_id,
                        found_by: *embedder,
                        rank_in_finder: rank,
                        score_in_finder: *score,
                        relevance_score: relevance,
                        e1_rank: e1_result_scores.get(doc_id).map(|(r, _)| *r),
                        e1_score: e1_result_scores.get(doc_id).map(|(_, s)| *s),
                    };

                    // Only count relevant documents as true blind spots
                    if relevant_docs.contains(doc_id) {
                        unique.add(example.clone());
                        if analysis.blind_spot_examples.len() < 20 {
                            analysis.blind_spot_examples.push(example);
                        }
                    }
                }
            }

            unique.finalize(analysis.total_relevant);
            analysis.enhancer_unique_finds.insert(*embedder, unique);
        }

        // Build overlap matrix
        for (i, (emb_i, found_i)) in per_embedder_found.iter().enumerate() {
            for (j, (emb_j, found_j)) in per_embedder_found.iter().enumerate() {
                if i <= j {
                    let idx_i = emb_i.to_index().unwrap_or(0);
                    let idx_j = emb_j.to_index().unwrap_or(0);
                    let overlap = found_i.intersection(found_j).count();
                    analysis.overlap_matrix[idx_i][idx_j] = overlap;
                    analysis.overlap_matrix[idx_j][idx_i] = overlap;
                }
            }
        }

        analysis
    }

    /// Get enhancers sorted by unique find count.
    pub fn sorted_by_unique_finds(&self) -> Vec<&UniqueFinds> {
        let mut sorted: Vec<_> = self.enhancer_unique_finds.values().collect();
        sorted.sort_by(|a, b| b.count.cmp(&a.count));
        sorted
    }

    /// Total relevant documents found only by enhancers (not by E1).
    pub fn total_enhancer_unique_relevant(&self) -> usize {
        self.enhancer_unique_finds.values().map(|u| u.count).sum()
    }

    /// Percentage of relevant docs that E1 would miss without enhancers.
    pub fn e1_blind_spot_percentage(&self) -> f64 {
        if self.total_relevant == 0 {
            return 0.0;
        }
        let unique_relevant = self.total_enhancer_unique_relevant();
        (unique_relevant as f64 / self.total_relevant as f64) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_uuid(n: u8) -> Uuid {
        Uuid::from_bytes([n; 16])
    }

    #[test]
    fn test_result_contribution() {
        let mut result = ResultContribution::new(make_uuid(1), 0, 0.95);
        result.add_contribution(EmbedderIndex::E1Semantic, 0, 50.0);
        result.add_contribution(EmbedderIndex::E7Code, 2, 30.0);
        result.is_relevant = true;

        assert_eq!(result.contribution_from(EmbedderIndex::E1Semantic), 50.0);
        assert_eq!(result.contribution_from(EmbedderIndex::E7Code), 30.0);
        assert_eq!(result.contribution_from(EmbedderIndex::E5Causal), 0.0);
        assert_eq!(result.contributing_count(), 2);
    }

    #[test]
    fn test_contribution_attribution() {
        let mut attribution = ContributionAttribution::new();

        let mut r1 = ResultContribution::new(make_uuid(1), 0, 0.9);
        r1.add_contribution(EmbedderIndex::E1Semantic, 0, 60.0);
        r1.add_contribution(EmbedderIndex::E7Code, 1, 40.0);
        r1.is_relevant = true;
        attribution.add_result(r1);

        let mut r2 = ResultContribution::new(make_uuid(2), 1, 0.8);
        r2.add_contribution(EmbedderIndex::E1Semantic, 1, 70.0);
        r2.add_contribution(EmbedderIndex::E5Causal, 0, 30.0);
        r2.is_relevant = false;
        attribution.add_result(r2);

        attribution.finalize();

        assert_eq!(attribution.result_count, 2);
        assert_eq!(attribution.relevant_count, 1);

        // Check that contributions are normalized to percentages
        let total: f64 = attribution.embedder_contributions.values().sum();
        assert!((total - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_blind_spot_analysis() {
        let mut per_embedder = HashMap::new();

        // E1 finds docs 1, 2, 3
        per_embedder.insert(
            EmbedderIndex::E1Semantic,
            vec![
                (make_uuid(1), 0.9),
                (make_uuid(2), 0.8),
                (make_uuid(3), 0.7),
            ],
        );

        // E7 finds docs 2, 4, 5 (4 and 5 are blind spots)
        per_embedder.insert(
            EmbedderIndex::E7Code,
            vec![
                (make_uuid(2), 0.95),
                (make_uuid(4), 0.85), // Blind spot
                (make_uuid(5), 0.75), // Blind spot
            ],
        );

        // Relevant docs: 1, 2, 4 (so 4 is a relevant blind spot)
        let relevant: HashSet<Uuid> = [make_uuid(1), make_uuid(2), make_uuid(4)].into();

        let analysis = BlindSpotAnalysis::compute(&per_embedder, &relevant, 10);

        assert_eq!(analysis.total_relevant, 3);
        assert_eq!(analysis.e1_found_count, 3);

        // E7 should have 1 relevant unique find (doc 4)
        let e7_unique = analysis
            .enhancer_unique_finds
            .get(&EmbedderIndex::E7Code)
            .unwrap();
        assert_eq!(e7_unique.count, 1);
    }

    #[test]
    fn test_agreement_patterns() {
        let mut patterns = AgreementPatternAnalysis::new();

        // Record results where E1 and E7 both contribute
        patterns.record_result(&[EmbedderIndex::E1Semantic, EmbedderIndex::E7Code]);
        patterns.record_result(&[EmbedderIndex::E1Semantic, EmbedderIndex::E7Code]);
        patterns.record_result(&[EmbedderIndex::E1Semantic, EmbedderIndex::E5Causal]);

        patterns.finalize();

        assert_eq!(patterns.total_results, 3);
        assert!(!patterns.pairwise_agreement.is_empty());
    }
}
