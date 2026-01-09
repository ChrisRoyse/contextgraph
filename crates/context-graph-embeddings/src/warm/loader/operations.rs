//! Model loading operations for the warm model loader.
//!
//! Contains the core loading, allocation, and validation logic for individual models.
//!
//! # CRITICAL: Real CUDA Allocations Required
//!
//! Per Constitution AP-007, stub data in production is FORBIDDEN.
//! All VRAM allocations MUST use `WarmCudaAllocator::allocate_protected()`
//! for real cudaMalloc calls. Fake pointer patterns (0x7f80...) are DELETED.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::Instant;

use safetensors::SafeTensors;
use sha2::{Digest, Sha256};

use crate::types::ModelId;
use crate::warm::config::WarmConfig;
use crate::warm::cuda_alloc::WarmCudaAllocator;
use crate::warm::error::{WarmError, WarmResult};
use crate::warm::handle::ModelHandle;
use crate::warm::memory_pool::WarmMemoryPools;
use crate::warm::registry::SharedWarmRegistry;
use crate::warm::state::WarmModelState;
use crate::warm::validation::WarmValidator;

use super::helpers::format_bytes;

/// Load a single model into VRAM.
///
/// # CRITICAL: Real CUDA Allocation
///
/// This function uses `WarmCudaAllocator` for real cudaMalloc calls.
/// Fake pointers are FORBIDDEN per Constitution AP-007.
///
/// # Arguments
/// * `model_id` - Model identifier (e.g., "E1_Semantic")
/// * `config` - Warm loading configuration
/// * `registry` - Shared registry for state tracking
/// * `memory_pools` - Memory pools for accounting
/// * `cuda_allocator` - CUDA allocator for real GPU memory
/// * `validator` - Model validator
///
/// # Errors
/// - `WarmError::CudaAllocFailed` - CUDA allocation failed
/// - `WarmError::WeightFileMissing` - Weight file not found
/// - `WarmError::ModelValidationFailed` - Validation failed
pub fn load_single_model(
    model_id: &str,
    config: &WarmConfig,
    registry: &SharedWarmRegistry,
    memory_pools: &mut WarmMemoryPools,
    cuda_allocator: &mut WarmCudaAllocator,
    validator: &WarmValidator,
) -> WarmResult<()> {
    tracing::info!("Loading model: {}", model_id);

    let load_start = Instant::now();

    // Parse model ID string to ModelId enum for type-safe operations
    let model_enum = ModelId::try_from(model_id).map_err(|e| {
        tracing::error!("Failed to parse model ID '{}': {}", model_id, e);
        WarmError::ModelNotRegistered {
            model_id: model_id.to_string(),
        }
    })?;

    // Get model metadata from registry
    let (expected_bytes, expected_dimension) = {
        let reg = registry
            .read()
            .map_err(|_| WarmError::RegistryLockPoisoned)?;
        let entry = reg
            .get_entry(model_id)
            .ok_or_else(|| WarmError::ModelNotRegistered {
                model_id: model_id.to_string(),
            })?;
        (entry.expected_bytes, entry.expected_dimension)
    };

    // Check if this is a custom model (temporal, HDC) that computes embeddings mathematically
    // Custom models do NOT require weight files - they are algorithmic implementations
    if model_enum.is_custom() {
        tracing::info!(
            "Model {} is a custom implementation (no pretrained weights required)",
            model_id
        );
        return load_custom_model(
            model_id,
            expected_bytes,
            expected_dimension,
            config,
            registry,
            memory_pools,
            cuda_allocator,
            validator,
            load_start,
        );
    }

    // For pretrained models, proceed with weight loading

    // Transition: Pending -> Loading
    {
        let mut reg = registry
            .write()
            .map_err(|_| WarmError::RegistryLockPoisoned)?;
        reg.start_loading(model_id)?;
    }

    // Allocate REAL VRAM via cudaMalloc (non-evictable)
    let vram_ptr = allocate_model_vram(model_id, expected_bytes, memory_pools, cuda_allocator)?;

    // Update progress
    {
        let mut reg = registry
            .write()
            .map_err(|_| WarmError::RegistryLockPoisoned)?;
        reg.update_progress(model_id, 50, expected_bytes / 2)?;
    }

    // Load real model weights from SafeTensors file
    // Use ModelId::model_path() for correct directory structure: {base}/{subdir}/model.safetensors
    let weight_path = model_enum
        .model_path(&config.model_weights_path)
        .join("model.safetensors");
    tracing::debug!("Loading weights from: {:?}", weight_path);
    let (file_bytes, checksum_bytes, _metadata) = load_weights(&weight_path, model_id)?;

    // Convert [u8; 32] to u64 for handle (first 8 bytes as checksum identifier)
    let checksum = u64::from_le_bytes([
        checksum_bytes[0],
        checksum_bytes[1],
        checksum_bytes[2],
        checksum_bytes[3],
        checksum_bytes[4],
        checksum_bytes[5],
        checksum_bytes[6],
        checksum_bytes[7],
    ]);

    // Verify file size matches expected
    if file_bytes.len() != expected_bytes {
        tracing::warn!(
            "Weight file size mismatch for {}: expected {}, got {}",
            model_id,
            expected_bytes,
            file_bytes.len()
        );
    }

    // Update progress to 100%
    {
        let mut reg = registry
            .write()
            .map_err(|_| WarmError::RegistryLockPoisoned)?;
        reg.update_progress(model_id, 100, expected_bytes)?;
    }

    // Transition: Loading -> Validating
    {
        let mut reg = registry
            .write()
            .map_err(|_| WarmError::RegistryLockPoisoned)?;
        reg.mark_validating(model_id)?;
    }

    // Run validation (validates VRAM allocation is real and properly sized)
    validate_model(
        model_id,
        vram_ptr,
        expected_bytes,
        expected_dimension,
        config,
        validator,
    )?;

    // Create model handle
    let handle = ModelHandle::new(vram_ptr, expected_bytes, config.cuda_device_id, checksum);

    // Transition: Validating -> Warm
    {
        let mut reg = registry
            .write()
            .map_err(|_| WarmError::RegistryLockPoisoned)?;
        reg.mark_warm(model_id, handle)?;
    }

    let load_duration = load_start.elapsed();
    tracing::info!(
        "Model {} loaded successfully in {:?} ({} VRAM)",
        model_id,
        load_duration,
        format_bytes(expected_bytes)
    );

    Ok(())
}

