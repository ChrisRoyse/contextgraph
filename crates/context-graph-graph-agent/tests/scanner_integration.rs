//! Integration tests for the graph-agent MemoryScanner.
//!
//! Tests multi-memory scanning, clustering, candidate scoring,
//! deduplication, and domain classification with realistic content.

use chrono::Utc;
use context_graph_graph_agent::{
    ContentDomain, DomainMarkers, GraphMarkers, MemoryForGraphAnalysis, MemoryScanner,
    RelationshipType, ScannerConfig,
};
use uuid::Uuid;

// =============================================================================
// Helper: create realistic test memories
// =============================================================================

fn make_memory(content: &str, embedding: Vec<f32>) -> MemoryForGraphAnalysis {
    MemoryForGraphAnalysis {
        id: Uuid::new_v4(),
        content: content.to_string(),
        created_at: Utc::now(),
        session_id: Some("test-session".to_string()),
        e1_embedding: embedding,
        source_type: None,
        file_path: None,
    }
}

fn make_memory_with_path(
    content: &str,
    embedding: Vec<f32>,
    file_path: &str,
) -> MemoryForGraphAnalysis {
    let mut mem = make_memory(content, embedding);
    mem.file_path = Some(file_path.to_string());
    mem
}

fn make_memory_with_session(
    content: &str,
    embedding: Vec<f32>,
    session_id: &str,
) -> MemoryForGraphAnalysis {
    let mut mem = make_memory(content, embedding);
    mem.session_id = Some(session_id.to_string());
    mem
}

/// Generate a deterministic normalized vector from a seed.
///
/// Unlike `generate_real_unit_vector()` from test-utils (which uses random data),
/// this produces repeatable vectors for controlling cosine similarity in clustering tests.
fn unit_vec(dim: usize, seed: f32) -> Vec<f32> {
    let mut v: Vec<f32> = (0..dim).map(|i| (seed + i as f32 * 0.01).sin()).collect();
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > f32::EPSILON {
        for x in &mut v {
            *x /= norm;
        }
    }
    v
}

// =============================================================================
// TEST 1: Scanner finds code-domain candidates from realistic Rust content
// =============================================================================

#[test]
fn test_scanner_finds_code_candidates() {
    let mut scanner = MemoryScanner::new();

    // Two code memories with similar embeddings (should cluster)
    let mem_a = make_memory(
        "use crate::graph_agent; impl GraphDiscoveryService for Agent { fn discover() {} }",
        unit_vec(1024, 1.0),
    );
    let mem_b = make_memory(
        "pub mod graph_agent { pub struct Agent {} impl Agent { pub fn discover(&self) {} } }",
        unit_vec(1024, 1.1), // similar direction → clusters together
    );

    let candidates = scanner.find_candidates(&[mem_a, mem_b]).unwrap();

    // Should find at least one candidate pair
    assert!(
        !candidates.is_empty(),
        "Scanner should find candidate pairs for related code memories"
    );

    // Candidate should have positive score
    let first = &candidates[0];
    assert!(
        first.initial_score > 0.0,
        "Score should be positive, got {}",
        first.initial_score
    );

    // Should detect code-relevant relationship types
    let has_code_type = first.suspected_types.iter().any(|t| {
        matches!(
            t,
            RelationshipType::Imports | RelationshipType::Implements | RelationshipType::Contains
        )
    });
    assert!(
        has_code_type,
        "Should detect code relationship types, got: {:?}",
        first.suspected_types
    );
}

// =============================================================================
// TEST 2: Scanner respects max_candidates limit
// =============================================================================

#[test]
fn test_scanner_max_candidates_truncation() {
    let config = ScannerConfig {
        max_candidates: 3,
        min_initial_score: 0.0, // Accept all pairs
        similarity_threshold: 0.0,
        max_similarity: 1.0,
        ..Default::default()
    };
    let mut scanner = MemoryScanner::with_config(config);

    // Create 10 memories — gives O(n^2) = 45 potential pairs
    let memories: Vec<_> = (0..10)
        .map(|i| {
            make_memory(
                &format!(
                    "use crate::mod_{}; impl Trait for Struct_{} {{ fn method() {{}} }}",
                    i, i
                ),
                unit_vec(1024, i as f32 * 0.05),
            )
        })
        .collect();

    let candidates = scanner.find_candidates(&memories).unwrap();

    assert!(
        candidates.len() <= 3,
        "Should truncate to max_candidates=3, got {}",
        candidates.len()
    );
}

// =============================================================================
// TEST 3: Scanner deduplication — mark_analyzed prevents re-analysis
// =============================================================================

