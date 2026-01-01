//! Multi-modal input types for the embedding pipeline.
//!
//! ModelInput provides a unified interface for passing different types of content
//! to the embedding models, allowing each model to handle inputs it supports.
//!
//! # Design Principles
//!
//! - **NO FALLBACKS**: Empty content returns `EmbeddingError::InvalidInput` immediately
//! - **NO MOCK DATA**: All validation is real, no stubs
//! - **DETERMINISTIC HASHING**: `content_hash()` uses xxhash64 for cache keying
//!
//! # Supported Input Types
//!
//! | Variant | Models | Metadata |
//! |---------|--------|----------|
//! | Text | E1, E5-E6, E8-E9, E11-E12 | Optional instruction prefix |
//! | Code | E7 (CodeBERT) | Language identifier |
//! | Image | E10 (CLIP) | Image format |
//! | Audio | Future | Sample rate, channels |

use crate::error::{EmbeddingError, EmbeddingResult};
use serde::{Deserialize, Serialize};
use xxhash_rust::xxh64::xxh64;

/// Image format for binary image inputs.
///
/// Supports detection via magic bytes for automatic format identification.
///
/// # Example
///
/// ```rust
/// use context_graph_embeddings::types::ImageFormat;
///
/// // Detect PNG from magic bytes
/// let png_bytes = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
/// assert_eq!(ImageFormat::detect(&png_bytes), Some(ImageFormat::Png));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum ImageFormat {
    /// PNG format (magic: 0x89 0x50 0x4E 0x47)
    Png = 0,
    /// JPEG format (magic: 0xFF 0xD8 0xFF)
    Jpeg = 1,
    /// WebP format (magic: RIFF....WEBP)
    WebP = 2,
    /// GIF format (magic: GIF8)
    Gif = 3,
}

impl ImageFormat {
    /// Get MIME type for this image format.
    ///
    /// # Returns
    /// Standard MIME type string for HTTP Content-Type headers.
    #[must_use]
    pub const fn mime_type(&self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::WebP => "image/webp",
            Self::Gif => "image/gif",
        }
    }

    /// Get file extension for this image format.
    ///
    /// # Returns
    /// Lowercase extension without leading dot.
    #[must_use]
    pub const fn extension(&self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::WebP => "webp",
            Self::Gif => "gif",
        }
    }

    /// Try to detect format from magic bytes.
    ///
    /// Uses the first bytes of file content to identify format:
    /// - PNG: `\x89PNG` (4 bytes)
    /// - JPEG: `\xFF\xD8\xFF` (3 bytes)
    /// - GIF: `GIF8` (4 bytes)
    /// - WebP: `RIFF....WEBP` (12 bytes)
    ///
    /// # Arguments
    /// * `bytes` - Raw image bytes to analyze
    ///
    /// # Returns
    /// `Some(format)` if detected, `None` if format unknown or bytes too short.
    ///
    /// # Example
    ///
    /// ```rust
    /// use context_graph_embeddings::types::ImageFormat;
    ///
    /// // JPEG magic bytes
    /// let jpeg_start = [0xFF, 0xD8, 0xFF, 0xE0];
    /// assert_eq!(ImageFormat::detect(&jpeg_start), Some(ImageFormat::Jpeg));
    ///
    /// // Unknown format
    /// let unknown = [0x00, 0x01, 0x02, 0x03];
    /// assert_eq!(ImageFormat::detect(&unknown), None);
    /// ```
    #[must_use]
    pub fn detect(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 4 {
            return None;
        }

        // PNG magic: \x89PNG
        if bytes[0..4] == [0x89, 0x50, 0x4E, 0x47] {
            return Some(Self::Png);
        }

        // JPEG magic: \xFF\xD8\xFF (check first 3 bytes)
        if bytes[0..3] == [0xFF, 0xD8, 0xFF] {
            return Some(Self::Jpeg);
        }

        // GIF magic: GIF8 (GIF87a or GIF89a)
        if bytes[0..4] == [0x47, 0x49, 0x46, 0x38] {
            return Some(Self::Gif);
        }

        // WebP magic: RIFF....WEBP (requires 12 bytes)
        if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
            return Some(Self::WebP);
        }

        None
    }
}

impl std::fmt::Display for ImageFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.extension().to_uppercase())
    }
}

/// Multi-modal input for embedding models.
///
/// Each variant carries the data needed for that input type:
/// - Text: content string with optional instruction prefix (for e5-style models)
/// - Code: source code with language identifier
/// - Image: raw bytes with format information
/// - Audio: raw bytes with sample rate and channel count
///
/// # Validation
///
/// All constructors validate inputs and return `EmbeddingError::InvalidInput` for:
/// - Empty content/bytes
/// - Invalid parameters (e.g., sample_rate=0, channels not 1 or 2)
///
/// # Example
///
/// ```rust
/// use context_graph_embeddings::types::ModelInput;
///
/// // Create text input
/// let text_input = ModelInput::text("Hello, world!").unwrap();
/// assert!(text_input.is_text());
///
/// // Create code input with language
/// let code_input = ModelInput::code("fn main() {}", "rust").unwrap();
/// assert!(code_input.is_code());
///
/// // Hash for cache key
/// let hash = text_input.content_hash();
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ModelInput {
    /// Text content with optional instruction prefix.
    /// Instruction is prepended for e5-style models (e.g., "query: " or "passage: ").
    Text {
        /// The actual text content to embed.
        content: String,
        /// Optional instruction prefix for e5-style models.
        instruction: Option<String>,
    },
    /// Source code with programming language identifier.
    /// Language should be lowercase (e.g., "rust", "python", "javascript").
    Code {
        /// The source code content.
        content: String,
        /// Programming language identifier (lowercase).
        language: String,
    },
    /// Image bytes with format information.
    /// Bytes must be valid encoded image data (not raw pixels).
    Image {
        /// Raw encoded image bytes (PNG, JPEG, WebP, or GIF).
        bytes: Vec<u8>,
        /// Image format for proper decoding.
        format: ImageFormat,
    },
    /// Audio bytes with sample metadata.
    /// For future audio embedding models.
    Audio {
        /// Raw audio bytes (PCM or encoded).
        bytes: Vec<u8>,
        /// Sample rate in Hz (e.g., 16000, 44100).
        sample_rate: u32,
        /// Number of channels: 1 = mono, 2 = stereo.
        channels: u8,
    },
}

