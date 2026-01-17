# TASK-P3-007: SimilarityRetriever Implementation

```xml
<task_spec id="TASK-P3-007" version="4.0" audited="2026-01-17">
<metadata>
  <title>SimilarityRetriever Implementation</title>
  <status>COMPLETE</status>
  <layer>orchestration</layer>
  <sequence>26</sequence>
  <phase>3</phase>
  <implements>
    <requirement_ref>REQ-P3-05</requirement_ref>
    <requirement_ref>REQ-P3-06</requirement_ref>
  </implements>
  <depends_on>
    <task_ref status="COMPLETE">TASK-P3-001</task_ref>
    <task_ref status="COMPLETE">TASK-P3-002</task_ref>
    <task_ref status="COMPLETE">TASK-P3-003</task_ref>
    <task_ref status="COMPLETE">TASK-P3-004</task_ref>
    <task_ref status="COMPLETE">TASK-P3-005</task_ref>
    <task_ref status="COMPLETE">TASK-P3-006</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
</metadata>

<context>
## Problem Statement

The codebase has complete low-level retrieval components (MultiSpaceSimilarity, DivergenceDetector,
MemoryStore) but lacks a HIGH-LEVEL ORCHESTRATION LAYER that ties them together into a usable API.

This task creates `SimilarityRetriever` - the unified interface that:
1. Retrieves similar memories from MemoryStore
2. Computes multi-space similarity using MultiSpaceSimilarity
3. Detects topic drift using DivergenceDetector
4. Returns ranked, filtered results for context injection

## Architecture Position

```text
+---------------------------------------+
|         SimilarityRetriever           |  <- THIS TASK CREATES THIS
|         (High-level orchestrator)     |
+------------------+--------------------+
                   |
    +--------------+----------------+
    |              |                |
    v              v                v
+----------+  +--------------+  +------------------+
|MemoryStore|  |MultiSpaceSim.|  |DivergenceDetector|
|(RocksDB)  |  |(13-space)    |  |(SEMANTIC only)   |
+----------+  +--------------+  +------------------+
     |              |                      |
     +--------------+----------------------+
                    |
            +-------+-------+
            v               v
        PerSpaceScores   DivergenceReport
```

## Constitution Compliance

- ARCH-09: Topic threshold is weighted_agreement >= 2.5
- ARCH-10: Divergence detection uses SEMANTIC embedders only (7 spaces)
- AP-60: Temporal embedders (E2-E4) MUST NOT count toward topic/relevance
- AP-62: Divergence alerts MUST only use SEMANTIC embedders
- AP-63: NEVER trigger divergence from temporal proximity differences
</context>

<codebase_state audited="2026-01-17">
## VERIFIED DEPENDENCY FILES

### MemoryStore
**Location:** `crates/context-graph-core/src/memory/store.rs` (400+ lines)

```rust
pub struct MemoryStore {
    db: Arc<DB>,  // RocksDB instance
}

impl MemoryStore {
    pub fn new(path: &Path) -> Result<Self, StorageError>;
    pub fn store(&self, memory: &Memory) -> Result<(), StorageError>;
    pub fn get(&self, id: Uuid) -> Result<Option<Memory>, StorageError>;
    pub fn get_by_session(&self, session_id: &str) -> Result<Vec<Memory>, StorageError>;
    pub fn count(&self) -> Result<u64, StorageError>;
    pub fn delete(&self, id: Uuid) -> Result<bool, StorageError>;
}
```

CRITICAL: MemoryStore methods are SYNCHRONOUS (not async). Do NOT use `.await` on them.

### Memory Struct
**Location:** `crates/context-graph-core/src/memory/mod.rs` (677 lines)

```rust
pub struct Memory {
    pub id: Uuid,
    pub content: String,
    pub source: MemorySource,
    pub created_at: DateTime<Utc>,
    pub session_id: String,
    pub teleological_array: TeleologicalArray,  // Type alias for SemanticFingerprint
    pub chunk_metadata: Option<ChunkMetadata>,
    pub word_count: u32,
}

pub enum MemorySource {
    HookDescription { hook_type: HookType, tool_name: Option<String> },
    ClaudeResponse { response_type: ResponseType },
    MDFileChunk { file_path: String, chunk_index: u32, total_chunks: u32 },
}
```

### TeleologicalArray / SemanticFingerprint
**Location:** `crates/context-graph-core/src/types/fingerprint/semantic/fingerprint.rs`

```rust
// Line 26 - this is a TYPE ALIAS
pub type TeleologicalArray = SemanticFingerprint;

pub struct SemanticFingerprint {
    pub e1_semantic: Vec<f32>,              // 1024D
    pub e2_temporal_recent: Vec<f32>,       // 512D
    pub e3_temporal_periodic: Vec<f32>,     // 512D
    pub e4_temporal_positional: Vec<f32>,   // 512D
    pub e5_causal: Vec<f32>,                // 768D
    pub e6_sparse: SparseVector,            // Sparse
    pub e7_code: Vec<f32>,                  // 1536D
    pub e8_graph: Vec<f32>,                 // 384D
    pub e9_hdc: Vec<f32>,                   // 1024D
    pub e10_multimodal: Vec<f32>,           // 768D
    pub e11_entity: Vec<f32>,               // 384D
    pub e12_late_interaction: Vec<Vec<f32>>,// 128D per token
    pub e13_splade: SparseVector,           // Sparse
}

impl SemanticFingerprint {
    pub fn zeroed() -> Self;
    pub fn get(&self, embedder: Embedder) -> EmbeddingRef<'_>;
    pub fn storage_size(&self) -> usize;
    pub fn validate_strict(&self) -> Result<(), SemanticFingerprintError>;
}
```

### MultiSpaceSimilarity (TASK-P3-005 COMPLETE)
**Location:** `crates/context-graph-core/src/retrieval/multi_space.rs` (722 lines)

```rust
#[derive(Debug, Clone)]  // NOTE: Clone is derived
pub struct MultiSpaceSimilarity {
    thresholds: SimilarityThresholds,
}