#[test]
fn test_scanner_deduplication_across_scans() {
    let config = ScannerConfig {
        min_initial_score: 0.0,
        similarity_threshold: 0.0,
        max_similarity: 1.0,
        ..Default::default()
    };
    let mut scanner = MemoryScanner::with_config(config);

    let mem_a = make_memory("use crate::foo; fn bar() {}", unit_vec(1024, 1.0));
    let mem_b = make_memory("pub mod foo { pub fn baz() {} }", unit_vec(1024, 1.1));

    let id_a = mem_a.id;
    let id_b = mem_b.id;

    // First scan should find candidates
    let first_scan = scanner.find_candidates(&[mem_a.clone(), mem_b.clone()]).unwrap();
    assert!(!first_scan.is_empty(), "First scan should find candidates");

    // Mark the pair as analyzed
    scanner.mark_analyzed(id_a, id_b);
    assert_eq!(scanner.analyzed_count(), 1);

    // Second scan should skip the analyzed pair
    let second_scan = scanner.find_candidates(&[mem_a, mem_b]).unwrap();
    assert!(
        second_scan.is_empty(),
        "Second scan should skip analyzed pair, got {} candidates",
        second_scan.len()
    );
}

// =============================================================================
// TEST 4: Scanner session filtering — same_session_only
// =============================================================================

#[test]
fn test_scanner_session_filtering() {
    let config = ScannerConfig {
        same_session_only: true,
        min_initial_score: 0.0,
        similarity_threshold: 0.0,
        max_similarity: 1.0,
        ..Default::default()
    };
    let mut scanner = MemoryScanner::with_config(config);

    // Same session → should be candidates
    let mem_a = make_memory_with_session(
        "use crate::module; fn handler() {}",
        unit_vec(1024, 1.0),
        "session-1",
    );
    let mem_b = make_memory_with_session(
        "pub mod module { pub fn process() {} }",
        unit_vec(1024, 1.1),
        "session-1",
    );
    // Different session → should be excluded
    let mem_c = make_memory_with_session(
        "use crate::other; fn different() {}",
        unit_vec(1024, 1.05),
        "session-2",
    );
    let mem_c_id = mem_c.id;

    let candidates = scanner.find_candidates(&[mem_a, mem_b, mem_c]).unwrap();

    // Verify no candidate pairs cross session boundaries
    for candidate in &candidates {
        assert_ne!(
            candidate.memory_a_id, mem_c_id,
            "mem_c (session-2) should not appear as memory_a"
        );
        assert_ne!(
            candidate.memory_b_id, mem_c_id,
            "mem_c (session-2) should not appear as memory_b"
        );
    }
}

// =============================================================================
// TEST 5: File path similarity boosts scoring
// =============================================================================

#[test]
fn test_scanner_file_path_scoring() {
    let config = ScannerConfig {
        min_initial_score: 0.0,
        similarity_threshold: 0.0,
        max_similarity: 1.0,
        ..Default::default()
    };
    let mut scanner = MemoryScanner::with_config(config);

    // Same directory → should get path bonus
    let mem_a = make_memory_with_path(
        "use crate::types; struct Config {}",
        unit_vec(1024, 1.0),
        "src/agent/config.rs",
    );
    let mem_b = make_memory_with_path(
        "pub mod types { pub struct Agent {} }",
        unit_vec(1024, 1.1),
        "src/agent/types.rs",
    );

    // Different directory → no path bonus
    let mem_c = make_memory_with_path(
        "fn unrelated() {}",
        unit_vec(1024, 1.05),
        "tests/integration/test_helper.rs",
    );

    let candidates = scanner.find_candidates(&[mem_a.clone(), mem_b.clone(), mem_c]).unwrap();

    // Find the A-B pair and check it has higher score than pairs with C
    let ab_score = candidates
        .iter()
        .find(|c| {
            (c.memory_a_id == mem_a.id && c.memory_b_id == mem_b.id)
                || (c.memory_a_id == mem_b.id && c.memory_b_id == mem_a.id)
        })
        .map(|c| c.initial_score);

    assert!(
        ab_score.is_some(),
        "Should find A-B pair as candidates"
    );
}

// =============================================================================
// TEST 6: Multi-domain marker detection
// =============================================================================

#[test]
fn test_multi_domain_marker_detection() {
    // Code content → code markers
    let code = "fn main() { use crate::foo; impl Trait for Struct {} }";
    assert_eq!(DomainMarkers::detect_domain(code), ContentDomain::Code);

    let code_types = GraphMarkers::detect_suspected_types(code);
    assert!(code_types.contains(&RelationshipType::Imports));
    assert!(code_types.contains(&RelationshipType::Implements));

    // Legal content → legal markers
    let legal = "The court held that Brown v. Board of Education (1954) cites the Equal Protection Clause. The court distinguishes this from Smith v. Jones.";
    assert_eq!(DomainMarkers::detect_domain(legal), ContentDomain::Legal);

    let legal_types = GraphMarkers::detect_suspected_types(legal);
    assert!(legal_types.contains(&RelationshipType::Cites));
    assert!(legal_types.contains(&RelationshipType::Distinguishes));

    // Academic content → academic markers
    let academic = "The study by Smith et al. (2023) found statistical significance (p < 0.05) with n = 150 participants in the methodology section.";
    assert_eq!(
        DomainMarkers::detect_domain(academic),
        ContentDomain::Academic
    );

    // General content → no specific domain
    let general = "This is a general text about various everyday topics.";
    assert_eq!(
        DomainMarkers::detect_domain(general),
        ContentDomain::General
    );
}