impl ModelInput {
    /// Create a text input.
    ///
    /// # Arguments
    /// * `content` - Text content to embed (must not be empty)
    ///
    /// # Errors
    /// Returns `EmbeddingError::InvalidInput` if content is empty.
    ///
    /// # Example
    ///
    /// ```rust
    /// use context_graph_embeddings::types::ModelInput;
    ///
    /// let input = ModelInput::text("Hello, world!").unwrap();
    /// assert!(input.is_text());
    /// ```
    pub fn text(content: impl Into<String>) -> EmbeddingResult<Self> {
        let content = content.into();
        if content.is_empty() {
            return Err(EmbeddingError::InvalidInput(
                "Text content cannot be empty".to_string(),
            ));
        }
        Ok(Self::Text {
            content,
            instruction: None,
        })
    }

    /// Create a text input with instruction prefix.
    ///
    /// The instruction is prepended for e5-style models (e.g., "query: " or "passage: ").
    /// This helps the model understand the semantic role of the text.
    ///
    /// # Arguments
    /// * `content` - Text content to embed (must not be empty)
    /// * `instruction` - Instruction prefix (e.g., "query:", "passage:", "document:")
    ///
    /// # Errors
    /// Returns `EmbeddingError::InvalidInput` if content is empty.
    /// Note: Empty instruction is allowed (will be stored as Some("")).
    ///
    /// # Example
    ///
    /// ```rust
    /// use context_graph_embeddings::types::ModelInput;
    ///
    /// let query = ModelInput::text_with_instruction(
    ///     "What is Rust?",
    ///     "query:"
    /// ).unwrap();
    /// ```
    pub fn text_with_instruction(
        content: impl Into<String>,
        instruction: impl Into<String>,
    ) -> EmbeddingResult<Self> {
        let content = content.into();
        if content.is_empty() {
            return Err(EmbeddingError::InvalidInput(
                "Text content cannot be empty".to_string(),
            ));
        }
        Ok(Self::Text {
            content,
            instruction: Some(instruction.into()),
        })
    }

    /// Create a code input.
    ///
    /// # Arguments
    /// * `content` - Source code content (must not be empty)
    /// * `language` - Programming language identifier (must not be empty)
    ///
    /// # Errors
    /// Returns `EmbeddingError::InvalidInput` if content or language is empty.
    ///
    /// # Supported Languages
    ///
    /// Common language identifiers (validation not enforced):
    /// - rust, python, javascript, typescript
    /// - java, kotlin, scala, go, c, cpp, csharp
    /// - ruby, php, swift, sql, html, css
    /// - json, yaml, toml, bash, powershell
    ///
    /// # Example
    ///
    /// ```rust
    /// use context_graph_embeddings::types::ModelInput;
    ///
    /// let code = ModelInput::code(
    ///     "fn main() { println!(\"Hello\"); }",
    ///     "rust"
    /// ).unwrap();
    /// ```
    pub fn code(content: impl Into<String>, language: impl Into<String>) -> EmbeddingResult<Self> {
        let content = content.into();
        let language = language.into();

        if content.is_empty() {
            return Err(EmbeddingError::InvalidInput(
                "Code content cannot be empty".to_string(),
            ));
        }
        if language.is_empty() {
            return Err(EmbeddingError::InvalidInput(
                "Code language cannot be empty".to_string(),
            ));
        }

        Ok(Self::Code { content, language })
    }

    /// Create an image input.
    ///
    /// # Arguments
    /// * `bytes` - Raw encoded image bytes (must not be empty)
    /// * `format` - Image format (PNG, JPEG, WebP, or GIF)
    ///
    /// # Errors
    /// Returns `EmbeddingError::InvalidInput` if bytes is empty.
    ///
    /// # Example
    ///
    /// ```rust
    /// use context_graph_embeddings::types::{ModelInput, ImageFormat};
    ///
    /// let png_bytes = vec![0x89, 0x50, 0x4E, 0x47]; // PNG magic + data
    /// let image = ModelInput::image(png_bytes, ImageFormat::Png).unwrap();
    /// ```
    pub fn image(bytes: Vec<u8>, format: ImageFormat) -> EmbeddingResult<Self> {
        if bytes.is_empty() {
            return Err(EmbeddingError::InvalidInput(
                "Image bytes cannot be empty".to_string(),
            ));
        }
        Ok(Self::Image { bytes, format })
    }

    /// Create an audio input.
    ///
    /// # Arguments
    /// * `bytes` - Raw audio bytes (must not be empty)
    /// * `sample_rate` - Sample rate in Hz (must be > 0, e.g., 16000, 44100)
    /// * `channels` - Number of channels (must be 1 for mono or 2 for stereo)
    ///
    /// # Errors
    /// Returns `EmbeddingError::InvalidInput` if:
    /// - bytes is empty
    /// - sample_rate is 0
    /// - channels is not 1 or 2
    ///
    /// # Example
    ///
    /// ```rust
    /// use context_graph_embeddings::types::ModelInput;
    ///
    /// let audio_bytes = vec![0u8; 1024]; // PCM samples
    /// let audio = ModelInput::audio(audio_bytes, 16000, 1).unwrap(); // 16kHz mono
    /// ```
    pub fn audio(bytes: Vec<u8>, sample_rate: u32, channels: u8) -> EmbeddingResult<Self> {
        if bytes.is_empty() {
            return Err(EmbeddingError::InvalidInput(
                "Audio bytes cannot be empty".to_string(),
            ));
        }
        if sample_rate == 0 {
            return Err(EmbeddingError::InvalidInput(
                "Audio sample_rate cannot be 0".to_string(),
            ));
        }
        if channels != 1 && channels != 2 {
            return Err(EmbeddingError::InvalidInput(format!(
                "Audio channels must be 1 (mono) or 2 (stereo), got {}",
                channels
            )));
        }
        Ok(Self::Audio {
            bytes,
            sample_rate,
            channels,
        })
    }

