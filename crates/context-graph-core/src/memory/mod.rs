//! Memory capture types for the Context Graph system.
//!
//! This module provides the core data types for memory capture:
//! - [`Memory`] - Primary memory unit with 13-embedding TeleologicalArray
//! - [`MemorySource`] - Discriminated source type (Hook, Response, MDChunk)
//! - [`HookType`] - Hook event types per .claude/settings.json
//! - [`ResponseType`] - Claude response capture types
//!
//! # Constitution Compliance
//! - ARCH-01: TeleologicalArray is atomic (all 13 embeddings or nothing)
//! - ARCH-05: All 13 embedders required
//! - ARCH-11: Memory sources: HookDescription, ClaudeResponse, MDFileChunk
//!
//! # Example
//! ```ignore
//! use context_graph_core::memory::{Memory, MemorySource, HookType};
//! use context_graph_core::types::fingerprint::TeleologicalArray;
//!
//! // TeleologicalArray must come from real embedding pipeline
//! let teleological_array: TeleologicalArray = embed_pipeline.embed_all(&content).await?;
//!
//! let memory = Memory::new(
//!     "Claude edited the config file".to_string(),
//!     MemorySource::HookDescription {
//!         hook_type: HookType::PostToolUse,
//!         tool_name: Some("Edit".to_string()),
//!     },
//!     "session-123".to_string(),
//!     teleological_array,
//!     None,
//! );
//! ```

pub mod ast;
pub mod ast_chunker;
pub mod capture;
pub mod chunker;
pub mod code_capture;
pub mod code_watcher;
pub mod manager;
pub mod session;
pub mod source;
pub mod store;
pub mod watcher;

pub use ast::{
    chunk as chunk_ast_source, chunk_path as chunk_ast_path, route_for_entity_type, route_for_key,
    route_to_embedders, routing_table_entries, try_route_to_embedders, AstChunk, AstChunkOptions,
    AstChunkerError as MultiAstChunkerError, DirectInstrument, EmbedderId,
    EntityType as AstEntityType, Language as AstLanguage,
    ParseDiagnostic as MultiAstParseDiagnostic, RoutingError, RoutingKey, RoutingResult,
};
#[allow(deprecated)]
pub use ast_chunker::{
    AstChunkConfig, AstChunkerError, AstCodeChunker, AstParseDiagnostic, CodeChunk,
    CodeChunkMetadata, EntityType,
};
pub use capture::{
    CaptureError, EmbedderError, EmbeddingProvider, MemoryCaptureService,
    MultiArrayEmbeddingAdapter,
};
pub use chunker::{ChunkerError, TextChunk, TextChunker};
pub use code_capture::{
    CodeCaptureError, CodeCaptureResult, CodeCaptureService, CodeEmbedderError,
    CodeEmbeddingProvider, CodeSearchResult, CodeStorage,
};
pub use code_watcher::{CodeFileWatcher, CodeWatcherError, WatcherStats};
// ChunkMetadata is defined in this file and exported directly
pub use manager::{SessionError, SessionManager, CF_SESSIONS};
pub use session::{Session, SessionStatus};
pub use source::{HookType, MemorySource, ResponseType};
pub use store::{MemoryStore, StorageError};
pub use watcher::{GitFileWatcher, WatcherError};

#[cfg(any(test, feature = "test-utils"))]
pub use capture::{FailingEmbeddingProvider, TestEmbeddingProvider};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::fingerprint::TeleologicalArray;

/// Metadata for a chunk of text extracted from a source file.
///
/// Stores positional information including line numbers for context injection.
/// Line numbers are 1-based (matching editor conventions).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChunkMetadata {
    /// Path to the source file.
    pub file_path: String,
    /// Zero-based chunk index.
    pub chunk_index: u32,
    /// Total chunks from source file.
    pub total_chunks: u32,
    /// Word offset from start of file.
    pub word_offset: u32,
    /// Character offset from start of file.
    pub char_offset: u32,
    /// SHA256 hash of original file content.
    pub original_file_hash: String,
    /// Starting line number in the source file (1-based).
    pub start_line: u32,
    /// Ending line number in the source file (1-based, inclusive).
    pub end_line: u32,
}

