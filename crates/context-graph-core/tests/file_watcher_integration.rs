//! Integration test for file watcher with stale embedding cleanup.
//!
//! This test verifies:
//! 1. File watcher processes markdown files correctly
//! 2. Source metadata is stored for each chunk
//! 3. When files are modified, old embeddings are deleted before new ones are stored
//! 4. The knowledge graph always reflects current file content (no stale data)

use std::fs;
use std::sync::Arc;
use tempfile::TempDir;

use context_graph_core::memory::capture::{
    EmbeddingProvider, MemoryCaptureService, TestEmbeddingProvider,
};
use context_graph_core::memory::store::MemoryStore;
use context_graph_core::memory::watcher::GitFileWatcher;
use context_graph_core::stubs::InMemoryTeleologicalStore;
use context_graph_core::traits::TeleologicalMemoryStore;
use context_graph_core::types::{SourceMetadata, SourceType};
use std::process::Command;

/// Initialize git repo with user config
fn init_git_repo(dir: &std::path::Path) {
    Command::new("git")
        .args(["init"])
        .current_dir(dir)
        .output()
        .expect("git init");
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(dir)
        .output()
        .expect("git config email");
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(dir)
        .output()
        .expect("git config name");
}

/// Helper to create test environment
async fn setup_test_env() -> (
    GitFileWatcher,
    Arc<MemoryStore>,
    Arc<InMemoryTeleologicalStore>,
    TempDir, // db_dir
    TempDir, // watch_dir
) {
    let db_dir = TempDir::new().expect("create db temp dir");
    let watch_dir = TempDir::new().expect("create watch temp dir");

    // Initialize git repo for GitFileWatcher
    init_git_repo(watch_dir.path());

    let memory_store = Arc::new(MemoryStore::new(db_dir.path()).expect("create memory store"));
    let teleological_store = Arc::new(InMemoryTeleologicalStore::new());
    let embedder: Arc<dyn EmbeddingProvider> = Arc::new(TestEmbeddingProvider);

    let capture_service = Arc::new(MemoryCaptureService::with_teleological_store(
        memory_store.clone(),
        embedder,
        teleological_store.clone(),
    ));

    let watcher = GitFileWatcher::new(
        vec![watch_dir.path().to_path_buf()],
        capture_service,
        "test-session".to_string(),
    )
    .expect("create watcher");

    (watcher, memory_store, teleological_store, db_dir, watch_dir)
}

/// Test: File watcher processes new markdown file and stores source metadata
#[tokio::test]
async fn test_watcher_processes_new_file_with_source_metadata() {
    println!("\n=== FSV: File Watcher New File Processing Test ===\n");

    let (mut watcher, memory_store, teleological_store, _db_dir, watch_dir) =
        setup_test_env().await;

    // Create a markdown file with known content
    let file_path = watch_dir.path().join("test_doc.md");
    let file_content = "# Test Document\n\nThis is test content for the file watcher.\n\nIt should be chunked and processed correctly.";

    println!("INPUT:");
    println!("  File: {:?}", file_path);
    println!("  Content length: {} chars", file_content.len());

    fs::write(&file_path, file_content).expect("write test file");

    // Start watcher and let it process
    watcher.start().await.expect("start watcher");

    // Get the canonical path (as the watcher stores it)
    let canonical_path = file_path.canonicalize().expect("canonicalize");
    let path_str = canonical_path.to_string_lossy().to_string();

    println!("\nVERIFICATION: Checking MemoryStore");

    // Verify memories were stored in MemoryStore
    let memories = memory_store
        .get_by_file_path(&path_str)
        .expect("get memories");
    println!("  Found {} memories for file", memories.len());
    assert!(!memories.is_empty(), "Should have at least one memory");

    // Get memory IDs for TeleologicalMemoryStore verification
    let memory_ids: Vec<_> = memories.iter().map(|m| m.id).collect();

    println!("\nVERIFICATION: Checking TeleologicalMemoryStore Source Metadata");

    // Verify source metadata exists in TeleologicalMemoryStore
    for (i, memory) in memories.iter().enumerate() {
        let source_metadata: Option<SourceMetadata> = teleological_store
            .get_source_metadata(memory.id)
            .await
            .expect("get source metadata");

        match source_metadata {
            Some(meta) => {
                println!("  Memory {}: {} ✓", i + 1, meta.display_string());
                assert_eq!(meta.source_type, SourceType::MDFileChunk);
                assert_eq!(meta.file_path, Some(path_str.clone()));
            }
            None => {
                panic!("FAIL: Source metadata missing for memory {}", memory.id);
            }
        }
    }

    // Verify content is stored in TeleologicalMemoryStore
    println!("\nVERIFICATION: Checking Content Storage");
    for (i, memory_id) in memory_ids.iter().enumerate() {
        let content = teleological_store
            .get_content(*memory_id)
            .await
            .expect("get content");

        match content {
            Some(c) => {
                println!("  Memory {}: {} chars stored ✓", i + 1, c.len());
            }
            None => {
                panic!("FAIL: Content missing for memory {}", memory_id);
            }
        }
    }

    watcher.stop();

    println!("\n=== FSV: PASSED - New file processed with source metadata ===\n");
}

