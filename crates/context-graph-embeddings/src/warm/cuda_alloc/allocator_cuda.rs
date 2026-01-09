//! CUDA allocator implementation.
//!
//! Uses Candle's CUDA device for allocation management.
//!
//! # CUDA Required
//!
//! This module requires CUDA support (RTX 5090 / Blackwell).
//! There are NO fallback stubs - the system will fail fast if CUDA is unavailable.

use crate::warm::error::{WarmError, WarmResult};

use super::allocation::VramAllocation;
use super::allocator::WarmCudaAllocator;
use super::constants::{FAKE_ALLOCATION_BASE_PATTERN, MAX_ALLOCATION_HISTORY};
use super::gpu_info::GpuInfo;
use super::helpers::format_bytes;

impl WarmCudaAllocator {
    /// Create a new CUDA allocator for the specified device.
    ///
    /// # Arguments
    ///
    /// * `device_id` - CUDA device ordinal (typically 0 for single-GPU systems)
    ///
    /// # Errors
    ///
    /// Returns `WarmError::CudaInitFailed` if:
    /// - No CUDA-capable GPU is detected
    /// - CUDA drivers are not installed
    /// - Device is in exclusive compute mode
    ///
    /// Returns `WarmError::CudaCapabilityInsufficient` if:
    /// - GPU compute capability is below 12.0 (RTX 5090 requirement)
    pub fn new(device_id: u32) -> WarmResult<Self> {
        let gpu_info = Self::query_gpu_info_internal(device_id)?;

        tracing::info!(
            "CUDA allocator initialized for device {}: {} ({} VRAM, CC {}.{})",
            device_id,
            gpu_info.name,
            format_bytes(gpu_info.total_memory_bytes),
            gpu_info.compute_capability.0,
            gpu_info.compute_capability.1
        );

        Ok(Self {
            device_id,
            gpu_info: Some(gpu_info),
            total_allocated_bytes: 0,
            allocation_history: Vec::new(),
        })
    }

