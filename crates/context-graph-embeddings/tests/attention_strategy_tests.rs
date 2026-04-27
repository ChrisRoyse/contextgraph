//! Integration tests for attention strategy implementations.
//!
//! Verifies:
//! 1. All strategies produce correct output shapes
//! 2. TiledAttention matches DenseAttention within tolerance
//! 3. SlidingWindow matches Dense when window >= seq_len
//! 4. Config wiring maps use_flash_attention correctly

use candle_core::{Device, Tensor};

use context_graph_embeddings::models::attention::{
    create_strategy, resolve_attention_mode, AttentionMode,
};

/// Create a simple attention test scenario on CPU.
///
/// Returns (q, k, v, mask, scale) with shape:
/// - q, k, v: [batch=1, heads=2, seq_len, head_dim=4]
/// - mask: [1, 1, 1, seq_len] (no masking)
fn make_test_inputs(seq_len: usize) -> (Tensor, Tensor, Tensor, Tensor, f64) {
    let device = Device::Cpu;
    let head_dim = 4usize;
    let batch = 1usize;
    let heads = 2usize;

    // Use deterministic values (not random — reproducible)
    let total_q = batch * heads * seq_len * head_dim;
    let q_data: Vec<f32> = (0..total_q).map(|i| (i as f32 * 0.1).sin() * 0.5).collect();
    let k_data: Vec<f32> = (0..total_q)
        .map(|i| (i as f32 * 0.13 + 1.0).sin() * 0.5)
        .collect();
    let v_data: Vec<f32> = (0..total_q)
        .map(|i| (i as f32 * 0.17 + 2.0).sin() * 0.5)
        .collect();

    let q = Tensor::from_slice(&q_data, (batch, heads, seq_len, head_dim), &device).unwrap();
    let k = Tensor::from_slice(&k_data, (batch, heads, seq_len, head_dim), &device).unwrap();
    let v = Tensor::from_slice(&v_data, (batch, heads, seq_len, head_dim), &device).unwrap();

    // No masking: all zeros
    let mask_data = vec![0.0f32; seq_len];
    let mask = Tensor::from_slice(&mask_data, (1, 1, 1, seq_len), &device).unwrap();

    let scale = (head_dim as f64).sqrt();

    (q, k, v, mask, scale)
}

/// Create inputs with padding mask (last `pad` tokens masked).
fn make_test_inputs_with_padding(
    seq_len: usize,
    pad: usize,
) -> (Tensor, Tensor, Tensor, Tensor, f64) {
    let (q, k, v, _mask, scale) = make_test_inputs(seq_len);
    let device = Device::Cpu;

    let mut mask_data = vec![0.0f32; seq_len];
    for item in mask_data.iter_mut().skip(seq_len - pad) {
        *item = -10000.0;
    }
    let mask = Tensor::from_slice(&mask_data, (1, 1, 1, seq_len), &device).unwrap();

    (q, k, v, mask, scale)
}

/// Assert two tensors are approximately equal (within tolerance).
fn assert_tensors_close(a: &Tensor, b: &Tensor, tol: f64, label: &str) {
    let a_flat: Vec<f32> = a.flatten_all().unwrap().to_vec1().unwrap();
    let b_flat: Vec<f32> = b.flatten_all().unwrap().to_vec1().unwrap();
    assert_eq!(
        a_flat.len(),
        b_flat.len(),
        "{}: shape mismatch {} vs {}",
        label,
        a_flat.len(),
        b_flat.len()
    );

    let mut max_diff = 0.0f64;
    for (i, (x, y)) in a_flat.iter().zip(b_flat.iter()).enumerate() {
        let diff = (*x as f64 - *y as f64).abs();
        if diff > max_diff {
            max_diff = diff;
        }
        assert!(
            diff <= tol,
            "{}: element {} differs: {} vs {} (diff={}, tol={})",
            label,
            i,
            x,
            y,
            diff,
            tol,
        );
    }
}

// =============================================================================
// Output shape tests
// =============================================================================

