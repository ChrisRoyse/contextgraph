//! Memory capture service for coordinating embedding and storage.
//!
//! # Architecture
//!
//! The MemoryCaptureService coordinates:
//! 1. Content validation (empty, length)
//! 2. Embedding via EmbeddingProvider trait
//! 3. Memory construction with proper source type
//! 4. Persistence via MemoryStore
//!
//! # Fail-Fast Semantics
//!
//! All operations fail immediately on any error. No retries, no partial states.
//! This follows constitution.yaml AP-14: "No .unwrap() in library code".
//!
//! # Constitution Compliance
//! - ARCH-01: TeleologicalArray is atomic (all 13 embeddings)
//! - ARCH-06: All memory ops through MCP tools (this is the service layer)
//! - AP-14: No .unwrap() - all errors propagate via Result

use std::sync::Arc;

use async_trait::async_trait;
use thiserror::Error;
use tracing::{debug, error, info, instrument};
use uuid::Uuid;

use sha2::{Digest, Sha256};

use super::store::{MemoryStore, StorageError};
use super::{
    ChunkMetadata, HookType, Memory, MemorySource, ResponseType, TextChunk, MAX_CONTENT_LENGTH,
};
use crate::traits::TeleologicalMemoryStore;
use crate::types::fingerprint::{TeleologicalArray, TeleologicalFingerprint};
use crate::types::SourceMetadata;

/// Errors from embedding operations.
///
/// These errors indicate failures in the embedding pipeline.
/// In Phase 1, only the mock embedder is used; in Phase 2,
/// the GPU pipeline will produce these errors.
#[derive(Debug, Clone, Error)]
pub enum EmbedderError {
    /// Embedding service is not available (GPU offline, model not loaded).
    #[error("Embedding service unavailable")]
    Unavailable,

    /// Embedding computation failed (GPU error, memory exhaustion).
    #[error("Embedding computation failed: {message}")]
    ComputationFailed { message: String },

    /// Input is invalid for embedding (e.g., unsupported characters).
    #[error("Invalid input for embedding: {reason}")]
    InvalidInput { reason: String },
}

/// Errors from memory capture operations.
///
/// Captures all failure modes in the capture pipeline:
/// validation, embedding, and storage.
#[derive(Debug, Error)]
pub enum CaptureError {
    /// Content is empty or contains only whitespace.
    #[error("Content is empty or whitespace-only")]
    EmptyContent,

    /// Content exceeds maximum allowed length.
    #[error("Content exceeds maximum length of {max} characters: got {actual}")]
    ContentTooLong { max: usize, actual: usize },

    /// Embedding operation failed.
    #[error("Embedding failed: {0}")]
    EmbeddingFailed(#[from] EmbedderError),

    /// Storage operation failed.
    #[error("Storage failed: {0}")]
    StorageFailed(#[from] StorageError),

    /// Memory validation failed after construction.
    #[error("Memory validation failed: {reason}")]
    ValidationFailed { reason: String },
}

/// Trait for embedding providers.
///
/// Implementations must produce a complete TeleologicalArray with all
/// 13 embeddings. Partial arrays are not allowed (ARCH-01).
///
/// # Phase 1
///
/// Uses TestEmbeddingProvider with zeroed arrays for testing.
///
/// # Phase 2+
///
/// GPU pipeline implementation will provide real embeddings.
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Embed content into a full 13-embedding TeleologicalArray.
    ///
    /// # Arguments
    ///
    /// * `content` - The text content to embed (assumed non-empty)
    ///
    /// # Returns
    ///
    /// * `Ok(TeleologicalArray)` - Complete array with all 13 embeddings
    /// * `Err(EmbedderError)` - If embedding fails for any reason
    ///
    /// # Contract
    ///
    /// The returned TeleologicalArray MUST pass `validate_strict()`.
    /// Partial or invalid arrays are considered bugs in the implementation.
    async fn embed_all(&self, content: &str) -> Result<TeleologicalArray, EmbedderError>;
}

/// Memory capture service coordinating embedding and storage.
///
/// This is the primary interface for capturing memories from:
/// - Hook events (SessionStart, PostToolUse, etc.)
/// - Claude responses (SessionSummary, StopResponse)
/// - Markdown file chunks (from MDFileWatcher)
///
/// # Thread Safety
///
/// The service is `Send + Sync` and can be shared across async tasks.
/// Both MemoryStore and EmbeddingProvider are accessed through Arc.
///
/// # Teleological Storage (Phase 3.6)
///
/// When a TeleologicalMemoryStore is configured, captured memories are ALSO
/// stored there with source metadata. This enables:
/// - Semantic search via MCP tools
/// - Source file tracking (file path, chunk info)
/// - Context injection with provenance
pub struct MemoryCaptureService {
    store: Arc<MemoryStore>,
    embedder: Arc<dyn EmbeddingProvider>,
    /// Optional TeleologicalMemoryStore for semantic search integration.
    /// When set, memories are also stored here with source metadata.
    teleological_store: Option<Arc<dyn TeleologicalMemoryStore>>,
}

impl MemoryCaptureService {
    /// Create a new MemoryCaptureService.
    ///
    /// # Arguments
    ///
    /// * `store` - The MemoryStore for persistence
    /// * `embedder` - The embedding provider (mock in Phase 1, GPU in Phase 2+)
    pub fn new(store: Arc<MemoryStore>, embedder: Arc<dyn EmbeddingProvider>) -> Self {
        Self {
            store,
            embedder,
            teleological_store: None,
        }
    }

