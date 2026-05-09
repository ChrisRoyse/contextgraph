// Inspired by ruvnet/RuVector crates/ruvector-mincut/src/canonical/* at HEAD ef5274c2 (read 2026-05-08).
// Clean-room reimplementation; no code copied, no upstream tracking. See
// memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md
// for the policy.

use super::*;

#[test]
fn rejects_too_small_graph() {
    let err = stoer_wagner(&[0.0], 1, StoerWagnerConfig::default()).expect_err("n=1 must error");
    assert_eq!(err.code(), "MINCUT_GRAPH_TOO_SMALL");
}

#[test]
fn rejects_non_square_matrix() {
    let err = stoer_wagner(&[0.0, 1.0, 1.0], 2, StoerWagnerConfig::default())
        .expect_err("3 entries for n=2 must error");
    assert_eq!(err.code(), "MINCUT_NON_SQUARE_MATRIX");
}

#[test]
fn rejects_negative_weight() {
    let weights = vec![0.0, -1.0, -1.0, 0.0];
    let err = stoer_wagner(&weights, 2, StoerWagnerConfig::default())
        .expect_err("negative weight must error");
    assert_eq!(err.code(), "MINCUT_WEIGHT_INVALID");
}

#[test]
fn rejects_nan_weight() {
    let weights = vec![0.0, f64::NAN, f64::NAN, 0.0];
    let err =
        stoer_wagner(&weights, 2, StoerWagnerConfig::default()).expect_err("NaN weight must error");
    assert_eq!(err.code(), "MINCUT_WEIGHT_INVALID");
}

#[test]
fn rejects_asymmetric_matrix() {
    let weights = vec![0.0, 1.0, 2.0, 0.0];
    let err =
        stoer_wagner(&weights, 2, StoerWagnerConfig::default()).expect_err("asymmetric must error");
    assert_eq!(err.code(), "MINCUT_ASYMMETRIC_EDGE");
}

#[test]
fn rejects_invalid_symmetry_tolerance() {
    let weights = vec![0.0, 1.0, 1.0, 0.0];
    let err = stoer_wagner(
        &weights,
        2,
        StoerWagnerConfig {
            symmetry_tolerance: f64::NAN,
        },
    )
    .expect_err("NaN tolerance must error");
    assert_eq!(err.code(), "MINCUT_INVALID_SYMMETRY_TOLERANCE");
}

#[test]
fn rejects_self_loop() {
    let weights = vec![1.0, 1.0, 1.0, 0.0];
    let err =
        stoer_wagner(&weights, 2, StoerWagnerConfig::default()).expect_err("self-loop must error");
    assert_eq!(err.code(), "MINCUT_SELF_LOOP_NONZERO");
}

#[test]
fn two_vertex_graph_cuts_at_single_edge() {
    let weights = vec![0.0, 5.0, 5.0, 0.0];
    let cut = stoer_wagner(&weights, 2, StoerWagnerConfig::default()).unwrap();
    assert_eq!(cut.cut_weight, 5.0);
    assert_eq!(cut.small_side.len() + cut.large_side.len(), 2);
}

#[test]
fn known_example_from_stoer_wagner_paper() {
    let edges = [
        (0, 1, 2.0),
        (0, 4, 3.0),
        (1, 2, 3.0),
        (1, 4, 2.0),
        (1, 5, 2.0),
        (2, 3, 4.0),
        (2, 6, 2.0),
        (3, 6, 2.0),
        (3, 7, 2.0),
        (4, 5, 3.0),
        (5, 6, 1.0),
        (6, 7, 3.0),
    ];
    let weights = weights_from_edges(8, edges.iter().copied()).unwrap();
    let cut = stoer_wagner(&weights, 8, StoerWagnerConfig::default()).unwrap();
    assert_eq!(cut.cut_weight, 4.0);
    assert_eq!(cut.small_side.len() + cut.large_side.len(), 8);
}

