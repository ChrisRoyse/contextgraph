---
id: "M04-T25"
title: "Create Module Integration Tests"
description: |
  Implement comprehensive integration tests for Module 4:
  - End-to-end FAISS index lifecycle (train, add, search)
  - Hyperbolic distance computation (CPU vs GPU comparison)
  - Entailment cone containment queries
  - Graph traversal with Marblestone edge modulation
  - Domain-aware search ranking
  - Contradiction detection pipeline
  Performance benchmarks against NFR targets.
layer: "surface"
status: "pending"
priority: "critical"
estimated_hours: 6
sequence: 33
depends_on:
  - "M04-T18"
  - "M04-T19"
  - "M04-T20"
  - "M04-T21"
  - "M04-T23"
  - "M04-T24"
  - "M04-T26"
  - "M04-T27"
spec_refs:
  - "TECH-GRAPH-004 Section 11"
  - "All NFR-KG requirements"
files_to_create:
  - path: "crates/context-graph-graph/tests/integration_tests.rs"
    description: "Comprehensive integration tests"
  - path: "crates/context-graph-graph/tests/common/mod.rs"
    description: "Shared test utilities"
  - path: "crates/context-graph-graph/tests/common/fixtures.rs"
    description: "Test data fixtures"
files_to_modify: []
test_file: "crates/context-graph-graph/tests/integration_tests.rs"
---

## Context

Integration tests validate that all Module 4 components work correctly together in realistic scenarios. Unlike unit tests which verify individual functions, integration tests exercise complete workflows: indexing millions of vectors, performing semantic search, traversing the graph with Marblestone modulation, and detecting contradictions. These tests ensure the system meets performance NFRs and catches integration issues between components.

**CRITICAL**: Per REQ-KG-TEST, all tests MUST use real FAISS GPU index. Mock implementations are forbidden for vector similarity search.

## Scope

### In Scope
- End-to-end FAISS index lifecycle tests
- CPU vs GPU comparison tests
- Entailment hierarchy tests
- Graph traversal tests
- Domain-aware search tests
- Contradiction detection tests
- Performance validation against NFR targets
- Test fixtures and utilities

### Out of Scope
- Stress testing / load testing
- Multi-GPU tests
- Distributed system tests
- Fuzzing

## Definition of Done

### Test File Structure

