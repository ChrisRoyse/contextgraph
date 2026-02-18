//! Topic types for multi-space clustering.
//!
//! Per constitution ARCH-09: Topic threshold is weighted_agreement >= 2.5
//! Per constitution AP-60: Temporal embedders (E2-E4) NEVER count toward topic detection
//! Per constitution clustering.parameters.silhouette_threshold: 0.3
//!
//! # Weighted Agreement Formula
//!
//! ```text
//! weighted_agreement = Sum(topic_weight_i * strength_i)
//! max_weighted_agreement = 8.5
//! topic_confidence = weighted_agreement / 8.5
//! ```
//!
//! Category weights:
//! - SEMANTIC (E1, E5, E6, E7, E10, E12, E13): 1.0
//! - TEMPORAL (E2, E3, E4): 0.0 (NEVER counts)
//! - RELATIONAL (E8, E11): 0.5
//! - STRUCTURAL (E9): 0.5
//!
//! # Silhouette Validation
//!
//! Topics are only valid if their average silhouette score >= 0.3
//! (per constitution clustering.parameters.silhouette_threshold).

/// Minimum silhouette score threshold for valid topics.
/// Per constitution clustering.parameters.silhouette_threshold.
pub const TOPIC_SILHOUETTE_THRESHOLD: f32 = 0.3;

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::embeddings::category::{category_for, max_weighted_agreement, topic_threshold};
use crate::teleological::Embedder;

// =============================================================================
// TopicPhase
// =============================================================================

/// Lifecycle phase of a topic.
///
/// Topics transition through phases based on age and membership churn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum TopicPhase {
    /// Less than 1 hour old, membership changing rapidly (churn > 0.3).
    #[default]
    Emerging,
    /// Consistent membership for 24+ hours, churn < 0.1.
    Stable,
    /// Decreasing access, members leaving (churn > 0.5).
    Declining,
    /// Being absorbed into another topic.
    Merging,
}

impl std::fmt::Display for TopicPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TopicPhase::Emerging => write!(f, "Emerging"),
            TopicPhase::Stable => write!(f, "Stable"),
            TopicPhase::Declining => write!(f, "Declining"),
            TopicPhase::Merging => write!(f, "Merging"),
        }
    }
}

// =============================================================================
// TopicProfile
// =============================================================================

/// Per-space strength profile for a topic.
///
/// Each of the 13 embedding spaces has a strength value (0.0..=1.0)
/// indicating how strongly the topic is represented in that space.
///
/// # Weighted Agreement
///
/// The `weighted_agreement()` method computes the topic score using
/// category weights from the constitution:
/// - SEMANTIC: 1.0 weight
/// - TEMPORAL: 0.0 weight (excluded per AP-60)
/// - RELATIONAL: 0.5 weight
/// - STRUCTURAL: 0.5 weight
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TopicProfile {
    /// Strength in each of 13 embedding spaces (0.0..=1.0).
    pub strengths: [f32; 13],
}

impl Default for TopicProfile {
    fn default() -> Self {
        Self { strengths: [0.0; 13] }
    }
}

impl TopicProfile {
    /// Create a new topic profile with clamped strengths.
    ///
    /// All strength values are clamped to the range 0.0..=1.0.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_core::clustering::TopicProfile;
    ///
    /// let profile = TopicProfile::new([1.5, -0.5, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    /// assert_eq!(profile.strengths[0], 1.0); // clamped from 1.5
    /// assert_eq!(profile.strengths[1], 0.0); // clamped from -0.5
    /// ```
    pub fn new(strengths: [f32; 13]) -> Self {
        let mut clamped = [0.0f32; 13];
        for (i, &s) in strengths.iter().enumerate() {
            clamped[i] = s.clamp(0.0, 1.0);
        }
        Self { strengths: clamped }
    }

    /// Get strength for a specific embedder.
    #[inline]
    pub fn strength(&self, embedder: Embedder) -> f32 {
        self.strengths[embedder.index()]
    }

    /// Set strength for a specific embedder (clamped to 0.0..=1.0).
    pub fn set_strength(&mut self, embedder: Embedder, strength: f32) {
        self.strengths[embedder.index()] = strength.clamp(0.0, 1.0);
    }

    /// Get spaces where this topic is dominant (strength > 0.5).
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_core::clustering::TopicProfile;
    /// use context_graph_core::teleological::Embedder;
    ///
    /// let mut strengths = [0.0; 13];
    /// strengths[0] = 0.8; // Semantic
    /// strengths[4] = 0.7; // Causal
    /// let profile = TopicProfile::new(strengths);
    ///
    /// let dominant = profile.dominant_spaces();
    /// assert!(dominant.contains(&Embedder::Semantic));
    /// assert!(dominant.contains(&Embedder::Causal));
    /// ```
    /// Returns embedders that are dominant AND contribute to topic detection.
    ///
    /// Per ARCH-04 and AP-60: Temporal embedders (E2-E4) are EXCLUDED
    /// because they have topic_weight = 0.0. They should NEVER appear
    /// in contributing_spaces, regardless of their strength.
    ///
    /// Only returns embedders where:
    /// - strength > 0.5 (dominant signal)
    /// - topic_weight > 0.0 (contributes to topics)
    pub fn dominant_spaces(&self) -> Vec<Embedder> {
        Embedder::all()
            .filter(|e| {
                let strength = self.strength(*e);
                let topic_weight = category_for(*e).topic_weight();
                // Must have both: strong signal AND non-zero topic weight
                // This excludes temporal embedders (E2-E4) per AP-60
                strength > 0.5 && topic_weight > 0.0
            })
            .collect()
    }

