//! Model factory trait and configuration types.
//!
//! This module defines the factory pattern for creating embedding model instances.
//! The factory abstracts model creation, enabling dependency injection and testability.
//!
//! # Thread Safety
//!
//! All types require `Send + Sync` for safe concurrent access.
//! Factory implementations can be shared across async tasks via `Arc<dyn ModelFactory>`.
//!
//! # Design Principles
//!
//! - **NO FALLBACKS**: Invalid config returns `EmbeddingError::ConfigError`
//! - **FAIL FAST**: Unknown ModelId returns `EmbeddingError::ModelNotFound`
//! - **CONSERVATIVE ESTIMATES**: Memory estimates are overestimates, never underestimates

use serde::{Deserialize, Serialize};

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::traits::EmbeddingModel;
use crate::types::ModelId;

// ============================================================================
// DEVICE PLACEMENT
// ============================================================================

/// Device placement options for model inference.
///
/// Determines where model weights are loaded and inference is executed.
///
/// # Serialization
///
/// Serializes as snake_case strings for TOML compatibility:
/// - `"cpu"` -> `DevicePlacement::Cpu`
/// - `"auto"` -> `DevicePlacement::Auto`
/// - `{ "cuda": 0 }` -> `DevicePlacement::Cuda(0)`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DevicePlacement {
    /// CPU-only inference. Slower but always available.
    Cpu,

    /// Specific CUDA device by index.
    /// Index 0 is the primary GPU (RTX 5090).
    Cuda(u32),

    /// Auto-select best available device.
    /// Prefers CUDA if available, falls back to CPU.
    #[default]
    Auto,
}

impl DevicePlacement {
    /// Returns true if this placement requires a GPU.
    pub fn requires_gpu(&self) -> bool {
        matches!(self, DevicePlacement::Cuda(_))
    }

    /// Returns the CUDA device ID if specified, None otherwise.
    pub fn cuda_device_id(&self) -> Option<u32> {
        match self {
            DevicePlacement::Cuda(id) => Some(*id),
            _ => None,
        }
    }
}

// ============================================================================
// QUANTIZATION MODE
// ============================================================================

/// Quantization modes for memory reduction.
///
/// Lower precision reduces memory and may increase throughput,
/// but can affect embedding quality.
///
/// # RTX 5090 Tensor Core Support
///
/// | Mode | Memory | Speed | Quality | Tensor Core |
/// |------|--------|-------|---------|-------------|
/// | None | 100% | Baseline | 100% | FP32/TF32 |
/// | FP16 | 50% | ~2x | ~99.5% | Yes |
/// | BF16 | 50% | ~2x | ~99.5% | Yes |
/// | Int8 | 25% | ~3x | ~99% | Yes |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum QuantizationMode {
    /// No quantization (FP32 weights).
    #[default]
    None,

    /// 8-bit integer quantization.
    /// Best memory savings, slight quality reduction.
    Int8,

    /// 16-bit floating point.
    /// Good balance of memory and quality.
    Fp16,

    /// Brain floating point 16-bit.
    /// Better for training, good for inference.
    Bf16,
}

impl QuantizationMode {
    /// Returns the memory multiplier relative to FP32.
    /// Example: FP16 returns 0.5 (50% of FP32 memory).
    pub fn memory_multiplier(&self) -> f32 {
        match self {
            QuantizationMode::None => 1.0,
            QuantizationMode::Int8 => 0.25,
            QuantizationMode::Fp16 | QuantizationMode::Bf16 => 0.5,
        }
    }

    /// Returns bytes per parameter.
    pub fn bytes_per_param(&self) -> usize {
        match self {
            QuantizationMode::None => 4,
            QuantizationMode::Fp16 | QuantizationMode::Bf16 => 2,
            QuantizationMode::Int8 => 1,
        }
    }
}

// ============================================================================
// SINGLE MODEL CONFIG
// ============================================================================

