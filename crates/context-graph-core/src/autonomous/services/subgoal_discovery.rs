//! NORTH-015: Sub-Goal Discovery Service
//!
//! Discovers emergent sub-goals from memory clusters. This service analyzes
//! clusters of related memories to identify patterns that suggest new goals
//! should be added to the goal hierarchy.
//!
//! # Architecture
//!
//! The discovery process:
//! 1. Analyze memory clusters for coherent themes
//! 2. Extract candidate sub-goals with confidence scores
//! 3. Find appropriate parent goals in the hierarchy
//! 4. Determine goal level based on evidence
//! 5. Rank and filter candidates for promotion

use crate::autonomous::bootstrap::GoalId;
use crate::autonomous::curation::MemoryId;
use crate::autonomous::evolution::{GoalEvolutionConfig, GoalLevel, SubGoalCandidate};

/// Memory cluster representing a group of related memories
#[derive(Clone, Debug)]
pub struct MemoryCluster {
    /// Centroid embedding of the cluster (normalized)
    pub centroid: Vec<f32>,
    /// Member memory IDs in this cluster
    pub members: Vec<MemoryId>,
    /// Coherence score of the cluster (0.0 to 1.0)
    pub coherence: f32,
    /// Optional label or description extracted from members
    pub label: Option<String>,
    /// Average alignment of members to current goals
    pub avg_alignment: f32,
}

impl MemoryCluster {
    /// Create a new memory cluster
    pub fn new(centroid: Vec<f32>, members: Vec<MemoryId>, coherence: f32) -> Self {
        assert!(!centroid.is_empty(), "Centroid cannot be empty");
        assert!(
            (0.0..=1.0).contains(&coherence),
            "Coherence must be in [0.0, 1.0]"
        );

        Self {
            centroid,
            members,
            coherence,
            label: None,
            avg_alignment: 0.0,
        }
    }

    /// Create a cluster with a label
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set the average alignment
    pub fn with_avg_alignment(mut self, alignment: f32) -> Self {
        self.avg_alignment = alignment.clamp(0.0, 1.0);
        self
    }

    /// Get the cluster size
    pub fn size(&self) -> usize {
        self.members.len()
    }

    /// Check if cluster is empty
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }
}

/// Configuration for sub-goal discovery
#[derive(Clone, Debug)]
pub struct DiscoveryConfig {
    /// Minimum cluster size to consider for sub-goal extraction
    pub min_cluster_size: usize,
    /// Minimum coherence for a cluster to be viable
    pub min_coherence: f32,
    /// Threshold for emergence (confidence above which to promote)
    pub emergence_threshold: f32,
    /// Maximum candidates to return
    pub max_candidates: usize,
    /// Minimum confidence for a candidate to be viable
    pub min_confidence: f32,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            min_cluster_size: 10,
            min_coherence: 0.6,
            emergence_threshold: 0.7,
            max_candidates: 20,
            min_confidence: 0.5,
        }
    }
}

impl From<&GoalEvolutionConfig> for DiscoveryConfig {
    fn from(config: &GoalEvolutionConfig) -> Self {
        Self {
            min_cluster_size: config.min_cluster_size,
            ..Default::default()
        }
    }
}

/// Result of sub-goal discovery process
#[derive(Clone, Debug)]
pub struct DiscoveryResult {
    /// Discovered sub-goal candidates
    pub candidates: Vec<SubGoalCandidate>,
    /// Number of clusters analyzed
    pub cluster_count: usize,
    /// Average confidence across all candidates
    pub avg_confidence: f32,
    /// Number of clusters that passed minimum size threshold
    pub viable_clusters: usize,
    /// Number of candidates filtered out
    pub filtered_count: usize,
}

impl DiscoveryResult {
    /// Create a new discovery result
    fn new(candidates: Vec<SubGoalCandidate>, cluster_count: usize) -> Self {
        let avg_confidence = if candidates.is_empty() {
            0.0
        } else {
            candidates.iter().map(|c| c.confidence).sum::<f32>() / candidates.len() as f32
        };

        Self {
            candidates,
            cluster_count,
            avg_confidence,
            viable_clusters: 0,
            filtered_count: 0,
        }
    }

    /// Check if any candidates were discovered
    pub fn has_candidates(&self) -> bool {
        !self.candidates.is_empty()
    }

    /// Get candidates that should be promoted
    pub fn promotable_candidates(&self, threshold: f32) -> Vec<&SubGoalCandidate> {
        self.candidates
            .iter()
            .filter(|c| c.confidence >= threshold)
            .collect()
    }
}

/// Sub-goal discovery service
///
/// Discovers emergent sub-goals by analyzing memory clusters for patterns
/// that suggest coherent new goal areas.
#[derive(Clone, Debug)]
pub struct SubGoalDiscovery {
    config: DiscoveryConfig,
}

impl SubGoalDiscovery {
    /// Create a new discovery service with default configuration
    pub fn new() -> Self {
        Self {
            config: DiscoveryConfig::default(),
        }
    }