```rust
// In crates/context-graph-graph/tests/integration_tests.rs

//! Integration tests for Knowledge Graph Module 4
//!
//! These tests validate complete workflows and ensure all components
//! work together correctly. Tests marked with #[requires_gpu] require
//! CUDA-capable hardware and will be skipped in CI without GPU.
//!
//! IMPORTANT: Per REQ-KG-TEST, NO MOCK FAISS. All vector similarity
//! tests use real FAISS GPU index.

mod common;

use std::time::{Duration, Instant};
use tempfile::tempdir;

use context_graph_graph::{
    config::{IndexConfig, HyperbolicConfig, ConeConfig},
    error::GraphResult,
    index::gpu_index::FaissGpuIndex,
    storage::rocksdb::GraphStorage,
    storage::edges::{GraphEdge, EdgeType, Domain, NeurotransmitterWeights},
    hyperbolic::{PoincarePoint, PoincareBall},
    entailment::cones::EntailmentCone,
    search::{semantic_search, SearchFilters},
    marblestone::{domain_aware_search, get_modulated_weight},
    entailment::query::{entailment_query, EntailmentDirection, EntailmentQueryParams},
    contradiction::detector::{contradiction_detect, ContradictionParams},
    traversal::bfs::{bfs_traverse, BfsParams},
    Vector1536,
};

use common::{fixtures::*, helpers::*};

/// Custom attribute for tests requiring GPU
/// These tests will be skipped in CI environments without CUDA
#[cfg(not(feature = "skip_gpu_tests"))]
macro_rules! requires_gpu {
    () => {};
}

// ============================================================
// FAISS Index Lifecycle Tests
// ============================================================

mod faiss_lifecycle {
    use super::*;

    /// Test complete FAISS index workflow: create, train, add, search
    #[test]
    #[requires_gpu]
    fn test_index_full_lifecycle() -> GraphResult<()> {
        let config = IndexConfig::default();
        let mut index = FaissGpuIndex::new(&config)?;

        // Verify initial state
        assert_eq!(index.ntotal(), 0);
        assert!(!index.is_trained());

        // Generate training data (minimum 4M for IVF16384)
        let train_data = generate_training_vectors(4_194_304);

        // Train index
        let train_start = Instant::now();
        index.train(&train_data)?;
        let train_time = train_start.elapsed();
        println!("Training time: {:?}", train_time);

        assert!(index.is_trained());

        // Add vectors
        let vectors = generate_normalized_vectors(10_000);
        let ids: Vec<i64> = (0..10_000).collect();

        let add_start = Instant::now();
        index.add_with_ids(&vectors, &ids)?;
        let add_time = add_start.elapsed();
        println!("Add time for 10K vectors: {:?}", add_time);

        assert_eq!(index.ntotal(), 10_000);

        // Search
        let query = &vectors[0];
        let k = 10;

        let search_start = Instant::now();
        let results = index.search(&[query.clone()], k)?;
        let search_time = search_start.elapsed();
        println!("Search time: {:?}", search_time);

        // Verify self is top result
        let (top_id, top_dist) = results.query_results(0).next().unwrap();
        assert_eq!(top_id, 0);
        assert!(top_dist < 0.01);

        Ok(())
    }

    /// Test FAISS search performance meets NFR target
    #[test]
    #[requires_gpu]
    fn test_search_performance_k100() -> GraphResult<()> {
        let (index, _) = create_trained_index_with_vectors(1_000_000)?;

        let query = generate_normalized_vectors(1)[0].clone();
        let k = 100;

        // Warm-up
        let _ = index.search(&[query.clone()], k)?;

        // Measure
        let iterations = 100;
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = index.search(&[query.clone()], k)?;
        }
        let total_time = start.elapsed();
        let avg_time = total_time / iterations;

        println!("Average search time (k=100, 1M vectors): {:?}", avg_time);

        // NFR target: <10ms for k=100 on 10M vectors
        // For 1M vectors, should be well under
        assert!(avg_time < Duration::from_millis(10),
            "Search took {:?}, expected <10ms", avg_time);

        Ok(())
    }
}

// ============================================================
// Hyperbolic Geometry Tests
// ============================================================

mod hyperbolic_tests {
    use super::*;

    /// Test CPU vs GPU Poincare distance computation
    #[test]
    #[requires_gpu]
    fn test_poincare_distance_cpu_gpu_match() -> GraphResult<()> {
        let config = HyperbolicConfig::default();
        let ball = PoincareBall::new(&config);

        let n_points = 100;
        let points = generate_poincare_points(n_points);

        // CPU computation
        let mut cpu_distances = Vec::new();
        for i in 0..n_points {
            for j in 0..n_points {
                let d = ball.distance(&points[i], &points[j]);
                cpu_distances.push(d);
            }
        }

        // GPU computation
        let gpu_distances = poincare_distance_batch_gpu(&points, &points)?;

        // Compare
        for (i, (cpu, gpu)) in cpu_distances.iter().zip(gpu_distances.iter()).enumerate() {
            let diff = (cpu - gpu).abs();
            assert!(diff < 1e-5,
                "Distance mismatch at {}: CPU={}, GPU={}, diff={}",
                i, cpu, gpu, diff);
        }

        Ok(())
    }

    /// Test Poincare distance GPU performance
    #[test]
    #[requires_gpu]
    fn test_poincare_distance_performance() -> GraphResult<()> {
        let n_queries = 1000;
        let n_database = 1000;

        let queries = generate_poincare_points(n_queries);
        let database = generate_poincare_points(n_database);

        // Warm-up
        let _ = poincare_distance_batch_gpu(&queries[..10], &database[..10])?;

        // Measure
        let start = Instant::now();
        let _ = poincare_distance_batch_gpu(&queries, &database)?;
        let elapsed = start.elapsed();

        println!("1K x 1K Poincare distance: {:?}", elapsed);

        // NFR target: <1ms
        assert!(elapsed < Duration::from_millis(1),
            "Poincare distance took {:?}, expected <1ms", elapsed);

        Ok(())
    }
}

// ============================================================
// Entailment Cone Tests
// ============================================================

mod entailment_tests {
    use super::*;

    /// Test entailment hierarchy queries
    #[test]
    fn test_entailment_hierarchy() -> GraphResult<()> {
        let dir = tempdir()?;
        let storage = create_test_hierarchy(&dir)?;

        // Query ancestors of "Dog"
        let dog_id = 3;  // From fixture
        let params = EntailmentQueryParams::default().max_depth(3);

        let ancestors = entailment_query(
            &storage,
            dog_id,
            EntailmentDirection::Ancestors,
            params.clone(),
        )?;

        // Should find Mammal and Animal
        let ancestor_ids: Vec<_> = ancestors.iter().map(|r| r.node_id).collect();
        println!("Dog's ancestors: {:?}", ancestor_ids);

        // Query descendants of "Animal"
        let animal_id = 1;
        let descendants = entailment_query(
            &storage,
            animal_id,
            EntailmentDirection::Descendants,
            params,
        )?;

        let descendant_ids: Vec<_> = descendants.iter().map(|r| r.node_id).collect();
        println!("Animal's descendants: {:?}", descendant_ids);

        // Membership scores should be in [0, 1]
        for result in ancestors.iter().chain(descendants.iter()) {
            assert!(result.membership_score >= 0.0);
            assert!(result.membership_score <= 1.0);
        }

        Ok(())
    }

    /// Test cone membership GPU performance
    #[test]
    #[requires_gpu]
    fn test_cone_membership_performance() -> GraphResult<()> {
        let n_cones = 1000;
        let n_points = 1000;

        let cones = generate_entailment_cones(n_cones);
        let points = generate_poincare_points(n_points);

        // Warm-up
        let _ = cone_check_batch_gpu(&cones[..10], &points[..10])?;

        // Measure
        let start = Instant::now();
        let scores = cone_check_batch_gpu(&cones, &points)?;
        let elapsed = start.elapsed();

        println!("1K x 1K cone membership: {:?}", elapsed);

        // NFR target: <2ms
        assert!(elapsed < Duration::from_millis(2),
            "Cone check took {:?}, expected <2ms", elapsed);

        // Verify scores in valid range
        for score in scores {
            assert!(score >= 0.0 && score <= 1.0);
        }

        Ok(())
    }
}

// ============================================================
// Graph Traversal Tests
// ============================================================

mod traversal_tests {
    use super::*;

    /// Test BFS traversal with Marblestone modulation
    #[test]
    fn test_bfs_with_domain_modulation() -> GraphResult<()> {
        let dir = tempdir()?;
        let storage = create_test_graph_with_domains(&dir)?;

        let start_node = 1;

        // BFS with Code domain
        let params = BfsParams::default()
            .max_depth(3)
            .domain(Domain::Code);

        let result = bfs_traverse(&storage, start_node, params)?;

        println!("BFS found {} nodes, {} edges", result.nodes.len(), result.edges.len());

        // Verify depth distribution
        for (depth, count) in result.depth_counts.iter() {
            println!("Depth {}: {} nodes", depth, count);
        }

        // All nodes should be reachable within max_depth
        assert!(!result.nodes.is_empty());

        Ok(())
    }

    /// Test BFS performance on large graph
    #[test]
    fn test_bfs_performance() -> GraphResult<()> {
        let dir = tempdir()?;
        let storage = create_large_test_graph(&dir, 100_000, 6)?;  // 100K nodes, avg 6 edges

        let start_node = 0;
        let params = BfsParams::default().max_depth(6).max_nodes(10_000);

        // Warm-up
        let _ = bfs_traverse(&storage, start_node, params.clone())?;

        // Measure
        let start = Instant::now();
        let result = bfs_traverse(&storage, start_node, params)?;
        let elapsed = start.elapsed();

        println!("BFS depth=6 on 100K graph: {:?}, found {} nodes", elapsed, result.nodes.len());

        // NFR target: <100ms for depth=6 on 10M nodes
        // For 100K nodes, should be much faster
        assert!(elapsed < Duration::from_millis(100),
            "BFS took {:?}, expected <100ms", elapsed);

        Ok(())
    }
}

// ============================================================
// Domain-Aware Search Tests
// ============================================================

mod domain_search_tests {
    use super::*;

    /// Test domain-aware search re-ranking
    #[test]
    #[requires_gpu]
    fn test_domain_aware_ranking() -> GraphResult<()> {
        let dir = tempdir()?;
        let (index, storage) = create_index_with_domain_nodes(&dir)?;

        // Query with Code domain
        let query = generate_normalized_vectors(1)[0].clone();

        let results = domain_aware_search(
            &index,
            &storage,
            &query,
            Domain::Code,
            20,
            None,
        )?;

        // Verify modulation was applied
        for result in &results {
            // Modulated score should differ from base (unless General domain)
            if result.node_domain == Domain::Code {
                // Code domain should get boost
                assert!(result.modulated_score >= result.base_similarity,
                    "Code domain should be boosted");
            }
        }

        // Results should be sorted by modulated score
        for i in 1..results.len() {
            assert!(results[i-1].modulated_score >= results[i].modulated_score,
                "Results not sorted by modulated score");
        }

        Ok(())
    }

    /// Test domain-aware search performance
    #[test]
    #[requires_gpu]
    fn test_domain_search_performance() -> GraphResult<()> {
        let dir = tempdir()?;
        let (index, storage) = create_index_with_domain_nodes(&dir)?;

        let query = generate_normalized_vectors(1)[0].clone();

        // Warm-up
        let _ = domain_aware_search(&index, &storage, &query, Domain::Code, 10, None)?;

        // Measure
        let iterations = 100;
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = domain_aware_search(&index, &storage, &query, Domain::Code, 10, None)?;
        }
        let avg_time = start.elapsed() / iterations;

        println!("Average domain-aware search time: {:?}", avg_time);

        // NFR target: <10ms
        assert!(avg_time < Duration::from_millis(10),
            "Domain search took {:?}, expected <10ms", avg_time);

        Ok(())
    }
}

// ============================================================
// Contradiction Detection Tests
// ============================================================

mod contradiction_tests {
    use super::*;

    /// Test contradiction detection with explicit edges
    #[test]
    #[requires_gpu]
    fn test_contradiction_detection() -> GraphResult<()> {
        let dir = tempdir()?;
        let (index, storage) = create_index_with_contradictions(&dir)?;

        let node_with_contradiction = 1;
        let embedding = get_node_embedding(&storage, node_with_contradiction)?;

        let results = contradiction_detect(
            &index,
            &storage,
            node_with_contradiction,
            &embedding,
            ContradictionParams::default().threshold(0.5),
        )?;

        println!("Found {} contradictions", results.len());

        // Should find at least the explicit contradiction
        assert!(!results.is_empty(), "Should detect contradiction");

        for result in &results {
            assert!(result.confidence >= 0.5);
            if result.has_explicit_edge {
                println!("Explicit contradiction with node {}", result.contradicting_node_id);
            }
        }

        Ok(())
    }

    /// Test semantic similarity-based contradiction detection
    #[test]
    #[requires_gpu]
    fn test_semantic_contradiction() -> GraphResult<()> {
        let dir = tempdir()?;
        let (index, storage) = create_index_with_similar_content(&dir)?;

        let node_id = 1;
        let embedding = get_node_embedding(&storage, node_id)?;

        // High sensitivity to catch semantic contradictions
        let results = contradiction_detect(
            &index,
            &storage,
            node_id,
            &embedding,
            ContradictionParams::default()
                .high_sensitivity()
                .threshold(0.3),
        )?;

        // Should find semantically similar potential contradictions
        for result in &results {
            println!("Potential contradiction: node={}, sim={:.3}, conf={:.3}",
                result.contradicting_node_id,
                result.semantic_similarity,
                result.confidence);
        }

        Ok(())
    }
}

// ============================================================
// End-to-End Integration Test
// ============================================================

mod e2e_tests {
    use super::*;

    /// Complete knowledge graph workflow test
    #[test]
    #[requires_gpu]
    fn test_complete_workflow() -> GraphResult<()> {
        let dir = tempdir()?;

        // 1. Create infrastructure
        let config = IndexConfig::default();
        let mut index = FaissGpuIndex::new(&config)?;
        let storage = GraphStorage::open_default(dir.path())?;

        // 2. Train FAISS index
        let train_data = generate_training_vectors(4_194_304);
        index.train(&train_data)?;
        assert!(index.is_trained());

        // 3. Add knowledge nodes with embeddings
        let n_nodes = 10_000;
        let vectors = generate_normalized_vectors(n_nodes);
        let ids: Vec<i64> = (0..n_nodes as i64).collect();
        index.add_with_ids(&vectors, &ids)?;

        // 4. Add hyperbolic coordinates and cones
        let hyperbolic_config = HyperbolicConfig::default();
        for id in 0..n_nodes {
            let point = PoincarePoint::random(&hyperbolic_config);
            storage.put_hyperbolic(id as i64, &point)?;

            let cone = EntailmentCone::new(point, 0.8, 1.0, (id % 5) as u32);
            storage.put_cone(id as i64, &cone)?;
        }

        // 5. Add graph edges with domains
        for id in 0..n_nodes {
            let mut edges = Vec::new();
            for j in 1..=3 {
                let target = (id + j * 100) % n_nodes;
                let domain = match id % 4 {
                    0 => Domain::Code,
                    1 => Domain::Legal,
                    2 => Domain::Medical,
                    _ => Domain::General,
                };
                edges.push(GraphEdge::semantic(
                    id as i64 * 100 + j as i64,
                    id as i64,
                    target as i64,
                    0.7,
                ));
            }
            storage.put_adjacency(id as i64, &edges)?;
        }

        // 6. Perform semantic search
        let query = &vectors[0];
        let search_results = semantic_search(&index, &storage, query, 10, None)?;
        assert!(!search_results.is_empty());
        println!("Semantic search found {} results", search_results.len());

        // 7. Perform domain-aware search
        let domain_results = domain_aware_search(
            &index, &storage, query, Domain::Code, 10, None
        )?;
        println!("Domain search found {} results", domain_results.len());

        // 8. Perform entailment query
        let entailment_results = entailment_query(
            &storage,
            0,
            EntailmentDirection::Descendants,
            EntailmentQueryParams::default(),
        )?;
        println!("Entailment found {} descendants", entailment_results.len());

        // 9. Perform BFS traversal
        let bfs_results = bfs_traverse(
            &storage,
            0,
            BfsParams::default().max_depth(3),
        )?;
        println!("BFS found {} nodes", bfs_results.nodes.len());

        // 10. All operations completed successfully
        println!("Complete workflow test PASSED");

        Ok(())
    }
}
```

