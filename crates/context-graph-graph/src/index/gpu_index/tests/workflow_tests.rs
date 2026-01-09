//! Full workflow integration tests.
//!
//! End-to-end tests for the complete GPU index workflow including
//! training, adding vectors, searching, and save/load operations.

#![allow(clippy::field_reassign_with_default)]

use crate::config::IndexConfig;
use crate::index::faiss_ffi::gpu_available;
use crate::index::gpu_index::{FaissGpuIndex, GpuResources};
use std::sync::Arc;

#[test]
fn test_search_trained_index() {
    // Check GPU availability BEFORE making FFI calls to prevent segfaults
    if !gpu_available() {
        println!("Skipping test: No GPU available (faiss_get_num_gpus() returned 0)");
        return;
    }

    // Use smaller nlist for testing: nlist=64 requires 64*256=16384 training vectors
    // This is much more manageable than the default 16384*256=4M vectors
    let mut config = IndexConfig::default();
    config.nlist = 64;
    config.min_train_vectors = 64 * 256; // 16384 vectors

    let resources = match GpuResources::new(0) {
        Ok(r) => Arc::new(r),
        Err(e) => panic!("GPU resources creation failed with GPU available: {}", e),
    };

    let mut index = match FaissGpuIndex::with_resources(config.clone(), resources) {
        Ok(idx) => idx,
        Err(e) => panic!("Index creation failed with GPU available: {}", e),
    };

    // Generate training data (16384 vectors of dimension 1536)
    let n_train = config.min_train_vectors;
    println!("Generating {} training vectors...", n_train);

    let training_data: Vec<f32> = (0..n_train)
        .flat_map(|i| {
            (0..config.dimension).map(move |d| {
                // Simple deterministic pattern
                ((i * config.dimension + d) as f32 * 0.001).sin()
            })
        })
        .collect();

    // Train the index
    println!("Training index (nlist={})...", config.nlist);
    let train_start = std::time::Instant::now();
    match index.train(&training_data) {
        Ok(()) => {
            let train_time = train_start.elapsed();
            println!("Training completed in {:?}", train_time);
        }
        Err(e) => panic!("Training failed: {}", e),
    }
    assert!(index.is_trained(), "Index should be trained");

    // Add some vectors
    let n_add = 1000;
    println!("Adding {} vectors...", n_add);

    let add_data: Vec<f32> = (0..n_add)
        .flat_map(|i| (0..config.dimension).map(move |d| ((i * 7 + d) as f32 * 0.001).cos()))
        .collect();
    let add_ids: Vec<i64> = (0..n_add as i64).collect();

    match index.add_with_ids(&add_data, &add_ids) {
        Ok(()) => println!("Added {} vectors", n_add),
        Err(e) => panic!("Add failed: {}", e),
    }
    assert_eq!(index.ntotal(), n_add);

    // Search
    println!("Searching for k=10 neighbors...");
    let query: Vec<f32> = (0..config.dimension)
        .map(|d| (d as f32 * 0.001).sin())
        .collect();

    let search_start = std::time::Instant::now();
    match index.search(&query, 10) {
        Ok((distances, indices)) => {
            let search_time = search_start.elapsed();
            println!("Search completed in {:?}", search_time);

            assert_eq!(distances.len(), 10);
            assert_eq!(indices.len(), 10);
            assert!(indices[0] >= 0, "First result should be valid");
            println!("Top result: idx={}, dist={:.4}", indices[0], distances[0]);
        }
        Err(e) => panic!("Search failed: {}", e),
    }
}

