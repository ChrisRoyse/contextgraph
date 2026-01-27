//! CausalRelationship type for storing LLM-generated causal descriptions with provenance.
//!
//! This module defines the [`CausalRelationship`] struct that stores:
//! - LLM-generated 1-3 paragraph causal descriptions
//! - E1 semantic embedding of the description (1024D) for search
//! - Full provenance linking back to source content and fingerprint
//!
//! # Storage
//!
//! CausalRelationships are stored in dedicated RocksDB column families:
//! - `CF_CAUSAL_RELATIONSHIPS`: Primary storage by UUID
//! - `CF_CAUSAL_BY_SOURCE`: Secondary index by source fingerprint ID
//!
//! # Search
//!
//! The description_embedding enables "apples-to-apples" semantic search
//! against other causal descriptions, avoiding the cross-embedder comparison
//! issues that arise when mixing different embedding types.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A causal relationship identified by LLM analysis.
///
/// Contains the LLM-generated description, its embedding for search,
/// and full provenance linking back to the source content.
///
/// # Fields
///
/// - `id`: Unique identifier for this causal relationship
/// - `description`: LLM-generated 1-3 paragraph explanation
/// - `description_embedding`: E1 1024D embedding for semantic search
/// - `source_content`: Original text this was derived from (PROVENANCE)
/// - `source_fingerprint_id`: UUID of the source memory (PROVENANCE)
/// - `direction`: "cause", "effect", or "bidirectional"
/// - `confidence`: LLM confidence score [0.0, 1.0]
/// - `key_phrases`: Causal markers detected in the source
/// - `created_at`: Unix timestamp when relationship was identified
///
/// # Example
///
/// ```ignore
/// let rel = CausalRelationship::new(
///     "High cortisol causes memory impairment...".to_string(),
///     e1_embedding,
///     "Chronic stress elevates cortisol...".to_string(),
///     source_id,
///     "cause".to_string(),
///     0.9,
///     vec!["causes".to_string(), "leads to".to_string()],
/// );
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalRelationship {
    /// Unique ID for this causal relationship.
    pub id: Uuid,

    /// LLM-generated description (1-3 paragraphs).
    ///
    /// Structure:
    /// - Paragraph 1: What is the causal relationship
    /// - Paragraph 2: Mechanism/evidence details
    /// - Paragraph 3: Implications/context
    pub description: String,

    /// E1 semantic embedding of the description (1024D).
    ///
    /// Enables "apples-to-apples" search against other descriptions.
    /// Generated using the same E1 embedder as regular memories.
    pub description_embedding: Vec<f32>,

    /// PROVENANCE: Original source content this was derived from.
    ///
    /// Stored in full to enable verification without additional lookups.
    pub source_content: String,

    /// PROVENANCE: Fingerprint ID of the source memory.
    ///
    /// Links to the full 13-embedder TeleologicalFingerprint.
    pub source_fingerprint_id: Uuid,

    /// Direction: "cause", "effect", or "bidirectional".
    ///
    /// Indicates the causal direction of the source content:
    /// - "cause": Source describes something that causes effects
    /// - "effect": Source describes something that is an effect
    /// - "bidirectional": Source describes both cause and effect
    pub direction: String,

    /// LLM confidence score [0.0, 1.0].
    pub confidence: f32,

    /// Key causal phrases detected in the source.
    pub key_phrases: Vec<String>,

    /// Unix timestamp when relationship was identified.
    pub created_at: i64,
}