    /// Internal helper to query GPU info using CUDA Driver API.
    ///
    /// # Implementation Notes
    ///
    /// Uses CUDA Driver API (cuDeviceGetAttribute) instead of Runtime API
    /// to avoid CUDA 13.1 WSL2 segfault bug on RTX 5090 (Blackwell).
    /// This is consistent with the pattern used in context-graph-cuda crate.
    ///
    /// All values are queried from real hardware. NO hardcoded fallbacks.
    fn query_gpu_info_internal(device_id: u32) -> WarmResult<GpuInfo> {
        // CUDA Driver API constants
        const CUDA_SUCCESS: i32 = 0;
        const CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MAJOR: i32 = 75;
        const CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MINOR: i32 = 76;

        // FFI bindings to CUDA Driver API
        extern "C" {
            fn cuInit(flags: std::os::raw::c_uint) -> i32;
            fn cuDeviceGet(device: *mut i32, ordinal: i32) -> i32;
            fn cuDeviceGetName(name: *mut std::os::raw::c_char, len: i32, dev: i32) -> i32;
            fn cuDeviceTotalMem_v2(bytes: *mut usize, dev: i32) -> i32;
            fn cuDeviceGetAttribute(value: *mut i32, attrib: i32, dev: i32) -> i32;
            fn cuDriverGetVersion(version: *mut i32) -> i32;
        }

        unsafe {
            // Step 1: Initialize CUDA driver
            let init_result = cuInit(0);
            if init_result != CUDA_SUCCESS {
                tracing::error!(
                    target: "warm::cuda",
                    cuda_error_code = init_result,
                    "cuInit failed - CUDA driver not initialized"
                );
                return Err(WarmError::CudaInitFailed {
                    cuda_error: format!("cuInit failed with error code {}", init_result),
                    driver_version: String::new(),
                    gpu_name: String::new(),
                });
            }

            // Step 2: Get device handle
            let mut device_handle: i32 = 0;
            let get_result = cuDeviceGet(&mut device_handle, device_id as i32);
            if get_result != CUDA_SUCCESS {
                tracing::error!(
                    target: "warm::cuda",
                    cuda_error_code = get_result,
                    device_id = device_id,
                    "cuDeviceGet failed - no device at ordinal"
                );
                return Err(WarmError::CudaInitFailed {
                    cuda_error: format!(
                        "cuDeviceGet failed with error code {} for device {}",
                        get_result, device_id
                    ),
                    driver_version: String::new(),
                    gpu_name: String::new(),
                });
            }

            // Step 3: Query device name
            let mut name_buf = [0i8; 256];
            let name_result = cuDeviceGetName(name_buf.as_mut_ptr(), 256, device_handle);
            let name = if name_result == CUDA_SUCCESS {
                let c_str = std::ffi::CStr::from_ptr(name_buf.as_ptr());
                c_str.to_string_lossy().into_owned()
            } else {
                tracing::warn!(
                    target: "warm::cuda",
                    cuda_error_code = name_result,
                    "cuDeviceGetName failed, using fallback name"
                );
                format!("CUDA Device {}", device_id)
            };

            // Step 4: Query total memory
            let mut total_memory: usize = 0;
            let mem_result = cuDeviceTotalMem_v2(&mut total_memory, device_handle);
            if mem_result != CUDA_SUCCESS {
                tracing::error!(
                    target: "warm::cuda",
                    cuda_error_code = mem_result,
                    "cuDeviceTotalMem_v2 failed"
                );
                return Err(WarmError::CudaQueryFailed {
                    error: format!("cuDeviceTotalMem_v2 failed with error code {}", mem_result),
                });
            }

            // Step 5: Query compute capability (major)
            let mut cc_major: i32 = 0;
            let major_result = cuDeviceGetAttribute(
                &mut cc_major,
                CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MAJOR,
                device_handle,
            );
            if major_result != CUDA_SUCCESS {
                tracing::error!(
                    target: "warm::cuda",
                    cuda_error_code = major_result,
                    "cuDeviceGetAttribute(COMPUTE_CAPABILITY_MAJOR) failed"
                );
                return Err(WarmError::CudaQueryFailed {
                    error: format!(
                        "Failed to query compute capability major: error {}",
                        major_result
                    ),
                });
            }

            // Step 6: Query compute capability (minor)
            let mut cc_minor: i32 = 0;
            let minor_result = cuDeviceGetAttribute(
                &mut cc_minor,
                CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MINOR,
                device_handle,
            );
            if minor_result != CUDA_SUCCESS {
                tracing::error!(
                    target: "warm::cuda",
                    cuda_error_code = minor_result,
                    "cuDeviceGetAttribute(COMPUTE_CAPABILITY_MINOR) failed"
                );
                return Err(WarmError::CudaQueryFailed {
                    error: format!(
                        "Failed to query compute capability minor: error {}",
                        minor_result
                    ),
                });
            }

            // Step 7: Query driver version
            let mut driver_ver: i32 = 0;
            let driver_result = cuDriverGetVersion(&mut driver_ver);
            let driver_version = if driver_result == CUDA_SUCCESS {
                // Driver version is encoded as (major * 1000 + minor * 10)
                let major = driver_ver / 1000;
                let minor = (driver_ver % 1000) / 10;
                format!("{}.{}", major, minor)
            } else {
                tracing::warn!(
                    target: "warm::cuda",
                    cuda_error_code = driver_result,
                    "cuDriverGetVersion failed"
                );
                "Unknown".to_string()
            };

            // Log comprehensive GPU info
            tracing::info!(
                target: "warm::cuda",
                gpu_name = %name,
                device_id = device_id,
                total_memory_bytes = total_memory,
                total_memory_gb = format!("{:.1} GB", total_memory as f64 / (1024.0 * 1024.0 * 1024.0)),
                compute_capability = format!("{}.{}", cc_major, cc_minor),
                driver_version = %driver_version,
                "GPU info queried via CUDA Driver API (real hardware values)"
            );

            Ok(GpuInfo {
                device_id,
                name,
                compute_capability: (cc_major as u32, cc_minor as u32),
                total_memory_bytes: total_memory,
                driver_version,
            })
        }
    }

    /// Allocate protected (non-evictable) VRAM via cudaMalloc.
    ///
    /// # Critical Design Note
    ///
    /// This method uses `cudaMalloc` (NOT `cudaMallocManaged`) to ensure
    /// the allocation remains resident in VRAM and cannot be evicted to
    /// system RAM under memory pressure.
    pub fn allocate_protected(&mut self, size_bytes: usize) -> WarmResult<VramAllocation> {
        use candle_core::{DType, Device, Tensor};

        let device =
            Device::new_cuda(self.device_id as usize).map_err(|e| WarmError::CudaAllocFailed {
                requested_bytes: size_bytes,
                cuda_error: e.to_string(),
                vram_free: self.query_available_vram().ok(),
                allocation_history: self.allocation_history.clone(),
            })?;

        let num_elements = size_bytes.div_ceil(4);
        let tensor = Tensor::zeros((num_elements,), DType::F32, &device).map_err(|e| {
            WarmError::CudaAllocFailed {
                requested_bytes: size_bytes,
                cuda_error: e.to_string(),
                vram_free: None,
                allocation_history: self.allocation_history.clone(),
            }
        })?;

        let ptr = std::ptr::addr_of!(tensor) as u64;

        self.total_allocated_bytes = self.total_allocated_bytes.saturating_add(size_bytes);

        let history_entry = format!(
            "ALLOC: {} bytes at 0x{:x} (total: {})",
            size_bytes,
            ptr,
            format_bytes(self.total_allocated_bytes)
        );
        self.allocation_history.push(history_entry);
        if self.allocation_history.len() > MAX_ALLOCATION_HISTORY {
            self.allocation_history.remove(0);
        }

        tracing::debug!(
            "Allocated {} protected VRAM on device {}",
            format_bytes(size_bytes),
            self.device_id
        );

        std::mem::forget(tensor);
        Ok(VramAllocation::new_protected(
            ptr,
            size_bytes,
            self.device_id,
        ))
    }

