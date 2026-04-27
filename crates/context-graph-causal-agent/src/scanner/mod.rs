//! Memory Scanner for finding candidate causal pairs.
//!
//! Scans existing memories to identify pairs that may have causal relationships.
//! Uses E1 semantic clustering and temporal ordering to find candidates.
//!
//! # Strategy
//!
//! 1. Cluster memories by E1 semantic similarity (related content)
//! 2. Within clusters, find pairs with temporal ordering (cause before effect)
//! 3. Score pairs by causal marker presence
//! 4. Return top candidates for LLM analysis

use std::collections::{HashMap, HashSet};

use chrono::Duration;
use tracing::{debug, info};
use uuid::Uuid;

use crate::error::{CausalAgentError, CausalAgentResult};
use crate::types::{CausalCandidate, CausalMarkers, MemoryForAnalysis};

/// Configuration for the memory scanner.
#[derive(Debug, Clone)]
pub struct ScannerConfig {
    /// Minimum E1 similarity for memories to be considered related.
    pub similarity_threshold: f32,

    /// Maximum E1 similarity (to avoid duplicates).
    pub max_similarity: f32,

    /// Maximum candidates to return per scan.
    pub max_candidates: usize,

    /// Minimum time gap between memories (to avoid same-event pairs).
    pub min_time_gap: Duration,

    /// Maximum time gap between memories (causal relationships are usually proximate).
    pub max_time_gap: Duration,

    /// Whether to only scan memories within the same session.
    pub same_session_only: bool,

    /// Minimum initial score to include a candidate.
    pub min_initial_score: f32,
}

impl Default for ScannerConfig {
    fn default() -> Self {
        Self {
            similarity_threshold: 0.5, // Related but not too similar
            max_similarity: 0.9,       // Avoid duplicates
            max_candidates: 100,
            min_time_gap: Duration::seconds(10),
            max_time_gap: Duration::hours(24),
            same_session_only: false,
            min_initial_score: 0.3,
        }
    }
}

/// Scanner for finding candidate memory pairs for causal analysis.
pub struct MemoryScanner {
    /// Configuration.
    config: ScannerConfig,

    /// Set of already-analyzed pairs (to avoid re-analyzing).
    analyzed_pairs: HashSet<(Uuid, Uuid)>,
}

impl MemoryScanner {
    /// Create a new scanner with default configuration.
    pub fn new() -> Self {
        Self::with_config(ScannerConfig::default())
    }

    /// Create with custom configuration.
    pub fn with_config(config: ScannerConfig) -> Self {
        Self {
            config,
            analyzed_pairs: HashSet::new(),
        }
    }

