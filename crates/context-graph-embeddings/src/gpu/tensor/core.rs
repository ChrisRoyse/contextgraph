//! Core GpuTensor type and constructors.
//!
//! This module contains the main `GpuTensor` struct and its constructors.

use crate::gpu::device::{default_dtype, device};
use candle_core::{DType, Device, Tensor};

/// Type-safe wrapper for GPU tensors with memory tracking.
#[derive(Debug, Clone)]
pub struct GpuTensor {
    /// Underlying Candle tensor on GPU.
    pub(crate) inner: Tensor,
    /// Original shape for validation.
    pub(crate) shape: Vec<usize>,
    /// Memory size in bytes.
    pub(crate) memory_bytes: usize,
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
    /// ```rust,no_run
    /// # use context_graph_embeddings::gpu::GpuTensor;
    /// # use context_graph_embeddings::gpu::init_gpu;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// init_gpu()?;
    /// let embedding = vec![0.1, 0.2, 0.3, 0.4];
    /// let tensor = GpuTensor::from_vec(&embedding)?;
    /// assert_eq!(tensor.dim(), 4);
    /// # Ok(())
    /// # }
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
    /// ```rust,no_run
    /// # use context_graph_embeddings::gpu::GpuTensor;
    /// # use context_graph_embeddings::gpu::init_gpu;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// init_gpu()?;
    /// // 2 vectors of dimension 4
    /// let batch = vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8];
    /// let tensor = GpuTensor::from_batch(&batch, 2, 4)?;
    /// assert_eq!(tensor.batch_size(), 2);
    /// # Ok(())
    /// # }
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
