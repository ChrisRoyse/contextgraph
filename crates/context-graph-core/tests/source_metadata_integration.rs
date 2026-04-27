//! Integration test for source metadata feature.
//!
//! This test verifies that:
//! 1. MemoryCaptureService with TeleologicalMemoryStore stores source metadata
//! 2. Source metadata is retrievable via TeleologicalMemoryStore
//! 3. The full capture -> store -> retrieve flow works

use std::sync::Arc;
use tempfile::TempDir;

use context_graph_core::memory::capture::{
    EmbeddingProvider, MemoryCaptureService, TestEmbeddingProvider,
};
use context_graph_core::memory::store::MemoryStore;
use context_graph_core::memory::{ChunkMetadata, HookType, TextChunk};
use context_graph_core::stubs::InMemoryTeleologicalStore;
use context_graph_core::traits::TeleologicalMemoryStore;
use context_graph_core::types::SourceType;

/// Test that MDFileChunk captures store source metadata in TeleologicalMemoryStore.
#[tokio::test]
async fn test_md_chunk_stores_source_metadata() {
    println!("\n=== FSV: Source Metadata Storage Test ===\n");

    // Setup: Create stores and capture service
    let db_dir = TempDir::new().expect("create temp dir");
    let memory_store = Arc::new(MemoryStore::new(db_dir.path()).expect("create memory store"));
    let teleological_store = Arc::new(InMemoryTeleologicalStore::new());
    let embedder: Arc<dyn EmbeddingProvider> = Arc::new(TestEmbeddingProvider);

    // Create capture service with TeleologicalMemoryStore integration
    let capture_service = MemoryCaptureService::with_teleological_store(
        memory_store.clone(),
        embedder,
        teleological_store.clone(),
    );

    // Synthetic test data
    let file_path = "/test/docs/authentication.md";
    let chunk_content =
        "## Authentication Flow\n\nThe system uses JWT tokens for stateless authentication.";
    let chunk_index = 0u32;
    let total_chunks = 3u32;

    println!("INPUT:");
    println!("  File path: {}", file_path);
    println!("  Chunk: {}/{}", chunk_index + 1, total_chunks);
    println!("  Content: {}...", &chunk_content[..50]);

    // Create TextChunk with metadata
    let chunk = TextChunk::new(
        chunk_content.to_string(),
        ChunkMetadata {
            file_path: file_path.to_string(),
            chunk_index,
            total_chunks,
            word_offset: 0,
            char_offset: 0,
            original_file_hash: "sha256:test_hash_123".to_string(),
            start_line: 1,
            end_line: 3,
        },
    );

    // Capture the chunk
    let memory_id = capture_service
        .capture_md_chunk(chunk, "test-session-001".to_string())
        .await
        .expect("capture should succeed");

    println!("\nCAPTURE RESULT:");
    println!("  Memory ID: {}", memory_id);

    // VERIFICATION 1: Check source metadata was stored
    let source_metadata = teleological_store
        .get_source_metadata(memory_id)
        .await
        .expect("get source metadata");

    println!("\nVERIFICATION 1: Source Metadata Storage");
    match source_metadata {
        Some(meta) => {
            println!("  Source type: {:?}", meta.source_type);
            println!("  File path: {:?}", meta.file_path);
            println!("  Chunk index: {:?}", meta.chunk_index);
            println!("  Total chunks: {:?}", meta.total_chunks);

            // Assertions
            assert_eq!(meta.source_type, SourceType::MDFileChunk);
            assert_eq!(meta.file_path, Some(file_path.to_string()));
            assert_eq!(meta.chunk_index, Some(chunk_index));
            assert_eq!(meta.total_chunks, Some(total_chunks));

            println!("  ✓ All source metadata fields match expected values");
        }
        None => {
            panic!("FAIL: Source metadata was NOT stored!");
        }
    }

    // VERIFICATION 2: Check content was stored
    let content = teleological_store
        .get_content(memory_id)
        .await
        .expect("get content");

    println!("\nVERIFICATION 2: Content Storage");
    match content {
        Some(c) => {
            println!("  Content length: {} chars", c.len());
            assert_eq!(c, chunk_content);
            println!("  ✓ Content matches original");
        }
        None => {
            panic!("FAIL: Content was NOT stored!");
        }
    }

    // VERIFICATION 3: Check fingerprint was stored
    let fingerprint = teleological_store
        .retrieve(memory_id)
        .await
        .expect("retrieve fingerprint");

    println!("\nVERIFICATION 3: Fingerprint Storage");
    match fingerprint {
        Some(fp) => {
            println!("  Fingerprint ID: {}", fp.id);
            println!("  Importance: {}", fp.importance);
            assert_eq!(fp.id, memory_id, "Fingerprint ID should match Memory ID");
            println!("  ✓ Fingerprint exists with correct ID");
        }
        None => {
            panic!("FAIL: Fingerprint was NOT stored!");
        }
    }

    // VERIFICATION 4: Display string format
    let meta = teleological_store
        .get_source_metadata(memory_id)
        .await
        .expect("get")
        .expect("exists");
    let display = meta.display_string();
    println!("\nVERIFICATION 4: Display String Format");
    println!("  Display: {}", display);
    assert!(
        display.contains(file_path),
        "Display should contain file path"
    );
    assert!(display.contains("1/3"), "Display should contain chunk info");
    println!("  ✓ Display string formatted correctly");

    println!("\n=== FSV: PASSED - All source metadata verifications succeeded ===\n");
}