    /// Create a discovery service with custom configuration
    pub fn with_config(config: DiscoveryConfig) -> Self {
        Self { config }
    }

    /// Create from goal evolution config
    pub fn from_evolution_config(config: &GoalEvolutionConfig) -> Self {
        Self {
            config: DiscoveryConfig::from(config),
        }
    }

    /// Get the current configuration
    pub fn config(&self) -> &DiscoveryConfig {
        &self.config
    }

    /// Discover sub-goals from a set of memory clusters
    ///
    /// Analyzes each cluster and extracts candidate sub-goals that meet
    /// the minimum requirements.
    pub fn discover_from_clusters(&self, clusters: &[MemoryCluster]) -> DiscoveryResult {
        let cluster_count = clusters.len();

        if clusters.is_empty() {
            return DiscoveryResult::new(vec![], 0);
        }

        let mut candidates = Vec::new();
        let mut viable_clusters = 0;
        let mut filtered_count = 0;

        for cluster in clusters {
            // Skip clusters that don't meet minimum size
            if cluster.size() < self.config.min_cluster_size {
                filtered_count += 1;
                continue;
            }

            // Skip clusters with low coherence
            if cluster.coherence < self.config.min_coherence {
                filtered_count += 1;
                continue;
            }

            viable_clusters += 1;

            if let Some(candidate) = self.extract_candidate(cluster) {
                if candidate.confidence >= self.config.min_confidence {
                    candidates.push(candidate);
                } else {
                    filtered_count += 1;
                }
            }
        }

        // Rank candidates by confidence
        self.rank_candidates(&mut candidates);

        // Limit to max candidates
        if candidates.len() > self.config.max_candidates {
            filtered_count += candidates.len() - self.config.max_candidates;
            candidates.truncate(self.config.max_candidates);
        }

        let mut result = DiscoveryResult::new(candidates, cluster_count);
        result.viable_clusters = viable_clusters;
        result.filtered_count = filtered_count;
        result
    }

    /// Extract a sub-goal candidate from a memory cluster
    ///
    /// Returns None if the cluster doesn't meet requirements or lacks
    /// sufficient signal for goal extraction.
    pub fn extract_candidate(&self, cluster: &MemoryCluster) -> Option<SubGoalCandidate> {
        // Validate cluster
        if cluster.is_empty() {
            return None;
        }

        if cluster.size() < self.config.min_cluster_size {
            return None;
        }

        if cluster.coherence < self.config.min_coherence {
            return None;
        }

        let confidence = self.compute_confidence(cluster);
        let level = self.determine_level(confidence, cluster.size());

        // Generate description from label or placeholder
        let description = cluster
            .label
            .clone()
            .unwrap_or_else(|| format!("Emergent goal from {} memories", cluster.size()));

        Some(SubGoalCandidate {
            suggested_description: description,
            level,
            parent_id: GoalId::new(), // Placeholder, will be assigned by find_parent_goal
            cluster_size: cluster.size(),
            centroid_alignment: cluster.avg_alignment,
            confidence,
            supporting_memories: cluster.members.clone(),
        })
    }

    /// Compute confidence score for a cluster
    ///
    /// Confidence is based on:
    /// - Cluster coherence (40%)
    /// - Cluster size (30%)
    /// - Average alignment (30%)
    pub fn compute_confidence(&self, cluster: &MemoryCluster) -> f32 {
        if cluster.is_empty() {
            return 0.0;
        }

        // Coherence contribution (40%)
        let coherence_score = cluster.coherence * 0.4;

        // Size contribution (30%) - logarithmic scaling, max at ~100 members
        let size_score = (cluster.size() as f32).ln().min(4.6) / 4.6 * 0.3;

        // Alignment contribution (30%)
        let alignment_score = cluster.avg_alignment * 0.3;

        (coherence_score + size_score + alignment_score).clamp(0.0, 1.0)
    }

    /// Find the best parent goal for a candidate
    ///
    /// Uses the candidate's centroid alignment and level to find
    /// the most appropriate parent in the existing hierarchy.
    pub fn find_parent_goal(
        &self,
        candidate: &SubGoalCandidate,
        existing_goals: &[GoalId],
    ) -> Option<GoalId> {
        if existing_goals.is_empty() {
            return None;
        }

        // For now, return the first goal as a simple heuristic
        // In production, this would compute similarity between
        // the candidate's centroid and each goal's embedding
        match candidate.level {
            GoalLevel::Strategic | GoalLevel::Tactical => Some(existing_goals[0].clone()),
            GoalLevel::Operational => {
                // Prefer non-first goal if available (assuming hierarchy)
                if existing_goals.len() > 1 {
                    Some(existing_goals[1].clone())
                } else {
                    Some(existing_goals[0].clone())
                }
            }
            GoalLevel::NorthStar => None, // NorthStar has no parent
        }
    }