    /// Find candidate memory pairs for causal analysis.
    ///
    /// # Arguments
    ///
    /// * `memories` - All memories to scan
    ///
    /// # Returns
    ///
    /// Sorted list of candidate pairs (highest score first)
    pub fn find_candidates(
        &mut self,
        memories: &[MemoryForAnalysis],
    ) -> CausalAgentResult<Vec<CausalCandidate>> {
        if memories.is_empty() {
            return Err(CausalAgentError::NoCandidatesFound);
        }

        info!(
            memory_count = memories.len(),
            "Scanning memories for causal candidates"
        );

        // Build index of memories by ID for quick lookup
        let memory_index: HashMap<Uuid, &MemoryForAnalysis> =
            memories.iter().map(|m| (m.id, m)).collect();

        // Cluster memories by E1 similarity
        let clusters = self.cluster_by_similarity(memories);

        debug!(cluster_count = clusters.len(), "Created semantic clusters");

        // Find candidate pairs within clusters
        let mut candidates = Vec::new();

        for cluster in &clusters {
            let cluster_candidates = self.find_candidates_in_cluster(cluster, &memory_index);
            candidates.extend(cluster_candidates);
        }

        // Sort by score (descending) and truncate
        candidates.sort_by(|a, b| {
            b.initial_score
                .partial_cmp(&a.initial_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        candidates.truncate(self.config.max_candidates);

        info!(
            candidate_count = candidates.len(),
            "Found causal candidates"
        );

        if candidates.is_empty() {
            return Err(CausalAgentError::NoCandidatesFound);
        }

        Ok(candidates)
    }

    /// Cluster memories by E1 semantic similarity.
    ///
    /// Uses a simple greedy clustering approach:
    /// - Start with first memory as cluster seed
    /// - Add memories with similarity > threshold to cluster
    /// - Repeat for remaining unclustered memories
    fn cluster_by_similarity<'a>(
        &self,
        memories: &'a [MemoryForAnalysis],
    ) -> Vec<Vec<&'a MemoryForAnalysis>> {
        let mut clusters: Vec<Vec<&MemoryForAnalysis>> = Vec::new();
        let mut unclustered: Vec<&MemoryForAnalysis> = memories.iter().collect();

        while !unclustered.is_empty() {
            // Start new cluster with first unclustered memory
            let seed = unclustered.remove(0);
            let mut cluster = vec![seed];

            // Find all memories similar to seed
            let mut i = 0;
            while i < unclustered.len() {
                let candidate = unclustered[i];
                let sim = cosine_similarity(&seed.e1_embedding, &candidate.e1_embedding);

                if sim >= self.config.similarity_threshold && sim <= self.config.max_similarity {
                    cluster.push(unclustered.remove(i));
                } else {
                    i += 1;
                }
            }

            if cluster.len() > 1 {
                clusters.push(cluster);
            }
        }

        clusters
    }

    /// Find candidate pairs within a cluster.
    fn find_candidates_in_cluster(
        &mut self,
        cluster: &[&MemoryForAnalysis],
        _memory_index: &HashMap<Uuid, &MemoryForAnalysis>,
    ) -> Vec<CausalCandidate> {
        let mut candidates = Vec::new();

        // Compare all pairs within cluster
        for i in 0..cluster.len() {
            for j in (i + 1)..cluster.len() {
                let mem_a = cluster[i];
                let mem_b = cluster[j];

                // Skip if already analyzed
                let pair_key = if mem_a.id < mem_b.id {
                    (mem_a.id, mem_b.id)
                } else {
                    (mem_b.id, mem_a.id)
                };

                if self.analyzed_pairs.contains(&pair_key) {
                    continue;
                }

                // Check session constraint
                if self.config.same_session_only && mem_a.session_id != mem_b.session_id {
                    continue;
                }

                // Determine temporal order (earlier = potential cause)
                let (earlier, later) = if mem_a.created_at <= mem_b.created_at {
                    (mem_a, mem_b)
                } else {
                    (mem_b, mem_a)
                };

                // Check time gap constraints
                let time_gap = later.created_at - earlier.created_at;
                if time_gap < self.config.min_time_gap || time_gap > self.config.max_time_gap {
                    continue;
                }

                // Score the pair
                let score = self.score_candidate_pair(earlier, later);

                if score >= self.config.min_initial_score {
                    candidates.push(CausalCandidate::new(
                        earlier.id,
                        earlier.content.clone(),
                        later.id,
                        later.content.clone(),
                        score,
                        earlier.created_at,
                        later.created_at,
                    ));

                    // AGT-01 FIX: Do NOT mark as analyzed here. This runs BEFORE
                    // LLM confirmation. If the LLM fails (timeout, error, unavailable),
                    // the pair would be permanently skipped with no retry.
                    // Callers must use mark_analyzed() AFTER successful LLM analysis.
                }
            }
        }

        candidates
    }

    /// Score a candidate pair based on heuristics.
    fn score_candidate_pair(&self, earlier: &MemoryForAnalysis, later: &MemoryForAnalysis) -> f32 {
        let mut score = 0.0;

        // 1. Causal markers in text (+0.3 each)
        let earlier_markers = CausalMarkers::count_markers(&earlier.content);
        let later_markers = CausalMarkers::count_markers(&later.content);
        score += (earlier_markers + later_markers) as f32 * 0.15;

        // 2. Temporal ordering bonus (+0.1)
        // Earlier coming before later is expected for causation
        score += 0.1;

        // 3. Same session bonus (+0.1)
        if earlier.session_id.is_some() && earlier.session_id == later.session_id {
            score += 0.1;
        }

        // 4. Content length bonus (longer = more context)
        let min_len = earlier.content.len().min(later.content.len());
        if min_len > 100 {
            score += 0.05;
        }
        if min_len > 300 {
            score += 0.05;
        }

        // 5. Semantic similarity bonus (not too similar, not too different)
        let sim = cosine_similarity(&earlier.e1_embedding, &later.e1_embedding);
        if (0.5..=0.8).contains(&sim) {
            score += 0.1;
        }

        // Clamp to [0, 1]
        score.clamp(0.0, 1.0)
    }

    /// Mark a pair as analyzed (to skip in future scans).
    pub fn mark_analyzed(&mut self, mem_a: Uuid, mem_b: Uuid) {
        let pair_key = if mem_a < mem_b {
            (mem_a, mem_b)
        } else {
            (mem_b, mem_a)
        };
        self.analyzed_pairs.insert(pair_key);
    }

    /// Clear the analyzed pairs set.
    pub fn clear_analyzed(&mut self) {
        self.analyzed_pairs.clear();
    }

    /// Get the number of analyzed pairs.
    pub fn analyzed_count(&self) -> usize {
        self.analyzed_pairs.len()
    }

    /// Get the configuration.
    pub fn config(&self) -> &ScannerConfig {
        &self.config
    }
}

impl Default for MemoryScanner {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute cosine similarity between two vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a < f32::EPSILON || norm_b < f32::EPSILON {
        return 0.0;
    }

