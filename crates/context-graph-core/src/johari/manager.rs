//! JohariTransitionManager trait for coordinating Johari quadrant transitions.
//!
//! This module defines the core trait that manages:
//! - Classification of semantic fingerprints into Johari quadrants
//! - Transition execution with validation
//! - Batch operations for efficiency
//! - Blind spot discovery from external signals
//! - Transition statistics aggregation

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::types::fingerprint::{JohariFingerprint, SemanticFingerprint};
use crate::types::{JohariQuadrant, JohariTransition, TransitionTrigger};

use super::error::JohariError;
use super::external_signal::{BlindSpotCandidate, ExternalSignal};
use super::stats::TransitionStats;

/// Number of embedders in the system (E1-E13).
pub const NUM_EMBEDDERS: usize = 13;

/// Memory ID for Johari operations (wraps TeleologicalFingerprint's UUID).
pub type MemoryId = Uuid;

/// Time range for statistics queries.
#[derive(Debug, Clone)]
pub struct TimeRange {
    /// Start of the time range (inclusive)
    pub start: DateTime<Utc>,
    /// End of the time range (exclusive)
    pub end: DateTime<Utc>,
}

impl TimeRange {
    /// Create a new time range.
    pub fn new(start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        Self { start, end }
    }

    /// Create a time range for the last N hours.
    pub fn last_hours(hours: i64) -> Self {
        let end = Utc::now();
        let start = end - chrono::Duration::hours(hours);
        Self { start, end }
    }

    /// Create a time range for the last N days.
    pub fn last_days(days: i64) -> Self {
        let end = Utc::now();
        let start = end - chrono::Duration::days(days);
        Self { start, end }
    }
}

/// Context for classification decisions.
///
/// Contains the UTL (Unified Theory of Learning) state used to classify
/// each embedder space into Johari quadrants.
#[derive(Debug, Clone)]
pub struct ClassificationContext {
    /// ΔS values per embedder (entropy change from UTL).
    /// Higher values indicate more information entropy.
    pub delta_s: [f32; NUM_EMBEDDERS],

    /// ΔC values per embedder (coherence change from UTL).
    /// Higher values indicate better pattern coherence.
    pub delta_c: [f32; NUM_EMBEDDERS],

    /// Whether each embedder should be disclosed (user preference).
    /// If false, forces Hidden quadrant even if classification is Open.
    pub disclosure_intent: [bool; NUM_EMBEDDERS],

    /// Recent access counts per embedder.
    /// Higher values indicate more frequent retrieval.
    pub access_counts: [u32; NUM_EMBEDDERS],
}

impl ClassificationContext {
    /// Create from UTL state with default disclosure (all disclosed).
    pub fn from_utl(delta_s: [f32; NUM_EMBEDDERS], delta_c: [f32; NUM_EMBEDDERS]) -> Self {
        Self {
            delta_s,
            delta_c,
            disclosure_intent: [true; NUM_EMBEDDERS],
            access_counts: [0; NUM_EMBEDDERS],
        }
    }

    /// Create with uniform UTL values for all embedders.
    pub fn uniform(delta_s: f32, delta_c: f32) -> Self {
        Self {
            delta_s: [delta_s; NUM_EMBEDDERS],
            delta_c: [delta_c; NUM_EMBEDDERS],
            disclosure_intent: [true; NUM_EMBEDDERS],
            access_counts: [0; NUM_EMBEDDERS],
        }
    }

    /// Set disclosure intent for a specific embedder.
    pub fn with_disclosure(mut self, embedder_idx: usize, disclosed: bool) -> Self {
        if embedder_idx < NUM_EMBEDDERS {
            self.disclosure_intent[embedder_idx] = disclosed;
        }
        self
    }

    /// Set all embedders to hidden (not disclosed).
    pub fn all_hidden(mut self) -> Self {
        self.disclosure_intent = [false; NUM_EMBEDDERS];
        self
    }
}

impl Default for ClassificationContext {
    fn default() -> Self {
        Self {
            delta_s: [0.3; NUM_EMBEDDERS], // Low entropy (Open-friendly)
            delta_c: [0.7; NUM_EMBEDDERS], // High coherence (Open-friendly)
            disclosure_intent: [true; NUM_EMBEDDERS],
            access_counts: [0; NUM_EMBEDDERS],
        }
    }
}

/// Pattern for querying memories by Johari configuration.
#[derive(Debug, Clone)]
pub enum QuadrantPattern {
    /// All 13 embedders in this quadrant.
    AllIn(JohariQuadrant),

    /// At least N embedders in this quadrant.
    AtLeast {
        quadrant: JohariQuadrant,
        count: usize,
    },

    /// Specific dominant quadrant per embedder.
    Exact([JohariQuadrant; NUM_EMBEDDERS]),

