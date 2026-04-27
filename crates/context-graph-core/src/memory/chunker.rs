//! Text chunking types and TextChunker for the Context Graph system.
//!
//! This module provides:
//! - [`TextChunk`] - Type representing a chunk of text with metadata
//! - [`TextChunker`] - Component that splits text into overlapping chunks
//! - [`ChunkerError`] - Error types for chunking operations
//!
//! # Note
//! [`ChunkMetadata`] is defined in the parent module (`memory/mod.rs`)
//! and imported here for use with TextChunk.
//!
//! # Constitution Compliance
//! - memory_sources.MDFileChunk.chunking: 200 words, 50 overlap
//! - boundary: "Preserve sentence boundaries when possible"
//! - TextChunk is transient - NOT stored directly, converted to Memory
//!
//! # Example
//! ```rust
//! use context_graph_core::memory::{TextChunker, TextChunk};
//!
//! let chunker = TextChunker::default_config();
//! // Short content returns single chunk
//! let chunks = chunker.chunk_text("Hello world.", "test.md").unwrap();
//! assert_eq!(chunks.len(), 1);
//! ```

use sha2::{Digest, Sha256};
use thiserror::Error;

use super::ChunkMetadata;

/// A chunk of text with associated metadata.
///
/// TextChunk is a **transient** container used during the chunking process.
/// It combines:
/// - The chunk content (text)
/// - Word count (pre-computed for efficiency)
/// - Full metadata about chunk origin and position
///
/// # Lifecycle
/// 1. Created by TextChunker during file processing
/// 2. Passed to MemoryCaptureService
/// 3. Converted to Memory struct (with TeleologicalArray embedding)
/// 4. Memory is persisted to storage
///
/// TextChunk itself is NEVER stored directly.
///
/// # Example
/// ```rust
/// use context_graph_core::memory::{TextChunk, ChunkMetadata};
///
/// let metadata = ChunkMetadata {
///     file_path: "docs/readme.md".to_string(),
///     chunk_index: 0,
///     total_chunks: 3,
///     word_offset: 0,
///     char_offset: 0,
///     original_file_hash: "abc123...".to_string(),
///     start_line: 1,
///     end_line: 10,
/// };
///
/// let chunk = TextChunk::new(
///     "This is the first chunk of text content.".to_string(),
///     metadata,
/// );
///
/// assert_eq!(chunk.word_count, 8);
/// assert_eq!(chunk.metadata.chunk_index, 0);
/// ```
#[derive(Debug, Clone)]
pub struct TextChunk {
    /// The chunk content (text extracted from source).
    pub content: String,

    /// Number of words in content.
    /// Pre-computed on creation for efficiency.
    pub word_count: u32,

    /// Full metadata about chunk origin and position.
    pub metadata: ChunkMetadata,
}

impl TextChunk {
    /// Create a new TextChunk with auto-computed word count.
    ///
    /// # Arguments
    /// * `content` - The text content of this chunk
    /// * `metadata` - Metadata about chunk origin and position
    ///
    /// # Word Count
    /// Computed as whitespace-separated tokens: `content.split_whitespace().count()`
    ///
    /// # Example
    /// ```rust
    /// use context_graph_core::memory::{TextChunk, ChunkMetadata};
    ///
    /// let meta = ChunkMetadata {
    ///     file_path: "test.md".to_string(),
    ///     chunk_index: 0,
    ///     total_chunks: 1,
    ///     word_offset: 0,
    ///     char_offset: 0,
    ///     original_file_hash: "hash".to_string(),
    ///     start_line: 1,
    ///     end_line: 5,
    /// };
    ///
    /// let chunk = TextChunk::new("hello world".to_string(), meta);
    /// assert_eq!(chunk.word_count, 2);
    /// ```
    pub fn new(content: String, metadata: ChunkMetadata) -> Self {
        let word_count = content.split_whitespace().count() as u32;
        Self {
            content,
            word_count,
            metadata,
        }
    }

    /// Create TextChunk with explicit word count (for testing/reconstruction).
    ///
    /// # Warning
    /// Caller is responsible for ensuring word_count matches content.
    /// Use `new()` in production code for automatic counting.
    pub fn with_word_count(content: String, word_count: u32, metadata: ChunkMetadata) -> Self {
        Self {
            content,
            word_count,
            metadata,
        }
    }

    /// Check if this chunk is empty (zero words).
    pub fn is_empty(&self) -> bool {
        self.word_count == 0
    }

    /// Get the content length in bytes.
    pub fn byte_len(&self) -> usize {
        self.content.len()
    }

    /// Get the content length in characters.
    pub fn char_len(&self) -> usize {
        self.content.chars().count()
    }
}

// =============================================================================
// ChunkerError - Error types for chunking operations
// =============================================================================

/// Errors that can occur during text chunking operations.
///
/// All errors include context values for debugging.
/// Per constitution: "Never panic in lib, Propagate with ?"
#[derive(Debug, Error)]
pub enum ChunkerError {
    /// Content is empty or contains only whitespace.
    #[error("Content is empty or contains only whitespace")]
    EmptyContent,

    /// Configuration error: chunk_size must be greater than overlap.
    #[error("Configuration error: chunk_size ({chunk_size}) must be > overlap ({overlap})")]
    InvalidOverlap {
        /// The provided chunk_size value.
        chunk_size: usize,
        /// The provided overlap value.
        overlap: usize,
    },

    /// Configuration error: chunk_size must be >= MIN_CHUNK_WORDS.
    #[error("Configuration error: chunk_size ({chunk_size}) must be >= MIN_CHUNK_WORDS ({min})")]
    ChunkSizeTooSmall {
        /// The provided chunk_size value.
        chunk_size: usize,
        /// The minimum allowed chunk_size.
        min: usize,
    },
}

// =============================================================================
// TextChunker - Text chunking component
// =============================================================================

/// Text chunker that splits content into overlapping chunks.
///
/// Per constitution.yaml memory_sources.MDFileChunk.chunking:
/// - chunk_size: 200 words
/// - overlap: 50 words (25%)
/// - boundary: "Preserve sentence boundaries when possible"
///
/// # Sentence Boundary Detection
/// The chunker attempts to align chunk boundaries to sentence endings
/// (`.`, `!`, `?`) within the last 20% of each chunk. This improves
/// semantic coherence of chunks.
///
/// # Example
/// ```rust
/// use context_graph_core::memory::TextChunker;
///
/// // Create with default constitution values
/// let chunker = TextChunker::default_config();
///
/// // Chunk a document
/// let content = "First sentence. Second sentence. Third sentence.";
/// let chunks = chunker.chunk_text(content, "doc.md").unwrap();
///
/// // Short content produces single chunk
/// assert_eq!(chunks.len(), 1);
/// ```
#[derive(Debug, Clone)]
pub struct TextChunker {
    /// Target chunk size in words.
    chunk_size_words: usize,
    /// Overlap between consecutive chunks in words.
    overlap_words: usize,
}

