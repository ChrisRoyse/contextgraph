//! Topic portfolio persistence for session continuity.
//!
//! This module provides serialization and deserialization of the topic portfolio
//! to enable persistence across sessions. It reuses the existing `Topic` type
//! which already has Serde derives.
//!
//! # Architecture
//!
//! Per constitution:
//! - AP-14: No .unwrap() in library code
//! - SEC-06: Soft delete 30-day recovery (handled at storage layer)
//!
//! # Usage
//!
//! ```rust,ignore
//! use context_graph_core::clustering::{Topic, PersistedTopicPortfolio};
//!
//! // Create from current topics
//! let portfolio = PersistedTopicPortfolio::new(
//!     topics,
//!     0.15,  // churn_rate
//!     0.45,  // entropy
//!     "session-123".to_string(),
//! );
//!
//! // Serialize to bytes
//! let bytes = portfolio.to_bytes()?;
//!
//! // Deserialize from bytes
//! let restored = PersistedTopicPortfolio::from_bytes(&bytes)?;
//! ```

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::stability::TopicStabilityTracker;
use super::topic::Topic;

// =============================================================================
// Error Types
// =============================================================================

/// Errors that can occur during topic portfolio persistence.
#[derive(Debug, Error)]
pub enum PersistenceError {
    /// JSON serialization failed.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Invalid data format.
    #[error("Invalid data: {message}")]
    InvalidData {
        /// Description of what's wrong with the data
        message: String,
    },
}

impl PersistenceError {
    /// Create an InvalidData error.
    pub fn invalid_data(message: impl Into<String>) -> Self {
        Self::InvalidData {
            message: message.into(),
        }
    }
}

// =============================================================================
// PersistedTopicPortfolio
// =============================================================================

/// Persisted snapshot of the topic portfolio for session continuity.
///
/// Contains all topics with their profiles and stability metrics, plus
/// portfolio-level metrics (churn, entropy) at the time of persistence.
///
/// # Serialization
///
/// Uses JSON format for human readability and debuggability. The `Topic`
/// type already has `#[derive(Serialize, Deserialize)]` so we reuse it
/// directly without transformation.
///
/// # Example
///
/// ```rust,ignore
/// use context_graph_core::clustering::{Topic, TopicProfile, PersistedTopicPortfolio};
/// use std::collections::HashMap;
///
/// let topics = vec![
///     Topic::new(TopicProfile::default(), HashMap::new(), vec![]),
/// ];
///
/// let portfolio = PersistedTopicPortfolio::new(
///     topics,
///     0.15,
///     0.45,
///     "session-abc".to_string(),
/// );
///
/// // Serialize and deserialize
/// let bytes = portfolio.to_bytes().expect("serialize");
/// let restored = PersistedTopicPortfolio::from_bytes(&bytes).expect("deserialize");
///
/// assert_eq!(restored.session_id, "session-abc");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedTopicPortfolio {
    /// All topics in the portfolio with profiles and stability metrics.
    pub topics: Vec<Topic>,

    /// Portfolio-level churn rate at time of persistence [0.0, 1.0].
    ///
    /// Per constitution topic_stability.thresholds:
    /// - < 0.3: healthy
    /// - [0.3, 0.5): warning
    /// - >= 0.5: unstable
    pub churn_rate: f32,

    /// Portfolio-level entropy at time of persistence [0.0, 1.0].
    ///
    /// Per constitution AP-70: entropy > 0.7 contributes to dream triggers.
    pub entropy: f32,

    /// Session ID that created this snapshot.
    pub session_id: String,

    /// Unix timestamp in milliseconds when this portfolio was persisted.
    pub persisted_at_ms: u64,
}

