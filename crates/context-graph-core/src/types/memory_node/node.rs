//! MemoryNode struct representing a stored memory unit in the knowledge graph.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{
    EmbeddingVector, NodeId, NodeMetadata, ValidationError, DEFAULT_EMBEDDING_DIM, MAX_CONTENT_SIZE,
};
use crate::types::JohariQuadrant;

/// A memory node representing a single knowledge unit in the Context Graph.
///
/// # Performance Characteristics
/// - Average serialized size: ~6.5KB (with embedding)
/// - Insert latency target: <1ms
/// - Retrieval latency target: <500us
///
/// # PRD Section 4.1 KnowledgeNode Mapping
/// - `id`: UUID v4 unique identifier
/// - `content`: str[<=65536] - actual stored knowledge (max 1MB enforced)
/// - `embedding`: Vec1536 - dense vector representation
/// - `quadrant`: Johari Window classification
/// - `importance`: f32 \[0,1\] - relevance score
/// - `emotional_valence`: f32[-1,1] - emotional charge
/// - `created_at`: Creation timestamp
/// - `accessed_at`: Last access timestamp
/// - `access_count`: Number of accesses
/// - `metadata`: Rich metadata container
///
/// # Constitution Compliance
/// - AP-009: All f32 fields must be validated (no NaN/Infinity)
/// - SEC-06: Soft delete via metadata.deleted flag
/// - Naming: snake_case fields per constitution.yaml
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryNode {
    /// Unique identifier for this node (UUID v4).
    pub id: NodeId,

    /// The content/knowledge stored in this node (max 1MB).
    pub content: String,

    /// Dense embedding vector (1536 dimensions by default).
    pub embedding: EmbeddingVector,

    /// Johari Window quadrant classification.
    pub quadrant: JohariQuadrant,

    /// Importance/relevance score [0.0, 1.0].
    pub importance: f32,

    /// Emotional valence [-1.0, 1.0].
    /// Negative = negative emotion, Positive = positive emotion.
    pub emotional_valence: f32,

    /// Timestamp when this node was created.
    pub created_at: DateTime<Utc>,

    /// Timestamp when this node was last accessed.
    pub accessed_at: DateTime<Utc>,

    /// Number of times this node has been accessed.
    pub access_count: u64,

    /// Rich metadata container.
    pub metadata: NodeMetadata,
}

impl MemoryNode {
    /// Tolerance for embedding normalization check (magnitude must be in [0.99, 1.01])
    const NORMALIZATION_TOLERANCE: f64 = 0.01;

    /// Consolidation threshold score (weighted score >= 0.7 triggers consolidation)
    const CONSOLIDATION_THRESHOLD: f32 = 0.7;

