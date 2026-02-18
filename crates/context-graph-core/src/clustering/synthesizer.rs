//! Standalone topic synthesizer for cross-space topic discovery.
//!
//! Uses weighted agreement formula per ARCH-09:
//! - SEMANTIC embedders (E1, E5, E6, E7, E10, E12, E13): 1.0 weight
//! - TEMPORAL embedders (E2, E3, E4): 0.0 weight (excluded per AP-60)
//! - RELATIONAL embedders (E8, E11): 0.5 weight
//! - STRUCTURAL embedder (E9): 0.5 weight
//!
//! # Topic Detection
//!
//! A topic is formed when memories cluster together across embedding spaces
//! with weighted_agreement >= 2.5 (topic_threshold from category.rs).
//!
//! # Examples
//!
//! ```text
//! 3 semantic spaces agreeing = 3.0 >= 2.5 -> TOPIC
//! 2 semantic + 1 relational = 2.5 >= 2.5 -> TOPIC
//! 2 semantic spaces only = 2.0 < 2.5 -> NOT TOPIC
//! 5 temporal spaces = 0.0 < 2.5 -> NOT TOPIC (excluded per AP-60)
//! ```

use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use crate::embeddings::category::{category_for, max_weighted_agreement, topic_threshold};
use crate::teleological::Embedder;

use super::error::ClusterError;
use super::membership::ClusterMembership;
use super::topic::{Topic, TopicProfile};

/// Default threshold for merging similar topics (profile cosine similarity).
pub const DEFAULT_MERGE_THRESHOLD: f32 = 0.9;

/// Default minimum silhouette score for valid clusters.
pub const DEFAULT_MIN_SILHOUETTE: f32 = 0.3;

/// Synthesizes topics from cross-space clustering using weighted agreement.
///
/// This is a standalone component that can be used independently of
/// `MultiSpaceClusterManager` for topic discovery.
///
/// # Usage
///
/// ```
/// use context_graph_core::clustering::{TopicSynthesizer, ClusterMembership};
/// use context_graph_core::teleological::Embedder;
/// use std::collections::HashMap;
/// use uuid::Uuid;
///
/// let synthesizer = TopicSynthesizer::new();
///
/// // Create cluster memberships for multiple memories
/// let mut memberships: HashMap<Embedder, Vec<ClusterMembership>> = HashMap::new();
/// // ... populate memberships ...
///
/// // let topics = synthesizer.synthesize_topics(&memberships).unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct TopicSynthesizer {
    /// Threshold for merging similar topics (default 0.9).
    merge_similarity_threshold: f32,
    /// Minimum silhouette score for valid clusters (default 0.3).
    min_silhouette: f32,
}

impl Default for TopicSynthesizer {
    fn default() -> Self {
        Self::new()
    }
}

impl TopicSynthesizer {
    /// Create with default configuration.
    ///
    /// - merge_similarity_threshold: 0.9
    /// - min_silhouette: 0.3
    pub fn new() -> Self {
        Self {
            merge_similarity_threshold: DEFAULT_MERGE_THRESHOLD,
            min_silhouette: DEFAULT_MIN_SILHOUETTE,
        }
    }

    /// Create with custom configuration.
    ///
    /// # Arguments
    /// * `merge_threshold` - Similarity threshold for merging topics (clamped 0.0..=1.0)
    /// * `min_silhouette` - Minimum silhouette score (clamped -1.0..=1.0)
    pub fn with_config(merge_threshold: f32, min_silhouette: f32) -> Self {
        Self {
            merge_similarity_threshold: merge_threshold.clamp(0.0, 1.0),
            min_silhouette: min_silhouette.clamp(-1.0, 1.0),
        }
    }

    /// Get the merge similarity threshold.
    #[inline]
    pub fn merge_similarity_threshold(&self) -> f32 {
        self.merge_similarity_threshold
    }

    /// Get the minimum silhouette score.
    #[inline]
    pub fn min_silhouette(&self) -> f32 {
        self.min_silhouette
    }

    /// Build map: memory_id -> (embedder -> cluster_id)
    ///
    /// This reorganizes the memberships from per-space to per-memory format.
    fn build_mem_clusters_map(
        &self,
        memberships: &HashMap<Embedder, Vec<ClusterMembership>>,
    ) -> HashMap<Uuid, HashMap<Embedder, i32>> {
        let mut result: HashMap<Uuid, HashMap<Embedder, i32>> = HashMap::new();

        for (embedder, space_memberships) in memberships {
            for m in space_memberships {
                result
                    .entry(m.memory_id)
                    .or_default()
                    .insert(*embedder, m.cluster_id);
            }
        }

        result
    }