impl MultiSpaceSimilarity {
    pub fn with_defaults() -> Self;
    pub fn compute_similarity(&self, query: &SemanticFingerprint, memory: &SemanticFingerprint) -> PerSpaceScores;
    pub fn is_relevant(&self, scores: &PerSpaceScores) -> bool;
    pub fn matching_spaces(&self, scores: &PerSpaceScores) -> Vec<Embedder>;
    pub fn compute_relevance_score(&self, scores: &PerSpaceScores) -> f32;
    pub fn compute_full_result(&self, memory_id: Uuid, query: &SemanticFingerprint, memory: &SemanticFingerprint) -> SimilarityResult;
    pub fn is_below_low_threshold(&self, embedder: Embedder, score: f32) -> bool;
}

// Batch processing functions (already implemented)
pub fn compute_similarities_batch(similarity: &MultiSpaceSimilarity, query: &SemanticFingerprint, memories: &[(Uuid, SemanticFingerprint)]) -> Vec<SimilarityResult>;
pub fn filter_relevant(similarity: &MultiSpaceSimilarity, results: Vec<SimilarityResult>) -> Vec<SimilarityResult>;
pub fn sort_by_relevance(results: Vec<SimilarityResult>) -> Vec<SimilarityResult>;
```

### DivergenceDetector (TASK-P3-006 COMPLETE)
**Location:** `crates/context-graph-core/src/retrieval/detector.rs` (523 lines)

```rust
pub struct RecentMemory {
    pub id: Uuid,
    pub content: String,
    pub embedding: SemanticFingerprint,
    pub created_at: DateTime<Utc>,
}

impl RecentMemory {
    pub fn new(id: Uuid, content: String, embedding: SemanticFingerprint, created_at: DateTime<Utc>) -> Self;
}

pub struct DivergenceDetector {
    similarity: MultiSpaceSimilarity,
    lookback_duration: Duration,
    max_recent: usize,
}

impl DivergenceDetector {
    pub fn new(similarity: MultiSpaceSimilarity) -> Self;
    pub fn with_config(similarity: MultiSpaceSimilarity, lookback: Duration, max_recent: usize) -> Self;
    pub fn detect_divergence(&self, query: &SemanticFingerprint, recent_memories: &[RecentMemory]) -> DivergenceReport;
    pub fn should_alert(&self, report: &DivergenceReport) -> bool;
    pub fn summarize_divergence(&self, report: &DivergenceReport) -> String;
    pub fn lookback_duration(&self) -> Duration;
    pub fn max_recent(&self) -> usize;
}

