//! Source metadata for tracking memory provenance.
//!
//! This module provides types for tracking where memories originate from,
//! enabling context injection to display source information (e.g., file path
//! for MDFileChunk memories).
//!
//! # Architecture
//!
//! SourceMetadata is stored alongside fingerprints in TeleologicalMemoryStore,
//! providing provenance tracking for all stored memories. This enables:
//!
//! - Context injection to show file paths for chunked markdown files
//! - Debugging and auditing of memory origins
//! - File-based invalidation and re-chunking
//!
//! # Source Types
//!
//! - `MDFileChunk`: From markdown file watcher with file path and chunk info
//! - `HookDescription`: From Claude Code hook events
//! - `ClaudeResponse`: From session end captured responses
//! - `Manual`: User-injected via MCP tools (no special metadata)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Source metadata for memory provenance tracking.
///
/// Stores information about where a memory originated from, enabling
/// context injection to display source information including file location
/// and line numbers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SourceMetadata {
    /// The type of source (MDFileChunk, HookDescription, etc.)
    pub source_type: SourceType,

    /// Optional file path (for MDFileChunk sources)
    pub file_path: Option<String>,

    /// Chunk index within file (0-based, for MDFileChunk)
    pub chunk_index: Option<u32>,

    /// Total chunks in file (for MDFileChunk)
    pub total_chunks: Option<u32>,

    /// Starting line number in source file (1-based, for MDFileChunk)
    pub start_line: Option<u32>,

    /// Ending line number in source file (1-based, inclusive, for MDFileChunk)
    pub end_line: Option<u32>,

    /// Optional hook type (for HookDescription)
    pub hook_type: Option<String>,

    /// Optional tool name (for HookDescription)
    pub tool_name: Option<String>,

    /// Session ID when memory was created (for temporal filtering).
    /// Enables filtering memories to a specific Claude Code session.
    pub session_id: Option<String>,

    /// Sequence number within session (for E4 temporal ordering).
    /// Memories with higher sequence numbers are more recent within the session.
    pub session_sequence: Option<u64>,

    /// Precomputed causal direction at embedding time (cause/effect/unknown).
    /// Inferred from E5 embedding norms at storage time for efficient filtering.
    /// - "cause": Document primarily describes causes (higher E5 cause norm)
    /// - "effect": Document primarily describes effects (higher E5 effect norm)
    /// - "unknown": No clear causal direction detected
    pub causal_direction: Option<String>,

    // ===== CausalExplanation-specific fields =====

    /// UUID of the original memory that was analyzed (for CausalExplanation).
    pub source_fingerprint_id: Option<uuid::Uuid>,

    /// Link to the associated CausalRelationship (for CausalExplanation).
    pub causal_relationship_id: Option<uuid::Uuid>,

    /// Type of causal mechanism (for CausalExplanation): "direct", "mediated", "feedback", "temporal"
    pub mechanism_type: Option<String>,

    /// LLM confidence score [0.0, 1.0] (for CausalExplanation).
    pub confidence: Option<f32>,

    /// User/agent who created this memory (Phase 1.2 operator attribution).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,

    /// Explicit creation timestamp (UTC, Phase 1.2 operator attribution).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,

    /// SHA-256 hash of the source file content (Phase 3b code git provenance).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_content_hash: Option<String>,

    /// When the source file was last modified on disk (Phase 3b).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_modified_at: Option<DateTime<Utc>>,

    /// UUIDs of source memories this was derived from (Phase 4, item 5.10).
    /// Set when a memory is created by merging other memories.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derived_from: Option<Vec<uuid::Uuid>>,

    /// Method used to derive this memory (Phase 4, item 5.10).
    /// E.g., "merge:union", "merge:intersection", "merge:weighted_average".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derivation_method: Option<String>,

    /// Tool use ID from Claude Code hook payload (Phase 5, item 5.12).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,

    /// MCP JSON-RPC request ID (Phase 5, item 5.12).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_request_id: Option<String>,

    /// Hook execution timestamp in milliseconds (Phase 5, item 5.12).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hook_execution_timestamp_ms: Option<i64>,

    /// Provenance of LLM guidance applied during E5 embedding.
    /// Tracks static vs LLM-guided markers, asymmetry strength, and effective boost.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding_hint_provenance: Option<crate::traits::EmbeddingHintProvenance>,

    /// Canonical entity names extracted at store_memory time.
    /// Enables entity Jaccard overlap scoring in search_by_entities
    /// without re-extracting from content at search time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity_names: Option<Vec<String>>,
}