    /// Create a MemoryCaptureService with TeleologicalMemoryStore integration.
    ///
    /// When a TeleologicalMemoryStore is provided, captured memories are also
    /// stored there with source metadata. This enables semantic search via MCP
    /// tools and source provenance tracking.
    ///
    /// # Arguments
    ///
    /// * `store` - The MemoryStore for persistence
    /// * `embedder` - The embedding provider
    /// * `teleological_store` - The TeleologicalMemoryStore for semantic search
    pub fn with_teleological_store(
        store: Arc<MemoryStore>,
        embedder: Arc<dyn EmbeddingProvider>,
        teleological_store: Arc<dyn TeleologicalMemoryStore>,
    ) -> Self {
        Self {
            store,
            embedder,
            teleological_store: Some(teleological_store),
        }
    }

    /// Capture a hook description as memory.
    ///
    /// # Arguments
    ///
    /// * `content` - Description of what Claude did during the hook
    /// * `hook_type` - Which hook triggered this capture
    /// * `session_id` - Current session identifier
    /// * `tool_name` - Tool name for PreToolUse/PostToolUse hooks
    ///
    /// # Returns
    ///
    /// * `Ok(Uuid)` - The ID of the stored memory
    /// * `Err(CaptureError)` - If validation, embedding, or storage fails
    #[instrument(skip(self, content), fields(content_len = content.len()))]
    pub async fn capture_hook_description(
        &self,
        content: String,
        hook_type: HookType,
        session_id: String,
        tool_name: Option<String>,
    ) -> Result<Uuid, CaptureError> {
        info!(
            hook_type = %hook_type,
            session_id = %session_id,
            tool_name = ?tool_name,
            "Capturing hook description"
        );

        let source = MemorySource::HookDescription {
            hook_type,
            tool_name,
        };
        self.capture_memory(content, source, session_id, None).await
    }

    /// Capture a Claude response as memory.
    ///
    /// # Arguments
    ///
    /// * `content` - The response content to capture
    /// * `response_type` - Type of response (SessionSummary, StopResponse, etc.)
    /// * `session_id` - Current session identifier
    ///
    /// # Returns
    ///
    /// * `Ok(Uuid)` - The ID of the stored memory
    /// * `Err(CaptureError)` - If validation, embedding, or storage fails
    #[instrument(skip(self, content), fields(content_len = content.len()))]
    pub async fn capture_claude_response(
        &self,
        content: String,
        response_type: ResponseType,
        session_id: String,
    ) -> Result<Uuid, CaptureError> {
        info!(
            response_type = %response_type,
            session_id = %session_id,
            "Capturing Claude response"
        );

        let source = MemorySource::ClaudeResponse { response_type };
        self.capture_memory(content, source, session_id, None).await
    }

