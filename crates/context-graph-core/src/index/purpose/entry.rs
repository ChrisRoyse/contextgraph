//! Purpose index entry types for teleological alignment indexing.
//!
//! # Overview
//!
//! This module provides the entry types for the Purpose Pattern Index (Stage 4):
//!
//! - `PurposeMetadata`: Metadata about purpose vector computation
//! - `PurposeIndexEntry`: Complete entry for the purpose index with memory ID
//!
//! # Fail-Fast Semantics
//!
//! All operations validate inputs immediately and fail on invalid data:
//! - Confidence must be in [0.0, 1.0]
//! - No silent clamping or fallbacks
//!
//! # Usage
//!
//! ```ignore
//! use context_graph_core::index::purpose::entry::{PurposeMetadata, PurposeIndexEntry};
//! use context_graph_core::types::fingerprint::purpose::PurposeVector;
//! use context_graph_core::purpose::goals::GoalId;
//! use context_graph_core::types::johari::quadrant::JohariQuadrant;
//! use std::time::SystemTime;
//! use uuid::Uuid;
//!
//! // Create metadata with validation
//! let metadata = PurposeMetadata::new(
//!     GoalId::new("master_ml"),
//!     0.85,
//!     JohariQuadrant::Open,
//! )?;
//!
//! // Create entry with all fields
//! let entry = PurposeIndexEntry::new(
//!     Uuid::new_v4(),
//!     PurposeVector::default(),
//!     metadata,
//! );
//!
//! // Access alignments array
//! let alignments: &[f32; 13] = entry.get_alignments();
//! ```

use crate::types::fingerprint::PurposeVector;
use crate::types::JohariQuadrant;

use super::error::{PurposeIndexError, PurposeIndexResult};
use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use uuid::Uuid;

/// String-based goal identifier for purpose indexing.
///
/// This type wraps goal names (like "master_ml", "learn_pytorch") for use in
/// purpose metadata and filtering. It is distinct from `uuid::Uuid` which is
/// used for `GoalNode.id` identifiers.
///
/// # Usage
///
/// ```ignore
/// let goal = GoalId::new("master_machine_learning");
/// assert_eq!(goal.as_str(), "master_machine_learning");
/// ```
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct GoalId(String);

impl GoalId {
    /// Create a new goal ID from a string.
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Get the goal ID as a string slice.
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for GoalId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Metadata about a purpose vector computation.
///
/// Contains information about when and how the purpose vector was computed,
/// including the primary goal it aligns with and the confidence level.
///
/// # Invariants
///
/// - `confidence` is always in the range [0.0, 1.0]
/// - `computed_at` is set at creation time and represents when the vector was computed
///
/// # Fail-Fast
///
/// The `new()` constructor validates confidence immediately and fails on invalid values.
/// There is no clamping - invalid values result in an error.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PurposeMetadata {
    /// The primary goal this memory aligns with.
    ///
    /// Represents the goal from the goal hierarchy that this memory
    /// most strongly contributes to.
    pub primary_goal: GoalId,

    /// Confidence level of the alignment computation.
    ///
    /// Range: [0.0, 1.0]
    /// - 1.0: Perfect confidence in alignment measurement
    /// - 0.0: No confidence (should rarely occur)
    pub confidence: f32,

    /// Timestamp when the purpose vector was computed.
    ///
    /// Used for staleness detection and recomputation scheduling.
    pub computed_at: SystemTime,

    /// The dominant Johari quadrant for this memory.
    ///
    /// Indicates the visibility/awareness classification of the memory
    /// from the Johari Window model.
    pub dominant_quadrant: JohariQuadrant,
}

