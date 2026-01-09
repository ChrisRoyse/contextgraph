//! GPU VRAM handle for warm-loaded model weights.
//!
//! # Safety
//!
//! The `vram_base_ptr` contains a raw GPU device pointer from `cudaMalloc`:
//! - Only valid on the CUDA device identified by `device_ordinal`
//! - Invalid after `cudaFree` or CUDA context destruction
//! - Cannot be dereferenced from host code
//!
//! Not `Clone`/`Copy` to prevent VRAM ownership duplication.

use std::time::{Duration, Instant};

/// Protected handle for VRAM-resident model weights.
///
/// Tracks GPU memory allocation for a warm-loaded embedding model.
/// The `vram_base_ptr` remains valid only while the CUDA context is active.
#[derive(Debug)]
pub struct ModelHandle {
    /// GPU device pointer from cudaMalloc (valid only in CUDA context).
    vram_base_ptr: u64,
    /// Total bytes allocated for model weights.
    allocation_bytes: usize,
    /// CUDA device ordinal (0-indexed GPU ID).
    device_ordinal: u32,
    /// Timestamp when allocation was created.
    allocated_at: Instant,
    /// SHA256 checksum of weights, truncated to 64 bits.
    weight_checksum: u64,
}

impl ModelHandle {
    /// Create a new VRAM handle for a model allocation.
    #[must_use]
    pub fn new(vram_ptr: u64, bytes: usize, device: u32, checksum: u64) -> Self {
        Self {
            vram_base_ptr: vram_ptr,
            allocation_bytes: bytes,
            device_ordinal: device,
            allocated_at: Instant::now(),
            weight_checksum: checksum,
        }
    }

    /// Get the raw VRAM device pointer. Only valid within the CUDA context.
    #[must_use]
    pub fn vram_address(&self) -> u64 {
        self.vram_base_ptr
    }

    /// Get the total bytes allocated in VRAM.
    #[must_use]
    pub fn allocation_bytes(&self) -> usize {
        self.allocation_bytes
    }

    /// Get the CUDA device ordinal for this allocation.
    #[must_use]
    pub fn device_ordinal(&self) -> u32 {
        self.device_ordinal
    }

    /// Get the weight checksum (SHA256 truncated to 64 bits).
    #[must_use]
    pub fn weight_checksum(&self) -> u64 {
        self.weight_checksum
    }

    /// Get the duration since this allocation was created.
    #[must_use]
    pub fn uptime(&self) -> Duration {
        self.allocated_at.elapsed()
    }

    /// Format VRAM address as hex for health check (e.g., "0x00007f8a00000000").
    #[must_use]
    pub fn vram_address_hex(&self) -> String {
        format!("0x{:016x}", self.vram_base_ptr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_and_accessors() {
        let h = ModelHandle::new(
            0x7f8a_0000_0000,
            512 * 1024 * 1024,
            0,
            0xdead_beef_cafe_babe,
        );
        assert_eq!(h.vram_address(), 0x7f8a_0000_0000);
        assert_eq!(h.allocation_bytes(), 512 * 1024 * 1024);
        assert_eq!(h.device_ordinal(), 0);
        assert_eq!(h.weight_checksum(), 0xdead_beef_cafe_babe);
    }

    #[test]
    fn test_vram_address_hex_formatting() {
        assert_eq!(
            ModelHandle::new(0x7f8a_0000_0000, 1024, 0, 0).vram_address_hex(),
            "0x00007f8a00000000"
        );
        assert_eq!(
            ModelHandle::new(u64::MAX, 1024, 0, 0).vram_address_hex(),
            "0xffffffffffffffff"
        );
        assert_eq!(
            ModelHandle::new(0, 1024, 0, 0).vram_address_hex(),
            "0x0000000000000000"
        );
    }

    #[test]
    fn test_uptime_increases() {
        let h = ModelHandle::new(0x1000, 1024, 0, 0);
        let t1 = h.uptime();
        std::thread::sleep(Duration::from_millis(10));
        assert!(h.uptime() > t1);
    }

    #[test]
    fn test_different_devices() {
        let h0 = ModelHandle::new(0x1000, 1024, 0, 0x1111);
        let h1 = ModelHandle::new(0x2000, 2048, 1, 0x2222);
        assert_eq!(h0.device_ordinal(), 0);
        assert_eq!(h1.device_ordinal(), 1);
        assert_ne!(h0.weight_checksum(), h1.weight_checksum());
    }
}