#[test]
fn test_full_index_workflow() {
    // Check GPU availability BEFORE making FFI calls to prevent segfaults
    if !gpu_available() {
        println!("Skipping test: No GPU available (faiss_get_num_gpus() returned 0)");
        return;
    }

    // Use smaller nlist for testing (256 instead of 16384)
    // This requires 256*256=65536 training vectors instead of 4M
    let mut config = IndexConfig::default();
    config.nlist = 256;
    config.min_train_vectors = 256 * 256; // 65536 vectors

    println!("Creating GPU resources...");
    let resources = match GpuResources::new(config.gpu_id) {
        Ok(r) => Arc::new(r),
        Err(e) => panic!("GPU resources creation failed with GPU available: {}", e),
    };

    println!("Creating index with factory: {}", config.factory_string());
    let mut index = match FaissGpuIndex::with_resources(config.clone(), resources) {
        Ok(idx) => idx,
        Err(e) => panic!("Index creation failed: {}", e),
    };

    // Generate training data
    println!(
        "Generating {} training vectors (dimension={})...",
        config.min_train_vectors, config.dimension
    );

    let training_data: Vec<f32> = (0..config.min_train_vectors)
        .flat_map(|i| {
            (0..config.dimension).map(move |d| ((i * config.dimension + d) as f32 * 0.0001).sin())
        })
        .collect();

    // Train
    println!("Training index...");
    let train_start = std::time::Instant::now();
    match index.train(&training_data) {
        Ok(()) => {
            let train_time = train_start.elapsed();
            println!("Training completed in {:?}", train_time);
        }
        Err(e) => panic!("Training failed: {}", e),
    }
    assert!(index.is_trained());

    // Add vectors
    let n_add = 10_000;
    println!("Adding {} vectors...", n_add);

    let add_data: Vec<f32> = (0..n_add)
        .flat_map(|i| (0..config.dimension).map(move |d| ((i * 7 + d) as f32 * 0.001).cos()))
        .collect();
    let add_ids: Vec<i64> = (0..n_add as i64).collect();

    let add_start = std::time::Instant::now();
    match index.add_with_ids(&add_data, &add_ids) {
        Ok(()) => {
            let add_time = add_start.elapsed();
            println!("Added {} vectors in {:?}", n_add, add_time);
        }
        Err(e) => panic!("Add failed: {}", e),
    }
    assert_eq!(index.ntotal(), n_add);

    // Search
    println!("Searching for k=10 neighbors...");
    let query: Vec<f32> = (0..config.dimension)
        .map(|d| (d as f32 * 0.001).sin())
        .collect();

    let search_start = std::time::Instant::now();
    match index.search(&query, 10) {
        Ok((distances, indices)) => {
            let search_time = search_start.elapsed();
            println!("Search completed in {:?}", search_time);
            println!("Top result: idx={}, dist={:.4}", indices[0], distances[0]);

            assert_eq!(distances.len(), 10);
            assert_eq!(indices.len(), 10);
            assert!(indices[0] >= 0, "First result should be valid");

            // Relaxed performance check for smaller dataset
            assert!(
                search_time.as_millis() < 500,
                "Search took too long: {:?}",
                search_time
            );
        }
        Err(e) => panic!("Search failed: {}", e),
    }
}

#[test]
fn test_save_load_roundtrip() {
    // Check GPU availability BEFORE making FFI calls to prevent segfaults
    if !gpu_available() {
        println!("Skipping test: No GPU available (faiss_get_num_gpus() returned 0)");
        return;
    }

    // Use smaller config for testing
    let mut config = IndexConfig::default();
    config.nlist = 64;
    config.min_train_vectors = 64 * 256; // 16384 vectors

    let resources = match GpuResources::new(config.gpu_id) {
        Ok(r) => Arc::new(r),
        Err(e) => panic!("GPU resources creation failed with GPU available: {}", e),
    };

    // Create and train index
    let mut index = match FaissGpuIndex::with_resources(config.clone(), resources.clone()) {
        Ok(idx) => idx,
        Err(e) => panic!("Index creation failed: {}", e),
    };

    let training_data: Vec<f32> = (0..config.min_train_vectors)
        .flat_map(|i| (0..config.dimension).map(move |d| ((i + d) as f32) * 0.001))
        .collect();

    match index.train(&training_data) {
        Ok(()) => println!("Training completed"),
        Err(e) => panic!("Training failed: {}", e),
    }

    // Add some vectors
    let vectors: Vec<f32> = (0..1000)
        .flat_map(|i| (0..config.dimension).map(move |d| (i + d) as f32 * 0.01))
        .collect();
    let ids: Vec<i64> = (0..1000).collect();

    match index.add_with_ids(&vectors, &ids) {
        Ok(()) => println!("Added 1000 vectors"),
        Err(e) => panic!("Add failed: {}", e),
    }

    // Save to temp file
    let temp_path = std::env::temp_dir().join(format!("test_index_{}.faiss", std::process::id()));

    match index.save(&temp_path) {
        Ok(()) => println!("Index saved to {:?}", temp_path),
        Err(e) => panic!("Save failed: {}", e),
    }

    // Load
    match FaissGpuIndex::load_with_resources(&temp_path, config, resources) {
        Ok(loaded) => {
            assert_eq!(loaded.ntotal(), index.ntotal());
            assert!(loaded.is_trained());
            println!("Index loaded with {} vectors", loaded.ntotal());
        }
        Err(e) => {
            // Clean up temp file on error
            let _ = std::fs::remove_file(&temp_path);
            panic!("Load failed: {}", e);
        }
    }

    // Clean up temp file
    let _ = std::fs::remove_file(&temp_path);
}
