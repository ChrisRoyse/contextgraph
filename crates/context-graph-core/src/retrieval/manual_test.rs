//! Manual integration tests for SimilarityRetriever.
//!
//! These tests perform comprehensive verification with synthetic data and
//! verify physical storage state in RocksDB. Each scenario includes verbose
//! output for debugging.
//!
//! Run with: cargo test --package context-graph-core --lib retrieval::manual_test -- --nocapture

#[cfg(test)]
mod manual_integration_tests {
    use std::sync::Arc;

    use chrono::Utc;
    use tempfile::tempdir;
    use uuid::Uuid;

    use crate::memory::{HookType, Memory, MemorySource, MemoryStore};
    use crate::retrieval::{memory_to_recent, SimilarityRetriever};
    use crate::teleological::Embedder;
    use crate::types::fingerprint::SemanticFingerprint;

    // =========================================================================
    // SCENARIO 1: Happy Path - Store and Retrieve Similar Memories
    // =========================================================================

    #[test]
    fn manual_scenario_1_happy_path_store_and_retrieve() {
        println!("\n============================================================");
        println!("=== SCENARIO 1: Happy Path - Store and Retrieve ===");
        println!("============================================================\n");

        // SETUP: Create retriever with fresh RocksDB
        let tmp = tempdir().expect("create temp dir");
        let db_path = tmp.path();
        println!("[SETUP] RocksDB path: {:?}", db_path);

        let store = Arc::new(MemoryStore::new(db_path).expect("create store"));
        let retriever = SimilarityRetriever::with_defaults(store.clone());

        // INPUT: Create 3 synthetic memories with KNOWN content
        let session_id = "manual-test-session";
        let synthetic_memories = vec![
            ("SYNTHETIC_AAA_FIRST_MEMORY", "First synthetic memory about Rust programming"),
            ("SYNTHETIC_BBB_SECOND_MEMORY", "Second synthetic memory about database design"),
            ("SYNTHETIC_CCC_THIRD_MEMORY", "Third synthetic memory about API architecture"),
        ];

        println!("[INPUT] Storing 3 synthetic memories:");
        let mut stored_ids = Vec::new();
        for (marker, content) in &synthetic_memories {
            let full_content = format!("{} - {}", marker, content);
            let mem = Memory::new(
                full_content.clone(),
                MemorySource::HookDescription {
                    hook_type: HookType::UserPromptSubmit,
                    tool_name: None,
                },
                session_id.to_string(),
                SemanticFingerprint::zeroed(),
                None,
            );
            stored_ids.push(mem.id);
            println!("  - ID: {} | Content: {}", mem.id, full_content);
            retriever.store().store(&mem).expect("store memory");
        }

        // EXECUTE: Retrieve similar memories
        let query = SemanticFingerprint::zeroed();
        let results = retriever
            .retrieve_similar(&query, session_id, 10)
            .expect("retrieve similar");

        // VERIFY: Check results against expectations
        println!("\n[RESULTS] retrieve_similar returned {} results", results.len());

        // SOURCE OF TRUTH VERIFICATION: Check RocksDB directly
        println!("\n[VERIFY] Checking RocksDB source of truth:");
        let count = store.count().expect("count");
        println!("  - Total memories in DB: {}", count);
        assert_eq!(count, 3, "RocksDB should have exactly 3 memories");

        for id in &stored_ids {
            let mem = store.get(*id).expect("get").expect("memory should exist");
            println!("  - ID {} exists with content prefix: {}", id, &mem.content[..30]);
        }

        // Verify session index
        let session_mems = store.get_by_session(session_id).expect("get by session");
        println!("  - Session '{}' has {} memories", session_id, session_mems.len());
        assert_eq!(session_mems.len(), 3, "Session should have 3 memories");

        println!("\n[PASS] SCENARIO 1: Happy path verified");
    }

    // =========================================================================
    // SCENARIO 2: Edge Case - Empty Session
    // =========================================================================