    /// Compute weighted agreement per ARCH-09.
    ///
    /// Uses EmbedderCategory::topic_weight() for each space:
    /// - SEMANTIC (E1, E5, E6, E7, E10, E12, E13): 1.0 weight
    /// - TEMPORAL (E2, E3, E4): 0.0 weight (NEVER counts per AP-60)
    /// - RELATIONAL (E8, E11): 0.5 weight
    /// - STRUCTURAL (E9): 0.5 weight
    ///
    /// # Returns
    ///
    /// Sum of (strength_i * topic_weight_i) for all spaces, clamped to max = 8.5
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_core::clustering::TopicProfile;
    /// use context_graph_core::teleological::Embedder;
    ///
    /// // 3 semantic spaces at strength 1.0 = weighted_agreement 3.0
    /// let mut strengths = [0.0; 13];
    /// strengths[Embedder::Semantic.index()] = 1.0;
    /// strengths[Embedder::Causal.index()] = 1.0;
    /// strengths[Embedder::Code.index()] = 1.0;
    ///
    /// let profile = TopicProfile::new(strengths);
    /// let weighted = profile.weighted_agreement();
    /// assert!((weighted - 3.0).abs() < 0.001);
    /// ```
    pub fn weighted_agreement(&self) -> f32 {
        let mut sum = 0.0f32;
        for embedder in Embedder::all() {
            let strength = self.strength(embedder);
            let category = category_for(embedder);
            let weight = category.topic_weight();
            sum += strength * weight;
        }
        // Clamp to valid range and handle NaN (AP-10)
        if sum.is_nan() || sum.is_infinite() {
            0.0
        } else {
            sum.clamp(0.0, max_weighted_agreement())
        }
    }

    /// Check if this profile meets the topic threshold.
    ///
    /// Per ARCH-09: weighted_agreement >= 2.5
    #[inline]
    pub fn is_topic(&self) -> bool {
        self.weighted_agreement() >= topic_threshold()
    }

    /// Compute cosine similarity with another profile.
    ///
    /// Handles zero vectors gracefully (returns 0.0).
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_core::clustering::TopicProfile;
    ///
    /// let p1 = TopicProfile::new([1.0, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    /// let p2 = TopicProfile::new([1.0, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    ///
    /// let sim = p1.similarity(&p2);
    /// assert!((sim - 1.0).abs() < 0.001); // identical profiles
    /// ```
    pub fn similarity(&self, other: &TopicProfile) -> f32 {
        let mut dot = 0.0f32;
        let mut norm_a = 0.0f32;
        let mut norm_b = 0.0f32;

        for i in 0..13 {
            dot += self.strengths[i] * other.strengths[i];
            norm_a += self.strengths[i] * self.strengths[i];
            norm_b += other.strengths[i] * other.strengths[i];
        }

        let norm = (norm_a.sqrt() * norm_b.sqrt()).max(1e-10);
        let result = dot / norm;

        // Handle NaN/Infinity (AP-10)
        if result.is_nan() || result.is_infinite() {
            0.0
        } else {
            result.clamp(0.0, 1.0)
        }
    }

    /// Count spaces with non-zero strength (> 0.1 threshold).
    pub fn active_space_count(&self) -> usize {
        self.strengths.iter().filter(|&&s| s > 0.1).count()
    }
}

// =============================================================================
// TopicStability
// =============================================================================

/// Stability metrics for a topic.
///
/// Tracks the lifecycle state and health indicators for a topic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicStability {
    /// Current lifecycle phase.
    pub phase: TopicPhase,
    /// Age in hours since creation.
    pub age_hours: f32,
    /// Membership churn rate (0.0..=1.0) - how often members change.
    pub membership_churn: f32,
    /// Centroid drift since last snapshot (0.0..=1.0).
    pub centroid_drift: f32,
    /// Total access count.
    pub access_count: u32,
    /// Last access time.
    pub last_accessed: Option<DateTime<Utc>>,
}

impl Default for TopicStability {
    fn default() -> Self {
        Self {
            phase: TopicPhase::Emerging,
            age_hours: 0.0,
            membership_churn: 0.0,
            centroid_drift: 0.0,
            access_count: 0,
            last_accessed: None,
        }
    }
}

impl TopicStability {
    /// Create new stability metrics in Emerging phase.
    pub fn new() -> Self {
        Self::default()
    }