/// Load a custom model (temporal, HDC) that computes embeddings algorithmically.
///
/// Custom models do NOT require pretrained weight files - they implement
/// mathematical transformations directly (exponential decay, Fourier basis, etc.).
///
/// # Arguments
/// * `model_id` - Model identifier (e.g., "E2_TemporalRecent")
/// * `expected_bytes` - Expected VRAM size for the model's computation buffers
/// * `expected_dimension` - Output dimension for validation
/// * `config` - Warm loading configuration
/// * `registry` - Shared registry for state tracking
/// * `memory_pools` - Memory pools for accounting
/// * `cuda_allocator` - CUDA allocator for GPU memory
/// * `validator` - Model validator
/// * `load_start` - Start time for duration tracking
///
/// # Returns
/// * `Ok(())` - Model loaded and marked as Warm
/// * `Err(WarmError)` - If allocation or validation fails
#[allow(clippy::too_many_arguments)]
fn load_custom_model(
    model_id: &str,
    expected_bytes: usize,
    expected_dimension: usize,
    config: &WarmConfig,
    registry: &SharedWarmRegistry,
    memory_pools: &mut WarmMemoryPools,
    cuda_allocator: &mut WarmCudaAllocator,
    validator: &WarmValidator,
    load_start: Instant,
) -> WarmResult<()> {
    // Transition: Pending -> Loading
    {
        let mut reg = registry
            .write()
            .map_err(|_| WarmError::RegistryLockPoisoned)?;
        reg.start_loading(model_id)?;
    }

    // Custom models still need VRAM for computation buffers
    // Allocate REAL VRAM via cudaMalloc (non-evictable)
    let vram_ptr = allocate_model_vram(model_id, expected_bytes, memory_pools, cuda_allocator)?;

    // Update progress - custom models load instantly (no weight file to read)
    {
        let mut reg = registry
            .write()
            .map_err(|_| WarmError::RegistryLockPoisoned)?;
        reg.update_progress(model_id, 100, expected_bytes)?;
    }

    // Transition: Loading -> Validating
    {
        let mut reg = registry
            .write()
            .map_err(|_| WarmError::RegistryLockPoisoned)?;
        reg.mark_validating(model_id)?;
    }

    // Run validation (validates VRAM allocation is real and properly sized)
    validate_model(
        model_id,
        vram_ptr,
        expected_bytes,
        expected_dimension,
        config,
        validator,
    )?;

    // Generate a deterministic checksum for custom models based on model_id
    // This ensures consistent checksums across runs without needing weight files
    let mut hasher = Sha256::new();
    hasher.update(model_id.as_bytes());
    hasher.update(b"custom_model_v1");
    let hash_result = hasher.finalize();
    let checksum = u64::from_le_bytes([
        hash_result[0],
        hash_result[1],
        hash_result[2],
        hash_result[3],
        hash_result[4],
        hash_result[5],
        hash_result[6],
        hash_result[7],
    ]);

    // Create model handle
    let handle = ModelHandle::new(vram_ptr, expected_bytes, config.cuda_device_id, checksum);

    // Transition: Validating -> Warm
    {
        let mut reg = registry
            .write()
            .map_err(|_| WarmError::RegistryLockPoisoned)?;
        reg.mark_warm(model_id, handle)?;
    }

    let load_duration = load_start.elapsed();
    tracing::info!(
        "Custom model {} loaded successfully in {:?} ({} VRAM) - no weight file required",
        model_id,
        load_duration,
        format_bytes(expected_bytes)
    );

    Ok(())
}