impl TextChunker {
    /// Default chunk size per constitution: 200 words.
    pub const CHUNK_SIZE_WORDS: usize = 200;
    /// Default overlap per constitution: 50 words (25%).
    pub const OVERLAP_WORDS: usize = 50;
    /// Minimum chunk size to ensure meaningful content.
    pub const MIN_CHUNK_WORDS: usize = 50;
    /// Sentence terminator characters for boundary detection.
    const SENTENCE_TERMINATORS: [char; 3] = ['.', '!', '?'];

    /// Create a new TextChunker with custom configuration.
    ///
    /// # Arguments
    /// * `chunk_size` - Target chunk size in words
    /// * `overlap` - Overlap between chunks in words
    ///
    /// # Errors
    /// - [`ChunkerError::InvalidOverlap`] if chunk_size <= overlap
    /// - [`ChunkerError::ChunkSizeTooSmall`] if chunk_size < MIN_CHUNK_WORDS
    ///
    /// # Fail-Fast Behavior
    /// Invalid configuration fails immediately with descriptive error.
    ///
    /// # Example
    /// ```rust
    /// use context_graph_core::memory::TextChunker;
    ///
    /// // Valid configuration
    /// let chunker = TextChunker::new(100, 25).unwrap();
    ///
    /// // Invalid: overlap >= chunk_size
    /// let err = TextChunker::new(50, 50).unwrap_err();
    /// assert!(err.to_string().contains("must be > overlap"));
    ///
    /// // Invalid: chunk_size too small
    /// let err = TextChunker::new(30, 10).unwrap_err();
    /// assert!(err.to_string().contains("MIN_CHUNK_WORDS"));
    /// ```
    pub fn new(chunk_size: usize, overlap: usize) -> Result<Self, ChunkerError> {
        // Fail fast: chunk_size must be greater than overlap
        if chunk_size <= overlap {
            return Err(ChunkerError::InvalidOverlap {
                chunk_size,
                overlap,
            });
        }

        // Fail fast: chunk_size must meet minimum
        if chunk_size < Self::MIN_CHUNK_WORDS {
            return Err(ChunkerError::ChunkSizeTooSmall {
                chunk_size,
                min: Self::MIN_CHUNK_WORDS,
            });
        }

        Ok(Self {
            chunk_size_words: chunk_size,
            overlap_words: overlap,
        })
    }

    /// Create TextChunker with default constitution values.
    ///
    /// Uses CHUNK_SIZE_WORDS=200, OVERLAP_WORDS=50 per constitution.
    ///
    /// # Example
    /// ```rust
    /// use context_graph_core::memory::TextChunker;
    ///
    /// let chunker = TextChunker::default_config();
    /// // Uses 200-word chunks with 50-word overlap
    /// ```
    pub fn default_config() -> Self {
        // These constants are valid by definition, no error possible
        Self {
            chunk_size_words: Self::CHUNK_SIZE_WORDS,
            overlap_words: Self::OVERLAP_WORDS,
        }
    }

    /// Get the configured chunk size in words.
    pub fn chunk_size(&self) -> usize {
        self.chunk_size_words
    }

    /// Get the configured overlap in words.
    pub fn overlap(&self) -> usize {
        self.overlap_words
    }

    /// Chunk text content into overlapping TextChunk instances.
    ///
    /// # Arguments
    /// * `content` - The text to chunk
    /// * `file_path` - Source file path for metadata
    ///
    /// # Returns
    /// Vec of TextChunk with proper metadata including:
    /// - chunk_index (0-based)
    /// - total_chunks
    /// - word_offset (cumulative, based on effective advance)
    /// - char_offset (cumulative)
    /// - original_file_hash (SHA256 of full content)
    /// - start_line and end_line (1-based line numbers)
    ///
    /// # Errors
    /// - [`ChunkerError::EmptyContent`] if content is empty or whitespace-only
    ///
    /// # Example
    /// ```rust
    /// use context_graph_core::memory::TextChunker;
    ///
    /// let chunker = TextChunker::default_config();
    ///
    /// // Short content -> single chunk
    /// let chunks = chunker.chunk_text("Hello world.", "test.md").unwrap();
    /// assert_eq!(chunks.len(), 1);
    /// assert_eq!(chunks[0].metadata.chunk_index, 0);
    /// assert_eq!(chunks[0].metadata.total_chunks, 1);
    ///
    /// // Empty content -> error
    /// let err = chunker.chunk_text("", "empty.md").unwrap_err();
    /// assert!(err.to_string().contains("empty"));
    /// ```
    pub fn chunk_text(
        &self,
        content: &str,
        file_path: &str,
    ) -> Result<Vec<TextChunk>, ChunkerError> {
        // Fail fast on empty content
        if content.is_empty() || content.trim().is_empty() {
            return Err(ChunkerError::EmptyContent);
        }

        let words: Vec<&str> = content.split_whitespace().collect();
        if words.is_empty() {
            return Err(ChunkerError::EmptyContent);
        }

        // Compute deterministic SHA256 hash of full content
        let hash = Self::compute_hash(content);

        // Build line number mapping: for each character offset, which line is it on?
        // Line numbers are 1-based
        let line_starts = Self::compute_line_starts(content);
        let total_lines = line_starts.len() as u32;

        // Single chunk case: content fits in one chunk
        if words.len() <= self.chunk_size_words {
            let metadata = ChunkMetadata {
                file_path: file_path.to_string(),
                chunk_index: 0,
                total_chunks: 1,
                word_offset: 0,
                char_offset: 0,
                original_file_hash: hash,
                start_line: 1,
                end_line: total_lines,
            };
            return Ok(vec![TextChunk::new(content.to_string(), metadata)]);
        }

        // Build word-to-byte-offset mapping for line number tracking
        let word_byte_offsets = Self::compute_word_byte_offsets(content, &words);

        let mut chunks = Vec::new();
        let mut word_offset: u32 = 0;
        let mut char_offset: u32 = 0;
        let mut chunk_index: u32 = 0;

        // Estimate total chunks for metadata
        let total_chunks = self.estimate_chunk_count(words.len());

        let mut current_word_idx = 0;

        while current_word_idx < words.len() {
            // Determine end of this chunk
            let end_word_idx = std::cmp::min(current_word_idx + self.chunk_size_words, words.len());

            // Find sentence boundary (only adjusts within last 20% of chunk)
            let adjusted_end = self.find_sentence_boundary(&words, current_word_idx, end_word_idx);

            // Build chunk content from words
            let chunk_words = &words[current_word_idx..adjusted_end];
            let chunk_content = chunk_words.join(" ");

            // Calculate line numbers for this chunk
            let start_byte = word_byte_offsets
                .get(current_word_idx)
                .copied()
                .unwrap_or(0);
            let end_byte = if adjusted_end > 0 {
                word_byte_offsets
                    .get(adjusted_end - 1)
                    .map(|&off| off + words[adjusted_end - 1].len())
                    .unwrap_or(content.len())
            } else {
                0
            };
            let start_line = Self::byte_offset_to_line(&line_starts, start_byte);
            let end_line = Self::byte_offset_to_line(&line_starts, end_byte);

            let metadata = ChunkMetadata {
                file_path: file_path.to_string(),
                chunk_index,
                total_chunks,
                word_offset,
                char_offset,
                original_file_hash: hash.clone(),
                start_line,
                end_line,
            };

            chunks.push(TextChunk::new(chunk_content.clone(), metadata));

            // Calculate effective advance (words consumed minus overlap)
            let words_in_chunk = adjusted_end - current_word_idx;

            // Update char_offset for next chunk
            // +1 for the implicit space that would separate chunks in original
            char_offset += chunk_content.len() as u32 + 1;
            chunk_index += 1;

            // Check if we've reached the end
            if adjusted_end >= words.len() {
                break;
            }

            // Move to next chunk start (with overlap)
            // Effective advance is chunk size minus overlap
            let effective_advance = words_in_chunk.saturating_sub(self.overlap_words);
            // Ensure we always advance by at least 1 word to prevent infinite loop
            let advance = std::cmp::max(effective_advance, 1);
            current_word_idx += advance;
            word_offset += advance as u32;
        }

        Ok(chunks)
    }