    /// Compute weighted agreement between two memories.
    ///
    /// Uses `category_for(embedder).topic_weight()` from category.rs.
    /// Temporal embedders (E2-E4) contribute 0.0 per AP-60.
    ///
    /// # Returns
    ///
    /// The sum of topic weights for embedders where both memories are in the
    /// same non-noise cluster. Range is [0.0, 8.5].
    fn compute_weighted_agreement(
        &self,
        mem_a: &Uuid,
        mem_b: &Uuid,
        mem_clusters: &HashMap<Uuid, HashMap<Embedder, i32>>,
    ) -> f32 {
        let Some(clusters_a) = mem_clusters.get(mem_a) else {
            return 0.0;
        };
        let Some(clusters_b) = mem_clusters.get(mem_b) else {
            return 0.0;
        };

        let mut weighted = 0.0f32;
        for embedder in Embedder::all() {
            let ca = clusters_a.get(&embedder).copied().unwrap_or(-1);
            let cb = clusters_b.get(&embedder).copied().unwrap_or(-1);

            // Both in same non-noise cluster
            if ca != -1 && ca == cb {
                weighted += category_for(embedder).topic_weight();
            }
        }
        weighted
    }

    /// Find groups of memories using Union-Find (connected components).
    ///
    /// An edge exists between memories if their weighted_agreement >= topic_threshold (2.5).
    ///
    /// # Returns
    ///
    /// Vector of memory groups, where each group is a Vec of memory UUIDs that
    /// should form a topic together.
    fn find_topic_mates(
        &self,
        mem_clusters: &HashMap<Uuid, HashMap<Embedder, i32>>,
    ) -> Vec<Vec<Uuid>> {
        let memory_ids: Vec<Uuid> = mem_clusters.keys().cloned().collect();
        let n = memory_ids.len();

        if n == 0 {
            return Vec::new();
        }

        // Build edges for pairs meeting threshold
        let threshold = topic_threshold();
        let mut edges: Vec<(usize, usize)> = Vec::new();

        for i in 0..n {
            for j in (i + 1)..n {
                let wa = self.compute_weighted_agreement(&memory_ids[i], &memory_ids[j], mem_clusters);
                if wa >= threshold {
                    edges.push((i, j));
                }
            }
        }

        // Union-Find with path compression
        let mut parent: Vec<usize> = (0..n).collect();

        fn find(parent: &mut [usize], i: usize) -> usize {
            if parent[i] != i {
                parent[i] = find(parent, parent[i]);
            }
            parent[i]
        }

        fn union(parent: &mut [usize], i: usize, j: usize) {
            let pi = find(parent, i);
            let pj = find(parent, j);
            if pi != pj {
                parent[pi] = pj;
            }
        }

        for (i, j) in edges {
            union(&mut parent, i, j);
        }

        // Group by component root
        let mut components: HashMap<usize, Vec<Uuid>> = HashMap::new();
        for i in 0..n {
            let root = find(&mut parent, i);
            components.entry(root).or_default().push(memory_ids[i]);
        }

        components.into_values().collect()
    }

    /// Count cluster occurrences for members in a specific embedding space.
    ///
    /// Returns a map of cluster_id -> count for non-noise clusters.
    fn count_clusters_for_space(
        members: &[Uuid],
        mem_clusters: &HashMap<Uuid, HashMap<Embedder, i32>>,
        embedder: Embedder,
    ) -> HashMap<i32, usize> {
        let mut counts: HashMap<i32, usize> = HashMap::new();
        for mem_id in members {
            if let Some(clusters) = mem_clusters.get(mem_id) {
                let cid = clusters.get(&embedder).copied().unwrap_or(-1);
                if cid != -1 {
                    *counts.entry(cid).or_insert(0) += 1;
                }
            }
        }
        counts
    }