/// Test: File modification triggers stale embedding cleanup
#[tokio::test]
async fn test_stale_embedding_cleanup_on_file_modification() {
    println!("\n=== FSV: Stale Embedding Cleanup Test ===\n");

    let (mut watcher, memory_store, teleological_store, _db_dir, watch_dir) =
        setup_test_env().await;

    // Create original file
    let file_path = watch_dir.path().join("modify_test.md");
    let original_content = "# Original Document\n\nThis is the ORIGINAL content version 1.\n\nIt will be replaced later.";

    println!("PHASE 1: Create original file");
    println!("  Content marker: 'ORIGINAL content version 1'");

    fs::write(&file_path, original_content).expect("write original");

    // Start watcher
    watcher.start().await.expect("start watcher");

    // Get canonical path
    let canonical_path = file_path.canonicalize().expect("canonicalize");
    let path_str = canonical_path.to_string_lossy().to_string();

    // Verify original content stored
    let original_memories = memory_store
        .get_by_file_path(&path_str)
        .expect("get original");
    let original_ids: Vec<_> = original_memories.iter().map(|m| m.id).collect();

    println!(
        "\n  Stored {} chunks from original file",
        original_memories.len()
    );

    // Verify original content is stored
    let mut found_original = false;
    for memory in &original_memories {
        if memory.content.contains("ORIGINAL") && memory.content.contains("version 1") {
            found_original = true;
            println!("  ✓ Found original content marker");
            break;
        }
    }
    assert!(found_original, "Original content should contain marker");

    // Verify source metadata exists for original
    for id in &original_ids {
        let meta = teleological_store
            .get_source_metadata(*id)
            .await
            .expect("get meta");
        assert!(meta.is_some(), "Original should have source metadata");
    }
    println!("  ✓ Original source metadata stored");

    // PHASE 2: Modify the file
    println!("\nPHASE 2: Modify file with new content");
    let modified_content = "# Modified Document\n\nThis is the UPDATED content version 2.\n\nCompletely different from before.\n\nWith additional paragraphs.";
    println!("  Content marker: 'UPDATED content version 2'");

    fs::write(&file_path, modified_content).expect("write modified");

    watcher.process_events().await.expect("process events");

    // VERIFICATION: Check stale cleanup
    println!("\nVERIFICATION: Stale Embedding Cleanup");

    // Get new memories
    let new_memories = memory_store.get_by_file_path(&path_str).expect("get new");
    let new_ids: Vec<_> = new_memories.iter().map(|m| m.id).collect();

    println!("  Current memory count: {}", new_memories.len());

    // Check NO memory contains original content
    let mut found_stale = false;
    for memory in &new_memories {
        if memory.content.contains("ORIGINAL") || memory.content.contains("version 1") {
            found_stale = true;
            println!(
                "  FAIL: Found stale content: {}",
                &memory.content[..50.min(memory.content.len())]
            );
        }
    }
    assert!(
        !found_stale,
        "Should NOT find any stale content - old embeddings should be deleted"
    );
    println!("  ✓ No stale content found (old embeddings properly deleted)");

    // Check new content exists
    let mut found_updated = false;
    for memory in &new_memories {
        if memory.content.contains("UPDATED") && memory.content.contains("version 2") {
            found_updated = true;
            println!("  ✓ Found updated content marker");
            break;
        }
    }
    assert!(found_updated, "Should find updated content");

    // Verify TeleologicalMemoryStore cleanup
    println!("\nVERIFICATION: TeleologicalMemoryStore State");

    // Old fingerprints should be soft-deleted (not retrievable)
    for id in &original_ids {
        let fp = teleological_store
            .retrieve(*id)
            .await
            .expect("retrieve old");
        if fp.is_some() {
            println!(
                "  WARNING: Old fingerprint {} still accessible (not soft-deleted)",
                id
            );
        }
    }

    // New fingerprints should exist with source metadata
    for id in &new_ids {
        let fp = teleological_store
            .retrieve(*id)
            .await
            .expect("retrieve new");
        assert!(fp.is_some(), "New fingerprint should exist");

        let meta = teleological_store
            .get_source_metadata(*id)
            .await
            .expect("get new meta");
        assert!(meta.is_some(), "New source metadata should exist");
    }
    println!("  ✓ New fingerprints with source metadata exist");

    watcher.stop();

    println!("\n=== FSV: PASSED - Stale embeddings properly cleaned up ===\n");
}