/// Type of memory source.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SourceType {
    /// From markdown file watcher chunks
    MDFileChunk,
    /// From Claude Code hook events
    HookDescription,
    /// From session end captured responses
    ClaudeResponse,
    /// User-injected via MCP tools
    Manual,
    /// LLM-generated causal explanation (E5+LLM knowledge generation)
    ///
    /// E5+LLM is the ONLY embedder pair that GENERATES new knowledge.
    /// This source type indicates the memory content is an LLM-articulated
    /// causal relationship, stored as a first-class teleological fingerprint
    /// with all 13 embeddings.
    CausalExplanation,
    /// Unknown source
    Unknown,
}

impl Default for SourceMetadata {
    fn default() -> Self {
        Self {
            source_type: SourceType::Unknown,
            file_path: None,
            chunk_index: None,
            total_chunks: None,
            start_line: None,
            end_line: None,
            hook_type: None,
            tool_name: None,
            session_id: None,
            session_sequence: None,
            causal_direction: None,
            source_fingerprint_id: None,
            causal_relationship_id: None,
            mechanism_type: None,
            confidence: None,
            created_by: None,
            created_at: None,
            file_content_hash: None,
            file_modified_at: None,
            derived_from: None,
            derivation_method: None,
            tool_use_id: None,
            mcp_request_id: None,
            hook_execution_timestamp_ms: None,
            embedding_hint_provenance: None,
            entity_names: None,
        }
    }
}

impl SourceMetadata {
    /// Create metadata for an MDFileChunk source.
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the source markdown file
    /// * `chunk_index` - 0-based index of this chunk
    /// * `total_chunks` - Total number of chunks from this file
    pub fn md_file_chunk(file_path: impl Into<String>, chunk_index: u32, total_chunks: u32) -> Self {
        Self {
            source_type: SourceType::MDFileChunk,
            file_path: Some(file_path.into()),
            chunk_index: Some(chunk_index),
            total_chunks: Some(total_chunks),
            ..Self::default()
        }
    }

    /// Create metadata for an MDFileChunk source with line numbers.
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the source markdown file
    /// * `chunk_index` - 0-based index of this chunk
    /// * `total_chunks` - Total number of chunks from this file
    /// * `start_line` - Starting line number (1-based)
    /// * `end_line` - Ending line number (1-based, inclusive)
    pub fn md_file_chunk_with_lines(
        file_path: impl Into<String>,
        chunk_index: u32,
        total_chunks: u32,
        start_line: u32,
        end_line: u32,
    ) -> Self {
        Self {
            source_type: SourceType::MDFileChunk,
            file_path: Some(file_path.into()),
            chunk_index: Some(chunk_index),
            total_chunks: Some(total_chunks),
            start_line: Some(start_line),
            end_line: Some(end_line),
            ..Self::default()
        }
    }

    /// Create metadata for a HookDescription source.
    ///
    /// # Arguments
    ///
    /// * `hook_type` - Type of hook (e.g., "SessionStart", "PostToolUse")
    /// * `tool_name` - Optional tool name for tool-related hooks
    pub fn hook_description(hook_type: impl Into<String>, tool_name: Option<String>) -> Self {
        Self {
            source_type: SourceType::HookDescription,
            hook_type: Some(hook_type.into()),
            tool_name,
            ..Self::default()
        }
    }