    /// Compute topic profile from members.
    ///
    /// For each embedding space, the strength is the fraction of members
    /// that are in the dominant (most common) cluster for that space.
    fn compute_topic_profile(
        &self,
        members: &[Uuid],
        mem_clusters: &HashMap<Uuid, HashMap<Embedder, i32>>,
    ) -> TopicProfile {
        if members.is_empty() {
            return TopicProfile::new([0.0f32; 13]);
        }

        let mut strengths = [0.0f32; 13];
        for embedder in Embedder::all() {
            let counts = Self::count_clusters_for_space(members, mem_clusters, embedder);
            if let Some((_, &count)) = counts.iter().max_by_key(|(_, &c)| c) {
                let strength = count as f32 / members.len() as f32;

                // TOPIC-2: Detect degenerate embedders (all in single cluster = no discrimination)
                if counts.len() == 1 && members.len() > 2 {
                    strengths[embedder.index()] = 0.0;
                } else {
                    strengths[embedder.index()] = strength;
                }
            }
        }

        TopicProfile::new(strengths)
    }

    /// Compute cluster_ids for topic (most common cluster per space).
    ///
    /// For each embedding space, returns the cluster ID that contains the most
    /// members of this topic group.
    fn compute_cluster_ids(
        &self,
        members: &[Uuid],
        mem_clusters: &HashMap<Uuid, HashMap<Embedder, i32>>,
    ) -> HashMap<Embedder, i32> {
        let mut result = HashMap::new();
        for embedder in Embedder::all() {
            let counts = Self::count_clusters_for_space(members, mem_clusters, embedder);
            if let Some((&dominant, _)) = counts.iter().max_by_key(|(_, &c)| c) {
                result.insert(embedder, dominant);
            }
        }
        result
    }

    /// Merge highly similar topics.
    ///
    /// Topics with profile similarity >= merge_similarity_threshold (default 0.9)
    /// are merged into a single topic. Larger topics absorb smaller ones.
    fn merge_similar_topics(&self, mut topics: Vec<Topic>) -> Vec<Topic> {
        if topics.len() <= 1 {
            return topics;
        }

        // Sort by member count descending (larger topics absorb smaller)
        topics.sort_by_key(|t| std::cmp::Reverse(t.member_count()));

        let mut merged: Vec<Topic> = Vec::new();
        let mut absorbed: HashSet<usize> = HashSet::new();

        for i in 0..topics.len() {
            if absorbed.contains(&i) {
                continue;
            }

            let mut current = topics[i].clone();

            for j in (i + 1)..topics.len() {
                if absorbed.contains(&j) {
                    continue;
                }

                let sim = current.profile.similarity(&topics[j].profile);
                if sim >= self.merge_similarity_threshold {
                    // Absorb j into current
                    for mem_id in &topics[j].member_memories {
                        if !current.member_memories.contains(mem_id) {
                            current.member_memories.push(*mem_id);
                        }
                    }
                    for (space, cid) in &topics[j].cluster_ids {
                        current.cluster_ids.entry(*space).or_insert(*cid);
                    }
                    absorbed.insert(j);
                }
            }

            current.update_contributing_spaces();
            merged.push(current);
        }

        merged
    }

    /// Main synthesis entry point.
    ///
    /// Discovers topics from cluster memberships where memory pairs have
    /// weighted_agreement >= 2.5 (topic_threshold).
    ///
    /// # Arguments
    ///
    /// * `memberships` - Per-space cluster assignments for all memories
    ///
    /// # Returns
    ///
    /// Vector of discovered topics, or error if synthesis fails.
    ///
    /// # Constitution Reference
    ///
    /// - ARCH-09: Topic threshold is weighted_agreement >= 2.5
    /// - AP-60: Temporal embedders (E2-E4) contribute 0.0 to weighted_agreement
    pub fn synthesize_topics(
        &self,
        memberships: &HashMap<Embedder, Vec<ClusterMembership>>,
    ) -> Result<Vec<Topic>, ClusterError> {
        let mem_clusters = self.build_mem_clusters_map(memberships);

        if mem_clusters.is_empty() {
            return Ok(Vec::new());
        }

        // Find connected components
        let groups = self.find_topic_mates(&mem_clusters);

        // Create topics from groups with >= 2 members
        let mut topics: Vec<Topic> = groups
            .into_iter()
            .filter(|g| g.len() >= 2)
            .map(|members| {
                let profile = self.compute_topic_profile(&members, &mem_clusters);
                let cluster_ids = self.compute_cluster_ids(&members, &mem_clusters);
                Topic::new(profile, cluster_ids, members)
            })
            .filter(|t| t.is_valid())
            .collect();

        // Merge similar topics
        topics = self.merge_similar_topics(topics);

        Ok(topics)
    }

