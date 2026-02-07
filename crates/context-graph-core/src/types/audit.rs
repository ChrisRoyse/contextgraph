//! Audit log types for append-only provenance tracking.
//!
//! This module defines the core types for the audit log infrastructure.
//! All operations on memories (create, merge, delete, boost, etc.) produce
//! an `AuditRecord` that is stored in an append-only log backed by RocksDB.
//!
//! # Design Principles
//!
//! - **Append-only**: Records are never updated or deleted once written.
//! - **Chronological ordering**: Primary key is `{timestamp_nanos}_{uuid}` for
//!   natural time-ordered iteration.
//! - **Secondary indexes**: Per-target and per-operator indexes for efficient queries.
//! - **Full provenance**: Every mutation captures who, what, when, why, and previous state.
//!
//! # Key Format
//!
//! - Primary: `{timestamp_nanos_be}_{uuid_bytes}` (8 + 16 = 24 bytes)
//! - By target: `{target_uuid_bytes}_{timestamp_nanos_be}` (16 + 8 = 24 bytes)
//!
//! # Usage
//!
//! ```rust
//! use context_graph_core::types::audit::{AuditRecord, AuditOperation, AuditResult};
//! use uuid::Uuid;
//!
//! let record = AuditRecord::new(
//!     AuditOperation::MemoryCreated,
//!     Uuid::new_v4(),
//! );
//! assert!(matches!(record.result, AuditResult::Success));
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// Merge History (Phase 4, item 5.10)
// ============================================================================

/// Permanent merge history record (Phase 4, item 5.10).
///
/// Unlike ReversalRecord which expires after 30 days, MergeRecord is permanent.
/// Stored in CF_MERGE_HISTORY for complete lineage tracking.
///
/// # Key Format (CF_MERGE_HISTORY)
///
/// `{merged_uuid_bytes}_{timestamp_nanos_be}` (16 + 8 = 24 bytes)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeRecord {
    /// Unique identifier for this merge record
    pub id: Uuid,
    /// UUID of the newly created merged fingerprint
    pub merged_id: Uuid,
    /// UUIDs of the source memories that were merged
    pub source_ids: Vec<Uuid>,
    /// The merge strategy used (e.g., "Union", "Intersection", "WeightedAverage")
    pub strategy: String,
    /// Why the merge was performed
    pub rationale: String,
    /// Who initiated the merge
    pub operator_id: Option<String>,
    /// When the merge occurred
    pub timestamp: DateTime<Utc>,
    /// SHA-256 reversal hash for linking to ReversalRecord
    pub reversal_hash: String,
    /// Serialized original fingerprints (for permanent audit trail)
    pub original_fingerprints_json: Vec<String>,
}

impl MergeRecord {
    /// Generate the storage key for CF_MERGE_HISTORY.
    ///
    /// Format: `{merged_uuid_bytes}_{timestamp_nanos_be}` (24 bytes).
    pub fn storage_key(&self) -> [u8; 24] {
        let mut key = [0u8; 24];
        key[..16].copy_from_slice(self.merged_id.as_bytes());
        let nanos = self.timestamp.timestamp_nanos_opt().unwrap_or(0);
        key[16..24].copy_from_slice(&nanos.to_be_bytes());
        key
    }

    /// Generate a prefix key for scanning all merge records for a given merged_id.
    pub fn prefix_key(merged_id: &Uuid) -> [u8; 16] {
        let mut prefix = [0u8; 16];
        prefix.copy_from_slice(merged_id.as_bytes());
        prefix
    }
}

impl ImportanceChangeRecord {
    /// Generate the storage key for CF_IMPORTANCE_HISTORY.
    ///
    /// Format: `{memory_uuid_bytes}_{timestamp_nanos_be}` (24 bytes).
    pub fn storage_key(&self) -> [u8; 24] {
        let mut key = [0u8; 24];
        key[..16].copy_from_slice(self.memory_id.as_bytes());
        let nanos = self.timestamp.timestamp_nanos_opt().unwrap_or(0);
        key[16..24].copy_from_slice(&nanos.to_be_bytes());
        key
    }