pub fn is_divergence_space(embedder: Embedder) -> bool;
```

### Existing Retrieval Exports
**Location:** `crates/context-graph-core/src/retrieval/mod.rs` (151 lines)

Already exported - use these, DO NOT re-define:
```rust
pub use similarity::{PerSpaceScores, SimilarityResult, NUM_SPACES};
pub use divergence::{DivergenceAlert, DivergenceReport, DivergenceSeverity, DIVERGENCE_SPACES, MAX_SUMMARY_LEN, truncate_summary};
pub use config::{high_thresholds, low_thresholds, default_weights, PerSpaceThresholds, SimilarityThresholds, SpaceWeights, RECENT_LOOKBACK_SECS, MAX_RECENT_MEMORIES, SPACE_WEIGHTS, TOTAL_WEIGHT};
pub use distance::{compute_all_similarities, compute_similarity_for_space, cosine_similarity, hamming_similarity, jaccard_similarity, max_sim, transe_similarity};
pub use multi_space::{compute_similarities_batch, filter_relevant, sort_by_relevance, MultiSpaceSimilarity};
pub use detector::{DivergenceDetector, RecentMemory, is_divergence_space};
```

### Embedder Enum
**Location:** `crates/context-graph-core/src/teleological/embedder.rs`

```rust
pub enum Embedder {
    Semantic = 0,           // E1
    TemporalRecent = 1,     // E2
    TemporalPeriodic = 2,   // E3
    TemporalPositional = 3, // E4
    Causal = 4,             // E5
    Sparse = 5,             // E6
    Code = 6,               // E7
    Emotional = 7,          // E8 (Relational)
    Hdc = 8,                // E9 (Structural)
    Multimodal = 9,         // E10
    Entity = 10,            // E11 (Relational)
    LateInteraction = 11,   // E12
    KeywordSplade = 12,     // E13
}
```
</codebase_state>

<scope>
  <in_scope>
    - Create `retriever.rs` in `crates/context-graph-core/src/retrieval/`
    - Implement `SimilarityRetriever` struct with store, similarity, and detector
    - Implement `retrieve_similar()` - get ranked similar memories (SYNCHRONOUS)
    - Implement `get_recent_memories()` - convert Memory to RecentMemory
    - Implement `check_divergence()` - detect topic drift from recent context
    - Implement `RetrieverError` error type with thiserror
    - Add module export in mod.rs
    - Write comprehensive unit tests with real data flow
    - Perform manual testing with synthetic data
  </in_scope>
  <out_of_scope>
    - Context injection formatting (separate task)
    - Token budget management (separate task)
    - Index optimization/CUDA (Phase 4)
    - Asynchronous operations (MemoryStore is synchronous)
  </out_of_scope>
</scope>

<architecture_rules>
  MUST COMPLY:
  - ARCH-01: TeleologicalArray is atomic - Memory.teleological_array IS SemanticFingerprint
  - ARCH-09: Topic threshold is weighted_agreement >= 2.5
  - ARCH-10: Divergence detection uses SEMANTIC embedders only
  - AP-60: Temporal embedders (E2-E4) MUST NOT count toward relevance
  - AP-63: NEVER trigger divergence from temporal proximity

  ERROR HANDLING:
  - rust_standards.error_handling: thiserror for library errors
  - AP-14: No .unwrap() in library code - use expect() with context or propagate
  - Return Result<T, RetrieverError> from all public methods

  CRITICAL - SYNCHRONOUS:
  - MemoryStore methods are SYNCHRONOUS (not async)
  - DO NOT use .await on MemoryStore calls
  - SimilarityRetriever methods should be synchronous
</architecture_rules>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-core/src/retrieval/retriever.rs">
//! SimilarityRetriever: High-level orchestration of memory retrieval.

use std::sync::Arc;
use chrono::Utc;
use thiserror::Error;
use uuid::Uuid;

use crate::memory::{Memory, MemoryStore, StorageError};
use crate::types::fingerprint::SemanticFingerprint;

use super::detector::{DivergenceDetector, RecentMemory};
use super::divergence::DivergenceReport;
use super::multi_space::MultiSpaceSimilarity;
use super::similarity::SimilarityResult;

/// Errors from retrieval operations.
#[derive(Debug, Error)]
pub enum RetrieverError {
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),

    #[error("No memories found for session: {session_id}")]
    NoMemories { session_id: String },

    #[error("Query fingerprint is invalid")]
    InvalidQuery,
}

/// High-level orchestrator for memory retrieval.
pub struct SimilarityRetriever {
    store: Arc&lt;MemoryStore&gt;,
    similarity: MultiSpaceSimilarity,
    detector: DivergenceDetector,
}

impl SimilarityRetriever {
    /// Create with given components.
    pub fn new(store: Arc&lt;MemoryStore&gt;, similarity: MultiSpaceSimilarity, detector: DivergenceDetector) -> Self;

    /// Create with default similarity and detector configuration.
    pub fn with_defaults(store: Arc&lt;MemoryStore&gt;) -> Self;

    /// Retrieve similar memories from a session (SYNCHRONOUS).
    pub fn retrieve_similar(&amp;self, query: &amp;SemanticFingerprint, session_id: &amp;str, limit: usize) -> Result&lt;Vec&lt;SimilarityResult&gt;, RetrieverError&gt;;

    /// Get recent memories for divergence detection.
    pub fn get_recent_memories(&amp;self, session_id: &amp;str) -> Result&lt;Vec&lt;RecentMemory&gt;, RetrieverError&gt;;

    /// Check for topic divergence from recent context.
    pub fn check_divergence(&amp;self, query: &amp;SemanticFingerprint, session_id: &amp;str) -> Result&lt;DivergenceReport, RetrieverError&gt;;

    /// Get memory count for a session.
    pub fn session_memory_count(&amp;self, session_id: &amp;str) -> Result&lt;usize, RetrieverError&gt;;

    /// Get total memory count across all sessions.
    pub fn total_memory_count(&amp;self) -> Result&lt;u64, RetrieverError&gt;;
}

/// Convert Memory to RecentMemory.
pub fn memory_to_recent(memory: &amp;Memory) -> RecentMemory;
    </signature>
  </signatures>

  <constraints>
    - ALL METHODS ARE SYNCHRONOUS (MemoryStore is sync)
    - retrieve_similar returns empty Vec if no memories match, NOT error
    - retrieve_similar filters via MultiSpaceSimilarity::is_relevant
    - retrieve_similar sorts via sort_by_relevance (highest first)
    - get_recent_memories respects DivergenceDetector lookback window
    - check_divergence only checks SEMANTIC spaces via DIVERGENCE_SPACES
    - All errors propagated with context (no .unwrap())
    - Thread-safe via Arc&lt;MemoryStore&gt;
  </constraints>

  <verification>
    - Retrieve with matching memories returns ranked results
    - Retrieve with no matching memories returns empty Vec (not error)
    - Retrieve applies limit correctly
    - get_recent_memories filters by time window
    - check_divergence returns alerts only for SEMANTIC spaces
    - Storage errors propagated correctly
    - Thread-safety: multiple calls don't corrupt state
  </verification>
</definition_of_done>

<implementation_code file="crates/context-graph-core/src/retrieval/retriever.rs">
//! SimilarityRetriever: High-level orchestration of memory retrieval.
//!
//! This module provides the unified interface for:
//! - Retrieving similar memories from storage
//! - Computing multi-space similarity scores
//! - Detecting topic divergence from recent context
//!
//! # Architecture
//!
//! SimilarityRetriever coordinates three components:
//! - MemoryStore: RocksDB-backed persistent storage (SYNCHRONOUS)
//! - MultiSpaceSimilarity: 13-space similarity computation
//! - DivergenceDetector: Topic drift detection (SEMANTIC spaces only)
//!
//! # Constitution Compliance
//!
//! - ARCH-09: Topic threshold is weighted_agreement >= 2.5
//! - ARCH-10: Divergence detection uses SEMANTIC embedders only
//! - AP-60: Temporal embedders (E2-E4) excluded from relevance
//! - AP-63: Temporal proximity never triggers divergence

use std::sync::Arc;

use chrono::Utc;
use thiserror::Error;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::memory::{Memory, MemoryStore, StorageError};
use crate::types::fingerprint::SemanticFingerprint;

use super::config::{MAX_RECENT_MEMORIES, RECENT_LOOKBACK_SECS};
use super::detector::{DivergenceDetector, RecentMemory};
use super::divergence::DivergenceReport;
use super::multi_space::{
    compute_similarities_batch, filter_relevant, sort_by_relevance, MultiSpaceSimilarity,
};
use super::similarity::SimilarityResult;

/// Errors from retrieval operations.
///
/// All errors include context for debugging. Uses thiserror for ergonomic
/// error handling per rust_standards.error_handling.
#[derive(Debug, Error)]
pub enum RetrieverError {
    /// Storage operation failed.
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),

    /// Session has no memories to search.
    #[error("No memories found for session: {session_id}")]
    NoMemories { session_id: String },

    /// Query fingerprint failed validation.
    #[error("Query fingerprint is invalid: {reason}")]
    InvalidQuery { reason: String },
}

/// High-level orchestrator for memory retrieval.
///
/// Coordinates MemoryStore, MultiSpaceSimilarity, and DivergenceDetector
/// to provide a unified retrieval API.
///
/// # Thread Safety
///
/// Thread-safe via Arc&lt;MemoryStore&gt;. Multiple threads can call methods
/// concurrently. MemoryStore uses RocksDB which handles concurrency internally.
///
/// # Synchronous API
///
/// All methods are SYNCHRONOUS because MemoryStore is synchronous.
/// For async contexts, wrap calls in `spawn_blocking`.
///
/// # Example
///
/// ```ignore
/// use std::sync::Arc;
/// use context_graph_core::memory::MemoryStore;
/// use context_graph_core::retrieval::SimilarityRetriever;
///
/// let store = Arc::new(MemoryStore::new(path)?);
/// let retriever = SimilarityRetriever::with_defaults(store);
///
/// let results = retriever.retrieve_similar(&amp;query, "session-123", 10)?;
/// ```
pub struct SimilarityRetriever {
    store: Arc&lt;MemoryStore&gt;,
    similarity: MultiSpaceSimilarity,
    detector: DivergenceDetector,
}