#[test]
fn test_dense_output_shape() {
    let (q, k, v, mask, scale) = make_test_inputs(8);
    let strategy = create_strategy(&AttentionMode::Dense);
    let output = strategy.forward(&q, &k, &v, &mask, scale).unwrap();
    assert_eq!(output.dims(), q.dims(), "Dense output shape must match Q");
}

#[test]
fn test_tiled_output_shape() {
    let (q, k, v, mask, scale) = make_test_inputs(8);
    let strategy = create_strategy(&AttentionMode::MemoryEfficient { tile_size: 4 });
    let output = strategy.forward(&q, &k, &v, &mask, scale).unwrap();
    assert_eq!(output.dims(), q.dims(), "Tiled output shape must match Q");
}

#[test]
fn test_sliding_window_output_shape() {
    let (q, k, v, mask, scale) = make_test_inputs(8);
    let strategy = create_strategy(&AttentionMode::SlidingWindow { window_size: 4 });
    let output = strategy.forward(&q, &k, &v, &mask, scale).unwrap();
    assert_eq!(
        output.dims(),
        q.dims(),
        "SlidingWindow output shape must match Q"
    );
}

// =============================================================================
// Numerical equivalence: Tiled vs Dense
// =============================================================================

#[test]
fn test_tiled_matches_dense_short_sequence() {
    let (q, k, v, mask, scale) = make_test_inputs(8);

    let dense = create_strategy(&AttentionMode::Dense);
    let tiled = create_strategy(&AttentionMode::MemoryEfficient { tile_size: 4 });

    let dense_out = dense.forward(&q, &k, &v, &mask, scale).unwrap();
    let tiled_out = tiled.forward(&q, &k, &v, &mask, scale).unwrap();

    assert_tensors_close(&dense_out, &tiled_out, 1e-4, "tiled vs dense (seq=8)");
}

#[test]
fn test_tiled_matches_dense_longer_sequence() {
    let (q, k, v, mask, scale) = make_test_inputs(32);

    let dense = create_strategy(&AttentionMode::Dense);
    let tiled = create_strategy(&AttentionMode::MemoryEfficient { tile_size: 8 });

    let dense_out = dense.forward(&q, &k, &v, &mask, scale).unwrap();
    let tiled_out = tiled.forward(&q, &k, &v, &mask, scale).unwrap();

    assert_tensors_close(&dense_out, &tiled_out, 1e-4, "tiled vs dense (seq=32)");
}

#[test]
fn test_tiled_matches_dense_with_padding() {
    let (q, k, v, mask, scale) = make_test_inputs_with_padding(16, 4);

    let dense = create_strategy(&AttentionMode::Dense);
    let tiled = create_strategy(&AttentionMode::MemoryEfficient { tile_size: 4 });

    let dense_out = dense.forward(&q, &k, &v, &mask, scale).unwrap();
    let tiled_out = tiled.forward(&q, &k, &v, &mask, scale).unwrap();

    assert_tensors_close(
        &dense_out,
        &tiled_out,
        1e-4,
        "tiled vs dense (seq=16, pad=4)",
    );
}

#[test]
fn test_tiled_fallback_to_dense_when_tile_covers_sequence() {
    // When tile_size >= seq_len, tiled should fall back to dense and produce identical output.
    let (q, k, v, mask, scale) = make_test_inputs(8);

    let dense = create_strategy(&AttentionMode::Dense);
    let tiled = create_strategy(&AttentionMode::MemoryEfficient { tile_size: 256 });

    let dense_out = dense.forward(&q, &k, &v, &mask, scale).unwrap();
    let tiled_out = tiled.forward(&q, &k, &v, &mask, scale).unwrap();

    // Should be bit-exact since it falls back to dense
    assert_tensors_close(&dense_out, &tiled_out, 1e-6, "tiled fallback (tile >= seq)");
}

// =============================================================================
// Numerical equivalence: SlidingWindow vs Dense (when window >= seq_len)
// =============================================================================