```rust
// In crates/context-graph-graph/tests/common/mod.rs

pub mod fixtures;
pub mod helpers;
```

```rust
// In crates/context-graph-graph/tests/common/fixtures.rs

//! Test data fixtures for integration tests

use context_graph_graph::Vector1536;
use context_graph_graph::hyperbolic::PoincarePoint;
use rand::Rng;

/// Generate normalized random vectors for FAISS
pub fn generate_normalized_vectors(n: usize) -> Vec<Vector1536> {
    (0..n).map(|_| random_normalized_vector()).collect()
}

/// Generate training vectors (larger dataset)
pub fn generate_training_vectors(n: usize) -> Vec<Vector1536> {
    generate_normalized_vectors(n)
}

/// Generate random Poincare points inside the ball
pub fn generate_poincare_points(n: usize) -> Vec<PoincarePoint> {
    (0..n).map(|_| random_poincare_point()).collect()
}

/// Generate entailment cones
pub fn generate_entailment_cones(n: usize) -> Vec<ConeData> {
    (0..n).map(|_| random_cone()).collect()
}

fn random_normalized_vector() -> Vector1536 {
    let mut rng = rand::thread_rng();
    let mut v = [0.0f32; 1536];
    let mut norm = 0.0f32;

    for x in v.iter_mut() {
        *x = rng.gen_range(-1.0..1.0);
        norm += *x * *x;
    }

    norm = norm.sqrt();
    for x in v.iter_mut() {
        *x /= norm;
    }

    Vector1536::from(v)
}

fn random_poincare_point() -> PoincarePoint {
    let mut rng = rand::thread_rng();
    let mut p = [0.0f32; 64];

    loop {
        let mut norm_sq = 0.0f32;
        for x in &mut p {
            *x = rng.gen_range(-1.0..1.0);
            norm_sq += *x * *x;
        }
        if norm_sq < 0.81 {  // norm < 0.9
            break;
        }
        let scale = 0.85 / norm_sq.sqrt();
        for x in &mut p { *x *= scale; }
        break;
    }

    PoincarePoint::from_coords(&p)
}

fn random_cone() -> ConeData {
    let mut rng = rand::thread_rng();
    let point = random_poincare_point();

    ConeData {
        apex: point.coords,
        aperture: rng.gen_range(0.3..1.2),
    }
}
```