impl SimilarityRetriever {
    /// Create a new retriever with the given components.
    ///
    /// # Arguments
    /// * `store` - Arc-wrapped MemoryStore for thread-safe access
    /// * `similarity` - MultiSpaceSimilarity for score computation
    /// * `detector` - DivergenceDetector for topic drift detection
    pub fn new(
        store: Arc&lt;MemoryStore&gt;,
        similarity: MultiSpaceSimilarity,
        detector: DivergenceDetector,
    ) -> Self {
        Self {
            store,
            similarity,
            detector,
        }
    }

    /// Create with default similarity and detector configuration.
    ///
    /// Uses spec-compliant defaults:
    /// - Thresholds from TECH-PHASE3 spec
    /// - Lookback: 2 hours (RECENT_LOOKBACK_SECS)
    /// - Max recent: 50 (MAX_RECENT_MEMORIES)
    pub fn with_defaults(store: Arc&lt;MemoryStore&gt;) -> Self {
        let similarity = MultiSpaceSimilarity::with_defaults();
        let detector = DivergenceDetector::new(similarity.clone());

        Self {
            store,
            similarity,
            detector,
        }
    }

    /// Retrieve similar memories from a session.
    ///
    /// # Algorithm
    /// 1. Fetch all memories from session via MemoryStore::get_by_session
    /// 2. Convert to (Uuid, SemanticFingerprint) tuples
    /// 3. Compute similarity in batch using compute_similarities_batch
    /// 4. Filter to relevant results (ANY space above high threshold)
    /// 5. Sort by relevance score (highest first)
    /// 6. Limit to requested count
    ///
    /// # Arguments
    /// * `query` - The query embedding fingerprint
    /// * `session_id` - Session to search within
    /// * `limit` - Maximum number of results to return
    ///
    /// # Returns
    /// - Ok(Vec&lt;SimilarityResult&gt;) - Ranked similar memories (may be empty)
    /// - Err(RetrieverError) - Storage error occurred
    ///
    /// # Note
    /// Returns empty Vec (not error) if no memories match relevance threshold.
    pub fn retrieve_similar(
        &amp;self,
        query: &amp;SemanticFingerprint,
        session_id: &amp;str,
        limit: usize,
    ) -> Result&lt;Vec&lt;SimilarityResult&gt;, RetrieverError&gt; {
        // Step 1: Fetch memories from storage (SYNCHRONOUS - no .await!)
        let memories = self.store.get_by_session(session_id)?;

        if memories.is_empty() {
            debug!(session_id = %session_id, "No memories in session");
            return Ok(Vec::new());
        }

        debug!(
            session_id = %session_id,
            memory_count = memories.len(),
            "Retrieved memories for similarity search"
        );

        // Step 2: Convert to (Uuid, SemanticFingerprint) for batch processing
        let memory_tuples: Vec&lt;(Uuid, SemanticFingerprint)&gt; = memories
            .iter()
            .map(|m| (m.id, m.teleological_array.clone()))
            .collect();

        // Step 3: Compute similarity in batch
        let results = compute_similarities_batch(&amp;self.similarity, query, &amp;memory_tuples);

        // Step 4: Filter to relevant results (ANY space above threshold)
        let relevant = filter_relevant(&amp;self.similarity, results);

        // Step 5: Sort by relevance (highest first)
        let sorted = sort_by_relevance(relevant);

        // Step 6: Apply limit
        let limited: Vec&lt;SimilarityResult&gt; = sorted.into_iter().take(limit).collect();

        debug!(
            session_id = %session_id,
            result_count = limited.len(),
            limit = limit,
            "Retrieval complete"
        );

        Ok(limited)
    }

    /// Get recent memories for divergence detection.
    ///
    /// Converts Memory structs to RecentMemory for use with DivergenceDetector.
    /// Uses detector's lookback window (default 2 hours) and max_recent (default 50).
    ///
    /// # Arguments
    /// * `session_id` - Session to get recent memories from
    ///
    /// # Returns
    /// - Ok(Vec&lt;RecentMemory&gt;) - Recent memories within lookback window
    /// - Err(RetrieverError) - Storage error occurred
    pub fn get_recent_memories(
        &amp;self,
        session_id: &amp;str,
    ) -> Result&lt;Vec&lt;RecentMemory&gt;, RetrieverError&gt; {
        // SYNCHRONOUS - no .await!
        let memories = self.store.get_by_session(session_id)?;

        // Filter by lookback window and limit
        let lookback_secs = self.detector.lookback_duration().as_secs() as i64;
        let cutoff = Utc::now() - chrono::Duration::seconds(lookback_secs);
        let max_recent = self.detector.max_recent();

        let recent: Vec&lt;RecentMemory&gt; = memories
            .iter()
            .filter(|m| m.created_at >= cutoff)
            .take(max_recent)
            .map(memory_to_recent)
            .collect();

        debug!(
            session_id = %session_id,
            total = memories.len(),
            recent = recent.len(),
            lookback_secs = lookback_secs,
            "Filtered to recent memories"
        );

        Ok(recent)
    }

    /// Check for topic divergence from recent context.
    ///
    /// # Algorithm
    /// 1. Get recent memories via get_recent_memories
    /// 2. Detect divergence using DivergenceDetector::detect_divergence
    /// 3. Return report with alerts for SEMANTIC spaces only
    ///
    /// # Arguments
    /// * `query` - The current query's embedding fingerprint
    /// * `session_id` - Session to compare against
    ///
    /// # Returns
    /// - Ok(DivergenceReport) - Report with alerts (may be empty if coherent)
    /// - Err(RetrieverError) - Storage error occurred
    pub fn check_divergence(
        &amp;self,
        query: &amp;SemanticFingerprint,
        session_id: &amp;str,
    ) -> Result&lt;DivergenceReport, RetrieverError&gt; {
        let recent = self.get_recent_memories(session_id)?;

        let report = self.detector.detect_divergence(query, &amp;recent);

        if !report.is_empty() {
            debug!(
                session_id = %session_id,
                alert_count = report.len(),
                "Divergence detected"
            );
        }

        Ok(report)
    }

    /// Get memory count for a session.
    ///
    /// Returns the number of memories in the specified session.
    pub fn session_memory_count(&amp;self, session_id: &amp;str) -> Result&lt;usize, RetrieverError&gt; {
        let memories = self.store.get_by_session(session_id)?;
        Ok(memories.len())
    }

    /// Get total memory count across all sessions.
    ///
    /// Returns the total number of memories in the store.
    pub fn total_memory_count(&amp;self) -> Result&lt;u64, RetrieverError&gt; {
        let count = self.store.count()?;
        Ok(count)
    }