#[test]
fn dumbbell_graph_cuts_at_bridge() {
    let edges = [
        (0, 1, 10.0),
        (0, 2, 10.0),
        (1, 2, 10.0),
        (2, 3, 1.0),
        (3, 4, 10.0),
        (3, 5, 10.0),
        (4, 5, 10.0),
    ];
    let weights = weights_from_edges(6, edges.iter().copied()).unwrap();
    let cut = stoer_wagner(&weights, 6, StoerWagnerConfig::default()).unwrap();
    assert_eq!(cut.cut_weight, 1.0);
    let mut combined: Vec<usize> = cut
        .small_side
        .iter()
        .chain(cut.large_side.iter())
        .copied()
        .collect();
    combined.sort();
    assert_eq!(combined, vec![0, 1, 2, 3, 4, 5]);
}

#[test]
fn complete_graph_mincut_isolates_one_vertex() {
    let mut weights = vec![1.0; 16];
    for i in 0..4 {
        weights[i * 4 + i] = 0.0;
    }
    let cut = stoer_wagner(&weights, 4, StoerWagnerConfig::default()).unwrap();
    assert_eq!(cut.cut_weight, 3.0);
    let small = cut.small_side.len();
    let large = cut.large_side.len();
    assert!(
        (small == 1 && large == 3) || (small == 3 && large == 1),
        "K4 mincut isolates a single vertex: got small={small}, large={large}"
    );
}

#[test]
fn rejects_disconnected_graph() {
    let edges = [(0, 1, 1.0), (2, 3, 1.0)];
    let weights = weights_from_edges(4, edges.iter().copied()).unwrap();
    let err = stoer_wagner(&weights, 4, StoerWagnerConfig::default())
        .expect_err("disconnected graph must error");
    assert_eq!(err.code(), "MINCUT_GRAPH_DISCONNECTED");
}

#[test]
fn deterministic_across_runs() {
    let edges = [
        (0, 1, 1.0),
        (1, 2, 1.0),
        (2, 0, 1.0),
        (2, 3, 0.5),
        (3, 4, 1.0),
        (3, 5, 1.0),
        (4, 5, 1.0),
    ];
    let weights = weights_from_edges(6, edges.iter().copied()).unwrap();
    let a = stoer_wagner(&weights, 6, StoerWagnerConfig::default()).unwrap();
    let b = stoer_wagner(&weights, 6, StoerWagnerConfig::default()).unwrap();
    assert_eq!(a, b);
}

#[test]
fn weights_from_edges_accumulates_parallel() {
    let w = weights_from_edges(3, [(0, 1, 1.0), (0, 1, 2.5), (1, 2, 0.5)].iter().copied()).unwrap();
    assert_eq!(w[1], 3.5);
    assert_eq!(w[3], 3.5);
    assert_eq!(w[5], 0.5);
}

#[test]
fn weights_from_edges_rejects_out_of_bounds_endpoint() {
    let err = weights_from_edges(3, [(0, 3, 1.0)].iter().copied())
        .expect_err("out-of-bounds edge must error");
    assert_eq!(err.code(), "MINCUT_EDGE_ENDPOINT_OUT_OF_BOUNDS");
}

#[test]
fn weights_from_edges_rejects_self_loop() {
    let err = weights_from_edges(3, [(1, 1, 1.0)].iter().copied())
        .expect_err("self-loop edge must error");
    assert_eq!(err.code(), "MINCUT_SELF_LOOP_EDGE");
}

#[test]
fn weights_from_edges_rejects_invalid_weight() {
    let err = weights_from_edges(3, [(0, 1, f64::INFINITY)].iter().copied())
        .expect_err("non-finite edge must error");
    assert_eq!(err.code(), "MINCUT_WEIGHT_INVALID");
}

#[test]
fn small_balanced_cluster_split() {
    let edges = [
        (0, 1, 5.0),
        (0, 2, 5.0),
        (1, 2, 5.0),
        (3, 4, 5.0),
        (3, 5, 5.0),
        (4, 5, 5.0),
        (0, 3, 1.0),
        (1, 4, 0.5),
        (2, 5, 0.5),
    ];
    let weights = weights_from_edges(6, edges.iter().copied()).unwrap();
    let cut = stoer_wagner(&weights, 6, StoerWagnerConfig::default()).unwrap();
    assert!((cut.cut_weight - 2.0).abs() < 1e-9);
}