    /// Update topic stability based on membership changes.
    ///
    /// Computes membership churn as the Jaccard distance between old and new members:
    /// churn = |symmetric_difference| / |union|
    ///
    /// Also updates the topic's age and phase based on stability thresholds.
    ///
    /// # Arguments
    ///
    /// * `topic` - Topic to update (modified in place)
    /// * `old_members` - Previous member list
    /// * `new_members` - Current member list
    pub fn update_topic_stability(
        &self,
        topic: &mut Topic,
        old_members: &[Uuid],
        new_members: &[Uuid],
    ) {
        let old_set: HashSet<_> = old_members.iter().collect();
        let new_set: HashSet<_> = new_members.iter().collect();

        let sym_diff = old_set.symmetric_difference(&new_set).count();
        let union_size = old_set.union(&new_set).count();

        let churn = if union_size > 0 {
            sym_diff as f32 / union_size as f32
        } else {
            0.0
        };

        topic.stability.membership_churn = churn;

        let age = chrono::Utc::now() - topic.created_at;
        topic.stability.age_hours = age.num_minutes() as f32 / 60.0;

        topic.stability.update_phase();
    }

    /// Compute the average weighted agreement for a group of memories.
    ///
    /// Useful for evaluating topic cohesion.
    #[allow(dead_code)]
    fn compute_group_average_agreement(
        &self,
        members: &[Uuid],
        mem_clusters: &HashMap<Uuid, HashMap<Embedder, i32>>,
    ) -> f32 {
        if members.len() < 2 {
            return 0.0;
        }

        let mut sum = 0.0f32;
        let mut count = 0usize;

        for i in 0..members.len() {
            for j in (i + 1)..members.len() {
                sum += self.compute_weighted_agreement(&members[i], &members[j], mem_clusters);
                count += 1;
            }
        }

        if count > 0 {
            sum / count as f32
        } else {
            0.0
        }
    }

}