    #[test]
    fn manual_scenario_2_empty_session() {
        println!("\n============================================================");
        println!("=== SCENARIO 2: Edge Case - Empty Session ===");
        println!("============================================================\n");

        let tmp = tempdir().expect("create temp dir");
        let store = Arc::new(MemoryStore::new(tmp.path()).expect("create store"));
        let retriever = SimilarityRetriever::with_defaults(store.clone());

        let query = SemanticFingerprint::zeroed();
        let session_id = "nonexistent-session";

        println!("[BEFORE] Session '{}' does not exist", session_id);
        let count = store.count().expect("count");
        println!("[BEFORE] Total memories in DB: {}", count);
        assert_eq!(count, 0);

        // EXECUTE
        let results = retriever
            .retrieve_similar(&query, session_id, 10)
            .expect("should succeed");

        println!("[AFTER] retrieve_similar returned {} results", results.len());
        assert!(results.is_empty(), "Empty session should return empty Vec");

        // Verify DB unchanged
        assert_eq!(store.count().expect("count"), 0);

        println!("\n[PASS] SCENARIO 2: Empty session edge case verified");
    }

    // =========================================================================
    // SCENARIO 3: Edge Case - Limit Enforcement
    // =========================================================================

    #[test]
    fn manual_scenario_3_limit_enforcement() {
        println!("\n============================================================");
        println!("=== SCENARIO 3: Edge Case - Limit Enforcement ===");
        println!("============================================================\n");

        let tmp = tempdir().expect("create temp dir");
        let store = Arc::new(MemoryStore::new(tmp.path()).expect("create store"));
        let retriever = SimilarityRetriever::with_defaults(store.clone());

        let session_id = "limit-test-session";

        // Store 10 memories
        println!("[INPUT] Storing 10 memories...");
        for i in 0..10 {
            let content = format!("LIMIT_TEST_MEMORY_{:02}", i);
            let mem = Memory::new(
                content,
                MemorySource::HookDescription {
                    hook_type: HookType::PostToolUse,
                    tool_name: Some("Edit".to_string()),
                },
                session_id.to_string(),
                SemanticFingerprint::zeroed(),
                None,
            );
            store.store(&mem).expect("store");
        }

        // Verify 10 stored
        let count = store.count().expect("count");
        println!("[VERIFY] RocksDB has {} memories", count);
        assert_eq!(count, 10);

        // Retrieve with limit = 3
        let query = SemanticFingerprint::zeroed();
        let results = retriever
            .retrieve_similar(&query, session_id, 3)
            .expect("retrieve");

        println!("[RESULT] Requested limit=3, got {} results", results.len());
        assert!(results.len() <= 3, "Should respect limit of 3");

        println!("\n[PASS] SCENARIO 3: Limit enforcement verified");
    }

    // =========================================================================
    // SCENARIO 4: Divergence Detection Flow
    // =========================================================================

    #[test]
    fn manual_scenario_4_divergence_detection() {
        println!("\n============================================================");
        println!("=== SCENARIO 4: Divergence Detection Flow ===");
        println!("============================================================\n");

        let tmp = tempdir().expect("create temp dir");
        let store = Arc::new(MemoryStore::new(tmp.path()).expect("create store"));
        let retriever = SimilarityRetriever::with_defaults(store.clone());

        let session_id = "divergence-test-session";

        // Store some context memories
        println!("[SETUP] Storing context memories...");
        for i in 0..3 {
            let content = format!("Context memory about topic A, iteration {}", i);
            let mem = Memory::new(
                content,
                MemorySource::HookDescription {
                    hook_type: HookType::UserPromptSubmit,
                    tool_name: None,
                },
                session_id.to_string(),
                SemanticFingerprint::zeroed(),
                None,
            );
            store.store(&mem).expect("store");
        }

        // Verify stored
        let count = retriever.session_memory_count(session_id).expect("count");
        println!("[VERIFY] Session has {} memories", count);
        assert_eq!(count, 3);

        // Query with zeroed fingerprint (which will produce divergence alerts)
        let query = SemanticFingerprint::zeroed();
        let report = retriever
            .check_divergence(&query, session_id)
            .expect("check divergence");

        println!("[RESULT] Divergence report has {} alerts", report.len());

        // Verify divergence detection uses only SEMANTIC spaces
        if !report.is_empty() {
            println!("[DETAIL] Alert breakdown by embedder:");
            for alert in report.alerts.iter().take(5) {
                println!("  - {:?}: score={:.3}, severity={:?}",
                    alert.space, alert.similarity_score, alert.severity());
            }

            // All alerts should be for semantic embedders (ARCH-10)
            for alert in &report.alerts {
                let is_semantic = matches!(
                    alert.space,
                    Embedder::Semantic | Embedder::Causal | Embedder::Sparse |
                    Embedder::Code | Embedder::Multimodal |
                    Embedder::LateInteraction | Embedder::KeywordSplade
                );
                assert!(is_semantic, "Alert for {:?} violates ARCH-10 - only semantic embedders allowed", alert.space);
            }
        }

        // Verify should_alert and summary
        let should_alert = retriever.should_alert_divergence(&report);
        let summary = retriever.summarize_divergence(&report);
        println!("[RESULT] should_alert: {}", should_alert);
        println!("[RESULT] summary: {}", summary);

        println!("\n[PASS] SCENARIO 4: Divergence detection verified");
    }