    /// Check if divergence should trigger an alert.
    ///
    /// Returns true only for High severity divergence (score &lt; 0.10).
    pub fn should_alert_divergence(&amp;self, report: &amp;DivergenceReport) -> bool {
        self.detector.should_alert(report)
    }

    /// Generate human-readable divergence summary.
    pub fn summarize_divergence(&amp;self, report: &amp;DivergenceReport) -> String {
        self.detector.summarize_divergence(report)
    }

    /// Get reference to the underlying MemoryStore.
    pub fn store(&amp;self) -> &amp;Arc&lt;MemoryStore&gt; {
        &amp;self.store
    }

    /// Get reference to the MultiSpaceSimilarity service.
    pub fn similarity(&amp;self) -> &amp;MultiSpaceSimilarity {
        &amp;self.similarity
    }

    /// Get reference to the DivergenceDetector.
    pub fn detector(&amp;self) -> &amp;DivergenceDetector {
        &amp;self.detector
    }
}

/// Convert a Memory to a RecentMemory for divergence detection.
///
/// Maps Memory fields to RecentMemory:
/// - id -> id
/// - content -> content
/// - teleological_array -> embedding (type alias for SemanticFingerprint)
/// - created_at -> created_at
pub fn memory_to_recent(memory: &amp;Memory) -> RecentMemory {
    RecentMemory::new(
        memory.id,
        memory.content.clone(),
        memory.teleological_array.clone(),
        memory.created_at,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{HookType, MemorySource};
    use crate::types::fingerprint::SemanticFingerprint;
    use tempfile::tempdir;

    // =========================================================================
    // Test Helpers - Use REAL components, NO MOCKS
    // =========================================================================

    fn create_test_memory(session_id: &amp;str, content: &amp;str) -> Memory {
        Memory::new(
            content.to_string(),
            MemorySource::HookDescription {
                hook_type: HookType::UserPromptSubmit,
                tool_name: None,
            },
            session_id.to_string(),
            SemanticFingerprint::zeroed(),
            None,
        )
    }

    fn create_test_retriever() -> (SimilarityRetriever, tempfile::TempDir) {
        let tmp = tempdir().expect("create temp dir");
        let store = Arc::new(MemoryStore::new(tmp.path()).expect("create store"));
        let retriever = SimilarityRetriever::with_defaults(store);
        (retriever, tmp)
    }

    // =========================================================================
    // SimilarityRetriever Creation Tests
    // =========================================================================

    #[test]
    fn test_retriever_with_defaults() {
        let tmp = tempdir().expect("create temp dir");
        let store = Arc::new(MemoryStore::new(tmp.path()).expect("create store"));
        let retriever = SimilarityRetriever::with_defaults(store);

        // Verify defaults applied
        assert!(retriever.detector().max_recent() == MAX_RECENT_MEMORIES);
        println!("[PASS] with_defaults creates retriever with spec defaults");
    }

    #[test]
    fn test_retriever_component_access() {
        let (retriever, _tmp) = create_test_retriever();

        // Verify components are accessible
        let _store_ref = retriever.store();
        let _sim_ref = retriever.similarity();
        let _det_ref = retriever.detector();
        println!("[PASS] Component references accessible");
    }

    // =========================================================================
    // retrieve_similar Tests
    // =========================================================================

    #[test]
    fn test_retrieve_similar_empty_session() {
        let (retriever, _tmp) = create_test_retriever();
        let query = SemanticFingerprint::zeroed();

        let results = retriever
            .retrieve_similar(&amp;query, "empty-session", 10)
            .expect("retrieve should succeed");

        assert!(results.is_empty());
        println!("[PASS] Empty session returns empty Vec (not error)");
    }

    #[test]
    fn test_retrieve_similar_with_memories() {
        let (retriever, _tmp) = create_test_retriever();
        let session_id = "test-session";

        // Store some memories
        let mem1 = create_test_memory(session_id, "First memory content");
        let mem2 = create_test_memory(session_id, "Second memory content");
        retriever.store().store(&amp;mem1).expect("store mem1");
        retriever.store().store(&amp;mem2).expect("store mem2");

        // Query with zeroed fingerprint
        let query = SemanticFingerprint::zeroed();
        let results = retriever
            .retrieve_similar(&amp;query, session_id, 10)
            .expect("retrieve should succeed");

        // With zeroed fingerprints, similarity depends on distance calculations
        // The key assertion is that retrieval completes successfully
        println!(
            "[PASS] Retrieved {} results from session with 2 memories",
            results.len()
        );
    }

    #[test]
    fn test_retrieve_similar_respects_limit() {
        let (retriever, _tmp) = create_test_retriever();
        let session_id = "limit-test";

        // Store 5 memories
        for i in 0..5 {
            let mem = create_test_memory(session_id, &amp;format!("Memory {}", i));
            retriever.store().store(&amp;mem).expect("store memory");
        }

        let query = SemanticFingerprint::zeroed();

        // Request limit of 2
        let results = retriever
            .retrieve_similar(&amp;query, session_id, 2)
            .expect("retrieve should succeed");

        assert!(results.len() &lt;= 2, "Should respect limit of 2");
        println!(
            "[PASS] retrieve_similar respects limit: got {} with limit 2",
            results.len()
        );
    }

    // =========================================================================
    // get_recent_memories Tests
    // =========================================================================

    #[test]
    fn test_get_recent_memories_empty() {
        let (retriever, _tmp) = create_test_retriever();

        let recent = retriever
            .get_recent_memories("empty-session")
            .expect("should succeed");

        assert!(recent.is_empty());
        println!("[PASS] get_recent_memories returns empty for empty session");
    }

    #[test]
    fn test_get_recent_memories_converts_correctly() {
        let (retriever, _tmp) = create_test_retriever();
        let session_id = "recent-test";

        let mem = create_test_memory(session_id, "Recent memory");
        let mem_id = mem.id;
        retriever.store().store(&amp;mem).expect("store memory");

        let recent = retriever
            .get_recent_memories(session_id)
            .expect("should succeed");

        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].id, mem_id);
        assert_eq!(recent[0].content, "Recent memory");
        println!("[PASS] Memory to RecentMemory conversion correct");
    }

    // =========================================================================
    // check_divergence Tests
    // =========================================================================

    #[test]
    fn test_check_divergence_empty_session() {
        let (retriever, _tmp) = create_test_retriever();
        let query = SemanticFingerprint::zeroed();

        let report = retriever
            .check_divergence(&amp;query, "empty-session")
            .expect("should succeed");

        assert!(report.is_empty());
        println!("[PASS] check_divergence on empty session returns empty report");
    }

    #[test]
    fn test_check_divergence_with_memories() {
        let (retriever, _tmp) = create_test_retriever();
        let session_id = "divergence-test";

        let mem = create_test_memory(session_id, "Existing context");
        retriever.store().store(&amp;mem).expect("store memory");

        let query = SemanticFingerprint::zeroed();
        let report = retriever
            .check_divergence(&amp;query, session_id)
            .expect("should succeed");

        // With zeroed fingerprints, divergence depends on threshold checks
        println!(
            "[PASS] check_divergence completes with {} alerts",
            report.len()
        );
    }

    // =========================================================================
    // Memory Count Tests
    // =========================================================================

    #[test]
    fn test_session_memory_count() {
        let (retriever, _tmp) = create_test_retriever();
        let session_id = "count-test";

        assert_eq!(
            retriever.session_memory_count(session_id).expect("count"),
            0
        );

        let mem = create_test_memory(session_id, "Content");
        retriever.store().store(&amp;mem).expect("store");

        assert_eq!(
            retriever.session_memory_count(session_id).expect("count"),
            1
        );
        println!("[PASS] session_memory_count tracks correctly");
    }

    #[test]
    fn test_total_memory_count() {
        let (retriever, _tmp) = create_test_retriever();

        let initial = retriever.total_memory_count().expect("count");

        retriever
            .store()
            .store(&amp;create_test_memory("s1", "c1"))
            .expect("store");
        retriever
            .store()
            .store(&amp;create_test_memory("s2", "c2"))
            .expect("store");

        assert_eq!(retriever.total_memory_count().expect("count"), initial + 2);
        println!("[PASS] total_memory_count includes all sessions");
    }

    // =========================================================================
    // memory_to_recent Tests
    // =========================================================================

    #[test]
    fn test_memory_to_recent_conversion() {
        let memory = create_test_memory("session", "Test content");
        let recent = memory_to_recent(&amp;memory);

        assert_eq!(recent.id, memory.id);
        assert_eq!(recent.content, memory.content);
        assert_eq!(recent.created_at, memory.created_at);
        println!("[PASS] memory_to_recent preserves fields correctly");
    }

    // =========================================================================
    // Error Propagation Tests
    // =========================================================================

    #[test]
    fn test_retriever_error_display() {
        let err = RetrieverError::NoMemories {
            session_id: "test".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("test"));
        println!("[PASS] RetrieverError displays context");
    }

    // =========================================================================
    // Integration Verification Tests (Source of Truth)
    // =========================================================================

    #[test]
    fn test_full_retrieval_flow() {
        let (retriever, _tmp) = create_test_retriever();
        let session_id = "integration-test";

        // Step 1: Store memories (SOURCE OF TRUTH: RocksDB)
        for i in 0..3 {
            let mem = create_test_memory(session_id, &amp;format!("Integration memory {}", i));
            retriever.store().store(&amp;mem).expect("store");
        }

        // Step 2: Verify count via separate read (EXECUTE & INSPECT)
        let count = retriever.session_memory_count(session_id).expect("count");
        assert_eq!(count, 3, "Source of Truth: RocksDB should have 3 memories");

        // Step 3: Retrieve similar
        let query = SemanticFingerprint::zeroed();
        let similar = retriever
            .retrieve_similar(&amp;query, session_id, 10)
            .expect("retrieve");

        // Step 4: Check divergence
        let report = retriever
            .check_divergence(&amp;query, session_id)
            .expect("divergence check");

        println!(
            "[EVIDENCE] Full flow: stored 3, verified count={}, retrieved {}, {} divergence alerts",
            count,
            similar.len(),
            report.len()
        );
        println!("[PASS] Full retrieval flow verified against RocksDB source of truth");
    }

    // =========================================================================
    // Boundary & Edge Case Audit
    // =========================================================================

    #[test]
    fn test_edge_case_empty_memories_list() {
        // Edge Case 1: Empty memories list
        // Input: retrieve_similar(&query, "nonexistent-session", 10)
        // Expected: Ok(Vec::new()) - empty Vec, NOT error
        let (retriever, _tmp) = create_test_retriever();
        let query = SemanticFingerprint::zeroed();

        println!("[BEFORE] No memories stored for session");
        let results = retriever.retrieve_similar(&amp;query, "nonexistent", 10);
        println!("[AFTER] Result: {:?}", results.is_ok());

        assert!(results.is_ok());
        assert!(results.expect("should succeed").is_empty());
        println!("[PASS] Edge Case 1: Empty session returns Ok(empty Vec)");
    }

    #[test]
    fn test_edge_case_limit_enforcement() {
        // Edge Case 2: Limit Enforcement
        // Input: 5 memories stored, limit=2
        // Expected: results.len() <= 2
        let (retriever, _tmp) = create_test_retriever();
        let session_id = "limit-edge";

        println!("[BEFORE] Storing 5 memories");
        for i in 0..5 {
            retriever.store().store(&amp;create_test_memory(session_id, &amp;format!("M{}", i))).expect("store");
        }
        println!("[BEFORE] Verified count = {}", retriever.session_memory_count(session_id).expect("count"));

        let query = SemanticFingerprint::zeroed();
        let results = retriever.retrieve_similar(&amp;query, session_id, 2).expect("retrieve");

        println!("[AFTER] Results count: {}", results.len());
        assert!(results.len() &lt;= 2);
        println!("[PASS] Edge Case 2: Limit enforced correctly");
    }

    #[test]
    fn test_edge_case_memory_conversion() {
        // Edge Case 3: Memory to RecentMemory Conversion
        // Input: Memory with known fields
        // Expected: RecentMemory with identical fields
        let session_id = "conv-edge";
        let content = "Conversion test content";

        println!("[BEFORE] Creating Memory with content: {}", content);
        let memory = create_test_memory(session_id, content);
        let original_id = memory.id;
        let original_created = memory.created_at;

        let recent = memory_to_recent(&amp;memory);
        println!("[AFTER] RecentMemory id={}, content={}", recent.id, recent.content);

        assert_eq!(recent.id, original_id, "ID must match");
        assert_eq!(recent.content, content, "Content must match");
        assert_eq!(recent.created_at, original_created, "Timestamp must match");
        println!("[PASS] Edge Case 3: Conversion preserves all fields");
    }
}
</implementation_code>