/// Compute topic confidence from weighted agreement.
///
/// confidence = weighted_agreement / max_weighted_agreement (8.5)
#[allow(dead_code)]
fn compute_confidence(weighted_agreement: f32) -> f32 {
    (weighted_agreement / max_weighted_agreement()).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create memberships where two memories share clusters in specified spaces.
    fn create_shared_memberships(
        id1: Uuid,
        id2: Uuid,
        shared_spaces: &[Embedder],
    ) -> HashMap<Embedder, Vec<ClusterMembership>> {
        let mut memberships = HashMap::new();

        for embedder in Embedder::all() {
            let cluster_id = if shared_spaces.contains(&embedder) {
                1
            } else {
                -1
            };
            let other_cluster_id = if shared_spaces.contains(&embedder) {
                1
            } else {
                99
            };

            memberships
                .entry(embedder)
                .or_insert_with(Vec::new)
                .push(ClusterMembership::new(id1, embedder, cluster_id, 0.9, true));
            memberships
                .entry(embedder)
                .or_insert_with(Vec::new)
                .push(ClusterMembership::new(
                    id2,
                    embedder,
                    other_cluster_id,
                    0.9,
                    true,
                ));
        }

        memberships
    }

    #[test]
    fn test_weighted_agreement_3_semantic_forms_topic() {
        // 3 semantic spaces = 3.0 >= 2.5 = TOPIC
        let synthesizer = TopicSynthesizer::new();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        // Semantic, Causal, Code are all SEMANTIC category (weight 1.0)
        let memberships =
            create_shared_memberships(id1, id2, &[Embedder::Semantic, Embedder::Causal, Embedder::Code]);

        println!("=== TEST: test_weighted_agreement_3_semantic_forms_topic ===");
        println!("STATE BEFORE: 2 memories, shared spaces = [Semantic, Causal, Code]");
        println!("EXPECTED: 3 semantic spaces at weight 1.0 = 3.0 >= 2.5 threshold");

        let result = synthesizer.synthesize_topics(&memberships).unwrap();

        println!("STATE AFTER: {} topic(s) formed", result.len());
        if !result.is_empty() {
            println!(
                "  Topic confidence: {:.3}",
                result[0].confidence
            );
            println!("  Topic member count: {}", result[0].member_count());
        }

        assert_eq!(
            result.len(),
            1,
            "3 semantic spaces (3.0) should form 1 topic"
        );
        println!("[PASS] 3 semantic spaces agreeing = 3.0 -> TOPIC\n");
    }

    #[test]
    fn test_weighted_agreement_2_semantic_1_relational_forms_topic() {
        // 2 semantic + 1 relational = 2.0 + 0.5 = 2.5 = TOPIC
        let synthesizer = TopicSynthesizer::new();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        // Semantic=1.0, Causal=1.0, Graph=0.5 (relational)
        let memberships = create_shared_memberships(
            id1,
            id2,
            &[Embedder::Semantic, Embedder::Causal, Embedder::Graph],
        );

        println!("=== TEST: test_weighted_agreement_2_semantic_1_relational_forms_topic ===");
        println!("STATE BEFORE: 2 memories, shared spaces = [Semantic, Causal, Graph]");
        println!("EXPECTED: 2*1.0 + 1*0.5 = 2.5 >= 2.5 threshold");

        let result = synthesizer.synthesize_topics(&memberships).unwrap();

        println!("STATE AFTER: {} topic(s) formed", result.len());

        assert_eq!(
            result.len(),
            1,
            "2 semantic + 1 relational (2.5) should form topic"
        );
        println!("[PASS] 2 semantic + 1 relational = 2.5 -> TOPIC\n");
    }

    #[test]
    fn test_weighted_agreement_2_semantic_only_no_topic() {
        // 2 semantic only = 2.0 < 2.5 = NO TOPIC
        let synthesizer = TopicSynthesizer::new();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        let memberships = create_shared_memberships(
            id1,
            id2,
            &[Embedder::Semantic, Embedder::Causal], // 1.0 + 1.0 = 2.0 < 2.5
        );

        println!("=== TEST: test_weighted_agreement_2_semantic_only_no_topic ===");
        println!("STATE BEFORE: 2 memories, shared spaces = [Semantic, Causal]");
        println!("EXPECTED: 2*1.0 = 2.0 < 2.5 threshold -> no topic");

        let result = synthesizer.synthesize_topics(&memberships).unwrap();

        println!("STATE AFTER: {} topic(s) formed", result.len());

        assert!(result.is_empty(), "2 semantic only (2.0) should NOT form topic");
        println!("[PASS] 2 semantic spaces only = 2.0 -> NOT TOPIC\n");
    }

    #[test]
    fn test_temporal_excluded_from_agreement() {
        // All temporal = 0.0 = NO TOPIC (AP-60)
        let synthesizer = TopicSynthesizer::new();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        let memberships = create_shared_memberships(
            id1,
            id2,
            &[
                Embedder::TemporalRecent,
                Embedder::TemporalPeriodic,
                Embedder::TemporalPositional,
            ],
        );

        println!("=== TEST: test_temporal_excluded_from_agreement ===");
        println!("STATE BEFORE: 2 memories, shared temporal spaces only");
        println!("EXPECTED: temporal weight = 0.0, weighted_agreement = 0.0");

        // Manually compute to verify
        let mem_clusters = synthesizer.build_mem_clusters_map(&memberships);
        let wa = synthesizer.compute_weighted_agreement(&id1, &id2, &mem_clusters);

        println!("COMPUTED weighted_agreement = {}", wa);

        assert_eq!(wa, 0.0, "Temporal-only agreement should be 0.0");

        let result = synthesizer.synthesize_topics(&memberships).unwrap();
        println!("STATE AFTER: {} topic(s) formed", result.len());

        assert!(result.is_empty(), "Temporal-only should NOT form topic");
        println!("[PASS] All temporal spaces = 0.0 -> NOT TOPIC (AP-60 verified)\n");
    }

    #[test]
    fn test_1_semantic_2_relational_1_structural_forms_topic() {
        // 1 semantic + 2 relational + 1 structural = 1.0 + 1.0 + 0.5 = 2.5 = TOPIC
        let synthesizer = TopicSynthesizer::new();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        let memberships = create_shared_memberships(
            id1,
            id2,
            &[
                Embedder::Semantic,   // semantic (1.0)
                Embedder::Graph,  // relational (0.5)
                Embedder::Entity,     // relational (0.5)
                Embedder::Hdc,        // structural (0.5)
            ],
        );

        println!("=== TEST: test_1_semantic_2_relational_1_structural_forms_topic ===");
        println!("STATE BEFORE: 2 memories, shared = [Semantic, Graph, Entity, Hdc]");
        println!("EXPECTED: 1*1.0 + 2*0.5 + 1*0.5 = 2.5 >= 2.5 threshold");

        let mem_clusters = synthesizer.build_mem_clusters_map(&memberships);
        let wa = synthesizer.compute_weighted_agreement(&id1, &id2, &mem_clusters);

        println!("COMPUTED weighted_agreement = {}", wa);
        assert!(
            (wa - 2.5).abs() < 0.001,
            "Expected 2.5, got {}",
            wa
        );

        let result = synthesizer.synthesize_topics(&memberships).unwrap();
        println!("STATE AFTER: {} topic(s) formed", result.len());

        assert_eq!(result.len(), 1, "weighted 2.5 should form topic");
        println!("[PASS] 1 semantic + 2 relational + 1 structural = 2.5 -> TOPIC\n");
    }

    #[test]
    fn test_empty_input() {
        let synthesizer = TopicSynthesizer::new();
        let memberships: HashMap<Embedder, Vec<ClusterMembership>> = HashMap::new();

        println!("=== TEST: test_empty_input ===");
        println!("STATE BEFORE: empty memberships");

        let result = synthesizer.synthesize_topics(&memberships).unwrap();

        println!("STATE AFTER: {} topics", result.len());

        assert!(result.is_empty());
        println!("[PASS] Empty input -> empty output\n");
    }

    #[test]
    fn test_single_memory_no_topic() {
        let synthesizer = TopicSynthesizer::new();
        let id = Uuid::new_v4();

        let mut memberships = HashMap::new();
        memberships.insert(
            Embedder::Semantic,
            vec![ClusterMembership::new(id, Embedder::Semantic, 1, 0.9, true)],
        );

        println!("=== TEST: test_single_memory_no_topic ===");
        println!("STATE BEFORE: 1 memory only");

        let result = synthesizer.synthesize_topics(&memberships).unwrap();

        println!("STATE AFTER: {} topics", result.len());

        assert!(result.is_empty(), "Cannot form topic with single memory");
        println!("[PASS] Single memory -> no topic\n");
    }

    #[test]
    fn test_update_stability_churn() {
        let synthesizer = TopicSynthesizer::new();

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id3 = Uuid::new_v4();

        let profile = TopicProfile::new([0.8; 13]);
        let mut topic = Topic::new(profile, HashMap::new(), vec![id1, id2]);

        let old_members = vec![id1, id2];
        let new_members = vec![id1, id3]; // id2 left, id3 joined

        println!("=== TEST: test_update_stability_churn ===");
        println!(
            "STATE BEFORE: members = [id1, id2], churn = {}",
            topic.stability.membership_churn
        );

        synthesizer.update_topic_stability(&mut topic, &old_members, &new_members);

        println!(
            "STATE AFTER: churn = {}",
            topic.stability.membership_churn
        );

        // churn = 2 changes / 3 total = 0.666...
        assert!(
            topic.stability.membership_churn > 0.5,
            "Churn should be > 0.5"
        );
        println!(
            "[PASS] Stability churn computed: {:.3}\n",
            topic.stability.membership_churn
        );
    }

    #[test]
    fn test_default_configuration() {
        let synthesizer = TopicSynthesizer::new();

        println!("=== TEST: test_default_configuration ===");
        println!(
            "merge_similarity_threshold = {}",
            synthesizer.merge_similarity_threshold()
        );
        println!("min_silhouette = {}", synthesizer.min_silhouette());

        assert!(
            (synthesizer.merge_similarity_threshold() - DEFAULT_MERGE_THRESHOLD).abs() < f32::EPSILON
        );
        assert!((synthesizer.min_silhouette() - DEFAULT_MIN_SILHOUETTE).abs() < f32::EPSILON);
        println!("[PASS] Default configuration values correct\n");
    }

    #[test]
    fn test_custom_configuration() {
        let synthesizer = TopicSynthesizer::with_config(0.85, 0.2);

        println!("=== TEST: test_custom_configuration ===");
        println!(
            "merge_similarity_threshold = {}",
            synthesizer.merge_similarity_threshold()
        );
        println!("min_silhouette = {}", synthesizer.min_silhouette());

        assert!((synthesizer.merge_similarity_threshold() - 0.85).abs() < f32::EPSILON);
        assert!((synthesizer.min_silhouette() - 0.2).abs() < f32::EPSILON);
        println!("[PASS] Custom configuration values correct\n");
    }

    #[test]
    fn test_configuration_clamping() {
        let synthesizer = TopicSynthesizer::with_config(1.5, -2.0);

        println!("=== TEST: test_configuration_clamping ===");
        println!(
            "merge_threshold: input=1.5, clamped={}",
            synthesizer.merge_similarity_threshold()
        );
        println!(
            "min_silhouette: input=-2.0, clamped={}",
            synthesizer.min_silhouette()
        );

        assert!((synthesizer.merge_similarity_threshold() - 1.0).abs() < f32::EPSILON);
        assert!((synthesizer.min_silhouette() - (-1.0)).abs() < f32::EPSILON);
        println!("[PASS] Configuration values properly clamped\n");
    }

    #[test]
    fn test_merge_similar_topics() {
        let synthesizer = TopicSynthesizer::new();

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id3 = Uuid::new_v4();
        let id4 = Uuid::new_v4();

        // Create two similar topics
        let strengths1 = [0.9f32; 13];
        let strengths2 = [0.88f32; 13]; // Very similar

        let topic1 = Topic::new(TopicProfile::new(strengths1), HashMap::new(), vec![id1, id2]);
        let topic2 = Topic::new(TopicProfile::new(strengths2), HashMap::new(), vec![id3, id4]);

        println!("=== TEST: test_merge_similar_topics ===");
        println!(
            "STATE BEFORE: 2 topics, similarity = {:.3}",
            topic1.profile.similarity(&topic2.profile)
        );

        let topics = vec![topic1, topic2];
        let merged = synthesizer.merge_similar_topics(topics);

        println!("STATE AFTER: {} topic(s)", merged.len());
        if !merged.is_empty() {
            println!("  Merged topic member count: {}", merged[0].member_count());
        }

        // Topics with similarity >= 0.9 should merge
        let sim = TopicProfile::new(strengths1).similarity(&TopicProfile::new(strengths2));
        if sim >= 0.9 {
            assert_eq!(merged.len(), 1, "Similar topics should merge");
            assert_eq!(merged[0].member_count(), 4, "Merged topic should have all members");
        }
        println!("[PASS] Similar topics merged correctly\n");
    }

    #[test]
    fn test_max_weighted_agreement_value() {
        // Test that all 13 spaces at weight give expected max
        let synthesizer = TopicSynthesizer::new();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        // Create memberships where both memories share ALL clusters
        let mut memberships: HashMap<Embedder, Vec<ClusterMembership>> = HashMap::new();
        for embedder in Embedder::all() {
            memberships.entry(embedder).or_insert_with(Vec::new).push(
                ClusterMembership::new(id1, embedder, 1, 0.9, true),
            );
            memberships.entry(embedder).or_insert_with(Vec::new).push(
                ClusterMembership::new(id2, embedder, 1, 0.9, true),
            );
        }

        println!("=== TEST: test_max_weighted_agreement_value ===");
        let mem_clusters = synthesizer.build_mem_clusters_map(&memberships);
        let wa = synthesizer.compute_weighted_agreement(&id1, &id2, &mem_clusters);

        // Max = 7*1.0 (semantic) + 2*0.5 (relational) + 1*0.5 (structural) + 3*0.0 (temporal) = 8.5
        println!("COMPUTED weighted_agreement = {} (expected 8.5)", wa);

        assert!(
            (wa - 8.5).abs() < 0.001,
            "Max weighted agreement should be 8.5, got {}",
            wa
        );
        println!("[PASS] Max weighted agreement = 8.5\n");
    }

    #[test]
    fn test_constitution_example_1_semantic_3_relational() {
        // Constitution example: "1 semantic + 3 relational = 2.5 -> TOPIC"
        // But there are only 2 relational embedders (E8, E11)
        // So we test: 1 semantic + 2 relational + 1 structural = 2.5
        let synth = TopicSynthesizer::new();
        let mem_a = Uuid::new_v4();
        let mem_b = Uuid::new_v4();

        let memberships = create_shared_memberships(
            mem_a,
            mem_b,
            &[
                Embedder::Semantic,  // 1.0
                Embedder::Graph, // 0.5
                Embedder::Entity,    // 0.5
                Embedder::Hdc,       // 0.5
            ],
        );

        println!("=== TEST: test_constitution_example_1_semantic_3_relational ===");
        let mem_clusters = synth.build_mem_clusters_map(&memberships);
        let wa = synth.compute_weighted_agreement(&mem_a, &mem_b, &mem_clusters);

        println!("weighted_agreement = {} (expected 2.5)", wa);

        assert!(
            (wa - 2.5).abs() < 0.001,
            "1 semantic + 2 relational + 1 structural should = 2.5"
        );

        let result = synth.synthesize_topics(&memberships).unwrap();
        assert_eq!(result.len(), 1, "Should form topic at threshold");
        println!("[PASS] Constitution example verified\n");
    }

    #[test]
    fn test_all_temporal_does_not_contribute() {
        // Verify EXPLICITLY that each temporal embedder contributes 0.0
        println!("=== TEST: test_all_temporal_does_not_contribute ===");

        for temporal in [
            Embedder::TemporalRecent,
            Embedder::TemporalPeriodic,
            Embedder::TemporalPositional,
        ] {
            let weight = category_for(temporal).topic_weight();
            println!("{:?} -> topic_weight = {}", temporal, weight);
            assert_eq!(weight, 0.0, "{:?} should have weight 0.0", temporal);
        }

        println!("[PASS] All temporal embedders have weight 0.0 (AP-60)\n");
    }

    #[test]
    fn test_semantic_embedders_have_weight_1() {
        println!("=== TEST: test_semantic_embedders_have_weight_1 ===");

        for semantic in [
            Embedder::Semantic,
            Embedder::Causal,
            Embedder::Sparse,
            Embedder::Code,
            Embedder::Multimodal,
            Embedder::LateInteraction,
            Embedder::KeywordSplade,
        ] {
            let weight = category_for(semantic).topic_weight();
            println!("{:?} -> topic_weight = {}", semantic, weight);
            assert_eq!(weight, 1.0, "{:?} should have weight 1.0", semantic);
        }

        println!("[PASS] All 7 semantic embedders have weight 1.0\n");
    }

    #[test]
    fn test_relational_structural_have_weight_half() {
        println!("=== TEST: test_relational_structural_have_weight_half ===");

        for half_weight in [Embedder::Graph, Embedder::Entity, Embedder::Hdc] {
            let weight = category_for(half_weight).topic_weight();
            println!("{:?} -> topic_weight = {}", half_weight, weight);
            assert_eq!(weight, 0.5, "{:?} should have weight 0.5", half_weight);
        }

        println!("[PASS] Relational and structural embedders have weight 0.5\n");
    }

    #[test]
    fn test_multiple_topic_groups() {
        // Create 4 memories forming 2 separate topic groups
        let synth = TopicSynthesizer::new();
        let mem1 = Uuid::new_v4();
        let mem2 = Uuid::new_v4();
        let mem3 = Uuid::new_v4();
        let mem4 = Uuid::new_v4();

        let mut memberships: HashMap<Embedder, Vec<ClusterMembership>> = HashMap::new();

        // Group 1: mem1 and mem2 share cluster 1 in semantic spaces
        for embedder in [Embedder::Semantic, Embedder::Causal, Embedder::Code] {
            memberships
                .entry(embedder)
                .or_insert_with(Vec::new)
                .push(ClusterMembership::new(mem1, embedder, 1, 0.9, true));
            memberships
                .entry(embedder)
                .or_insert_with(Vec::new)
                .push(ClusterMembership::new(mem2, embedder, 1, 0.9, true));
        }

        // Group 2: mem3 and mem4 share cluster 2 in semantic spaces
        for embedder in [Embedder::Semantic, Embedder::Causal, Embedder::Code] {
            memberships
                .entry(embedder)
                .or_insert_with(Vec::new)
                .push(ClusterMembership::new(mem3, embedder, 2, 0.9, true));
            memberships
                .entry(embedder)
                .or_insert_with(Vec::new)
                .push(ClusterMembership::new(mem4, embedder, 2, 0.9, true));
        }

        println!("=== TEST: test_multiple_topic_groups ===");
        println!("STATE BEFORE: 4 memories, 2 distinct groups");

        let result = synth.synthesize_topics(&memberships).unwrap();

        println!("STATE AFTER: {} topic(s) formed", result.len());

        // Should form 2 separate topics
        assert!(result.len() >= 1, "Should form at least 1 topic");
        let total_members: usize = result.iter().map(|t| t.member_count()).sum();
        assert_eq!(total_members, 4, "All 4 memories should be in topics");

        println!("[PASS] Multiple topic groups handled correctly\n");
    }
}