    /// Generate a prefix key for scanning all changes for a given memory_id.
    pub fn prefix_key(memory_id: &Uuid) -> [u8; 16] {
        let mut prefix = [0u8; 16];
        prefix.copy_from_slice(memory_id.as_bytes());
        prefix
    }
}

// ============================================================================
// Importance Change History (Phase 4, item 5.11)
// ============================================================================

/// Record of an importance change for history tracking (Phase 4, item 5.11).
///
/// Stored in CF_IMPORTANCE_HISTORY for permanent importance audit trail.
/// Captures the BEFORE state (old_value) so history is complete.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportanceChangeRecord {
    /// UUID of the memory whose importance changed
    pub memory_id: Uuid,
    /// When the change occurred
    pub timestamp: DateTime<Utc>,
    /// Importance value before the change
    pub old_value: f32,
    /// Importance value after the change
    pub new_value: f32,
    /// The delta applied
    pub delta: f32,
    /// Who initiated the change
    pub operator_id: Option<String>,
    /// Why the importance was changed
    pub reason: Option<String>,
}

// ============================================================================
// Hook Execution Audit Log (Phase 5, item 5.13)
// ============================================================================

/// Record of a hook execution for audit purposes (Phase 5, item 5.13).
///
/// Stored via CF_AUDIT_LOG as an AuditOperation::HookExecuted variant.
/// Captures timing, exit code, and outcome of each hook invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookExecutionRecord {
    /// Type of hook (e.g., "PostToolUse", "SessionStart", "SessionEnd")
    pub hook_type: String,
    /// Session ID when the hook ran
    pub session_id: String,
    /// When the hook started executing
    pub timestamp: DateTime<Utc>,
    /// How long the hook took to complete (milliseconds)
    pub duration_ms: u64,
    /// Process exit code (0 = success)
    pub exit_code: i32,
    /// Tool name that triggered the hook (for tool-related hooks)
    pub tool_name: Option<String>,
    /// Tool use ID from Claude Code (for tool-related hooks)
    pub tool_use_id: Option<String>,
    /// Whether the hook completed successfully
    pub success: bool,
    /// Error message if the hook failed
    pub error_message: Option<String>,
    /// UUIDs of memories created by this hook execution
    pub memories_created: Vec<Uuid>,
}

// ============================================================================
// Consolidation Recommendation Persistence (Phase 5, item 5.14)
// ============================================================================

/// A consolidation recommendation for persistence (Phase 5, item 5.14).
///
/// Stored in CF_CONSOLIDATION_RECOMMENDATIONS for future review.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidationRecommendation {
    /// Unique ID for this recommendation
    pub id: Uuid,
    /// When the recommendation was generated
    pub timestamp: DateTime<Utc>,
    /// Consolidation strategy used ("similarity", "temporal", "semantic")
    pub strategy: String,
    /// Candidate groups for consolidation
    pub candidates: Vec<ConsolidationCandidate>,
    /// Session during which analysis occurred
    pub session_id: Option<String>,
    /// Who triggered the consolidation analysis
    pub operator_id: Option<String>,
    /// Current status of this recommendation
    pub status: RecommendationStatus,
    /// When the recommendation was acted upon
    pub acted_on_at: Option<DateTime<Utc>>,
    /// If accepted, the resulting merge ID
    pub resulting_merge_id: Option<Uuid>,
}

/// A single candidate group within a consolidation recommendation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidationCandidate {
    /// Source memory IDs that could be merged
    pub source_ids: Vec<Uuid>,
    /// Target memory ID (the one to keep)
    pub target_id: Uuid,
    /// Similarity score between candidates
    pub similarity: f32,
    /// Combined alignment score
    pub combined_alignment: f32,
}

/// Status of a consolidation recommendation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecommendationStatus {
    /// Recommendation is pending review
    Pending,
    /// Recommendation was accepted and merged
    Accepted { merge_id: Uuid },
    /// Recommendation was rejected
    Rejected { reason: Option<String> },
    /// Recommendation expired without action
    Expired,
}

// ============================================================================
// Embedding Version Registry (Phase 6, item 5.15)
// ============================================================================