/// Configuration for a single embedding model.
///
/// Controls device placement, quantization, and inference parameters.
///
/// # Example
///
/// ```rust,ignore
/// let config = SingleModelConfig {
///     device: DevicePlacement::Cuda(0),
///     quantization: QuantizationMode::Fp16,
///     max_batch_size: 32,
///     use_flash_attention: true,
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SingleModelConfig {
    /// Device placement for this model.
    #[serde(default)]
    pub device: DevicePlacement,

    /// Quantization mode for reduced memory.
    #[serde(default)]
    pub quantization: QuantizationMode,

    /// Maximum batch size for this model.
    /// Larger batches improve throughput but use more memory.
    #[serde(default = "default_max_batch_size")]
    pub max_batch_size: usize,

    /// Whether to use flash attention if available.
    /// Flash attention reduces memory and improves speed.
    #[serde(default = "default_use_flash_attention")]
    pub use_flash_attention: bool,
}

fn default_max_batch_size() -> usize {
    32
}
fn default_use_flash_attention() -> bool {
    true
}

impl Default for SingleModelConfig {
    fn default() -> Self {
        Self {
            device: DevicePlacement::Auto,
            quantization: QuantizationMode::None,
            max_batch_size: 32,
            use_flash_attention: true,
        }
    }
}

impl SingleModelConfig {
    /// Create config for CPU-only inference.
    pub fn cpu() -> Self {
        Self {
            device: DevicePlacement::Cpu,
            ..Default::default()
        }
    }

    /// Create config for CUDA device 0.
    pub fn cuda() -> Self {
        Self {
            device: DevicePlacement::Cuda(0),
            ..Default::default()
        }
    }

    /// Create config with FP16 quantization on CUDA.
    pub fn cuda_fp16() -> Self {
        Self {
            device: DevicePlacement::Cuda(0),
            quantization: QuantizationMode::Fp16,
            ..Default::default()
        }
    }

    /// Validate configuration values.
    ///
    /// # Errors
    /// - `EmbeddingError::ConfigError` if max_batch_size is 0
    pub fn validate(&self) -> EmbeddingResult<()> {
        if self.max_batch_size == 0 {
            return Err(EmbeddingError::ConfigError {
                message: "max_batch_size must be greater than 0".to_string(),
            });
        }
        Ok(())
    }
}

// ============================================================================
// MODEL FACTORY TRAIT
// ============================================================================

/// Factory trait for creating embedding model instances.
///
/// This trait abstracts model creation, enabling:
/// - Dependency injection for testing
/// - Configuration-driven model instantiation
/// - Memory estimation before allocation
///
/// # Thread Safety
///
/// Requires `Send + Sync` for concurrent access via `Arc<dyn ModelFactory>`.
///
/// # Lifecycle
///
/// ```text
/// [Factory] --create_model()--> [Unloaded Model] --load()--> [Ready Model]
/// ```
///
/// The factory creates unloaded model instances. Callers must call
/// `EmbeddingModel::load()` before using `embed()`.
///
/// # Example
///
/// ```rust,ignore
/// use context_graph_embeddings::traits::{ModelFactory, EmbeddingModel};
/// use context_graph_embeddings::types::ModelId;
///
/// async fn create_and_use(factory: &dyn ModelFactory) -> EmbeddingResult<()> {
///     let config = SingleModelConfig::cuda_fp16();
///
///     // Check memory before allocation
///     let memory_needed = factory.estimate_memory(ModelId::Semantic);
///     println!("Model needs {} bytes", memory_needed);
///
///     // Create and load model
///     let model = factory.create_model(ModelId::Semantic, &config)?;
///     model.load().await?;
///
///     // Model is now ready for inference
///     assert!(model.is_initialized());
///     Ok(())
/// }
/// ```
#[async_trait::async_trait]
pub trait ModelFactory: Send + Sync {
    /// Create a model instance for the given ModelId with configuration.
    ///
    /// # Arguments
    /// * `model_id` - The model variant to create (E1-E12)
    /// * `config` - Model-specific configuration (device, quantization, etc.)
    ///
    /// # Returns
    /// A boxed `EmbeddingModel` trait object. The model is **NOT** loaded yet.
    /// Call `model.load().await` before using `embed()`.
    ///
    /// # Errors
    /// - `EmbeddingError::ModelNotFound` if model_id not supported by this factory
    /// - `EmbeddingError::ConfigError` if configuration is invalid
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let model = factory.create_model(ModelId::Semantic, &config)?;
    /// assert!(!model.is_initialized()); // Not loaded yet
    /// model.load().await?;
    /// assert!(model.is_initialized()); // Now ready
    /// ```
    fn create_model(
        &self,
        model_id: ModelId,
        config: &SingleModelConfig,
    ) -> EmbeddingResult<Box<dyn EmbeddingModel>>;