    /// Determine the appropriate goal level based on confidence and evidence
    ///
    /// Higher confidence and more evidence suggest higher-level goals.
    pub fn determine_level(&self, confidence: f32, evidence_count: usize) -> GoalLevel {
        // Strong signal with lots of evidence -> Strategic
        if confidence >= 0.85 && evidence_count >= 50 {
            return GoalLevel::Strategic;
        }

        // Good signal with moderate evidence -> Tactical
        if confidence >= 0.7 && evidence_count >= 20 {
            return GoalLevel::Tactical;
        }

        // Default to Operational for weaker signals
        GoalLevel::Operational
    }

    /// Check if a candidate should be promoted to an actual goal
    ///
    /// Candidates should be promoted if they exceed the emergence threshold
    /// and have sufficient supporting evidence.
    pub fn should_promote(&self, candidate: &SubGoalCandidate) -> bool {
        // Must meet emergence threshold
        if candidate.confidence < self.config.emergence_threshold {
            return false;
        }

        // Must have minimum cluster size
        if candidate.cluster_size < self.config.min_cluster_size {
            return false;
        }

        // Must have reasonable alignment
        if candidate.centroid_alignment < 0.3 {
            return false;
        }

        true
    }

    /// Rank candidates by priority for promotion
    ///
    /// Ranking considers confidence, cluster size, and alignment.
    pub fn rank_candidates(&self, candidates: &mut [SubGoalCandidate]) {
        candidates.sort_by(|a, b| {
            // Primary sort by confidence (descending)
            let conf_cmp = b
                .confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal);

            if conf_cmp != std::cmp::Ordering::Equal {
                return conf_cmp;
            }

            // Secondary sort by cluster size (descending)
            let size_cmp = b.cluster_size.cmp(&a.cluster_size);

            if size_cmp != std::cmp::Ordering::Equal {
                return size_cmp;
            }

            // Tertiary sort by alignment (descending)
            b.centroid_alignment
                .partial_cmp(&a.centroid_alignment)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }
}

impl Default for SubGoalDiscovery {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create a test cluster
    fn make_cluster(size: usize, coherence: f32, alignment: f32) -> MemoryCluster {
        let members: Vec<MemoryId> = (0..size).map(|_| MemoryId::new()).collect();
        MemoryCluster::new(vec![0.1, 0.2, 0.3], members, coherence).with_avg_alignment(alignment)
    }

    fn make_labeled_cluster(
        size: usize,
        coherence: f32,
        alignment: f32,
        label: &str,
    ) -> MemoryCluster {
        make_cluster(size, coherence, alignment).with_label(label)
    }

    // MemoryCluster tests
    #[test]
    fn test_memory_cluster_new() {
        let centroid = vec![0.1, 0.2, 0.3];
        let members = vec![MemoryId::new(), MemoryId::new()];
        let cluster = MemoryCluster::new(centroid.clone(), members.clone(), 0.8);

        assert_eq!(cluster.centroid, centroid);
        assert_eq!(cluster.members.len(), 2);
        assert!((cluster.coherence - 0.8).abs() < f32::EPSILON);
        assert!(cluster.label.is_none());
        assert!((cluster.avg_alignment - 0.0).abs() < f32::EPSILON);

        println!("[PASS] test_memory_cluster_new");
    }

    #[test]
    fn test_memory_cluster_with_label() {
        let cluster = make_cluster(5, 0.7, 0.5).with_label("Test Label");

        assert_eq!(cluster.label, Some("Test Label".to_string()));

        println!("[PASS] test_memory_cluster_with_label");
    }

    #[test]
    fn test_memory_cluster_with_avg_alignment() {
        let cluster = make_cluster(5, 0.7, 0.0).with_avg_alignment(0.85);

        assert!((cluster.avg_alignment - 0.85).abs() < f32::EPSILON);

        println!("[PASS] test_memory_cluster_with_avg_alignment");
    }

    #[test]
    fn test_memory_cluster_alignment_clamping() {
        let cluster1 = make_cluster(5, 0.7, 0.0).with_avg_alignment(1.5);
        assert!((cluster1.avg_alignment - 1.0).abs() < f32::EPSILON);

        let cluster2 = make_cluster(5, 0.7, 0.0).with_avg_alignment(-0.5);
        assert!((cluster2.avg_alignment - 0.0).abs() < f32::EPSILON);

        println!("[PASS] test_memory_cluster_alignment_clamping");
    }

    #[test]
    fn test_memory_cluster_size() {
        let cluster = make_cluster(10, 0.7, 0.5);
        assert_eq!(cluster.size(), 10);

        println!("[PASS] test_memory_cluster_size");
    }

    #[test]
    fn test_memory_cluster_is_empty() {
        let empty_cluster = MemoryCluster::new(vec![0.1], vec![], 0.5);
        assert!(empty_cluster.is_empty());

        let non_empty = make_cluster(5, 0.7, 0.5);
        assert!(!non_empty.is_empty());

        println!("[PASS] test_memory_cluster_is_empty");
    }

    #[test]
    #[should_panic(expected = "Centroid cannot be empty")]
    fn test_memory_cluster_empty_centroid_panics() {
        MemoryCluster::new(vec![], vec![MemoryId::new()], 0.5);
    }