impl PurposeMetadata {
    /// Create new purpose metadata with validation.
    ///
    /// # Arguments
    ///
    /// * `primary_goal` - The goal this memory primarily aligns with
    /// * `confidence` - Confidence level in [0.0, 1.0]
    /// * `dominant_quadrant` - The Johari quadrant classification
    ///
    /// # Errors
    ///
    /// Returns `PurposeIndexError::InvalidConfidence` if confidence is not in [0.0, 1.0].
    ///
    /// # Fail-Fast
    ///
    /// This constructor validates immediately. Invalid confidence values
    /// are NOT clamped - they result in an error.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use context_graph_core::index::purpose::entry::PurposeMetadata;
    /// use context_graph_core::purpose::goals::GoalId;
    /// use context_graph_core::types::johari::quadrant::JohariQuadrant;
    ///
    /// // Valid confidence
    /// let metadata = PurposeMetadata::new(
    ///     GoalId::new("learn_pytorch"),
    ///     0.85,
    ///     JohariQuadrant::Open,
    /// ).expect("Valid metadata");
    ///
    /// // Invalid confidence fails immediately
    /// let result = PurposeMetadata::new(
    ///     GoalId::new("learn_pytorch"),
    ///     1.5, // INVALID
    ///     JohariQuadrant::Open,
    /// );
    /// assert!(result.is_err());
    /// ```
    pub fn new(
        primary_goal: GoalId,
        confidence: f32,
        dominant_quadrant: JohariQuadrant,
    ) -> PurposeIndexResult<Self> {
        // Fail-fast validation: confidence must be in [0.0, 1.0]
        if !(0.0..=1.0).contains(&confidence) {
            return Err(PurposeIndexError::invalid_confidence(
                confidence,
                "confidence must be in range [0.0, 1.0]",
            ));
        }

        Ok(Self {
            primary_goal,
            confidence,
            computed_at: SystemTime::now(),
            dominant_quadrant,
        })
    }

    /// Create metadata with a specific computed_at timestamp.
    ///
    /// Used primarily for deserialization and testing.
    ///
    /// # Errors
    ///
    /// Returns `PurposeIndexError::InvalidConfidence` if confidence is not in [0.0, 1.0].
    pub fn with_timestamp(
        primary_goal: GoalId,
        confidence: f32,
        computed_at: SystemTime,
        dominant_quadrant: JohariQuadrant,
    ) -> PurposeIndexResult<Self> {
        // Fail-fast validation: confidence must be in [0.0, 1.0]
        if !(0.0..=1.0).contains(&confidence) {
            return Err(PurposeIndexError::invalid_confidence(
                confidence,
                "confidence must be in range [0.0, 1.0]",
            ));
        }

        Ok(Self {
            primary_goal,
            confidence,
            computed_at,
            dominant_quadrant,
        })
    }

    /// Check if the metadata is stale (older than the given duration).
    ///
    /// # Arguments
    ///
    /// * `max_age` - Maximum age before metadata is considered stale
    ///
    /// # Returns
    ///
    /// `true` if the metadata was computed more than `max_age` ago.
    pub fn is_stale(&self, max_age: std::time::Duration) -> bool {
        match self.computed_at.elapsed() {
            Ok(elapsed) => elapsed > max_age,
            Err(_) => false, // Clock went backwards, treat as not stale
        }
    }
}

/// An entry in the Purpose Pattern Index.
///
/// Combines a memory's UUID with its purpose vector and associated metadata
/// for efficient teleological alignment queries.
///
/// # Structure
///
/// Each entry contains:
/// - `memory_id`: Unique identifier for the memory
/// - `purpose_vector`: 13D alignment vector (one dimension per embedder)
/// - `metadata`: Computation metadata including goal, confidence, timestamp
///
/// # Usage
///
/// Entries are stored in the Purpose HNSW index (Stage 4) for
/// similarity search based on teleological alignment patterns.
#[derive(Clone, Debug)]
pub struct PurposeIndexEntry {
    /// Unique identifier for the memory this entry represents.
    pub memory_id: Uuid,

    /// The 13D purpose vector with alignment values for each embedder.
    ///
    /// See `PurposeVector` for details on structure and computation.
    pub purpose_vector: PurposeVector,