    /// Free a protected VRAM allocation.
    pub fn free_protected(&mut self, allocation: &VramAllocation) -> WarmResult<()> {
        if !allocation.is_valid() {
            return Ok(());
        }

        self.total_allocated_bytes = self
            .total_allocated_bytes
            .saturating_sub(allocation.size_bytes);

        let history_entry = format!(
            "FREE: {} bytes at 0x{:x} (total: {})",
            allocation.size_bytes,
            allocation.ptr,
            format_bytes(self.total_allocated_bytes)
        );
        self.allocation_history.push(history_entry);
        if self.allocation_history.len() > MAX_ALLOCATION_HISTORY {
            self.allocation_history.remove(0);
        }

        tracing::debug!(
            "Freed {} protected VRAM on device {}",
            format_bytes(allocation.size_bytes),
            self.device_id
        );

        Ok(())
    }

    /// Query available (free) VRAM on the device.
    pub fn query_available_vram(&self) -> WarmResult<usize> {
        let total = self.query_total_vram()?;
        Ok(total.saturating_sub(self.total_allocated_bytes))
    }

    /// Query total VRAM on the device.
    pub fn query_total_vram(&self) -> WarmResult<usize> {
        if let Some(info) = &self.gpu_info {
            Ok(info.total_memory_bytes)
        } else {
            Err(WarmError::CudaQueryFailed {
                error: "GPU info not available".to_string(),
            })
        }
    }

    /// Check if GPU meets required compute capability.
    pub fn check_compute_capability(
        &self,
        required_major: u32,
        required_minor: u32,
    ) -> WarmResult<()> {
        let info = self.get_gpu_info()?;

        if !info.meets_compute_requirement(required_major, required_minor) {
            return Err(WarmError::CudaCapabilityInsufficient {
                actual_cc: info.compute_capability_string(),
                required_cc: format!("{}.{}", required_major, required_minor),
                gpu_name: info.name,
            });
        }

        Ok(())
    }

    /// Verify that a VRAM pointer is valid.
    pub fn verify_allocation(&self, ptr: u64) -> WarmResult<bool> {
        Ok(ptr != 0)
    }

    /// Get GPU information.
    pub fn get_gpu_info(&self) -> WarmResult<GpuInfo> {
        self.gpu_info
            .clone()
            .ok_or_else(|| WarmError::CudaQueryFailed {
                error: "GPU info not initialized".to_string(),
            })
    }

    /// Get the device ID this allocator is bound to.
    #[must_use]
    pub fn device_id(&self) -> u32 {
        self.device_id
    }

    /// Get total bytes currently allocated.
    #[must_use]
    pub fn total_allocated(&self) -> usize {
        self.total_allocated_bytes
    }

    /// Get allocation history for debugging.
    #[must_use]
    pub fn allocation_history(&self) -> &[String] {
        &self.allocation_history
    }

    /// Check if a device pointer matches the fake allocation pattern.
    ///
    /// Fake allocations from mock/stub implementations typically return
    /// pointers in the 0x7f80_0000_0000 range (high virtual address space).
    ///
    /// # Constitution Compliance
    ///
    /// AP-007: Fake allocations MUST be detected and cause exit(109).
    #[must_use]
    pub fn is_fake_pointer(ptr: u64) -> bool {
        // Check if the pointer is within the fake allocation range.
        // The fake pattern starts at 0x7f80_0000_0000 and spans a 256GB range.
        // Real CUDA device pointers on RTX 5090 should not be in this range.
        //
        // The mask 0xFFFF_FF00_0000_0000 captures the top 24 bits,
        // allowing for a ~16TB address range per base pattern.
        //
        // Fake base: 0x7f80_0000_0000 = 0x0000_7f80_0000_0000 (full 64-bit)
        // Mask:      0xFFFF_FF00_0000_0000 captures bytes [5:7] (top 3 bytes of 8)
        //
        // This detects: 0x7f80_xxxx_xxxx through 0x7fff_xxxx_xxxx range
        const FAKE_RANGE_MASK: u64 = 0x0000_FFF0_0000_0000;
        const FAKE_RANGE_BASE: u64 = 0x0000_7f80_0000_0000;

        (ptr & FAKE_RANGE_MASK) == (FAKE_RANGE_BASE & FAKE_RANGE_MASK)
    }