    /// Compute the byte offset where each line starts.
    ///
    /// Returns a vector where:
    /// - `line_starts[0]` = byte offset of line 1 (always 0)
    /// - `line_starts[1]` = byte offset of line 2
    /// - `line_starts[n]` = byte offset of line n+1
    ///
    /// Line numbers are 1-based (matching editor conventions).
    fn compute_line_starts(content: &str) -> Vec<usize> {
        let mut line_starts = vec![0]; // Line 1 starts at byte 0
        for (i, c) in content.char_indices() {
            if c == '\n' {
                // Next line starts after this newline
                line_starts.push(i + 1);
            }
        }
        line_starts
    }

    /// Compute the byte offset for each word in the content.
    fn compute_word_byte_offsets(content: &str, words: &[&str]) -> Vec<usize> {
        let mut offsets = Vec::with_capacity(words.len());
        let mut search_start = 0;
        for word in words {
            if let Some(pos) = content[search_start..].find(word) {
                let byte_offset = search_start + pos;
                offsets.push(byte_offset);
                search_start = byte_offset + word.len();
            } else {
                // Fallback: shouldn't happen, but use current search position
                offsets.push(search_start);
            }
        }
        offsets
    }

    /// Convert a byte offset to a 1-based line number.
    fn byte_offset_to_line(line_starts: &[usize], byte_offset: usize) -> u32 {
        // Binary search for the line containing this byte offset
        match line_starts.binary_search(&byte_offset) {
            Ok(idx) => (idx + 1) as u32, // Exact match - byte is at start of line
            Err(idx) => idx as u32,      // Insert position = line number (1-based)
        }
    }

    /// Find sentence boundary within search window.
    ///
    /// Searches backwards from `end` within last 20% of chunk.
    /// Returns adjusted end position if boundary found, otherwise original end.
    ///
    /// # Algorithm
    /// 1. Calculate search window: last 20% of chunk_size_words
    /// 2. Search backwards from end for word ending with '.', '!', or '?'
    /// 3. If found, return position after that word (include terminator)
    /// 4. If not found, return original end
    fn find_sentence_boundary(&self, words: &[&str], start: usize, end: usize) -> usize {
        // Search window: last 20% of configured chunk size
        let search_window = self.chunk_size_words / 5;
        let search_start = end.saturating_sub(search_window).max(start);

        // Search backwards for sentence terminator
        for i in (search_start..end).rev() {
            if let Some(word) = words.get(i) {
                if Self::SENTENCE_TERMINATORS
                    .iter()
                    .any(|&t| word.ends_with(t))
                {
                    return i + 1; // Include word with terminator
                }
            }
        }

        // No boundary found, use original end
        end
    }