/// Allocate VRAM for a model from the model pool using CUDA.
///
/// # CRITICAL: Real cudaMalloc - NO FAKE POINTERS
///
/// This function uses real CUDA memory allocation via `WarmCudaAllocator`.
/// Fake pointers (0x7f80...) are FORBIDDEN per Constitution AP-007.
///
/// # Arguments
/// * `model_id` - Model identifier for error messages
/// * `size_bytes` - Size to allocate in bytes
/// * `memory_pools` - Pool tracking for accounting
/// * `cuda_allocator` - CUDA allocator for real GPU memory
///
/// # Returns
/// * `Ok(vram_ptr)` - Real CUDA device pointer from cudaMalloc
/// * `Err(WarmError)` - If allocation fails
///
/// # Errors
/// - `WarmError::VramAllocationFailed` - Pool capacity exceeded
/// - `WarmError::CudaAllocFailed` - CUDA allocation failed
pub fn allocate_model_vram(
    model_id: &str,
    size_bytes: usize,
    memory_pools: &mut WarmMemoryPools,
    cuda_allocator: &mut WarmCudaAllocator,
) -> WarmResult<u64> {
    // Check if we have enough space in the model pool (accounting check)
    if memory_pools.available_model_bytes() < size_bytes {
        tracing::error!(
            "[EMB-E008] Pool capacity exceeded for {}: {} bytes requested, {} bytes available",
            model_id,
            size_bytes,
            memory_pools.available_model_bytes()
        );
        return Err(WarmError::VramAllocationFailed {
            requested_bytes: size_bytes,
            available_bytes: memory_pools.available_model_bytes(),
            error: format!(
                "Model pool exhausted: {} bytes requested, {} bytes available",
                size_bytes,
                memory_pools.available_model_bytes()
            ),
        });
    }

    // Allocate REAL VRAM via cudaMalloc (non-evictable)
    // NO FAKE POINTERS - Constitution AP-007 forbids stub data in production
    let allocation = cuda_allocator.allocate_protected(size_bytes).map_err(|e| {
        tracing::error!(
            "[EMB-E008] CUDA allocation failed for {}: {} bytes - {}",
            model_id,
            size_bytes,
            e
        );
        e
    })?;

    let vram_ptr = allocation.ptr;

    // Verify allocation is valid (non-null pointer)
    if vram_ptr == 0 {
        tracing::error!(
            "[EMB-E008] CUDA returned null pointer for {}: {} bytes",
            model_id,
            size_bytes
        );
        return Err(WarmError::CudaAllocFailed {
            requested_bytes: size_bytes,
            cuda_error: "cudaMalloc returned null pointer".to_string(),
            vram_free: cuda_allocator.query_available_vram().ok(),
            allocation_history: cuda_allocator.allocation_history().to_vec(),
        });
    }

    // Record allocation in memory pool for accounting
    memory_pools.allocate_model(model_id, size_bytes, vram_ptr)?;

    tracing::info!(
        "Allocated {} for {} at 0x{:016x} (REAL cudaMalloc)",
        format_bytes(size_bytes),
        model_id,
        vram_ptr
    );

    Ok(vram_ptr)
}

