use context_graph_cuda::{
    compute_core_distances_gpu, compute_pairwise_distances_gpu, cuda_available, cuda_device_count,
};

fn assert_close(actual: f32, expected: f32) {
    let diff = (actual - expected).abs();
    assert!(
        diff <= 1e-5,
        "actual {actual} differed from expected {expected} by {diff}"
    );
}

#[test]
fn native_sm120a_cubin_knn_outputs_expected_distances() {
    println!(
        "BEFORE source_of_truth=device_copyback cuda_available={} cuda_device_count={:?}",
        cuda_available(),
        cuda_device_count()
    );
    assert!(
        cuda_available(),
        "CUDA is required; no CPU fallback is allowed"
    );

    let vectors = vec![
        0.0f32, 0.0, // point 0
        3.0, 4.0, // point 1
        6.0, 8.0, // point 2
    ];

    let pairwise = compute_pairwise_distances_gpu(&vectors, 3, 2)
        .expect("native sm_120a cubin pairwise kernel must run");
    let core = compute_core_distances_gpu(&vectors, 3, 2, 1)
        .expect("native sm_120a cubin core-distance kernel must run");

    println!("AFTER source_of_truth=device_copyback pairwise={pairwise:?} core={core:?}");

    let expected_pairwise = [5.0f32, 10.0, 5.0];
    let expected_core = [5.0f32, 5.0, 5.0];

    assert_eq!(pairwise.len(), expected_pairwise.len());
    assert_eq!(core.len(), expected_core.len());
    for (actual, expected) in pairwise.iter().zip(expected_pairwise) {
        assert_close(*actual, expected);
    }
    for (actual, expected) in core.iter().zip(expected_core) {
        assert_close(*actual, expected);
    }
}

#[test]
fn native_sm120a_cubin_empty_inputs_return_empty_outputs() {
    println!("BEFORE edge=empty_inputs source_of_truth=device_copyback expected=[]");
    let core =
        compute_core_distances_gpu(&[], 0, 2, 1).expect("empty core input should be accepted");
    let pairwise =
        compute_pairwise_distances_gpu(&[0.0, 0.0], 1, 2).expect("single point has no pairs");
    println!("AFTER edge=empty_inputs source_of_truth=device_copyback core={core:?} pairwise={pairwise:?}");
    assert!(core.is_empty());
    assert!(pairwise.is_empty());
}

#[test]
fn native_sm120a_cubin_rejects_invalid_vector_length() {
    let vectors = vec![0.0f32, 0.0, 3.0, 4.0, 6.0];
    println!(
        "BEFORE edge=invalid_vector_length source_of_truth=error_state len={} n_points=3 dimension=2",
        vectors.len()
    );
    let err = compute_pairwise_distances_gpu(&vectors, 3, 2)
        .expect_err("invalid vector length must fail before launch");
    println!("AFTER edge=invalid_vector_length source_of_truth=error_state error={err}");
    let msg = err.to_string();
    assert!(msg.contains("Expected 6 elements, got 5"));
}