### Constraints
- NO MOCK FAISS (per REQ-KG-TEST)
- GPU tests marked with #[requires_gpu]
- All NFR targets validated
- Tests must be deterministic (use seeded RNG where needed)
- Clean up temp directories after tests

### Acceptance Criteria
- [ ] FAISS search returns correct top-k in <10ms
- [ ] Hyperbolic distance CPU/GPU match within tolerance
- [ ] Entailment query finds correct hierarchy
- [ ] BFS traversal respects depth limits
- [ ] Domain-aware search re-ranks correctly
- [ ] All tests use real FAISS index (no mocks per spec)
- [ ] Tests marked #[requires_gpu] for CI skip on non-GPU
- [ ] Compiles with `cargo build`
- [ ] Tests pass with `cargo test`
- [ ] No clippy warnings

## Verification

### Test Commands
```bash
# Run all integration tests
cargo test -p context-graph-graph --test integration_tests

# Run with GPU tests (requires CUDA)
cargo test -p context-graph-graph --test integration_tests --features gpu

# Skip GPU tests (for CI without GPU)
cargo test -p context-graph-graph --test integration_tests --features skip_gpu_tests

# Run specific test
cargo test -p context-graph-graph --test integration_tests test_complete_workflow
```

### Manual Verification
- [ ] All tests pass on GPU machine
- [ ] Tests skip gracefully without GPU
- [ ] Performance numbers printed match NFR targets
- [ ] No memory leaks (run with valgrind on subset)