    /// Compute xxHash64 of the content for cache keying.
    ///
    /// Hash includes all content bytes:
    /// - Text: UTF-8 bytes of content + instruction if present
    /// - Code: UTF-8 bytes of content + language
    /// - Image: raw bytes + format discriminant
    /// - Audio: raw bytes + sample_rate (little-endian) + channels
    ///
    /// # Returns
    /// 64-bit hash value. Deterministic for identical inputs.
    ///
    /// # Example
    ///
    /// ```rust
    /// use context_graph_embeddings::types::ModelInput;
    ///
    /// let input1 = ModelInput::text("Hello").unwrap();
    /// let input2 = ModelInput::text("Hello").unwrap();
    /// assert_eq!(input1.content_hash(), input2.content_hash());
    ///
    /// let input3 = ModelInput::text("World").unwrap();
    /// assert_ne!(input1.content_hash(), input3.content_hash());
    /// ```
    #[must_use]
    pub fn content_hash(&self) -> u64 {
        match self {
            Self::Text {
                content,
                instruction,
            } => {
                let mut data = content.as_bytes().to_vec();
                match instruction {
                    Some(inst) => {
                        // Discriminator byte 0x01 indicates Some (even for empty string)
                        data.push(0x01);
                        data.extend_from_slice(inst.as_bytes());
                    }
                    None => {
                        // Discriminator byte 0x00 indicates None
                        data.push(0x00);
                    }
                }
                xxh64(&data, 0)
            }
            Self::Code { content, language } => {
                let mut data = content.as_bytes().to_vec();
                data.extend_from_slice(language.as_bytes());
                xxh64(&data, 0)
            }
            Self::Image { bytes, format } => {
                let mut data = bytes.clone();
                data.push(*format as u8);
                xxh64(&data, 0)
            }
            Self::Audio {
                bytes,
                sample_rate,
                channels,
            } => {
                let mut data = bytes.clone();
                data.extend_from_slice(&sample_rate.to_le_bytes());
                data.push(*channels);
                xxh64(&data, 0)
            }
        }
    }

    /// Calculate total memory size in bytes.
    ///
    /// Includes heap allocations for strings and byte vectors.
    /// Used by MemoryTracker (M03-L02) for memory budget management.
    ///
    /// # Returns
    /// Approximate memory usage including:
    /// - String heap allocations (capacity, not just len for accuracy)
    /// - Vec<u8> heap allocations
    /// - Struct overhead is NOT included (stack-allocated)
    ///
    /// # Example
    ///
    /// ```rust
    /// use context_graph_embeddings::types::ModelInput;
    ///
    /// let input = ModelInput::text("Hello, world!").unwrap();
    /// let size = input.byte_size();
    /// assert!(size >= 13); // At least the string length
    /// ```
    #[must_use]
    pub fn byte_size(&self) -> usize {
        match self {
            Self::Text {
                content,
                instruction,
            } => {
                content.len()
                    + instruction
                        .as_ref()
                        .map_or(0, |s| s.len())
            }
            Self::Code { content, language } => content.len() + language.len(),
            Self::Image { bytes, .. } => bytes.len(),
            Self::Audio { bytes, .. } => bytes.len(),
        }
    }

    /// Returns true if this is a Text variant.
    #[must_use]
    pub const fn is_text(&self) -> bool {
        matches!(self, Self::Text { .. })
    }

    /// Returns true if this is a Code variant.
    #[must_use]
    pub const fn is_code(&self) -> bool {
        matches!(self, Self::Code { .. })
    }

    /// Returns true if this is an Image variant.
    #[must_use]
    pub const fn is_image(&self) -> bool {
        matches!(self, Self::Image { .. })
    }

    /// Returns true if this is an Audio variant.
    #[must_use]
    pub const fn is_audio(&self) -> bool {
        matches!(self, Self::Audio { .. })
    }

    /// Get text content if this is a Text variant.
    ///
    /// # Returns
    /// `Some((content, instruction))` where instruction is `Some(&str)` if set,
    /// or `None` if this is not a Text variant.
    #[must_use]
    pub fn as_text(&self) -> Option<(&str, Option<&str>)> {
        match self {
            Self::Text {
                content,
                instruction,
            } => Some((content.as_str(), instruction.as_deref())),
            _ => None,
        }
    }

    /// Get code content if this is a Code variant.
    ///
    /// # Returns
    /// `Some((content, language))` or `None` if not a Code variant.
    #[must_use]
    pub fn as_code(&self) -> Option<(&str, &str)> {
        match self {
            Self::Code { content, language } => Some((content.as_str(), language.as_str())),
            _ => None,
        }
    }

    /// Get image bytes if this is an Image variant.
    ///
    /// # Returns
    /// `Some((bytes, format))` or `None` if not an Image variant.
    #[must_use]
    pub fn as_image(&self) -> Option<(&[u8], ImageFormat)> {
        match self {
            Self::Image { bytes, format } => Some((bytes.as_slice(), *format)),
            _ => None,
        }
    }

    /// Get audio bytes if this is an Audio variant.
    ///
    /// # Returns
    /// `Some((bytes, sample_rate, channels))` or `None` if not an Audio variant.
    #[must_use]
    pub fn as_audio(&self) -> Option<(&[u8], u32, u8)> {
        match self {
            Self::Audio {
                bytes,
                sample_rate,
                channels,
            } => Some((bytes.as_slice(), *sample_rate, *channels)),
            _ => None,
        }
    }
}

impl std::fmt::Display for ModelInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text { content, instruction } => {
                let preview: String = content.chars().take(50).collect();
                let suffix = if content.len() > 50 { "..." } else { "" };
                match instruction {
                    Some(inst) => write!(f, "Text[{}: {}{}]", inst, preview, suffix),
                    None => write!(f, "Text[{}{}]", preview, suffix),
                }
            }
            Self::Code { content, language } => {
                let preview: String = content.chars().take(30).collect();
                let suffix = if content.len() > 30 { "..." } else { "" };
                write!(f, "Code[{}: {}{}]", language, preview, suffix)
            }
            Self::Image { bytes, format } => {
                write!(f, "Image[{}: {} bytes]", format, bytes.len())
            }
            Self::Audio {
                bytes,
                sample_rate,
                channels,
            } => {
                let ch_str = if *channels == 1 { "mono" } else { "stereo" };
                write!(f, "Audio[{}Hz {}: {} bytes]", sample_rate, ch_str, bytes.len())
            }
        }
    }
}

