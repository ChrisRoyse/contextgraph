//! Full State Verification Test for Divergence Module
//!
//! This test performs comprehensive manual testing with synthetic data,
//! verifying all edge cases and checking the actual outputs.

use context_graph_core::retrieval::{
    DivergenceAlert, DivergenceReport, DivergenceSeverity,
    DIVERGENCE_SPACES, MAX_SUMMARY_LEN, truncate_summary,
};
use context_graph_core::teleological::Embedder;
use context_graph_core::embeddings::category::category_for;
use uuid::Uuid;

fn main() {
    println!("{}", "=".repeat(80));
    println!("FULL STATE VERIFICATION - DivergenceAlert Types (TASK-P3-002)");
    println!("{}", "=".repeat(80));
    println!();

    // =========================================================================
    // TEST 1: DIVERGENCE_SPACES Constant Verification
    // =========================================================================
    println!("TEST 1: DIVERGENCE_SPACES Constant Verification");
    println!("{}", "-".repeat(60));
    
    // Source of Truth: DIVERGENCE_SPACES constant
    // AP-77: E5 (Causal) excluded — requires CausalDirection for meaningful scores
    assert_eq!(DIVERGENCE_SPACES.len(), 6, "DIVERGENCE_SPACES must have exactly 6 embedders (E5 excluded per AP-77)");
    println!("  Count: {} (expected: 6, E5 excluded per AP-77) [PASS]", DIVERGENCE_SPACES.len());

    // Verify each embedder
    let expected = [
        (Embedder::Semantic, "E1"),
        (Embedder::Sparse, "E6"),
        (Embedder::Code, "E7"),
        (Embedder::Multimodal, "E10"),
        (Embedder::LateInteraction, "E12"),
        (Embedder::KeywordSplade, "E13"),
    ];
    
    for (i, (embedder, short_name)) in expected.iter().enumerate() {
        assert_eq!(DIVERGENCE_SPACES[i], *embedder, "Position {} mismatch", i);
        let cat = category_for(DIVERGENCE_SPACES[i]);
        assert!(cat.is_semantic(), "{:?} is not semantic", DIVERGENCE_SPACES[i]);
        println!("  [{}]: {:?} = {} is semantic: {} [PASS]", i, DIVERGENCE_SPACES[i], short_name, cat.is_semantic());
    }
    
    // Verify exclusions
    let excluded = [
        (Embedder::Causal, "E5", "AP-77: requires CausalDirection"),
        (Embedder::TemporalRecent, "E2", "temporal"),
        (Embedder::TemporalPeriodic, "E3", "temporal"),
        (Embedder::TemporalPositional, "E4", "temporal"),
        (Embedder::Emotional, "E8", "relational"),
        (Embedder::Entity, "E11", "relational"),
        (Embedder::Hdc, "E9", "structural"),
    ];
    
    for (embedder, short_name, reason) in excluded.iter() {
        assert!(!DIVERGENCE_SPACES.contains(embedder), "{:?} should be excluded", embedder);
        println!("  {} ({}) correctly excluded (reason: {}) [PASS]", short_name, embedder.name(), reason);
    }
    println!();

    // =========================================================================
    // TEST 2: DivergenceSeverity Threshold Mapping
    // =========================================================================
    println!("TEST 2: DivergenceSeverity Threshold Mapping");
    println!("{}", "-".repeat(60));
    
    // Test exact boundary values
    let test_cases = [
        (0.00, DivergenceSeverity::High, "High"),
        (0.05, DivergenceSeverity::High, "High"),
        (0.09, DivergenceSeverity::High, "High"),
        (0.10, DivergenceSeverity::Medium, "Medium"),  // Exact boundary
        (0.15, DivergenceSeverity::Medium, "Medium"),
        (0.19, DivergenceSeverity::Medium, "Medium"),
        (0.20, DivergenceSeverity::Low, "Low"),        // Exact boundary
        (0.25, DivergenceSeverity::Low, "Low"),
        (0.99, DivergenceSeverity::Low, "Low"),
        (1.00, DivergenceSeverity::Low, "Low"),
    ];
    
    println!("  Score thresholds: <0.10=High, 0.10-0.20=Medium, >=0.20=Low");
    for (score, expected_sev, label) in test_cases.iter() {
        let actual = DivergenceSeverity::from_score(*score);
        assert_eq!(actual, *expected_sev, "Score {} should be {:?}", score, expected_sev);
        assert_eq!(actual.as_str(), *label);
        println!("  Score {:.2} -> {} [PASS]", score, actual);
    }
    println!();

    // =========================================================================
    // TEST 3: DivergenceAlert Creation and Field Verification
    // =========================================================================
    println!("TEST 3: DivergenceAlert Creation and Field Verification");
    println!("{}", "-".repeat(60));
    
    // Known input
    let memory_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let content = "Working on the DivergenceAlert implementation for context-graph-core";
    
    let alert = DivergenceAlert::new(memory_id, Embedder::Semantic, 0.15, content);
    
    // Verify all fields (Source of Truth: struct fields)
    println!("  Input: memory_id={}", memory_id);
    println!("  Input: space=Embedder::Semantic");
    println!("  Input: score=0.15");
    println!("  Input: content=\"{}\" (len={})", content, content.len());
    println!();
    println!("  Output verification:");
    assert_eq!(alert.memory_id, memory_id);
    println!("    memory_id: {} [PASS]", alert.memory_id);
    
    assert_eq!(alert.space, Embedder::Semantic);
    println!("    space: {:?} [PASS]", alert.space);
    
    assert_eq!(alert.similarity_score, 0.15);
    println!("    similarity_score: {} [PASS]", alert.similarity_score);
    
    assert_eq!(alert.memory_summary, content);  // Content < 100 chars
    println!("    memory_summary: \"{}\" (len={}) [PASS]", alert.memory_summary, alert.memory_summary.len());
    
    assert_eq!(alert.severity(), DivergenceSeverity::Medium);
    println!("    severity(): {:?} (0.15 in 0.10..0.20) [PASS]", alert.severity());
    println!();

    // =========================================================================
    // TEST 4: Score Clamping Edge Cases
    // =========================================================================
    println!("TEST 4: Score Clamping Edge Cases");
    println!("{}", "-".repeat(60));
    
    let id = Uuid::new_v4();
    
    // Test score > 1.0
    let alert_high = DivergenceAlert::new(id, Embedder::Code, 1.5, "test");
    assert_eq!(alert_high.similarity_score, 1.0, "Score 1.5 should clamp to 1.0");
    println!("  Input: 1.5 -> Output: {} [PASS]", alert_high.similarity_score);
    
    // Test score < 0.0
    let alert_low = DivergenceAlert::new(id, Embedder::Code, -0.5, "test");
    assert_eq!(alert_low.similarity_score, 0.0, "Score -0.5 should clamp to 0.0");
    println!("  Input: -0.5 -> Output: {} [PASS]", alert_low.similarity_score);
    
    // Test NaN and Infinity behavior would panic - we don't test those
    println!();

    // =========================================================================
    // TEST 5: Summary Truncation Edge Cases
    // =========================================================================
    println!("TEST 5: Summary Truncation Edge Cases");
    println!("{}", "-".repeat(60));
    
    // Test: Content shorter than max
    let short = "Hello world";
    let result = truncate_summary(short, MAX_SUMMARY_LEN);
    assert_eq!(result, "Hello world");
    println!("  Short content ({} chars): \"{}\" -> \"{}\" [PASS]", short.len(), short, result);
    
    // Test: Content exactly max length
    let exact = "a".repeat(100);
    let result = truncate_summary(&exact, 100);
    assert_eq!(result.len(), 100);
    assert!(!result.contains("..."));
    println!("  Exact length (100 chars): len={}, no ellipsis [PASS]", result.len());
    
    // Test: Content needs truncation with word boundary
    let long = "This is a long sentence that needs to be truncated at word boundary for readability";
    let result = truncate_summary(long, 50);
    assert!(result.len() <= 50);
    assert!(result.ends_with("..."));
    println!("  Word boundary truncation: \"{}\" (len={}) [PASS]", result, result.len());
    
    // Test: Content with no spaces (hard truncation)
    let no_spaces = "a".repeat(200);
    let result = truncate_summary(&no_spaces, 50);
    assert_eq!(result.len(), 50);
    assert!(result.ends_with("..."));
    println!("  No-space truncation: len={} with ellipsis [PASS]", result.len());
    
    // Test: Empty content
    let empty = "";
    let result = truncate_summary(empty, 100);
    assert_eq!(result, "");
    println!("  Empty content: \"{}\" [PASS]", result);
    
    // Test: Whitespace-only content
    let whitespace = "     ";
    let result = truncate_summary(whitespace, 100);
    assert_eq!(result, "");
    println!("  Whitespace-only: \"{}\" [PASS]", result);
    println!();

    // =========================================================================
    // TEST 6: DivergenceReport Operations
    // =========================================================================
    println!("TEST 6: DivergenceReport Operations");
    println!("{}", "-".repeat(60));
    
    // Test: Empty report
    let report = DivergenceReport::new();
    assert!(report.is_empty());
    assert_eq!(report.len(), 0);
    assert!(report.most_severe().is_none());
    assert_eq!(report.format_all(), "");
    println!("  Empty report: is_empty={}, len={}, most_severe=None, format_all=\"\" [PASS]", report.is_empty(), report.len());
    
    // Test: Add alerts and verify order
    let mut report = DivergenceReport::new();
    let id1 = Uuid::new_v4();
    let id2 = Uuid::new_v4();
    let id3 = Uuid::new_v4();
    
    report.add(DivergenceAlert::new(id1, Embedder::Semantic, 0.25, "low"));
    report.add(DivergenceAlert::new(id2, Embedder::Code, 0.05, "high"));  // Most severe
    report.add(DivergenceAlert::new(id3, Embedder::Causal, 0.15, "medium"));
    
    assert_eq!(report.len(), 3);
    println!("  After adding 3 alerts: len={} [PASS]", report.len());
    
    // Verify most_severe returns lowest score
    let most_severe = report.most_severe().unwrap();
    assert_eq!(most_severe.similarity_score, 0.05);
    assert_eq!(most_severe.severity(), DivergenceSeverity::High);
    println!("  most_severe(): score={}, severity={:?} [PASS]", most_severe.similarity_score, most_severe.severity());
    
    // Test sort_by_severity
    report.sort_by_severity();
    assert_eq!(report.alerts[0].similarity_score, 0.05);  // Most severe first
    assert_eq!(report.alerts[1].similarity_score, 0.15);
    assert_eq!(report.alerts[2].similarity_score, 0.25);  // Least severe last
    println!("  sort_by_severity(): [0.05, 0.15, 0.25] [PASS]");
    
    // Test count_by_severity
    let mut report2 = DivergenceReport::new();
    report2.add(DivergenceAlert::new(Uuid::new_v4(), Embedder::Semantic, 0.05, "h1"));
    report2.add(DivergenceAlert::new(Uuid::new_v4(), Embedder::Code, 0.08, "h2"));
    report2.add(DivergenceAlert::new(Uuid::new_v4(), Embedder::Causal, 0.15, "m1"));
    report2.add(DivergenceAlert::new(Uuid::new_v4(), Embedder::Sparse, 0.22, "l1"));
    report2.add(DivergenceAlert::new(Uuid::new_v4(), Embedder::Multimodal, 0.28, "l2"));
    
    let (high, medium, low) = report2.count_by_severity();
    assert_eq!(high, 2);
    assert_eq!(medium, 1);
    assert_eq!(low, 2);
    println!("  count_by_severity(): high={}, medium={}, low={} [PASS]", high, medium, low);
    println!();

    // =========================================================================
    // TEST 7: Serialization Roundtrip
    // =========================================================================
    println!("TEST 7: Serialization Roundtrip");
    println!("{}", "-".repeat(60));
    
    // Test DivergenceAlert
    let id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440001").unwrap();
    let alert = DivergenceAlert::new(id, Embedder::Semantic, 0.15, "test content");
    let json = serde_json::to_string(&alert).expect("serialize alert");
    let recovered: DivergenceAlert = serde_json::from_str(&json).expect("deserialize alert");
    assert_eq!(recovered.memory_id, id);
    assert_eq!(recovered.space, Embedder::Semantic);
    assert_eq!(recovered.similarity_score, 0.15);
    println!("  DivergenceAlert: JSON -> struct -> fields match [PASS]");
    println!("    JSON: {}", &json[..json.len().min(80)]); 
    
    // Test DivergenceSeverity
    let severity = DivergenceSeverity::High;
    let json = serde_json::to_string(&severity).expect("serialize severity");
    let recovered: DivergenceSeverity = serde_json::from_str(&json).expect("deserialize severity");
    assert_eq!(recovered, DivergenceSeverity::High);
    println!("  DivergenceSeverity: {} -> {:?} [PASS]", json, recovered);
    
    // Test DivergenceReport
    let mut report = DivergenceReport::new();
    report.add(DivergenceAlert::new(Uuid::new_v4(), Embedder::Code, 0.12, "test"));
    let json = serde_json::to_string(&report).expect("serialize report");
    let recovered: DivergenceReport = serde_json::from_str(&json).expect("deserialize report");
    assert_eq!(recovered.len(), 1);
    println!("  DivergenceReport: len={} after roundtrip [PASS]", recovered.len());
    println!();

    // =========================================================================
    // TEST 8: Format Output Verification
    // =========================================================================
    println!("TEST 8: Format Output Verification");
    println!("{}", "-".repeat(60));
    
    let id = Uuid::new_v4();
    let alert = DivergenceAlert::new(id, Embedder::Code, 0.15, "Implementing test suite");
    
    let formatted = alert.format_alert();
    assert!(formatted.contains("DIVERGENCE"));
    assert!(formatted.contains("E7"));
    assert!(formatted.contains("Implementing test suite"));
    assert!(formatted.contains("0.15"));
    println!("  format_alert(): \"{}\" [PASS]", formatted);
    
    let formatted_sev = alert.format_with_severity();
    assert!(formatted_sev.starts_with("[Medium]"));
    println!("  format_with_severity(): \"{}\" [PASS]", formatted_sev);
    
    // Test format_all for report
    let mut report = DivergenceReport::new();
    report.add(DivergenceAlert::new(Uuid::new_v4(), Embedder::Semantic, 0.15, "first"));
    report.add(DivergenceAlert::new(Uuid::new_v4(), Embedder::Code, 0.20, "second"));
    let formatted = report.format_all();
    let lines: Vec<&str> = formatted.lines().collect();
    assert_eq!(lines.len(), 2);
    println!("  format_all() produces {} lines [PASS]", lines.len());
    for (i, line) in lines.iter().enumerate() {
        println!("    Line {}: {}", i, line);
    }
    println!();

    // =========================================================================
    // FINAL SUMMARY
    // =========================================================================
    println!("{}", "=".repeat(80));
    println!("FULL STATE VERIFICATION COMPLETE");
    println!("{}", "=".repeat(80));
    println!();
    println!("Source of Truth Verification:");
    println!("  - File exists: crates/context-graph-core/src/retrieval/divergence.rs");
    println!("  - Module exported: pub mod divergence in mod.rs");
    println!("  - Re-exports present: DivergenceAlert, DivergenceReport, etc.");
    println!("  - DIVERGENCE_SPACES: 6 semantic embedders (E1, E6, E7, E10, E12, E13; E5 excluded per AP-77)");
    println!();
    println!("All tests passed successfully!");
}