    /// Returns list of ModelIds this factory can create.
    ///
    /// # Returns
    /// Static slice of supported `ModelId` variants.
    /// A full factory supports all 12 models.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let models = factory.supported_models();
    /// assert!(models.contains(&ModelId::Semantic));
    /// assert_eq!(models.len(), 12); // Full factory
    /// ```
    fn supported_models(&self) -> &[ModelId];

    /// Check if this factory can create the specified model.
    ///
    /// # Arguments
    /// * `model_id` - The model to check
    ///
    /// # Returns
    /// `true` if `create_model()` will succeed for this model_id.
    fn supports_model(&self, model_id: ModelId) -> bool {
        self.supported_models().contains(&model_id)
    }

    /// Estimate memory usage for loading a model.
    ///
    /// Returns a **conservative overestimate** of bytes required.
    /// Actual memory may be lower, but never higher.
    ///
    /// # Arguments
    /// * `model_id` - The model to estimate
    ///
    /// # Returns
    /// Estimated bytes required. Returns 0 only if model_id is unsupported.
    ///
    /// # Memory Estimates (FP32, no quantization)
    ///
    /// | ModelId | Estimate |
    /// |---------|----------|
    /// | Semantic (e5-large) | 1.3 GB |
    /// | TemporalRecent | 10 MB |
    /// | TemporalPeriodic | 10 MB |
    /// | TemporalPositional | 10 MB |
    /// | Causal (Longformer) | 600 MB |
    /// | Sparse (SPLADE) | 500 MB |
    /// | Code (CodeBERT) | 500 MB |
    /// | Graph (MiniLM) | 100 MB |
    /// | Hdc | 50 MB |
    /// | Multimodal (CLIP) | 1.5 GB |
    /// | Entity (MiniLM) | 100 MB |
    /// | LateInteraction (ColBERT) | 400 MB |
    fn estimate_memory(&self, model_id: ModelId) -> usize;

    /// Estimate memory with specific quantization.
    ///
    /// Applies the quantization multiplier to the base estimate.
    ///
    /// # Arguments
    /// * `model_id` - The model to estimate
    /// * `quantization` - The quantization mode to apply
    ///
    /// # Returns
    /// Adjusted memory estimate in bytes.
    fn estimate_memory_quantized(&self, model_id: ModelId, quantization: QuantizationMode) -> usize {
        let base = self.estimate_memory(model_id);
        (base as f32 * quantization.memory_multiplier()) as usize
    }
}

// ============================================================================
// MEMORY ESTIMATES (CONSTANTS)
// ============================================================================

/// Memory estimates for each model (in bytes, FP32).
/// These are conservative overestimates.
pub const MEMORY_ESTIMATES: [(ModelId, usize); 12] = [
    (ModelId::Semantic, 1_400_000_000),        // 1.3 GB + buffer
    (ModelId::TemporalRecent, 15_000_000),     // 10 MB + buffer
    (ModelId::TemporalPeriodic, 15_000_000),   // 10 MB + buffer
    (ModelId::TemporalPositional, 15_000_000), // 10 MB + buffer
    (ModelId::Causal, 650_000_000),            // 600 MB + buffer
    (ModelId::Sparse, 550_000_000),            // 500 MB + buffer
    (ModelId::Code, 550_000_000),              // 500 MB + buffer
    (ModelId::Graph, 120_000_000),             // 100 MB + buffer
    (ModelId::Hdc, 60_000_000),                // 50 MB + buffer
    (ModelId::Multimodal, 1_600_000_000),      // 1.5 GB + buffer
    (ModelId::Entity, 120_000_000),            // 100 MB + buffer
    (ModelId::LateInteraction, 450_000_000),   // 400 MB + buffer
];

/// Get memory estimate for a ModelId.
pub fn get_memory_estimate(model_id: ModelId) -> usize {
    MEMORY_ESTIMATES
        .iter()
        .find(|(id, _)| *id == model_id)
        .map(|(_, mem)| *mem)
        .unwrap_or(0)
}

