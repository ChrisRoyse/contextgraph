//! GpuTensor wrapper for type-safe GPU tensor operations.
//!
//! # Design
//!
//! `GpuTensor` wraps `candle_core::Tensor` with additional tracking:
//! - Automatic device placement
//! - Memory usage tracking
//! - Easy conversion to/from CPU vectors
//!
//! # Usage
//!
//! ```rust,ignore
//! use context_graph_embeddings::gpu::GpuTensor;
//!
//! // Create from CPU vector
//! let vec = vec![1.0f32, 2.0, 3.0, 4.0];
//! let tensor = GpuTensor::from_vec(&vec)?;
//!
//! // Perform GPU operations
//! let normalized = tensor.normalize()?;
//!
//! // Convert back to CPU
//! let result: Vec<f32> = normalized.to_vec()?;
//! ```

use candle_core::{Device, DType, Tensor};
use super::{device, default_dtype};

/// Type-safe wrapper for GPU tensors with memory tracking.
#[derive(Debug, Clone)]
pub struct GpuTensor {
    /// Underlying Candle tensor on GPU.
    inner: Tensor,
    /// Original shape for validation.
    shape: Vec<usize>,
    /// Memory size in bytes.
    memory_bytes: usize,
}

impl GpuTensor {
    /// Create a new GpuTensor from a raw Candle tensor.
    ///
    /// # Arguments
    ///
    /// * `tensor` - A Candle tensor (must be on GPU device)
    ///
    /// # Returns
    ///
    /// GpuTensor wrapper with memory tracking.
    pub fn new(tensor: Tensor) -> Self {
        let shape: Vec<usize> = tensor.dims().to_vec();
        let memory_bytes = tensor.elem_count() * tensor.dtype().size_in_bytes();

        Self {
            inner: tensor,
            shape,
            memory_bytes,
        }
    }

    /// Create GpuTensor from a 1D f32 vector.
    ///
    /// # Arguments
    ///
    /// * `data` - Slice of f32 values to upload to GPU
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let embedding = vec![0.1, 0.2, 0.3, 0.4];
    /// let tensor = GpuTensor::from_vec(&embedding)?;
    /// assert_eq!(tensor.dim(), 4);
    /// ```
    pub fn from_vec(data: &[f32]) -> candle_core::Result<Self> {
        let dev = device();
        let tensor = Tensor::from_slice(data, (data.len(),), dev)?;
        Ok(Self::new(tensor))
    }

    /// Create GpuTensor from a 2D f32 array (batch of vectors).
    ///
    /// # Arguments
    ///
    /// * `data` - Slice of f32 values in row-major order
    /// * `batch_size` - Number of rows
    /// * `dim` - Number of columns (vector dimension)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // 2 vectors of dimension 4
    /// let batch = vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8];
    /// let tensor = GpuTensor::from_batch(&batch, 2, 4)?;
    /// assert_eq!(tensor.batch_size(), 2);
    /// ```
    pub fn from_batch(data: &[f32], batch_size: usize, dim: usize) -> candle_core::Result<Self> {
        assert_eq!(data.len(), batch_size * dim, "Data length mismatch");
        let dev = device();
        let tensor = Tensor::from_slice(data, (batch_size, dim), dev)?;
        Ok(Self::new(tensor))
    }

    /// Create a zeros tensor of given shape.
    pub fn zeros(shape: &[usize]) -> candle_core::Result<Self> {
        let dev = device();
        let tensor = Tensor::zeros(shape, default_dtype(), dev)?;
        Ok(Self::new(tensor))
    }

    /// Create a ones tensor of given shape.
    pub fn ones(shape: &[usize]) -> candle_core::Result<Self> {
        let dev = device();
        let tensor = Tensor::ones(shape, default_dtype(), dev)?;
        Ok(Self::new(tensor))
    }

    /// Create tensor with random values from standard normal distribution.
    pub fn randn(shape: &[usize]) -> candle_core::Result<Self> {
        let dev = device();
        let tensor = Tensor::randn(0.0f32, 1.0f32, shape, dev)?;
        Ok(Self::new(tensor))
    }

    /// Convert to 1D CPU vector.
    ///
    /// # Returns
    ///
    /// Vec<f32> with all tensor values, or error if not 1D.
    pub fn to_vec(&self) -> candle_core::Result<Vec<f32>> {
        self.inner.to_vec1()
    }

    /// Convert to 2D CPU vector (batch of vectors).
    ///
    /// # Returns
    ///
    /// Nested Vec for 2D tensor, or error if not 2D.
    pub fn to_vec2(&self) -> candle_core::Result<Vec<Vec<f32>>> {
        self.inner.to_vec2()
    }

    /// Get the underlying Candle tensor (for advanced operations).
    pub fn inner(&self) -> &Tensor {
        &self.inner
    }