/// Tensor metadata extracted from SafeTensors file.
#[derive(Debug, Clone)]
pub struct TensorMetadata {
    /// Map of tensor name to shape
    pub shapes: HashMap<String, Vec<usize>>,
    /// Data type of tensors
    pub dtype: safetensors::Dtype,
    /// Total parameter count
    pub total_params: usize,
}

/// Load model weights from SafeTensors file.
///
/// # CRITICAL: No Simulation
/// This function reads REAL bytes from REAL files.
/// Fake checksums (0xDEAD_BEEF...) are FORBIDDEN per Constitution AP-007.
///
/// # Arguments
/// * `weight_path` - Path to the SafeTensors file
/// * `model_id` - Model identifier for error messages
///
/// # Returns
/// * `Ok((file_bytes, checksum, metadata))` - Real data from real file
/// * `Err(WarmError)` - If file missing, corrupted, or parse fails
///
/// # Errors
/// - `WarmError::WeightFileMissing` - File not found at path
/// - `WarmError::WeightFileCorrupted` - Parse error or invalid format
pub fn load_weights(
    weight_path: &Path,
    model_id: &str,
) -> WarmResult<(Vec<u8>, [u8; 32], TensorMetadata)> {
    let start = Instant::now();

    // Step 1: Read actual file bytes
    let file_bytes = fs::read(weight_path).map_err(|e| {
        tracing::error!(
            "[EMB-E006] Weight file not found: {:?}, error: {}",
            weight_path,
            e
        );
        WarmError::WeightFileMissing {
            model_id: model_id.to_string(),
            path: weight_path.to_path_buf(),
        }
    })?;

    tracing::debug!(
        "Read {} bytes from {:?} for {}",
        file_bytes.len(),
        weight_path,
        model_id
    );

    // Step 2: Compute REAL SHA256 checksum
    let mut hasher = Sha256::new();
    hasher.update(&file_bytes);
    let checksum: [u8; 32] = hasher.finalize().into();

    tracing::debug!(
        "Computed SHA256 checksum for {}: {:02x}{:02x}{:02x}{:02x}...",
        model_id,
        checksum[0],
        checksum[1],
        checksum[2],
        checksum[3]
    );

    // Step 3: Parse SafeTensors to extract metadata
    let tensors = SafeTensors::deserialize(&file_bytes).map_err(|e| {
        tracing::error!(
            "[EMB-E004] SafeTensors parse failed for {}: {}",
            model_id,
            e
        );
        WarmError::WeightFileCorrupted {
            model_id: model_id.to_string(),
            path: weight_path.to_path_buf(),
            reason: format!("SafeTensors parse error: {}", e),
        }
    })?;

    // Step 4: Extract tensor metadata
    let mut shapes = HashMap::new();
    let mut total_params = 0usize;
    let mut dtype = safetensors::Dtype::F32;

    for (name, view) in tensors.tensors() {
        let shape: Vec<usize> = view.shape().to_vec();
        total_params += shape.iter().product::<usize>();
        dtype = view.dtype();
        shapes.insert(name.to_string(), shape);
    }

    let metadata = TensorMetadata {
        shapes,
        dtype,
        total_params,
    };

    let duration = start.elapsed();
    tracing::info!(
        "Loaded weights for {} in {:?}: {} params, {} bytes, checksum {:02x}{:02x}...",
        model_id,
        duration,
        total_params,
        file_bytes.len(),
        checksum[0],
        checksum[1]
    );

    Ok((file_bytes, checksum, metadata))
}