    #[test]
    #[should_panic(expected = "Coherence must be in")]
    fn test_memory_cluster_invalid_coherence_panics() {
        MemoryCluster::new(vec![0.1], vec![], 1.5);
    }

    // DiscoveryConfig tests
    #[test]
    fn test_discovery_config_default() {
        let config = DiscoveryConfig::default();

        assert_eq!(config.min_cluster_size, 10);
        assert!((config.min_coherence - 0.6).abs() < f32::EPSILON);
        assert!((config.emergence_threshold - 0.7).abs() < f32::EPSILON);
        assert_eq!(config.max_candidates, 20);
        assert!((config.min_confidence - 0.5).abs() < f32::EPSILON);

        println!("[PASS] test_discovery_config_default");
    }

    #[test]
    fn test_discovery_config_from_evolution_config() {
        let evolution_config = GoalEvolutionConfig {
            min_cluster_size: 15,
            ..Default::default()
        };

        let discovery_config = DiscoveryConfig::from(&evolution_config);

        assert_eq!(discovery_config.min_cluster_size, 15);

        println!("[PASS] test_discovery_config_from_evolution_config");
    }

    // DiscoveryResult tests
    #[test]
    fn test_discovery_result_empty() {
        let result = DiscoveryResult::new(vec![], 0);

        assert!(!result.has_candidates());
        assert_eq!(result.cluster_count, 0);
        assert!((result.avg_confidence - 0.0).abs() < f32::EPSILON);

        println!("[PASS] test_discovery_result_empty");
    }

    #[test]
    fn test_discovery_result_with_candidates() {
        let candidates = vec![
            SubGoalCandidate {
                suggested_description: "Test 1".into(),
                level: GoalLevel::Tactical,
                parent_id: GoalId::new(),
                cluster_size: 10,
                centroid_alignment: 0.8,
                confidence: 0.7,
                supporting_memories: vec![],
            },
            SubGoalCandidate {
                suggested_description: "Test 2".into(),
                level: GoalLevel::Operational,
                parent_id: GoalId::new(),
                cluster_size: 15,
                centroid_alignment: 0.6,
                confidence: 0.9,
                supporting_memories: vec![],
            },
        ];

        let result = DiscoveryResult::new(candidates, 5);

        assert!(result.has_candidates());
        assert_eq!(result.cluster_count, 5);
        assert_eq!(result.candidates.len(), 2);
        // Average confidence: (0.7 + 0.9) / 2 = 0.8
        assert!((result.avg_confidence - 0.8).abs() < f32::EPSILON);

        println!("[PASS] test_discovery_result_with_candidates");
    }

    #[test]
    fn test_discovery_result_promotable_candidates() {
        let candidates = vec![
            SubGoalCandidate {
                suggested_description: "High".into(),
                level: GoalLevel::Strategic,
                parent_id: GoalId::new(),
                cluster_size: 20,
                centroid_alignment: 0.9,
                confidence: 0.85,
                supporting_memories: vec![],
            },
            SubGoalCandidate {
                suggested_description: "Low".into(),
                level: GoalLevel::Operational,
                parent_id: GoalId::new(),
                cluster_size: 10,
                centroid_alignment: 0.5,
                confidence: 0.55,
                supporting_memories: vec![],
            },
        ];

        let result = DiscoveryResult::new(candidates, 2);
        let promotable = result.promotable_candidates(0.7);

        assert_eq!(promotable.len(), 1);
        assert_eq!(promotable[0].suggested_description, "High");

        println!("[PASS] test_discovery_result_promotable_candidates");
    }

    // SubGoalDiscovery tests
    #[test]
    fn test_subgoal_discovery_new() {
        let discovery = SubGoalDiscovery::new();

        assert_eq!(discovery.config().min_cluster_size, 10);

        println!("[PASS] test_subgoal_discovery_new");
    }

    #[test]
    fn test_subgoal_discovery_with_config() {
        let config = DiscoveryConfig {
            min_cluster_size: 5,
            min_coherence: 0.5,
            emergence_threshold: 0.8,
            max_candidates: 10,
            min_confidence: 0.4,
        };

        let discovery = SubGoalDiscovery::with_config(config);

        assert_eq!(discovery.config().min_cluster_size, 5);
        assert!((discovery.config().min_coherence - 0.5).abs() < f32::EPSILON);

        println!("[PASS] test_subgoal_discovery_with_config");
    }

    #[test]
    fn test_subgoal_discovery_from_evolution_config() {
        let evolution_config = GoalEvolutionConfig {
            min_cluster_size: 25,
            ..Default::default()
        };

        let discovery = SubGoalDiscovery::from_evolution_config(&evolution_config);

        assert_eq!(discovery.config().min_cluster_size, 25);

        println!("[PASS] test_subgoal_discovery_from_evolution_config");
    }

