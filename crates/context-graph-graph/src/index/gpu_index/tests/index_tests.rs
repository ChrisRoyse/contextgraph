//! Index creation and operation tests.
//!
//! Tests for index creation, configuration validation, training, and search.

#![allow(clippy::field_reassign_with_default)]

use crate::config::IndexConfig;
use crate::error::GraphError;
use crate::index::faiss_ffi::gpu_available;
use crate::index::gpu_index::{FaissGpuIndex, GpuResources};
use std::sync::Arc;

#[test]
fn test_index_creation_valid_config() {
    // Check GPU availability BEFORE making FFI calls to prevent segfaults
    if !gpu_available() {
        println!("Skipping test: No GPU available (faiss_get_num_gpus() returned 0)");
        return;
    }

    let config = IndexConfig::default();
    let resources = match GpuResources::new(config.gpu_id) {
        Ok(r) => Arc::new(r),
        Err(e) => {
            panic!("GPU resources creation failed with GPU available: {}", e);
        }
    };

    let result = FaissGpuIndex::with_resources(config.clone(), resources);

    match result {
        Ok(idx) => {
            assert_eq!(idx.dimension(), 1536);
            assert!(!idx.is_trained());
            assert!(idx.is_empty());
            assert_eq!(idx.config().nlist, 16384);
            println!(
                "Index created with factory: {}",
                idx.config().factory_string()
            );
        }
        Err(e) => panic!("Index creation failed: {}", e),
    }
}

#[test]
fn test_index_creation_invalid_dimension() {
    // Check GPU availability BEFORE making FFI calls to prevent segfaults
    if !gpu_available() {
        println!("Skipping test: No GPU available (faiss_get_num_gpus() returned 0)");
        return;
    }

    let mut config = IndexConfig::default();
    config.dimension = 0;

    let resources = match GpuResources::new(0) {
        Ok(r) => Arc::new(r),
        Err(e) => panic!("GPU resources creation failed with GPU available: {}", e),
    };

    let result = FaissGpuIndex::with_resources(config, resources);

    match result {
        Err(GraphError::InvalidConfig(msg)) => {
            assert!(msg.contains("dimension"));
            println!("Zero dimension correctly rejected: {}", msg);
        }
        _ => panic!("Expected InvalidConfig error for dimension=0"),
    }
}

#[test]
fn test_index_creation_invalid_pq_segments() {
    // Check GPU availability BEFORE making FFI calls to prevent segfaults
    if !gpu_available() {
        println!("Skipping test: No GPU available (faiss_get_num_gpus() returned 0)");
        return;
    }

    let mut config = IndexConfig::default();
    config.pq_segments = 7; // 1536 % 7 != 0

    let resources = match GpuResources::new(0) {
        Ok(r) => Arc::new(r),
        Err(e) => panic!("GPU resources creation failed with GPU available: {}", e),
    };

    let result = FaissGpuIndex::with_resources(config, resources);

    match result {
        Err(GraphError::InvalidConfig(msg)) => {
            assert!(msg.contains("pq_segments"));
            assert!(msg.contains("divide"));
            println!("Invalid pq_segments correctly rejected: {}", msg);
        }
        _ => panic!("Expected InvalidConfig error for pq_segments=7"),
    }
}

#[test]
fn test_index_creation_zero_nlist() {
    // Check GPU availability BEFORE making FFI calls to prevent segfaults
    if !gpu_available() {
        println!("Skipping test: No GPU available (faiss_get_num_gpus() returned 0)");
        return;
    }

    let mut config = IndexConfig::default();
    config.nlist = 0;

    let resources = match GpuResources::new(0) {
        Ok(r) => Arc::new(r),
        Err(e) => panic!("GPU resources creation failed with GPU available: {}", e),
    };

    let result = FaissGpuIndex::with_resources(config, resources);

    match result {
        Err(GraphError::InvalidConfig(msg)) => {
            assert!(msg.contains("nlist"));
            println!("Zero nlist correctly rejected: {}", msg);
        }
        _ => panic!("Expected InvalidConfig error for nlist=0"),
    }
}

