//! Real Data Fixtures for Integration Tests.
//!
//! Generates consistent, deterministic test data using REAL algorithms.
//! NO MOCKS - per constitution REQ-KG-TEST.
//!
//! # Design Principles
//!
//! 1. **Deterministic**: Same seed produces identical data
//! 2. **Real Data**: Uses actual domain profiles, valid geometries
//! 3. **Scalable**: Can generate from 1 to 1M+ test items
//! 4. **Documented**: Each fixture documents its invariants

#![allow(dead_code)]

use context_graph_graph::{
    storage::{EntailmentCone, NodeId, PoincarePoint},
    Domain, NeurotransmitterWeights,
};

/// Maximum norm for Poincare ball (per constitution max_norm=0.99999)
pub const POINCARE_MAX_NORM: f32 = 0.99999;

/// Poincare ball dimension (per constitution 64D)
pub const POINCARE_DIM: usize = 64;

/// Default cone aperture (π/4 radians)
pub const DEFAULT_APERTURE: f32 = std::f32::consts::FRAC_PI_4;

/// Deterministic hash function for reproducible test data.
///
/// Simple LCG (Linear Congruential Generator) for deterministic "randomness".
#[inline]
pub fn deterministic_hash(seed: u32) -> u32 {
    seed.wrapping_mul(1103515245).wrapping_add(12345)
}

/// Generate a deterministic f32 value in [0.0, 1.0).
#[inline]
pub fn deterministic_float(seed: u32) -> f32 {
    ((deterministic_hash(seed) >> 16) & 0x7FFF) as f32 / 32768.0
}

/// Generate a deterministic point inside the Poincare ball.
///
/// # Arguments
///
/// * `seed` - Deterministic seed for reproducibility
/// * `max_norm` - Maximum norm (must be < 1.0 for Poincare ball)
///
/// # Invariants
///
/// - Result norm is always < max_norm
/// - Same seed always produces identical point
/// - Point is valid for Poincare ball operations
pub fn generate_poincare_point(seed: u32, max_norm: f32) -> PoincarePoint {
    let mut coords = [0.0f32; POINCARE_DIM];
    let mut hash = seed;

    // Generate raw coordinates
    for (i, c) in coords.iter_mut().enumerate() {
        hash = deterministic_hash(hash.wrapping_add(i as u32));
        let val = ((hash >> 16) & 0x7FFF) as f32 / 32767.0;
        *c = (val - 0.5) * 2.0;
    }

    // Normalize to max_norm
    let norm_sq: f32 = coords.iter().map(|x| x * x).sum();
    if norm_sq > 0.0 {
        let scale = max_norm / norm_sq.sqrt();
        for c in &mut coords {
            *c *= scale;
        }
    }

    PoincarePoint { coords }
}

/// Generate a deterministic entailment cone.
///
/// # Arguments
///
/// * `seed` - Deterministic seed
/// * `max_apex_norm` - Maximum norm for apex point
/// * `aperture_range` - (min, max) aperture in radians
///
/// # Invariants
///
/// - Apex is inside Poincare ball
/// - Aperture is in valid range [0, π]
/// - depth and aperture_factor are valid
pub fn generate_entailment_cone(
    seed: u32,
    max_apex_norm: f32,
    aperture_range: (f32, f32),
) -> EntailmentCone {
    let apex = generate_poincare_point(seed, max_apex_norm);

    let aperture = aperture_range.0
        + deterministic_float(seed.wrapping_add(1000)) * (aperture_range.1 - aperture_range.0);

    let depth = (deterministic_float(seed.wrapping_add(2000)) * 10.0) as u32;
    let aperture_factor = 0.5 + deterministic_float(seed.wrapping_add(3000)) * 1.5;

    EntailmentCone {
        apex,
        aperture,
        aperture_factor,
        depth,
    }
}

/// Generate a batch of Poincare points.
///
/// # Arguments
///
/// * `start_seed` - Starting seed
/// * `count` - Number of points to generate
/// * `max_norm` - Maximum norm for all points
pub fn generate_poincare_points(
    start_seed: u32,
    count: usize,
    max_norm: f32,
) -> Vec<PoincarePoint> {
    (0..count)
        .map(|i| generate_poincare_point(start_seed.wrapping_add(i as u32), max_norm))
        .collect()
}

/// Generate a batch of entailment cones.
pub fn generate_entailment_cones(
    start_seed: u32,
    count: usize,
    max_apex_norm: f32,
    aperture_range: (f32, f32),
) -> Vec<EntailmentCone> {
    (0..count)
        .map(|i| {
            generate_entailment_cone(
                start_seed.wrapping_add(i as u32 * 1000),
                max_apex_norm,
                aperture_range,
            )
        })
        .collect()
}

/// Generate a flattened vector of coordinates for FAISS/CUDA operations.
///
/// Returns a Vec<f32> with count * POINCARE_DIM elements.
pub fn generate_flat_coordinates(start_seed: u32, count: usize, max_norm: f32) -> Vec<f32> {
    (0..count)
        .flat_map(|i| {
            generate_poincare_point(start_seed.wrapping_add(i as u32), max_norm)
                .coords
                .to_vec()
        })
        .collect()
}