/// Total memory for all 12 models (FP32).
/// ~5.5 GB without quantization.
pub const TOTAL_MEMORY_ESTIMATE: usize = 5_545_000_000;

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    // =========================================================================
    // Test Model Implementation
    // =========================================================================

    /// Test implementation of EmbeddingModel for factory testing.
    struct TestModel {
        model_id: ModelId,
        #[allow(dead_code)]
        config: SingleModelConfig,
        initialized: AtomicBool,
    }

    impl TestModel {
        fn new(model_id: ModelId, config: SingleModelConfig) -> Self {
            Self {
                model_id,
                config,
                initialized: AtomicBool::new(false),
            }
        }
    }

    use crate::types::{InputType, ModelEmbedding, ModelInput};

    #[async_trait::async_trait]
    impl EmbeddingModel for TestModel {
        fn model_id(&self) -> ModelId {
            self.model_id
        }

        fn supported_input_types(&self) -> &[InputType] {
            &[InputType::Text]
        }

        async fn embed(&self, input: &ModelInput) -> EmbeddingResult<ModelEmbedding> {
            if !self.is_initialized() {
                return Err(EmbeddingError::NotInitialized {
                    model_id: self.model_id,
                });
            }

            self.validate_input(input)?;

            let dim = self.dimension();
            let vector: Vec<f32> = (0..dim).map(|i| (i as f32 * 0.001).sin()).collect();
            Ok(ModelEmbedding::new(self.model_id, vector, 100))
        }

        fn is_initialized(&self) -> bool {
            self.initialized.load(Ordering::SeqCst)
        }
    }

    // =========================================================================
    // Test Factory Implementation
    // =========================================================================

    /// Test factory that creates TestModel instances.
    struct TestFactory;

    impl ModelFactory for TestFactory {
        fn create_model(
            &self,
            model_id: ModelId,
            config: &SingleModelConfig,
        ) -> EmbeddingResult<Box<dyn EmbeddingModel>> {
            config.validate()?;

            if !self.supports_model(model_id) {
                return Err(EmbeddingError::ModelNotFound { model_id });
            }

            Ok(Box::new(TestModel::new(model_id, config.clone())))
        }

        fn supported_models(&self) -> &[ModelId] {
            ModelId::all()
        }

        fn estimate_memory(&self, model_id: ModelId) -> usize {
            get_memory_estimate(model_id)
        }
    }

    // =========================================================================
    // DEVICE PLACEMENT TESTS (5 tests)
    // =========================================================================

    #[test]
    fn test_device_placement_default_is_auto() {
        let placement = DevicePlacement::default();
        assert_eq!(placement, DevicePlacement::Auto);
    }

    #[test]
    fn test_device_placement_requires_gpu() {
        assert!(!DevicePlacement::Cpu.requires_gpu());
        assert!(DevicePlacement::Cuda(0).requires_gpu());
        assert!(!DevicePlacement::Auto.requires_gpu());
    }

    #[test]
    fn test_device_placement_cuda_device_id() {
        assert_eq!(DevicePlacement::Cpu.cuda_device_id(), None);
        assert_eq!(DevicePlacement::Cuda(0).cuda_device_id(), Some(0));
        assert_eq!(DevicePlacement::Cuda(1).cuda_device_id(), Some(1));
        assert_eq!(DevicePlacement::Auto.cuda_device_id(), None);
    }

    #[test]
    fn test_device_placement_serde_roundtrip() {
        let placements = [
            DevicePlacement::Cpu,
            DevicePlacement::Cuda(0),
            DevicePlacement::Cuda(1),
            DevicePlacement::Auto,
        ];

        for placement in placements {
            let json = serde_json::to_string(&placement).unwrap();
            let restored: DevicePlacement = serde_json::from_str(&json).unwrap();
            assert_eq!(restored, placement);
        }
    }

    #[test]
    fn test_device_placement_is_copy() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<DevicePlacement>();
    }

    // =========================================================================
    // QUANTIZATION MODE TESTS (5 tests)
    // =========================================================================

    #[test]
    fn test_quantization_default_is_none() {
        let quant = QuantizationMode::default();
        assert_eq!(quant, QuantizationMode::None);
    }

    #[test]
    fn test_quantization_memory_multiplier() {
        assert_eq!(QuantizationMode::None.memory_multiplier(), 1.0);
        assert_eq!(QuantizationMode::Int8.memory_multiplier(), 0.25);
        assert_eq!(QuantizationMode::Fp16.memory_multiplier(), 0.5);
        assert_eq!(QuantizationMode::Bf16.memory_multiplier(), 0.5);
    }

    #[test]
    fn test_quantization_bytes_per_param() {
        assert_eq!(QuantizationMode::None.bytes_per_param(), 4);
        assert_eq!(QuantizationMode::Fp16.bytes_per_param(), 2);
        assert_eq!(QuantizationMode::Bf16.bytes_per_param(), 2);
        assert_eq!(QuantizationMode::Int8.bytes_per_param(), 1);
    }

    #[test]
    fn test_quantization_serde_roundtrip() {
        let modes = [
            QuantizationMode::None,
            QuantizationMode::Int8,
            QuantizationMode::Fp16,
            QuantizationMode::Bf16,
        ];

        for mode in modes {
            let json = serde_json::to_string(&mode).unwrap();
            let restored: QuantizationMode = serde_json::from_str(&json).unwrap();
            assert_eq!(restored, mode);
        }
    }

    #[test]
    fn test_quantization_is_copy() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<QuantizationMode>();
    }

    // =========================================================================
    // SINGLE MODEL CONFIG TESTS (6 tests)
    // =========================================================================

    #[test]
    fn test_single_model_config_default() {
        let config = SingleModelConfig::default();
        assert_eq!(config.device, DevicePlacement::Auto);
        assert_eq!(config.quantization, QuantizationMode::None);
        assert_eq!(config.max_batch_size, 32);
        assert!(config.use_flash_attention);
    }

    #[test]
    fn test_single_model_config_cpu() {
        let config = SingleModelConfig::cpu();
        assert_eq!(config.device, DevicePlacement::Cpu);
    }

    #[test]
    fn test_single_model_config_cuda() {
        let config = SingleModelConfig::cuda();
        assert_eq!(config.device, DevicePlacement::Cuda(0));
    }

    #[test]
    fn test_single_model_config_cuda_fp16() {
        let config = SingleModelConfig::cuda_fp16();
        assert_eq!(config.device, DevicePlacement::Cuda(0));
        assert_eq!(config.quantization, QuantizationMode::Fp16);
    }

    #[test]
    fn test_single_model_config_validate_success() {
        let config = SingleModelConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_single_model_config_validate_zero_batch_fails() {
        let config = SingleModelConfig {
            max_batch_size: 0,
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        match result {
            Err(EmbeddingError::ConfigError { message }) => {
                assert!(message.contains("max_batch_size"));
            }
            _ => panic!("Expected ConfigError"),
        }
    }

    // =========================================================================
    // FACTORY TRAIT TESTS (8 tests)
    // =========================================================================

    #[test]
    fn test_factory_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<TestFactory>();
    }

    #[test]
    fn test_factory_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<TestFactory>();
    }

    #[test]
    fn test_factory_supported_models_all_12() {
        let factory = TestFactory;
        let models = factory.supported_models();
        assert_eq!(models.len(), 12);

        // Verify all 12 models are present
        for model_id in ModelId::all() {
            assert!(
                models.contains(model_id),
                "Missing model: {:?}",
                model_id
            );
        }
    }

    #[test]
    fn test_factory_supports_model() {
        let factory = TestFactory;
        for model_id in ModelId::all() {
            assert!(
                factory.supports_model(*model_id),
                "Factory should support {:?}",
                model_id
            );
        }
    }

    #[test]
    fn test_factory_create_model_succeeds() {
        let factory = TestFactory;
        let config = SingleModelConfig::default();

        for model_id in ModelId::all() {
            let result = factory.create_model(*model_id, &config);
            assert!(
                result.is_ok(),
                "Failed to create {:?}: {:?}",
                model_id,
                result.err()
            );

            let model = result.unwrap();
            assert_eq!(model.model_id(), *model_id);
            assert!(!model.is_initialized()); // Not loaded yet
        }
    }

    #[test]
    fn test_factory_create_model_with_invalid_config_fails() {
        let factory = TestFactory;
        let config = SingleModelConfig {
            max_batch_size: 0,
            ..Default::default()
        };

        let result = factory.create_model(ModelId::Semantic, &config);
        assert!(result.is_err());
        match result {
            Err(EmbeddingError::ConfigError { .. }) => {}
            _ => panic!("Expected ConfigError"),
        }
    }

    #[test]
    fn test_factory_estimate_memory_nonzero() {
        let factory = TestFactory;

        for model_id in ModelId::all() {
            let estimate = factory.estimate_memory(*model_id);
            assert!(
                estimate > 0,
                "Memory estimate for {:?} should be > 0",
                model_id
            );
        }
    }

    #[test]
    fn test_factory_estimate_memory_quantized() {
        let factory = TestFactory;
        let model_id = ModelId::Semantic;

        let base = factory.estimate_memory(model_id);
        let fp16 = factory.estimate_memory_quantized(model_id, QuantizationMode::Fp16);
        let int8 = factory.estimate_memory_quantized(model_id, QuantizationMode::Int8);

        assert_eq!(fp16, (base as f32 * 0.5) as usize);
        assert_eq!(int8, (base as f32 * 0.25) as usize);
    }

    // =========================================================================
    // MEMORY ESTIMATE TESTS (4 tests)
    // =========================================================================

    #[test]
    fn test_memory_estimates_array_has_12_entries() {
        assert_eq!(MEMORY_ESTIMATES.len(), 12);
    }

    #[test]
    fn test_memory_estimates_all_nonzero() {
        for (model_id, memory) in MEMORY_ESTIMATES {
            assert!(
                memory > 0,
                "Memory for {:?} should be > 0",
                model_id
            );
        }
    }

    #[test]
    fn test_get_memory_estimate_finds_all_models() {
        for model_id in ModelId::all() {
            let estimate = get_memory_estimate(*model_id);
            assert!(
                estimate > 0,
                "get_memory_estimate({:?}) should return > 0",
                model_id
            );
        }
    }

    #[test]
    fn test_total_memory_estimate_matches_sum() {
        let sum: usize = MEMORY_ESTIMATES.iter().map(|(_, m)| m).sum();
        assert_eq!(TOTAL_MEMORY_ESTIMATE, sum);
    }

    // =========================================================================
    // OBJECT SAFETY TESTS (2 tests)
    // =========================================================================

    #[test]
    fn test_factory_trait_object_in_arc() {
        let factory: Arc<dyn ModelFactory> = Arc::new(TestFactory);
        assert!(factory.supports_model(ModelId::Semantic));
    }

    #[test]
    fn test_factory_trait_object_in_box() {
        let factory: Box<dyn ModelFactory> = Box::new(TestFactory);
        assert_eq!(factory.supported_models().len(), 12);
    }

    // =========================================================================
    // EDGE CASE TESTS (3 tests)
    // =========================================================================

    #[test]
    fn test_config_serde_roundtrip() {
        let config = SingleModelConfig {
            device: DevicePlacement::Cuda(1),
            quantization: QuantizationMode::Int8,
            max_batch_size: 64,
            use_flash_attention: false,
        };

        let json = serde_json::to_string(&config).unwrap();
        let restored: SingleModelConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.device, config.device);
        assert_eq!(restored.quantization, config.quantization);
        assert_eq!(restored.max_batch_size, config.max_batch_size);
        assert_eq!(restored.use_flash_attention, config.use_flash_attention);
    }

    #[test]
    fn test_memory_estimate_largest_model() {
        // Multimodal (CLIP) should be largest
        let multimodal = get_memory_estimate(ModelId::Multimodal);
        for model_id in ModelId::all() {
            if *model_id != ModelId::Multimodal {
                let other = get_memory_estimate(*model_id);
                assert!(
                    multimodal >= other,
                    "Multimodal should be >= {:?}",
                    model_id
                );
            }
        }
    }

    #[test]
    fn test_memory_estimate_smallest_models() {
        // Temporal models (15MB each) are the smallest
        let temporal_recent = get_memory_estimate(ModelId::TemporalRecent);
        let temporal_periodic = get_memory_estimate(ModelId::TemporalPeriodic);
        let temporal_positional = get_memory_estimate(ModelId::TemporalPositional);

        // All three temporal models have the same (smallest) size
        assert_eq!(temporal_recent, temporal_periodic);
        assert_eq!(temporal_periodic, temporal_positional);
        assert_eq!(temporal_recent, 15_000_000);

        // Verify they are smaller than or equal to all others
        for model_id in ModelId::all() {
            let other = get_memory_estimate(*model_id);
            assert!(
                temporal_recent <= other,
                "Temporal models ({}) should be <= {:?} ({})",
                temporal_recent,
                model_id,
                other
            );
        }
    }
}