    #[test]
    fn test_discover_from_clusters_empty() {
        let discovery = SubGoalDiscovery::new();
        let result = discovery.discover_from_clusters(&[]);

        assert!(!result.has_candidates());
        assert_eq!(result.cluster_count, 0);

        println!("[PASS] test_discover_from_clusters_empty");
    }

    #[test]
    fn test_discover_from_clusters_filters_small() {
        let discovery = SubGoalDiscovery::new(); // min_cluster_size = 10

        let clusters = vec![
            make_cluster(5, 0.8, 0.7),  // Too small
            make_cluster(15, 0.8, 0.7), // Valid
        ];

        let result = discovery.discover_from_clusters(&clusters);

        assert_eq!(result.cluster_count, 2);
        assert_eq!(result.viable_clusters, 1);
        assert!(result.filtered_count >= 1);

        println!("[PASS] test_discover_from_clusters_filters_small");
    }

    #[test]
    fn test_discover_from_clusters_filters_low_coherence() {
        let config = DiscoveryConfig {
            min_cluster_size: 5,
            min_coherence: 0.7,
            ..Default::default()
        };
        let discovery = SubGoalDiscovery::with_config(config);

        let clusters = vec![
            make_cluster(10, 0.5, 0.7), // Low coherence
            make_cluster(10, 0.8, 0.7), // Valid
        ];

        let result = discovery.discover_from_clusters(&clusters);

        assert_eq!(result.viable_clusters, 1);

        println!("[PASS] test_discover_from_clusters_filters_low_coherence");
    }

    #[test]
    fn test_discover_from_clusters_valid() {
        let config = DiscoveryConfig {
            min_cluster_size: 5,
            min_coherence: 0.5,
            min_confidence: 0.3,
            ..Default::default()
        };
        let discovery = SubGoalDiscovery::with_config(config);

        let clusters = vec![
            make_labeled_cluster(10, 0.8, 0.7, "Goal A"),
            make_labeled_cluster(15, 0.9, 0.8, "Goal B"),
        ];

        let result = discovery.discover_from_clusters(&clusters);

        assert!(result.has_candidates());
        assert_eq!(result.candidates.len(), 2);

        println!("[PASS] test_discover_from_clusters_valid");
    }

    #[test]
    fn test_discover_from_clusters_respects_max_candidates() {
        let config = DiscoveryConfig {
            min_cluster_size: 5,
            min_coherence: 0.5,
            min_confidence: 0.0,
            max_candidates: 2,
            ..Default::default()
        };
        let discovery = SubGoalDiscovery::with_config(config);

        let clusters: Vec<MemoryCluster> = (0..5)
            .map(|i| make_labeled_cluster(10, 0.8, 0.7, &format!("Goal {}", i)))
            .collect();

        let result = discovery.discover_from_clusters(&clusters);

        assert_eq!(result.candidates.len(), 2);
        assert!(result.filtered_count >= 3);

        println!("[PASS] test_discover_from_clusters_respects_max_candidates");
    }

    // extract_candidate tests
    #[test]
    fn test_extract_candidate_valid() {
        let config = DiscoveryConfig {
            min_cluster_size: 5,
            min_coherence: 0.5,
            ..Default::default()
        };
        let discovery = SubGoalDiscovery::with_config(config);

        let cluster = make_labeled_cluster(10, 0.8, 0.7, "Test Goal");

        let candidate = discovery.extract_candidate(&cluster);

        assert!(candidate.is_some());
        let c = candidate.unwrap();
        assert_eq!(c.suggested_description, "Test Goal");
        assert_eq!(c.cluster_size, 10);
        assert!((c.centroid_alignment - 0.7).abs() < f32::EPSILON);
        assert!(c.confidence > 0.0);

        println!("[PASS] test_extract_candidate_valid");
    }

    #[test]
    fn test_extract_candidate_empty_cluster() {
        let discovery = SubGoalDiscovery::new();
        let cluster = MemoryCluster::new(vec![0.1], vec![], 0.8);

        let candidate = discovery.extract_candidate(&cluster);

        assert!(candidate.is_none());

        println!("[PASS] test_extract_candidate_empty_cluster");
    }

    #[test]
    fn test_extract_candidate_too_small() {
        let discovery = SubGoalDiscovery::new(); // min_cluster_size = 10
        let cluster = make_cluster(5, 0.8, 0.7);

        let candidate = discovery.extract_candidate(&cluster);

        assert!(candidate.is_none());

        println!("[PASS] test_extract_candidate_too_small");
    }

    #[test]
    fn test_extract_candidate_low_coherence() {
        let discovery = SubGoalDiscovery::new(); // min_coherence = 0.6
        let cluster = make_cluster(15, 0.4, 0.7);

        let candidate = discovery.extract_candidate(&cluster);

        assert!(candidate.is_none());

        println!("[PASS] test_extract_candidate_low_coherence");
    }