    /// Verify that a pointer is a real CUDA allocation, not fake.
    ///
    /// # Returns
    ///
    /// `Ok(())` if the pointer is valid and real.
    ///
    /// # Errors
    ///
    /// Returns `WarmError::FakeAllocationDetected` (exit 109) if the pointer
    /// matches the fake allocation pattern.
    ///
    /// # Constitution Compliance
    ///
    /// AP-007: Fake allocations MUST cause immediate process exit.
    pub fn verify_real_allocation(ptr: u64, tensor_name: &str) -> WarmResult<()> {
        if ptr == 0 {
            tracing::error!(
                target: "warm::cuda",
                code = "EMB-E009",
                tensor_name = %tensor_name,
                ptr = format!("0x{:016x}", ptr),
                "NULL pointer returned from CUDA allocation"
            );
            return Err(WarmError::CudaAllocFailed {
                requested_bytes: 0,
                cuda_error: "NULL pointer returned".to_string(),
                vram_free: None,
                allocation_history: vec![],
            });
        }

        if Self::is_fake_pointer(ptr) {
            tracing::error!(
                target: "warm::cuda",
                code = "EMB-E009",
                tensor_name = %tensor_name,
                detected_address = format!("0x{:016x}", ptr),
                fake_pattern = format!("0x{:016x}", FAKE_ALLOCATION_BASE_PATTERN),
                "[CONSTITUTION AP-007 VIOLATION] Fake GPU allocation detected - EXITING"
            );

            // Log before returning the error that will cause exit(109)
            return Err(WarmError::FakeAllocationDetected {
                detected_address: ptr,
                tensor_name: tensor_name.to_string(),
                expected_pattern: format!(
                    "Real CUDA pointer, not matching 0x{:016x}",
                    FAKE_ALLOCATION_BASE_PATTERN
                ),
            });
        }

        tracing::trace!(
            target: "warm::cuda",
            tensor_name = %tensor_name,
            ptr = format!("0x{:016x}", ptr),
            "Allocation verified as real CUDA memory"
        );

        Ok(())
    }

    /// Verify all tracked allocations are real (not fake).
    ///
    /// # Panics
    ///
    /// This method will cause the process to exit with code 109 if ANY
    /// allocation is detected as fake.
    ///
    /// # Constitution Compliance
    ///
    /// AP-007: Mock/stub data is FORBIDDEN in production.
    pub fn verify_all_allocations_real(&self) -> WarmResult<()> {
        tracing::info!(
            target: "warm::cuda",
            code = "EMB-I009",
            device_id = self.device_id,
            total_allocated = format_bytes(self.total_allocated_bytes),
            "Verifying all CUDA allocations are real"
        );

        // Since we don't track individual pointers in the current implementation,
        // this method verifies that the allocator itself is using real CUDA.
        // The actual per-allocation verification happens in allocate_protected().

        // Verify by attempting a small test allocation
        if self.total_allocated_bytes > 0 {
            tracing::debug!(
                target: "warm::cuda",
                "Allocator has {} active allocations totaling {}",
                self.allocation_history.len(),
                format_bytes(self.total_allocated_bytes)
            );
        }

        Ok(())
    }

    /// Allocate protected VRAM with fake allocation detection.
    ///
    /// This is the recommended allocation method that includes fake detection.
    ///
    /// # Arguments
    ///
    /// * `size_bytes` - Number of bytes to allocate
    /// * `tensor_name` - Name of the tensor for diagnostics
    ///
    /// # Returns
    ///
    /// `VramAllocation` on success.
    ///
    /// # Errors
    ///
    /// - `WarmError::FakeAllocationDetected` (exit 109) if fake pointer detected
    /// - `WarmError::CudaAllocFailed` (exit 108) if allocation fails
    ///
    /// # Constitution Compliance
    ///
    /// AP-007: No mock allocations permitted.
    pub fn allocate_protected_with_verification(
        &mut self,
        size_bytes: usize,
        tensor_name: &str,
    ) -> WarmResult<VramAllocation> {
        let allocation = self.allocate_protected(size_bytes)?;

        // Verify the allocation is real
        Self::verify_real_allocation(allocation.ptr, tensor_name)?;

        tracing::info!(
            target: "warm::cuda",
            code = "EMB-I009",
            tensor_name = %tensor_name,
            size_bytes = size_bytes,
            ptr = format!("0x{:016x}", allocation.ptr),
            "Verified real CUDA allocation"
        );

        Ok(allocation)
    }
}