impl PersistedTopicPortfolio {
    /// Create a new persisted portfolio from current topics.
    ///
    /// Automatically captures the current timestamp.
    ///
    /// # Arguments
    ///
    /// * `topics` - Current topics to persist
    /// * `churn_rate` - Portfolio-level churn (clamped to 0.0..=1.0)
    /// * `entropy` - Portfolio-level entropy (clamped to 0.0..=1.0)
    /// * `session_id` - Session identifier
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let portfolio = PersistedTopicPortfolio::new(
    ///     topics,
    ///     0.15,
    ///     0.45,
    ///     "session-123".to_string(),
    /// );
    /// ```
    pub fn new(topics: Vec<Topic>, churn_rate: f32, entropy: f32, session_id: String) -> Self {
        let persisted_at_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        Self {
            topics,
            churn_rate: clamp_metric(churn_rate),
            entropy: clamp_metric(entropy),
            session_id,
            persisted_at_ms,
        }
    }

    /// Create a portfolio with a specific timestamp (for testing/migration).
    ///
    /// # Arguments
    ///
    /// * `topics` - Topics to persist
    /// * `churn_rate` - Portfolio-level churn (clamped to 0.0..=1.0)
    /// * `entropy` - Portfolio-level entropy (clamped to 0.0..=1.0)
    /// * `session_id` - Session identifier
    /// * `persisted_at_ms` - Unix timestamp in milliseconds
    pub fn with_timestamp(
        topics: Vec<Topic>,
        churn_rate: f32,
        entropy: f32,
        session_id: String,
        persisted_at_ms: u64,
    ) -> Self {
        Self {
            topics,
            churn_rate: clamp_metric(churn_rate),
            entropy: clamp_metric(entropy),
            session_id,
            persisted_at_ms,
        }
    }

    /// Serialize the portfolio to JSON bytes.
    ///
    /// # Returns
    ///
    /// JSON-encoded bytes suitable for storage.
    ///
    /// # Errors
    ///
    /// Returns `PersistenceError::Serialization` if JSON encoding fails.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let bytes = portfolio.to_bytes()?;
    /// storage.write("topic_portfolio.json", &bytes)?;
    /// ```
    pub fn to_bytes(&self) -> Result<Vec<u8>, PersistenceError> {
        let json = serde_json::to_vec(self)?;
        Ok(json)
    }

    /// Deserialize a portfolio from JSON bytes.
    ///
    /// # Arguments
    ///
    /// * `bytes` - JSON-encoded portfolio data
    ///
    /// # Returns
    ///
    /// The deserialized portfolio.
    ///
    /// # Errors
    ///
    /// Returns `PersistenceError::Serialization` if JSON decoding fails.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let bytes = storage.read("topic_portfolio.json")?;
    /// let portfolio = PersistedTopicPortfolio::from_bytes(&bytes)?;
    /// ```
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, PersistenceError> {
        let portfolio: Self = serde_json::from_slice(bytes)?;
        Ok(portfolio)
    }

    /// Get the number of topics in the portfolio.
    #[inline]
    pub fn topic_count(&self) -> usize {
        self.topics.len()
    }

    /// Check if the portfolio is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.topics.is_empty()
    }

    /// Check if churn indicates instability (>= 0.5 per constitution).
    #[inline]
    pub fn is_unstable(&self) -> bool {
        self.churn_rate >= 0.5
    }

    /// Get total member count across all topics.
    pub fn total_members(&self) -> usize {
        self.topics.iter().map(|t| t.member_count()).sum()
    }

    /// Get the age of this portfolio in seconds since persistence.
    ///
    /// Returns 0 if the system time is before the persistence time.
    pub fn age_seconds(&self) -> u64 {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        now_ms.saturating_sub(self.persisted_at_ms) / 1000
    }

    /// Get valid topics (those meeting the weighted_agreement threshold).
    ///
    /// Per ARCH-09: weighted_agreement >= 2.5
    pub fn valid_topics(&self) -> Vec<&Topic> {
        self.topics.iter().filter(|t| t.is_valid()).collect()
    }

    /// Get count of valid topics.
    pub fn valid_topic_count(&self) -> usize {
        self.topics.iter().filter(|t| t.is_valid()).count()
    }
}

impl Default for PersistedTopicPortfolio {
    fn default() -> Self {
        Self {
            topics: Vec::new(),
            churn_rate: 0.0,
            entropy: 0.0,
            session_id: String::new(),
            persisted_at_ms: 0,
        }
    }
}

// =============================================================================
// TopicPortfolio - Live Topic Portfolio with Stability Tracking
// =============================================================================