/// Memory: The primary data unit for captured memories.
///
/// Each Memory contains:
/// - Unique identifier (UUID)
/// - Content text
/// - Discriminated source type (MemorySource)
/// - Full 13-embedding TeleologicalArray
/// - Session association
/// - Optional chunk metadata for MDFileChunk sources
///
/// # Constitution Compliance
/// - ARCH-01: TeleologicalArray is atomic storage unit
/// - ARCH-05: All 13 embedders required - TeleologicalArray enforces this
/// - ARCH-11: Three source types: HookDescription, ClaudeResponse, MDFileChunk
///
/// # Storage
/// Typical size: ~46KB (TeleologicalArray) + content + metadata
/// Per constitution.yaml embeddings.paradigm: "NO FUSION - Store all 13 embeddings"
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    /// Unique identifier (UUID v4).
    pub id: Uuid,

    /// The actual content/knowledge being stored.
    /// Max 10,000 characters per TECH-PHASE1 spec.
    pub content: String,

    /// Discriminated source type indicating memory origin.
    pub source: MemorySource,

    /// Timestamp when this memory was created.
    pub created_at: DateTime<Utc>,

    /// Session identifier this memory belongs to.
    pub session_id: String,

    /// Full 13-embedding array (ARCH-01: atomic storage unit).
    /// MUST contain valid embeddings for all 13 spaces.
    pub teleological_array: TeleologicalArray,

    /// Optional chunk metadata for MDFileChunk sources.
    pub chunk_metadata: Option<ChunkMetadata>,

    /// Word count of content (for chunking/stats).
    pub word_count: u32,
}

/// Maximum content length in characters (10,000 per TECH-PHASE1 spec).
pub const MAX_CONTENT_LENGTH: usize = 10_000;

impl Memory {
    /// Create a new Memory with generated UUID and current timestamp.
    ///
    /// # Arguments
    /// * `content` - The text content to store
    /// * `source` - Discriminated source type
    /// * `session_id` - Session this memory belongs to
    /// * `teleological_array` - Full 13-embedding array (MUST be valid)
    /// * `chunk_metadata` - Optional chunk info for MDFileChunk sources
    ///
    /// # Panics
    /// Does NOT panic - validation should be done by caller via `validate()`.
    pub fn new(
        content: String,
        source: MemorySource,
        session_id: String,
        teleological_array: TeleologicalArray,
        chunk_metadata: Option<ChunkMetadata>,
    ) -> Self {
        let word_count = content.split_whitespace().count() as u32;

        Self {
            id: Uuid::new_v4(),
            content,
            source,
            created_at: Utc::now(),
            session_id,
            teleological_array,
            chunk_metadata,
            word_count,
        }
    }

    /// Create Memory with a specific UUID (for testing/reconstruction).
    pub fn with_id(
        id: Uuid,
        content: String,
        source: MemorySource,
        session_id: String,
        teleological_array: TeleologicalArray,
        chunk_metadata: Option<ChunkMetadata>,
    ) -> Self {
        let word_count = content.split_whitespace().count() as u32;

        Self {
            id,
            content,
            source,
            created_at: Utc::now(),
            session_id,
            teleological_array,
            chunk_metadata,
            word_count,
        }
    }

    /// Create Memory with a specific UUID and timestamp (for reconstruction from storage).
    pub fn with_id_and_timestamp(
        id: Uuid,
        content: String,
        source: MemorySource,
        session_id: String,
        teleological_array: TeleologicalArray,
        chunk_metadata: Option<ChunkMetadata>,
        created_at: DateTime<Utc>,
    ) -> Self {
        let word_count = content.split_whitespace().count() as u32;

        Self {
            id,
            content,
            source,
            created_at,
            session_id,
            teleological_array,
            chunk_metadata,
            word_count,
        }
    }