/// Test that HookDescription captures store source metadata.
#[tokio::test]
async fn test_hook_description_stores_source_metadata() {
    println!("\n=== FSV: Hook Description Source Metadata Test ===\n");

    let db_dir = TempDir::new().expect("create temp dir");
    let memory_store = Arc::new(MemoryStore::new(db_dir.path()).expect("create memory store"));
    let teleological_store = Arc::new(InMemoryTeleologicalStore::new());
    let embedder: Arc<dyn EmbeddingProvider> = Arc::new(TestEmbeddingProvider);

    let capture_service = MemoryCaptureService::with_teleological_store(
        memory_store,
        embedder,
        teleological_store.clone(),
    );

    // Synthetic test data
    let content = "Claude edited src/main.rs to add a new authentication module.";
    let hook_type = HookType::PostToolUse;
    let tool_name = "Edit";

    println!("INPUT:");
    println!("  Hook type: {:?}", hook_type);
    println!("  Tool name: {}", tool_name);
    println!("  Content: {}", content);

    // Capture hook description
    let memory_id = capture_service
        .capture_hook_description(
            content.to_string(),
            hook_type,
            "test-session-002".to_string(),
            Some(tool_name.to_string()),
        )
        .await
        .expect("capture should succeed");

    println!("\nCAPTURE RESULT:");
    println!("  Memory ID: {}", memory_id);

    // Verify source metadata
    let source_metadata = teleological_store
        .get_source_metadata(memory_id)
        .await
        .expect("get source metadata");

    println!("\nVERIFICATION:");
    match source_metadata {
        Some(meta) => {
            println!("  Source type: {:?}", meta.source_type);
            println!("  Hook type: {:?}", meta.hook_type);
            println!("  Tool name: {:?}", meta.tool_name);

            assert_eq!(meta.source_type, SourceType::HookDescription);
            assert_eq!(meta.hook_type, Some("PostToolUse".to_string()));
            assert_eq!(meta.tool_name, Some(tool_name.to_string()));

            let display = meta.display_string();
            println!("  Display: {}", display);
            assert!(display.contains("PostToolUse"));
            assert!(display.contains("Edit"));

            println!("  ✓ Hook description source metadata correct");
        }
        None => {
            panic!("FAIL: Source metadata was NOT stored!");
        }
    }

    println!("\n=== FSV: PASSED ===\n");
}