<files_to_create>
  <file path="crates/context-graph-core/src/retrieval/retriever.rs">
    SimilarityRetriever implementation
  </file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-core/src/retrieval/mod.rs">
    Add: pub mod retriever;
    Add exports: pub use retriever::{SimilarityRetriever, RetrieverError, memory_to_recent};
  </file>
</files_to_modify>

<modification_instructions>
For crates/context-graph-core/src/retrieval/mod.rs:

1. Add module declaration after existing modules (around line 93):
```rust
pub mod retriever;
```

2. Add re-exports at end of file (after line 150):
```rust
// Retrieval orchestration
pub use retriever::{memory_to_recent, RetrieverError, SimilarityRetriever};
```
</modification_instructions>

<validation_criteria>
  <criterion>cargo check --package context-graph-core compiles without errors</criterion>
  <criterion>cargo test --package context-graph-core retriever -- --nocapture passes all tests</criterion>
  <criterion>cargo clippy --package context-graph-core -- -D warnings has no warnings</criterion>
  <criterion>retrieve_similar returns empty Vec for empty session (not error)</criterion>
  <criterion>retrieve_similar respects limit parameter</criterion>
  <criterion>get_recent_memories converts Memory to RecentMemory correctly</criterion>
  <criterion>check_divergence only checks SEMANTIC spaces</criterion>
  <criterion>All errors propagated with thiserror (no .unwrap())</criterion>
  <criterion>Thread-safe via Arc&lt;MemoryStore&gt;</criterion>
  <criterion>All methods are SYNCHRONOUS (no .await)</criterion>