    /// Update the phase based on current metrics.
    ///
    /// Phase transition rules:
    /// - Emerging: age < 1hr AND churn > 0.3
    /// - Stable: age >= 24hr AND churn < 0.1
    /// - Declining: churn >= 0.5
    /// - Merging: set externally when merged
    pub fn update_phase(&mut self) {
        self.phase = if self.age_hours < 1.0 && self.membership_churn > 0.3 {
            TopicPhase::Emerging
        } else if self.membership_churn < 0.1 && self.age_hours >= 24.0 {
            TopicPhase::Stable
        } else if self.membership_churn >= 0.5 {
            TopicPhase::Declining
        } else {
            self.phase // Keep current if no transition triggers
        };
    }

    /// Check if topic is in stable phase.
    #[inline]
    pub fn is_stable(&self) -> bool {
        self.phase == TopicPhase::Stable
    }

    /// Check if topic is healthy (churn < 0.3 per constitution).
    #[inline]
    pub fn is_healthy(&self) -> bool {
        self.membership_churn < 0.3
    }
}

// =============================================================================
// Topic
// =============================================================================

/// A topic that emerges from cross-space clustering.
///
/// Topics are discovered when memories cluster together in multiple
/// embedding spaces with sufficient weighted agreement (>= 2.5).
///
/// # Constitution Reference
///
/// - ARCH-09: Topic threshold is weighted_agreement >= 2.5
/// - AP-60: Temporal embedders (E2-E4) NEVER count toward topic detection
/// - confidence = weighted_agreement / MAX_WEIGHTED_AGREEMENT (8.5)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Topic {
    /// Unique identifier.
    pub id: Uuid,
    /// Optional human-readable name (auto-generated or user-provided).
    pub name: Option<String>,
    /// Per-space strength profile.
    pub profile: TopicProfile,
    /// Spaces where this topic has strong representation (strength > 0.5).
    pub contributing_spaces: Vec<Embedder>,
    /// Cluster ID in each contributing space.
    pub cluster_ids: HashMap<Embedder, i32>,
    /// Memory IDs that belong to this topic.
    pub member_memories: Vec<Uuid>,
    /// Confidence score = weighted_agreement / 8.5 (per ARCH-09).
    pub confidence: f32,
    /// Average silhouette score from contributing clusters.
    /// Per constitution clustering.parameters.silhouette_threshold: 0.3
    /// Default is 1.0 (valid) if not computed.
    pub silhouette_score: f32,
    /// Stability metrics.
    pub stability: TopicStability,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

impl Topic {
    /// Create a new topic from profile and cluster assignments.
    ///
    /// Confidence is computed as weighted_agreement / MAX_WEIGHTED_AGREEMENT (8.5)
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_core::clustering::{Topic, TopicProfile};
    /// use context_graph_core::teleological::Embedder;
    /// use std::collections::HashMap;
    ///
    /// let mut strengths = [0.0; 13];
    /// strengths[Embedder::Semantic.index()] = 1.0;
    /// strengths[Embedder::Causal.index()] = 1.0;
    /// strengths[Embedder::Code.index()] = 1.0;
    ///
    /// let profile = TopicProfile::new(strengths);
    /// let topic = Topic::new(profile, HashMap::new(), vec![]);
    ///
    /// assert!(topic.is_valid()); // weighted 3.0 >= 2.5
    /// ```
    pub fn new(
        profile: TopicProfile,
        cluster_ids: HashMap<Embedder, i32>,
        members: Vec<Uuid>,
    ) -> Self {
        let contributing_spaces = profile.dominant_spaces();
        let weighted = profile.weighted_agreement();
        let confidence = (weighted / max_weighted_agreement()).clamp(0.0, 1.0);

        // Deterministic topic ID from sorted member UUIDs.
        // Same members always produce the same topic ID, which is essential
        // for accurate churn tracking (topic stability across reclusters).
        let id = Self::deterministic_id(&members);

        // TOPIC-1: Auto-generate name from dominant contributing spaces.
        // Format: "Semantic+Code+Entity (5 memories)" — human-readable at a glance.
        let name = Self::generate_name(&contributing_spaces, members.len());

        Self {
            id,
            name: Some(name),
            profile,
            contributing_spaces,
            cluster_ids,
            member_memories: members,
            confidence,
            silhouette_score: 1.0, // Default to valid, updated when cluster silhouettes are computed
            stability: TopicStability::new(),
            created_at: Utc::now(),
        }
    }

    /// Compute confidence based on weighted agreement.
    ///
    /// confidence = weighted_agreement / 8.5
    pub fn compute_confidence(&self) -> f32 {
        let weighted = self.profile.weighted_agreement();
        (weighted / max_weighted_agreement()).clamp(0.0, 1.0)
    }

    /// Record an access to this topic.
    pub fn record_access(&mut self) {
        self.stability.access_count = self.stability.access_count.saturating_add(1);
        self.stability.last_accessed = Some(Utc::now());
    }