/// Record of embedding model versions used to compute a fingerprint (Phase 6, item 5.15).
///
/// Stored in CF_EMBEDDING_REGISTRY per fingerprint ID. Enables detection of
/// stale embeddings that need re-computation when models are upgraded.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingVersionRecord {
    /// The fingerprint this record tracks
    pub fingerprint_id: Uuid,
    /// When the embeddings were computed
    pub computed_at: DateTime<Utc>,
    /// Map of embedder name to model version string
    /// e.g., "E1" → "all-MiniLM-L6-v2", "E7" → "qodo-embed-1-1.5b"
    pub embedder_versions: std::collections::HashMap<String, String>,
    /// E7 code model version (separated for quick access)
    pub e7_model_version: Option<String>,
    /// Total embedding computation time in milliseconds
    pub computation_time_ms: Option<u64>,
}

// ============================================================================
// Core Audit Record
// ============================================================================

/// A single audit log entry. Append-only -- never update or delete.
///
/// Each record captures a complete snapshot of a provenance-relevant operation:
/// - What operation was performed (`operation`)
/// - Which entity was affected (`target_id`)
/// - Who performed it (`operator_id`)
/// - Why it was done (`rationale`)
/// - What parameters were used (`parameters`)
/// - What the outcome was (`result`)
/// - What the previous state was (`previous_state`)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRecord {
    /// Unique identifier for this audit record.
    pub id: Uuid,

    /// When the operation occurred (UTC).
    pub timestamp: DateTime<Utc>,

    /// The operation that was performed.
    pub operation: AuditOperation,

    /// The entity (memory, relationship, etc.) that was affected.
    pub target_id: Uuid,

    /// Who or what initiated the operation (e.g., session ID, tool name, "system").
    pub operator_id: Option<String>,

    /// Session identifier for grouping related operations.
    pub session_id: Option<String>,

    /// Human-readable explanation for why the operation was performed.
    pub rationale: Option<String>,

    /// Structured parameters specific to the operation.
    pub parameters: serde_json::Value,

    /// The outcome of the operation.
    pub result: AuditResult,

    /// Serialized previous state of the target entity (for undo/diff).
    /// Stored as raw bytes to avoid coupling to a specific serialization format.
    pub previous_state: Option<Vec<u8>>,
}

// ============================================================================
// Audit Operations
// ============================================================================

/// Enumeration of all auditable operations in the Context Graph system.
///
/// Each variant captures operation-specific metadata that would be lost
/// if stored only as generic parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditOperation {
    /// A new memory was created and stored.
    MemoryCreated,

    /// Two or more memories were merged into one.
    MemoryMerged {
        /// UUIDs of the source memories that were merged.
        source_ids: Vec<Uuid>,
        /// The merge strategy used (e.g., "concatenate", "summarize", "weighted").
        strategy: String,
    },

    /// A memory was deleted (soft or hard).
    MemoryDeleted {
        /// Whether this was a soft delete (recoverable) or hard delete.
        soft: bool,
        /// Reason for deletion, if provided.
        reason: Option<String>,
    },

    /// A previously soft-deleted memory was restored.
    MemoryRestored,

    /// A memory's importance score was boosted.
    ImportanceBoosted {
        /// Previous importance value.
        old: f32,
        /// New importance value.
        new: f32,
        /// The delta applied.
        delta: f32,
    },

    /// A new relationship was discovered between entities.
    RelationshipDiscovered {
        /// The type of relationship (e.g., "causal", "temporal", "semantic").
        relationship_type: String,
        /// Confidence score of the discovered relationship [0.0, 1.0].
        confidence: f32,
    },

    /// Consolidation analysis was performed on the memory store.
    ConsolidationAnalyzed {
        /// Number of merge candidates identified.
        candidates_found: usize,
    },

    /// A topic was detected from memory clustering.
    TopicDetected {
        /// Identifier for the detected topic.
        topic_id: String,
        /// Number of memories assigned to this topic.
        members: usize,
    },

    /// An embedding was recomputed for a memory.
    EmbeddingRecomputed {
        /// The embedder that produced the new embedding (e.g., "E1", "E5").
        embedder: String,
        /// Version of the model used.
        model_version: String,
    },

    /// A hook execution was recorded (Phase 5, item 5.13).
    HookExecuted {
        /// Type of hook
        hook_type: String,
        /// Whether the hook succeeded
        success: bool,
    },
}

// ============================================================================
// Audit Result
// ============================================================================