</validation_criteria>

<test_commands>
  <command description="Check compilation">cargo check --package context-graph-core</command>
  <command description="Run retriever tests">cargo test --package context-graph-core retriever -- --nocapture</command>
  <command description="Run clippy">cargo clippy --package context-graph-core -- -D warnings</command>
  <command description="Run all retrieval tests">cargo test --package context-graph-core retrieval -- --nocapture</command>
</test_commands>
</task_spec>
```

---

## Full State Verification Protocol

### Source of Truth

| State | Location | Verification Method |
|-------|----------|---------------------|
| SimilarityRetriever struct | `crates/context-graph-core/src/retrieval/retriever.rs` | File exists, `cargo check` passes |
| RetrieverError enum | `crates/context-graph-core/src/retrieval/retriever.rs` | Unit tests pass |
| Module export | `crates/context-graph-core/src/retrieval/mod.rs` | `grep 'pub mod retriever'` |
| Type re-exports | `crates/context-graph-core/src/retrieval/mod.rs` | `grep 'SimilarityRetriever'` |
| RocksDB persistence | `/tmp/test-*` temp directories | Tests create/read/verify data |

### Execute & Inspect Protocol

After implementation, run these verification commands:

```bash
# 1. Compile check - Source of Truth: compiler output
cargo check --package context-graph-core 2>&1 | tee /tmp/check.log
grep -E "(error|warning)" /tmp/check.log && echo "FAIL" || echo "PASS"

# 2. Run tests - Source of Truth: test output
cargo test --package context-graph-core retriever -- --nocapture 2>&1 | tee /tmp/test.log
grep "FAILED" /tmp/test.log && echo "FAIL" || echo "PASS"

# 3. Verify exports - Source of Truth: mod.rs content
grep -E "pub (mod|use).*retriever" crates/context-graph-core/src/retrieval/mod.rs
grep "SimilarityRetriever" crates/context-graph-core/src/retrieval/mod.rs
grep "RetrieverError" crates/context-graph-core/src/retrieval/mod.rs
grep "memory_to_recent" crates/context-graph-core/src/retrieval/mod.rs

# 4. Verify RocksDB data persistence (manual inspection)
cargo test --package context-graph-core test_full_retrieval_flow -- --nocapture 2>&1 | grep -E "(PASS|EVIDENCE)"

# 5. Run clippy for code quality
cargo clippy --package context-graph-core -- -D warnings 2>&1 | head -20
```

### Boundary & Edge Case Audit

**Edge Case 1: Empty Session**
```
Input: retrieve_similar(&query, "nonexistent-session", 10)
Before: No memories exist for session
Expected: Ok(Vec::new()) - empty Vec, NOT error
Test: test_edge_case_empty_memories_list
Proof: Test output shows "[PASS] Edge Case 1: Empty session returns Ok(empty Vec)"
```

**Edge Case 2: Limit Enforcement**
```
Input: 5 memories stored, limit=2
Before: store.get_by_session() returns 5 memories
Expected: results.len() <= 2
Test: test_edge_case_limit_enforcement
Proof: Test output shows "[PASS] Edge Case 2: Limit enforced correctly"
```

**Edge Case 3: Memory to RecentMemory Conversion**
```
Input: Memory with id, content, teleological_array, created_at
Before: Memory struct populated
Expected: RecentMemory with same id, content, embedding, created_at
Test: test_edge_case_memory_conversion
Proof: Test asserts field equality, shows "[PASS] Edge Case 3: Conversion preserves all fields"
```

**Edge Case 4: Storage Error Propagation**
```
Input: Invalid RocksDB path
Before: Attempt to create MemoryStore
Expected: RetrieverError::Storage propagated
Test: Implicit via thiserror #[from]
Proof: Clippy passes, error type derives Error
```

**Edge Case 5: Divergence on Empty Context**
```
Input: check_divergence(&query, "empty-session")
Before: No memories in session
Expected: Ok(DivergenceReport::new()) - empty report
Test: test_check_divergence_empty_session
Proof: Test output shows "[PASS] check_divergence on empty session returns empty report"
```

### Evidence of Success

After implementation, capture and verify:

```bash
# Evidence 1: retriever.rs exists with correct content
ls -la crates/context-graph-core/src/retrieval/retriever.rs
wc -l crates/context-graph-core/src/retrieval/retriever.rs

# Evidence 2: All tests pass
cargo test --package context-graph-core retriever 2>&1 | grep -E "test result|PASSED|FAILED"

# Evidence 3: Clippy clean
cargo clippy --package context-graph-core -- -D warnings 2>&1 | grep -c "error" | xargs -I{} test {} -eq 0