    /// Capture a markdown file chunk as memory.
    ///
    /// # Arguments
    ///
    /// * `chunk` - The TextChunk containing content and metadata
    /// * `session_id` - Current session identifier
    ///
    /// # Returns
    ///
    /// * `Ok(Uuid)` - The ID of the stored memory
    /// * `Err(CaptureError)` - If validation, embedding, or storage fails
    #[instrument(skip(self, chunk), fields(
        file_path = %chunk.metadata.file_path,
        chunk_index = chunk.metadata.chunk_index,
        total_chunks = chunk.metadata.total_chunks
    ))]
    pub async fn capture_md_chunk(
        &self,
        chunk: TextChunk,
        session_id: String,
    ) -> Result<Uuid, CaptureError> {
        info!(
            file_path = %chunk.metadata.file_path,
            chunk = %format!("{}/{}", chunk.metadata.chunk_index + 1, chunk.metadata.total_chunks),
            session_id = %session_id,
            "Capturing MD chunk"
        );

        let source = MemorySource::MDFileChunk {
            file_path: chunk.metadata.file_path.clone(),
            chunk_index: chunk.metadata.chunk_index,
            total_chunks: chunk.metadata.total_chunks,
        };

        self.capture_memory(chunk.content, source, session_id, Some(chunk.metadata))
            .await
    }

    /// Delete all memories associated with a file path.
    ///
    /// Used to clear stale embeddings when a markdown file is modified.
    /// This ensures the knowledge graph always reflects the current state
    /// of monitored files.
    ///
    /// When TeleologicalMemoryStore is configured, deletes are also performed
    /// there to keep both stores in sync.
    ///
    /// # Arguments
    ///
    /// * `file_path` - The file path to delete memories for
    ///
    /// # Returns
    ///
    /// * `Ok(usize)` - Number of memories deleted from MemoryStore
    /// * `Err(CaptureError)` - If deletion fails
    #[instrument(skip(self))]
    pub async fn delete_by_file_path(&self, file_path: &str) -> Result<usize, CaptureError> {
        info!(
            file_path = %file_path,
            "Deleting all memories for file"
        );

        // Get memory IDs from MemoryStore (may be empty if using separate db)
        let memories_to_delete = self.store.get_by_file_path(file_path)?;
        let mut memory_ids: Vec<_> = memories_to_delete.iter().map(|m| m.id).collect();

        // Delete from primary MemoryStore
        let deleted = self.store.delete_by_file_path(file_path)?;

        info!(
            file_path = %file_path,
            deleted_count = deleted,
            "Deleted memories from MemoryStore"
        );

        // Also delete from TeleologicalMemoryStore if configured
        if let Some(ref tele_store) = self.teleological_store {
            // CRITICAL: Also find fingerprints directly from TeleologicalStore
            // This handles the case where MemoryStore is in a separate directory
            // and doesn't have the old memory IDs
            match tele_store.find_fingerprints_by_file_path(file_path).await {
                Ok(tele_ids) => {
                    // Merge with MemoryStore IDs (deduplicate)
                    for id in tele_ids {
                        if !memory_ids.contains(&id) {
                            memory_ids.push(id);
                        }
                    }
                    debug!(
                        file_path = %file_path,
                        total_ids = memory_ids.len(),
                        "Found fingerprints to delete from TeleologicalStore"
                    );
                }
                Err(e) => {
                    error!(
                        file_path = %file_path,
                        error = %e,
                        "Failed to find fingerprints by file path in TeleologicalStore"
                    );
                }
            }

            let mut tele_deleted = 0;
            for id in &memory_ids {
                // Delete content
                if let Err(e) = tele_store.delete_content(*id).await {
                    error!(id = %id, error = %e, "Failed to delete content from TeleologicalMemoryStore");
                }
                // Delete source metadata
                if let Err(e) = tele_store.delete_source_metadata(*id).await {
                    error!(id = %id, error = %e, "Failed to delete source metadata from TeleologicalMemoryStore");
                }
                // Delete fingerprint (soft delete for 30-day recovery per SEC-06)
                match tele_store.delete(*id, true).await {
                    Ok(true) => tele_deleted += 1,
                    Ok(false) => {
                        debug!(id = %id, "Fingerprint not found in TeleologicalMemoryStore (may not have been stored)");
                    }
                    Err(e) => {
                        error!(id = %id, error = %e, "Failed to delete fingerprint from TeleologicalMemoryStore");
                    }
                }
            }
            info!(
                file_path = %file_path,
                tele_deleted = tele_deleted,
                "Deleted {} fingerprints from TeleologicalMemoryStore",
                tele_deleted
            );

            // FILE INDEX INTEGRATION: Clear file index entry for this file path
            // This removes the file_path -> Vec<Uuid> mapping from CF_FILE_INDEX
            match tele_store.clear_file_index(file_path).await {
                Ok(cleared_count) => {
                    debug!(
                        file_path = %file_path,
                        cleared_count = cleared_count,
                        "Cleared file index entries"
                    );
                }
                Err(e) => {
                    // Non-fatal: fingerprints are deleted, index may become stale
                    // Reconcile tool can clean up orphaned index entries later
                    error!(
                        file_path = %file_path,
                        error = %e,
                        "Failed to clear file index. Index may contain stale entries."
                    );
                }
            }
        }

        Ok(deleted)
    }

    /// Get all memories associated with a file path.
    ///
    /// # Arguments
    ///
    /// * `file_path` - The file path to get memories for
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<Memory>)` - All memories for the file (may be empty)
    /// * `Err(CaptureError)` - If retrieval fails
    pub fn get_by_file_path(&self, file_path: &str) -> Result<Vec<super::Memory>, CaptureError> {
        self.store.get_by_file_path(file_path).map_err(|e| e.into())
    }

    /// Internal method to capture memory with validation, embedding, and storage.
    ///
    /// # Fail-Fast Behavior
    ///
    /// 1. Validate content (empty, length) - fail if invalid
    /// 2. Call embedder - fail if embedding fails
    /// 3. Construct Memory - fail if validation fails
    /// 4. Store memory - fail if storage fails
    /// 5. If TeleologicalMemoryStore is configured, also store there with source metadata
    /// 6. Return UUID only on complete success
    async fn capture_memory(
        &self,
        content: String,
        source: MemorySource,
        session_id: String,
        chunk_metadata: Option<ChunkMetadata>,
    ) -> Result<Uuid, CaptureError> {
        // Step 1: Validate content
        if content.trim().is_empty() {
            error!("Capture rejected: empty content");
            return Err(CaptureError::EmptyContent);
        }

        if content.len() > MAX_CONTENT_LENGTH {
            error!(
                content_len = content.len(),
                max = MAX_CONTENT_LENGTH,
                "Capture rejected: content too long"
            );
            return Err(CaptureError::ContentTooLong {
                max: MAX_CONTENT_LENGTH,
                actual: content.len(),
            });
        }

        // Step 2: Generate embeddings (fail fast on error)
        debug!(content_len = content.len(), "Starting embedding");
        let teleological_array = self.embedder.embed_all(&content).await?;
        debug!(
            storage_size = teleological_array.storage_size(),
            "Embedding complete"
        );

        // Step 3: Construct Memory
        let memory = Memory::new(
            content.clone(),
            source.clone(),
            session_id.clone(),
            teleological_array.clone(),
            chunk_metadata.clone(),
        );

        // Step 4: Validate Memory (defensive - Memory::new should produce valid data)
        memory.validate().map_err(|reason| {
            error!(reason = %reason, "Memory validation failed");
            CaptureError::ValidationFailed { reason }
        })?;

        let memory_id = memory.id;

        // Step 5: Store in MemoryStore (fail fast on error)
        self.store.store(&memory)?;

        info!(
            memory_id = %memory_id,
            session_id = %session_id,
            "Memory stored successfully"
        );

        // Step 6: If TeleologicalMemoryStore is configured, also store there
        // This enables semantic search via MCP tools with source metadata
        if let Some(ref tele_store) = self.teleological_store {
            // Compute content hash for TeleologicalFingerprint
            let mut hasher = Sha256::new();
            hasher.update(content.as_bytes());
            let hash_result = hasher.finalize();
            let mut content_hash = [0u8; 32];
            content_hash.copy_from_slice(&hash_result);

            // Create TeleologicalFingerprint with same ID as Memory for correlation
            let mut fingerprint = TeleologicalFingerprint::new(teleological_array, content_hash);
            fingerprint.id = memory_id; // Use same ID for correlation

            // Store fingerprint
            if let Err(e) = tele_store.store(fingerprint).await {
                // Log error but don't fail - MemoryStore is the primary storage
                error!(
                    memory_id = %memory_id,
                    error = %e,
                    "Failed to store in TeleologicalMemoryStore (continuing, MemoryStore is primary)"
                );
            } else {
                // Store content for retrieval
                if let Err(e) = tele_store.store_content(memory_id, &content).await {
                    error!(
                        memory_id = %memory_id,
                        error = %e,
                        "Failed to store content in TeleologicalMemoryStore"
                    );
                }

                // Store source metadata
                let source_metadata = Self::memory_source_to_metadata(&source, &chunk_metadata);
                if let Err(e) = tele_store
                    .store_source_metadata(memory_id, &source_metadata)
                    .await
                {
                    error!(
                        memory_id = %memory_id,
                        error = %e,
                        "Failed to store source metadata in TeleologicalMemoryStore"
                    );
                } else {
                    debug!(
                        memory_id = %memory_id,
                        source_type = %source_metadata.source_type,
                        "Stored in TeleologicalMemoryStore with source metadata"
                    );
                }

                // FILE INDEX INTEGRATION: Index MDFileChunk memories for O(1) file path lookups
                // Per plan: This enables list_watched_files, delete_file_content, reconcile_files tools
                if let MemorySource::MDFileChunk { ref file_path, .. } = source {
                    if let Err(e) = tele_store
                        .index_file_fingerprint(file_path, memory_id)
                        .await
                    {
                        // Non-fatal: fingerprint is stored, index can be rebuilt
                        error!(
                            memory_id = %memory_id,
                            file_path = %file_path,
                            error = %e,
                            "Failed to index file fingerprint. File watcher tools may not find this memory."
                        );
                    } else {
                        debug!(
                            memory_id = %memory_id,
                            file_path = %file_path,
                            "Indexed file fingerprint for O(1) lookup"
                        );
                    }
                }
            }
        }

        Ok(memory_id)
    }

    /// Convert MemorySource to SourceMetadata for TeleologicalMemoryStore.
    ///
    /// For MDFileChunk sources, extracts line numbers from chunk_metadata if available.
    fn memory_source_to_metadata(
        source: &MemorySource,
        chunk_metadata: &Option<ChunkMetadata>,
    ) -> SourceMetadata {
        match source {
            MemorySource::MDFileChunk {
                file_path,
                chunk_index,
                total_chunks,
            } => {
                // Extract line numbers from chunk_metadata if available
                if let Some(meta) = chunk_metadata {
                    SourceMetadata::md_file_chunk_with_lines(
                        file_path.clone(),
                        *chunk_index,
                        *total_chunks,
                        meta.start_line,
                        meta.end_line,
                    )
                } else {
                    SourceMetadata::md_file_chunk(file_path.clone(), *chunk_index, *total_chunks)
                }
            }
            MemorySource::HookDescription {
                hook_type,
                tool_name,
            } => SourceMetadata::hook_description(format!("{}", hook_type), tool_name.clone()),
            MemorySource::ClaudeResponse { response_type: _ } => SourceMetadata::claude_response(),
            MemorySource::CausalExplanation {
                source_fingerprint_id,
                causal_relationship_id,
                mechanism_type,
                confidence,
            } => SourceMetadata::causal_explanation(
                *source_fingerprint_id,
                *causal_relationship_id,
                mechanism_type.clone(),
                *confidence,
            ),
        }
    }
}

// ============================================================================
// MultiArrayEmbeddingAdapter - Wraps MultiArrayEmbeddingProvider
// ============================================================================

use crate::traits::MultiArrayEmbeddingProvider;

/// Adapter that wraps a `MultiArrayEmbeddingProvider` to implement `EmbeddingProvider`.
///
/// This allows the file watcher and memory capture service to use the GPU
/// embedding pipeline (which implements `MultiArrayEmbeddingProvider`) without
/// requiring separate implementations.
///
/// # Usage
///
/// ```ignore
/// use context_graph_embeddings::get_warm_provider;
/// use context_graph_core::memory::capture::MultiArrayEmbeddingAdapter;
///
/// let multi_array_provider = get_warm_provider()?;
/// let embedding_provider: Arc<dyn EmbeddingProvider> =
///     Arc::new(MultiArrayEmbeddingAdapter::new(multi_array_provider));
/// ```
pub struct MultiArrayEmbeddingAdapter {
    provider: Arc<dyn MultiArrayEmbeddingProvider>,
}