    /// Metadata about the purpose vector computation.
    pub metadata: PurposeMetadata,
}

impl PurposeIndexEntry {
    /// Create a new purpose index entry with all fields.
    ///
    /// # Arguments
    ///
    /// * `memory_id` - Unique identifier for the memory
    /// * `purpose_vector` - The computed 13D purpose vector
    /// * `metadata` - Metadata about the computation
    ///
    /// # Example
    ///
    /// ```ignore
    /// use context_graph_core::index::purpose::entry::{PurposeMetadata, PurposeIndexEntry};
    /// use context_graph_core::types::fingerprint::purpose::PurposeVector;
    /// use context_graph_core::purpose::goals::GoalId;
    /// use context_graph_core::types::johari::quadrant::JohariQuadrant;
    /// use uuid::Uuid;
    ///
    /// let metadata = PurposeMetadata::new(
    ///     GoalId::new("master_ml"),
    ///     0.9,
    ///     JohariQuadrant::Open,
    /// ).unwrap();
    ///
    /// let alignments = [0.8, 0.7, 0.9, 0.6, 0.75, 0.65, 0.85, 0.72, 0.78, 0.68, 0.82, 0.71, 0.76];
    /// let purpose_vector = PurposeVector::new(alignments);
    ///
    /// let entry = PurposeIndexEntry::new(
    ///     Uuid::new_v4(),
    ///     purpose_vector,
    ///     metadata,
    /// );
    /// ```
    pub fn new(memory_id: Uuid, purpose_vector: PurposeVector, metadata: PurposeMetadata) -> Self {
        Self {
            memory_id,
            purpose_vector,
            metadata,
        }
    }

    /// Get the 13D alignment values from the purpose vector.
    ///
    /// Returns a reference to the fixed-size array of alignment values,
    /// one for each embedder (E1-E13).
    ///
    /// # Returns
    ///
    /// Reference to the 13-element alignment array where each value
    /// is the cosine similarity to the North Star goal for that embedder.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let entry = PurposeIndexEntry::new(/* ... */);
    /// let alignments: &[f32; 13] = entry.get_alignments();
    ///
    /// // Access individual embedder alignments
    /// let e1_alignment = alignments[0];  // OpenAI
    /// let e2_alignment = alignments[1];  // Voyage
    /// ```
    #[inline]
    pub fn get_alignments(&self) -> &[f32; 13] {
        &self.purpose_vector.alignments
    }

    /// Get the aggregate (mean) alignment across all embedders.
    ///
    /// This is a convenience method that delegates to `PurposeVector::aggregate_alignment`.
    #[inline]
    pub fn aggregate_alignment(&self) -> f32 {
        self.purpose_vector.aggregate_alignment()
    }

    /// Get the dominant embedder index.
    ///
    /// Returns the index (0-12) of the embedder with the highest alignment.
    #[inline]
    pub fn dominant_embedder(&self) -> u8 {
        self.purpose_vector.dominant_embedder
    }

    /// Get the coherence score.
    ///
    /// Measures how much the embedders agree on alignment direction.
    #[inline]
    pub fn coherence(&self) -> f32 {
        self.purpose_vector.coherence
    }