    /// Generate a human-readable name from contributing spaces and member count.
    ///
    /// Format: "Semantic+Code+Entity (5 memories)"
    /// Falls back to "Topic (N memories)" if no contributing spaces.
    fn generate_name(contributing_spaces: &[Embedder], member_count: usize) -> String {
        if contributing_spaces.is_empty() {
            return format!("Topic ({member_count} memories)");
        }

        // Map each embedder to its human-readable label
        let labels: Vec<&str> = contributing_spaces
            .iter()
            .map(|e| match *e {
                Embedder::Semantic => "Semantic",
                Embedder::Causal => "Causal",
                Embedder::Sparse => "Keyword",
                Embedder::Code => "Code",
                Embedder::Graph => "Graph",
                Embedder::Hdc => "Robust",
                Embedder::Multimodal => "Paraphrase",
                Embedder::Entity => "Entity",
                Embedder::LateInteraction => "Precision",
                Embedder::KeywordSplade => "SPLADE",
                // Temporal embedders should never be contributing (AP-60)
                _ => "Other",
            })
            .collect();

        let spaces = labels.join("+");
        format!("{spaces} ({member_count} memories)")
    }

    /// Set the topic name.
    pub fn set_name(&mut self, name: String) {
        self.name = Some(name);
    }

    /// Check if this topic is valid.
    ///
    /// Per constitution requirements:
    /// - ARCH-09: weighted_agreement >= 2.5
    /// - clustering.parameters.silhouette_threshold: silhouette >= 0.3
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.profile.is_topic() && self.silhouette_score >= TOPIC_SILHOUETTE_THRESHOLD
    }

    /// Check if topic meets weighted_agreement threshold only.
    ///
    /// Use this for initial topic candidate detection before silhouette is computed.
    #[inline]
    pub fn meets_weighted_agreement(&self) -> bool {
        self.profile.is_topic()
    }

    /// Update the silhouette score from contributing cluster quality.
    ///
    /// Call after HDBSCAN reclustering to set the average silhouette of
    /// clusters that contribute to this topic.
    ///
    /// # Arguments
    /// * `score` - Average silhouette score from contributing clusters (-1.0..=1.0)
    pub fn set_silhouette_score(&mut self, score: f32) {
        self.silhouette_score = score.clamp(-1.0, 1.0);
    }

    /// Check if silhouette score meets threshold.
    #[inline]
    pub fn has_valid_silhouette(&self) -> bool {
        self.silhouette_score >= TOPIC_SILHOUETTE_THRESHOLD
    }

    /// Get member count.
    #[inline]
    pub fn member_count(&self) -> usize {
        self.member_memories.len()
    }

    /// Check if a memory belongs to this topic.
    pub fn contains_memory(&self, memory_id: &Uuid) -> bool {
        self.member_memories.contains(memory_id)
    }

    /// Update contributing spaces from profile (call after modifying profile).
    pub fn update_contributing_spaces(&mut self) {
        self.contributing_spaces = self.profile.dominant_spaces();
        self.confidence = self.compute_confidence();
    }

    /// Compute a deterministic topic ID from member UUIDs.
    ///
    /// Same set of members always produces the same topic ID regardless of
    /// input order. This is essential for accurate churn tracking — without
    /// deterministic IDs, every recluster produces "new" topics even when
    /// the membership hasn't changed, causing churn to always read 1.0.
    fn deterministic_id(members: &[Uuid]) -> Uuid {
        if members.is_empty() {
            return Uuid::nil();
        }
        let mut sorted: Vec<Uuid> = members.to_vec();
        sorted.sort();
        let mut bytes = Vec::with_capacity(sorted.len() * 16);
        for id in &sorted {
            bytes.extend_from_slice(id.as_bytes());
        }
        Uuid::new_v5(&Uuid::NAMESPACE_OID, &bytes)
    }
}

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ===== TopicProfile Tests =====

    #[test]
    fn test_profile_strength_clamping() {
        let profile = TopicProfile::new([
            1.5, -0.5, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        ]);
        assert_eq!(profile.strengths[0], 1.0, "1.5 should clamp to 1.0");
        assert_eq!(profile.strengths[1], 0.0, "-0.5 should clamp to 0.0");
        assert_eq!(profile.strengths[2], 0.5, "0.5 should stay 0.5");
        println!("[PASS] test_profile_strength_clamping");
    }

    #[test]
    fn test_weighted_agreement_semantic_only() {
        // E1 (Semantic, weight=1.0), E5 (Causal/Semantic, weight=1.0), E7 (Code/Semantic, weight=1.0)
        let mut strengths = [0.0; 13];
        strengths[Embedder::Semantic.index()] = 1.0; // E1
        strengths[Embedder::Causal.index()] = 1.0; // E5
        strengths[Embedder::Code.index()] = 1.0; // E7

        let profile = TopicProfile::new(strengths);
        let weighted = profile.weighted_agreement();

        assert!(
            (weighted - 3.0).abs() < 0.001,
            "3 semantic spaces at strength 1.0 should give weighted_agreement = 3.0, got {}",
            weighted
        );
        assert!(
            profile.is_topic(),
            "weighted_agreement 3.0 >= 2.5 threshold should be topic"
        );
        println!(
            "[PASS] test_weighted_agreement_semantic_only - weighted={}",
            weighted
        );
    }

    #[test]
    fn test_weighted_agreement_temporal_excluded() {
        // CRITICAL TEST: Temporal spaces (E2, E3, E4) should contribute 0.0 per AP-60
        let mut strengths = [0.0; 13];
        strengths[Embedder::TemporalRecent.index()] = 1.0; // E2 - temporal
        strengths[Embedder::TemporalPeriodic.index()] = 1.0; // E3 - temporal
        strengths[Embedder::TemporalPositional.index()] = 1.0; // E4 - temporal

        let profile = TopicProfile::new(strengths);
        let weighted = profile.weighted_agreement();

        assert!(
            weighted.abs() < 0.001,
            "3 temporal spaces should give weighted_agreement = 0.0 per AP-60, got {}",
            weighted
        );
        assert!(!profile.is_topic(), "temporal-only should NOT be topic");
        println!(
            "[PASS] test_weighted_agreement_temporal_excluded - temporal contributes 0.0"
        );
    }

    #[test]
    fn test_weighted_agreement_mixed_categories() {
        // 2 semantic (2.0) + 1 relational (0.5) = 2.5 -> exactly threshold
        let mut strengths = [0.0; 13];
        strengths[Embedder::Semantic.index()] = 1.0; // E1 - semantic (1.0)
        strengths[Embedder::Causal.index()] = 1.0; // E5 - semantic (1.0)
        strengths[Embedder::Entity.index()] = 1.0; // E11 - relational (0.5)

        let profile = TopicProfile::new(strengths);
        let weighted = profile.weighted_agreement();

        assert!(
            (weighted - 2.5).abs() < 0.001,
            "2 semantic + 1 relational should give 2.5, got {}",
            weighted
        );
        assert!(profile.is_topic(), "weighted_agreement 2.5 meets threshold");
        println!(
            "[PASS] test_weighted_agreement_mixed_categories - weighted={}",
            weighted
        );
    }

    #[test]
    fn test_weighted_agreement_below_threshold() {
        // 2 semantic only = 2.0 < 2.5 threshold
        let mut strengths = [0.0; 13];
        strengths[Embedder::Semantic.index()] = 1.0; // E1
        strengths[Embedder::Causal.index()] = 1.0; // E5

        let profile = TopicProfile::new(strengths);
        let weighted = profile.weighted_agreement();

        assert!(
            (weighted - 2.0).abs() < 0.001,
            "2 semantic should give 2.0, got {}",
            weighted
        );
        assert!(
            !profile.is_topic(),
            "weighted_agreement 2.0 < 2.5 threshold"
        );
        println!(
            "[PASS] test_weighted_agreement_below_threshold - weighted={}",
            weighted
        );
    }

    #[test]
    fn test_weighted_agreement_max_value() {
        // All spaces at 1.0 - max possible
        let profile = TopicProfile::new([1.0; 13]);
        let weighted = profile.weighted_agreement();

        // Max = 7*1.0 (semantic) + 2*0.5 (relational) + 1*0.5 (structural) + 3*0.0 (temporal) = 8.5
        assert!(
            (weighted - 8.5).abs() < 0.001,
            "All spaces at 1.0 should give max 8.5, got {}",
            weighted
        );
        println!(
            "[PASS] test_weighted_agreement_max_value - weighted={}",
            weighted
        );
    }

    #[test]
    fn test_dominant_spaces() {
        let mut strengths = [0.0; 13];
        strengths[Embedder::Semantic.index()] = 0.8;
        strengths[Embedder::Causal.index()] = 0.7;
        strengths[Embedder::Code.index()] = 0.9;
        strengths[Embedder::Entity.index()] = 0.3; // Below 0.5, should not be dominant

        let profile = TopicProfile::new(strengths);
        let dominant = profile.dominant_spaces();

        assert_eq!(dominant.len(), 3, "Should have 3 dominant spaces (> 0.5)");
        assert!(dominant.contains(&Embedder::Semantic));
        assert!(dominant.contains(&Embedder::Causal));
        assert!(dominant.contains(&Embedder::Code));
        assert!(
            !dominant.contains(&Embedder::Entity),
            "0.3 should not be dominant"
        );
        println!(
            "[PASS] test_dominant_spaces - found {} dominant spaces",
            dominant.len()
        );
    }

    #[test]
    fn test_profile_similarity_identical() {
        let p1 = TopicProfile::new([
            1.0, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        ]);
        let p2 = TopicProfile::new([
            1.0, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        ]);

        let sim = p1.similarity(&p2);
        assert!(
            (sim - 1.0).abs() < 0.001,
            "Identical profiles should have similarity 1.0, got {}",
            sim
        );
        println!("[PASS] test_profile_similarity_identical - sim={}", sim);
    }

    #[test]
    fn test_profile_similarity_orthogonal() {
        let p1 = TopicProfile::new([
            1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        ]);
        let p2 = TopicProfile::new([
            0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        ]);

        let sim = p1.similarity(&p2);
        assert!(
            sim < 0.001,
            "Orthogonal profiles should have similarity ~0.0, got {}",
            sim
        );
        println!("[PASS] test_profile_similarity_orthogonal - sim={}", sim);
    }

    #[test]
    fn test_profile_similarity_zero_vector() {
        let p1 = TopicProfile::new([0.0; 13]);
        let p2 = TopicProfile::new([
            1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        ]);

        let sim = p1.similarity(&p2);
        assert!(!sim.is_nan(), "Zero vector similarity should not be NaN");
        assert!(
            (0.0..=1.0).contains(&sim),
            "Similarity should be in valid range"
        );
        println!(
            "[PASS] test_profile_similarity_zero_vector - handles zero vector gracefully"
        );
    }

    #[test]
    fn test_active_space_count() {
        let mut strengths = [0.0; 13];
        strengths[0] = 0.5;
        strengths[1] = 0.2; // Above 0.1 threshold
        strengths[2] = 0.05; // Below 0.1 threshold

        let profile = TopicProfile::new(strengths);
        let count = profile.active_space_count();

        assert_eq!(count, 2, "Should have 2 active spaces (>0.1 threshold)");
        println!("[PASS] test_active_space_count - count={}", count);
    }

    // ===== Topic Tests =====

    #[test]
    fn test_topic_confidence_calculation() {
        // 3 semantic spaces at 1.0 = weighted 3.0
        // confidence = 3.0 / 8.5 ≈ 0.353
        let mut strengths = [0.0; 13];
        strengths[Embedder::Semantic.index()] = 1.0;
        strengths[Embedder::Causal.index()] = 1.0;
        strengths[Embedder::Code.index()] = 1.0;

        let profile = TopicProfile::new(strengths);
        let topic = Topic::new(profile, HashMap::new(), vec![]);

        let expected_confidence = 3.0 / 8.5;
        assert!(
            (topic.confidence - expected_confidence).abs() < 0.001,
            "confidence should be weighted/8.5 = {}, got {}",
            expected_confidence,
            topic.confidence
        );
        println!(
            "[PASS] test_topic_confidence_calculation - confidence={}",
            topic.confidence
        );
    }

    #[test]
    fn test_topic_validity_weighted_threshold() {
        // Valid: 3 semantic = 3.0 >= 2.5
        let mut valid_strengths = [0.0; 13];
        valid_strengths[Embedder::Semantic.index()] = 1.0;
        valid_strengths[Embedder::Causal.index()] = 1.0;
        valid_strengths[Embedder::Code.index()] = 1.0;

        let valid_topic = Topic::new(TopicProfile::new(valid_strengths), HashMap::new(), vec![]);
        assert!(
            valid_topic.is_valid(),
            "3 semantic spaces (weighted=3.0) should be valid"
        );

        // Invalid: 2 semantic = 2.0 < 2.5
        let mut invalid_strengths = [0.0; 13];
        invalid_strengths[Embedder::Semantic.index()] = 1.0;
        invalid_strengths[Embedder::Causal.index()] = 1.0;

        let invalid_topic =
            Topic::new(TopicProfile::new(invalid_strengths), HashMap::new(), vec![]);
        assert!(
            !invalid_topic.is_valid(),
            "2 semantic spaces (weighted=2.0) should NOT be valid"
        );

        println!("[PASS] test_topic_validity_weighted_threshold");
    }

    #[test]
    fn test_topic_validity_temporal_ignored() {
        // CRITICAL: 5 temporal spaces should NOT make a valid topic
        let mut temporal_strengths = [0.0; 13];
        temporal_strengths[Embedder::TemporalRecent.index()] = 1.0;
        temporal_strengths[Embedder::TemporalPeriodic.index()] = 1.0;
        temporal_strengths[Embedder::TemporalPositional.index()] = 1.0;
        // Adding more temporal won't help - they're weight 0.0

        let temporal_topic =
            Topic::new(TopicProfile::new(temporal_strengths), HashMap::new(), vec![]);
        assert!(
            !temporal_topic.is_valid(),
            "Temporal-only topic (weighted=0.0) must NOT be valid per AP-60"
        );
        println!(
            "[PASS] test_topic_validity_temporal_ignored - temporal excluded from topic detection"
        );
    }

    #[test]
    fn test_topic_record_access() {
        let profile = TopicProfile::default();
        let mut topic = Topic::new(profile, HashMap::new(), vec![]);

        assert_eq!(topic.stability.access_count, 0);
        assert!(topic.stability.last_accessed.is_none());

        topic.record_access();

        assert_eq!(topic.stability.access_count, 1);
        assert!(topic.stability.last_accessed.is_some());
        println!("[PASS] test_topic_record_access");
    }

    #[test]
    fn test_topic_member_operations() {
        let profile = TopicProfile::default();
        let mem1 = Uuid::new_v4();
        let mem2 = Uuid::new_v4();
        let mem3 = Uuid::new_v4();

        let topic = Topic::new(profile, HashMap::new(), vec![mem1, mem2]);

        assert_eq!(topic.member_count(), 2);
        assert!(topic.contains_memory(&mem1));
        assert!(topic.contains_memory(&mem2));
        assert!(!topic.contains_memory(&mem3));
        println!("[PASS] test_topic_member_operations");
    }

    #[test]
    fn test_topic_update_contributing_spaces() {
        let mut strengths = [0.0; 13];
        strengths[Embedder::Semantic.index()] = 0.9;

        let profile = TopicProfile::new(strengths);
        let mut topic = Topic::new(profile, HashMap::new(), vec![]);

        assert_eq!(topic.contributing_spaces.len(), 1);
        assert!(topic.contributing_spaces.contains(&Embedder::Semantic));

        // Modify profile and update
        topic.profile.set_strength(Embedder::Causal, 0.8);
        topic.update_contributing_spaces();

        assert_eq!(topic.contributing_spaces.len(), 2);
        assert!(topic.contributing_spaces.contains(&Embedder::Semantic));
        assert!(topic.contributing_spaces.contains(&Embedder::Causal));
        println!("[PASS] test_topic_update_contributing_spaces");
    }

    // ===== TopicStability Tests =====

    #[test]
    fn test_stability_phase_transitions() {
        let mut stability = TopicStability::new();
        assert_eq!(stability.phase, TopicPhase::Emerging, "Should start as Emerging");

        // Young with high churn -> Emerging
        stability.age_hours = 0.5;
        stability.membership_churn = 0.4;
        stability.update_phase();
        assert_eq!(stability.phase, TopicPhase::Emerging);

        // Old with low churn -> Stable
        stability.age_hours = 48.0;
        stability.membership_churn = 0.05;
        stability.update_phase();
        assert_eq!(stability.phase, TopicPhase::Stable);

        // High churn -> Declining
        stability.membership_churn = 0.6;
        stability.update_phase();
        assert_eq!(stability.phase, TopicPhase::Declining);

        println!("[PASS] test_stability_phase_transitions");
    }

    #[test]
    fn test_stability_health_check() {
        let mut stability = TopicStability::new();
        stability.membership_churn = 0.2;
        assert!(stability.is_healthy(), "churn 0.2 < 0.3 should be healthy");

        stability.membership_churn = 0.4;
        assert!(!stability.is_healthy(), "churn 0.4 >= 0.3 should not be healthy");
        println!("[PASS] test_stability_health_check");
    }

    // ===== Serialization Tests =====

    #[test]
    fn test_topic_serialization_roundtrip() {
        let mut strengths = [0.0; 13];
        strengths[0] = 0.9;
        strengths[4] = 0.8;

        let profile = TopicProfile::new(strengths);
        let topic = Topic::new(profile, HashMap::new(), vec![Uuid::new_v4()]);

        let json = serde_json::to_string(&topic).expect("serialize should work");
        let restored: Topic = serde_json::from_str(&json).expect("deserialize should work");

        assert_eq!(topic.id, restored.id);
        assert_eq!(topic.profile.strengths, restored.profile.strengths);
        assert_eq!(topic.member_memories.len(), restored.member_memories.len());
        println!(
            "[PASS] test_topic_serialization_roundtrip - JSON length: {}",
            json.len()
        );
    }

    #[test]
    fn test_topic_phase_serialization() {
        for phase in [
            TopicPhase::Emerging,
            TopicPhase::Stable,
            TopicPhase::Declining,
            TopicPhase::Merging,
        ] {
            let json = serde_json::to_string(&phase).expect("serialize phase");
            let restored: TopicPhase = serde_json::from_str(&json).expect("deserialize phase");
            assert_eq!(phase, restored);
        }
        println!("[PASS] test_topic_phase_serialization - all phases serialize correctly");
    }

    #[test]
    fn test_topic_profile_serialization() {
        let profile = TopicProfile::new([0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 0.0, 0.0, 0.0]);

        let json = serde_json::to_string(&profile).expect("serialize profile");
        let restored: TopicProfile = serde_json::from_str(&json).expect("deserialize profile");

        assert_eq!(profile.strengths, restored.strengths);
        println!("[PASS] test_topic_profile_serialization");
    }

    #[test]
    fn test_topic_stability_serialization() {
        let mut stability = TopicStability::new();
        stability.age_hours = 12.5;
        stability.membership_churn = 0.25;
        stability.access_count = 100;

        let json = serde_json::to_string(&stability).expect("serialize stability");
        let restored: TopicStability = serde_json::from_str(&json).expect("deserialize stability");

        assert_eq!(stability.phase, restored.phase);
        assert!((stability.age_hours - restored.age_hours).abs() < f32::EPSILON);
        assert!((stability.membership_churn - restored.membership_churn).abs() < f32::EPSILON);
        assert_eq!(stability.access_count, restored.access_count);
        println!("[PASS] test_topic_stability_serialization");
    }

    // ===== TopicPhase Display Tests =====

    #[test]
    fn test_topic_phase_display() {
        assert_eq!(format!("{}", TopicPhase::Emerging), "Emerging");
        assert_eq!(format!("{}", TopicPhase::Stable), "Stable");
        assert_eq!(format!("{}", TopicPhase::Declining), "Declining");
        assert_eq!(format!("{}", TopicPhase::Merging), "Merging");
        println!("[PASS] test_topic_phase_display");
    }

    // ===== Edge Cases =====

    #[test]
    fn test_nan_handling_weighted_agreement() {
        // Force a NaN scenario by manipulating internals if possible
        // For now, ensure normal paths don't produce NaN
        let profile = TopicProfile::new([0.5; 13]);
        let weighted = profile.weighted_agreement();

        assert!(!weighted.is_nan(), "weighted_agreement should not be NaN");
        assert!(!weighted.is_infinite(), "weighted_agreement should not be infinite");
        println!("[PASS] test_nan_handling_weighted_agreement");
    }

    #[test]
    fn test_extreme_strength_values() {
        // Test with max f32 values (should clamp)
        let profile = TopicProfile::new([f32::MAX; 13]);
        assert_eq!(profile.strengths[0], 1.0, "MAX should clamp to 1.0");

        // Test with negative infinity (should clamp to 0.0)
        let profile2 = TopicProfile::new([f32::NEG_INFINITY; 13]);
        assert_eq!(profile2.strengths[0], 0.0, "NEG_INFINITY should clamp to 0.0");

        println!("[PASS] test_extreme_strength_values");
    }

    #[test]
    fn test_constitution_examples() {
        // From constitution.yaml topic_detection.examples:

        // "3 semantic spaces agreeing = 3.0 -> TOPIC"
        let mut s1 = [0.0f32; 13];
        s1[Embedder::Semantic.index()] = 1.0;
        s1[Embedder::Causal.index()] = 1.0;
        s1[Embedder::Code.index()] = 1.0;
        let p1 = TopicProfile::new(s1);
        assert!(p1.is_topic(), "3 semantic = 3.0 should be topic");

        // "2 semantic + 1 relational = 2.5 -> TOPIC"
        let mut s2 = [0.0f32; 13];
        s2[Embedder::Semantic.index()] = 1.0;
        s2[Embedder::Causal.index()] = 1.0;
        s2[Embedder::Entity.index()] = 1.0; // relational (0.5)
        let p2 = TopicProfile::new(s2);
        assert!(p2.is_topic(), "2 semantic + 1 relational = 2.5 should be topic");

        // "2 semantic spaces only = 2.0 -> NOT TOPIC"
        let mut s3 = [0.0f32; 13];
        s3[Embedder::Semantic.index()] = 1.0;
        s3[Embedder::Causal.index()] = 1.0;
        let p3 = TopicProfile::new(s3);
        assert!(!p3.is_topic(), "2 semantic = 2.0 should NOT be topic");

        // "5 temporal spaces = 0.0 -> NOT TOPIC (excluded)"
        let mut s4 = [0.0f32; 13];
        s4[Embedder::TemporalRecent.index()] = 1.0;
        s4[Embedder::TemporalPeriodic.index()] = 1.0;
        s4[Embedder::TemporalPositional.index()] = 1.0;
        let p4 = TopicProfile::new(s4);
        assert!(!p4.is_topic(), "temporal-only = 0.0 should NOT be topic (AP-60)");

        // "1 semantic + 3 relational = 2.5 -> TOPIC"
        // Note: There are only 2 relational embedders (E8, E11), so 1 semantic + 2 relational = 2.0
        // Let's test with 1 semantic + 2 relational + 1 structural = 2.5
        let mut s5 = [0.0f32; 13];
        s5[Embedder::Semantic.index()] = 1.0;       // semantic (1.0)
        s5[Embedder::Graph.index()] = 1.0;      // relational (0.5)
        s5[Embedder::Entity.index()] = 1.0;         // relational (0.5)
        s5[Embedder::Hdc.index()] = 1.0;            // structural (0.5)
        let p5 = TopicProfile::new(s5);
        let w5 = p5.weighted_agreement();
        assert!((w5 - 2.5).abs() < 0.001, "1 semantic + 2 relational + 1 structural = 2.5, got {}", w5);
        assert!(p5.is_topic(), "weighted 2.5 should be topic");

        println!("[PASS] test_constitution_examples - all verified");
    }

    #[test]
    fn test_deterministic_topic_id_same_members() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let c = Uuid::new_v4();

        // Same members in different order produce the same ID
        let id1 = Topic::deterministic_id(&[a, b, c]);
        let id2 = Topic::deterministic_id(&[c, a, b]);
        let id3 = Topic::deterministic_id(&[b, c, a]);
        assert_eq!(id1, id2);
        assert_eq!(id2, id3);
        println!("[PASS] deterministic_id: same members, different order → same ID");
    }

    #[test]
    fn test_deterministic_topic_id_different_members() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let c = Uuid::new_v4();
        let d = Uuid::new_v4();

        let id1 = Topic::deterministic_id(&[a, b, c]);
        let id2 = Topic::deterministic_id(&[a, b, d]);
        assert_ne!(id1, id2);
        println!("[PASS] deterministic_id: different members → different ID");
    }

    #[test]
    fn test_deterministic_topic_id_empty() {
        let id = Topic::deterministic_id(&[]);
        assert_eq!(id, Uuid::nil());
        println!("[PASS] deterministic_id: empty members → nil UUID");
    }
}