impl MultiArrayEmbeddingAdapter {
    /// Create a new adapter wrapping the given `MultiArrayEmbeddingProvider`.
    pub fn new(provider: Arc<dyn MultiArrayEmbeddingProvider>) -> Self {
        Self { provider }
    }
}

#[async_trait]
impl EmbeddingProvider for MultiArrayEmbeddingAdapter {
    async fn embed_all(&self, content: &str) -> Result<TeleologicalArray, EmbedderError> {
        let output = self.provider.embed_all(content).await.map_err(|e| {
            EmbedderError::ComputationFailed {
                message: e.to_string(),
            }
        })?;

        Ok(output.fingerprint)
    }
}

// ============================================================================
// Test Embedding Provider (test-utils feature only)
// ============================================================================

/// Test embedding provider that returns zeroed TeleologicalArrays.
///
/// # ⚠️ TEST ONLY
///
/// This provider returns zeroed embeddings which:
/// - Pass dimension validation
/// - Have zero magnitude (undefined cosine similarity)
/// - Should NEVER be used in production
///
/// For production, use the GPU embedding pipeline from Phase 2+.
#[cfg(any(test, feature = "test-utils"))]
pub struct TestEmbeddingProvider;

#[cfg(any(test, feature = "test-utils"))]
#[async_trait]
impl EmbeddingProvider for TestEmbeddingProvider {
    async fn embed_all(&self, _content: &str) -> Result<TeleologicalArray, EmbedderError> {
        use crate::types::fingerprint::SemanticFingerprint;
        Ok(SemanticFingerprint::zeroed())
    }
}