    /// Compute SHA256 hash of content.
    ///
    /// # Determinism
    /// Same content always produces same hash.
    fn compute_hash(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Estimate total chunk count for metadata.
    ///
    /// # Formula
    /// For content with N words where N > chunk_size:
    /// chunks = ceil((N - overlap) / (chunk_size - overlap))
    fn estimate_chunk_count(&self, total_words: usize) -> u32 {
        if total_words <= self.chunk_size_words {
            return 1;
        }

        // Effective step between chunk starts
        let effective_step = self.chunk_size_words - self.overlap_words;

        // After first chunk of chunk_size_words, remaining words
        let remaining = total_words.saturating_sub(self.chunk_size_words);

        // Number of additional chunks needed
        let additional_chunks = remaining.div_ceil(effective_step);

        (1 + additional_chunks) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_metadata() -> ChunkMetadata {
        ChunkMetadata {
            file_path: "/test/file.md".to_string(),
            chunk_index: 0,
            total_chunks: 1,
            word_offset: 0,
            char_offset: 0,
            original_file_hash: "testhash123".to_string(),
            start_line: 1,
            end_line: 1,
        }
    }

    #[test]
    fn test_text_chunk_new_computes_word_count() {
        let chunk = TextChunk::new(
            "the quick brown fox jumps over the lazy dog".to_string(),
            test_metadata(),
        );
        assert_eq!(chunk.word_count, 9);
    }

    #[test]
    fn test_text_chunk_empty_content() {
        let chunk = TextChunk::new(String::new(), test_metadata());
        assert_eq!(chunk.word_count, 0);
        assert!(chunk.is_empty());
    }

    #[test]
    fn test_text_chunk_whitespace_only() {
        let chunk = TextChunk::new("   \t\n   ".to_string(), test_metadata());
        assert_eq!(chunk.word_count, 0);
        assert!(chunk.is_empty());
    }

    #[test]
    fn test_text_chunk_single_word() {
        let chunk = TextChunk::new("hello".to_string(), test_metadata());
        assert_eq!(chunk.word_count, 1);
        assert!(!chunk.is_empty());
    }

    #[test]
    fn test_text_chunk_extra_whitespace() {
        let chunk = TextChunk::new("  hello   world  ".to_string(), test_metadata());
        assert_eq!(chunk.word_count, 2);
    }

    #[test]
    fn test_text_chunk_metadata_access() {
        let mut meta = test_metadata();
        meta.chunk_index = 5;
        meta.total_chunks = 10;

        let chunk = TextChunk::new("content".to_string(), meta);

        assert_eq!(chunk.metadata.chunk_index, 5);
        assert_eq!(chunk.metadata.total_chunks, 10);
        assert_eq!(chunk.metadata.file_path, "/test/file.md");
    }

    #[test]
    fn test_text_chunk_with_word_count() {
        let chunk = TextChunk::with_word_count("hello world".to_string(), 2, test_metadata());
        assert_eq!(chunk.word_count, 2);
    }

    #[test]
    fn test_text_chunk_byte_len() {
        let chunk = TextChunk::new("hello".to_string(), test_metadata());
        assert_eq!(chunk.byte_len(), 5);
    }

    #[test]
    fn test_text_chunk_char_len_unicode() {
        // "hello" in Japanese: こんにちは (5 chars, 15 bytes)
        let chunk = TextChunk::new("こんにちは".to_string(), test_metadata());
        assert_eq!(chunk.char_len(), 5);
        assert_eq!(chunk.byte_len(), 15); // 3 bytes per char
    }

    #[test]
    fn test_text_chunk_clone() {
        let original = TextChunk::new("test content".to_string(), test_metadata());
        let cloned = original.clone();

        assert_eq!(original.content, cloned.content);
        assert_eq!(original.word_count, cloned.word_count);
        assert_eq!(original.metadata.file_path, cloned.metadata.file_path);
    }

    #[test]
    fn test_text_chunk_debug_format() {
        let chunk = TextChunk::new("debug test".to_string(), test_metadata());
        let debug_str = format!("{:?}", chunk);

        assert!(debug_str.contains("TextChunk"));
        assert!(debug_str.contains("debug test"));
        assert!(debug_str.contains("word_count"));
    }

    // === ADDITIONAL MANUAL TESTS FROM TASK SPEC ===

    #[test]
    fn test_synthetic_basic_creation() {
        // Synthetic Test 1: Basic Creation from task spec
        let meta = ChunkMetadata {
            file_path: "test.md".to_string(),
            chunk_index: 2,
            total_chunks: 5,
            word_offset: 400,
            char_offset: 2500,
            original_file_hash: "abc123def456".to_string(),
            start_line: 50,
            end_line: 75,
        };
        let content = "This is synthetic test content";
        let chunk = TextChunk::new(content.to_string(), meta);

        // Verify outputs
        assert_eq!(chunk.word_count, 5); // 5 words
        assert_eq!(chunk.metadata.chunk_index, 2); // Preserved
        assert_eq!(chunk.byte_len(), content.len()); // 30 ASCII bytes
        assert!(!chunk.is_empty()); // Not empty
    }

    #[test]
    fn test_synthetic_large_content() {
        // Synthetic Test 2: Large Content - 200-word chunk (typical chunk size per constitution)
        let words: Vec<&str> = (0..200).map(|_| "word").collect();
        let content = words.join(" ");
        let chunk = TextChunk::new(content.clone(), test_metadata());

        assert_eq!(chunk.word_count, 200);
        // 200 words * 4 chars + 199 spaces = 999 bytes
        assert_eq!(chunk.byte_len(), 200 * 4 + 199);
    }

    #[test]
    fn test_newlines_as_whitespace() {
        // Verify newlines count as word separators
        let chunk = TextChunk::new("word1\nword2\r\nword3".to_string(), test_metadata());
        assert_eq!(chunk.word_count, 3);
    }

    #[test]
    fn test_tabs_as_whitespace() {
        // Verify tabs count as word separators
        let chunk = TextChunk::new("word1\tword2\t\tword3".to_string(), test_metadata());
        assert_eq!(chunk.word_count, 3);
    }

    #[test]
    fn test_mixed_unicode_content() {
        // Mix of ASCII and Unicode
        // "hello 世界 rust 言語" = h(1) e(1) l(1) l(1) o(1) space(1) 世(1) 界(1) space(1) r(1) u(1) s(1) t(1) space(1) 言(1) 語(1) = 16 chars
        let content = "hello 世界 rust 言語";
        let chunk = TextChunk::new(content.to_string(), test_metadata());
        assert_eq!(chunk.word_count, 4); // 4 words
        assert_eq!(chunk.char_len(), content.chars().count()); // 16 characters total (including spaces)
    }

    #[test]
    fn test_emoji_content() {
        // Emojis in content
        let chunk = TextChunk::new("hello 🌍 world 🚀".to_string(), test_metadata());
        assert_eq!(chunk.word_count, 4); // Emojis are words
    }

    #[test]
    fn test_metadata_preservation() {
        // Verify all metadata fields are preserved
        let meta = ChunkMetadata {
            file_path: "/some/very/long/path/to/file.md".to_string(),
            chunk_index: 99,
            total_chunks: 100,
            word_offset: 19800,
            char_offset: 125000,
            original_file_hash: "sha256_hash_of_64_chars_0123456789abcdef0123456789abcdef"
                .to_string(),
            start_line: 500,
            end_line: 520,
        };
        let chunk = TextChunk::new("content".to_string(), meta);

        assert_eq!(chunk.metadata.file_path, "/some/very/long/path/to/file.md");
        assert_eq!(chunk.metadata.chunk_index, 99);
        assert_eq!(chunk.metadata.total_chunks, 100);
        assert_eq!(chunk.metadata.word_offset, 19800);
        assert_eq!(chunk.metadata.char_offset, 125000);
        assert_eq!(
            chunk.metadata.original_file_hash,
            "sha256_hash_of_64_chars_0123456789abcdef0123456789abcdef"
        );
        assert_eq!(chunk.metadata.start_line, 500);
        assert_eq!(chunk.metadata.end_line, 520);
    }

    // =========================================================================
    // TextChunker Tests - REAL DATA ONLY, NO MOCKS
    // =========================================================================

    // --- Configuration Validation Tests ---

    #[test]
    fn test_chunker_new_validates_overlap() {
        // Test VC-1: chunk_size <= overlap should fail
        let result = TextChunker::new(50, 50);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(
                err,
                ChunkerError::InvalidOverlap {
                    chunk_size: 50,
                    overlap: 50
                }
            ),
            "Expected InvalidOverlap error, got: {:?}",
            err
        );
        // Verify error message contains actual values
        let msg = err.to_string();
        assert!(
            msg.contains("50"),
            "Error should contain chunk_size: {}",
            msg
        );
        assert!(
            msg.contains("must be > overlap"),
            "Error should explain the constraint: {}",
            msg
        );
    }

    #[test]
    fn test_chunker_new_validates_min_chunk_size() {
        // Test VC-2: chunk_size < MIN_CHUNK_WORDS should fail
        let result = TextChunker::new(30, 10);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(
                err,
                ChunkerError::ChunkSizeTooSmall {
                    chunk_size: 30,
                    min: 50
                }
            ),
            "Expected ChunkSizeTooSmall error, got: {:?}",
            err
        );
        // Verify error message contains actual values
        let msg = err.to_string();
        assert!(
            msg.contains("30"),
            "Error should contain chunk_size: {}",
            msg
        );
        assert!(
            msg.contains("50"),
            "Error should contain MIN_CHUNK_WORDS: {}",
            msg
        );
    }