#[test]
fn test_train_insufficient_data() {
    // Check GPU availability BEFORE making FFI calls to prevent segfaults
    if !gpu_available() {
        println!("Skipping test: No GPU available (faiss_get_num_gpus() returned 0)");
        return;
    }

    let config = IndexConfig::default();
    let resources = match GpuResources::new(0) {
        Ok(r) => Arc::new(r),
        Err(e) => panic!("GPU resources creation failed with GPU available: {}", e),
    };

    let mut index = match FaissGpuIndex::with_resources(config.clone(), resources) {
        Ok(idx) => idx,
        Err(e) => panic!("Index creation failed with GPU available: {}", e),
    };

    // Only 1000 vectors, need 4M+
    let vectors: Vec<f32> = vec![0.0; 1000 * config.dimension];
    let result = index.train(&vectors);

    match result {
        Err(GraphError::InsufficientTrainingData { required, provided }) => {
            assert_eq!(required, 4194304);
            assert_eq!(provided, 1000);
            println!("Insufficient training data correctly rejected");
        }
        _ => panic!("Expected InsufficientTrainingData error"),
    }
}

#[test]
fn test_train_dimension_mismatch() {
    // Check GPU availability BEFORE making FFI calls to prevent segfaults
    if !gpu_available() {
        println!("Skipping test: No GPU available (faiss_get_num_gpus() returned 0)");
        return;
    }

    let config = IndexConfig::default();
    let resources = match GpuResources::new(0) {
        Ok(r) => Arc::new(r),
        Err(e) => panic!("GPU resources creation failed with GPU available: {}", e),
    };

    let mut index = match FaissGpuIndex::with_resources(config, resources) {
        Ok(idx) => idx,
        Err(e) => panic!("Index creation failed with GPU available: {}", e),
    };

    // 1537 elements - not divisible by 1536
    let vectors: Vec<f32> = vec![0.0; 1537];
    let result = index.train(&vectors);

    match result {
        Err(GraphError::DimensionMismatch { expected, actual }) => {
            assert_eq!(expected, 1536);
            assert_eq!(actual, 1); // 1537 % 1536 = 1
            println!("Dimension mismatch correctly rejected");
        }
        _ => panic!("Expected DimensionMismatch error"),
    }
}

#[test]
fn test_add_without_training() {
    // Check GPU availability BEFORE making FFI calls to prevent segfaults
    if !gpu_available() {
        println!("Skipping test: No GPU available (faiss_get_num_gpus() returned 0)");
        return;
    }

    let config = IndexConfig::default();
    let resources = match GpuResources::new(0) {
        Ok(r) => Arc::new(r),
        Err(e) => panic!("GPU resources creation failed with GPU available: {}", e),
    };

    let mut index = match FaissGpuIndex::with_resources(config.clone(), resources) {
        Ok(idx) => idx,
        Err(e) => panic!("Index creation failed with GPU available: {}", e),
    };

    let vectors: Vec<f32> = vec![0.0; config.dimension];
    let ids: Vec<i64> = vec![0];
    let result = index.add_with_ids(&vectors, &ids);

    match result {
        Err(GraphError::IndexNotTrained) => {
            println!("Add without training correctly rejected");
        }
        _ => panic!("Expected IndexNotTrained error"),
    }
}

#[test]
fn test_search_without_training() {
    // Check GPU availability BEFORE making FFI calls to prevent segfaults
    if !gpu_available() {
        println!("Skipping test: No GPU available (faiss_get_num_gpus() returned 0)");
        return;
    }

    let config = IndexConfig::default();
    let resources = match GpuResources::new(0) {
        Ok(r) => Arc::new(r),
        Err(e) => panic!("GPU resources creation failed with GPU available: {}", e),
    };

    let index = match FaissGpuIndex::with_resources(config.clone(), resources) {
        Ok(idx) => idx,
        Err(e) => panic!("Index creation failed with GPU available: {}", e),
    };

    let queries: Vec<f32> = vec![0.0; config.dimension];
    let result = index.search(&queries, 10);

    match result {
        Err(GraphError::IndexNotTrained) => {
            println!("Search without training correctly rejected");
        }
        _ => panic!("Expected IndexNotTrained error"),
    }
}

#[test]
fn test_faiss_gpu_index_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<FaissGpuIndex>();
    println!("FaissGpuIndex is Send");
}
