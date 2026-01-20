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

    /// Serialize the portfolio to pretty-printed JSON bytes.
    ///
    /// Useful for debugging and human inspection.
    ///
    /// # Returns
    ///
    /// Pretty-printed JSON-encoded bytes.
    ///
    /// # Errors
    ///
    /// Returns `PersistenceError::Serialization` if JSON encoding fails.
    pub fn to_bytes_pretty(&self) -> Result<Vec<u8>, PersistenceError> {
        let json = serde_json::to_vec_pretty(self)?;
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

    /// Check if entropy is high (> 0.7 per AP-70).
    #[inline]
    pub fn has_high_entropy(&self) -> bool {
        self.entropy > 0.7
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
    fn create_test_topic(strengths: [f32; 13]) -> Topic {
        Topic::new(TopicProfile::new(strengths), HashMap::new(), vec![Uuid::new_v4()])
    }

    /// Create a valid topic (meets weighted_agreement >= 2.5).
    fn create_valid_topic() -> Topic {
        // 3 semantic spaces at 1.0 = weighted_agreement 3.0
        let mut strengths = [0.0; 13];
        strengths[0] = 1.0; // E1 Semantic
        strengths[4] = 1.0; // E5 Causal
        strengths[6] = 1.0; // E7 Code
        create_test_topic(strengths)
    }

    /// Create an invalid topic (weighted_agreement < 2.5).
    fn create_invalid_topic() -> Topic {
        // 2 semantic spaces = weighted_agreement 2.0 < 2.5
        let mut strengths = [0.0; 13];
        strengths[0] = 1.0; // E1 Semantic
        strengths[4] = 1.0; // E5 Causal
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

    #[test]
    fn test_with_timestamp() {
        let topics = vec![create_valid_topic()];
        let timestamp = 1700000000000u64;

        let portfolio =
            PersistedTopicPortfolio::with_timestamp(topics, 0.2, 0.5, "session-1".to_string(), timestamp);

        assert_eq!(portfolio.persisted_at_ms, timestamp);
        assert_eq!(portfolio.session_id, "session-1");

        println!("[PASS] test_with_timestamp");
    }

    #[test]
    fn test_default_portfolio() {
        let portfolio = PersistedTopicPortfolio::default();

        assert!(portfolio.is_empty());
        assert_eq!(portfolio.churn_rate, 0.0);
        assert_eq!(portfolio.entropy, 0.0);
        assert!(portfolio.session_id.is_empty());
        assert_eq!(portfolio.persisted_at_ms, 0);

        println!("[PASS] test_default_portfolio");
    }

    // ===== Metric Clamping Tests =====

    #[test]
    fn test_churn_rate_clamping() {
        // Test clamping high value
        let portfolio = PersistedTopicPortfolio::new(vec![], 1.5, 0.5, "test".to_string());
        assert_eq!(portfolio.churn_rate, 1.0);

        // Test clamping negative value
        let portfolio = PersistedTopicPortfolio::new(vec![], -0.5, 0.5, "test".to_string());
        assert_eq!(portfolio.churn_rate, 0.0);

        println!("[PASS] test_churn_rate_clamping");
    }

    #[test]
    fn test_entropy_clamping() {
        // Test clamping high value
        let portfolio = PersistedTopicPortfolio::new(vec![], 0.5, 2.0, "test".to_string());
        assert_eq!(portfolio.entropy, 1.0);

        // Test clamping negative value
        let portfolio = PersistedTopicPortfolio::new(vec![], 0.5, -1.0, "test".to_string());
        assert_eq!(portfolio.entropy, 0.0);

        println!("[PASS] test_entropy_clamping");
    }

    #[test]
    fn test_nan_handling() {
        let portfolio = PersistedTopicPortfolio::new(vec![], f32::NAN, f32::NAN, "test".to_string());

        assert!(!portfolio.churn_rate.is_nan());
        assert!(!portfolio.entropy.is_nan());
        assert_eq!(portfolio.churn_rate, 0.0);
        assert_eq!(portfolio.entropy, 0.0);

        println!("[PASS] test_nan_handling");
    }

    #[test]
    fn test_infinity_handling() {
        let portfolio =
            PersistedTopicPortfolio::new(vec![], f32::INFINITY, f32::NEG_INFINITY, "test".to_string());

        assert!(!portfolio.churn_rate.is_infinite());
        assert!(!portfolio.entropy.is_infinite());
        assert_eq!(portfolio.churn_rate, 0.0);
        assert_eq!(portfolio.entropy, 0.0);

        println!("[PASS] test_infinity_handling");
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
    fn test_pretty_serialization() {
        let topics = vec![create_valid_topic()];
        let portfolio = PersistedTopicPortfolio::new(topics, 0.1, 0.3, "pretty-test".to_string());

        let bytes = portfolio.to_bytes_pretty().expect("pretty serialize should succeed");
        let json_str = String::from_utf8(bytes.clone()).expect("should be valid UTF-8");

        // Pretty print should contain newlines
        assert!(json_str.contains('\n'), "Pretty JSON should have newlines");

        // Should still deserialize correctly
        let restored =
            PersistedTopicPortfolio::from_bytes(&bytes).expect("deserialize should succeed");
        assert_eq!(portfolio.session_id, restored.session_id);

        println!("[PASS] test_pretty_serialization");
    }

    #[test]
    fn test_serialization_empty_portfolio() {
        let portfolio = PersistedTopicPortfolio::default();

        let bytes = portfolio.to_bytes().expect("serialize empty portfolio");
        let restored = PersistedTopicPortfolio::from_bytes(&bytes).expect("deserialize empty portfolio");

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
        assert!(matches!(result.unwrap_err(), PersistenceError::Serialization(_)));

        println!("[PASS] test_deserialization_invalid_json");
    }

    #[test]
    fn test_deserialization_wrong_structure() {
        let wrong_json = br#"{"wrong": "structure"}"#;

        let result = PersistedTopicPortfolio::from_bytes(wrong_json);

        assert!(result.is_err());

        println!("[PASS] test_deserialization_wrong_structure");
    }

    // ===== Accessor Tests =====

    #[test]
    fn test_topic_count() {
        let topics = vec![create_valid_topic(), create_valid_topic(), create_valid_topic()];
        let portfolio = PersistedTopicPortfolio::new(topics, 0.1, 0.2, "test".to_string());

        assert_eq!(portfolio.topic_count(), 3);

        println!("[PASS] test_topic_count");
    }

    #[test]
    fn test_is_empty() {
        let empty = PersistedTopicPortfolio::new(vec![], 0.1, 0.2, "test".to_string());
        let non_empty = PersistedTopicPortfolio::new(vec![create_valid_topic()], 0.1, 0.2, "test".to_string());

        assert!(empty.is_empty());
        assert!(!non_empty.is_empty());

        println!("[PASS] test_is_empty");
    }

    #[test]
    fn test_is_unstable() {
        // Stable (churn < 0.5)
        let stable = PersistedTopicPortfolio::new(vec![], 0.3, 0.5, "test".to_string());
        assert!(!stable.is_unstable());

        // Unstable (churn >= 0.5)
        let unstable = PersistedTopicPortfolio::new(vec![], 0.5, 0.5, "test".to_string());
        assert!(unstable.is_unstable());

        let very_unstable = PersistedTopicPortfolio::new(vec![], 0.8, 0.5, "test".to_string());
        assert!(very_unstable.is_unstable());

        println!("[PASS] test_is_unstable");
    }

    #[test]
    fn test_has_high_entropy() {
        // Normal entropy (<= 0.7)
        let normal = PersistedTopicPortfolio::new(vec![], 0.3, 0.7, "test".to_string());
        assert!(!normal.has_high_entropy());

        // High entropy (> 0.7)
        let high = PersistedTopicPortfolio::new(vec![], 0.3, 0.71, "test".to_string());
        assert!(high.has_high_entropy());

        println!("[PASS] test_has_high_entropy");
    }

    #[test]
    fn test_total_members() {
        // Create topics with known member counts
        let mut topic1 = create_valid_topic();
        topic1.member_memories = vec![Uuid::new_v4(), Uuid::new_v4()]; // 2 members

        let mut topic2 = create_valid_topic();
        topic2.member_memories = vec![Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()]; // 3 members

        let portfolio = PersistedTopicPortfolio::new(vec![topic1, topic2], 0.1, 0.2, "test".to_string());

        assert_eq!(portfolio.total_members(), 5);

        println!("[PASS] test_total_members");
    }

    #[test]
    fn test_valid_topics() {
        let valid1 = create_valid_topic();
        let valid2 = create_valid_topic();
        let invalid = create_invalid_topic();

        let portfolio =
            PersistedTopicPortfolio::new(vec![valid1, invalid, valid2], 0.1, 0.2, "test".to_string());

        assert_eq!(portfolio.topic_count(), 3);
        assert_eq!(portfolio.valid_topic_count(), 2);
        assert_eq!(portfolio.valid_topics().len(), 2);

        println!("[PASS] test_valid_topics");
    }

    // ===== Age Tests =====

    #[test]
    fn test_age_seconds() {
        // Create a portfolio from 10 seconds ago
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let ten_seconds_ago = now_ms.saturating_sub(10_000);
        let portfolio =
            PersistedTopicPortfolio::with_timestamp(vec![], 0.1, 0.2, "test".to_string(), ten_seconds_ago);

        let age = portfolio.age_seconds();

        // Allow some tolerance for test execution time
        assert!(age >= 9 && age <= 12, "Age should be ~10 seconds, got {}", age);

        println!("[PASS] test_age_seconds");
    }

    #[test]
    fn test_age_seconds_future_timestamp() {
        // Test with future timestamp (should return 0 due to saturating_sub)
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let future = now_ms + 10_000;
        let portfolio =
            PersistedTopicPortfolio::with_timestamp(vec![], 0.1, 0.2, "test".to_string(), future);

        let age = portfolio.age_seconds();
        assert_eq!(age, 0, "Future timestamp should yield 0 age");

        println!("[PASS] test_age_seconds_future_timestamp");
    }

    // ===== Error Tests =====

    #[test]
    fn test_persistence_error_display() {
        let invalid_err = PersistenceError::invalid_data("test error message");
        let msg = invalid_err.to_string();

        assert!(msg.contains("test error message"));

        println!("[PASS] test_persistence_error_display");
    }

    #[test]
    fn test_persistence_error_debug() {
        let err = PersistenceError::invalid_data("debug test");
        let debug = format!("{:?}", err);

        assert!(!debug.is_empty());

        println!("[PASS] test_persistence_error_debug");
    }

    // ===== Topic Content Preservation Tests =====

    #[test]
    fn test_topic_profile_preserved() {
        let mut strengths = [0.0; 13];
        strengths[0] = 0.8;
        strengths[4] = 0.7;
        strengths[6] = 0.9;

        let topic = create_test_topic(strengths);
        let original_profile = topic.profile.strengths;

        let portfolio = PersistedTopicPortfolio::new(vec![topic], 0.1, 0.2, "test".to_string());

        let bytes = portfolio.to_bytes().expect("serialize");
        let restored = PersistedTopicPortfolio::from_bytes(&bytes).expect("deserialize");

        assert_eq!(restored.topics[0].profile.strengths, original_profile);

        println!("[PASS] test_topic_profile_preserved");
    }

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

    #[test]
    fn test_topic_members_preserved() {
        let mut topic = create_valid_topic();
        let mem1 = Uuid::new_v4();
        let mem2 = Uuid::new_v4();
        let mem3 = Uuid::new_v4();
        topic.member_memories = vec![mem1, mem2, mem3];

        let portfolio = PersistedTopicPortfolio::new(vec![topic], 0.1, 0.2, "test".to_string());

        let bytes = portfolio.to_bytes().expect("serialize");
        let restored = PersistedTopicPortfolio::from_bytes(&bytes).expect("deserialize");

        assert_eq!(restored.topics[0].member_memories.len(), 3);
        assert!(restored.topics[0].member_memories.contains(&mem1));
        assert!(restored.topics[0].member_memories.contains(&mem2));
        assert!(restored.topics[0].member_memories.contains(&mem3));

        println!("[PASS] test_topic_members_preserved");
    }

    #[test]
    fn test_topic_name_preserved() {
        let mut topic = create_valid_topic();
        topic.set_name("Authentication Patterns".to_string());

        let portfolio = PersistedTopicPortfolio::new(vec![topic], 0.1, 0.2, "test".to_string());

        let bytes = portfolio.to_bytes().expect("serialize");
        let restored = PersistedTopicPortfolio::from_bytes(&bytes).expect("deserialize");

        assert_eq!(
            restored.topics[0].name,
            Some("Authentication Patterns".to_string())
        );

        println!("[PASS] test_topic_name_preserved");
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

    #[test]
    fn test_constitution_entropy_threshold() {
        // Per AP-70: entropy > 0.7 is high
        let at_threshold = PersistedTopicPortfolio::new(vec![], 0.3, 0.7, "test".to_string());
        assert!(!at_threshold.has_high_entropy(), "0.7 should NOT be high (> not >=)");

        let above_threshold = PersistedTopicPortfolio::new(vec![], 0.3, 0.701, "test".to_string());
        assert!(above_threshold.has_high_entropy(), "0.701 should be high");

        println!("[PASS] test_constitution_entropy_threshold");
    }
}