    /// Create a new memory node with the given content and embedding.
    ///
    /// # Arguments
    /// * `content` - The content/knowledge to store
    /// * `embedding` - The embedding vector (should be 1536 dimensions)
    ///
    /// # Default Values
    /// - `importance`: 0.5
    /// - `emotional_valence`: 0.0 (neutral)
    /// - `quadrant`: JohariQuadrant::Open
    /// - `access_count`: 0
    pub fn new(content: String, embedding: EmbeddingVector) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            content,
            embedding,
            quadrant: JohariQuadrant::default(),
            importance: 0.5,
            emotional_valence: 0.0,
            created_at: now,
            accessed_at: now,
            access_count: 0,
            metadata: NodeMetadata::default(),
        }
    }

    /// Create a new MemoryNode with a specific ID.
    ///
    /// # Arguments
    /// * `id` - The specific NodeId (UUID) to use
    /// * `content` - The content to store
    /// * `embedding` - The embedding vector (should be 1536 dimensions)
    pub fn with_id(id: NodeId, content: String, embedding: EmbeddingVector) -> Self {
        let mut node = Self::new(content, embedding);
        node.id = id;
        node
    }

    /// Record an access to this node, updating accessed_at and incrementing access_count.
    ///
    /// Uses saturating_add to prevent overflow (will stay at u64::MAX if at limit).
    pub fn record_access(&mut self) {
        self.accessed_at = Utc::now();
        self.access_count = self.access_count.saturating_add(1);
    }

    /// Get the age of this node in seconds since creation.
    ///
    /// # Returns
    /// Number of seconds since node creation (always >= 0).
    pub fn age_seconds(&self) -> i64 {
        (Utc::now() - self.created_at).num_seconds()
    }

    /// Get the time since last access in seconds.
    ///
    /// # Returns
    /// Number of seconds since last access (always >= 0).
    pub fn time_since_access_seconds(&self) -> i64 {
        (Utc::now() - self.accessed_at).num_seconds()
    }

    /// Compute memory decay using modified Ebbinghaus forgetting curve.
    ///
    /// Formula: R = e^(-t / (S * k * 24))
    /// Where:
    /// - R = retention (0.0 to 1.0)
    /// - t = time since access in hours
    /// - S = memory strength: 1 + ln(access_count + 1)
    /// - k = importance factor: 1 + importance
    /// - 24 = baseline decay period in hours
    ///
    /// # Returns
    /// Retention value between 0.0 (forgotten) and 1.0 (fully retained).
    ///
    /// # Constitution Compliance
    /// - AP-009: Result is clamped to [0.0, 1.0] to prevent NaN/Infinity
    pub fn compute_decay(&self) -> f32 {
        let t_hours = self.time_since_access_seconds() as f64 / 3600.0;
        let strength = 1.0 + ((self.access_count as f64) + 1.0).ln().max(0.0);
        let k = 1.0 + self.importance as f64;
        let decay = (-t_hours / (strength * k * 24.0)).exp();
        decay.clamp(0.0, 1.0) as f32
    }

    /// Determine if this node should be consolidated based on weighted score.
    ///
    /// Score = 0.4 * importance + 0.3 * (1 - decay) + 0.3 * access_frequency
    /// Where access_frequency = accesses per hour, clamped to [0, 1]
    ///
    /// # Returns
    /// `true` if score >= CONSOLIDATION_THRESHOLD (0.7)
    pub fn should_consolidate(&self) -> bool {
        let decay = self.compute_decay();
        let age_hours = (self.age_seconds().max(1) as f32) / 3600.0;
        let access_freq = ((self.access_count as f32) / age_hours).min(1.0);
        let score = 0.4 * self.importance + 0.3 * (1.0 - decay) + 0.3 * access_freq;
        score >= Self::CONSOLIDATION_THRESHOLD
    }

    /// Validate all node constraints.
    ///
    /// # Checks (in order)
    /// 1. Embedding dimension is 1536
    /// 2. Importance is in [0.0, 1.0]
    /// 3. Emotional valence is in [-1.0, 1.0]
    /// 4. Content size is <= 1MB (1,048,576 bytes)
    /// 5. Embedding is normalized (magnitude within +/-0.01 of 1.0)
    ///
    /// # Returns
    /// `Ok(())` if all validations pass, `Err(ValidationError)` on first failure.
    ///
    /// # Constitution Compliance
    /// - AP-009: Validates numeric values to prevent NaN/Infinity propagation
    pub fn validate(&self) -> Result<(), ValidationError> {
        // 1. Check embedding dimension
        if self.embedding.len() != DEFAULT_EMBEDDING_DIM {
            return Err(ValidationError::InvalidEmbeddingDimension {
                expected: DEFAULT_EMBEDDING_DIM,
                actual: self.embedding.len(),
            });
        }

        // 2. Check importance range [0.0, 1.0]
        if self.importance < 0.0 || self.importance > 1.0 || self.importance.is_nan() {
            return Err(ValidationError::OutOfBounds {
                field: "importance".to_string(),
                value: self.importance as f64,
                min: 0.0,
                max: 1.0,
            });
        }

        // 3. Check emotional valence range [-1.0, 1.0]
        if self.emotional_valence < -1.0
            || self.emotional_valence > 1.0
            || self.emotional_valence.is_nan()
        {
            return Err(ValidationError::OutOfBounds {
                field: "emotional_valence".to_string(),
                value: self.emotional_valence as f64,
                min: -1.0,
                max: 1.0,
            });
        }

        // 4. Check content size <= 1MB
        if self.content.len() > MAX_CONTENT_SIZE {
            return Err(ValidationError::ContentTooLarge {
                size: self.content.len(),
                max_size: MAX_CONTENT_SIZE,
            });
        }

        // 5. Check embedding normalization (magnitude ~= 1.0)
        let magnitude: f64 = self
            .embedding
            .iter()
            .map(|x| (*x as f64).powi(2))
            .sum::<f64>()
            .sqrt();

        if (magnitude - 1.0).abs() > Self::NORMALIZATION_TOLERANCE {
            return Err(ValidationError::EmbeddingNotNormalized { magnitude });
        }

        Ok(())
    }
}

impl Default for MemoryNode {
    /// Create a default MemoryNode with empty content and zero-filled embedding.
    ///
    /// NOTE: Default creates a node that will FAIL validation because
    /// zero-filled embedding is not normalized. Use for testing only.
    fn default() -> Self {
        Self::new(String::new(), vec![0.0; DEFAULT_EMBEDDING_DIM])
    }
}