    /// Mixed with constraints.
    Mixed { min_open: usize, max_unknown: usize },
}

impl QuadrantPattern {
    /// Pattern for mostly-open memories (at least 10 Open embedders).
    pub fn mostly_open() -> Self {
        Self::AtLeast {
            quadrant: JohariQuadrant::Open,
            count: 10,
        }
    }

    /// Pattern for memories with discovery opportunities (at least 3 Blind).
    pub fn has_blind_spots() -> Self {
        Self::AtLeast {
            quadrant: JohariQuadrant::Blind,
            count: 3,
        }
    }

    /// Pattern for frontier memories (at least 5 Unknown).
    pub fn frontier() -> Self {
        Self::AtLeast {
            quadrant: JohariQuadrant::Unknown,
            count: 5,
        }
    }
}

/// Manages Johari quadrant transitions with persistence.
///
/// This trait defines the complete interface for managing Johari state
/// across all 13 embedding spaces. Implementations must:
/// - Validate transitions using the state machine in JohariQuadrant
/// - Persist changes via TeleologicalMemoryStore
/// - Support batch operations for efficiency
/// - Discover blind spots from external signals
///
/// # Performance Requirements (from TASK-L004)
/// - `classify()`: <1ms (13 O(1) classifications)
/// - `transition()`: <5ms (single store read + write)
/// - `transition_batch()`: <10ms (atomic batch with validation)
/// - `find_by_quadrant()`: <10ms per 10K memories
/// - `discover_blind_spots()`: <2ms (linear scan of signals)
#[async_trait]
pub trait JohariTransitionManager: Send + Sync {
    /// Classify a SemanticFingerprint's Johari quadrants from UTL state.
    ///
    /// Uses `JohariFingerprint::classify_quadrant()` per embedder.
    /// Does NOT persist - returns the computed JohariFingerprint.
    ///
    /// # Arguments
    /// * `semantic` - The semantic fingerprint to classify
    /// * `context` - Classification context with ΔS, ΔC values
    ///
    /// # Returns
    /// A JohariFingerprint with quadrant weights set based on UTL thresholds.
    ///
    /// # Performance
    /// Must complete in <1ms (13 classifications at O(1) each).
    async fn classify(
        &self,
        semantic: &SemanticFingerprint,
        context: &ClassificationContext,
    ) -> Result<JohariFingerprint, JohariError>;

    /// Execute and persist a transition for a single embedder space.
    ///
    /// 1. Loads current JohariFingerprint from storage
    /// 2. Validates transition via `JohariQuadrant::can_transition_to()`
    /// 3. Updates the fingerprint's quadrant weights
    /// 4. Records JohariTransition in history
    /// 5. Persists updated fingerprint
    ///
    /// # Arguments
    /// * `memory_id` - UUID of the memory to transition
    /// * `embedder_idx` - Which embedder space to transition (0-12)
    /// * `to_quadrant` - Target quadrant
    /// * `trigger` - What caused this transition
    ///
    /// # Returns
    /// The updated JohariFingerprint after the transition.
    ///
    /// # Errors
    /// - `JohariError::InvalidTransition` - Transition not allowed by state machine
    /// - `JohariError::NotFound` - memory_id not in store
    /// - `JohariError::InvalidEmbedderIndex` - embedder_idx >= 13
    /// - `JohariError::StorageError` - Persistence failed
    async fn transition(
        &self,
        memory_id: MemoryId,
        embedder_idx: usize,
        to_quadrant: JohariQuadrant,
        trigger: TransitionTrigger,
    ) -> Result<JohariFingerprint, JohariError>;

    /// Execute multiple transitions atomically.
    ///
    /// All-or-nothing: if any transition is invalid, none are applied.
    /// Transitions are validated in order, then applied in order.
    ///
    /// # Arguments
    /// * `memory_id` - UUID of the memory to transition
    /// * `transitions` - Vec of (embedder_idx, target_quadrant, trigger)
    ///
    /// # Returns
    /// The updated JohariFingerprint after all transitions.
    ///
    /// # Errors
    /// - `JohariError::BatchValidationFailed` - One or more transitions invalid
    async fn transition_batch(
        &self,
        memory_id: MemoryId,
        transitions: Vec<(usize, JohariQuadrant, TransitionTrigger)>,
    ) -> Result<JohariFingerprint, JohariError>;

    /// Find memories matching a quadrant pattern.
    ///
    /// Scans TeleologicalMemoryStore and filters by JohariFingerprint state.
    ///
    /// # Arguments
    /// * `pattern` - Quadrant pattern to match
    /// * `limit` - Maximum number of results
    ///
    /// # Returns
    /// Vector of (MemoryId, JohariFingerprint) pairs matching the pattern.
    ///
    /// # Performance
    /// <10ms for up to 10K memories (uses indexed quadrant scan if available).
    async fn find_by_quadrant(
        &self,
        pattern: QuadrantPattern,
        limit: usize,
    ) -> Result<Vec<(MemoryId, JohariFingerprint)>, JohariError>;