/// Outcome of an audited operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditResult {
    /// The operation completed successfully.
    Success,

    /// The operation failed.
    Failure {
        /// Description of the error.
        error: String,
    },

    /// The operation partially succeeded with warnings.
    Partial {
        /// List of warning messages.
        warnings: Vec<String>,
    },
}

// ============================================================================
// Constructors and Key Generation
// ============================================================================

impl AuditRecord {
    /// Create a new audit record with a fresh UUID and current timestamp.
    ///
    /// The record starts with `AuditResult::Success` and empty parameters.
    /// Use builder methods to set additional fields.
    ///
    /// # Arguments
    ///
    /// * `operation` - The operation being audited.
    /// * `target_id` - The entity being affected.
    pub fn new(operation: AuditOperation, target_id: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            operation,
            target_id,
            operator_id: None,
            session_id: None,
            rationale: None,
            parameters: serde_json::Value::Null,
            result: AuditResult::Success,
            previous_state: None,
        }
    }

    /// Set the operator ID.
    pub fn with_operator(mut self, operator_id: impl Into<String>) -> Self {
        self.operator_id = Some(operator_id.into());
        self
    }

    /// Set the session ID.
    pub fn with_session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Set the rationale.
    pub fn with_rationale(mut self, rationale: impl Into<String>) -> Self {
        self.rationale = Some(rationale.into());
        self
    }

    /// Set the parameters.
    pub fn with_parameters(mut self, parameters: serde_json::Value) -> Self {
        self.parameters = parameters;
        self
    }

    /// Set the result.
    pub fn with_result(mut self, result: AuditResult) -> Self {
        self.result = result;
        self
    }

    /// Set the previous state snapshot.
    pub fn with_previous_state(mut self, state: Vec<u8>) -> Self {
        self.previous_state = Some(state);
        self
    }

    /// Generate the primary key for this record.
    ///
    /// Format: `{timestamp_nanos_be}_{uuid_bytes}` (24 bytes total).
    /// Big-endian timestamp ensures chronological ordering in RocksDB iteration.
    pub fn primary_key(&self) -> [u8; 24] {
        let mut key = [0u8; 24];
        let nanos = self.timestamp.timestamp_nanos_opt().unwrap_or(0);
        key[..8].copy_from_slice(&nanos.to_be_bytes());
        key[8..24].copy_from_slice(self.id.as_bytes());
        key
    }

    /// Generate the secondary index key for target-based queries.
    ///
    /// Format: `{target_uuid_bytes}_{timestamp_nanos_be}` (24 bytes total).
    /// UUID prefix enables efficient prefix scans for all records targeting
    /// a specific entity.
    pub fn target_index_key(&self) -> [u8; 24] {
        let mut key = [0u8; 24];
        key[..16].copy_from_slice(self.target_id.as_bytes());
        let nanos = self.timestamp.timestamp_nanos_opt().unwrap_or(0);
        key[16..24].copy_from_slice(&nanos.to_be_bytes());
        key
    }

    /// Generate a target index prefix key for scanning all records for a target.
    ///
    /// Returns the 16-byte UUID prefix used for RocksDB prefix iteration.
    pub fn target_index_prefix(target_id: &Uuid) -> [u8; 16] {
        let mut prefix = [0u8; 16];
        prefix.copy_from_slice(target_id.as_bytes());
        prefix
    }
}

impl std::fmt::Display for AuditOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditOperation::MemoryCreated => write!(f, "MemoryCreated"),
            AuditOperation::MemoryMerged { source_ids, strategy } => {
                write!(f, "MemoryMerged({} sources, {})", source_ids.len(), strategy)
            }
            AuditOperation::MemoryDeleted { soft, .. } => {
                write!(f, "MemoryDeleted(soft={})", soft)
            }
            AuditOperation::MemoryRestored => write!(f, "MemoryRestored"),
            AuditOperation::ImportanceBoosted { old, new, delta } => {
                write!(
                    f,
                    "ImportanceBoosted({:.2} -> {:.2}, delta={:.2})",
                    old, new, delta
                )
            }
            AuditOperation::RelationshipDiscovered {
                relationship_type,
                confidence,
            } => {
                write!(
                    f,
                    "RelationshipDiscovered({}, confidence={:.2})",
                    relationship_type, confidence
                )
            }
            AuditOperation::ConsolidationAnalyzed { candidates_found } => {
                write!(
                    f,
                    "ConsolidationAnalyzed({} candidates)",
                    candidates_found
                )
            }
            AuditOperation::TopicDetected { topic_id, members } => {
                write!(f, "TopicDetected({}, {} members)", topic_id, members)
            }
            AuditOperation::EmbeddingRecomputed {
                embedder,
                model_version,
            } => {
                write!(f, "EmbeddingRecomputed({}, v{})", embedder, model_version)
            }
            AuditOperation::HookExecuted { hook_type, success } => {
                write!(f, "HookExecuted({}, success={})", hook_type, success)
            }
        }
    }
}