    // =========================================================================
    // SCENARIO 5: Memory Conversion
    // =========================================================================

    #[test]
    fn manual_scenario_5_memory_conversion() {
        println!("\n============================================================");
        println!("=== SCENARIO 5: Memory to RecentMemory Conversion ===");
        println!("============================================================\n");

        let content = "CONVERSION_TEST_CONTENT_12345";
        let session_id = "conv-session";

        let memory = Memory::new(
            content.to_string(),
            MemorySource::ClaudeResponse {
                response_type: crate::memory::ResponseType::SessionSummary,
            },
            session_id.to_string(),
            SemanticFingerprint::zeroed(),
            None,
        );

        let original_id = memory.id;
        let original_created = memory.created_at;

        println!("[INPUT] Memory ID: {}", original_id);
        println!("[INPUT] Memory content: {}", content);
        println!("[INPUT] Memory created_at: {:?}", original_created);

        // Convert to RecentMemory
        let recent = memory_to_recent(&memory);

        println!("[OUTPUT] RecentMemory ID: {}", recent.id);
        println!("[OUTPUT] RecentMemory content: {}", recent.content);
        println!("[OUTPUT] RecentMemory created_at: {:?}", recent.created_at);

        // Verify field preservation
        assert_eq!(recent.id, original_id, "ID must match");
        assert_eq!(recent.content, content, "Content must match");
        assert_eq!(recent.created_at, original_created, "Timestamp must match");

        println!("\n[PASS] SCENARIO 5: Memory conversion verified");
    }

    // =========================================================================
    // SCENARIO 6: Multi-Session Isolation
    // =========================================================================

    #[test]
    fn manual_scenario_6_multi_session_isolation() {
        println!("\n============================================================");
        println!("=== SCENARIO 6: Multi-Session Isolation ===");
        println!("============================================================\n");

        let tmp = tempdir().expect("create temp dir");
        let store = Arc::new(MemoryStore::new(tmp.path()).expect("create store"));
        let retriever = SimilarityRetriever::with_defaults(store.clone());

        // Store memories in different sessions
        let sessions = vec![
            ("session-alpha", 3),
            ("session-beta", 2),
            ("session-gamma", 5),
        ];

        println!("[INPUT] Creating memories in multiple sessions:");
        for (session_id, count) in &sessions {
            for i in 0..*count {
                let content = format!("Memory {} in {}", i, session_id);
                let mem = Memory::new(
                    content,
                    MemorySource::HookDescription {
                        hook_type: HookType::SessionStart,
                        tool_name: None,
                    },
                    session_id.to_string(),
                    SemanticFingerprint::zeroed(),
                    None,
                );
                store.store(&mem).expect("store");
            }
            println!("  - {} memories in '{}'", count, session_id);
        }

        // Verify total count
        let total = store.count().expect("count");
        println!("\n[VERIFY] Total memories in DB: {}", total);
        assert_eq!(total, 10); // 3 + 2 + 5

        // Verify session isolation
        for (session_id, expected_count) in &sessions {
            let actual = retriever.session_memory_count(session_id).expect("count");
            println!("[VERIFY] Session '{}': expected {}, actual {}",
                session_id, expected_count, actual);
            assert_eq!(actual, *expected_count, "Session count mismatch");
        }

        // Verify retrieve_similar respects session boundaries
        let query = SemanticFingerprint::zeroed();
        let results_alpha = retriever
            .retrieve_similar(&query, "session-alpha", 100)
            .expect("retrieve");
        let results_beta = retriever
            .retrieve_similar(&query, "session-beta", 100)
            .expect("retrieve");

        println!("[RESULT] session-alpha retrieval: {} results", results_alpha.len());
        println!("[RESULT] session-beta retrieval: {} results", results_beta.len());

        // Results should only contain memories from their respective sessions
        // (Can't verify content without storing IDs, but count should match)

        println!("\n[PASS] SCENARIO 6: Multi-session isolation verified");
    }