/// Test embedding provider that always fails.
///
/// Use this to test error propagation paths.
#[cfg(any(test, feature = "test-utils"))]
pub struct FailingEmbeddingProvider {
    /// The error to return.
    pub error: EmbedderError,
}

#[cfg(any(test, feature = "test-utils"))]
impl FailingEmbeddingProvider {
    /// Create a provider that returns Unavailable error.
    pub fn unavailable() -> Self {
        Self {
            error: EmbedderError::Unavailable,
        }
    }

    /// Create a provider that returns ComputationFailed error.
    pub fn computation_failed(message: impl Into<String>) -> Self {
        Self {
            error: EmbedderError::ComputationFailed {
                message: message.into(),
            },
        }
    }
}

#[cfg(any(test, feature = "test-utils"))]
#[async_trait]
impl EmbeddingProvider for FailingEmbeddingProvider {
    async fn embed_all(&self, _content: &str) -> Result<TeleologicalArray, EmbedderError> {
        Err(self.error.clone())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    // Helper to create test service
    async fn test_service() -> (MemoryCaptureService, Arc<MemoryStore>, tempfile::TempDir) {
        let dir = tempdir().expect("create temp dir");
        let store = Arc::new(MemoryStore::new(dir.path()).expect("create store"));
        let embedder = Arc::new(TestEmbeddingProvider);
        let service = MemoryCaptureService::new(store.clone(), embedder);
        (service, store, dir)
    }

    #[tokio::test]
    async fn test_capture_hook_description_success() {
        let (service, store, _dir) = test_service().await;

        let result = service
            .capture_hook_description(
                "Claude edited the config file".to_string(),
                HookType::PostToolUse,
                "sess-001".to_string(),
                Some("Edit".to_string()),
            )
            .await;

        assert!(result.is_ok(), "capture should succeed: {:?}", result);
        let uuid = result.expect("should have uuid");

        // Verify persistence
        let memory = store.get(uuid).expect("get memory").expect("memory exists");
        assert_eq!(memory.content, "Claude edited the config file");
        assert!(matches!(
            memory.source,
            MemorySource::HookDescription {
                hook_type: HookType::PostToolUse,
                tool_name: Some(ref name)
            } if name == "Edit"
        ));
        assert_eq!(memory.session_id, "sess-001");
        assert_eq!(memory.word_count, 5);
    }

    #[tokio::test]
    async fn test_capture_claude_response_success() {
        let (service, store, _dir) = test_service().await;

        let result = service
            .capture_claude_response(
                "Session completed successfully with 5 tasks done".to_string(),
                ResponseType::SessionSummary,
                "sess-002".to_string(),
            )
            .await;

        assert!(result.is_ok(), "capture should succeed: {:?}", result);
        let uuid = result.expect("should have uuid");

        let memory = store.get(uuid).expect("get").expect("exists");
        assert!(matches!(
            memory.source,
            MemorySource::ClaudeResponse {
                response_type: ResponseType::SessionSummary
            }
        ));
    }

    #[tokio::test]
    async fn test_capture_md_chunk_success() {
        let (service, store, _dir) = test_service().await;

        let chunk = TextChunk::new(
            "# Documentation\n\nThis is the documentation content.".to_string(),
            ChunkMetadata {
                file_path: "docs/README.md".to_string(),
                chunk_index: 0,
                total_chunks: 3,
                word_offset: 0,
                char_offset: 0,
                original_file_hash: "sha256:abc123".to_string(),
                start_line: 1,
                end_line: 3,
            },
        );

        let result = service
            .capture_md_chunk(chunk, "sess-003".to_string())
            .await;

        assert!(result.is_ok(), "capture should succeed: {:?}", result);
        let uuid = result.expect("should have uuid");

        let memory = store.get(uuid).expect("get").expect("exists");
        assert!(matches!(
            memory.source,
            MemorySource::MDFileChunk {
                ref file_path,
                chunk_index: 0,
                total_chunks: 3
            } if file_path == "docs/README.md"
        ));
        assert!(memory.chunk_metadata.is_some());
        let meta = memory.chunk_metadata.expect("should have chunk_metadata");
        assert_eq!(meta.file_path, "docs/README.md");
        assert_eq!(meta.chunk_index, 0);
        assert_eq!(meta.total_chunks, 3);
    }

    #[tokio::test]
    async fn test_capture_empty_content_fails() {
        let (service, store, _dir) = test_service().await;
        let count_before = store.count().expect("count");

        let result = service
            .capture_hook_description(
                String::new(),
                HookType::SessionStart,
                "sess-empty".to_string(),
                None,
            )
            .await;

        assert!(matches!(result, Err(CaptureError::EmptyContent)));
        assert_eq!(store.count().expect("count"), count_before);
    }

    #[tokio::test]
    async fn test_capture_whitespace_only_fails() {
        let (service, store, _dir) = test_service().await;
        let count_before = store.count().expect("count");

        let result = service
            .capture_hook_description(
                "   \n\t  \r\n   ".to_string(),
                HookType::UserPromptSubmit,
                "sess-ws".to_string(),
                None,
            )
            .await;

        assert!(matches!(result, Err(CaptureError::EmptyContent)));
        assert_eq!(store.count().expect("count"), count_before);
    }

    #[tokio::test]
    async fn test_capture_content_at_max_length_succeeds() {
        let (service, _store, _dir) = test_service().await;

        let content = "x".repeat(MAX_CONTENT_LENGTH);
        let result = service
            .capture_hook_description(content, HookType::Stop, "sess-boundary".to_string(), None)
            .await;

        assert!(
            result.is_ok(),
            "capture should succeed at max length: {:?}",
            result
        );
    }

    #[tokio::test]
    async fn test_capture_content_over_max_length_fails() {
        let (service, store, _dir) = test_service().await;
        let count_before = store.count().expect("count");

        let content = "x".repeat(MAX_CONTENT_LENGTH + 1);
        let result = service
            .capture_hook_description(content, HookType::Stop, "sess-toolong".to_string(), None)
            .await;

        assert!(
            matches!(
                result,
                Err(CaptureError::ContentTooLong {
                    max: 10000,
                    actual: 10001
                })
            ),
            "should fail with ContentTooLong: {:?}",
            result
        );
        assert_eq!(store.count().expect("count"), count_before);
    }

    #[tokio::test]
    async fn test_embedding_error_propagates() {
        let dir = tempdir().expect("create temp dir");
        let store = Arc::new(MemoryStore::new(dir.path()).expect("create store"));
        let embedder = Arc::new(FailingEmbeddingProvider::unavailable());
        let service = MemoryCaptureService::new(store.clone(), embedder);

        let count_before = store.count().expect("count");

        let result = service
            .capture_hook_description(
                "Valid content".to_string(),
                HookType::SessionStart,
                "sess-fail".to_string(),
                None,
            )
            .await;

        assert!(
            matches!(
                result,
                Err(CaptureError::EmbeddingFailed(EmbedderError::Unavailable))
            ),
            "should propagate embedding error: {:?}",
            result
        );
        assert_eq!(store.count().expect("count"), count_before);
    }

    #[tokio::test]
    async fn test_memory_indexed_by_session() {
        let (service, store, _dir) = test_service().await;

        let session_id = "sess-index-test";

        // Capture multiple memories for same session
        let uuid1 = service
            .capture_hook_description(
                "First memory".to_string(),
                HookType::SessionStart,
                session_id.to_string(),
                None,
            )
            .await
            .expect("capture 1");

        let uuid2 = service
            .capture_claude_response(
                "Second memory".to_string(),
                ResponseType::StopResponse,
                session_id.to_string(),
            )
            .await
            .expect("capture 2");

        // Verify session index
        let session_memories = store.get_by_session(session_id).expect("get by session");

        assert_eq!(session_memories.len(), 2);
        let ids: Vec<_> = session_memories.iter().map(|m| m.id).collect();
        assert!(ids.contains(&uuid1));
        assert!(ids.contains(&uuid2));
    }

    #[tokio::test]
    async fn test_multiple_sessions_isolated() {
        let (service, store, _dir) = test_service().await;

        // Capture to different sessions
        service
            .capture_hook_description(
                "Session A memory".to_string(),
                HookType::SessionStart,
                "sess-A".to_string(),
                None,
            )
            .await
            .expect("capture A");

        service
            .capture_hook_description(
                "Session B memory".to_string(),
                HookType::SessionStart,
                "sess-B".to_string(),
                None,
            )
            .await
            .expect("capture B");

        // Verify isolation
        let a_memories = store.get_by_session("sess-A").expect("get A");
        let b_memories = store.get_by_session("sess-B").expect("get B");

        assert_eq!(a_memories.len(), 1);
        assert_eq!(b_memories.len(), 1);
        assert_eq!(a_memories[0].content, "Session A memory");
        assert_eq!(b_memories[0].content, "Session B memory");
    }

    // ========== EDGE CASE TESTS ==========

    #[tokio::test]
    async fn edge_case_empty_database_capture() {
        println!("=== EDGE CASE: Capture to empty database ===");
        let (service, store, _dir) = test_service().await;

        println!("BEFORE: count = {}", store.count().expect("count"));
        assert_eq!(store.count().expect("count"), 0);

        let uuid = service
            .capture_hook_description(
                "First memory ever".to_string(),
                HookType::SessionStart,
                "first-session".to_string(),
                None,
            )
            .await
            .expect("capture");

        println!("AFTER: count = {}", store.count().expect("count"));
        assert_eq!(store.count().expect("count"), 1);

        let memory = store.get(uuid).expect("get").expect("exists");
        println!(
            "Captured memory: id={}, content='{}'",
            memory.id, memory.content
        );
        assert_eq!(memory.content, "First memory ever");

        println!("RESULT: PASS - First capture to empty database works");
    }

    #[tokio::test]
    async fn edge_case_multiple_captures_same_session() {
        println!("=== EDGE CASE: Multiple captures to same session ===");
        let (service, store, _dir) = test_service().await;

        let session_id = "multi-session";

        // Capture 5 memories to the same session
        let mut results = Vec::new();
        for i in 0..5 {
            let result = service
                .capture_hook_description(
                    format!("Memory number {}", i),
                    HookType::PostToolUse,
                    session_id.to_string(),
                    Some(format!("Tool{}", i)),
                )
                .await;
            results.push(result);
        }

        println!(
            "Capture results: {} successful",
            results.iter().filter(|r| r.is_ok()).count()
        );

        // All should succeed
        for (i, result) in results.iter().enumerate() {
            assert!(result.is_ok(), "Capture {} should succeed: {:?}", i, result);
        }

        // All 5 should be in the session
        let session_memories = store.get_by_session(session_id).expect("get by session");
        println!("AFTER: session has {} memories", session_memories.len());
        assert_eq!(session_memories.len(), 5);

        println!("RESULT: PASS - Multiple captures to same session handled correctly");
    }

    #[tokio::test]
    async fn edge_case_special_characters_in_content() {
        println!("=== EDGE CASE: Special characters in content ===");
        let (service, store, _dir) = test_service().await;

        let special_content =
            "Content with special chars: 日本語 🚀 <script>alert('xss')</script> \n\t\r";
        let result = service
            .capture_hook_description(
                special_content.to_string(),
                HookType::UserPromptSubmit,
                "special-session".to_string(),
                None,
            )
            .await;

        assert!(result.is_ok(), "Should handle special chars: {:?}", result);
        let uuid = result.expect("uuid");

        let memory = store.get(uuid).expect("get").expect("exists");
        println!("Stored content: '{}'", memory.content);
        assert_eq!(memory.content, special_content);

        println!("RESULT: PASS - Special characters preserved correctly");
    }

    // ========== FULL STATE VERIFICATION TEST ==========

    #[tokio::test]
    async fn fsv_verify_capture_persistence() {
        println!("\n============================================================");
        println!("=== FSV: MemoryCaptureService Persistence Verification ===");
        println!("============================================================\n");

        let dir = tempdir().expect("create temp dir");
        let db_path = dir.path();

        // Phase 1: Capture memories
        let (uuid1, uuid2, uuid3);
        {
            let store = Arc::new(MemoryStore::new(db_path).expect("create store"));
            let embedder = Arc::new(TestEmbeddingProvider);
            let service = MemoryCaptureService::new(store.clone(), embedder);

            println!("[FSV-1] Initial count: {}", store.count().expect("count"));
            assert_eq!(store.count().expect("count"), 0);

            // Capture hook description
            uuid1 = service
                .capture_hook_description(
                    "FSV: Hook description memory".to_string(),
                    HookType::PostToolUse,
                    "fsv-session".to_string(),
                    Some("Edit".to_string()),
                )
                .await
                .expect("capture hook");
            println!("[FSV-2] Captured hook description: {}", uuid1);

            // Capture Claude response
            uuid2 = service
                .capture_claude_response(
                    "FSV: Claude response memory".to_string(),
                    ResponseType::SessionSummary,
                    "fsv-session".to_string(),
                )
                .await
                .expect("capture response");
            println!("[FSV-3] Captured Claude response: {}", uuid2);

            // Capture MD chunk
            let chunk = TextChunk::new(
                "FSV: MD chunk memory content".to_string(),
                ChunkMetadata {
                    file_path: "fsv/test.md".to_string(),
                    chunk_index: 0,
                    total_chunks: 1,
                    word_offset: 0,
                    char_offset: 0,
                    original_file_hash: "fsv_hash_123".to_string(),
                    start_line: 1,
                    end_line: 1,
                },
            );
            uuid3 = service
                .capture_md_chunk(chunk, "fsv-session".to_string())
                .await
                .expect("capture chunk");
            println!("[FSV-4] Captured MD chunk: {}", uuid3);

            println!("[FSV-5] Final count: {}", store.count().expect("count"));
            assert_eq!(store.count().expect("count"), 3);
        }
        // Store dropped, DB closed

        // Phase 2: Reopen and verify persistence
        println!("\n[FSV-6] Reopening database to verify persistence...");
        {
            let store = MemoryStore::new(db_path).expect("reopen store");

            let count = store.count().expect("count");
            println!("[FSV-7] Reopened count: {}", count);
            assert_eq!(count, 3, "All 3 memories should persist");

            // Verify each memory by ID
            let mem1 = store.get(uuid1).expect("get 1").expect("mem1 exists");
            let mem2 = store.get(uuid2).expect("get 2").expect("mem2 exists");
            let mem3 = store.get(uuid3).expect("get 3").expect("mem3 exists");

            println!(
                "[FSV-8] Memory 1: source={:?}, content='{}'",
                mem1.source, mem1.content
            );
            println!(
                "[FSV-9] Memory 2: source={:?}, content='{}'",
                mem2.source, mem2.content
            );
            println!(
                "[FSV-10] Memory 3: source={:?}, has_chunk_metadata={}",
                mem3.source,
                mem3.chunk_metadata.is_some()
            );

            // Verify content integrity
            assert_eq!(mem1.content, "FSV: Hook description memory");
            assert!(matches!(mem1.source, MemorySource::HookDescription { .. }));

            assert_eq!(mem2.content, "FSV: Claude response memory");
            assert!(matches!(mem2.source, MemorySource::ClaudeResponse { .. }));

            assert_eq!(mem3.content, "FSV: MD chunk memory content");
            assert!(matches!(mem3.source, MemorySource::MDFileChunk { .. }));
            assert!(mem3.chunk_metadata.is_some());

            // Verify session index
            let session_memories = store.get_by_session("fsv-session").expect("get session");
            println!(
                "[FSV-11] Session 'fsv-session' has {} memories",
                session_memories.len()
            );
            assert_eq!(session_memories.len(), 3);

            // Verify all UUIDs are in session
            let ids: Vec<_> = session_memories.iter().map(|m| m.id).collect();
            assert!(ids.contains(&uuid1), "Session should contain uuid1");
            assert!(ids.contains(&uuid2), "Session should contain uuid2");
            assert!(ids.contains(&uuid3), "Session should contain uuid3");
        }

        println!("\n============================================================");
        println!("[FSV] VERIFIED: All capture persistence checks passed");
        println!("============================================================\n");
    }

    // ========== SYNTHETIC TEST DATA FROM TASK SPEC ==========

    #[tokio::test]
    async fn synthetic_syn_001_hook_description_valid() {
        // SYN-001: Valid hook description
        let (service, store, _dir) = test_service().await;

        let content = "Claude used the Edit tool to modify src/main.rs, adding a new function called process_data that handles incoming API requests.";
        let uuid = service
            .capture_hook_description(
                content.to_string(),
                HookType::PostToolUse,
                "sess_abc123".to_string(),
                Some("Edit".to_string()),
            )
            .await
            .expect("SYN-001 should succeed");

        let memory = store.get(uuid).expect("get").expect("exists");
        assert_eq!(memory.content, content);
        assert!(matches!(
            memory.source,
            MemorySource::HookDescription {
                hook_type: HookType::PostToolUse,
                tool_name: Some(ref name)
            } if name == "Edit"
        ));
        assert_eq!(memory.session_id, "sess_abc123");
        assert_eq!(memory.word_count, 19); // "Claude used the Edit tool to modify src/main.rs, adding a new function called process_data that handles incoming API requests."
        assert!(memory.chunk_metadata.is_none());
    }

    #[tokio::test]
    async fn synthetic_syn_002_claude_response_valid() {
        // SYN-002: Valid Claude response
        let (service, store, _dir) = test_service().await;

        let content = "I've completed the implementation of the authentication module. The key changes include: 1) Added JWT token validation, 2) Implemented refresh token rotation, 3) Added rate limiting for login attempts.";
        let uuid = service
            .capture_claude_response(
                content.to_string(),
                ResponseType::SessionSummary,
                "sess_xyz789".to_string(),
            )
            .await
            .expect("SYN-002 should succeed");

        let memory = store.get(uuid).expect("get").expect("exists");
        assert_eq!(memory.content, content);
        assert!(matches!(
            memory.source,
            MemorySource::ClaudeResponse {
                response_type: ResponseType::SessionSummary
            }
        ));
        assert_eq!(memory.session_id, "sess_xyz789");
    }

    #[tokio::test]
    async fn synthetic_syn_003_md_chunk_valid() {
        // SYN-003: Valid MD chunk
        let (service, store, _dir) = test_service().await;

        let content = "## Authentication Flow\n\nThe system uses JWT tokens for stateless authentication. When a user logs in, they receive an access token (15 min expiry) and a refresh token (7 day expiry).";
        let chunk = TextChunk::new(
            content.to_string(),
            ChunkMetadata {
                file_path: "docs/auth.md".to_string(),
                chunk_index: 0,
                total_chunks: 5,
                word_offset: 0,
                char_offset: 0,
                original_file_hash: "sha256:abc123def456".to_string(),
                start_line: 1,
                end_line: 3,
            },
        );

        let uuid = service
            .capture_md_chunk(chunk, "sess_chunk001".to_string())
            .await
            .expect("SYN-003 should succeed");

        let memory = store.get(uuid).expect("get").expect("exists");
        assert!(matches!(
            memory.source,
            MemorySource::MDFileChunk {
                ref file_path,
                chunk_index: 0,
                total_chunks: 5
            } if file_path == "docs/auth.md"
        ));
        assert!(memory.chunk_metadata.is_some());
        let meta = memory.chunk_metadata.as_ref().expect("metadata");
        assert_eq!(meta.file_path, "docs/auth.md");
    }

    #[tokio::test]
    async fn synthetic_syn_004_empty_content_rejected() {
        // SYN-004: Empty content rejected
        let (service, store, _dir) = test_service().await;
        let count_before = store.count().expect("count");

        let result = service
            .capture_hook_description(
                String::new(),
                HookType::SessionStart,
                "sess_empty".to_string(),
                None,
            )
            .await;

        assert!(matches!(result, Err(CaptureError::EmptyContent)));
        // No storage call made
        assert_eq!(store.count().expect("count"), count_before);
    }

    #[tokio::test]
    async fn synthetic_syn_005_whitespace_only_rejected() {
        // SYN-005: Whitespace-only content rejected
        let (service, store, _dir) = test_service().await;
        let count_before = store.count().expect("count");

        let result = service
            .capture_hook_description(
                "   \n\t  \r\n   ".to_string(),
                HookType::UserPromptSubmit,
                "sess_ws".to_string(),
                None,
            )
            .await;

        assert!(matches!(result, Err(CaptureError::EmptyContent)));
        assert_eq!(store.count().expect("count"), count_before);
    }

    #[tokio::test]
    async fn synthetic_syn_006_max_length_boundary() {
        // SYN-006: Content at exactly MAX_CONTENT_LENGTH succeeds
        let (service, store, _dir) = test_service().await;

        let content = "x".repeat(MAX_CONTENT_LENGTH);
        assert_eq!(content.len(), 10_000);

        let uuid = service
            .capture_hook_description(content, HookType::Stop, "sess_boundary".to_string(), None)
            .await
            .expect("SYN-006 should succeed");

        let memory = store.get(uuid).expect("get").expect("exists");
        assert_eq!(memory.content.len(), 10_000);
        assert_eq!(memory.word_count, 1); // One giant "word"
    }

    #[tokio::test]
    async fn synthetic_syn_007_over_max_length_rejected() {
        // SYN-007: Content over MAX_CONTENT_LENGTH rejected
        let (service, store, _dir) = test_service().await;
        let count_before = store.count().expect("count");

        let content = "x".repeat(MAX_CONTENT_LENGTH + 1);
        assert_eq!(content.len(), 10_001);

        let result = service
            .capture_hook_description(content, HookType::Stop, "sess_toolong".to_string(), None)
            .await;

        assert!(
            matches!(
                result,
                Err(CaptureError::ContentTooLong {
                    max: 10000,
                    actual: 10001
                })
            ),
            "SYN-007 should fail with ContentTooLong: {:?}",
            result
        );
        assert_eq!(store.count().expect("count"), count_before);
    }
}