impl std::fmt::Display for AuditResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditResult::Success => write!(f, "Success"),
            AuditResult::Failure { error } => write!(f, "Failure({})", error),
            AuditResult::Partial { warnings } => {
                write!(f, "Partial({} warnings)", warnings.len())
            }
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_record_creation() {
        let target_id = Uuid::new_v4();
        let record = AuditRecord::new(AuditOperation::MemoryCreated, target_id);

        assert_eq!(record.target_id, target_id);
        assert!(matches!(record.operation, AuditOperation::MemoryCreated));
        assert!(matches!(record.result, AuditResult::Success));
        assert!(record.operator_id.is_none());
        assert!(record.session_id.is_none());
        assert!(record.rationale.is_none());
        assert!(record.previous_state.is_none());
    }

    #[test]
    fn test_audit_record_builder() {
        let target_id = Uuid::new_v4();
        let record = AuditRecord::new(AuditOperation::MemoryCreated, target_id)
            .with_operator("test-agent")
            .with_session("session-123")
            .with_rationale("unit test")
            .with_parameters(serde_json::json!({"key": "value"}))
            .with_previous_state(vec![1, 2, 3]);

        assert_eq!(record.operator_id.as_deref(), Some("test-agent"));
        assert_eq!(record.session_id.as_deref(), Some("session-123"));
        assert_eq!(record.rationale.as_deref(), Some("unit test"));
        assert_eq!(record.parameters, serde_json::json!({"key": "value"}));
        assert_eq!(record.previous_state, Some(vec![1, 2, 3]));
    }

    #[test]
    fn test_serialization_roundtrip_json() {
        let target_id = Uuid::new_v4();
        let original = AuditRecord::new(
            AuditOperation::MemoryMerged {
                source_ids: vec![Uuid::new_v4(), Uuid::new_v4()],
                strategy: "concatenate".to_string(),
            },
            target_id,
        )
        .with_operator("merger")
        .with_rationale("duplicate content");

        let json = serde_json::to_string(&original).expect("serialize to JSON");
        let deserialized: AuditRecord =
            serde_json::from_str(&json).expect("deserialize from JSON");

        assert_eq!(original.id, deserialized.id);
        assert_eq!(original.target_id, deserialized.target_id);
        assert_eq!(
            original.operator_id.as_deref(),
            deserialized.operator_id.as_deref()
        );
    }

    #[test]
    fn test_audit_operation_bincode_roundtrip() {
        // NOTE: AuditRecord contains serde_json::Value (parameters field)
        // which bincode cannot deserialize (DeserializeAnyNotSupported).
        // AuditRecord roundtrip is tested via JSON (test_serialization_roundtrip_json).
        // Here we test AuditOperation bincode roundtrip instead.
        let operation = AuditOperation::ImportanceBoosted {
            old: 0.5,
            new: 0.8,
            delta: 0.3,
        };

        let bytes = bincode::serialize(&operation).expect("serialize to bincode");
        let deserialized: AuditOperation =
            bincode::deserialize(&bytes).expect("deserialize from bincode");

        match deserialized {
            AuditOperation::ImportanceBoosted { old, new, delta } => {
                assert!((old - 0.5).abs() < f32::EPSILON);
                assert!((new - 0.8).abs() < f32::EPSILON);
                assert!((delta - 0.3).abs() < f32::EPSILON);
            }
            _ => panic!("Expected ImportanceBoosted"),
        }
    }

    #[test]
    fn test_all_operations_serialize() {
        let operations = vec![
            AuditOperation::MemoryCreated,
            AuditOperation::MemoryMerged {
                source_ids: vec![Uuid::new_v4()],
                strategy: "test".to_string(),
            },
            AuditOperation::MemoryDeleted {
                soft: true,
                reason: Some("test".to_string()),
            },
            AuditOperation::MemoryRestored,
            AuditOperation::ImportanceBoosted {
                old: 0.0,
                new: 1.0,
                delta: 1.0,
            },
            AuditOperation::RelationshipDiscovered {
                relationship_type: "causal".to_string(),
                confidence: 0.95,
            },
            AuditOperation::ConsolidationAnalyzed {
                candidates_found: 42,
            },
            AuditOperation::TopicDetected {
                topic_id: "topic-1".to_string(),
                members: 10,
            },
            AuditOperation::EmbeddingRecomputed {
                embedder: "E1".to_string(),
                model_version: "1.0".to_string(),
            },
            AuditOperation::HookExecuted {
                hook_type: "PostToolUse".to_string(),
                success: true,
            },
        ];

        for op in operations {
            let record = AuditRecord::new(op, Uuid::new_v4());
            let json = serde_json::to_string(&record).expect("serialize operation");
            let _: AuditRecord = serde_json::from_str(&json).expect("deserialize operation");
        }
    }

    #[test]
    fn test_all_results_serialize() {
        let results = vec![
            AuditResult::Success,
            AuditResult::Failure {
                error: "something broke".to_string(),
            },
            AuditResult::Partial {
                warnings: vec!["warn1".to_string(), "warn2".to_string()],
            },
        ];

        for result in results {
            let json = serde_json::to_string(&result).expect("serialize result");
            let _: AuditResult = serde_json::from_str(&json).expect("deserialize result");
        }
    }

    #[test]
    fn test_primary_key_format() {
        let record = AuditRecord::new(AuditOperation::MemoryCreated, Uuid::new_v4());
        let key = record.primary_key();

        assert_eq!(key.len(), 24);

        // Verify timestamp bytes are big-endian
        let nanos = record.timestamp.timestamp_nanos_opt().unwrap_or(0);
        let expected_ts = nanos.to_be_bytes();
        assert_eq!(&key[..8], &expected_ts);

        // Verify UUID bytes
        assert_eq!(&key[8..24], record.id.as_bytes());
    }

    #[test]
    fn test_target_index_key_format() {
        let target_id = Uuid::new_v4();
        let record = AuditRecord::new(AuditOperation::MemoryCreated, target_id);
        let key = record.target_index_key();

        assert_eq!(key.len(), 24);

        // Verify target UUID prefix
        assert_eq!(&key[..16], target_id.as_bytes());

        // Verify timestamp bytes are big-endian
        let nanos = record.timestamp.timestamp_nanos_opt().unwrap_or(0);
        let expected_ts = nanos.to_be_bytes();
        assert_eq!(&key[16..24], &expected_ts);
    }

    #[test]
    fn test_target_index_prefix() {
        let target_id = Uuid::new_v4();
        let prefix = AuditRecord::target_index_prefix(&target_id);

        assert_eq!(prefix.len(), 16);
        assert_eq!(&prefix, target_id.as_bytes());
    }

    #[test]
    fn test_chronological_key_ordering() {
        // Records created later should have lexicographically larger primary keys
        let r1 = AuditRecord::new(AuditOperation::MemoryCreated, Uuid::new_v4());
        // Small sleep to ensure different timestamp
        std::thread::sleep(std::time::Duration::from_millis(1));
        let r2 = AuditRecord::new(AuditOperation::MemoryCreated, Uuid::new_v4());

        let k1 = r1.primary_key();
        let k2 = r2.primary_key();

        assert!(k1 < k2, "Later record should have larger key");
    }

    #[test]
    fn test_display_impls() {
        assert_eq!(
            format!("{}", AuditOperation::MemoryCreated),
            "MemoryCreated"
        );
        assert_eq!(format!("{}", AuditResult::Success), "Success");
        assert_eq!(
            format!(
                "{}",
                AuditResult::Failure {
                    error: "oops".to_string()
                }
            ),
            "Failure(oops)"
        );
    }
}