/// Live topic portfolio with integrated stability tracking.
///
/// This is the runtime counterpart to `PersistedTopicPortfolio`. It holds
/// topics in a HashMap for O(1) lookup and integrates `TopicStabilityTracker`
/// for dream trigger support (per AP-70).
///
/// # AP-70 Compliance
///
/// Dream triggers require: entropy > 0.7 AND churn > 0.5
/// The stability_tracker computes churn from topic snapshots.
///
/// # Example
///
/// ```rust,ignore
/// use context_graph_core::clustering::TopicPortfolio;
///
/// let mut portfolio = TopicPortfolio::new();
///
/// // After clustering operations...
/// portfolio.take_stability_snapshot();
/// let churn = portfolio.track_churn();
///
/// // Check if dream should trigger
/// let should_dream = portfolio.check_dream_trigger(current_entropy);
/// ```
#[derive(Debug)]
pub struct TopicPortfolio {
    /// Topic storage: UUID -> Topic for O(1) lookup.
    topics: std::collections::HashMap<uuid::Uuid, Topic>,

    /// Stability tracker for churn calculation and dream triggers.
    stability_tracker: TopicStabilityTracker,
}

impl Default for TopicPortfolio {
    fn default() -> Self {
        Self::new()
    }
}

impl TopicPortfolio {
    /// Create a new empty portfolio with default stability tracking.
    pub fn new() -> Self {
        Self {
            topics: std::collections::HashMap::new(),
            stability_tracker: TopicStabilityTracker::new(),
        }
    }

    /// Create a portfolio with custom stability thresholds.
    ///
    /// # Arguments
    ///
    /// * `churn_threshold` - Churn threshold for dream trigger (default 0.5)
    /// * `entropy_threshold` - Entropy threshold for dream trigger (default 0.7)
    /// * `entropy_duration_secs` - Required high-entropy duration (default 300)
    pub fn with_thresholds(
        churn_threshold: f32,
        entropy_threshold: f32,
        entropy_duration_secs: u64,
    ) -> Self {
        Self {
            topics: std::collections::HashMap::new(),
            stability_tracker: TopicStabilityTracker::with_thresholds(
                churn_threshold,
                entropy_threshold,
                entropy_duration_secs,
            ),
        }
    }

    /// Insert or update a topic.
    pub fn insert(&mut self, topic: Topic) {
        self.topics.insert(topic.id, topic);
    }

    /// Get a topic by ID.
    pub fn get(&self, id: &uuid::Uuid) -> Option<&Topic> {
        self.topics.get(id)
    }

    /// Get a mutable reference to a topic.
    pub fn get_mut(&mut self, id: &uuid::Uuid) -> Option<&mut Topic> {
        self.topics.get_mut(id)
    }

    /// Remove a topic by ID.
    pub fn remove(&mut self, id: &uuid::Uuid) -> Option<Topic> {
        self.topics.remove(id)
    }

    /// Check if a topic exists.
    pub fn contains(&self, id: &uuid::Uuid) -> bool {
        self.topics.contains_key(id)
    }

    /// Get all topics as a reference to the internal HashMap.
    pub fn topics(&self) -> &std::collections::HashMap<uuid::Uuid, Topic> {
        &self.topics
    }

    /// Get all topics as a mutable reference.
    pub fn topics_mut(&mut self) -> &mut std::collections::HashMap<uuid::Uuid, Topic> {
        &mut self.topics
    }

    /// Get an iterator over topics.
    pub fn iter(&self) -> impl Iterator<Item = (&uuid::Uuid, &Topic)> {
        self.topics.iter()
    }

    /// Get topic count.
    #[inline]
    pub fn len(&self) -> usize {
        self.topics.len()
    }

