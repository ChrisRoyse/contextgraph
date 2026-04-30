//! CUDA allocation wrappers for non-evictable VRAM allocations.
//!
//! # Critical Design Decision: cudaMalloc vs cudaMallocManaged
//!
//! This module ensures that model weights use `cudaMalloc` for VRAM allocation,
//! **NOT** `cudaMallocManaged` (Unified Virtual Memory / UVM).
//!
//! ## Why This Matters
//!
//! | Allocation Type | Eviction Behavior | Use Case |
//! |-----------------|-------------------|----------|
//! | `cudaMalloc` | Non-evictable, stays resident | Model weights (CRITICAL) |
//! | `cudaMallocManaged` (UVM) | Can be evicted to system RAM | General purpose |
//!
//! **Problem with UVM**: Under memory pressure, CUDA can transparently migrate
//! UVM allocations to system RAM. For inference workloads, this causes:
//! - **Severe latency spikes** (PCIe 5.0 is ~128GB/s vs GDDR7's 1.8TB/s)
//! - **Unpredictable performance** (page faults during inference)
//! - **Cascading failures** if multiple models get evicted simultaneously
//!
//! **Solution**: Use `cudaMalloc` for all model weights to guarantee they remain
//! resident in VRAM. Working memory (inference activations) can use the standard
//! Candle allocator since temporary eviction is acceptable.
//!
//! # Target Hardware: RTX 5090 (Blackwell) - REQUIRED
//!
//! - **Compute Capability**: 12.0 (required)
//! - **VRAM**: 32GB GDDR7
//! - **CUDA Version**: 13.2+
//! - **Memory Bandwidth**: 1.8 TB/s
//!
//! # CUDA Required - No Fallbacks
//!
//! This module requires CUDA support. There are NO fallback stubs.
//! If CUDA is unavailable, the system will fail fast at initialization.
//!
//! # Requirements Implemented
//!
//! - REQ-WARM-004: cudaMalloc not UVM (non-evictable allocations)
//! - REQ-WARM-010: CUDA init error handling

mod allocation;
mod allocator;
mod allocator_cuda;
// NOTE: allocator_stub.rs REMOVED - CUDA is REQUIRED (RTX 5090)
mod constants;
mod gpu_info;
mod helpers;

#[cfg(test)]
mod tests;

// Re-export all public types - CUDA is ALWAYS required
pub use allocation::VramAllocation;
pub use allocator::WarmCudaAllocator;
pub use constants::{
    FAKE_ALLOCATION_BASE_MASK, FAKE_ALLOCATION_BASE_PATTERN, GB, GOLDEN_SIMILARITY_THRESHOLD,
    MAX_ALLOCATION_HISTORY, MINIMUM_VRAM_BYTES, REQUIRED_COMPUTE_MAJOR, REQUIRED_COMPUTE_MINOR,
    SIN_WAVE_ENERGY_THRESHOLD,
};
pub use gpu_info::GpuInfo;
pub use helpers::format_bytes;