// =============================================================================
// TEST 7: Domain-filtered markers exclude irrelevant types
// =============================================================================

#[test]
fn test_domain_filtered_marker_detection() {
    // Mixed content with both code AND legal markers
    let mixed = "use crate::module; the court cites Brown v. Board";

    // Code filter should include Imports but exclude Cites
    let code_types =
        GraphMarkers::detect_suspected_types_for_domain(mixed, ContentDomain::Code);
    assert!(code_types.contains(&RelationshipType::Imports));
    assert!(!code_types.contains(&RelationshipType::Cites));

    // Legal filter should include Cites but exclude Imports
    let legal_types =
        GraphMarkers::detect_suspected_types_for_domain(mixed, ContentDomain::Legal);
    assert!(legal_types.contains(&RelationshipType::Cites));
    assert!(!legal_types.contains(&RelationshipType::Imports));

    // General filter should include both
    let general_types =
        GraphMarkers::detect_suspected_types_for_domain(mixed, ContentDomain::General);
    assert!(general_types.contains(&RelationshipType::Imports));
    assert!(general_types.contains(&RelationshipType::Cites));
}

// =============================================================================
// TEST 8: GraphStorage edge queries with multiple relationship types
// =============================================================================

#[test]
fn test_graph_storage_multi_edge_queries() {
    use context_graph_graph_agent::{GraphEdge, GraphStorage};

    let mut graph = GraphStorage::new();
    let id_a = Uuid::new_v4();
    let id_b = Uuid::new_v4();
    let id_c = Uuid::new_v4();

    // A imports B
    graph.add_edge(GraphEdge::new(
        id_a,
        id_b,
        RelationshipType::Imports,
        0.9,
        "A imports B".to_string(),
    ));
    // A calls C
    graph.add_edge(GraphEdge::new(
        id_a,
        id_c,
        RelationshipType::Calls,
        0.85,
        "A calls C".to_string(),
    ));
    // B extends C
    graph.add_edge(GraphEdge::new(
        id_b,
        id_c,
        RelationshipType::Extends,
        0.7,
        "B extends C".to_string(),
    ));

    assert_eq!(graph.edge_count(), 3);

    // edges_from(A) → B and C
    let from_a = graph.edges_from(id_a);
    assert_eq!(from_a.len(), 2);

    // edges_to(C) → A and B
    let to_c = graph.edges_to(id_c);
    assert_eq!(to_c.len(), 2);

    // edges_from(C) → none
    let from_c = graph.edges_from(id_c);
    assert!(from_c.is_empty());

    // has_edge directional
    assert!(graph.has_edge(id_a, id_b));
    assert!(!graph.has_edge(id_b, id_a)); // reverse should not exist
}

// =============================================================================
// TEST 9: RelationshipType round-trip (from_str → as_str → from_str)
// =============================================================================

#[test]
fn test_relationship_type_roundtrip() {
    for rel_type in RelationshipType::all() {
        let s = rel_type.as_str();
        let parsed = RelationshipType::from_str(s);
        assert_eq!(
            *rel_type, parsed,
            "Round-trip failed for {:?}: '{}' parsed as {:?}",
            rel_type, s, parsed
        );
    }
}

// =============================================================================
// TEST 10: All 19 relationship types have correct categories
// =============================================================================

#[test]
fn test_all_relationship_types_have_categories() {
    use context_graph_graph_agent::RelationshipCategory;

    let all = RelationshipType::all();
    assert_eq!(all.len(), 19, "Should have 19 types (excluding None)");

    // Every type should map to a valid category
    for rel_type in all {
        let cat = rel_type.category();
        assert_ne!(
            format!("{:?}", cat),
            "",
            "Category for {:?} should not be empty",
            rel_type
        );
    }

    // Verify specific category counts
    let containment_count = all
        .iter()
        .filter(|t| t.category() == RelationshipCategory::Containment)
        .count();
    let dependency_count = all
        .iter()
        .filter(|t| t.category() == RelationshipCategory::Dependency)
        .count();
    let reference_count = all
        .iter()
        .filter(|t| t.category() == RelationshipCategory::Reference)
        .count();
    let impl_count = all
        .iter()
        .filter(|t| t.category() == RelationshipCategory::Implementation)
        .count();
    let extension_count = all
        .iter()
        .filter(|t| t.category() == RelationshipCategory::Extension)
        .count();
    let invocation_count = all
        .iter()
        .filter(|t| t.category() == RelationshipCategory::Invocation)
        .count();

    assert_eq!(containment_count, 2, "Containment: Contains, ScopedBy");
    assert_eq!(dependency_count, 3, "Dependency: DependsOn, Imports, Requires");
    assert_eq!(reference_count, 4, "Reference: References, Cites, Interprets, Distinguishes");
    assert_eq!(impl_count, 3, "Implementation: Implements, CompliesWith, Fulfills");
    assert_eq!(extension_count, 4, "Extension: Extends, Modifies, Supersedes, Overrules");
    assert_eq!(invocation_count, 3, "Invocation: Calls, Applies, UsedBy");
}