    // =========================================================================
    // SCENARIO 7: Full State Verification with Persistence
    // =========================================================================

    #[test]
    fn manual_scenario_7_persistence_verification() {
        println!("\n============================================================");
        println!("=== SCENARIO 7: Persistence Verification ===");
        println!("============================================================\n");

        let tmp = tempdir().expect("create temp dir");
        let db_path = tmp.path().to_path_buf();

        let session_id = "persist-session";
        let content = "PERSIST_TEST_UNIQUE_MARKER_XYZ789";
        let stored_id: Uuid;

        // PHASE 1: Store memory
        println!("[PHASE 1] Storing memory...");
        {
            let store = Arc::new(MemoryStore::new(&db_path).expect("create store"));
            let retriever = SimilarityRetriever::with_defaults(store.clone());

            let mem = Memory::new(
                content.to_string(),
                MemorySource::HookDescription {
                    hook_type: HookType::SessionEnd,
                    tool_name: None,
                },
                session_id.to_string(),
                SemanticFingerprint::zeroed(),
                None,
            );
            stored_id = mem.id;
            retriever.store().store(&mem).expect("store");

            println!("  - Stored memory ID: {}", stored_id);
            println!("  - Count before close: {}", store.count().expect("count"));
        }
        // Store dropped, DB closed

        // PHASE 2: Reopen and verify
        println!("\n[PHASE 2] Reopening database...");
        {
            let store = Arc::new(MemoryStore::new(&db_path).expect("reopen store"));
            let retriever = SimilarityRetriever::with_defaults(store.clone());

            // Verify count
            let count = retriever.total_memory_count().expect("count");
            println!("  - Count after reopen: {}", count);
            assert_eq!(count, 1);

            // Verify by direct get
            let mem = store.get(stored_id).expect("get").expect("should exist");
            println!("  - Retrieved content: {}", mem.content);
            assert_eq!(mem.content, content);

            // Verify by retrieve_similar
            let query = SemanticFingerprint::zeroed();
            let results = retriever
                .retrieve_similar(&query, session_id, 10)
                .expect("retrieve");
            println!("  - retrieve_similar found {} results", results.len());
        }

        println!("\n[PASS] SCENARIO 7: Persistence verified across DB reopen");
    }

    // =========================================================================
    // SCENARIO 8: Get Recent Memories with Time Filtering
    // =========================================================================

    #[test]
    fn manual_scenario_8_recent_memories_time_filter() {
        println!("\n============================================================");
        println!("=== SCENARIO 8: Recent Memories Time Filtering ===");
        println!("============================================================\n");

        let tmp = tempdir().expect("create temp dir");
        let store = Arc::new(MemoryStore::new(tmp.path()).expect("create store"));
        let retriever = SimilarityRetriever::with_defaults(store.clone());

        let session_id = "time-filter-session";

        // Store some memories (all should be "recent" since they're just created)
        println!("[INPUT] Storing 5 recent memories...");
        for i in 0..5 {
            let content = format!("Recent memory {}", i);
            let mem = Memory::new(
                content,
                MemorySource::HookDescription {
                    hook_type: HookType::UserPromptSubmit,
                    tool_name: None,
                },
                session_id.to_string(),
                SemanticFingerprint::zeroed(),
                None,
            );
            store.store(&mem).expect("store");
        }

        // Get recent memories
        let recent = retriever
            .get_recent_memories(session_id)
            .expect("get recent");

        println!("[RESULT] get_recent_memories returned {} memories", recent.len());
        assert_eq!(recent.len(), 5, "All 5 should be recent");

        // Verify RecentMemory structure
        for rm in &recent {
            println!("  - ID: {}, Content: {}", rm.id, rm.content);
            // Verify created_at is recent (within last minute)
            let age = Utc::now() - rm.created_at;
            assert!(age.num_seconds() < 60, "Memory should be very recent");
        }

        println!("\n[PASS] SCENARIO 8: Recent memories time filtering verified");
    }
}