    #[test]
    fn test_extract_candidate_generates_description() {
        let config = DiscoveryConfig {
            min_cluster_size: 5,
            min_coherence: 0.5,
            ..Default::default()
        };
        let discovery = SubGoalDiscovery::with_config(config);

        let cluster = make_cluster(10, 0.8, 0.7); // No label

        let candidate = discovery.extract_candidate(&cluster).unwrap();

        assert!(candidate.suggested_description.contains("10 memories"));

        println!("[PASS] test_extract_candidate_generates_description");
    }

    // compute_confidence tests
    #[test]
    fn test_compute_confidence_empty_cluster() {
        let discovery = SubGoalDiscovery::new();
        let cluster = MemoryCluster::new(vec![0.1], vec![], 0.8);

        let confidence = discovery.compute_confidence(&cluster);

        assert!((confidence - 0.0).abs() < f32::EPSILON);

        println!("[PASS] test_compute_confidence_empty_cluster");
    }

    #[test]
    fn test_compute_confidence_components() {
        let discovery = SubGoalDiscovery::new();

        // High coherence cluster
        let high_coherence = make_cluster(10, 0.9, 0.5);
        let conf1 = discovery.compute_confidence(&high_coherence);

        // Low coherence cluster (same size and alignment)
        let low_coherence = make_cluster(10, 0.3, 0.5);
        let conf2 = discovery.compute_confidence(&low_coherence);

        assert!(
            conf1 > conf2,
            "Higher coherence should yield higher confidence"
        );

        // Larger cluster (same coherence and alignment)
        let larger = make_cluster(50, 0.7, 0.5);
        let smaller = make_cluster(10, 0.7, 0.5);
        let conf3 = discovery.compute_confidence(&larger);
        let conf4 = discovery.compute_confidence(&smaller);

        assert!(
            conf3 > conf4,
            "Larger cluster should yield higher confidence"
        );

        println!("[PASS] test_compute_confidence_components");
    }

    #[test]
    fn test_compute_confidence_bounds() {
        let discovery = SubGoalDiscovery::new();

        // Maximum values
        let max_cluster = make_cluster(100, 1.0, 1.0);
        let max_conf = discovery.compute_confidence(&max_cluster);
        assert!(max_conf <= 1.0);
        assert!(max_conf >= 0.0);

        // Minimum non-empty values
        let min_cluster = make_cluster(1, 0.0, 0.0);
        let min_conf = discovery.compute_confidence(&min_cluster);
        assert!(min_conf >= 0.0);
        assert!(min_conf <= 1.0);

        println!("[PASS] test_compute_confidence_bounds");
    }

    // find_parent_goal tests
    #[test]
    fn test_find_parent_goal_empty_goals() {
        let discovery = SubGoalDiscovery::new();
        let candidate = SubGoalCandidate {
            suggested_description: "Test".into(),
            level: GoalLevel::Tactical,
            parent_id: GoalId::new(),
            cluster_size: 10,
            centroid_alignment: 0.7,
            confidence: 0.8,
            supporting_memories: vec![],
        };

        let parent = discovery.find_parent_goal(&candidate, &[]);

        assert!(parent.is_none());

        println!("[PASS] test_find_parent_goal_empty_goals");
    }

    #[test]
    fn test_find_parent_goal_strategic() {
        let discovery = SubGoalDiscovery::new();
        let goals = vec![GoalId::new(), GoalId::new()];

        let candidate = SubGoalCandidate {
            suggested_description: "Test".into(),
            level: GoalLevel::Strategic,
            parent_id: GoalId::new(),
            cluster_size: 50,
            centroid_alignment: 0.9,
            confidence: 0.9,
            supporting_memories: vec![],
        };

        let parent = discovery.find_parent_goal(&candidate, &goals);

        assert!(parent.is_some());
        assert_eq!(parent.unwrap(), goals[0]);

        println!("[PASS] test_find_parent_goal_strategic");
    }

    #[test]
    fn test_find_parent_goal_operational() {
        let discovery = SubGoalDiscovery::new();
        let goals = vec![GoalId::new(), GoalId::new()];

        let candidate = SubGoalCandidate {
            suggested_description: "Test".into(),
            level: GoalLevel::Operational,
            parent_id: GoalId::new(),
            cluster_size: 10,
            centroid_alignment: 0.5,
            confidence: 0.6,
            supporting_memories: vec![],
        };

        let parent = discovery.find_parent_goal(&candidate, &goals);

        assert!(parent.is_some());
        // Operational prefers second goal if available
        assert_eq!(parent.unwrap(), goals[1]);

        println!("[PASS] test_find_parent_goal_operational");
    }

    #[test]
    fn test_find_parent_goal_northstar() {
        let discovery = SubGoalDiscovery::new();
        let goals = vec![GoalId::new()];

        let candidate = SubGoalCandidate {
            suggested_description: "Test".into(),
            level: GoalLevel::NorthStar,
            parent_id: GoalId::new(),
            cluster_size: 100,
            centroid_alignment: 1.0,
            confidence: 1.0,
            supporting_memories: vec![],
        };

        let parent = discovery.find_parent_goal(&candidate, &goals);

        assert!(parent.is_none(), "NorthStar should have no parent");

        println!("[PASS] test_find_parent_goal_northstar");
    }