/// Input type capability descriptor for model compatibility checking.
///
/// Unlike `ModelInput` which carries actual data, `InputType` is a simple
/// discriminator used to:
/// - Query what input types a model supports
/// - Route inputs to compatible models
/// - Reject unsupported inputs early (fail-fast)
///
/// # Model Compatibility Matrix
///
/// | Model | Text | Code | Image | Audio |
/// |-------|------|------|-------|-------|
/// | Semantic (E1) | ✓ | ✓* | ✗ | ✗ |
/// | TemporalRecent (E2) | ✓ | ✓ | ✗ | ✗ |
/// | TemporalPeriodic (E3) | ✓ | ✓ | ✗ | ✗ |
/// | TemporalPositional (E4) | ✓ | ✓ | ✗ | ✗ |
/// | Causal (E5) | ✓ | ✓ | ✗ | ✗ |
/// | Sparse (E6) | ✓ | ✓* | ✗ | ✗ |
/// | Code (E7) | ✓* | ✓ | ✗ | ✗ |
/// | Graph (E8) | ✓ | ✓* | ✗ | ✗ |
/// | HDC (E9) | ✓ | ✓ | ✗ | ✗ |
/// | Multimodal (E10) | ✓ | ✗ | ✓ | ✗ |
/// | Entity (E11) | ✓ | ✓* | ✗ | ✗ |
/// | LateInteraction (E12) | ✓ | ✓* | ✗ | ✗ |
///
/// *Model can process but is not optimized for this type
///
/// # Example
///
/// ```rust
/// use context_graph_embeddings::types::{InputType, ModelInput};
/// use std::collections::HashSet;
///
/// // Query input type from ModelInput
/// let input = ModelInput::text("Hello").unwrap();
/// let input_type = InputType::from(&input);
/// assert_eq!(input_type, InputType::Text);
///
/// // Use in HashSet for model capability checking
/// let mut supported: HashSet<InputType> = HashSet::new();
/// supported.insert(InputType::Text);
/// supported.insert(InputType::Code);
/// assert!(supported.contains(&InputType::Text));
/// assert!(!supported.contains(&InputType::Image));
///
/// // Check all variants
/// for input_type in InputType::all() {
///     println!("Type: {}", input_type);
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum InputType {
    /// Text content (natural language, documents, queries)
    Text = 0,
    /// Source code with language metadata
    Code = 1,
    /// Image data (PNG, JPEG, WebP, GIF)
    Image = 2,
    /// Audio data (PCM, encoded)
    Audio = 3,
}

impl InputType {
    /// Returns a static slice containing all InputType variants.
    ///
    /// Useful for iteration when checking model compatibility across all types.
    ///
    /// # Returns
    /// Static slice with all 4 variants in discriminant order.
    ///
    /// # Example
    ///
    /// ```rust
    /// use context_graph_embeddings::types::InputType;
    ///
    /// let all_types = InputType::all();
    /// assert_eq!(all_types.len(), 4);
    /// assert_eq!(all_types[0], InputType::Text);
    /// ```
    #[must_use]
    pub const fn all() -> &'static [InputType] {
        &[
            InputType::Text,
            InputType::Code,
            InputType::Image,
            InputType::Audio,
        ]
    }

    /// Returns the discriminant value (0-3).
    ///
    /// Matches the `#[repr(u8)]` values for binary serialization.
    #[must_use]
    pub const fn discriminant(&self) -> u8 {
        *self as u8
    }
}

impl std::fmt::Display for InputType {
    /// Displays lowercase type name: "text", "code", "image", "audio".
    ///
    /// This format is used in error messages, logging, and configuration.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::Text => "text",
            Self::Code => "code",
            Self::Image => "image",
            Self::Audio => "audio",
        };
        write!(f, "{}", name)
    }
}

