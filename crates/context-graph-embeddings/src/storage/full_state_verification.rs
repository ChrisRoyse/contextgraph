//! Full State Verification Tests for TASK-EMB-023
//!
//! These tests prove the physical state of the system BEFORE and AFTER each operation.
//! They verify actual computed values, not just return codes.

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;
    use uuid::Uuid;

    use crate::error::EmbeddingError;
    use crate::storage::multi_space::{
        MultiSpaceIndexProvider, MultiSpaceSearchEngine, QuantizedFingerprintRetriever,
    };
    use crate::storage::types::{StoredQuantizedFingerprint, RRF_K};

    // =========================================================================
    // TEST FIXTURES
    // =========================================================================

    struct VerificationStorage {
        topic_profiles: HashMap<Uuid, [f32; 13]>,
    }

    impl VerificationStorage {
        fn new() -> Self {
            Self {
                topic_profiles: HashMap::new(),
            }
        }
    }

    impl QuantizedFingerprintRetriever for VerificationStorage {
        fn get_fingerprint(
            &self,
            _id: Uuid,
        ) -> Result<Option<StoredQuantizedFingerprint>, EmbeddingError> {
            Ok(None)
        }

        fn get_topic_profile(&self, id: Uuid) -> Result<Option<[f32; 13]>, EmbeddingError> {
            Ok(self.topic_profiles.get(&id).copied())
        }
    }

    struct VerificationHnswManager {
        results: HashMap<u8, Vec<(Uuid, f32)>>,
    }

    impl VerificationHnswManager {
        fn new() -> Self {
            Self {
                results: HashMap::new(),
            }
        }

        fn set_results(&mut self, embedder_idx: u8, results: Vec<(Uuid, f32)>) {
            self.results.insert(embedder_idx, results);
        }
    }

    impl MultiSpaceIndexProvider for VerificationHnswManager {
        fn search_embedder(
            &self,
            embedder_idx: u8,
            _query: &[f32],
            k: usize,
        ) -> Result<Vec<(Uuid, f32)>, EmbeddingError> {
            Ok(self
                .results
                .get(&embedder_idx)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .take(k)
                .collect())
        }

        fn embedder_uses_hnsw(&self, embedder_idx: u8) -> bool {
            embedder_idx != 5 && embedder_idx != 11
        }
    }

    // =========================================================================
    // EDGE CASE 1: RRF with Single Embedder
    // =========================================================================

    #[test]
    fn full_state_verify_edge_case_1_single_embedder_rrf() {
        eprintln!(
            "\n================================================================================"
        );
        eprintln!("FULL STATE VERIFICATION - EDGE CASE 1: Single Embedder RRF");
        eprintln!(
            "================================================================================\n"
        );

        // Create known UUIDs for tracking
        let id_rank0 = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let id_rank1 = Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap();
        let id_rank2 = Uuid::parse_str("00000000-0000-0000-0000-000000000003").unwrap();

        // BEFORE STATE
        eprintln!(">>> BEFORE STATE:");
        eprintln!("    Input documents:");
        eprintln!("      - id_rank0: {} (similarity=0.95, rank=0)", id_rank0);
        eprintln!("      - id_rank1: {} (similarity=0.80, rank=1)", id_rank1);
        eprintln!("      - id_rank2: {} (similarity=0.65, rank=2)", id_rank2);
        eprintln!("    RRF_K constant: {}", RRF_K);
        eprintln!("    Expected RRF scores:");
        let expected_rrf_0 = 1.0 / (RRF_K + 0.0 + 1.0);
        let expected_rrf_1 = 1.0 / (RRF_K + 1.0 + 1.0);
        let expected_rrf_2 = 1.0 / (RRF_K + 2.0 + 1.0);
        eprintln!("      - rank 0: 1/(60+0+1) = {:.10}", expected_rrf_0);
        eprintln!("      - rank 1: 1/(60+1+1) = {:.10}", expected_rrf_1);
        eprintln!("      - rank 2: 1/(60+2+1) = {:.10}", expected_rrf_2);

        // Setup
        let storage = Arc::new(VerificationStorage::new());
        let mut hnsw = VerificationHnswManager::new();
        hnsw.set_results(
            0,
            vec![(id_rank0, 0.95), (id_rank1, 0.80), (id_rank2, 0.65)],
        );

        let engine = MultiSpaceSearchEngine::new(storage, Arc::new(hnsw));

        let mut queries = HashMap::new();
        queries.insert(0, vec![0.0f32; 1024]);

        // EXECUTE
        eprintln!("\n>>> EXECUTING: search_multi_space()");
        let results = engine.search_multi_space(&queries, None, 10, 10).unwrap();

        // AFTER STATE - Physical inspection
        eprintln!("\n>>> AFTER STATE (Physical Inspection of Results):");
        eprintln!("    Number of results returned: {}", results.len());

        for (i, result) in results.iter().enumerate() {
            eprintln!("\n    Result[{}]:", i);
            eprintln!("      - ID: {}", result.id);
            eprintln!("      - Computed RRF score: {:.10}", result.rrf_score);
            eprintln!("      - Embedder count: {}", result.embedder_count);
            eprintln!(
                "      - Embedder similarities[0]: {:.4}",
                result.embedder_similarities[0]
            );

            // Verify against expected
            let expected = 1.0 / (RRF_K + i as f32 + 1.0);
            let diff = (result.rrf_score - expected).abs();
            eprintln!("      - Expected RRF: {:.10}", expected);
            eprintln!("      - Difference: {:.15}", diff);
            eprintln!(
                "      - MATCH: {}",
                if diff < 0.0001 { "YES" } else { "NO" }
            );

            assert!(diff < 0.0001, "RRF mismatch at rank {}", i);
        }

        // Verify ordering
        eprintln!("\n>>> VERIFICATION:");
        eprintln!(
            "    Result[0].id == id_rank0: {}",
            results[0].id == id_rank0
        );
        eprintln!(
            "    Result[1].id == id_rank1: {}",
            results[1].id == id_rank1
        );
        eprintln!(
            "    Result[2].id == id_rank2: {}",
            results[2].id == id_rank2
        );

        assert_eq!(results[0].id, id_rank0, "Rank 0 ID mismatch");
        assert_eq!(results[1].id, id_rank1, "Rank 1 ID mismatch");
        assert_eq!(results[2].id, id_rank2, "Rank 2 ID mismatch");

        eprintln!("\n>>> EDGE CASE 1: VERIFIED");
        eprintln!(
            "================================================================================\n"
        );
    }

    // =========================================================================
    // EDGE CASE 2: Partial Space Coverage
    // =========================================================================

    #[test]
    fn full_state_verify_edge_case_2_partial_coverage() {
        eprintln!(
            "\n================================================================================"
        );
        eprintln!("FULL STATE VERIFICATION - EDGE CASE 2: Partial Space Coverage");
        eprintln!(
            "================================================================================\n"
        );

        let doc_partial = Uuid::parse_str("aaaa0000-0000-0000-0000-000000000001").unwrap();
        let doc_full = Uuid::parse_str("bbbb0000-0000-0000-0000-000000000002").unwrap();

        // BEFORE STATE
        eprintln!(">>> BEFORE STATE:");
        eprintln!(
            "    doc_partial ({}): appears ONLY in E1 (embedder 0)",
            doc_partial
        );
        eprintln!("    doc_full ({}): appears in BOTH E1 and E2", doc_full);
        eprintln!("    Expected behavior:");
        eprintln!("      - doc_partial.embedder_similarities[0] = 0.90 (present)");
        eprintln!("      - doc_partial.embedder_similarities[1] = NaN (missing)");
        eprintln!("      - doc_full.embedder_similarities[0] = 0.80 (present)");
        eprintln!("      - doc_full.embedder_similarities[1] = 0.85 (present)");

        // Setup
        let storage = Arc::new(VerificationStorage::new());
        let mut hnsw = VerificationHnswManager::new();
        hnsw.set_results(0, vec![(doc_partial, 0.90), (doc_full, 0.80)]);
        hnsw.set_results(1, vec![(doc_full, 0.85)]);

        let engine = MultiSpaceSearchEngine::new(storage, Arc::new(hnsw));

        let mut queries = HashMap::new();
        queries.insert(0, vec![0.0f32; 1024]);
        queries.insert(1, vec![0.0f32; 512]);

        // EXECUTE
        eprintln!("\n>>> EXECUTING: search_multi_space() with 2 embedders");
        let results = engine.search_multi_space(&queries, None, 10, 10).unwrap();

        // AFTER STATE - Physical inspection
        eprintln!("\n>>> AFTER STATE (Physical Inspection):");
        eprintln!("    Total results: {}", results.len());

        let partial = results.iter().find(|r| r.id == doc_partial).unwrap();
        let full = results.iter().find(|r| r.id == doc_full).unwrap();

        eprintln!("\n    doc_partial inspection:");
        eprintln!(
            "      - embedder_similarities[0]: {:.4}",
            partial.embedder_similarities[0]
        );
        eprintln!(
            "      - embedder_similarities[1]: {} (is_nan={})",
            partial.embedder_similarities[1],
            partial.embedder_similarities[1].is_nan()
        );
        eprintln!("      - embedder_count: {}", partial.embedder_count);
        eprintln!("      - rrf_score: {:.10}", partial.rrf_score);

        eprintln!("\n    doc_full inspection:");
        eprintln!(
            "      - embedder_similarities[0]: {:.4}",
            full.embedder_similarities[0]
        );
        eprintln!(
            "      - embedder_similarities[1]: {:.4}",
            full.embedder_similarities[1]
        );
        eprintln!("      - embedder_count: {}", full.embedder_count);
        eprintln!("      - rrf_score: {:.10}", full.rrf_score);

        // Expected RRF calculations
        let partial_expected_rrf = 1.0 / (RRF_K + 0.0 + 1.0); // rank 0 in E1 only (1-indexed)
        let full_expected_rrf = 1.0 / (RRF_K + 1.0 + 1.0) + 1.0 / (RRF_K + 0.0 + 1.0); // rank 1 in E1, rank 0 in E2 (1-indexed)

        eprintln!("\n>>> VERIFICATION:");
        eprintln!(
            "    partial.embedder_similarities[0] is NOT NaN: {}",
            !partial.embedder_similarities[0].is_nan()
        );
        eprintln!(
            "    partial.embedder_similarities[1] IS NaN: {}",
            partial.embedder_similarities[1].is_nan()
        );
        eprintln!(
            "    partial.embedder_count == 1: {}",
            partial.embedder_count == 1
        );
        eprintln!("    full.embedder_count == 2: {}", full.embedder_count == 2);
        eprintln!("    Expected partial RRF: {:.10}", partial_expected_rrf);
        eprintln!("    Expected full RRF: {:.10}", full_expected_rrf);

        assert!(!partial.embedder_similarities[0].is_nan());
        assert!(partial.embedder_similarities[1].is_nan());
        assert_eq!(partial.embedder_count, 1);
        assert_eq!(full.embedder_count, 2);

        eprintln!("\n>>> EDGE CASE 2: VERIFIED");
        eprintln!(
            "================================================================================\n"
        );
    }

    // =========================================================================
    // EDGE CASE 3: Tied Similarities - RRF Breaks Ties
    // =========================================================================

    #[test]
    fn full_state_verify_edge_case_3_tied_similarities() {
        eprintln!(
            "\n================================================================================"
        );
        eprintln!("FULL STATE VERIFICATION - EDGE CASE 3: Tied Similarities");
        eprintln!(
            "================================================================================\n"
        );

        let id_0 = Uuid::parse_str("cccc0000-0000-0000-0000-000000000001").unwrap();
        let id_1 = Uuid::parse_str("cccc0000-0000-0000-0000-000000000002").unwrap();
        let id_2 = Uuid::parse_str("cccc0000-0000-0000-0000-000000000003").unwrap();

        // BEFORE STATE
        eprintln!(">>> BEFORE STATE:");
        eprintln!("    All documents have IDENTICAL similarity = 0.80");
        eprintln!("    But they have DIFFERENT ranks:");
        eprintln!("      - id_0 ({}) at rank 0", id_0);
        eprintln!("      - id_1 ({}) at rank 1", id_1);
        eprintln!("      - id_2 ({}) at rank 2", id_2);
        eprintln!("    RRF should break the tie by rank (lower rank = higher score)");

        // Setup
        let storage = Arc::new(VerificationStorage::new());
        let mut hnsw = VerificationHnswManager::new();
        hnsw.set_results(
            0,
            vec![
                (id_0, 0.80), // All same similarity
                (id_1, 0.80),
                (id_2, 0.80),
            ],
        );

        let engine = MultiSpaceSearchEngine::new(storage, Arc::new(hnsw));

        let mut queries = HashMap::new();
        queries.insert(0, vec![0.0f32; 1024]);

        // EXECUTE
        eprintln!("\n>>> EXECUTING: search_multi_space() with tied similarities");
        let results = engine.search_multi_space(&queries, None, 10, 10).unwrap();

        // AFTER STATE - Physical inspection
        eprintln!("\n>>> AFTER STATE (Physical Inspection):");
        for (i, result) in results.iter().enumerate() {
            eprintln!("    Result[{}]:", i);
            eprintln!("      - ID: {}", result.id);
            eprintln!("      - RRF score: {:.10}", result.rrf_score);
            eprintln!(
                "      - similarity[0]: {:.4}",
                result.embedder_similarities[0]
            );
        }

        // Physical verification of ordering
        eprintln!("\n>>> VERIFICATION OF TIE-BREAKING:");
        eprintln!(
            "    results[0].rrf_score ({:.10}) > results[1].rrf_score ({:.10}): {}",
            results[0].rrf_score,
            results[1].rrf_score,
            results[0].rrf_score > results[1].rrf_score
        );
        eprintln!(
            "    results[1].rrf_score ({:.10}) > results[2].rrf_score ({:.10}): {}",
            results[1].rrf_score,
            results[2].rrf_score,
            results[1].rrf_score > results[2].rrf_score
        );

        eprintln!("\n>>> VERIFICATION OF DETERMINISTIC ORDER:");
        eprintln!(
            "    results[0].id == id_0: {} (expected: true)",
            results[0].id == id_0
        );
        eprintln!(
            "    results[1].id == id_1: {} (expected: true)",
            results[1].id == id_1
        );
        eprintln!(
            "    results[2].id == id_2: {} (expected: true)",
            results[2].id == id_2
        );

        assert!(
            results[0].rrf_score > results[1].rrf_score,
            "RRF should decrease with rank"
        );
        assert!(
            results[1].rrf_score > results[2].rrf_score,
            "RRF should decrease with rank"
        );
        assert_eq!(results[0].id, id_0);
        assert_eq!(results[1].id, id_1);
        assert_eq!(results[2].id, id_2);

        eprintln!("\n>>> EDGE CASE 3: VERIFIED");
        eprintln!(
            "================================================================================\n"
        );
    }

    // =========================================================================
    // PHYSICAL PROOF: RRF Formula Calculation
    // =========================================================================

    #[test]
    fn full_state_verify_rrf_formula_physical_proof() {
        eprintln!(
            "\n================================================================================"
        );
        eprintln!("FULL STATE VERIFICATION - RRF FORMULA PHYSICAL PROOF");
        eprintln!(
            "================================================================================\n"
        );

        let target_id = Uuid::parse_str("dddd0000-0000-0000-0000-000000000001").unwrap();

        // BEFORE STATE - Mathematical expectation
        eprintln!(">>> BEFORE STATE (Mathematical Expectation):");
        eprintln!("    Constitution RRF formula: RRF(d) = Si wi / (k + ranki(d))");
        eprintln!("    Constitution k value: {} (from RRF_K constant)", RRF_K);
        eprintln!();
        eprintln!("    Test scenario:");
        eprintln!("      - target_id appears at rank 0 in E1 (embedder 0)");
        eprintln!("      - target_id appears at rank 2 in E2 (embedder 1)");
        eprintln!("      - All weights = 1.0 (default)");
        eprintln!();
        eprintln!("    Manual calculation:");
        let contrib_e1 = 1.0 / (60.0 + 0.0 + 1.0);
        let contrib_e2 = 1.0 / (60.0 + 2.0 + 1.0);
        let expected_total = contrib_e1 + contrib_e2;
        eprintln!(
            "      - E1 contribution: 1.0 / (60 + 0 + 1) = {:.15}",
            contrib_e1
        );
        eprintln!(
            "      - E2 contribution: 1.0 / (60 + 2 + 1) = {:.15}",
            contrib_e2
        );
        eprintln!("      - Expected total RRF: {:.15}", expected_total);

        // Setup
        let storage = Arc::new(VerificationStorage::new());
        let mut hnsw = VerificationHnswManager::new();
        hnsw.set_results(0, vec![(target_id, 0.90)]); // rank 0
        hnsw.set_results(
            1,
            vec![
                (Uuid::new_v4(), 0.95), // rank 0 (different doc)
                (Uuid::new_v4(), 0.85), // rank 1 (different doc)
                (target_id, 0.75),      // rank 2
            ],
        );

        let engine = MultiSpaceSearchEngine::new(storage, Arc::new(hnsw));

        let mut queries = HashMap::new();
        queries.insert(0, vec![0.0f32; 1024]);
        queries.insert(1, vec![0.0f32; 512]);

        // EXECUTE
        eprintln!("\n>>> EXECUTING: search_multi_space()");
        let results = engine.search_multi_space(&queries, None, 10, 10).unwrap();

        // AFTER STATE - Physical inspection
        eprintln!("\n>>> AFTER STATE (Physical Inspection):");
        let target_result = results.iter().find(|r| r.id == target_id).unwrap();

        eprintln!("    target_id result found: true");
        eprintln!("    Computed RRF score: {:.15}", target_result.rrf_score);
        eprintln!("    Expected RRF score: {:.15}", expected_total);

        let diff = (target_result.rrf_score - expected_total).abs();
        eprintln!("    Absolute difference: {:.20}", diff);
        eprintln!("    Tolerance: 0.0001");
        eprintln!("    Within tolerance: {}", diff < 0.0001);

        eprintln!("\n>>> BREAKDOWN:");
        eprintln!(
            "    embedder_similarities[0] (E1): {:.4}",
            target_result.embedder_similarities[0]
        );
        eprintln!(
            "    embedder_similarities[1] (E2): {:.4}",
            target_result.embedder_similarities[1]
        );
        eprintln!("    embedder_count: {}", target_result.embedder_count);

        assert!(
            diff < 0.0001,
            "RRF formula mismatch! Expected {}, got {}",
            expected_total,
            target_result.rrf_score
        );

        eprintln!("\n>>> RRF FORMULA: VERIFIED");
        eprintln!(
            "    1/(60+0+1) + 1/(60+2+1) = {:.15} matches computed {:.15}",
            expected_total, target_result.rrf_score
        );
        eprintln!(
            "================================================================================\n"
        );
    }
}
