//! Memory scanner for finding graph relationship candidates.
//!
//! The scanner identifies pairs of memories that may have structural
//! relationships based on heuristics like:
//! - Domain-specific markers (code, legal, academic, general)
//! - Reference markers (see:, ref:, links to, URLs, citations)
//! - Structural proximity (same session, similar file paths, shared entities)
//!
//! # Multi-Domain Support
//!
//! The scanner uses `DomainMarkers` to detect content domains and
//! adjust scoring and relationship detection accordingly:
//! - Code: imports, calls, implements, extends, contains
//! - Legal: cites, interprets, overrules, supersedes, distinguishes
//! - Academic: cites, applies, extends, references
//! - General: all relationship types

use std::collections::HashSet;

use uuid::Uuid;

use crate::error::GraphAgentResult;
use crate::types::{
    ContentDomain, DomainMarkers, GraphCandidate, GraphMarkers, MemoryForGraphAnalysis,
    RelationshipType,
};

/// Configuration for the memory scanner.
#[derive(Debug, Clone)]
pub struct ScannerConfig {
    /// Minimum E1 similarity for candidate clustering.
    pub similarity_threshold: f32,

    /// Maximum E1 similarity (avoid near-duplicates).
    pub max_similarity: f32,

    /// Maximum candidates to return per scan.
    pub max_candidates: usize,

    /// Minimum initial score to consider a pair.
    pub min_initial_score: f32,

    /// Whether to only consider memories from the same session.
    pub same_session_only: bool,

    /// Whether to prioritize code-related memories.
    pub prioritize_code: bool,
}

impl Default for ScannerConfig {
    fn default() -> Self {
        Self {
            similarity_threshold: 0.3,
            max_similarity: 0.95,
            max_candidates: 100,
            min_initial_score: 0.2,
            same_session_only: false,
            prioritize_code: true,
        }
    }
}

/// Scanner for finding graph relationship candidates.
///
/// Uses heuristics to identify memory pairs that may have structural
/// relationships worth analyzing with the LLM.
pub struct MemoryScanner {
    config: ScannerConfig,
    /// Pairs that have already been analyzed (to avoid re-analysis).
    analyzed_pairs: HashSet<(Uuid, Uuid)>,
}

impl MemoryScanner {
    /// Create a new scanner with default configuration.
    pub fn new() -> Self {
        Self::with_config(ScannerConfig::default())
    }

    /// Create a scanner with custom configuration.
    pub fn with_config(config: ScannerConfig) -> Self {
        Self {
            config,
            analyzed_pairs: HashSet::new(),
        }
    }

