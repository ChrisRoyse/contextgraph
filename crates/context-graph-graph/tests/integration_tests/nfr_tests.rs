//! NFR Timing Tests.
//!
//! Aggregate timing tests for NFR targets.

use context_graph_graph::storage::NodeId;

use crate::common::fixtures::{generate_entailment_cone, generate_poincare_point};
use crate::common::helpers::{create_test_storage, measure_latency, TimingBatch};

/// Aggregate timing test for all NFR targets.
#[test]
fn test_nfr_timing_summary() {
    println!("\n=== TEST: NFR Timing Summary ===");

    let mut batch = TimingBatch::new();

    // Storage operations
    let (storage, _temp_dir) = create_test_storage().expect("Failed to create storage");

    let (_, storage_write) = measure_latency("storage_write_100", 50_000, || {
        for i in 0..100 {
            let point = generate_poincare_point(i, 0.9);
            storage
                .put_hyperbolic(i as NodeId, &point)
                .expect("Put failed");
        }
    });
    batch.add(storage_write);

    let (_, storage_read) = measure_latency("storage_read_100", 25_000, || {
        for i in 0..100 {
            let _ = storage.get_hyperbolic(i as NodeId).expect("Get failed");
        }
    });
    batch.add(storage_read);

    // Poincare distance (CPU)
    use context_graph_cuda::poincare::poincare_distance_cpu;
    let points: Vec<_> = (0..100).map(|i| generate_poincare_point(i, 0.9)).collect();

    let (_, poincare_timing) = measure_latency("poincare_distance_100x100_cpu", 100_000, || {
        for x in &points {
            for y in &points {
                let _ = poincare_distance_cpu(&x.coords, &y.coords, -1.0);
            }
        }
    });
    batch.add(poincare_timing);

    // Cone membership (CPU)
    use context_graph_cuda::cone::cone_membership_score_cpu;
    let cones: Vec<_> = (0..100)
        .map(|i| generate_entailment_cone(i, 0.8, (0.2, 0.6)))
        .collect();

    let (_, cone_timing) = measure_latency("cone_membership_100x100_cpu", 100_000, || {
        for cone in &cones {
            for point in &points {
                let _ = cone_membership_score_cpu(
                    &cone.apex.coords,
                    cone.aperture,
                    &point.coords,
                    -1.0,
                );
            }
        }
    });
    batch.add(cone_timing);

    batch.summary();

    // Note: We don't assert all_passed here because CPU benchmarks may not meet GPU NFR targets
    // GPU tests are in the cuda crate with appropriate feature flags

    println!("=== COMPLETED: NFR Timing Summary ===\n");
}