    /// Check if the entry's metadata is stale.
    ///
    /// # Arguments
    ///
    /// * `max_age` - Maximum age before the entry is considered stale
    pub fn is_stale(&self, max_age: std::time::Duration) -> bool {
        self.metadata.is_stale(max_age)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== PurposeMetadata Tests =====

    #[test]
    fn test_purpose_metadata_new_valid() {
        let goal = GoalId::new("master_ml");
        let result = PurposeMetadata::new(goal.clone(), 0.85, JohariQuadrant::Open);

        assert!(result.is_ok());
        let metadata = result.unwrap();

        assert_eq!(metadata.primary_goal.as_str(), "master_ml");
        assert!((metadata.confidence - 0.85).abs() < f32::EPSILON);
        assert_eq!(metadata.dominant_quadrant, JohariQuadrant::Open);

        // Verify computed_at is recent
        let elapsed = metadata.computed_at.elapsed().unwrap();
        assert!(elapsed.as_secs() < 1);

        println!("[VERIFIED] PurposeMetadata::new creates valid metadata with confidence=0.85");
    }

    #[test]
    fn test_purpose_metadata_valid_boundary_values() {
        // Test confidence = 0.0 (valid minimum)
        let result = PurposeMetadata::new(GoalId::new("test"), 0.0, JohariQuadrant::Hidden);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().confidence, 0.0);

        // Test confidence = 1.0 (valid maximum)
        let result = PurposeMetadata::new(GoalId::new("test"), 1.0, JohariQuadrant::Blind);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().confidence, 1.0);