/// Test node with all associated data.
#[derive(Debug, Clone)]
pub struct TestNode {
    pub id: NodeId,
    pub point: PoincarePoint,
    pub cone: EntailmentCone,
    pub domain: Domain,
    pub embedding: Vec<f32>,
}

impl TestNode {
    /// Generate a test node with consistent, deterministic data.
    pub fn generate(seed: u32, embedding_dim: usize) -> Self {
        let id = seed as i64;
        let point = generate_poincare_point(seed, 0.9);
        let cone = generate_entailment_cone(seed, 0.8, (0.2, 0.8));

        // Cycle through domains deterministically
        let domain_idx = (seed % 6) as usize;
        let domain = Domain::all()[domain_idx];

        // Generate embedding
        let embedding: Vec<f32> = (0..embedding_dim)
            .map(|i| deterministic_float(seed.wrapping_add(10000 + i as u32)) - 0.5)
            .collect();

        Self {
            id,
            point,
            cone,
            domain,
            embedding,
        }
    }
}

/// Generate a batch of test nodes.
pub fn generate_test_nodes(start_seed: u32, count: usize, embedding_dim: usize) -> Vec<TestNode> {
    (0..count)
        .map(|i| TestNode::generate(start_seed.wrapping_add(i as u32), embedding_dim))
        .collect()
}

/// Test edge with NT weights.
#[derive(Debug, Clone)]
pub struct TestEdge {
    pub source_id: NodeId,
    pub target_id: NodeId,
    pub domain: Domain,
    pub nt_weights: NeurotransmitterWeights,
    pub base_weight: f32,
}

impl TestEdge {
    /// Generate a test edge with domain-specific NT weights.
    pub fn generate(seed: u32, source_id: NodeId, target_id: NodeId) -> Self {
        let domain_idx = (seed % 6) as usize;
        let domain = Domain::all()[domain_idx];
        let nt_weights = NeurotransmitterWeights::for_domain(domain);
        let base_weight = 0.5 + deterministic_float(seed) * 0.5;

        Self {
            source_id,
            target_id,
            domain,
            nt_weights,
            base_weight,
        }
    }

    /// Compute effective weight using Marblestone formula.
    ///
    /// Formula: w_eff = base_weight * (1.0 + net_activation + domain_bonus) * steering_factor
    /// where net_activation = excitatory - inhibitory
    pub fn effective_weight(&self, query_domain: Domain, domain_match_bonus: f32) -> f32 {
        let net = self.nt_weights.excitatory - self.nt_weights.inhibitory;
        let domain_bonus = if self.domain == query_domain {
            domain_match_bonus
        } else {
            0.0
        };
        // Simplified steering factor = 1.0 + (modulatory - 0.5) * 0.4
        let steering = 1.0 + (self.nt_weights.modulatory - 0.5) * 0.4;

        (self.base_weight * (1.0 + net + domain_bonus) * steering).clamp(0.0, 1.0)
    }
}

/// Generate test edges connecting nodes.
pub fn generate_test_edges(
    start_seed: u32,
    node_ids: &[NodeId],
    edges_per_node: usize,
) -> Vec<TestEdge> {
    let mut edges = Vec::with_capacity(node_ids.len() * edges_per_node);

    for (i, &source_id) in node_ids.iter().enumerate() {
        for j in 0..edges_per_node {
            // Connect to next few nodes (circular)
            let target_idx = (i + j + 1) % node_ids.len();
            let target_id = node_ids[target_idx];

            let seed = start_seed
                .wrapping_add(i as u32 * 1000)
                .wrapping_add(j as u32);

            edges.push(TestEdge::generate(seed, source_id, target_id));
        }
    }

    edges
}

/// Hierarchical test data for entailment testing.
#[derive(Debug)]
pub struct HierarchicalTestData {
    pub root: TestNode,
    pub children: Vec<TestNode>,
    pub grandchildren: Vec<TestNode>,
}