    /// Discover potential blind spots from external signals.
    ///
    /// Compares external signal references to current JohariFingerprint state.
    /// Returns embedders that are Unknown/Hidden but externally referenced.
    ///
    /// # Algorithm
    /// For each embedder where current quadrant is Unknown or Hidden:
    ///   signal_strength = sum of external signal strengths for that embedder
    ///   if signal_strength > threshold (0.5 default):
    ///     Add to blind spot candidates
    ///
    /// # Arguments
    /// * `memory_id` - UUID of the memory to analyze
    /// * `external_signals` - Signals from external sources
    ///
    /// # Returns
    /// Vector of BlindSpotCandidate sorted by signal strength (descending).
    async fn discover_blind_spots(
        &self,
        memory_id: MemoryId,
        external_signals: &[ExternalSignal],
    ) -> Result<Vec<BlindSpotCandidate>, JohariError>;

    /// Get transition statistics for a time range.
    ///
    /// Aggregates:
    /// - Count per transition type (e.g., Hidden→Open: 42)
    /// - Count per trigger (e.g., DreamConsolidation: 15)
    /// - Average transitions per memory
    /// - Most common transition paths
    ///
    /// # Arguments
    /// * `time_range` - Time range to aggregate statistics for
    ///
    /// # Returns
    /// TransitionStats with aggregated metrics.
    async fn get_transition_stats(
        &self,
        time_range: TimeRange,
    ) -> Result<TransitionStats, JohariError>;

    /// Get transition history for a specific memory.
    ///
    /// Returns last N transitions ordered by timestamp descending.
    ///
    /// # Arguments
    /// * `memory_id` - UUID of the memory
    /// * `limit` - Maximum number of transitions to return
    ///
    /// # Returns
    /// Vector of JohariTransition ordered by timestamp (most recent first).
    async fn get_transition_history(
        &self,
        memory_id: MemoryId,
        limit: usize,
    ) -> Result<Vec<JohariTransition>, JohariError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classification_context_from_utl() {
        let delta_s = [0.3; NUM_EMBEDDERS];
        let delta_c = [0.7; NUM_EMBEDDERS];
        let ctx = ClassificationContext::from_utl(delta_s, delta_c);

        assert_eq!(ctx.delta_s, [0.3; NUM_EMBEDDERS]);
        assert_eq!(ctx.delta_c, [0.7; NUM_EMBEDDERS]);
        assert!(ctx.disclosure_intent.iter().all(|&d| d));

        println!(
            "[VERIFIED] test_classification_context_from_utl: Context creation works correctly"
        );
    }

    #[test]
    fn test_classification_context_uniform() {
        let ctx = ClassificationContext::uniform(0.4, 0.6);
        assert!(ctx.delta_s.iter().all(|&s| (s - 0.4).abs() < f32::EPSILON));
        assert!(ctx.delta_c.iter().all(|&c| (c - 0.6).abs() < f32::EPSILON));

        println!("[VERIFIED] test_classification_context_uniform: Uniform context works correctly");
    }

    #[test]
    fn test_classification_context_disclosure() {
        let ctx = ClassificationContext::default()
            .with_disclosure(5, false)
            .with_disclosure(10, false);

        assert!(ctx.disclosure_intent[0]);
        assert!(!ctx.disclosure_intent[5]);
        assert!(!ctx.disclosure_intent[10]);

        println!(
            "[VERIFIED] test_classification_context_disclosure: Disclosure settings work correctly"
        );
    }

    #[test]
    fn test_time_range() {
        let range = TimeRange::last_hours(24);
        assert!(range.end > range.start);

        let range = TimeRange::last_days(7);
        let duration = range.end - range.start;
        assert_eq!(duration.num_days(), 7);

        println!("[VERIFIED] test_time_range: Time range creation works correctly");
    }

    #[test]
    fn test_quadrant_patterns() {
        let mostly_open = QuadrantPattern::mostly_open();
        match mostly_open {
            QuadrantPattern::AtLeast { quadrant, count } => {
                assert_eq!(quadrant, JohariQuadrant::Open);
                assert_eq!(count, 10);
            }
            _ => panic!("Expected AtLeast pattern"),
        }

        let frontier = QuadrantPattern::frontier();
        match frontier {
            QuadrantPattern::AtLeast { quadrant, count } => {
                assert_eq!(quadrant, JohariQuadrant::Unknown);
                assert_eq!(count, 5);
            }
            _ => panic!("Expected AtLeast pattern"),
        }

        println!("[VERIFIED] test_quadrant_patterns: Pattern constructors work correctly");
    }
}