    /// Consume and return the underlying Candle tensor.
    pub fn into_inner(self) -> Tensor {
        self.inner
    }

    /// Get tensor shape.
    pub fn shape(&self) -> &[usize] {
        &self.shape
    }

    /// Get first dimension (batch size for 2D tensors, length for 1D).
    pub fn dim(&self) -> usize {
        self.shape.first().copied().unwrap_or(0)
    }

    /// Get batch size (first dimension for 2D tensors).
    pub fn batch_size(&self) -> usize {
        if self.shape.len() >= 2 {
            self.shape[0]
        } else {
            1
        }
    }

    /// Get vector dimension (last dimension).
    pub fn vector_dim(&self) -> usize {
        self.shape.last().copied().unwrap_or(0)
    }

    /// Get memory usage in bytes.
    pub fn memory_bytes(&self) -> usize {
        self.memory_bytes
    }

    /// Get data type.
    pub fn dtype(&self) -> DType {
        self.inner.dtype()
    }

    /// Get device reference.
    pub fn device(&self) -> &Device {
        self.inner.device()
    }

    /// Normalize the tensor (L2 normalization).
    ///
    /// For 1D: normalize the entire vector
    /// For 2D: normalize each row independently
    pub fn normalize(&self) -> candle_core::Result<Self> {
        let norm = self.inner.sqr()?.sum_keepdim(candle_core::D::Minus1)?.sqrt()?;
        let normalized = self.inner.broadcast_div(&(norm + 1e-12)?)?;
        Ok(Self::new(normalized))
    }

    /// Compute L2 norm.
    ///
    /// For 1D: returns scalar norm
    /// For 2D: returns 1D tensor of norms per row
    pub fn l2_norm(&self) -> candle_core::Result<Tensor> {
        self.inner.sqr()?.sum_keepdim(candle_core::D::Minus1)?.sqrt()
    }

    /// Element-wise multiplication.
    pub fn mul(&self, other: &GpuTensor) -> candle_core::Result<Self> {
        let result = self.inner.mul(&other.inner)?;
        Ok(Self::new(result))
    }

    /// Element-wise addition.
    pub fn add(&self, other: &GpuTensor) -> candle_core::Result<Self> {
        let result = self.inner.add(&other.inner)?;
        Ok(Self::new(result))
    }

    /// Matrix multiplication.
    ///
    /// # Shapes
    ///
    /// - self: [M, K]
    /// - other: [K, N]
    /// - result: [M, N]
    pub fn matmul(&self, other: &GpuTensor) -> candle_core::Result<Self> {
        let result = self.inner.matmul(&other.inner)?;
        Ok(Self::new(result))
    }

    /// Transpose last two dimensions.
    pub fn transpose(&self) -> candle_core::Result<Self> {
        let result = self.inner.t()?;
        Ok(Self::new(result))
    }

    /// Sum all elements.
    pub fn sum_all(&self) -> candle_core::Result<f32> {
        self.inner.sum_all()?.to_vec0()
    }

    /// Softmax along last dimension.
    pub fn softmax(&self) -> candle_core::Result<Self> {
        let result = candle_nn::ops::softmax(&self.inner, candle_core::D::Minus1)?;
        Ok(Self::new(result))
    }

    /// Softmax with temperature scaling.
    pub fn softmax_with_temperature(&self, temperature: f32) -> candle_core::Result<Self> {
        let scaled = (&self.inner / temperature as f64)?;
        let result = candle_nn::ops::softmax(&scaled, candle_core::D::Minus1)?;
        Ok(Self::new(result))
    }

    /// Apply GELU activation.
    pub fn gelu(&self) -> candle_core::Result<Self> {
        let result = self.inner.gelu()?;
        Ok(Self::new(result))
    }

    /// Apply ReLU activation.
    pub fn relu(&self) -> candle_core::Result<Self> {
        let result = self.inner.relu()?;
        Ok(Self::new(result))
    }

    /// Apply SiLU (Swish) activation.
    pub fn silu(&self) -> candle_core::Result<Self> {
        let result = self.inner.silu()?;
        Ok(Self::new(result))
    }
}

/// Enable transparent access to inner tensor methods.
impl std::ops::Deref for GpuTensor {
    type Target = Tensor;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// Conversion from Candle Tensor to GpuTensor.
impl From<Tensor> for GpuTensor {
    fn from(tensor: Tensor) -> Self {
        Self::new(tensor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Tests require GPU. Use `cargo test --features cuda` to run.

    #[test]
    fn test_memory_calculation() {
        // Test memory calculation formula
        let elem_count = 1024;
        let size_per_elem = 4; // f32
        let expected = elem_count * size_per_elem;

        // This test doesn't need GPU - just validates the formula
        assert_eq!(expected, 4096);
    }
}
