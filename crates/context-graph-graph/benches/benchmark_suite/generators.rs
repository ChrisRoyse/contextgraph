//! Real data generators for benchmarks (NO MOCK DATA).

use std::collections::HashMap;

/// Generate real Poincare point within the unit ball as fixed-size array.
/// Uses deterministic seeding to ensure valid hyperbolic coordinates.
pub fn generate_poincare_point_fixed(seed: u64) -> [f32; 64] {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut point = [0.0f32; 64];
    let mut hasher = DefaultHasher::new();

    // Generate point components using deterministic seeding
    for (i, p) in point.iter_mut().enumerate() {
        (seed, i).hash(&mut hasher);
        let hash = hasher.finish();
        // Map to [-0.7, 0.7] to stay within Poincare ball
        let val = ((hash as f32 / u64::MAX as f32) * 1.4 - 0.7) * 0.7;
        *p = val;
        hasher = DefaultHasher::new();
    }

    // Normalize to ensure ||x|| < 1 (Poincare ball constraint)
    let norm: f32 = point.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm >= 0.95 {
        let scale = 0.9 / norm;
        for val in &mut point {
            *val *= scale;
        }
    }

    point
}

/// Generate batch of real Poincare points as flattened array.
pub fn generate_poincare_batch_flat(count: usize) -> Vec<f32> {
    let mut result = Vec::with_capacity(count * 64);
    for i in 0..count {
        let point = generate_poincare_point_fixed(i as u64 * 12345);
        result.extend_from_slice(&point);
    }
    result
}

/// Generate real cone data (apex + aperture) for entailment testing.
/// Returns (apex array, aperture) tuple.
pub fn generate_cone_data_fixed(seed: u64) -> ([f32; 64], f32) {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let apex = generate_poincare_point_fixed(seed);

    // Generate aperture in valid range [0.1, PI/2]
    let mut hasher = DefaultHasher::new();
    seed.hash(&mut hasher);
    let hash = hasher.finish();
    let aperture = 0.1 + (hash as f32 / u64::MAX as f32) * (std::f32::consts::FRAC_PI_2 - 0.1);

    (apex, aperture)
}

/// Generate batch of cone data as flattened array.
/// Each cone is 65 floats: 64 for apex + 1 for aperture.
pub fn generate_cone_batch_flat(count: usize) -> Vec<f32> {
    let mut result = Vec::with_capacity(count * 65);
    for i in 0..count {
        let (apex, aperture) = generate_cone_data_fixed(i as u64 * 2000);
        result.extend_from_slice(&apex);
        result.push(aperture);
    }
    result
}

/// Generate real graph adjacency for BFS benchmarks.
/// Creates a connected graph with controlled edge density.
pub fn generate_graph_adjacency(
    node_count: usize,
    avg_edges_per_node: usize,
) -> HashMap<u64, Vec<u64>> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut adjacency: HashMap<u64, Vec<u64>> = HashMap::new();

    for node in 0..node_count as u64 {
        let mut edges = Vec::new();

        // Ensure connectivity: connect to next node (forms spanning path)
        if node < (node_count - 1) as u64 {
            edges.push(node + 1);
        }

        // Add random edges for density
        let mut hasher = DefaultHasher::new();
        for i in 0..avg_edges_per_node {
            (node, i).hash(&mut hasher);
            let target = hasher.finish() % node_count as u64;
            if target != node && !edges.contains(&target) {
                edges.push(target);
            }
            hasher = DefaultHasher::new();
        }

        adjacency.insert(node, edges);
    }

    adjacency
}