    // determine_level tests
    #[test]
    fn test_determine_level_strategic() {
        let discovery = SubGoalDiscovery::new();

        let level = discovery.determine_level(0.9, 60);

        assert_eq!(level, GoalLevel::Strategic);

        println!("[PASS] test_determine_level_strategic");
    }

    #[test]
    fn test_determine_level_tactical() {
        let discovery = SubGoalDiscovery::new();

        let level = discovery.determine_level(0.75, 30);

        assert_eq!(level, GoalLevel::Tactical);

        println!("[PASS] test_determine_level_tactical");
    }

    #[test]
    fn test_determine_level_operational() {
        let discovery = SubGoalDiscovery::new();

        let level = discovery.determine_level(0.5, 10);

        assert_eq!(level, GoalLevel::Operational);

        println!("[PASS] test_determine_level_operational");
    }

    #[test]
    fn test_determine_level_boundary_strategic() {
        let discovery = SubGoalDiscovery::new();

        // Just at strategic threshold
        let level = discovery.determine_level(0.85, 50);
        assert_eq!(level, GoalLevel::Strategic);

        // Just below strategic (high confidence but not enough evidence)
        let level = discovery.determine_level(0.85, 49);
        assert_eq!(level, GoalLevel::Tactical);

        println!("[PASS] test_determine_level_boundary_strategic");
    }

    #[test]
    fn test_determine_level_boundary_tactical() {
        let discovery = SubGoalDiscovery::new();

        // Just at tactical threshold
        let level = discovery.determine_level(0.7, 20);
        assert_eq!(level, GoalLevel::Tactical);

        // Just below tactical
        let level = discovery.determine_level(0.69, 20);
        assert_eq!(level, GoalLevel::Operational);

        println!("[PASS] test_determine_level_boundary_tactical");
    }

    // should_promote tests
    #[test]
    fn test_should_promote_valid() {
        let discovery = SubGoalDiscovery::new(); // emergence_threshold = 0.7

        let candidate = SubGoalCandidate {
            suggested_description: "Test".into(),
            level: GoalLevel::Tactical,
            parent_id: GoalId::new(),
            cluster_size: 15,
            centroid_alignment: 0.6,
            confidence: 0.8,
            supporting_memories: vec![],
        };

        assert!(discovery.should_promote(&candidate));

        println!("[PASS] test_should_promote_valid");
    }

    #[test]
    fn test_should_promote_low_confidence() {
        let discovery = SubGoalDiscovery::new(); // emergence_threshold = 0.7

        let candidate = SubGoalCandidate {
            suggested_description: "Test".into(),
            level: GoalLevel::Tactical,
            parent_id: GoalId::new(),
            cluster_size: 15,
            centroid_alignment: 0.6,
            confidence: 0.5, // Below threshold
            supporting_memories: vec![],
        };

        assert!(!discovery.should_promote(&candidate));

        println!("[PASS] test_should_promote_low_confidence");
    }

    #[test]
    fn test_should_promote_small_cluster() {
        let discovery = SubGoalDiscovery::new(); // min_cluster_size = 10

        let candidate = SubGoalCandidate {
            suggested_description: "Test".into(),
            level: GoalLevel::Tactical,
            parent_id: GoalId::new(),
            cluster_size: 5, // Below minimum
            centroid_alignment: 0.6,
            confidence: 0.8,
            supporting_memories: vec![],
        };

        assert!(!discovery.should_promote(&candidate));

        println!("[PASS] test_should_promote_small_cluster");
    }

    #[test]
    fn test_should_promote_low_alignment() {
        let discovery = SubGoalDiscovery::new();

        let candidate = SubGoalCandidate {
            suggested_description: "Test".into(),
            level: GoalLevel::Tactical,
            parent_id: GoalId::new(),
            cluster_size: 15,
            centroid_alignment: 0.2, // Below 0.3 threshold
            confidence: 0.8,
            supporting_memories: vec![],
        };

        assert!(!discovery.should_promote(&candidate));

        println!("[PASS] test_should_promote_low_alignment");
    }

    // rank_candidates tests
    #[test]
    fn test_rank_candidates_by_confidence() {
        let discovery = SubGoalDiscovery::new();

        let mut candidates = vec![
            SubGoalCandidate {
                suggested_description: "Low".into(),
                level: GoalLevel::Operational,
                parent_id: GoalId::new(),
                cluster_size: 10,
                centroid_alignment: 0.5,
                confidence: 0.5,
                supporting_memories: vec![],
            },
            SubGoalCandidate {
                suggested_description: "High".into(),
                level: GoalLevel::Strategic,
                parent_id: GoalId::new(),
                cluster_size: 10,
                centroid_alignment: 0.5,
                confidence: 0.9,
                supporting_memories: vec![],
            },
        ];

        discovery.rank_candidates(&mut candidates);

        assert_eq!(candidates[0].suggested_description, "High");
        assert_eq!(candidates[1].suggested_description, "Low");

        println!("[PASS] test_rank_candidates_by_confidence");
    }