    /// Validate the Memory struct.
    ///
    /// # Checks
    /// - Content is not empty and <= 10,000 characters
    /// - Session ID is not empty
    /// - TeleologicalArray passes strict validation (all 13 embeddings valid)
    /// - MDFileChunk sources have chunk_metadata
    /// - Word count matches actual content
    pub fn validate(&self) -> Result<(), String> {
        if self.content.is_empty() {
            return Err("Memory content cannot be empty".to_string());
        }

        if self.content.len() > MAX_CONTENT_LENGTH {
            return Err(format!(
                "Memory content exceeds {MAX_CONTENT_LENGTH} chars: {} chars",
                self.content.len()
            ));
        }

        if self.session_id.is_empty() {
            return Err("Session ID cannot be empty".to_string());
        }

        self.teleological_array
            .validate_strict()
            .map_err(|e| format!("TeleologicalArray validation failed: {e}"))?;

        if matches!(self.source, MemorySource::MDFileChunk { .. }) && self.chunk_metadata.is_none()
        {
            return Err("MDFileChunk source requires chunk_metadata".to_string());
        }

        let actual_word_count = self.content.split_whitespace().count() as u32;
        if self.word_count != actual_word_count {
            return Err(format!(
                "Word count mismatch: stored {} but actual is {actual_word_count}",
                self.word_count
            ));
        }

        Ok(())
    }

    /// Check if this memory is from a hook event.
    pub fn is_hook_description(&self) -> bool {
        matches!(self.source, MemorySource::HookDescription { .. })
    }

    /// Check if this memory is from a Claude response.
    pub fn is_claude_response(&self) -> bool {
        matches!(self.source, MemorySource::ClaudeResponse { .. })
    }

    /// Check if this memory is from an MD file chunk.
    pub fn is_md_file_chunk(&self) -> bool {
        matches!(self.source, MemorySource::MDFileChunk { .. })
    }

    /// Get the hook type if this is a HookDescription source.
    pub fn hook_type(&self) -> Option<HookType> {
        match &self.source {
            MemorySource::HookDescription { hook_type, .. } => Some(*hook_type),
            _ => None,
        }
    }

    /// Get the response type if this is a ClaudeResponse source.
    pub fn response_type(&self) -> Option<ResponseType> {
        match &self.source {
            MemorySource::ClaudeResponse { response_type } => Some(*response_type),
            _ => None,
        }
    }

    /// Get the tool name if this is a HookDescription source with a tool.
    pub fn tool_name(&self) -> Option<&str> {
        match &self.source {
            MemorySource::HookDescription { tool_name, .. } => tool_name.as_deref(),
            _ => None,
        }
    }