    /// Check if portfolio is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.topics.is_empty()
    }

    /// Clear all topics (preserves stability history).
    pub fn clear(&mut self) {
        self.topics.clear();
    }

    // =========================================================================
    // Stability Tracking (AP-70)
    // =========================================================================

    /// Take a stability snapshot of current topics.
    ///
    /// Call this periodically (e.g., every minute) to track portfolio changes.
    pub fn take_stability_snapshot(&mut self) {
        let topics_vec: Vec<Topic> = self.topics.values().cloned().collect();
        self.stability_tracker.take_snapshot(&topics_vec);
    }

    /// Compute churn by comparing current state to ~1 hour ago.
    ///
    /// # Returns
    ///
    /// Churn rate [0.0, 1.0] where:
    /// - 0.0 = no change (stable)
    /// - 1.0 = complete turnover
    pub fn track_churn(&mut self) -> f32 {
        self.stability_tracker.track_churn()
    }

    /// Get current churn rate (last computed value).
    #[inline]
    pub fn current_churn(&self) -> f32 {
        self.stability_tracker.current_churn()
    }

    /// Check if dream consolidation should trigger (AP-70).
    ///
    /// Per constitution AP-70, triggers when EITHER:
    /// 1. entropy > 0.7 AND churn > 0.5 (both simultaneously)
    /// 2. entropy > 0.7 for 5+ continuous minutes
    ///
    /// # Arguments
    ///
    /// * `entropy` - Current system entropy [0.0, 1.0]
    ///
    /// # Returns
    ///
    /// true if dream should be triggered
    pub fn check_dream_trigger(&mut self, entropy: f32) -> bool {
        self.stability_tracker.check_dream_trigger(entropy)
    }

    /// Get reference to stability tracker for advanced queries.
    pub fn stability_tracker(&self) -> &TopicStabilityTracker {
        &self.stability_tracker
    }

    /// Get mutable reference to stability tracker.
    pub fn stability_tracker_mut(&mut self) -> &mut TopicStabilityTracker {
        &mut self.stability_tracker
    }

    /// Reset entropy tracking (call after dream completes).
    pub fn reset_entropy_tracking(&mut self) {
        self.stability_tracker.reset_entropy_tracking();
    }

    /// Check if system is stable (low churn over 6 hours).
    pub fn is_stable(&self) -> bool {
        self.stability_tracker.is_stable()
    }

    /// Get average churn over specified hours.
    pub fn average_churn(&self, hours: i64) -> f32 {
        self.stability_tracker.average_churn(hours)
    }

    // =========================================================================
    // Persistence Integration
    // =========================================================================

    /// Export to a persisted portfolio snapshot.
    ///
    /// # Arguments
    ///
    /// * `session_id` - Session identifier
    /// * `entropy` - Current system entropy
    pub fn export(&self, session_id: impl Into<String>, entropy: f32) -> PersistedTopicPortfolio {
        let topics: Vec<Topic> = self.topics.values().cloned().collect();
        PersistedTopicPortfolio::new(
            topics,
            self.stability_tracker.current_churn(),
            entropy,
            session_id.into(),
        )
    }

    /// Import topics from a persisted portfolio.
    ///
    /// Replaces current topics with imported ones. Does NOT restore
    /// stability tracker state (snapshots are not persisted).
    ///
    /// # Returns
    ///
    /// Number of topics imported.
    pub fn import(&mut self, portfolio: &PersistedTopicPortfolio) -> usize {
        self.topics.clear();
        for topic in &portfolio.topics {
            self.topics.insert(topic.id, topic.clone());
        }
        self.topics.len()
    }

    /// Get portfolio summary (topic_count, total_members).
    pub fn summary(&self) -> (usize, usize) {
        let total_members: usize = self.topics.values().map(|t| t.member_count()).sum();
        (self.topics.len(), total_members)
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Clamp a metric value to [0.0, 1.0] and handle NaN/Infinity (per AP-10).
#[inline]
fn clamp_metric(value: f32) -> f32 {
    if value.is_nan() || value.is_infinite() {
        0.0
    } else {
        value.clamp(0.0, 1.0)
    }
}

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clustering::topic::{TopicPhase, TopicProfile, TopicStability};
    use std::collections::HashMap;
    use uuid::Uuid;

    /// Create a test topic with specified profile strengths.
    fn create_test_topic(strengths: [f32; 14]) -> Topic {
        Topic::new(
            TopicProfile::new(strengths),
            HashMap::new(),
            vec![Uuid::new_v4()],
        )
    }

    /// Create a valid topic (meets weighted_agreement >= 2.5).
    fn create_valid_topic() -> Topic {
        // 3 semantic spaces at 1.0 = weighted_agreement 3.0
        let mut strengths = [0.0; 14];
        strengths[0] = 1.0; // E1 Semantic
        strengths[4] = 1.0; // E5 Causal
        strengths[6] = 1.0; // E7 Code
        create_test_topic(strengths)
    }

    // ===== Constructor Tests =====

    #[test]
    fn test_new_portfolio() {
        let topics = vec![create_valid_topic(), create_valid_topic()];

        let portfolio =
            PersistedTopicPortfolio::new(topics, 0.15, 0.45, "test-session".to_string());

        assert_eq!(portfolio.topic_count(), 2);
        assert!((portfolio.churn_rate - 0.15).abs() < f32::EPSILON);
        assert!((portfolio.entropy - 0.45).abs() < f32::EPSILON);
        assert_eq!(portfolio.session_id, "test-session");
        assert!(portfolio.persisted_at_ms > 0);

        println!("[PASS] test_new_portfolio");
    }

    // ===== Serialization Tests =====

    #[test]
    fn test_serialization_roundtrip() {
        let topics = vec![create_valid_topic(), create_valid_topic()];
        let original =
            PersistedTopicPortfolio::new(topics, 0.25, 0.65, "roundtrip-test".to_string());

        let bytes = original.to_bytes().expect("serialize should succeed");
        let restored =
            PersistedTopicPortfolio::from_bytes(&bytes).expect("deserialize should succeed");

        assert_eq!(original.topic_count(), restored.topic_count());
        assert!((original.churn_rate - restored.churn_rate).abs() < f32::EPSILON);
        assert!((original.entropy - restored.entropy).abs() < f32::EPSILON);
        assert_eq!(original.session_id, restored.session_id);
        assert_eq!(original.persisted_at_ms, restored.persisted_at_ms);

        // Verify topic IDs match
        for (orig, rest) in original.topics.iter().zip(restored.topics.iter()) {
            assert_eq!(orig.id, rest.id);
        }

        println!("[PASS] test_serialization_roundtrip");
    }

    #[test]
    fn test_serialization_empty_portfolio() {
        let portfolio = PersistedTopicPortfolio::default();

        let bytes = portfolio.to_bytes().expect("serialize empty portfolio");
        let restored =
            PersistedTopicPortfolio::from_bytes(&bytes).expect("deserialize empty portfolio");

        assert!(restored.is_empty());
        assert_eq!(restored.churn_rate, 0.0);
        assert_eq!(restored.entropy, 0.0);

        println!("[PASS] test_serialization_empty_portfolio");
    }

    #[test]
    fn test_deserialization_invalid_json() {
        let invalid_bytes = b"not valid json";

        let result = PersistedTopicPortfolio::from_bytes(invalid_bytes);

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            PersistenceError::Serialization(_)
        ));

        println!("[PASS] test_deserialization_invalid_json");
    }

    // ===== Topic Content Preservation Tests =====

    #[test]
    fn test_topic_stability_preserved() {
        let mut topic = create_valid_topic();
        topic.stability = TopicStability {
            phase: TopicPhase::Stable,
            age_hours: 48.5,
            membership_churn: 0.05,
            centroid_drift: 0.02,
            access_count: 150,
            last_accessed: None,
        };

        let portfolio = PersistedTopicPortfolio::new(vec![topic], 0.1, 0.2, "test".to_string());

        let bytes = portfolio.to_bytes().expect("serialize");
        let restored = PersistedTopicPortfolio::from_bytes(&bytes).expect("deserialize");

        let restored_stability = &restored.topics[0].stability;
        assert_eq!(restored_stability.phase, TopicPhase::Stable);
        assert!((restored_stability.age_hours - 48.5).abs() < f32::EPSILON);
        assert!((restored_stability.membership_churn - 0.05).abs() < f32::EPSILON);
        assert_eq!(restored_stability.access_count, 150);

        println!("[PASS] test_topic_stability_preserved");
    }

    // ===== Constitution Compliance Tests =====

    #[test]
    fn test_constitution_churn_thresholds() {
        // healthy (< 0.3)
        let healthy = PersistedTopicPortfolio::new(vec![], 0.29, 0.5, "test".to_string());
        assert!(!healthy.is_unstable());

        // warning (0.3 to 0.5) - not unstable yet
        let warning = PersistedTopicPortfolio::new(vec![], 0.4, 0.5, "test".to_string());
        assert!(!warning.is_unstable());

        // unstable (>= 0.5)
        let unstable = PersistedTopicPortfolio::new(vec![], 0.5, 0.5, "test".to_string());
        assert!(unstable.is_unstable());

        println!("[PASS] test_constitution_churn_thresholds");
    }
}