    /// Create metadata for a ClaudeResponse source.
    pub fn claude_response() -> Self {
        Self {
            source_type: SourceType::ClaudeResponse,
            ..Self::default()
        }
    }

    /// Create metadata for a manually injected memory.
    pub fn manual() -> Self {
        Self {
            source_type: SourceType::Manual,
            ..Self::default()
        }
    }

    /// Create metadata for a CausalExplanation source.
    ///
    /// # Arguments
    ///
    /// * `source_fingerprint_id` - UUID of the original memory that was analyzed
    /// * `causal_relationship_id` - UUID of the associated CausalRelationship
    /// * `mechanism_type` - Type of causal mechanism: "direct", "mediated", "feedback", "temporal"
    /// * `confidence` - LLM confidence score [0.0, 1.0]
    pub fn causal_explanation(
        source_fingerprint_id: uuid::Uuid,
        causal_relationship_id: uuid::Uuid,
        mechanism_type: String,
        confidence: f32,
    ) -> Self {
        Self {
            source_type: SourceType::CausalExplanation,
            source_fingerprint_id: Some(source_fingerprint_id),
            causal_relationship_id: Some(causal_relationship_id),
            mechanism_type: Some(mechanism_type),
            confidence: Some(confidence),
            ..Self::default()
        }
    }

    /// Set causal direction for this memory.
    ///
    /// # Arguments
    ///
    /// * `direction` - Causal direction ("cause", "effect", or "unknown")
    pub fn with_causal_direction(mut self, direction: impl Into<String>) -> Self {
        self.causal_direction = Some(direction.into());
        self
    }

    /// Set session context for temporal tracking.
    ///
    /// # Arguments
    ///
    /// * `session_id` - Claude Code session ID
    /// * `session_sequence` - Order within session
    pub fn with_session(mut self, session_id: impl Into<String>, session_sequence: u64) -> Self {
        self.session_id = Some(session_id.into());
        self.session_sequence = Some(session_sequence);
        self
    }

    /// Set operator attribution (Phase 1.2 provenance improvement).
    ///
    /// Records who created this memory and when.
    ///
    /// # Arguments
    ///
    /// * `operator_id` - User/agent who created this memory
    pub fn with_operator(mut self, operator_id: impl Into<String>) -> Self {
        self.created_by = Some(operator_id.into());
        self.created_at = Some(Utc::now());
        self
    }

    /// Check if this is an MDFileChunk source.
    pub fn is_md_file_chunk(&self) -> bool {
        matches!(self.source_type, SourceType::MDFileChunk)
    }

    /// Format as a display string for context injection.
    ///
    /// Returns a human-readable string describing the source including
    /// file path, chunk info, and line numbers when available.
    ///
    /// # Examples
    ///
    /// - MDFileChunk with lines: "Source: `docs/readme.md:10-45` (chunk 2/5)"
    /// - MDFileChunk without lines: "Source: `/path/to/file.md` (chunk 2/5)"
    /// - HookDescription: "Source: Hook[PostToolUse] (tool: Edit)"
    /// - Manual: "Source: Manual injection"
    pub fn display_string(&self) -> String {
        match self.source_type {
            SourceType::MDFileChunk => {
                let path = self.file_path.as_deref().unwrap_or("unknown");
                // Include line numbers if available
                let path_with_lines = match (self.start_line, self.end_line) {
                    (Some(start), Some(end)) => format!("{}:{}-{}", path, start, end),
                    (Some(start), None) => format!("{}:{}", path, start),
                    _ => path.to_string(),
                };
                let chunk_info = match (self.chunk_index, self.total_chunks) {
                    (Some(idx), Some(total)) => format!(" (chunk {}/{})", idx + 1, total),
                    _ => String::new(),
                };
                format!("Source: `{}`{}", path_with_lines, chunk_info)
            }
            SourceType::HookDescription => {
                let hook = self.hook_type.as_deref().unwrap_or("Unknown");
                match &self.tool_name {
                    Some(tool) => format!("Source: Hook[{}] (tool: {})", hook, tool),
                    None => format!("Source: Hook[{}]", hook),
                }
            }
            SourceType::ClaudeResponse => "Source: Claude response capture".to_string(),
            SourceType::Manual => "Source: Manual injection".to_string(),
            SourceType::CausalExplanation => {
                let mech = self.mechanism_type.as_deref().unwrap_or("unknown");
                let conf = self.confidence.unwrap_or(0.0);
                format!("Source: CausalExplanation[{}] (confidence: {:.2})", mech, conf)
            }
            SourceType::Unknown => "Source: Unknown".to_string(),
        }
    }
}