impl CausalRelationship {
    /// Create a new causal relationship.
    ///
    /// # Arguments
    ///
    /// * `description` - LLM-generated 1-3 paragraph explanation
    /// * `description_embedding` - E1 1024D embedding of the description
    /// * `source_content` - Original text this was derived from
    /// * `source_fingerprint_id` - UUID of the source memory
    /// * `direction` - "cause", "effect", or "bidirectional"
    /// * `confidence` - LLM confidence score [0.0, 1.0]
    /// * `key_phrases` - Causal markers detected
    pub fn new(
        description: String,
        description_embedding: Vec<f32>,
        source_content: String,
        source_fingerprint_id: Uuid,
        direction: String,
        confidence: f32,
        key_phrases: Vec<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            description,
            description_embedding,
            source_content,
            source_fingerprint_id,
            direction,
            confidence: confidence.clamp(0.0, 1.0),
            key_phrases,
            created_at: chrono::Utc::now().timestamp(),
        }
    }

    /// Create with a specific ID (for testing or migration).
    pub fn with_id(
        id: Uuid,
        description: String,
        description_embedding: Vec<f32>,
        source_content: String,
        source_fingerprint_id: Uuid,
        direction: String,
        confidence: f32,
        key_phrases: Vec<String>,
    ) -> Self {
        Self {
            id,
            description,
            description_embedding,
            source_content,
            source_fingerprint_id,
            direction,
            confidence: confidence.clamp(0.0, 1.0),
            key_phrases,
            created_at: chrono::Utc::now().timestamp(),
        }
    }

    /// Check if the description embedding has the expected dimension.
    ///
    /// E1 embeddings should be 1024D per constitution.yaml.
    pub fn has_valid_embedding(&self) -> bool {
        self.description_embedding.len() == 1024
    }

    /// Get the description embedding as a slice.
    pub fn embedding(&self) -> &[f32] {
        &self.description_embedding
    }

    /// Check if this is a high-confidence relationship (>= 0.7).
    pub fn is_high_confidence(&self) -> bool {
        self.confidence >= 0.7
    }

    /// Get the direction as a normalized string.
    pub fn normalized_direction(&self) -> &str {
        match self.direction.to_lowercase().as_str() {
            "cause" | "causes" | "causal" => "cause",
            "effect" | "effects" | "result" => "effect",
            "bidirectional" | "both" | "mutual" => "bidirectional",
            _ => "unknown",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_causal_relationship_new() {
        let embedding = vec![0.0_f32; 1024];
        let source_id = Uuid::new_v4();

        let rel = CausalRelationship::new(
            "Test description".to_string(),
            embedding.clone(),
            "Source content".to_string(),
            source_id,
            "cause".to_string(),
            0.9,
            vec!["causes".to_string()],
        );

        assert!(!rel.id.is_nil());
        assert_eq!(rel.description, "Test description");
        assert_eq!(rel.source_fingerprint_id, source_id);
        assert_eq!(rel.direction, "cause");
        assert!((rel.confidence - 0.9).abs() < 0.01);
        assert!(rel.has_valid_embedding());
    }

    #[test]
    fn test_confidence_clamping() {
        let embedding = vec![0.0_f32; 1024];
        let source_id = Uuid::new_v4();

        let rel = CausalRelationship::new(
            "Test".to_string(),
            embedding,
            "Source".to_string(),
            source_id,
            "cause".to_string(),
            1.5, // Exceeds max
            vec![],
        );

        assert_eq!(rel.confidence, 1.0);
    }

    #[test]
    fn test_normalized_direction() {
        let embedding = vec![0.0_f32; 1024];
        let source_id = Uuid::new_v4();

        let rel = CausalRelationship::new(
            "Test".to_string(),
            embedding,
            "Source".to_string(),
            source_id,
            "CAUSES".to_string(), // Different case
            0.8,
            vec![],
        );

        assert_eq!(rel.normalized_direction(), "cause");
    }

    #[test]
    fn test_is_high_confidence() {
        let embedding = vec![0.0_f32; 1024];
        let source_id = Uuid::new_v4();

        let high = CausalRelationship::new(
            "Test".to_string(),
            embedding.clone(),
            "Source".to_string(),
            source_id,
            "cause".to_string(),
            0.8,
            vec![],
        );

        let low = CausalRelationship::new(
            "Test".to_string(),
            embedding,
            "Source".to_string(),
            source_id,
            "cause".to_string(),
            0.5,
            vec![],
        );

        assert!(high.is_high_confidence());
        assert!(!low.is_high_confidence());
    }
}