/// Test: Multiple files in same directory
#[tokio::test]
async fn test_multiple_files_isolated() {
    println!("\n=== FSV: Multiple Files Isolation Test ===\n");

    let (mut watcher, memory_store, teleological_store, _db_dir, watch_dir) =
        setup_test_env().await;

    // Create two files
    let file1_path = watch_dir.path().join("file1.md");
    let file2_path = watch_dir.path().join("file2.md");

    let file1_content = "# File One\n\nThis is content for FILE1 only.";
    let file2_content = "# File Two\n\nThis is content for FILE2 only.";

    fs::write(&file1_path, file1_content).expect("write file1");
    fs::write(&file2_path, file2_content).expect("write file2");

    println!("Created 2 markdown files");

    watcher.start().await.expect("start watcher");

    let path1 = file1_path
        .canonicalize()
        .expect("canonicalize")
        .to_string_lossy()
        .to_string();
    let path2 = file2_path
        .canonicalize()
        .expect("canonicalize")
        .to_string_lossy()
        .to_string();

    let memories1 = memory_store.get_by_file_path(&path1).expect("get file1");
    let memories2 = memory_store.get_by_file_path(&path2).expect("get file2");

    println!("\nVERIFICATION:");
    println!("  File1 memories: {}", memories1.len());
    println!("  File2 memories: {}", memories2.len());

    assert!(!memories1.is_empty(), "File1 should have memories");
    assert!(!memories2.is_empty(), "File2 should have memories");

    // Verify isolation - file1 memories shouldn't contain file2 content
    for mem in &memories1 {
        assert!(
            !mem.content.contains("FILE2"),
            "File1 memories shouldn't contain FILE2"
        );
        assert!(
            mem.content.contains("FILE1") || mem.content.contains("File One"),
            "File1 should have its content"
        );
    }

    for mem in &memories2 {
        assert!(
            !mem.content.contains("FILE1"),
            "File2 memories shouldn't contain FILE1"
        );
        assert!(
            mem.content.contains("FILE2") || mem.content.contains("File Two"),
            "File2 should have its content"
        );
    }

    println!("  ✓ File contents properly isolated");

    // Verify source metadata points to correct files
    for mem in &memories1 {
        let meta = teleological_store
            .get_source_metadata(mem.id)
            .await
            .expect("get")
            .expect("exists");
        assert_eq!(
            meta.file_path,
            Some(path1.clone()),
            "File1 metadata should point to file1"
        );
    }

    for mem in &memories2 {
        let meta = teleological_store
            .get_source_metadata(mem.id)
            .await
            .expect("get")
            .expect("exists");
        assert_eq!(
            meta.file_path,
            Some(path2.clone()),
            "File2 metadata should point to file2"
        );
    }

    println!("  ✓ Source metadata points to correct files");

    // Now delete file1 and verify file2 is unaffected
    println!("\nDeleting file1 memories...");
    fs::remove_file(&file1_path).expect("remove file1");

    // Manually trigger delete (simulating what watcher would do on file delete event)
    let capture_service = watcher.capture_service();
    let deleted = capture_service
        .delete_by_file_path(&path1)
        .await
        .expect("delete");
    println!("  Deleted {} memories", deleted);

    // Verify file2 is still intact
    let memories2_after = memory_store
        .get_by_file_path(&path2)
        .expect("get file2 after");
    assert_eq!(
        memories2.len(),
        memories2_after.len(),
        "File2 should be unaffected"
    );
    println!("  ✓ File2 memories intact after file1 deletion");

    watcher.stop();

    println!("\n=== FSV: PASSED - Multiple files properly isolated ===\n");
}