/// Verify checksum against expected value.
///
/// # Arguments
/// * `actual` - Computed SHA256 checksum (32 bytes)
/// * `expected` - Expected checksum (32 bytes)
/// * `model_id` - Model identifier for error messages
///
/// # Returns
/// * `Ok(())` - Checksums match
/// * `Err(WarmError::WeightChecksumMismatch)` - Checksums differ
pub fn verify_checksum(actual: &[u8; 32], expected: &[u8; 32], model_id: &str) -> WarmResult<()> {
    if actual != expected {
        let actual_hex = hex::encode(actual);
        let expected_hex = hex::encode(expected);
        tracing::error!(
            "[EMB-E004] Checksum mismatch for {}: expected {}, got {}",
            model_id,
            expected_hex,
            actual_hex
        );
        return Err(WarmError::WeightChecksumMismatch {
            model_id: model_id.to_string(),
            expected: expected_hex,
            actual: actual_hex,
        });
    }
    Ok(())
}

/// Validate a model after loading.
///
/// # Constitution AP-007 Compliance
///
/// NO FAKE DATA GENERATED. This function validates the VRAM allocation
/// is real and properly sized. Actual inference validation requires
/// LoadedModelWeights and the InferenceEngine, which happens at a later stage.
///
/// # Arguments
/// * `model_id` - Model identifier
/// * `vram_ptr` - VRAM pointer from cudaMalloc
/// * `allocation_bytes` - Size of VRAM allocation
/// * `expected_dimension` - Expected embedding dimension
/// * `config` - Warm configuration
/// * `_validator` - Warm validator (reserved for future inference validation)
///
/// # Returns
/// `WarmResult<()>` - Ok if validation passes
///
/// # Errors
/// - `WarmError::ModelValidationFailed` - If VRAM pointer is null or allocation too small
pub fn validate_model(
    model_id: &str,
    vram_ptr: u64,
    allocation_bytes: usize,
    expected_dimension: usize,
    config: &WarmConfig,
    _validator: &WarmValidator,
) -> WarmResult<()> {
    if !config.enable_test_inference {
        tracing::info!(
            target: "warm::validation",
            code = "EMB-I015",
            model_id = %model_id,
            "Skipping validation (disabled in config)"
        );
        return Ok(());
    }

    tracing::info!(
        target: "warm::validation",
        code = "EMB-I015",
        model_id = %model_id,
        expected_dimension = expected_dimension,
        vram_ptr = format!("0x{:016x}", vram_ptr),
        "Starting model validation"
    );

    // Validate VRAM pointer is non-null (real CUDA allocation required)
    // Constitution AP-007: NO FAKE DATA - real cudaMalloc must return non-null
    if vram_ptr == 0 {
        tracing::error!(
            target: "warm::validation",
            code = "EMB-E011",
            model_id = %model_id,
            "Model has null VRAM pointer - real CUDA allocation required"
        );
        return Err(WarmError::ModelValidationFailed {
            model_id: model_id.to_string(),
            reason: "Model has null VRAM pointer - real CUDA allocation required".to_string(),
            expected_output: Some("non-null VRAM pointer".to_string()),
            actual_output: Some("0x0000000000000000".to_string()),
        });
    }

    // Validate allocation size is reasonable for the expected dimension
    // Minimum: dimension * sizeof(f32) for at least one embedding vector
    let min_expected_bytes = expected_dimension * std::mem::size_of::<f32>();
    if allocation_bytes < min_expected_bytes {
        tracing::error!(
            target: "warm::validation",
            code = "EMB-E011",
            model_id = %model_id,
            expected_min = min_expected_bytes,
            actual = allocation_bytes,
            "Allocation size too small for expected dimension"
        );
        return Err(WarmError::ModelValidationFailed {
            model_id: model_id.to_string(),
            reason: format!(
                "Allocation size {} too small for dimension {} (min {} bytes)",
                allocation_bytes, expected_dimension, min_expected_bytes
            ),
            expected_output: Some(format!(">= {} bytes", min_expected_bytes)),
            actual_output: Some(format!("{} bytes", allocation_bytes)),
        });
    }

    tracing::info!(
        target: "warm::validation",
        code = "EMB-I015",
        model_id = %model_id,
        expected_dimension = expected_dimension,
        allocation_bytes = allocation_bytes,
        vram_ptr = format!("0x{:016x}", vram_ptr),
        "Model validation passed (VRAM allocation verified)"
    );

    Ok(())
}

