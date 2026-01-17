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
use tracing::debug;
use uuid::Uuid;

use crate::memory::{Memory, MemoryStore, StorageError};
use crate::types::fingerprint::SemanticFingerprint;

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
}

/// High-level orchestrator for memory retrieval.
///
/// Coordinates MemoryStore, MultiSpaceSimilarity, and DivergenceDetector
/// to provide a unified retrieval API.
///
/// # Thread Safety
///
/// Thread-safe via Arc<MemoryStore>. Multiple threads can call methods
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
/// let results = retriever.retrieve_similar(&query, "session-123", 10)?;
/// ```
pub struct SimilarityRetriever {
    store: Arc<MemoryStore>,
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
        store: Arc<MemoryStore>,
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
    pub fn with_defaults(store: Arc<MemoryStore>) -> Self {
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
    /// - Ok(Vec<SimilarityResult>) - Ranked similar memories (may be empty)
    /// - Err(RetrieverError) - Storage error occurred
    ///
    /// # Note
    /// Returns empty Vec (not error) if no memories match relevance threshold.
    pub fn retrieve_similar(
        &self,
        query: &SemanticFingerprint,
        session_id: &str,
        limit: usize,
    ) -> Result<Vec<SimilarityResult>, RetrieverError> {
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

        // Convert to (Uuid, SemanticFingerprint) for batch processing
        let memory_tuples: Vec<(Uuid, SemanticFingerprint)> = memories
            .iter()
            .map(|m| (m.id, m.teleological_array.clone()))
            .collect();

        // Compute similarity, filter to relevant, sort, and apply limit
        let results = compute_similarities_batch(&self.similarity, query, &memory_tuples);
        let relevant = filter_relevant(&self.similarity, results);
        let sorted = sort_by_relevance(relevant);
        let limited: Vec<SimilarityResult> = sorted.into_iter().take(limit).collect();

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
    /// - Ok(Vec<RecentMemory>) - Recent memories within lookback window
    /// - Err(RetrieverError) - Storage error occurred
    pub fn get_recent_memories(
        &self,
        session_id: &str,
    ) -> Result<Vec<RecentMemory>, RetrieverError> {
        let memories = self.store.get_by_session(session_id)?;

        let lookback_secs = self.detector.lookback_duration().as_secs() as i64;
        let cutoff = Utc::now() - chrono::Duration::seconds(lookback_secs);

        let recent: Vec<RecentMemory> = memories
            .iter()
            .filter(|m| m.created_at >= cutoff)
            .take(self.detector.max_recent())
            .map(memory_to_recent)
            .collect();

        debug!(
            session_id = %session_id,
            total = memories.len(),
            recent = recent.len(),
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
        &self,
        query: &SemanticFingerprint,
        session_id: &str,
    ) -> Result<DivergenceReport, RetrieverError> {
        let recent = self.get_recent_memories(session_id)?;

        let report = self.detector.detect_divergence(query, &recent);

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
    pub fn session_memory_count(&self, session_id: &str) -> Result<usize, RetrieverError> {
        let memories = self.store.get_by_session(session_id)?;
        Ok(memories.len())
    }

    /// Get total memory count across all sessions.
    ///
    /// Returns the total number of memories in the store.
    pub fn total_memory_count(&self) -> Result<u64, RetrieverError> {
        let count = self.store.count()?;
        Ok(count)
    }

    /// Check if divergence should trigger an alert.
    ///
    /// Returns true only for High severity divergence (score < 0.10).
    pub fn should_alert_divergence(&self, report: &DivergenceReport) -> bool {
        self.detector.should_alert(report)
    }

    /// Generate human-readable divergence summary.
    pub fn summarize_divergence(&self, report: &DivergenceReport) -> String {
        self.detector.summarize_divergence(report)
    }

    /// Get reference to the underlying MemoryStore.
    pub fn store(&self) -> &Arc<MemoryStore> {
        &self.store
    }

    /// Get reference to the MultiSpaceSimilarity service.
    pub fn similarity(&self) -> &MultiSpaceSimilarity {
        &self.similarity
    }

    /// Get reference to the DivergenceDetector.
    pub fn detector(&self) -> &DivergenceDetector {
        &self.detector
    }
}

/// Convert a Memory to a RecentMemory for divergence detection.
///
/// Maps Memory fields to RecentMemory:
/// - id -> id
/// - content -> content
/// - teleological_array -> embedding (type alias for SemanticFingerprint)
/// - created_at -> created_at
pub fn memory_to_recent(memory: &Memory) -> RecentMemory {
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
    use crate::retrieval::config::MAX_RECENT_MEMORIES;
    use crate::types::fingerprint::SemanticFingerprint;
    use tempfile::tempdir;

    // =========================================================================
    // Test Helpers - Use REAL components, NO MOCKS
    // =========================================================================

    fn create_test_memory(session_id: &str, content: &str) -> Memory {
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
            .retrieve_similar(&query, "empty-session", 10)
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
        retriever.store().store(&mem1).expect("store mem1");
        retriever.store().store(&mem2).expect("store mem2");

        // Query with zeroed fingerprint
        let query = SemanticFingerprint::zeroed();
        let results = retriever
            .retrieve_similar(&query, session_id, 10)
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
            let mem = create_test_memory(session_id, &format!("Memory {}", i));
            retriever.store().store(&mem).expect("store memory");
        }

        let query = SemanticFingerprint::zeroed();

        // Request limit of 2
        let results = retriever
            .retrieve_similar(&query, session_id, 2)
            .expect("retrieve should succeed");

        assert!(results.len() <= 2, "Should respect limit of 2");
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
        retriever.store().store(&mem).expect("store memory");

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
            .check_divergence(&query, "empty-session")
            .expect("should succeed");

        assert!(report.is_empty());
        println!("[PASS] check_divergence on empty session returns empty report");
    }

    #[test]
    fn test_check_divergence_with_memories() {
        let (retriever, _tmp) = create_test_retriever();
        let session_id = "divergence-test";

        let mem = create_test_memory(session_id, "Existing context");
        retriever.store().store(&mem).expect("store memory");

        let query = SemanticFingerprint::zeroed();
        let report = retriever
            .check_divergence(&query, session_id)
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
        retriever.store().store(&mem).expect("store");

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
            .store(&create_test_memory("s1", "c1"))
            .expect("store");
        retriever
            .store()
            .store(&create_test_memory("s2", "c2"))
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
        let recent = memory_to_recent(&memory);

        assert_eq!(recent.id, memory.id);
        assert_eq!(recent.content, memory.content);
        assert_eq!(recent.created_at, memory.created_at);
        println!("[PASS] memory_to_recent preserves fields correctly");
    }

    // =========================================================================
    // Error Propagation Tests
    // =========================================================================

    #[test]
    fn test_retriever_error_from_storage() {
        // Verify StorageError converts to RetrieverError
        let storage_err = StorageError::SerializationFailed("test".to_string());
        let retriever_err: RetrieverError = storage_err.into();
        let msg = format!("{}", retriever_err);
        assert!(msg.contains("test"));
        println!("[PASS] StorageError converts to RetrieverError");
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
            let mem = create_test_memory(session_id, &format!("Integration memory {}", i));
            retriever.store().store(&mem).expect("store");
        }

        // Step 2: Verify count via separate read (EXECUTE & INSPECT)
        let count = retriever.session_memory_count(session_id).expect("count");
        assert_eq!(count, 3, "Source of Truth: RocksDB should have 3 memories");

        // Step 3: Retrieve similar
        let query = SemanticFingerprint::zeroed();
        let similar = retriever
            .retrieve_similar(&query, session_id, 10)
            .expect("retrieve");

        // Step 4: Check divergence
        let report = retriever
            .check_divergence(&query, session_id)
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
    // Divergence Helper Tests
    // =========================================================================

    #[test]
    fn test_should_alert_divergence_false_on_empty() {
        let (retriever, _tmp) = create_test_retriever();
        let report = DivergenceReport::new();

        assert!(!retriever.should_alert_divergence(&report));
        println!("[PASS] should_alert_divergence returns false for empty report");
    }

    #[test]
    fn test_summarize_divergence_empty() {
        let (retriever, _tmp) = create_test_retriever();
        let report = DivergenceReport::new();

        let summary = retriever.summarize_divergence(&report);
        assert!(summary.contains("No divergence"));
        println!("[PASS] summarize_divergence for empty report: {}", summary);
    }

    // =========================================================================
    // Full State Verification (FSV) Test
    // =========================================================================

    #[test]
    fn fsv_verify_rocksdb_retrieval_state() {
        println!("\n============================================================");
        println!("=== FSV: SimilarityRetriever RocksDB State Verification ===");
        println!("============================================================\n");

        let tmp = tempdir().expect("create temp dir");
        let db_path = tmp.path();

        println!("[FSV-1] Creating SimilarityRetriever at: {:?}", db_path);

        let store = Arc::new(MemoryStore::new(db_path).expect("create store"));
        let retriever = SimilarityRetriever::with_defaults(store.clone());

        // Store 3 memories with known content
        let session_id = "fsv-session";
        let content_1 = "FSV_SYNTHETIC_CONTENT_AAA";
        let content_2 = "FSV_SYNTHETIC_CONTENT_BBB";
        let content_3 = "FSV_SYNTHETIC_CONTENT_CCC";

        let mem1 = create_test_memory(session_id, content_1);
        let mem2 = create_test_memory(session_id, content_2);
        let mem3 = create_test_memory(session_id, content_3);

        let id1 = mem1.id;
        let id2 = mem2.id;
        let id3 = mem3.id;

        println!("[FSV-2] Storing 3 memories with IDs:");
        println!("  - ID1: {}", id1);
        println!("  - ID2: {}", id2);
        println!("  - ID3: {}", id3);

        retriever.store().store(&mem1).expect("store 1");
        retriever.store().store(&mem2).expect("store 2");
        retriever.store().store(&mem3).expect("store 3");

        // VERIFICATION STEP 1: Check count via separate read
        println!("\n[FSV-3] Verifying via session_memory_count...");
        let count = retriever.session_memory_count(session_id).expect("count");
        println!("  Memory count: {}", count);
        assert_eq!(count, 3, "Should have exactly 3 memories");

        // VERIFICATION STEP 2: Check raw store.get() for each ID
        println!("\n[FSV-4] Verifying each memory exists via store.get()...");
        let r1 = store.get(id1).expect("get 1");
        let r2 = store.get(id2).expect("get 2");
        let r3 = store.get(id3).expect("get 3");

        assert!(r1.is_some(), "Memory 1 should exist");
        assert!(r2.is_some(), "Memory 2 should exist");
        assert!(r3.is_some(), "Memory 3 should exist");

        println!("  Memory 1 content: {:?}", r1.as_ref().map(|m| &m.content));
        println!("  Memory 2 content: {:?}", r2.as_ref().map(|m| &m.content));
        println!("  Memory 3 content: {:?}", r3.as_ref().map(|m| &m.content));

        assert_eq!(r1.as_ref().map(|m| m.content.as_str()), Some(content_1));
        assert_eq!(r2.as_ref().map(|m| m.content.as_str()), Some(content_2));
        assert_eq!(r3.as_ref().map(|m| m.content.as_str()), Some(content_3));

        // VERIFICATION STEP 3: Retrieve similar and verify flow
        println!("\n[FSV-5] Testing retrieve_similar flow...");
        let query = SemanticFingerprint::zeroed();
        let results = retriever.retrieve_similar(&query, session_id, 10).expect("retrieve");
        println!("  retrieve_similar returned {} results", results.len());

        // VERIFICATION STEP 4: Test get_recent_memories
        println!("\n[FSV-6] Testing get_recent_memories...");
        let recent = retriever.get_recent_memories(session_id).expect("recent");
        println!("  get_recent_memories returned {} memories", recent.len());
        assert_eq!(recent.len(), 3, "Should have 3 recent memories");

        // Verify recent memory IDs match stored IDs
        let recent_ids: Vec<Uuid> = recent.iter().map(|r| r.id).collect();
        assert!(recent_ids.contains(&id1), "Recent should contain ID1");
        assert!(recent_ids.contains(&id2), "Recent should contain ID2");
        assert!(recent_ids.contains(&id3), "Recent should contain ID3");

        // VERIFICATION STEP 5: Test check_divergence
        println!("\n[FSV-7] Testing check_divergence...");
        let report = retriever.check_divergence(&query, session_id).expect("divergence");
        println!("  check_divergence returned {} alerts", report.len());

        // VERIFICATION STEP 6: Verify total_memory_count
        println!("\n[FSV-8] Testing total_memory_count...");
        let total = retriever.total_memory_count().expect("total");
        println!("  total_memory_count: {}", total);
        assert!(total >= 3, "Total should be at least 3");

        println!("\n============================================================");
        println!("[FSV] VERIFIED: All retrieval state checks passed");
        println!("============================================================\n");
    }
}