    /// Estimate memory size in bytes.
    pub fn estimated_size(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.content.len()
            + self.teleological_array.storage_size()
            + self.session_id.len()
            + self
                .chunk_metadata
                .as_ref()
                .map_or(0, |m| m.file_path.len() + m.original_file_hash.len() + 16)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "test-utils")]
    use crate::types::fingerprint::SemanticFingerprint;

    // Helper to create valid test fingerprint (TEST ONLY)
    #[cfg(feature = "test-utils")]
    fn test_fingerprint() -> TeleologicalArray {
        SemanticFingerprint::zeroed()
    }

    #[cfg(feature = "test-utils")]
    #[test]
    fn test_memory_new_generates_uuid() {
        let fp = test_fingerprint();
        let memory = Memory::new(
            "test content".to_string(),
            MemorySource::HookDescription {
                hook_type: HookType::SessionStart,
                tool_name: None,
            },
            "session-1".to_string(),
            fp,
            None,
        );

        assert!(!memory.id.is_nil());
        assert_eq!(memory.content, "test content");
        assert_eq!(memory.word_count, 2);
    }

    #[cfg(feature = "test-utils")]
    #[test]
    fn test_memory_source_detection() {
        let fp = test_fingerprint();

        let hook_mem = Memory::new(
            "hook content".to_string(),
            MemorySource::HookDescription {
                hook_type: HookType::PostToolUse,
                tool_name: Some("Edit".to_string()),
            },
            "session".to_string(),
            fp.clone(),
            None,
        );
        assert!(hook_mem.is_hook_description());
        assert!(!hook_mem.is_claude_response());
        assert!(!hook_mem.is_md_file_chunk());
        assert_eq!(hook_mem.hook_type(), Some(HookType::PostToolUse));
        assert_eq!(hook_mem.tool_name(), Some("Edit"));

        let response_mem = Memory::new(
            "response content".to_string(),
            MemorySource::ClaudeResponse {
                response_type: ResponseType::SessionSummary,
            },
            "session".to_string(),
            fp.clone(),
            None,
        );
        assert!(!response_mem.is_hook_description());
        assert!(response_mem.is_claude_response());
        assert!(response_mem.hook_type().is_none());
        assert_eq!(
            response_mem.response_type(),
            Some(ResponseType::SessionSummary)
        );

        let chunk_mem = Memory::new(
            "chunk content".to_string(),
            MemorySource::MDFileChunk {
                file_path: "test.md".to_string(),
                chunk_index: 0,
                total_chunks: 1,
            },
            "session".to_string(),
            fp,
            Some(ChunkMetadata {
                file_path: "test.md".to_string(),
                chunk_index: 0,
                total_chunks: 1,
                word_offset: 0,
                char_offset: 0,
                original_file_hash: "abc123".to_string(),
                start_line: 1,
                end_line: 1,
            }),
        );
        assert!(chunk_mem.is_md_file_chunk());
    }

    #[cfg(feature = "test-utils")]
    #[test]
    fn test_memory_validation_empty_content() {
        let fp = test_fingerprint();
        let memory = Memory::new(
            String::new(),
            MemorySource::HookDescription {
                hook_type: HookType::SessionStart,
                tool_name: None,
            },
            "session".to_string(),
            fp,
            None,
        );

        let result = memory.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    #[cfg(feature = "test-utils")]
    #[test]
    fn test_memory_validation_empty_session() {
        let fp = test_fingerprint();
        let memory = Memory::new(
            "content".to_string(),
            MemorySource::HookDescription {
                hook_type: HookType::SessionStart,
                tool_name: None,
            },
            String::new(),
            fp,
            None,
        );

        let result = memory.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Session"));
    }

    #[cfg(feature = "test-utils")]
    #[test]
    fn test_memory_validation_content_too_long() {
        let fp = test_fingerprint();
        let long_content = "x".repeat(MAX_CONTENT_LENGTH + 1);
        let memory = Memory::new(
            long_content,
            MemorySource::HookDescription {
                hook_type: HookType::SessionStart,
                tool_name: None,
            },
            "session".to_string(),
            fp,
            None,
        );

        let result = memory.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("exceeds"),
            "Error should mention exceeds: {}",
            err
        );
    }

    #[cfg(feature = "test-utils")]
    #[test]
    fn test_memory_validation_mdfilechunk_missing_metadata() {
        let fp = test_fingerprint();
        let memory = Memory::new(
            "chunk content".to_string(),
            MemorySource::MDFileChunk {
                file_path: "test.md".to_string(),
                chunk_index: 0,
                total_chunks: 1,
            },
            "session".to_string(),
            fp,
            None, // Missing chunk_metadata!
        );

        let result = memory.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("chunk_metadata"),
            "Error should mention chunk_metadata: {}",
            err
        );
    }

    #[cfg(feature = "test-utils")]
    #[test]
    fn test_memory_validation_success() {
        let fp = test_fingerprint();
        let memory = Memory::new(
            "valid content".to_string(),
            MemorySource::HookDescription {
                hook_type: HookType::SessionStart,
                tool_name: None,
            },
            "session-123".to_string(),
            fp,
            None,
        );

        // Note: zeroed fingerprint passes dimension validation but has zero magnitude
        // In production, real embeddings would be used
        let result = memory.validate();
        assert!(result.is_ok(), "Validation should pass: {:?}", result);
    }

    #[cfg(feature = "test-utils")]
    #[test]
    fn test_memory_serialization_roundtrip() {
        let fp = test_fingerprint();
        let memory = Memory::new(
            "serialization test".to_string(),
            MemorySource::HookDescription {
                hook_type: HookType::UserPromptSubmit,
                tool_name: None,
            },
            "session-serialize".to_string(),
            fp,
            None,
        );

        // Test bincode serialization (used for storage)
        let bytes = bincode::serialize(&memory).expect("serialize failed");
        let restored: Memory = bincode::deserialize(&bytes).expect("deserialize failed");

        assert_eq!(memory.id, restored.id);
        assert_eq!(memory.content, restored.content);
        assert_eq!(memory.session_id, restored.session_id);
        assert_eq!(memory.word_count, restored.word_count);
    }

    #[test]
    fn test_chunk_metadata_fields() {
        let meta = ChunkMetadata {
            file_path: "/path/to/file.md".to_string(),
            chunk_index: 3,
            total_chunks: 10,
            word_offset: 600,
            char_offset: 4500,
            original_file_hash: "sha256hash".to_string(),
            start_line: 50,
            end_line: 75,
        };

        assert_eq!(meta.file_path, "/path/to/file.md");
        assert_eq!(meta.chunk_index, 3);
        assert_eq!(meta.total_chunks, 10);
        assert_eq!(meta.word_offset, 600);
        assert_eq!(meta.char_offset, 4500);
        assert_eq!(meta.original_file_hash, "sha256hash");
        assert_eq!(meta.start_line, 50);
        assert_eq!(meta.end_line, 75);
    }

    #[test]
    fn test_chunk_metadata_serialization() {
        let meta = ChunkMetadata {
            file_path: "test.md".to_string(),
            chunk_index: 0,
            total_chunks: 5,
            word_offset: 0,
            char_offset: 0,
            original_file_hash: "abc123".to_string(),
            start_line: 1,
            end_line: 10,
        };

        let bytes = bincode::serialize(&meta).expect("serialize failed");
        let restored: ChunkMetadata = bincode::deserialize(&bytes).expect("deserialize failed");
        assert_eq!(meta, restored);
    }

    #[cfg(feature = "test-utils")]
    #[test]
    fn test_memory_with_id() {
        let fp = test_fingerprint();
        let specific_id = Uuid::new_v4();
        let memory = Memory::with_id(
            specific_id,
            "test content".to_string(),
            MemorySource::HookDescription {
                hook_type: HookType::SessionStart,
                tool_name: None,
            },
            "session".to_string(),
            fp,
            None,
        );

        assert_eq!(memory.id, specific_id);
    }

    #[cfg(feature = "test-utils")]
    #[test]
    fn test_memory_estimated_size() {
        let fp = test_fingerprint();
        let memory = Memory::new(
            "test content for size estimation".to_string(),
            MemorySource::HookDescription {
                hook_type: HookType::SessionStart,
                tool_name: None,
            },
            "session-123".to_string(),
            fp,
            None,
        );

        let size = memory.estimated_size();
        // Size should be at least the dense portion of TeleologicalArray (~30KB for zeroed)
        // Note: zeroed fingerprint has empty sparse vectors and no tokens
        // Real embeddings would be larger (~46KB with sparse + tokens)
        assert!(size > 25_000, "Size should be > 25KB, got {}", size);

        // Verify the teleological array storage size is the major component
        let teleological_size = memory.teleological_array.storage_size();
        assert!(
            teleological_size > 25_000,
            "TeleologicalArray should be > 25KB, got {}",
            teleological_size
        );
    }

    #[cfg(feature = "test-utils")]
    #[test]
    fn test_word_count_calculation() {
        let fp = test_fingerprint();

        // Test various content patterns
        let test_cases = [
            ("single", 1),
            ("two words", 2),
            ("the quick brown fox jumps over the lazy dog", 9),
            ("  extra   spaces   everywhere  ", 3),
            ("", 0), // Empty string has 0 words
        ];

        for (content, expected_count) in test_cases {
            let memory = Memory::new(
                content.to_string(),
                MemorySource::HookDescription {
                    hook_type: HookType::SessionStart,
                    tool_name: None,
                },
                "session".to_string(),
                fp.clone(),
                None,
            );
            assert_eq!(
                memory.word_count, expected_count,
                "Word count for '{}' should be {}, got {}",
                content, expected_count, memory.word_count
            );
        }
    }

    #[cfg(feature = "test-utils")]
    #[test]
    fn test_memory_with_id_and_timestamp() {
        let fp = test_fingerprint();
        let specific_id = Uuid::new_v4();
        let specific_time = DateTime::parse_from_rfc3339("2025-01-15T10:00:00Z")
            .expect("parse time")
            .with_timezone(&Utc);

        let memory = Memory::with_id_and_timestamp(
            specific_id,
            "test content".to_string(),
            MemorySource::HookDescription {
                hook_type: HookType::SessionStart,
                tool_name: None,
            },
            "session".to_string(),
            fp,
            None,
            specific_time,
        );

        assert_eq!(memory.id, specific_id);
        assert_eq!(memory.created_at, specific_time);
    }
}