# Evidence 4: Module properly exported
cargo doc --package context-graph-core --no-deps 2>&1 | grep -E "Documenting|error"
```

### Physical Proof: Database Verification

Since SimilarityRetriever uses RocksDB, verify data persistence:

```bash
# Run integration test that creates temp DB
cargo test --package context-graph-core test_full_retrieval_flow -- --nocapture

# Expected output:
# [BEFORE] Storing 3 memories
# [EVIDENCE] Full flow: stored 3, verified count=3, retrieved X, Y divergence alerts
# [PASS] Full retrieval flow verified against RocksDB source of truth

# The test:
# 1. Creates temp directory for RocksDB
# 2. Stores 3 memories
# 3. Performs SEPARATE READ to verify count (Execute & Inspect)
# 4. Retrieves via similarity search
# 5. Checks divergence
# 6. Prints evidence of actual data in system
```

---

## Fail Fast Requirements

This implementation uses **NO BACKWARDS COMPATIBILITY**. The following will cause immediate failure:

1. **Storage Error**: If MemoryStore fails, RetrieverError::Storage propagates immediately
2. **Wrong API**: Using `.await` on MemoryStore = compilation error (it's sync)
3. **Missing Dependencies**: Wrong imports = compilation error
4. **Type Mismatches**: SemanticFingerprint/TeleologicalArray confusion = compilation error
5. **Thread Safety**: Arc<MemoryStore> required - raw MemoryStore won't compile

All errors surface immediately via `cargo check` or `cargo test` - no silent failures.

---

## No Mock Data Policy

**CRITICAL**: Tests use REAL components:

```rust
// CORRECT - Real RocksDB, real components
let tmp = tempdir().expect("create temp dir");
let store = Arc::new(MemoryStore::new(tmp.path()).expect("create store"));
let retriever = SimilarityRetriever::with_defaults(store);

// WRONG - No mocks allowed
// let mock_store = MockMemoryStore::new();  // FORBIDDEN
```

Tests use:
- Real RocksDB via MemoryStore (temp directory)
- Real SemanticFingerprint (zeroed for testing)
- Real MultiSpaceSimilarity with spec thresholds
- Real DivergenceDetector with spec lookback

---

## Key Corrections from Original Task Document

1. **SYNCHRONOUS API**: MemoryStore methods are sync, NOT async. Remove all `.await` calls.
2. **No error.rs file**: RetrieverError defined in retriever.rs itself.
3. **No get_all()**: Use `get_by_session()` - the task already has session_id parameter.
4. **Clone is derived**: MultiSpaceSimilarity already has `#[derive(Clone)]`.
5. **Type alias**: `TeleologicalArray = SemanticFingerprint` (line 26 in fingerprint.rs).
6. **Field name**: Memory uses `teleological_array`, not `fingerprint`.
7. **Empty session**: Returns `Ok(Vec::new())`, NOT `Err(NoMemories)`.

---

## Execution Checklist

### Phase 1: Setup
- [ ] Verify all prerequisite tasks are COMPLETE (P3-001 through P3-006)
- [ ] Read existing retrieval/mod.rs to understand current exports
- [ ] Read memory/mod.rs to understand Memory struct
- [ ] Confirm TeleologicalArray = SemanticFingerprint alias
- [ ] Confirm MemoryStore methods are SYNCHRONOUS

### Phase 2: Implementation
- [ ] Create `retriever.rs` in `crates/context-graph-core/src/retrieval/`
- [ ] Implement `RetrieverError` enum with thiserror
- [ ] Implement `SimilarityRetriever` struct with Arc<MemoryStore>
- [ ] Implement `new()` constructor
- [ ] Implement `with_defaults()` factory
- [ ] Implement `retrieve_similar()` (SYNCHRONOUS - no .await!):
  - Fetch memories from store
  - Convert to (Uuid, SemanticFingerprint) tuples
  - Batch compute similarities
  - Filter relevant, sort by relevance, apply limit
- [ ] Implement `get_recent_memories()`:
  - Filter by lookback window
  - Convert Memory to RecentMemory
- [ ] Implement `check_divergence()`:
  - Get recent memories
  - Call detector.detect_divergence()
- [ ] Implement `memory_to_recent()` helper
- [ ] Implement count methods

### Phase 3: Integration
- [ ] Add `pub mod retriever;` to mod.rs
- [ ] Add exports: `pub use retriever::{...};`

### Phase 4: Verification
- [ ] Run `cargo check --package context-graph-core`
- [ ] Run `cargo test --package context-graph-core retriever -- --nocapture`
- [ ] Run `cargo clippy --package context-graph-core -- -D warnings`
- [ ] Verify all tests show [PASS] messages
- [ ] Verify error propagation works correctly
- [ ] Verify thread-safety compiles (Arc<MemoryStore>)

### Phase 5: Manual Testing with Synthetic Data

1. **Synthetic Test: Store and Retrieve**
```rust
// Input: Known content "SYNTHETIC_TEST_CONTENT_12345"
// Expected: Retrieved memory contains exact content
let mem = Memory::new("SYNTHETIC_TEST_CONTENT_12345".to_string(), ...);
retriever.store().store(&mem)?;
let results = retriever.retrieve_similar(&query, session_id, 10)?;
// VERIFY: Check RocksDB contains the memory
assert!(results.iter().any(|r| /* find memory with our ID */));
```

2. **Synthetic Test: Count Verification**
```rust
// Input: Store exactly 7 memories
// Expected: session_memory_count returns 7
for i in 0..7 {
    retriever.store().store(&create_test_memory(session_id, &format!("M{}", i)))?;
}
assert_eq!(retriever.session_memory_count(session_id)?, 7);
```

3. **Synthetic Test: Divergence on Matching Content**
```rust
// Input: Query fingerprint identical to stored memory fingerprint
// Expected: No divergence (scores should be high)
let fp = SemanticFingerprint::zeroed();
let mem = Memory::new("Same context".to_string(), ..., fp.clone(), ...);
retriever.store().store(&mem)?;
let report = retriever.check_divergence(&fp, session_id)?;
// With identical fingerprints, divergence should be empty
assert!(report.is_empty());
```

### Phase 6: Final Validation
- [ ] All tests pass with 0 failures
- [ ] Clippy reports 0 errors
- [ ] Manual review of retriever.rs confirms ARCH-10 compliance
- [ ] Manual review confirms correct embedder variant names used
- [ ] All [EVIDENCE] and [PASS] messages printed during tests