#[test]
fn test_sliding_window_matches_dense_when_window_covers_sequence() {
    let (q, k, v, mask, scale) = make_test_inputs(8);

    let dense = create_strategy(&AttentionMode::Dense);
    let sliding = create_strategy(&AttentionMode::SlidingWindow { window_size: 16 });

    let dense_out = dense.forward(&q, &k, &v, &mask, scale).unwrap();
    let sliding_out = sliding.forward(&q, &k, &v, &mask, scale).unwrap();

    // Should be bit-exact since it falls back to dense
    assert_tensors_close(
        &dense_out,
        &sliding_out,
        1e-6,
        "sliding window fallback (w >= seq)",
    );
}

#[test]
fn test_sliding_window_restricts_attention() {
    // With a small window, the output should differ from dense attention
    let (q, k, v, mask, scale) = make_test_inputs(16);

    let dense = create_strategy(&AttentionMode::Dense);
    let sliding = create_strategy(&AttentionMode::SlidingWindow { window_size: 4 });

    let dense_out = dense.forward(&q, &k, &v, &mask, scale).unwrap();
    let sliding_out = sliding.forward(&q, &k, &v, &mask, scale).unwrap();

    // These should NOT be equal since the window restricts attention
    let dense_flat: Vec<f32> = dense_out.flatten_all().unwrap().to_vec1().unwrap();
    let sliding_flat: Vec<f32> = sliding_out.flatten_all().unwrap().to_vec1().unwrap();

    let max_diff: f64 = dense_flat
        .iter()
        .zip(sliding_flat.iter())
        .map(|(a, b)| (*a as f64 - *b as f64).abs())
        .fold(0.0, f64::max);

    assert!(
        max_diff > 1e-3,
        "Sliding window (w=4) should differ from dense for seq_len=16, but max_diff={}",
        max_diff
    );
}

// =============================================================================
// Config wiring tests
// =============================================================================

#[test]
fn test_config_wiring_flash_attention_true() {
    let mode = resolve_attention_mode(None, true);
    assert_eq!(mode, AttentionMode::MemoryEfficient { tile_size: 256 });
}

#[test]
fn test_config_wiring_flash_attention_false() {
    let mode = resolve_attention_mode(None, false);
    assert_eq!(mode, AttentionMode::Dense);
}

#[test]
fn test_config_wiring_explicit_overrides_legacy() {
    let explicit = AttentionMode::SlidingWindow { window_size: 128 };
    let mode = resolve_attention_mode(Some(&explicit), true);
    assert_eq!(mode, AttentionMode::SlidingWindow { window_size: 128 });
}

// =============================================================================
// Edge cases
// =============================================================================

#[test]
fn test_single_token_sequence() {
    let (q, k, v, mask, scale) = make_test_inputs(1);

    for mode in &[
        AttentionMode::Dense,
        AttentionMode::MemoryEfficient { tile_size: 4 },
        AttentionMode::SlidingWindow { window_size: 4 },
    ] {
        let strategy = create_strategy(mode);
        let output = strategy.forward(&q, &k, &v, &mask, scale).unwrap();
        assert_eq!(
            output.dims(),
            q.dims(),
            "seq_len=1 output shape for {:?}",
            mode
        );
    }
}

#[test]
fn test_tiled_non_divisible_sequence() {
    // seq_len=7 with tile_size=4: tiles are [4, 3] — tests remainder handling
    let (q, k, v, mask, scale) = make_test_inputs(7);

    let dense = create_strategy(&AttentionMode::Dense);
    let tiled = create_strategy(&AttentionMode::MemoryEfficient { tile_size: 4 });

    let dense_out = dense.forward(&q, &k, &v, &mask, scale).unwrap();
    let tiled_out = tiled.forward(&q, &k, &v, &mask, scale).unwrap();

    assert_tensors_close(
        &dense_out,
        &tiled_out,
        1e-4,
        "tiled vs dense (seq=7, tile=4, non-divisible)",
    );
}

#[test]
fn test_strategy_names() {
    assert_eq!(create_strategy(&AttentionMode::Dense).name(), "dense");
    assert_eq!(
        create_strategy(&AttentionMode::MemoryEfficient { tile_size: 128 }).name(),
        "tiled_memory_efficient"
    );
    assert_eq!(
        create_strategy(&AttentionMode::SlidingWindow { window_size: 256 }).name(),
        "sliding_window"
    );
}