/// Mark a model as failed in the registry.
pub fn mark_model_failed(
    model_id: &str,
    error: &WarmError,
    registry: &SharedWarmRegistry,
) -> WarmResult<()> {
    let mut reg = registry
        .write()
        .map_err(|_| WarmError::RegistryLockPoisoned)?;

    // Only mark failed if in Loading or Validating state
    let state = reg.get_state(model_id);
    if matches!(
        state,
        Some(WarmModelState::Loading { .. }) | Some(WarmModelState::Validating)
    ) {
        reg.mark_failed(model_id, error.exit_code() as u16, error.to_string())?;
    }

    Ok(())
}

/// Verify that all models are in Warm state.
pub fn verify_all_warm(
    loading_order: &[String],
    registry: &SharedWarmRegistry,
    total_model_count: usize,
) -> WarmResult<()> {
    let reg = registry
        .read()
        .map_err(|_| WarmError::RegistryLockPoisoned)?;

    if !reg.all_warm() {
        // Find the first non-warm model for error reporting
        for model_id in loading_order {
            let state = reg.get_state(model_id);
            match state {
                Some(WarmModelState::Warm) => continue,
                Some(WarmModelState::Failed {
                    error_code: _,
                    error_message,
                }) => {
                    return Err(WarmError::ModelLoadFailed {
                        model_id: model_id.clone(),
                        reason: error_message,
                        bytes_read: 0,
                        file_size: 0,
                    });
                }
                other => {
                    return Err(WarmError::ModelLoadFailed {
                        model_id: model_id.clone(),
                        reason: format!("Model in unexpected state: {:?}", other),
                        bytes_read: 0,
                        file_size: 0,
                    });
                }
            }
        }

        // Generic error if no specific model found
        return Err(WarmError::ModelValidationFailed {
            model_id: "unknown".to_string(),
            reason: "Not all models are warm after loading".to_string(),
            expected_output: Some(format!("{} models warm", total_model_count)),
            actual_output: Some(format!("{} models warm", reg.warm_count())),
        });
    }

    tracing::info!("All {} models verified warm", reg.warm_count());
    Ok(())
}