        // Test confidence = 0.5 (valid middle)
        let result = PurposeMetadata::new(GoalId::new("test"), 0.5, JohariQuadrant::Unknown);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().confidence, 0.5);

        println!("[VERIFIED] PurposeMetadata accepts valid boundary values 0.0, 0.5, 1.0");
    }

    #[test]
    fn test_purpose_metadata_invalid_confidence_over() {
        // FAIL FAST: confidence > 1.0 must fail immediately
        let result = PurposeMetadata::new(GoalId::new("test"), 1.5, JohariQuadrant::Open);

        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = err.to_string();

        assert!(msg.contains("1.5"));
        assert!(msg.contains("confidence"));
        println!(
            "[VERIFIED] FAIL FAST: PurposeMetadata::new rejects confidence=1.5 with error: {}",
            msg
        );
    }

    #[test]
    fn test_purpose_metadata_invalid_confidence_under() {
        // FAIL FAST: confidence < 0.0 must fail immediately
        let result = PurposeMetadata::new(GoalId::new("test"), -0.1, JohariQuadrant::Open);

        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = err.to_string();

        assert!(msg.contains("-0.1"));
        assert!(msg.contains("confidence"));
        println!(
            "[VERIFIED] FAIL FAST: PurposeMetadata::new rejects confidence=-0.1 with error: {}",
            msg
        );
    }

    #[test]
    fn test_purpose_metadata_invalid_confidence_nan() {
        // FAIL FAST: NaN must fail
        let result = PurposeMetadata::new(GoalId::new("test"), f32::NAN, JohariQuadrant::Open);

        assert!(result.is_err());
        println!("[VERIFIED] FAIL FAST: PurposeMetadata::new rejects NaN confidence");
    }

    #[test]
    fn test_purpose_metadata_invalid_confidence_infinity() {
        // FAIL FAST: Infinity must fail
        let result = PurposeMetadata::new(GoalId::new("test"), f32::INFINITY, JohariQuadrant::Open);
        assert!(result.is_err());

        let result =
            PurposeMetadata::new(GoalId::new("test"), f32::NEG_INFINITY, JohariQuadrant::Open);
        assert!(result.is_err());

        println!("[VERIFIED] FAIL FAST: PurposeMetadata::new rejects infinite confidence values");
    }

    #[test]
    fn test_purpose_metadata_with_timestamp() {
        use std::time::Duration;

        let past = SystemTime::now() - Duration::from_secs(3600);
        let result = PurposeMetadata::with_timestamp(
            GoalId::new("test"),
            0.75,
            past,
            JohariQuadrant::Unknown,
        );

        assert!(result.is_ok());
        let metadata = result.unwrap();
        assert_eq!(metadata.computed_at, past);

        println!("[VERIFIED] PurposeMetadata::with_timestamp sets custom timestamp correctly");
    }

    #[test]
    fn test_purpose_metadata_is_stale() {
        use std::time::Duration;

        let past = SystemTime::now() - Duration::from_secs(3600);
        let metadata =
            PurposeMetadata::with_timestamp(GoalId::new("test"), 0.75, past, JohariQuadrant::Open)
                .unwrap();

        // 1 hour old, max age 30 min = stale
        assert!(metadata.is_stale(Duration::from_secs(1800)));

        // 1 hour old, max age 2 hours = not stale
        assert!(!metadata.is_stale(Duration::from_secs(7200)));

        println!("[VERIFIED] PurposeMetadata::is_stale correctly detects stale metadata");
    }

    #[test]
    fn test_purpose_metadata_serialization() {
        let metadata =
            PurposeMetadata::new(GoalId::new("test_goal"), 0.75, JohariQuadrant::Hidden).unwrap();

        let json = serde_json::to_string(&metadata).expect("Serialization should work");
        assert!(json.contains("test_goal"));
        assert!(json.contains("0.75"));
        assert!(json.contains("hidden"));

        let deserialized: PurposeMetadata =
            serde_json::from_str(&json).expect("Deserialization should work");
        assert_eq!(deserialized.primary_goal.as_str(), "test_goal");
        assert!((deserialized.confidence - 0.75).abs() < f32::EPSILON);
        assert_eq!(deserialized.dominant_quadrant, JohariQuadrant::Hidden);

        println!("[VERIFIED] PurposeMetadata serializes and deserializes correctly");
    }

    // ===== PurposeIndexEntry Tests =====

    #[test]
    fn test_purpose_index_entry_new() {
        let memory_id = Uuid::new_v4();
        let alignments = [
            0.8, 0.7, 0.9, 0.6, 0.75, 0.65, 0.85, 0.72, 0.78, 0.68, 0.82, 0.71, 0.76,
        ];
        let purpose_vector = PurposeVector::new(alignments);
        let metadata =
            PurposeMetadata::new(GoalId::new("master_ml"), 0.9, JohariQuadrant::Open).unwrap();

        let entry = PurposeIndexEntry::new(memory_id, purpose_vector.clone(), metadata);

        assert_eq!(entry.memory_id, memory_id);
        assert_eq!(entry.purpose_vector.alignments, alignments);
        assert_eq!(entry.metadata.primary_goal.as_str(), "master_ml");
        assert!((entry.metadata.confidence - 0.9).abs() < f32::EPSILON);

        println!("[VERIFIED] PurposeIndexEntry::new creates entry with all fields correctly");
    }

    #[test]
    fn test_purpose_index_entry_get_alignments() {
        let alignments = [
            0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 0.11, 0.22, 0.33,
        ];
        let purpose_vector = PurposeVector::new(alignments);
        let metadata =
            PurposeMetadata::new(GoalId::new("test"), 0.5, JohariQuadrant::Unknown).unwrap();

        let entry = PurposeIndexEntry::new(Uuid::new_v4(), purpose_vector, metadata);

        let retrieved = entry.get_alignments();
        assert_eq!(retrieved.len(), 13);
        assert_eq!(*retrieved, alignments);

        // Verify specific values
        assert!((retrieved[0] - 0.1).abs() < f32::EPSILON);
        assert!((retrieved[9] - 1.0).abs() < f32::EPSILON);

        println!("[VERIFIED] PurposeIndexEntry::get_alignments returns correct 13-element array");
    }

    #[test]
    fn test_purpose_index_entry_aggregate_alignment() {
        let uniform = PurposeVector::new([0.75; 13]);
        let metadata =
            PurposeMetadata::new(GoalId::new("test"), 0.8, JohariQuadrant::Open).unwrap();

        let entry = PurposeIndexEntry::new(Uuid::new_v4(), uniform, metadata);

        let aggregate = entry.aggregate_alignment();
        assert!((aggregate - 0.75).abs() < f32::EPSILON);

        println!("[VERIFIED] PurposeIndexEntry::aggregate_alignment returns correct mean");
    }

    #[test]
    fn test_purpose_index_entry_dominant_embedder() {
        let mut alignments = [0.5; 13];
        alignments[7] = 0.95; // E8 is dominant

        let purpose_vector = PurposeVector::new(alignments);
        let metadata =
            PurposeMetadata::new(GoalId::new("test"), 0.8, JohariQuadrant::Blind).unwrap();

        let entry = PurposeIndexEntry::new(Uuid::new_v4(), purpose_vector, metadata);

        assert_eq!(entry.dominant_embedder(), 7);

        println!("[VERIFIED] PurposeIndexEntry::dominant_embedder returns correct index");
    }

    #[test]
    fn test_purpose_index_entry_coherence() {
        // Uniform alignments = perfect coherence
        let uniform = PurposeVector::new([0.8; 13]);
        let metadata =
            PurposeMetadata::new(GoalId::new("test"), 0.8, JohariQuadrant::Open).unwrap();

        let entry = PurposeIndexEntry::new(Uuid::new_v4(), uniform, metadata);

        let coherence = entry.coherence();
        // Use 1e-6 tolerance like the original PurposeVector tests
        assert!((coherence - 1.0).abs() < 1e-6);

        println!("[VERIFIED] PurposeIndexEntry::coherence returns correct value");
    }

    #[test]
    fn test_purpose_index_entry_is_stale() {
        use std::time::Duration;

        let past = SystemTime::now() - Duration::from_secs(7200); // 2 hours ago
        let metadata = PurposeMetadata::with_timestamp(
            GoalId::new("test"),
            0.75,
            past,
            JohariQuadrant::Hidden,
        )
        .unwrap();

        let entry = PurposeIndexEntry::new(Uuid::new_v4(), PurposeVector::default(), metadata);

        // Entry is 2 hours old
        assert!(entry.is_stale(Duration::from_secs(3600))); // Stale after 1 hour
        assert!(!entry.is_stale(Duration::from_secs(10800))); // Not stale if max is 3 hours

        println!("[VERIFIED] PurposeIndexEntry::is_stale correctly detects staleness");
    }

    #[test]
    fn test_purpose_index_entry_clone() {
        let entry = PurposeIndexEntry::new(
            Uuid::new_v4(),
            PurposeVector::default(),
            PurposeMetadata::new(GoalId::new("clone_test"), 0.5, JohariQuadrant::Unknown).unwrap(),
        );

        let cloned = entry.clone();

        assert_eq!(cloned.memory_id, entry.memory_id);
        assert_eq!(
            cloned.purpose_vector.alignments,
            entry.purpose_vector.alignments
        );
        assert_eq!(
            cloned.metadata.primary_goal.as_str(),
            entry.metadata.primary_goal.as_str()
        );

        println!("[VERIFIED] PurposeIndexEntry implements Clone correctly");
    }

    #[test]
    fn test_purpose_index_entry_debug() {
        let entry = PurposeIndexEntry::new(
            Uuid::nil(),
            PurposeVector::default(),
            PurposeMetadata::new(GoalId::new("debug_test"), 0.5, JohariQuadrant::Open).unwrap(),
        );

        let debug_str = format!("{:?}", entry);

        assert!(debug_str.contains("PurposeIndexEntry"));
        assert!(debug_str.contains("memory_id"));

        println!("[VERIFIED] PurposeIndexEntry implements Debug correctly");
    }

    // ===== Integration Tests =====

    #[test]
    fn test_entry_with_all_quadrants() {
        for quadrant in JohariQuadrant::all() {
            let metadata =
                PurposeMetadata::new(GoalId::new("multi_quadrant_test"), 0.8, quadrant).unwrap();

            assert_eq!(metadata.dominant_quadrant, quadrant);

            let entry = PurposeIndexEntry::new(Uuid::new_v4(), PurposeVector::default(), metadata);

            assert_eq!(entry.metadata.dominant_quadrant, quadrant);
        }

        println!("[VERIFIED] PurposeIndexEntry works with all JohariQuadrant variants");
    }
}