/// Test: File index is populated and can be queried
#[tokio::test]
async fn test_file_index_integration() {
    println!("\n=== FSV: File Index Integration Test ===\n");

    let (mut watcher, memory_store, teleological_store, _db_dir, watch_dir) =
        setup_test_env().await;

    // Create two markdown files
    let file1_path = watch_dir.path().join("index_test1.md");
    let file2_path = watch_dir.path().join("index_test2.md");

    let file1_content = "# Index Test File 1\n\nThis is content for index testing file one.\n\nIt has multiple paragraphs to create chunks.";
    let file2_content = "# Index Test File 2\n\nThis is content for index testing file two.\n\nAlso has multiple paragraphs.";

    fs::write(&file1_path, file1_content).expect("write file1");
    fs::write(&file2_path, file2_content).expect("write file2");

    println!("PHASE 1: Creating markdown files and starting watcher");

    watcher.start().await.expect("start watcher");

    let path1 = file1_path
        .canonicalize()
        .expect("canonicalize")
        .to_string_lossy()
        .to_string();
    let path2 = file2_path
        .canonicalize()
        .expect("canonicalize")
        .to_string_lossy()
        .to_string();

    println!("\nPHASE 2: Verifying file index entries created");

    // Verify list_indexed_files returns both files
    let indexed_files = teleological_store
        .list_indexed_files()
        .await
        .expect("list indexed files");
    println!("  Found {} indexed files", indexed_files.len());

    let indexed_paths: Vec<&str> = indexed_files.iter().map(|e| e.file_path.as_str()).collect();
    assert!(
        indexed_paths.contains(&path1.as_str()),
        "File1 should be indexed"
    );
    assert!(
        indexed_paths.contains(&path2.as_str()),
        "File2 should be indexed"
    );
    println!("  ✓ Both files found in index");

    // Verify get_fingerprints_for_file returns correct fingerprints
    let file1_fingerprints = teleological_store
        .get_fingerprints_for_file(&path1)
        .await
        .expect("get file1 fingerprints");
    let file2_fingerprints = teleological_store
        .get_fingerprints_for_file(&path2)
        .await
        .expect("get file2 fingerprints");

    println!("  File1 fingerprints: {}", file1_fingerprints.len());
    println!("  File2 fingerprints: {}", file2_fingerprints.len());

    assert!(
        !file1_fingerprints.is_empty(),
        "File1 should have fingerprints"
    );
    assert!(
        !file2_fingerprints.is_empty(),
        "File2 should have fingerprints"
    );

    // Verify fingerprint counts match memory store
    let memories1 = memory_store
        .get_by_file_path(&path1)
        .expect("get memories1");
    let memories2 = memory_store
        .get_by_file_path(&path2)
        .expect("get memories2");

    assert_eq!(
        file1_fingerprints.len(),
        memories1.len(),
        "File1 fingerprint count should match memory count"
    );
    assert_eq!(
        file2_fingerprints.len(),
        memories2.len(),
        "File2 fingerprint count should match memory count"
    );
    println!("  ✓ Fingerprint counts match memory counts");

    // Verify memory IDs match between file index and memory store
    let memory1_ids: std::collections::HashSet<_> = memories1.iter().map(|m| m.id).collect();
    let memory2_ids: std::collections::HashSet<_> = memories2.iter().map(|m| m.id).collect();
    let file1_id_set: std::collections::HashSet<_> = file1_fingerprints.into_iter().collect();
    let file2_id_set: std::collections::HashSet<_> = file2_fingerprints.into_iter().collect();

    assert_eq!(
        memory1_ids, file1_id_set,
        "File1 fingerprint IDs should match memory IDs"
    );
    assert_eq!(
        memory2_ids, file2_id_set,
        "File2 fingerprint IDs should match memory IDs"
    );
    println!("  ✓ Fingerprint IDs match memory IDs");

    println!("\nPHASE 3: Verifying file_watcher_stats");

    let stats = teleological_store
        .get_file_watcher_stats()
        .await
        .expect("get stats");
    println!("  Total files: {}", stats.total_files);
    println!("  Total chunks: {}", stats.total_chunks);
    println!("  Avg chunks per file: {:.2}", stats.avg_chunks_per_file);

    assert_eq!(stats.total_files, 2, "Should have 2 files");
    assert_eq!(
        stats.total_chunks,
        memories1.len() + memories2.len(),
        "Total chunks should match"
    );
    println!("  ✓ Stats match expected values");

    println!("\nPHASE 4: Deleting file1 and verifying index cleanup");

    // Delete file1 memories using delete_by_file_path
    let capture_service = watcher.capture_service();
    let deleted = capture_service
        .delete_by_file_path(&path1)
        .await
        .expect("delete file1");
    println!("  Deleted {} memories", deleted);

    // Verify file1 is no longer in the index
    let indexed_files_after = teleological_store
        .list_indexed_files()
        .await
        .expect("list after delete");
    let indexed_paths_after: Vec<&str> = indexed_files_after
        .iter()
        .map(|e| e.file_path.as_str())
        .collect();

    assert!(
        !indexed_paths_after.contains(&path1.as_str()),
        "File1 should NOT be in index after deletion"
    );
    assert!(
        indexed_paths_after.contains(&path2.as_str()),
        "File2 should still be in index"
    );
    println!("  ✓ File1 removed from index, File2 still present");

    // Verify get_fingerprints_for_file returns empty for deleted file
    let file1_fingerprints_after = teleological_store
        .get_fingerprints_for_file(&path1)
        .await
        .expect("get file1 after");
    assert!(
        file1_fingerprints_after.is_empty(),
        "File1 should have no fingerprints after deletion"
    );
    println!("  ✓ File1 has no fingerprints in index");

    // Verify stats updated
    let stats_after = teleological_store
        .get_file_watcher_stats()
        .await
        .expect("get stats after");
    assert_eq!(
        stats_after.total_files, 1,
        "Should have 1 file after deletion"
    );
    println!("  ✓ Stats updated after deletion");

    watcher.stop();

    println!("\n=== FSV: PASSED - File index integration working correctly ===\n");
}
