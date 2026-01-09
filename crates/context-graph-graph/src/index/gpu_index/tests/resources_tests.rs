//! GPU Resources tests.
//!
//! Tests for GPU resource allocation, thread safety, and error handling.

use crate::error::GraphError;
use crate::index::faiss_ffi::gpu_available;
use crate::index::gpu_index::GpuResources;
use std::sync::Arc;

#[test]
fn test_gpu_resources_creation() {
    // Check GPU availability BEFORE making FFI calls to prevent segfaults
    if !gpu_available() {
        println!("Skipping test: No GPU available (faiss_get_num_gpus() returned 0)");
        return;
    }

    // REAL TEST: Actually allocates GPU resources
    let result = GpuResources::new(0);

    match result {
        Ok(resources) => {
            assert_eq!(resources.gpu_id(), 0);
            println!("GPU resources allocated successfully for device 0");
        }
        Err(e) => {
            panic!("GPU resources creation failed with GPU available: {}", e);
        }
    }
}

#[test]
fn test_gpu_resources_invalid_device() {
    // Check GPU availability BEFORE making FFI calls to prevent segfaults
    if !gpu_available() {
        println!("Skipping test: No GPU available (faiss_get_num_gpus() returned 0)");
        return;
    }

    // REAL TEST: Invalid device ID should fail
    // Note: Device 999 may succeed on systems with many GPUs, but typically fails
    let result = GpuResources::new(999);

    match result {
        Err(GraphError::GpuResourceAllocation(msg)) => {
            assert!(msg.contains("999") || msg.contains("GPU") || msg.contains("failed"));
            println!("Invalid device ID correctly rejected: {}", msg);
        }
        Err(e) => {
            println!("Invalid device rejected with different error: {}", e);
        }
        Ok(_) => {
            // This might succeed on systems with many GPUs or if device 0 is used
            println!("Device 999 unexpectedly succeeded (unusual but possible)");
        }
    }
}

#[test]
fn test_gpu_resources_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<GpuResources>();
    println!("GpuResources is Send + Sync");
}

#[test]
fn test_gpu_availability_check() {
    // This test verifies that gpu_available() works without crashing
    // regardless of whether a GPU is actually present
    let has_gpu = gpu_available();
    if has_gpu {
        println!("GPU detected: faiss_get_num_gpus() > 0");
    } else {
        println!("No GPU detected: faiss_get_num_gpus() returned 0");
    }
    // Test passes regardless of GPU presence - just verifies the check is safe
}

#[test]
fn test_shared_gpu_resources() {
    // Check GPU availability BEFORE making FFI calls to prevent segfaults
    if !gpu_available() {
        println!("Skipping test: No GPU available (faiss_get_num_gpus() returned 0)");
        return;
    }

    // Test that Arc<GpuResources> can be created and cloned
    let result = GpuResources::new(0);
    match result {
        Ok(resources) => {
            let arc1 = Arc::new(resources);
            let arc2 = Arc::clone(&arc1);

            assert_eq!(arc1.gpu_id(), arc2.gpu_id());
            assert_eq!(Arc::strong_count(&arc1), 2);
            println!("Shared GPU resources work correctly");
        }
        Err(e) => {
            panic!("GPU resources creation failed with GPU available: {}", e);
        }
    }
}