/// Test batch retrieval of source metadata.
#[tokio::test]
async fn test_batch_source_metadata_retrieval() {
    println!("\n=== FSV: Batch Source Metadata Retrieval Test ===\n");

    let db_dir = TempDir::new().expect("create temp dir");
    let memory_store = Arc::new(MemoryStore::new(db_dir.path()).expect("create memory store"));
    let teleological_store = Arc::new(InMemoryTeleologicalStore::new());
    let embedder: Arc<dyn EmbeddingProvider> = Arc::new(TestEmbeddingProvider);

    let capture_service = MemoryCaptureService::with_teleological_store(
        memory_store,
        embedder,
        teleological_store.clone(),
    );

    // Create 5 chunks from a single file
    let file_path = "/test/docs/large_document.md";
    let mut memory_ids = Vec::new();

    println!("INPUT: Creating 5 chunks for {}", file_path);

    for i in 0..5u32 {
        let chunk = TextChunk::new(
            format!("This is chunk {} content about topic {}", i + 1, i + 1),
            ChunkMetadata {
                file_path: file_path.to_string(),
                chunk_index: i,
                total_chunks: 5,
                word_offset: i * 100,
                char_offset: i * 500,
                original_file_hash: "sha256:batch_test_hash".to_string(),
                start_line: (i * 10) + 1,
                end_line: (i * 10) + 10,
            },
        );

        let id = capture_service
            .capture_md_chunk(chunk, format!("batch-session-{}", i))
            .await
            .expect("capture");
        memory_ids.push(id);
        println!("  Stored chunk {} with ID: {}", i + 1, id);
    }

    // Batch retrieve source metadata
    println!("\nBATCH RETRIEVAL:");
    let batch_metadata = teleological_store
        .get_source_metadata_batch(&memory_ids)
        .await
        .expect("batch get");

    assert_eq!(batch_metadata.len(), 5, "Should return 5 metadata entries");

    for (i, meta_opt) in batch_metadata.iter().enumerate() {
        match meta_opt {
            Some(meta) => {
                assert_eq!(meta.source_type, SourceType::MDFileChunk);
                assert_eq!(meta.file_path, Some(file_path.to_string()));
                assert_eq!(meta.chunk_index, Some(i as u32));
                assert_eq!(meta.total_chunks, Some(5));
                println!("  ✓ Chunk {}: {}", i + 1, meta.display_string());
            }
            None => {
                panic!("FAIL: Metadata for chunk {} was not found!", i + 1);
            }
        }
    }

    println!("\n=== FSV: PASSED - Batch retrieval works correctly ===\n");
}

/// Test that delete_by_file_path removes source metadata too.
#[tokio::test]
async fn test_delete_by_file_path_removes_source_metadata() {
    println!("\n=== FSV: Delete by File Path Test ===\n");

    let db_dir = TempDir::new().expect("create temp dir");
    let memory_store = Arc::new(MemoryStore::new(db_dir.path()).expect("create memory store"));
    let teleological_store = Arc::new(InMemoryTeleologicalStore::new());
    let embedder: Arc<dyn EmbeddingProvider> = Arc::new(TestEmbeddingProvider);

    let capture_service = MemoryCaptureService::with_teleological_store(
        memory_store,
        embedder,
        teleological_store.clone(),
    );

    // Create chunks
    let file_path = "/test/docs/to_be_deleted.md";
    let mut memory_ids = Vec::new();

    println!("SETUP: Creating 3 chunks for {}", file_path);

    for i in 0..3u32 {
        let chunk = TextChunk::new(
            format!("Original content chunk {}", i + 1),
            ChunkMetadata {
                file_path: file_path.to_string(),
                chunk_index: i,
                total_chunks: 3,
                word_offset: 0,
                char_offset: 0,
                original_file_hash: "sha256:delete_test".to_string(),
                start_line: (i * 5) + 1,
                end_line: (i * 5) + 5,
            },
        );

        let id = capture_service
            .capture_md_chunk(chunk, "delete-test-session".to_string())
            .await
            .expect("capture");
        memory_ids.push(id);
    }

    // Verify they exist
    println!("\nBEFORE DELETE:");
    for id in &memory_ids {
        let meta = teleological_store
            .get_source_metadata(*id)
            .await
            .expect("get");
        assert!(meta.is_some(), "Metadata should exist before delete");
        println!("  ID {}: source metadata exists ✓", id);
    }

    // Delete by file path
    println!("\nDELETING by file path: {}", file_path);
    let deleted_count = capture_service
        .delete_by_file_path(file_path)
        .await
        .expect("delete");
    println!("  Deleted {} memories", deleted_count);

    // Verify they're gone from TeleologicalMemoryStore (soft deleted)
    println!("\nAFTER DELETE:");
    for id in &memory_ids {
        let fingerprint = teleological_store.retrieve(*id).await.expect("retrieve");
        // Fingerprint should be None because it was soft-deleted
        assert!(fingerprint.is_none(), "Fingerprint should be soft-deleted");
        println!("  ID {}: fingerprint soft-deleted ✓", id);
    }

    println!("\n=== FSV: PASSED - Delete removes data correctly ===\n");
}
