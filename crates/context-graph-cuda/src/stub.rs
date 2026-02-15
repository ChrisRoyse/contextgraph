//! Stub CPU implementation of vector operations.
//!
//! # WARNING: TEST ONLY - NOT FOR PRODUCTION USE
//!
//! This module provides CPU-based stub implementations that are intended
//! **exclusively for testing purposes**. Using these stubs in production
//! violates Constitution AP-007: "CUDA is ALWAYS required - no stub implementations".
//!
//! ## AP-007 Compliance
//!
//! Production code paths MUST use real CUDA implementations. The stub module
//! is gated with `#[cfg(test)]` in `lib.rs` to prevent accidental production usage.
//!
//! ## Allowed Usage
//!
//! - Unit tests that need to verify vector operation logic
//! - Integration tests where GPU hardware is unavailable
//! - CI/CD pipelines without GPU access (test builds only)
//!
//! ## Prohibited Usage
//!
//! - Any production code path
//! - Any code that processes real user data
//! - Any deployment artifact

use async_trait::async_trait;

use crate::error::{CudaError, CudaResult};
use crate::ops::VectorOps;

/// CPU stub for GPU operations.
///
/// # WARNING: TEST ONLY - NOT FOR PRODUCTION USE
///
/// This struct provides CPU-based implementations of vector operations
/// for **testing purposes only**. Using this in production violates
/// Constitution AP-007.
///
/// # Constitution AP-007 Compliance
///
/// Production code MUST use real CUDA implementations. This stub is
/// gated with `#[cfg(test)]` to prevent production usage.
#[derive(Debug, Clone, Default)]
pub struct StubVectorOps {
    device_name: String,
}

impl StubVectorOps {
    /// Create a new stub vector ops instance.
    pub fn new() -> Self {
        Self {
            device_name: "CPU (Stub)".to_string(),
        }
    }
}

#[async_trait]
impl VectorOps for StubVectorOps {
    async fn cosine_similarity(&self, a: &[f32], b: &[f32]) -> CudaResult<f32> {
        if a.len() != b.len() {
            return Err(CudaError::DimensionMismatch {
                expected: a.len(),
                actual: b.len(),
            });
        }

        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            return Ok(0.0);
        }

        Ok(dot / (norm_a * norm_b))
    }

    async fn dot_product(&self, a: &[f32], b: &[f32]) -> CudaResult<f32> {
        if a.len() != b.len() {
            return Err(CudaError::DimensionMismatch {
                expected: a.len(),
                actual: b.len(),
            });
        }

        Ok(a.iter().zip(b.iter()).map(|(x, y)| x * y).sum())
    }

    async fn normalize(&self, v: &[f32]) -> CudaResult<Vec<f32>> {
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm == 0.0 {
            return Ok(vec![0.0; v.len()]);
        }
        Ok(v.iter().map(|x| x / norm).collect())
    }

    async fn batch_cosine_similarity(
        &self,
        query: &[f32],
        vectors: &[Vec<f32>],
    ) -> CudaResult<Vec<f32>> {
        let mut results = Vec::with_capacity(vectors.len());
        for v in vectors {
            results.push(self.cosine_similarity(query, v).await?);
        }
        Ok(results)
    }

    async fn matmul(
        &self,
        a: &[f32],
        b: &[f32],
        m: usize,
        n: usize,
        k: usize,
    ) -> CudaResult<Vec<f32>> {
        if a.len() != m * k || b.len() != k * n {
            return Err(CudaError::DimensionMismatch {
                expected: m * k,
                actual: a.len(),
            });
        }

        let mut c = vec![0.0; m * n];
        for i in 0..m {
            for j in 0..n {
                let mut sum = 0.0;
                for l in 0..k {
                    sum += a[i * k + l] * b[l * n + j];
                }
                c[i * n + j] = sum;
            }
        }
        Ok(c)
    }

    async fn softmax(&self, v: &[f32]) -> CudaResult<Vec<f32>> {
        let max_val = v.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exp_sum: f32 = v.iter().map(|x| (x - max_val).exp()).sum();
        Ok(v.iter().map(|x| (x - max_val).exp() / exp_sum).collect())
    }

    fn is_gpu_available(&self) -> bool {
        false
    }

    fn device_name(&self) -> &str {
        &self.device_name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cosine_similarity_identical() {
        let ops = StubVectorOps::new();
        let v = vec![1.0, 2.0, 3.0];
        let sim = ops.cosine_similarity(&v, &v).await.unwrap();
        assert!((sim - 1.0).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_cosine_similarity_orthogonal() {
        let ops = StubVectorOps::new();
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let sim = ops.cosine_similarity(&a, &b).await.unwrap();
        assert!(sim.abs() < 0.001);
    }

    #[tokio::test]
    async fn test_dot_product() {
        let ops = StubVectorOps::new();
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let dot = ops.dot_product(&a, &b).await.unwrap();
        assert!((dot - 32.0).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_normalize() {
        let ops = StubVectorOps::new();
        let v = vec![3.0, 4.0];
        let normalized = ops.normalize(&v).await.unwrap();
        let norm: f32 = normalized.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_softmax() {
        let ops = StubVectorOps::new();
        let v = vec![1.0, 2.0, 3.0];
        let result = ops.softmax(&v).await.unwrap();
        let sum: f32 = result.iter().sum();
        assert!((sum - 1.0).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_matmul() {
        let ops = StubVectorOps::new();
        // 2x2 identity * 2x2 values
        let a = vec![1.0, 0.0, 0.0, 1.0];
        let b = vec![1.0, 2.0, 3.0, 4.0];
        let c = ops.matmul(&a, &b, 2, 2, 2).await.unwrap();
        assert_eq!(c, b);
    }

    #[tokio::test]
    async fn test_dimension_mismatch() {
        let ops = StubVectorOps::new();
        let a = vec![1.0, 2.0];
        let b = vec![1.0, 2.0, 3.0];
        let result = ops.cosine_similarity(&a, &b).await;
        assert!(
            matches!(result, Err(CudaError::DimensionMismatch { expected: 2, actual: 3 })),
            "Expected DimensionMismatch error, got: {:?}", result
        );
    }
}