impl HierarchicalTestData {
    /// Generate a 3-level hierarchy for entailment tests.
    ///
    /// Structure:
    /// - root (apex near origin, wide aperture)
    ///   - children (apex inside root's cone, medium aperture)
    ///     - grandchildren (apex inside child's cone, narrow aperture)
    pub fn generate(seed: u32, children_count: usize, grandchildren_per_child: usize) -> Self {
        // Root at origin with wide aperture
        let mut root_point = PoincarePoint::origin();
        root_point.coords[0] = 0.1;

        let root = TestNode {
            id: seed as i64,
            point: root_point.clone(),
            cone: EntailmentCone {
                apex: root_point,
                aperture: std::f32::consts::PI * 0.4, // Wide
                aperture_factor: 1.0,
                depth: 0,
            },
            domain: Domain::General,
            embedding: vec![0.0; 1536],
        };

        // Generate children inside root's cone
        let mut children = Vec::with_capacity(children_count);
        for i in 0..children_count {
            let child_seed = seed.wrapping_add(1000 + i as u32);

            // Child apex is between root and boundary, in the cone direction
            let mut child_point = PoincarePoint::origin();
            let angle = (i as f32 / children_count as f32) * 0.3; // Spread within cone
            child_point.coords[0] = 0.3 + angle * 0.1;
            child_point.coords[1] = angle * 0.1;

            children.push(TestNode {
                id: child_seed as i64,
                point: child_point.clone(),
                cone: EntailmentCone {
                    apex: child_point,
                    aperture: std::f32::consts::PI * 0.2, // Medium
                    aperture_factor: 1.0,
                    depth: 1,
                },
                domain: Domain::all()[i % 6],
                embedding: vec![0.0; 1536],
            });
        }

        // Generate grandchildren inside each child's cone
        let mut grandchildren = Vec::with_capacity(children_count * grandchildren_per_child);
        for (i, child) in children.iter().enumerate() {
            for j in 0..grandchildren_per_child {
                let gc_seed = seed.wrapping_add(2000000 + (i * 1000 + j) as u32);

                // Grandchild is further out, within child's cone
                let mut gc_point = child.point.clone();
                gc_point.coords[0] += 0.2;
                gc_point.coords[2] = (j as f32 / grandchildren_per_child as f32) * 0.05;

                grandchildren.push(TestNode {
                    id: gc_seed as i64,
                    point: gc_point.clone(),
                    cone: EntailmentCone {
                        apex: gc_point,
                        aperture: std::f32::consts::PI * 0.1, // Narrow
                        aperture_factor: 1.0,
                        depth: 2,
                    },
                    domain: Domain::all()[j % 6],
                    embedding: vec![0.0; 1536],
                });
            }
        }

        Self {
            root,
            children,
            grandchildren,
        }
    }
}

/// Contradiction test pair.
#[derive(Debug)]
pub struct ContradictionPair {
    pub node_a: TestNode,
    pub node_b: TestNode,
    pub expected_contradiction: bool,
    pub similarity_score: f32,
}

/// Generate pairs for contradiction testing.
///
/// Creates pairs with known contradiction status based on embedding similarity.
pub fn generate_contradiction_pairs(seed: u32, count: usize) -> Vec<ContradictionPair> {
    let mut pairs = Vec::with_capacity(count);

    for i in 0..(count / 2) {
        let pair_seed = seed.wrapping_add(i as u32 * 2000);

        // Non-contradicting pair: similar embeddings
        let node_a = TestNode::generate(pair_seed, 1536);
        let mut node_b = TestNode::generate(pair_seed.wrapping_add(1), 1536);

        // Make embeddings similar (not contradicting)
        for j in 0..1536 {
            node_b.embedding[j] =
                node_a.embedding[j] + (deterministic_float(pair_seed + j as u32) - 0.5) * 0.1;
        }

        pairs.push(ContradictionPair {
            node_a,
            node_b,
            expected_contradiction: false,
            similarity_score: 0.95,
        });
    }

    for i in (count / 2)..count {
        let pair_seed = seed.wrapping_add(i as u32 * 2000);

        // Contradicting pair: opposite embeddings
        let node_a = TestNode::generate(pair_seed, 1536);
        let mut node_b = TestNode::generate(pair_seed.wrapping_add(1), 1536);

        // Make embeddings opposite (contradicting)
        for j in 0..1536 {
            node_b.embedding[j] = -node_a.embedding[j];
        }

        pairs.push(ContradictionPair {
            node_a,
            node_b,
            expected_contradiction: true,
            similarity_score: -0.9,
        });
    }

    pairs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_poincare_point_norm() {
        for seed in 0..100 {
            let point = generate_poincare_point(seed, 0.9);
            let norm_sq: f32 = point.coords.iter().map(|x| x * x).sum();
            let norm = norm_sq.sqrt();

            assert!(
                norm <= 0.9 + 1e-5,
                "Norm {} exceeds max 0.9 for seed {}",
                norm,
                seed
            );
            assert!(norm >= 0.0, "Norm {} is negative for seed {}", norm, seed);
        }
    }

    #[test]
    fn test_determinism() {
        let point1 = generate_poincare_point(42, 0.9);
        let point2 = generate_poincare_point(42, 0.9);

        assert_eq!(
            point1.coords, point2.coords,
            "Same seed should produce identical points"
        );
    }

    #[test]
    fn test_test_node_generation() {
        let node = TestNode::generate(123, 1536);

        assert_eq!(node.id, 123);
        assert_eq!(node.embedding.len(), 1536);
        assert!(node.cone.aperture >= 0.2 && node.cone.aperture <= 0.8);
    }

    #[test]
    fn test_hierarchical_data() {
        let hierarchy = HierarchicalTestData::generate(42, 5, 3);

        assert_eq!(hierarchy.children.len(), 5);
        assert_eq!(hierarchy.grandchildren.len(), 15);
    }
}