    #[test]
    fn test_rank_candidates_by_size_tiebreaker() {
        let discovery = SubGoalDiscovery::new();

        let mut candidates = vec![
            SubGoalCandidate {
                suggested_description: "Small".into(),
                level: GoalLevel::Tactical,
                parent_id: GoalId::new(),
                cluster_size: 10,
                centroid_alignment: 0.5,
                confidence: 0.8,
                supporting_memories: vec![],
            },
            SubGoalCandidate {
                suggested_description: "Large".into(),
                level: GoalLevel::Tactical,
                parent_id: GoalId::new(),
                cluster_size: 50,
                centroid_alignment: 0.5,
                confidence: 0.8, // Same confidence
                supporting_memories: vec![],
            },
        ];

        discovery.rank_candidates(&mut candidates);

        assert_eq!(candidates[0].suggested_description, "Large");
        assert_eq!(candidates[1].suggested_description, "Small");

        println!("[PASS] test_rank_candidates_by_size_tiebreaker");
    }

    #[test]
    fn test_rank_candidates_by_alignment_tiebreaker() {
        let discovery = SubGoalDiscovery::new();

        let mut candidates = vec![
            SubGoalCandidate {
                suggested_description: "LowAlign".into(),
                level: GoalLevel::Tactical,
                parent_id: GoalId::new(),
                cluster_size: 20,
                centroid_alignment: 0.3,
                confidence: 0.8,
                supporting_memories: vec![],
            },
            SubGoalCandidate {
                suggested_description: "HighAlign".into(),
                level: GoalLevel::Tactical,
                parent_id: GoalId::new(),
                cluster_size: 20, // Same size
                centroid_alignment: 0.9,
                confidence: 0.8, // Same confidence
                supporting_memories: vec![],
            },
        ];

        discovery.rank_candidates(&mut candidates);

        assert_eq!(candidates[0].suggested_description, "HighAlign");
        assert_eq!(candidates[1].suggested_description, "LowAlign");

        println!("[PASS] test_rank_candidates_by_alignment_tiebreaker");
    }

    #[test]
    fn test_rank_candidates_empty() {
        let discovery = SubGoalDiscovery::new();
        let mut candidates: Vec<SubGoalCandidate> = vec![];

        discovery.rank_candidates(&mut candidates);

        assert!(candidates.is_empty());

        println!("[PASS] test_rank_candidates_empty");
    }

    #[test]
    fn test_rank_candidates_single() {
        let discovery = SubGoalDiscovery::new();
        let mut candidates = vec![SubGoalCandidate {
            suggested_description: "Only".into(),
            level: GoalLevel::Tactical,
            parent_id: GoalId::new(),
            cluster_size: 10,
            centroid_alignment: 0.5,
            confidence: 0.7,
            supporting_memories: vec![],
        }];

        discovery.rank_candidates(&mut candidates);

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].suggested_description, "Only");

        println!("[PASS] test_rank_candidates_single");
    }

    // Default trait test
    #[test]
    fn test_subgoal_discovery_default() {
        let discovery = SubGoalDiscovery::default();

        assert_eq!(discovery.config().min_cluster_size, 10);

        println!("[PASS] test_subgoal_discovery_default");
    }

    // Integration test
    #[test]
    fn test_full_discovery_workflow() {
        let config = DiscoveryConfig {
            min_cluster_size: 5,
            min_coherence: 0.5,
            min_confidence: 0.4,
            emergence_threshold: 0.6,
            max_candidates: 10,
        };
        let discovery = SubGoalDiscovery::with_config(config);

        let clusters = vec![
            make_labeled_cluster(20, 0.9, 0.8, "Primary Focus Area"),
            make_labeled_cluster(15, 0.8, 0.7, "Secondary Focus"),
            make_labeled_cluster(3, 0.9, 0.9, "Too Small"), // Filtered
            make_labeled_cluster(10, 0.3, 0.8, "Low Coherence"), // Filtered
            make_labeled_cluster(8, 0.7, 0.6, "Moderate Focus"),
        ];

        let result = discovery.discover_from_clusters(&clusters);

        assert_eq!(result.cluster_count, 5);
        assert_eq!(result.viable_clusters, 3);
        assert!(result.has_candidates());

        // Check candidates are ranked by confidence
        for i in 0..result.candidates.len() - 1 {
            assert!(result.candidates[i].confidence >= result.candidates[i + 1].confidence);
        }

        // Check promotable candidates
        let promotable = result.promotable_candidates(0.6);
        for c in &promotable {
            assert!(discovery.should_promote(c));
        }

        // Test parent assignment
        let goals = vec![GoalId::new(), GoalId::new(), GoalId::new()];
        for candidate in &result.candidates {
            let parent = discovery.find_parent_goal(candidate, &goals);
            if candidate.level != GoalLevel::NorthStar {
                assert!(parent.is_some());
            }
        }

        println!("[PASS] test_full_discovery_workflow");
    }
}
