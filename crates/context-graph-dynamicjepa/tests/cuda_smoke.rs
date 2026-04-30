use candle_core::{Device, Tensor};

#[test]
fn candle_cuda_matmul_returns_expected_values() -> candle_core::Result<()> {
    let device = Device::cuda_if_available(0)?;
    assert!(
        device.is_cuda(),
        "CUDA device 0 is required; Device::cuda_if_available returned {device:?}"
    );

    let lhs = Tensor::from_slice(&[1f32, 2., 3., 4., 5., 6.], (2, 3), &device)?;
    let rhs = Tensor::from_slice(&[7f32, 8., 9., 10., 11., 12.], (3, 2), &device)?;

    let product = lhs.matmul(&rhs)?;
    let actual = product.to_vec2::<f32>()?;
    let expected = [[58f32, 64.], [139., 154.]];

    eprintln!("CUDA smoke source_of_truth=tensor_copyback actual={actual:?}");
    for row in 0..2 {
        for col in 0..2 {
            assert!(
                (actual[row][col] - expected[row][col]).abs() < 1e-5,
                "unexpected CUDA matmul result at [{row}][{col}]: expected {}, got {}",
                expected[row][col],
                actual[row][col]
            );
        }
    }

    Ok(())
}