    (dot / (norm_a * norm_b)).clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_test_memory(
        id: u128,
        content: &str,
        hours_ago: i64,
        embedding: Vec<f32>,
    ) -> MemoryForAnalysis {
        MemoryForAnalysis {
            id: Uuid::from_u128(id),
            content: content.to_string(),
            created_at: Utc::now() - Duration::hours(hours_ago),
            session_id: Some("test-session".to_string()),
            e1_embedding: embedding,
        }
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        let c = vec![0.0, 1.0, 0.0];
        assert!(cosine_similarity(&a, &c).abs() < 0.001);

        let d = vec![-1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &d) - (-1.0)).abs() < 0.001);
    }

    #[test]
    fn test_find_candidates() {
        // Use a lower threshold for testing
        let config = ScannerConfig {
            similarity_threshold: 0.5,
            max_similarity: 0.95,
            min_time_gap: Duration::seconds(1),
            ..Default::default()
        };
        let mut scanner = MemoryScanner::with_config(config);

        // Create similar (but not identical) memories with temporal ordering
        // These vectors have cosine similarity ≈ 0.7 (between 0.5 and 0.95)
        let embedding1 = vec![1.0, 0.0, 0.0]; // norm = 1.0
        let embedding2 = vec![0.7, 0.7, 0.1]; // Different but related

        // Verify similarity is in range
        let sim = cosine_similarity(&embedding1, &embedding2);
        assert!(
            sim > 0.5 && sim < 0.95,
            "Similarity {} not in range (0.5, 0.95)",
            sim
        );

        let memories = vec![
            create_test_memory(1, "Because of the bug, users complained", 2, embedding1),
            create_test_memory(2, "Therefore we fixed the issue", 1, embedding2),
        ];

        let candidates = scanner.find_candidates(&memories).unwrap();
        assert!(!candidates.is_empty());
        assert!(candidates[0].initial_score > 0.3);
    }

    #[test]
    fn test_score_candidate_pair() {
        let scanner = MemoryScanner::new();

        let embedding = vec![0.7, 0.7, 0.1];
        let earlier =
            create_test_memory(1, "Because of X, something happened", 2, embedding.clone());
        let later = create_test_memory(2, "Therefore Y occurred as a result", 1, embedding.clone());

        let score = scanner.score_candidate_pair(&earlier, &later);

        // Should have high score due to causal markers
        assert!(score > 0.4);
    }

    #[test]
    fn test_mark_analyzed() {
        let mut scanner = MemoryScanner::new();

        let id1 = Uuid::from_u128(1);
        let id2 = Uuid::from_u128(2);

        assert_eq!(scanner.analyzed_count(), 0);

        scanner.mark_analyzed(id1, id2);
        assert_eq!(scanner.analyzed_count(), 1);

        // Same pair, different order - should not add
        scanner.mark_analyzed(id2, id1);
        assert_eq!(scanner.analyzed_count(), 1);
    }
}