impl From<&ModelInput> for InputType {
    /// Converts a ModelInput reference to its corresponding InputType.
    ///
    /// This is the primary bridge between the data-carrying `ModelInput`
    /// and the capability-describing `InputType`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use context_graph_embeddings::types::{InputType, ModelInput, ImageFormat};
    ///
    /// let text = ModelInput::text("Hello").unwrap();
    /// assert_eq!(InputType::from(&text), InputType::Text);
    ///
    /// let code = ModelInput::code("fn main() {}", "rust").unwrap();
    /// assert_eq!(InputType::from(&code), InputType::Code);
    ///
    /// let image = ModelInput::image(vec![1,2,3], ImageFormat::Png).unwrap();
    /// assert_eq!(InputType::from(&image), InputType::Image);
    ///
    /// let audio = ModelInput::audio(vec![1,2,3], 16000, 1).unwrap();
    /// assert_eq!(InputType::from(&audio), InputType::Audio);
    /// ```
    fn from(input: &ModelInput) -> Self {
        match input {
            ModelInput::Text { .. } => InputType::Text,
            ModelInput::Code { .. } => InputType::Code,
            ModelInput::Image { .. } => InputType::Image,
            ModelInput::Audio { .. } => InputType::Audio,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================
    // TEXT CONSTRUCTION TESTS (5 tests)
    // ============================================================

    #[test]
    fn test_text_with_valid_content_succeeds() {
        let input = ModelInput::text("Hello, world!");
        assert!(input.is_ok());
        let input = input.unwrap();
        assert!(input.is_text());
        let (content, instruction) = input.as_text().unwrap();
        assert_eq!(content, "Hello, world!");
        assert!(instruction.is_none());
    }

    #[test]
    fn test_text_with_empty_content_returns_invalid_input_error() {
        println!("BEFORE: Attempting to create ModelInput::text(\"\")");
        let result = ModelInput::text("");
        println!("AFTER: Result = {:?}", result);

        assert!(result.is_err());
        match result {
            Err(EmbeddingError::InvalidInput(msg)) => {
                assert!(msg.contains("empty"), "Error message should mention 'empty'");
            }
            _ => panic!("Expected InvalidInput error"),
        }
    }

    #[test]
    fn test_text_with_instruction_succeeds() {
        let input = ModelInput::text_with_instruction("What is Rust?", "query:");
        assert!(input.is_ok());
        let input = input.unwrap();
        let (content, instruction) = input.as_text().unwrap();
        assert_eq!(content, "What is Rust?");
        assert_eq!(instruction, Some("query:"));
    }

    #[test]
    fn test_text_with_instruction_stores_instruction_correctly() {
        let input = ModelInput::text_with_instruction("Document content", "passage:").unwrap();
        let (_, instruction) = input.as_text().unwrap();
        assert_eq!(instruction, Some("passage:"));

        // Empty instruction is allowed
        let input2 = ModelInput::text_with_instruction("Content", "").unwrap();
        let (_, instruction2) = input2.as_text().unwrap();
        assert_eq!(instruction2, Some(""));
    }

    #[test]
    fn test_text_with_instruction_empty_content_returns_invalid_input() {
        let result = ModelInput::text_with_instruction("", "query:");
        assert!(result.is_err());
        match result {
            Err(EmbeddingError::InvalidInput(msg)) => {
                assert!(msg.contains("empty"));
            }
            _ => panic!("Expected InvalidInput error"),
        }
    }

    // ============================================================
    // CODE CONSTRUCTION TESTS (4 tests)
    // ============================================================

    #[test]
    fn test_code_with_valid_content_and_language_succeeds() {
        let input = ModelInput::code("fn main() {}", "rust");
        assert!(input.is_ok());
        let input = input.unwrap();
        assert!(input.is_code());
        let (content, language) = input.as_code().unwrap();
        assert_eq!(content, "fn main() {}");
        assert_eq!(language, "rust");
    }

    #[test]
    fn test_code_with_empty_content_returns_invalid_input() {
        let result = ModelInput::code("", "rust");
        assert!(result.is_err());
        match result {
            Err(EmbeddingError::InvalidInput(msg)) => {
                assert!(msg.contains("content") && msg.contains("empty"));
            }
            _ => panic!("Expected InvalidInput error for empty content"),
        }
    }

    #[test]
    fn test_code_with_empty_language_returns_invalid_input() {
        let result = ModelInput::code("fn main() {}", "");
        assert!(result.is_err());
        match result {
            Err(EmbeddingError::InvalidInput(msg)) => {
                assert!(msg.contains("language") && msg.contains("empty"));
            }
            _ => panic!("Expected InvalidInput error for empty language"),
        }
    }

    #[test]
    fn test_code_stores_language_as_provided() {
        // Language is stored as-is (no normalization)
        let input = ModelInput::code("code", "Rust").unwrap();
        let (_, language) = input.as_code().unwrap();
        assert_eq!(language, "Rust");

        let input2 = ModelInput::code("code", "PYTHON").unwrap();
        let (_, language2) = input2.as_code().unwrap();
        assert_eq!(language2, "PYTHON");
    }

    // ============================================================
    // IMAGE CONSTRUCTION TESTS (3 tests)
    // ============================================================

    #[test]
    fn test_image_with_valid_bytes_succeeds() {
        let bytes = vec![0x89, 0x50, 0x4E, 0x47]; // PNG magic
        let input = ModelInput::image(bytes.clone(), ImageFormat::Png);
        assert!(input.is_ok());
        let input = input.unwrap();
        assert!(input.is_image());
        let (img_bytes, format) = input.as_image().unwrap();
        assert_eq!(img_bytes, bytes.as_slice());
        assert_eq!(format, ImageFormat::Png);
    }

    #[test]
    fn test_image_with_empty_bytes_returns_invalid_input() {
        let result = ModelInput::image(vec![], ImageFormat::Png);
        assert!(result.is_err());
        match result {
            Err(EmbeddingError::InvalidInput(msg)) => {
                assert!(msg.contains("empty"));
            }
            _ => panic!("Expected InvalidInput error"),
        }
    }

    #[test]
    fn test_image_stores_format_correctly() {
        let bytes = vec![1, 2, 3, 4];

        for format in [ImageFormat::Png, ImageFormat::Jpeg, ImageFormat::WebP, ImageFormat::Gif] {
            let input = ModelInput::image(bytes.clone(), format).unwrap();
            let (_, stored_format) = input.as_image().unwrap();
            assert_eq!(stored_format, format);
        }
    }

    // ============================================================
    // AUDIO CONSTRUCTION TESTS (5 tests)
    // ============================================================

    #[test]
    fn test_audio_with_valid_parameters_succeeds() {
        let bytes = vec![0u8; 1024];
        let input = ModelInput::audio(bytes.clone(), 16000, 1);
        assert!(input.is_ok());
        let input = input.unwrap();
        assert!(input.is_audio());
        let (audio_bytes, sample_rate, channels) = input.as_audio().unwrap();
        assert_eq!(audio_bytes, bytes.as_slice());
        assert_eq!(sample_rate, 16000);
        assert_eq!(channels, 1);
    }

    #[test]
    fn test_audio_with_empty_bytes_returns_invalid_input() {
        let result = ModelInput::audio(vec![], 16000, 1);
        assert!(result.is_err());
        match result {
            Err(EmbeddingError::InvalidInput(msg)) => {
                assert!(msg.contains("empty"));
            }
            _ => panic!("Expected InvalidInput error"),
        }
    }

    #[test]
    fn test_audio_with_sample_rate_zero_returns_invalid_input() {
        println!("BEFORE: audio bytes=[1,2,3], sample_rate=0, channels=1");
        let result = ModelInput::audio(vec![1, 2, 3], 0, 1);
        println!("AFTER: sample_rate=0 result = {:?}", result);

        assert!(result.is_err());
        match result {
            Err(EmbeddingError::InvalidInput(msg)) => {
                assert!(msg.contains("sample_rate"));
            }
            _ => panic!("Expected InvalidInput error"),
        }
    }

    #[test]
    fn test_audio_with_channels_zero_returns_invalid_input() {
        println!("BEFORE: audio bytes=[1,2,3], sample_rate=16000, channels=0");
        let result = ModelInput::audio(vec![1, 2, 3], 16000, 0);
        println!("AFTER: channels=0 result = {:?}", result);

        assert!(result.is_err());
        match result {
            Err(EmbeddingError::InvalidInput(msg)) => {
                assert!(msg.contains("channels") && msg.contains("1") && msg.contains("2"));
            }
            _ => panic!("Expected InvalidInput error"),
        }
    }

    #[test]
    fn test_audio_with_channels_three_returns_invalid_input() {
        println!("BEFORE: audio bytes=[1,2,3], sample_rate=16000, channels=3");
        let result = ModelInput::audio(vec![1, 2, 3], 16000, 3);
        println!("AFTER: channels=3 result = {:?}", result);

        assert!(result.is_err());
        match result {
            Err(EmbeddingError::InvalidInput(msg)) => {
                assert!(msg.contains("3"));
            }
            _ => panic!("Expected InvalidInput error"),
        }
    }

    // ============================================================
    // CONTENT HASH TESTS (4 tests)
    // ============================================================

    #[test]
    fn test_content_hash_same_for_identical_text_inputs() {
        let input1 = ModelInput::text("test content").unwrap();
        let hash1 = input1.content_hash();
        println!("BEFORE: First hash = {}", hash1);

        let input2 = ModelInput::text("test content").unwrap();
        let hash2 = input2.content_hash();
        println!("AFTER: Second hash = {}", hash2);

        assert_eq!(hash1, hash2, "Identical inputs must produce identical hashes");
    }

    #[test]
    fn test_content_hash_different_for_different_content() {
        let input1 = ModelInput::text("Hello").unwrap();
        let input2 = ModelInput::text("World").unwrap();
        let input3 = ModelInput::text("hello").unwrap(); // case sensitive

        assert_ne!(input1.content_hash(), input2.content_hash());
        assert_ne!(input1.content_hash(), input3.content_hash());
    }

    #[test]
    fn test_content_hash_includes_instruction_in_hash() {
        let without_inst = ModelInput::text("content").unwrap();
        let with_inst = ModelInput::text_with_instruction("content", "query:").unwrap();
        let with_empty_inst = ModelInput::text_with_instruction("content", "").unwrap();

        // All three should have different hashes
        assert_ne!(without_inst.content_hash(), with_inst.content_hash());
        assert_ne!(without_inst.content_hash(), with_empty_inst.content_hash());
        assert_ne!(with_inst.content_hash(), with_empty_inst.content_hash());
    }

    #[test]
    fn test_content_hash_includes_all_fields_for_each_variant() {
        // Code: different language = different hash
        let code1 = ModelInput::code("code", "rust").unwrap();
        let code2 = ModelInput::code("code", "python").unwrap();
        assert_ne!(code1.content_hash(), code2.content_hash());

        // Image: different format = different hash
        let img1 = ModelInput::image(vec![1, 2, 3], ImageFormat::Png).unwrap();
        let img2 = ModelInput::image(vec![1, 2, 3], ImageFormat::Jpeg).unwrap();
        assert_ne!(img1.content_hash(), img2.content_hash());

        // Audio: different sample_rate = different hash
        let audio1 = ModelInput::audio(vec![1, 2, 3], 16000, 1).unwrap();
        let audio2 = ModelInput::audio(vec![1, 2, 3], 44100, 1).unwrap();
        assert_ne!(audio1.content_hash(), audio2.content_hash());

        // Audio: different channels = different hash
        let audio3 = ModelInput::audio(vec![1, 2, 3], 16000, 2).unwrap();
        assert_ne!(audio1.content_hash(), audio3.content_hash());
    }

    // ============================================================
    // BYTE SIZE TESTS (3 tests)
    // ============================================================

    #[test]
    fn test_byte_size_returns_correct_size_for_text() {
        let content = "Hello, world!";
        let input = ModelInput::text(content).unwrap();
        let size = input.byte_size();
        assert_eq!(size, content.len());

        // With instruction
        let input_with_inst = ModelInput::text_with_instruction("Hello", "query:").unwrap();
        assert_eq!(input_with_inst.byte_size(), "Hello".len() + "query:".len());
    }

    #[test]
    fn test_byte_size_returns_correct_size_for_image_bytes() {
        let bytes = vec![0u8; 1000];
        let input = ModelInput::image(bytes.clone(), ImageFormat::Png).unwrap();
        assert_eq!(input.byte_size(), 1000);
    }

    #[test]
    fn test_byte_size_includes_string_heap_allocation() {
        let code_content = "fn main() {}";
        let language = "rust";
        let input = ModelInput::code(code_content, language).unwrap();

        // Should include both strings
        assert_eq!(input.byte_size(), code_content.len() + language.len());
    }

    // ============================================================
    // TYPE PREDICATE TESTS (4 tests)
    // ============================================================

    #[test]
    fn test_is_text_returns_true_only_for_text_variant() {
        let text = ModelInput::text("hello").unwrap();
        let code = ModelInput::code("code", "rust").unwrap();
        let image = ModelInput::image(vec![1], ImageFormat::Png).unwrap();
        let audio = ModelInput::audio(vec![1], 16000, 1).unwrap();

        assert!(text.is_text());
        assert!(!code.is_text());
        assert!(!image.is_text());
        assert!(!audio.is_text());
    }

    #[test]
    fn test_is_code_returns_true_only_for_code_variant() {
        let text = ModelInput::text("hello").unwrap();
        let code = ModelInput::code("code", "rust").unwrap();
        let image = ModelInput::image(vec![1], ImageFormat::Png).unwrap();
        let audio = ModelInput::audio(vec![1], 16000, 1).unwrap();

        assert!(!text.is_code());
        assert!(code.is_code());
        assert!(!image.is_code());
        assert!(!audio.is_code());
    }

    #[test]
    fn test_is_image_returns_true_only_for_image_variant() {
        let text = ModelInput::text("hello").unwrap();
        let code = ModelInput::code("code", "rust").unwrap();
        let image = ModelInput::image(vec![1], ImageFormat::Png).unwrap();
        let audio = ModelInput::audio(vec![1], 16000, 1).unwrap();

        assert!(!text.is_image());
        assert!(!code.is_image());
        assert!(image.is_image());
        assert!(!audio.is_image());
    }

    #[test]
    fn test_is_audio_returns_true_only_for_audio_variant() {
        let text = ModelInput::text("hello").unwrap();
        let code = ModelInput::code("code", "rust").unwrap();
        let image = ModelInput::image(vec![1], ImageFormat::Png).unwrap();
        let audio = ModelInput::audio(vec![1], 16000, 1).unwrap();

        assert!(!text.is_audio());
        assert!(!code.is_audio());
        assert!(!image.is_audio());
        assert!(audio.is_audio());
    }

    // ============================================================
    // IMAGE FORMAT TESTS (5 tests)
    // ============================================================

    #[test]
    fn test_image_format_mime_type() {
        assert_eq!(ImageFormat::Png.mime_type(), "image/png");
        assert_eq!(ImageFormat::Jpeg.mime_type(), "image/jpeg");
        assert_eq!(ImageFormat::WebP.mime_type(), "image/webp");
        assert_eq!(ImageFormat::Gif.mime_type(), "image/gif");
    }

    #[test]
    fn test_image_format_extension() {
        assert_eq!(ImageFormat::Png.extension(), "png");
        assert_eq!(ImageFormat::Jpeg.extension(), "jpg");
        assert_eq!(ImageFormat::WebP.extension(), "webp");
        assert_eq!(ImageFormat::Gif.extension(), "gif");
    }

    #[test]
    fn test_image_format_detect_png() {
        let png_bytes = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        assert_eq!(ImageFormat::detect(&png_bytes), Some(ImageFormat::Png));
    }

    #[test]
    fn test_image_format_detect_jpeg() {
        let jpeg_bytes = [0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46];
        assert_eq!(ImageFormat::detect(&jpeg_bytes), Some(ImageFormat::Jpeg));
    }

    #[test]
    fn test_image_format_detect_gif() {
        let gif_bytes = [0x47, 0x49, 0x46, 0x38, 0x39, 0x61]; // GIF89a
        assert_eq!(ImageFormat::detect(&gif_bytes), Some(ImageFormat::Gif));
    }

    #[test]
    fn test_image_format_detect_webp() {
        let webp_bytes = [
            0x52, 0x49, 0x46, 0x46, // RIFF
            0x00, 0x00, 0x00, 0x00, // size (ignored)
            0x57, 0x45, 0x42, 0x50, // WEBP
        ];
        assert_eq!(ImageFormat::detect(&webp_bytes), Some(ImageFormat::WebP));
    }

    #[test]
    fn test_image_format_detect_unknown() {
        let unknown = [0x00, 0x01, 0x02, 0x03, 0x04, 0x05];
        assert_eq!(ImageFormat::detect(&unknown), None);
    }

    #[test]
    fn test_image_format_detect_too_short() {
        let short = [0x89, 0x50, 0x4E]; // Only 3 bytes, need 4 for PNG
        assert_eq!(ImageFormat::detect(&short), None);
    }

    // ============================================================
    // ACCESSOR TESTS (4 tests)
    // ============================================================

    #[test]
    fn test_as_text_returns_none_for_non_text() {
        let code = ModelInput::code("code", "rust").unwrap();
        assert!(code.as_text().is_none());

        let image = ModelInput::image(vec![1], ImageFormat::Png).unwrap();
        assert!(image.as_text().is_none());
    }

    #[test]
    fn test_as_code_returns_none_for_non_code() {
        let text = ModelInput::text("hello").unwrap();
        assert!(text.as_code().is_none());

        let audio = ModelInput::audio(vec![1], 16000, 1).unwrap();
        assert!(audio.as_code().is_none());
    }

    #[test]
    fn test_as_image_returns_none_for_non_image() {
        let text = ModelInput::text("hello").unwrap();
        assert!(text.as_image().is_none());

        let code = ModelInput::code("code", "rust").unwrap();
        assert!(code.as_image().is_none());
    }

    #[test]
    fn test_as_audio_returns_none_for_non_audio() {
        let text = ModelInput::text("hello").unwrap();
        assert!(text.as_audio().is_none());

        let image = ModelInput::image(vec![1], ImageFormat::Png).unwrap();
        assert!(image.as_audio().is_none());
    }

    // ============================================================
    // DISPLAY AND SERDE TESTS (2 tests)
    // ============================================================

    #[test]
    fn test_display_formatting() {
        let text = ModelInput::text("Hello, world!").unwrap();
        let display = format!("{}", text);
        assert!(display.contains("Text"));
        assert!(display.contains("Hello"));

        let code = ModelInput::code("fn main() {}", "rust").unwrap();
        let display = format!("{}", code);
        assert!(display.contains("Code"));
        assert!(display.contains("rust"));

        let image = ModelInput::image(vec![1, 2, 3], ImageFormat::Png).unwrap();
        let display = format!("{}", image);
        assert!(display.contains("Image"));
        assert!(display.contains("PNG"));
        assert!(display.contains("3 bytes"));

        let audio = ModelInput::audio(vec![1, 2, 3, 4], 44100, 2).unwrap();
        let display = format!("{}", audio);
        assert!(display.contains("Audio"));
        assert!(display.contains("44100"));
        assert!(display.contains("stereo"));
    }

    #[test]
    fn test_serde_round_trip() {
        let text = ModelInput::text("Hello").unwrap();
        let json = serde_json::to_string(&text).unwrap();
        let recovered: ModelInput = serde_json::from_str(&json).unwrap();
        assert_eq!(text, recovered);

        let code = ModelInput::code("fn main() {}", "rust").unwrap();
        let json = serde_json::to_string(&code).unwrap();
        let recovered: ModelInput = serde_json::from_str(&json).unwrap();
        assert_eq!(code, recovered);

        let image = ModelInput::image(vec![1, 2, 3], ImageFormat::Jpeg).unwrap();
        let json = serde_json::to_string(&image).unwrap();
        let recovered: ModelInput = serde_json::from_str(&json).unwrap();
        assert_eq!(image, recovered);

        let audio = ModelInput::audio(vec![1, 2, 3], 16000, 1).unwrap();
        let json = serde_json::to_string(&audio).unwrap();
        let recovered: ModelInput = serde_json::from_str(&json).unwrap();
        assert_eq!(audio, recovered);
    }

    // ============================================================
    // STEREO AUDIO TEST
    // ============================================================

    #[test]
    fn test_audio_stereo_accepted() {
        let result = ModelInput::audio(vec![1, 2, 3], 44100, 2);
        assert!(result.is_ok());
        let input = result.unwrap();
        let (_, sample_rate, channels) = input.as_audio().unwrap();
        assert_eq!(sample_rate, 44100);
        assert_eq!(channels, 2);
    }

    // ============================================================
    // EDGE CASE: HASH DETERMINISM ACROSS CALLS
    // ============================================================

    #[test]
    fn test_hash_determinism_multiple_calls() {
        let input = ModelInput::text("deterministic content").unwrap();

        let hash1 = input.content_hash();
        let hash2 = input.content_hash();
        let hash3 = input.content_hash();

        assert_eq!(hash1, hash2);
        assert_eq!(hash2, hash3);
        println!("Hash determinism verified: {} == {} == {}", hash1, hash2, hash3);
    }

    // ============================================================
    // EDGE CASE: LARGE INPUTS
    // ============================================================

    #[test]
    fn test_large_text_input() {
        let large_content = "x".repeat(100_000);
        let input = ModelInput::text(&large_content).unwrap();
        assert_eq!(input.byte_size(), 100_000);

        // Hash should still work
        let hash = input.content_hash();
        assert_ne!(hash, 0);
    }

    #[test]
    fn test_large_image_input() {
        let large_bytes = vec![0xFFu8; 1_000_000]; // 1MB
        let input = ModelInput::image(large_bytes, ImageFormat::Jpeg).unwrap();
        assert_eq!(input.byte_size(), 1_000_000);

        // Hash should still work
        let hash = input.content_hash();
        assert_ne!(hash, 0);
    }

    // ============================================================
    // INPUT TYPE TESTS (M03-F07)
    // ============================================================

    #[test]
    fn test_input_type_from_model_input_text() {
        println!("BEFORE: Creating ModelInput::Text");
        let input = ModelInput::text("Hello").unwrap();
        println!("AFTER: Created input = {:?}", input);

        let input_type = InputType::from(&input);
        println!("RESULT: InputType = {:?}", input_type);

        assert_eq!(input_type, InputType::Text);
    }

    #[test]
    fn test_input_type_from_model_input_code() {
        println!("BEFORE: Creating ModelInput::Code");
        let input = ModelInput::code("fn main() {}", "rust").unwrap();
        println!("AFTER: Created input = {:?}", input);

        let input_type = InputType::from(&input);
        println!("RESULT: InputType = {:?}", input_type);

        assert_eq!(input_type, InputType::Code);
    }

    #[test]
    fn test_input_type_from_model_input_image() {
        println!("BEFORE: Creating ModelInput::Image");
        let input = ModelInput::image(vec![1, 2, 3, 4], ImageFormat::Png).unwrap();
        println!("AFTER: Created input = {:?}", input);

        let input_type = InputType::from(&input);
        println!("RESULT: InputType = {:?}", input_type);

        assert_eq!(input_type, InputType::Image);
    }

    #[test]
    fn test_input_type_from_model_input_audio() {
        println!("BEFORE: Creating ModelInput::Audio");
        let input = ModelInput::audio(vec![1, 2, 3, 4], 16000, 1).unwrap();
        println!("AFTER: Created input = {:?}", input);

        let input_type = InputType::from(&input);
        println!("RESULT: InputType = {:?}", input_type);

        assert_eq!(input_type, InputType::Audio);
    }

    #[test]
    fn test_input_type_display_lowercase() {
        assert_eq!(format!("{}", InputType::Text), "text");
        assert_eq!(format!("{}", InputType::Code), "code");
        assert_eq!(format!("{}", InputType::Image), "image");
        assert_eq!(format!("{}", InputType::Audio), "audio");
    }

    #[test]
    fn test_input_type_all_returns_4_variants() {
        let all = InputType::all();
        assert_eq!(all.len(), 4);
        assert_eq!(all[0], InputType::Text);
        assert_eq!(all[1], InputType::Code);
        assert_eq!(all[2], InputType::Image);
        assert_eq!(all[3], InputType::Audio);
    }

    #[test]
    fn test_input_type_can_be_used_as_hashmap_key() {
        use std::collections::HashMap;

        let mut map: HashMap<InputType, &str> = HashMap::new();
        map.insert(InputType::Text, "text_value");
        map.insert(InputType::Code, "code_value");
        map.insert(InputType::Image, "image_value");
        map.insert(InputType::Audio, "audio_value");

        assert_eq!(map.get(&InputType::Text), Some(&"text_value"));
        assert_eq!(map.get(&InputType::Code), Some(&"code_value"));
        assert_eq!(map.get(&InputType::Image), Some(&"image_value"));
        assert_eq!(map.get(&InputType::Audio), Some(&"audio_value"));
    }

    #[test]
    fn test_input_type_can_be_used_in_hashset() {
        use std::collections::HashSet;

        let mut set: HashSet<InputType> = HashSet::new();
        set.insert(InputType::Text);
        set.insert(InputType::Code);

        assert!(set.contains(&InputType::Text));
        assert!(set.contains(&InputType::Code));
        assert!(!set.contains(&InputType::Image));
        assert!(!set.contains(&InputType::Audio));
    }

    #[test]
    fn test_input_type_copy_semantics() {
        let original = InputType::Text;
        let copied = original; // Copy, not move
        assert_eq!(original, copied); // Both still valid
    }

    #[test]
    fn test_input_type_serde_roundtrip() {
        for input_type in InputType::all() {
            let json = serde_json::to_string(input_type).unwrap();
            let recovered: InputType = serde_json::from_str(&json).unwrap();
            assert_eq!(*input_type, recovered);
            println!("Serialized {:?} as {} and recovered successfully", input_type, json);
        }
    }

    #[test]
    fn test_input_type_discriminant_values() {
        assert_eq!(InputType::Text.discriminant(), 0);
        assert_eq!(InputType::Code.discriminant(), 1);
        assert_eq!(InputType::Image.discriminant(), 2);
        assert_eq!(InputType::Audio.discriminant(), 3);
    }

    #[test]
    fn test_input_type_debug_formatting() {
        // Debug should show variant name
        assert_eq!(format!("{:?}", InputType::Text), "Text");
        assert_eq!(format!("{:?}", InputType::Code), "Code");
        assert_eq!(format!("{:?}", InputType::Image), "Image");
        assert_eq!(format!("{:?}", InputType::Audio), "Audio");
    }

    #[test]
    fn test_input_type_equality() {
        // Same types should be equal
        assert_eq!(InputType::Text, InputType::Text);
        assert_eq!(InputType::Code, InputType::Code);
        assert_eq!(InputType::Image, InputType::Image);
        assert_eq!(InputType::Audio, InputType::Audio);

        // Different types should not be equal
        assert_ne!(InputType::Text, InputType::Code);
        assert_ne!(InputType::Text, InputType::Image);
        assert_ne!(InputType::Text, InputType::Audio);
        assert_ne!(InputType::Code, InputType::Image);
        assert_ne!(InputType::Code, InputType::Audio);
        assert_ne!(InputType::Image, InputType::Audio);
    }
}