impl std::fmt::Display for SourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SourceType::MDFileChunk => write!(f, "MDFileChunk"),
            SourceType::HookDescription => write!(f, "HookDescription"),
            SourceType::ClaudeResponse => write!(f, "ClaudeResponse"),
            SourceType::Manual => write!(f, "Manual"),
            SourceType::CausalExplanation => write!(f, "CausalExplanation"),
            SourceType::Unknown => write!(f, "Unknown"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_md_file_chunk_creation() {
        let meta = SourceMetadata::md_file_chunk("/path/to/doc.md", 2, 5);
        assert!(meta.is_md_file_chunk());
        assert_eq!(meta.file_path.as_deref(), Some("/path/to/doc.md"));
        assert_eq!(meta.chunk_index, Some(2));
        assert_eq!(meta.total_chunks, Some(5));
    }

    #[test]
    fn test_md_file_chunk_display() {
        let meta = SourceMetadata::md_file_chunk("/docs/readme.md", 1, 3);
        let display = meta.display_string();
        assert_eq!(display, "Source: `/docs/readme.md` (chunk 2/3)");
    }

    #[test]
    fn test_hook_description_display() {
        let meta = SourceMetadata::hook_description("PostToolUse", Some("Edit".to_string()));
        let display = meta.display_string();
        assert_eq!(display, "Source: Hook[PostToolUse] (tool: Edit)");
    }

    #[test]
    fn test_serialization_roundtrip() {
        let original = SourceMetadata::md_file_chunk("/test.md", 0, 1);
        let serialized = serde_json::to_string(&original).expect("serialize");
        let deserialized: SourceMetadata = serde_json::from_str(&serialized).expect("deserialize");
        assert_eq!(original, deserialized);
    }

    #[test]
    fn test_md_file_chunk_with_lines_creation() {
        let meta = SourceMetadata::md_file_chunk_with_lines("/docs/readme.md", 1, 5, 10, 35);
        assert!(meta.is_md_file_chunk());
        assert_eq!(meta.file_path.as_deref(), Some("/docs/readme.md"));
        assert_eq!(meta.chunk_index, Some(1));
        assert_eq!(meta.total_chunks, Some(5));
        assert_eq!(meta.start_line, Some(10));
        assert_eq!(meta.end_line, Some(35));
    }

    #[test]
    fn test_md_file_chunk_display_with_lines() {
        let meta = SourceMetadata::md_file_chunk_with_lines("/docs/readme.md", 1, 3, 10, 45);
        let display = meta.display_string();
        // Should show: Source: `/docs/readme.md:10-45` (chunk 2/3)
        assert!(display.contains("/docs/readme.md:10-45"), "Display should contain path:lines, got: {}", display);
        assert!(display.contains("2/3"), "Display should contain chunk info, got: {}", display);
    }

    #[test]
    fn test_md_file_chunk_display_without_lines() {
        let meta = SourceMetadata::md_file_chunk("/docs/readme.md", 1, 3);
        let display = meta.display_string();
        // Should show: Source: `/docs/readme.md` (chunk 2/3)
        assert!(!display.contains(":10"), "Display without lines should not have line numbers");
        assert!(display.contains("/docs/readme.md"), "Display should contain path, got: {}", display);
        assert!(display.contains("2/3"), "Display should contain chunk info, got: {}", display);
    }
}