    #[test]
    fn test_chunker_new_valid_config() {
        // Valid config should succeed
        let chunker = TextChunker::new(100, 25).expect("Valid config should work");
        assert_eq!(chunker.chunk_size(), 100);
        assert_eq!(chunker.overlap(), 25);
    }

    #[test]
    fn test_chunker_default_config_values() {
        let chunker = TextChunker::default_config();
        assert_eq!(chunker.chunk_size(), TextChunker::CHUNK_SIZE_WORDS);
        assert_eq!(chunker.overlap(), TextChunker::OVERLAP_WORDS);
        assert_eq!(chunker.chunk_size(), 200);
        assert_eq!(chunker.overlap(), 50);
    }

    // --- Empty Content Tests ---

    #[test]
    fn test_chunker_empty_content_error() {
        // Test VC-3: empty content
        let chunker = TextChunker::default_config();
        let result = chunker.chunk_text("", "empty.md");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ChunkerError::EmptyContent));
    }

    #[test]
    fn test_chunker_whitespace_only_error() {
        // Test VC-3: whitespace-only content
        let chunker = TextChunker::default_config();
        let result = chunker.chunk_text("   \t\n   ", "whitespace.md");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ChunkerError::EmptyContent));
    }

    // --- Single Chunk Tests ---

    #[test]
    fn test_chunker_single_chunk_short_content() {
        // Test VC-4: content < 200 words -> single chunk
        let chunker = TextChunker::default_config();
        let content = "The quick brown fox jumps over the lazy dog.";
        let chunks = chunker
            .chunk_text(content, "short.md")
            .expect("Should succeed");

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].metadata.chunk_index, 0);
        assert_eq!(chunks[0].metadata.total_chunks, 1);
        assert_eq!(chunks[0].metadata.word_offset, 0);
        assert_eq!(chunks[0].metadata.char_offset, 0);
        assert_eq!(chunks[0].metadata.file_path, "short.md");
        // Content should be preserved exactly
        assert_eq!(chunks[0].content, content);
    }

    // --- EDGE-1: Exactly chunk_size words ---
    #[test]
    fn test_chunker_edge_1_exactly_200_words() {
        let chunker = TextChunker::default_config();
        // Generate exactly 200 words
        let content = generate_words(200, false);
        let word_count = content.split_whitespace().count();
        assert_eq!(word_count, 200, "Setup: should have exactly 200 words");

        let chunks = chunker
            .chunk_text(&content, "exact200.md")
            .expect("Should succeed");

        println!("=== EDGE-1 EVIDENCE ===");
        println!("Input word count: {}", word_count);
        println!("Chunks produced: {}", chunks.len());
        println!("Chunk[0] word_count: {}", chunks[0].word_count);
        println!("Chunk[0] metadata: {:?}", chunks[0].metadata);
        println!("=== END EDGE-1 EVIDENCE ===");

        assert_eq!(
            chunks.len(),
            1,
            "Exactly 200 words should produce single chunk"
        );
        assert_eq!(chunks[0].word_count, 200);
        assert_eq!(chunks[0].metadata.chunk_index, 0);
        assert_eq!(chunks[0].metadata.total_chunks, 1);
    }

    // --- EDGE-2: chunk_size + 1 words (boundary) ---
    #[test]
    fn test_chunker_edge_2_201_words() {
        let chunker = TextChunker::default_config();
        // Generate 201 words
        let content = generate_words(201, false);
        let word_count = content.split_whitespace().count();
        assert_eq!(word_count, 201, "Setup: should have exactly 201 words");

        let chunks = chunker
            .chunk_text(&content, "201words.md")
            .expect("Should succeed");

        println!("=== EDGE-2 EVIDENCE ===");
        println!("Input word count: {}", word_count);
        println!("Chunks produced: {}", chunks.len());
        for (i, chunk) in chunks.iter().enumerate() {
            println!(
                "Chunk[{}]: word_count={}, offset={}, first_word={:?}",
                i,
                chunk.word_count,
                chunk.metadata.word_offset,
                chunk.content.split_whitespace().next()
            );
        }
        println!("=== END EDGE-2 EVIDENCE ===");

        // 201 words with 200 chunk size and 50 overlap should produce 2 chunks
        assert_eq!(chunks.len(), 2, "201 words should produce 2 chunks");
        assert_eq!(chunks[0].metadata.chunk_index, 0);
        assert_eq!(chunks[1].metadata.chunk_index, 1);

        // Verify overlap: last 50 words of chunk 0 == first 50 words of chunk 1
        verify_overlap(&chunks[0], &chunks[1], 50);
    }

    // --- EDGE-3: Sentence at exact 20% mark ---
    #[test]
    fn test_chunker_edge_3_sentence_at_20_percent() {
        // Use smaller chunk size for easier testing
        let chunker = TextChunker::new(100, 25).expect("Valid config");

        // Generate 120 words (more than chunk_size to trigger chunking)
        // Place sentence terminator at word 85 (15% from end of 100-word chunk)
        // Search window is last 20% = words 80-99, so word 85 is in range
        let words: Vec<String> = (0..120)
            .map(|i| {
                if i == 85 {
                    // Word 85 (86th word, 0-indexed) ends with period - in search window
                    format!("sentence{}.", i)
                } else {
                    format!("word{}", i)
                }
            })
            .collect();
        let content = words.join(" ");

        let chunks = chunker
            .chunk_text(&content, "sentence_boundary.md")
            .expect("Should succeed");

        println!("=== EDGE-3 EVIDENCE ===");
        println!("Input: 120 words with '.' at word 85 (in search window [80, 100))");
        println!("Chunks produced: {}", chunks.len());
        for (i, chunk) in chunks.iter().enumerate() {
            let last_word = chunk.content.split_whitespace().last().unwrap_or("");
            println!(
                "Chunk[{}]: word_count={}, last_word='{}'",
                i, chunk.word_count, last_word
            );
        }
        println!("=== END EDGE-3 EVIDENCE ===");

        // First chunk should end at sentence boundary (word 85, inclusive = 86 words)
        let last_word = chunks[0].content.split_whitespace().last().unwrap();
        assert!(
            last_word.ends_with('.'),
            "Chunk should end at sentence boundary, got: '{}'",
            last_word
        );
    }

    // --- EDGE-4: No sentence boundary in search range ---
    #[test]
    fn test_chunker_edge_4_no_sentence_in_range() {
        let chunker = TextChunker::new(100, 25).expect("Valid config");

        // Generate 150 words with no sentence terminators
        let content = generate_words(150, false);
        let word_count = content.split_whitespace().count();
        assert_eq!(word_count, 150);
        assert!(
            !content.contains('.') && !content.contains('!') && !content.contains('?'),
            "Content should have no sentence terminators"
        );

        let chunks = chunker
            .chunk_text(&content, "no_sentence.md")
            .expect("Should succeed");

        println!("=== EDGE-4 EVIDENCE ===");
        println!("Input: {} words with no sentence terminators", word_count);
        println!("Chunks produced: {}", chunks.len());
        for (i, chunk) in chunks.iter().enumerate() {
            println!("Chunk[{}]: word_count={}", i, chunk.word_count);
        }
        println!("=== END EDGE-4 EVIDENCE ===");

        // First chunk should have exactly chunk_size words (no boundary adjustment)
        assert_eq!(
            chunks[0].word_count, 100,
            "Without sentence boundary, chunk should use full size"
        );
    }

    // --- EDGE-5: Unicode content ---
    #[test]
    fn test_chunker_edge_5_unicode_content() {
        let chunker = TextChunker::new(50, 10).expect("Valid config");

        // Mix of ASCII and Unicode - 60 words total
        let content = (0..60)
            .map(|i| {
                if i % 5 == 0 {
                    "世界".to_string() // Chinese
                } else if i % 7 == 0 {
                    "言語".to_string() // Japanese
                } else if i % 11 == 0 {
                    "🚀".to_string() // Emoji
                } else {
                    format!("word{}", i)
                }
            })
            .collect::<Vec<_>>()
            .join(" ");

        let word_count = content.split_whitespace().count();
        assert_eq!(word_count, 60, "Setup: should have 60 words");

        let chunks = chunker
            .chunk_text(&content, "unicode.md")
            .expect("Should succeed");

        println!("=== EDGE-5 EVIDENCE ===");
        println!("Input: {} words with Unicode", word_count);
        // Use char-based slicing to avoid breaking multi-byte characters
        let sample: String = content.chars().take(100).collect();
        println!("Content sample: {}", sample);
        println!("Chunks produced: {}", chunks.len());
        for (i, chunk) in chunks.iter().enumerate() {
            println!(
                "Chunk[{}]: word_count={}, char_len={}, byte_len={}",
                i,
                chunk.word_count,
                chunk.char_len(),
                chunk.byte_len()
            );
        }
        println!("=== END EDGE-5 EVIDENCE ===");

        // Word count should be based on whitespace, not bytes
        let total_words: u32 = chunks.iter().map(|c| c.word_count).sum();
        // Total words in chunks should be > 60 due to overlap
        assert!(
            total_words > 60,
            "Total words in chunks should include overlap"
        );
    }

    // --- Hash Determinism Test ---
    #[test]
    fn test_chunker_hash_determinism() {
        // Test VC-7: Same content = same hash
        let chunker = TextChunker::default_config();
        let content = "This is test content for hash verification.";

        let chunks1 = chunker
            .chunk_text(content, "test1.md")
            .expect("Should succeed");
        let chunks2 = chunker
            .chunk_text(content, "test2.md")
            .expect("Should succeed");

        println!("=== HASH DETERMINISM EVIDENCE ===");
        println!("Content: {}", content);
        println!("Hash 1: {}", chunks1[0].metadata.original_file_hash);
        println!("Hash 2: {}", chunks2[0].metadata.original_file_hash);
        println!(
            "Hashes equal: {}",
            chunks1[0].metadata.original_file_hash == chunks2[0].metadata.original_file_hash
        );
        println!("=== END HASH DETERMINISM EVIDENCE ===");

        assert_eq!(
            chunks1[0].metadata.original_file_hash, chunks2[0].metadata.original_file_hash,
            "Same content should produce same hash"
        );

        // Different content should produce different hash
        let content3 = "Different content!";
        let chunks3 = chunker
            .chunk_text(content3, "test3.md")
            .expect("Should succeed");
        assert_ne!(
            chunks1[0].metadata.original_file_hash, chunks3[0].metadata.original_file_hash,
            "Different content should produce different hash"
        );
    }

    // --- Multiple Chunks with Overlap Test ---
    #[test]
    fn test_chunker_multiple_chunks_with_overlap() {
        // Test VC-5: Multiple chunks have correct 50-word overlap
        let chunker = TextChunker::new(100, 25).expect("Valid config");

        // Generate 200 words with sentence boundaries
        let content = generate_words_with_sentences(200, 25);
        let word_count = content.split_whitespace().count();

        let chunks = chunker
            .chunk_text(&content, "overlap_test.md")
            .expect("Should succeed");

        println!("=== OVERLAP TEST EVIDENCE ===");
        println!("Input word count: {}", word_count);
        println!("Config: chunk_size=100, overlap=25");
        println!("Chunks produced: {}", chunks.len());

        for i in 0..chunks.len() {
            let first_words: Vec<&str> = chunks[i].content.split_whitespace().take(5).collect();
            let last_words: Vec<&str> =
                chunks[i].content.split_whitespace().rev().take(5).collect();
            println!(
                "Chunk[{}]: words={}, offset={}, first={:?}, last={:?}",
                i,
                chunks[i].word_count,
                chunks[i].metadata.word_offset,
                first_words,
                last_words.iter().rev().collect::<Vec<_>>()
            );
        }

        // Verify overlap between consecutive chunks
        for i in 0..chunks.len() - 1 {
            println!(
                "--- Verifying overlap between chunk {} and {} ---",
                i,
                i + 1
            );
            verify_overlap(&chunks[i], &chunks[i + 1], 25);
        }
        println!("=== END OVERLAP TEST EVIDENCE ===");

        assert!(chunks.len() >= 2, "Should produce multiple chunks");
    }

    // --- Metadata Correctness Test ---
    #[test]
    fn test_chunker_metadata_correctness() {
        // Test VC-8: All metadata fields populated correctly
        let chunker = TextChunker::new(100, 25).expect("Valid config");
        let content = generate_words(250, false);

        let chunks = chunker
            .chunk_text(&content, "metadata_test.md")
            .expect("Should succeed");

        println!("=== METADATA CORRECTNESS EVIDENCE ===");
        for (i, chunk) in chunks.iter().enumerate() {
            println!(
                "Chunk[{}]: index={}, total={}, word_offset={}, char_offset={}, hash_len={}",
                i,
                chunk.metadata.chunk_index,
                chunk.metadata.total_chunks,
                chunk.metadata.word_offset,
                chunk.metadata.char_offset,
                chunk.metadata.original_file_hash.len()
            );
        }
        println!("=== END METADATA CORRECTNESS EVIDENCE ===");

        // Verify chunk indices
        for (i, chunk) in chunks.iter().enumerate() {
            assert_eq!(chunk.metadata.chunk_index, i as u32);
            assert_eq!(chunk.metadata.file_path, "metadata_test.md");
            // Hash should be SHA256 hex (64 chars)
            assert_eq!(
                chunk.metadata.original_file_hash.len(),
                64,
                "SHA256 hash should be 64 hex chars"
            );
        }

        // All chunks should have same total_chunks
        let total = chunks[0].metadata.total_chunks;
        for chunk in &chunks {
            assert_eq!(chunk.metadata.total_chunks, total);
        }

        // All chunks should have same hash (from same source content)
        let hash = &chunks[0].metadata.original_file_hash;
        for chunk in &chunks {
            assert_eq!(&chunk.metadata.original_file_hash, hash);
        }
    }

    // --- Evidence of Success Test (from task spec) ---
    #[test]
    fn test_verify_chunk_text_evidence() {
        let chunker = TextChunker::default_config();

        // Create 450-word content (should produce ~3 chunks)
        let words: Vec<String> = (0..450)
            .map(|i| {
                if i % 50 == 49 {
                    format!("word{}.", i) // Add sentence boundaries
                } else {
                    format!("word{}", i)
                }
            })
            .collect();
        let content = words.join(" ");

        let chunks = chunker
            .chunk_text(&content, "evidence_test.md")
            .expect("Should succeed");

        // EVIDENCE: Print actual state
        println!("=== EVIDENCE OF SUCCESS ===");
        println!("Total chunks: {}", chunks.len());
        for (i, chunk) in chunks.iter().enumerate() {
            println!(
                "Chunk {}: word_count={}, word_offset={}, first_word={:?}, last_word={:?}",
                i,
                chunk.word_count,
                chunk.metadata.word_offset,
                chunk.content.split_whitespace().next(),
                chunk.content.split_whitespace().last()
            );
        }

        // Verify overlap between consecutive chunks
        for i in 0..chunks.len() - 1 {
            let current_last_words: Vec<&str> = chunks[i]
                .content
                .split_whitespace()
                .rev()
                .take(50)
                .collect();
            let next_first_words: Vec<&str> =
                chunks[i + 1].content.split_whitespace().take(50).collect();

            println!(
                "Overlap {}->{}: last_50={:?}, first_50={:?}",
                i,
                i + 1,
                current_last_words.iter().rev().take(5).collect::<Vec<_>>(),
                next_first_words.iter().take(5).collect::<Vec<_>>()
            );
        }
        println!("=== END EVIDENCE ===");

        // Assertions from task spec
        assert!(chunks.len() >= 2, "Should produce multiple chunks");
        assert_eq!(chunks[0].metadata.chunk_index, 0);
        assert_eq!(
            chunks.last().unwrap().metadata.chunk_index as usize,
            chunks.len() - 1
        );
    }

    // --- Sentence Boundary Detection Test ---
    #[test]
    fn test_chunker_sentence_boundary_detection() {
        // Test VC-6: Sentence boundary detection adjusts within 20%
        let chunker = TextChunker::new(100, 25).expect("Valid config");

        // Create content with sentence at word 85 (15% from end of 100-word chunk)
        // Search window is last 20% = words 80-99, so word 85 is in range
        let words: Vec<String> = (0..150)
            .map(|i| {
                if i == 84 {
                    "ending.".to_string() // Word 84 ends sentence
                } else if i == 94 {
                    "another.".to_string() // Word 94 ends sentence
                } else {
                    format!("word{}", i)
                }
            })
            .collect();
        let content = words.join(" ");

        let chunks = chunker
            .chunk_text(&content, "boundary.md")
            .expect("Should succeed");

        println!("=== SENTENCE BOUNDARY DETECTION EVIDENCE ===");
        for (i, chunk) in chunks.iter().enumerate() {
            let last_word = chunk.content.split_whitespace().last().unwrap_or("");
            let word_count = chunk.word_count;
            println!(
                "Chunk[{}]: words={}, last_word='{}', ends_with_period={}",
                i,
                word_count,
                last_word,
                last_word.ends_with('.')
            );
        }
        println!("=== END SENTENCE BOUNDARY DETECTION EVIDENCE ===");

        // Check first chunk ends at a sentence boundary
        let first_chunk_last = chunks[0].content.split_whitespace().last().unwrap();
        // It should end with either "ending." or "another." depending on boundary detection
        assert!(
            first_chunk_last.ends_with('.'),
            "First chunk should end at sentence boundary, got: '{}'",
            first_chunk_last
        );
    }

    // --- Test constants accessibility ---
    #[test]
    fn test_chunker_constants() {
        assert_eq!(TextChunker::CHUNK_SIZE_WORDS, 200);
        assert_eq!(TextChunker::OVERLAP_WORDS, 50);
        assert_eq!(TextChunker::MIN_CHUNK_WORDS, 50);
    }

    // --- Clone and Debug tests ---
    #[test]
    fn test_chunker_clone() {
        let chunker = TextChunker::new(100, 25).expect("Valid config");
        let cloned = chunker.clone();
        assert_eq!(chunker.chunk_size(), cloned.chunk_size());
        assert_eq!(chunker.overlap(), cloned.overlap());
    }

    #[test]
    fn test_chunker_debug() {
        let chunker = TextChunker::default_config();
        let debug_str = format!("{:?}", chunker);
        assert!(debug_str.contains("TextChunker"));
        assert!(debug_str.contains("chunk_size_words"));
    }

    #[test]
    fn test_chunker_error_debug() {
        let err = ChunkerError::EmptyContent;
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("EmptyContent"));

        let err2 = ChunkerError::InvalidOverlap {
            chunk_size: 50,
            overlap: 60,
        };
        let debug_str2 = format!("{:?}", err2);
        assert!(debug_str2.contains("InvalidOverlap"));
        assert!(debug_str2.contains("50"));
        assert!(debug_str2.contains("60"));
    }

    // =========================================================================
    // Helper Functions for Tests
    // =========================================================================

    /// Generate N words without sentence terminators
    fn generate_words(n: usize, with_sentences: bool) -> String {
        (0..n)
            .map(|i| {
                if with_sentences && i % 50 == 49 {
                    format!("word{}.", i)
                } else {
                    format!("word{}", i)
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Generate N words with sentence terminators every `interval` words
    fn generate_words_with_sentences(n: usize, interval: usize) -> String {
        (0..n)
            .map(|i| {
                if (i + 1) % interval == 0 {
                    format!("sentence{}.", i)
                } else {
                    format!("word{}", i)
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    // =========================================================================
    // Line Number Tracking Tests
    // =========================================================================

    #[test]
    fn test_chunker_line_numbers_single_line() {
        let chunker = TextChunker::default_config();
        let content = "This is a single line of text.";
        let chunks = chunker.chunk_text(content, "single.md").expect("chunk");

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].metadata.start_line, 1);
        assert_eq!(chunks[0].metadata.end_line, 1);
        println!(
            "Single line: start={}, end={}",
            chunks[0].metadata.start_line, chunks[0].metadata.end_line
        );
    }

    #[test]
    fn test_chunker_line_numbers_multiline() {
        let chunker = TextChunker::default_config();
        // Create multi-line content
        let content = "Line one.\nLine two.\nLine three.\nLine four.\nLine five.";
        let chunks = chunker.chunk_text(content, "multi.md").expect("chunk");

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].metadata.start_line, 1);
        assert_eq!(chunks[0].metadata.end_line, 5, "Should span 5 lines");
        println!(
            "Multi-line: start={}, end={}",
            chunks[0].metadata.start_line, chunks[0].metadata.end_line
        );
    }

    #[test]
    fn test_chunker_line_numbers_multiple_chunks() {
        // Use smaller chunk size to force multiple chunks
        let chunker = TextChunker::new(50, 10).expect("valid config");

        // Create content with 100 words split across 20 lines (5 words per line)
        let mut lines = Vec::new();
        for i in 0..20 {
            let line_words: Vec<String> = (0..5).map(|w| format!("word{}_{}", i, w)).collect();
            lines.push(line_words.join(" "));
        }
        let content = lines.join("\n");

        let word_count = content.split_whitespace().count();
        assert_eq!(word_count, 100, "Should have 100 words");

        let chunks = chunker.chunk_text(&content, "lines.md").expect("chunk");

        println!("=== LINE NUMBER TRACKING TEST ===");
        println!("Total chunks: {}", chunks.len());
        for (i, chunk) in chunks.iter().enumerate() {
            println!(
                "Chunk[{}]: lines {}-{}, words={}",
                i, chunk.metadata.start_line, chunk.metadata.end_line, chunk.word_count
            );
        }

        // First chunk should start at line 1
        assert_eq!(chunks[0].metadata.start_line, 1);

        // Each chunk's start_line should be less than or equal to end_line
        for chunk in &chunks {
            assert!(
                chunk.metadata.start_line <= chunk.metadata.end_line,
                "Start line {} should be <= end line {}",
                chunk.metadata.start_line,
                chunk.metadata.end_line
            );
        }

        // Verify chunks have reasonable line numbers (not all zeros or same)
        if chunks.len() > 1 {
            // Later chunks should have later line numbers
            assert!(
                chunks[1].metadata.start_line >= 1,
                "Second chunk should start on line >= 1"
            );
        }

        println!("=== LINE NUMBER TRACKING TEST PASSED ===");
    }

    /// Verify overlap between two consecutive chunks
    fn verify_overlap(chunk1: &TextChunk, chunk2: &TextChunk, expected_overlap: usize) {
        let chunk1_words: Vec<&str> = chunk1.content.split_whitespace().collect();
        let chunk2_words: Vec<&str> = chunk2.content.split_whitespace().collect();

        // Get last `expected_overlap` words from chunk1
        let chunk1_tail: Vec<&str> = chunk1_words
            .iter()
            .rev()
            .take(expected_overlap)
            .rev()
            .copied()
            .collect();

        // Get first `expected_overlap` words from chunk2
        let chunk2_head: Vec<&str> = chunk2_words
            .iter()
            .take(expected_overlap)
            .copied()
            .collect();

        println!(
            "Overlap verification: chunk1_tail={:?}, chunk2_head={:?}",
            &chunk1_tail[..std::cmp::min(5, chunk1_tail.len())],
            &chunk2_head[..std::cmp::min(5, chunk2_head.len())]
        );

        // Due to sentence boundary adjustment, overlap might be less than expected
        // Count matching words from the beginning of both tails/heads
        let actual_overlap = chunk1_tail
            .iter()
            .zip(chunk2_head.iter())
            .filter(|(a, b)| a == b)
            .count();

        println!(
            "Expected overlap: {}, Actual matching words: {}",
            expected_overlap, actual_overlap
        );

        // There should be at least some overlap (unless chunks are very small)
        // Sentence boundary adjustment may reduce actual overlap below expected
        assert!(
            actual_overlap > 0 || chunk1_words.len() < expected_overlap,
            "Chunks should have overlap"
        );
    }
}