    /// Find candidate pairs for graph relationship analysis.
    ///
    /// # Arguments
    /// * `memories` - List of memories to scan
    ///
    /// # Returns
    /// Vector of candidate pairs sorted by initial score (descending)
    pub fn find_candidates(
        &mut self,
        memories: &[MemoryForGraphAnalysis],
    ) -> GraphAgentResult<Vec<GraphCandidate>> {
        if memories.len() < 2 {
            return Ok(Vec::new());
        }

        let mut candidates = Vec::new();

        // Group memories by potential relationship clusters
        let clusters = self.cluster_by_similarity(memories);

        // Find candidate pairs within each cluster
        for cluster in clusters {
            for i in 0..cluster.len() {
                for j in (i + 1)..cluster.len() {
                    let mem_a = &cluster[i];
                    let mem_b = &cluster[j];

                    // Skip already analyzed pairs
                    let pair_key = self.make_pair_key(mem_a.id, mem_b.id);
                    if self.analyzed_pairs.contains(&pair_key) {
                        continue;
                    }

                    // Check session constraint
                    if self.config.same_session_only
                        && mem_a.session_id != mem_b.session_id
                    {
                        continue;
                    }

                    // Score the candidate pair
                    let score = self.score_candidate_pair(mem_a, mem_b);

                    if score >= self.config.min_initial_score {
                        let suspected_types = self.detect_relationship_types(mem_a, mem_b);

                        candidates.push(GraphCandidate {
                            memory_a_id: mem_a.id,
                            memory_a_content: mem_a.content.clone(),
                            memory_b_id: mem_b.id,
                            memory_b_content: mem_b.content.clone(),
                            initial_score: score,
                            memory_a_timestamp: mem_a.created_at,
                            memory_b_timestamp: mem_b.created_at,
                            suspected_types,
                        });
                    }
                }
            }
        }

        // Sort by score descending
        candidates.sort_by(|a, b| {
            b.initial_score
                .partial_cmp(&a.initial_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Truncate to max candidates
        candidates.truncate(self.config.max_candidates);

        Ok(candidates)
    }

    /// Mark a pair as analyzed to avoid re-analysis.
    pub fn mark_analyzed(&mut self, memory_a_id: Uuid, memory_b_id: Uuid) {
        let pair_key = self.make_pair_key(memory_a_id, memory_b_id);
        self.analyzed_pairs.insert(pair_key);
    }

    /// Clear the analyzed pairs set.
    pub fn clear_analyzed(&mut self) {
        self.analyzed_pairs.clear();
    }

    /// Get number of analyzed pairs.
    pub fn analyzed_count(&self) -> usize {
        self.analyzed_pairs.len()
    }

    /// Cluster memories by E1 semantic similarity.
    fn cluster_by_similarity<'a>(
        &self,
        memories: &'a [MemoryForGraphAnalysis],
    ) -> Vec<Vec<&'a MemoryForGraphAnalysis>> {
        let mut clusters: Vec<Vec<&MemoryForGraphAnalysis>> = Vec::new();
        let mut assigned: HashSet<usize> = HashSet::new();

        for (i, mem) in memories.iter().enumerate() {
            if assigned.contains(&i) {
                continue;
            }

            // Start a new cluster with this memory
            let mut cluster = vec![mem];
            assigned.insert(i);

            // Find similar memories
            for (j, other) in memories.iter().enumerate() {
                if i == j || assigned.contains(&j) {
                    continue;
                }

                let sim = self.cosine_similarity(&mem.e1_embedding, &other.e1_embedding);

                if sim >= self.config.similarity_threshold && sim <= self.config.max_similarity {
                    cluster.push(other);
                    assigned.insert(j);
                }
            }

            if cluster.len() >= 2 {
                clusters.push(cluster);
            }
        }

        // If no clusters, create one big cluster (for small memory sets)
        if clusters.is_empty() && memories.len() >= 2 {
            clusters.push(memories.iter().collect());
        }

        clusters
    }

    /// Score a candidate pair based on heuristics.
    fn score_candidate_pair(
        &self,
        mem_a: &MemoryForGraphAnalysis,
        mem_b: &MemoryForGraphAnalysis,
    ) -> f32 {
        let mut score = 0.0;

        // 1. Graph markers (0.0-0.4)
        let markers_a = GraphMarkers::count_all_markers(&mem_a.content);
        let markers_b = GraphMarkers::count_all_markers(&mem_b.content);
        let marker_score = ((markers_a + markers_b) as f32 * 0.1).min(0.4);
        score += marker_score;

        // 2. Same session bonus (0.15)
        if mem_a.session_id.is_some() && mem_a.session_id == mem_b.session_id {
            score += 0.15;
        }

        // 3. Code content bonus (0.15)
        if self.config.prioritize_code {
            let is_code_a = self.looks_like_code(&mem_a.content);
            let is_code_b = self.looks_like_code(&mem_b.content);
            if is_code_a || is_code_b {
                score += 0.15;
            }
        }

        // 4. File path similarity (0.1)
        if let (Some(path_a), Some(path_b)) = (&mem_a.file_path, &mem_b.file_path) {
            if self.paths_related(path_a, path_b) {
                score += 0.1;
            }
        }

        // 5. Content length bonus (0.1)
        let min_len = mem_a.content.len().min(mem_b.content.len());
        if min_len > 100 {
            score += 0.05;
        }
        if min_len > 300 {
            score += 0.05;
        }

        // 6. E1 semantic similarity in sweet spot (0.1)
        let sim = self.cosine_similarity(&mem_a.e1_embedding, &mem_b.e1_embedding);
        if sim >= 0.4 && sim <= 0.8 {
            score += 0.1;
        }

        score.clamp(0.0, 1.0)
    }

    /// Detect suspected relationship types based on content markers.
    fn detect_relationship_types(
        &self,
        mem_a: &MemoryForGraphAnalysis,
        mem_b: &MemoryForGraphAnalysis,
    ) -> Vec<RelationshipType> {
        let mut types = HashSet::new();

        // Check content markers in both memories
        for rel_type in RelationshipType::all() {
            let count_a = GraphMarkers::count_markers_for_type(&mem_a.content, *rel_type);
            let count_b = GraphMarkers::count_markers_for_type(&mem_b.content, *rel_type);

            if count_a > 0 || count_b > 0 {
                types.insert(*rel_type);
            }
        }

        types.into_iter().collect()
    }

    /// Check if content looks like code (using DomainMarkers).
    fn looks_like_code(&self, content: &str) -> bool {
        DomainMarkers::looks_like_code(content)
    }

    /// Detect content domain for a memory.
    #[allow(dead_code)]
    fn detect_domain(&self, content: &str) -> ContentDomain {
        DomainMarkers::detect_domain(content)
    }

    /// Detect content domain for a pair of memories.
    #[allow(dead_code)]
    fn detect_domain_pair(&self, content_a: &str, content_b: &str) -> ContentDomain {
        DomainMarkers::detect_domain_pair(content_a, content_b)
    }

    /// Check if two file paths are related.
    fn paths_related(&self, path_a: &str, path_b: &str) -> bool {
        // Same directory
        if let (Some(dir_a), Some(dir_b)) = (
            std::path::Path::new(path_a).parent(),
            std::path::Path::new(path_b).parent(),
        ) {
            if dir_a == dir_b {
                return true;
            }
        }

        // Share common parent directory
        let parts_a: Vec<&str> = path_a.split('/').collect();
        let parts_b: Vec<&str> = path_b.split('/').collect();

        let common = parts_a
            .iter()
            .zip(parts_b.iter())
            .take_while(|(a, b)| a == b)
            .count();

        // At least 2 common path segments
        common >= 2
    }

    /// Cosine similarity between two embeddings.
    fn cosine_similarity(&self, a: &[f32], b: &[f32]) -> f32 {
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

    /// Create a canonical pair key for tracking.
    fn make_pair_key(&self, id_a: Uuid, id_b: Uuid) -> (Uuid, Uuid) {
        if id_a < id_b {
            (id_a, id_b)
        } else {
            (id_b, id_a)
        }
    }
}

impl Default for MemoryScanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_test_memory(id: Uuid, content: &str, embedding: Vec<f32>) -> MemoryForGraphAnalysis {
        MemoryForGraphAnalysis {
            id,
            content: content.to_string(),
            created_at: Utc::now(),
            session_id: Some("test-session".to_string()),
            e1_embedding: embedding,
            source_type: None,
            file_path: None,
        }
    }

    #[test]
    fn test_scanner_default_config() {
        let scanner = MemoryScanner::new();
        assert_eq!(scanner.config.max_candidates, 100);
        assert!(!scanner.config.same_session_only);
    }

    #[test]
    fn test_find_candidates_empty() {
        let mut scanner = MemoryScanner::new();
        let candidates = scanner.find_candidates(&[]).unwrap();
        assert!(candidates.is_empty());
    }

    #[test]
    fn test_find_candidates_single_memory() {
        let mut scanner = MemoryScanner::new();
        let mem = make_test_memory(Uuid::new_v4(), "use crate::foo;", vec![1.0; 1024]);
        let candidates = scanner.find_candidates(&[mem]).unwrap();
        assert!(candidates.is_empty());
    }

    #[test]
    fn test_score_candidate_pair() {
        let scanner = MemoryScanner::new();

        let mem_a = make_test_memory(
            Uuid::new_v4(),
            "use crate::module; impl Trait for Struct",
            vec![0.5; 1024],
        );
        let mem_b = make_test_memory(
            Uuid::new_v4(),
            "pub mod module { pub struct Struct {} }",
            vec![0.6; 1024],
        );

        let score = scanner.score_candidate_pair(&mem_a, &mem_b);
        assert!(score > 0.0);
    }

    #[test]
    fn test_looks_like_code() {
        let scanner = MemoryScanner::new();

        // Code content
        assert!(scanner.looks_like_code("fn main() { println!(\"hello\"); }"));
        assert!(scanner.looks_like_code("pub struct Foo {}"));
        assert!(scanner.looks_like_code("import React from 'react';"));

        // Non-code content
        assert!(!scanner.looks_like_code("This is just plain text without any code."));

        // Legal content should NOT look like code
        assert!(!scanner.looks_like_code(
            "The court held that pursuant to 42 U.S.C. § 1983, the plaintiff..."
        ));
    }

    #[test]
    fn test_domain_detection() {
        let scanner = MemoryScanner::new();

        // Code domain
        assert_eq!(
            scanner.detect_domain("fn main() { use crate::foo; }"),
            ContentDomain::Code
        );

        // Legal domain
        assert_eq!(
            scanner.detect_domain(
                "The court held that pursuant to 42 U.S.C. § 1983, the plaintiff..."
            ),
            ContentDomain::Legal
        );

        // Academic domain
        assert_eq!(
            scanner.detect_domain(
                "Smith et al. (2023) found statistical significance (p < 0.05) with n = 150"
            ),
            ContentDomain::Academic
        );

        // General content
        assert_eq!(
            scanner.detect_domain("This is just some general text."),
            ContentDomain::General
        );
    }

    #[test]
    fn test_mark_analyzed() {
        let mut scanner = MemoryScanner::new();
        let id_a = Uuid::new_v4();
        let id_b = Uuid::new_v4();

        assert_eq!(scanner.analyzed_count(), 0);
        scanner.mark_analyzed(id_a, id_b);
        assert_eq!(scanner.analyzed_count(), 1);

        // Same pair in reverse order
        scanner.mark_analyzed(id_b, id_a);
        assert_eq!(scanner.analyzed_count(), 1);
    }
}
