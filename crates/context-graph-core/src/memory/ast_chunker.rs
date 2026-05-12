//! Deprecated compatibility layer for code chunking.
//!
//! New code should call [`crate::memory::ast`] directly. This module remains so
//! older code-capture call sites keep compiling while using the same canonical,
//! multi-language parser and fail-closed diagnostics as the ME-JEPA path.

#![allow(deprecated)] // This module is the deprecated compatibility boundary.

use crate::memory::ast::{
    self, AstChunk as CanonicalAstChunk, AstChunkOptions, AstChunkerError as CanonicalAstError,
    EntityType as CanonicalEntityType, Language, ParseDiagnostic as CanonicalDiagnostic,
};
use sha2::{Digest, Sha256};
use std::path::Path;
use thiserror::Error;
use tracing::error;

/// Errors that can occur during deprecated AST-based code chunking.
#[derive(Debug, Error)]
pub enum AstChunkerError {
    /// Failed to parse the source code.
    #[error("Failed to parse source {file_path}: {reason}; diagnostics: {diagnostics:?}")]
    ParseFailed {
        file_path: String,
        reason: String,
        diagnostics: Vec<AstParseDiagnostic>,
    },

    /// Failed to set the parser language.
    #[error("Failed to set language for parser: {language}")]
    LanguageSetFailed { language: String },

    /// Source code is empty or contains only whitespace.
    #[error("Source code is empty or contains only whitespace")]
    EmptySource,

    /// Unsupported file extension.
    #[error("Unsupported file extension: {extension}")]
    UnsupportedExtension { extension: String },
}

impl AstChunkerError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::ParseFailed { .. } => "MEJEPA_CHUNKER_PARSE_FAILED",
            Self::LanguageSetFailed { .. } => "MEJEPA_CHUNKER_LANGUAGE_SET_FAILED",
            Self::EmptySource => "MEJEPA_CHUNKER_EMPTY_SOURCE",
            Self::UnsupportedExtension { .. } => "MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE",
        }
    }
}

/// Byte-precise parser diagnostic captured before failing closed.
#[derive(Debug, Clone, PartialEq, Eq)]
#[deprecated(
    since = "0.1.0",
    note = "use context_graph_core::memory::ast::{chunk_path, chunk_with_options, AstChunkOptions}"
)]
pub struct AstParseDiagnostic {
    pub kind: String,
    pub is_error: bool,
    pub is_missing: bool,
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub excerpt: String,
}

/// Configuration retained for source compatibility with the legacy API.
#[derive(Debug, Clone)]
#[deprecated(
    since = "0.1.0",
    note = "use context_graph_core::memory::ast::{chunk_path, chunk_with_options, AstChunkOptions}"
)]
pub struct AstChunkConfig {
    pub target_size: usize,
    pub min_size: usize,
    pub max_size: usize,
    pub include_parent_context: bool,
    pub include_imports: bool,
}

impl Default for AstChunkConfig {
    fn default() -> Self {
        Self {
            target_size: 500,
            min_size: 100,
            max_size: 1000,
            include_parent_context: true,
            include_imports: true,
        }
    }
}

/// Entity type detected in the AST.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[deprecated(
    since = "0.1.0",
    note = "use context_graph_core::memory::ast::{chunk_path, chunk_with_options, AstChunkOptions}"
)]
pub enum EntityType {
    Function,
    Method,
    Class,
    Struct,
    Enum,
    Trait,
    TraitOrInterface,
    Impl,
    Module,
    Namespace,
    TestFunction,
    Import,
    Const,
    Static,
    TypeAlias,
    Macro,
    Comment,
    CommentBlock,
    Docstring,
    Mixed,
    Unknown,
}

/// Metadata for a code chunk.
#[derive(Debug, Clone)]
#[deprecated(
    since = "0.1.0",
    note = "use context_graph_core::memory::ast::{chunk_path, chunk_with_options, AstChunkOptions}"
)]
pub struct CodeChunkMetadata {
    pub file_path: String,
    pub language: String,
    pub scope_chain: Vec<String>,
    pub entity_type: EntityType,
    pub entity_signature: Option<String>,
    pub start_line: u32,
    pub end_line: u32,
    pub start_byte: usize,
    pub end_byte: usize,
    pub non_whitespace_chars: usize,
    pub imports: Vec<String>,
    pub parent_definition: Option<String>,
}

/// A code chunk with full context for embedding.
#[derive(Debug, Clone)]
#[deprecated(
    since = "0.1.0",
    note = "use context_graph_core::memory::ast::{chunk_path, chunk_with_options, AstChunkOptions}"
)]
pub struct CodeChunk {
    pub code: String,
    pub contextualized_text: String,
    pub description: Option<String>,
    pub metadata: CodeChunkMetadata,
}

/// Deprecated AST chunker facade backed by `memory::ast`.
#[deprecated(
    since = "0.1.0",
    note = "use context_graph_core::memory::ast::{chunk_path, chunk_with_options, AstChunkOptions}"
)]
pub struct AstCodeChunker {
    config: AstChunkConfig,
    fixed_language: Option<Language>,
}

#[allow(deprecated)]
impl AstCodeChunker {
    pub fn new_rust(config: AstChunkConfig) -> Result<Self, AstChunkerError> {
        Ok(Self {
            config,
            fixed_language: Some(Language::Rust),
        })
    }

    pub fn default_rust() -> Result<Self, AstChunkerError> {
        Self::new_rust(AstChunkConfig::default())
    }

    pub fn new_multi_language(config: AstChunkConfig) -> Result<Self, AstChunkerError> {
        Ok(Self {
            config,
            fixed_language: None,
        })
    }

    pub fn default_multi_language() -> Result<Self, AstChunkerError> {
        Self::new_multi_language(AstChunkConfig::default())
    }

    pub fn config(&self) -> &AstChunkConfig {
        &self.config
    }

    pub fn chunk(
        &mut self,
        source: &str,
        file_path: &str,
    ) -> Result<Vec<CodeChunk>, AstChunkerError> {
        if source.trim().is_empty() {
            return Err(AstChunkerError::EmptySource);
        }

        let language = match self.fixed_language {
            Some(language) => language,
            None => Language::from_path(file_path).map_err(map_canonical_error)?,
        };

        let options = AstChunkOptions {
            file_path: file_path.to_string(),
            max_non_ws_chars: self.config.max_size,
        };
        let canonical = ast::chunk_with_options(source.as_bytes(), language, &options)
            .map_err(map_canonical_error)?;
        let imports = if self.config.include_imports {
            canonical
                .iter()
                .filter(|chunk| chunk.entity_type == CanonicalEntityType::Import)
                .map(|chunk| chunk.raw_source.clone())
                .filter(|raw| !raw.trim().is_empty())
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        let chunks = canonical
            .iter()
            .map(|chunk| self.convert_chunk(chunk, &imports, source, file_path))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(chunks)
    }

    pub fn compute_hash(source: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(source.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    fn convert_chunk(
        &self,
        chunk: &CanonicalAstChunk,
        imports: &[String],
        original_source: &str,
        file_path: &str,
    ) -> Result<CodeChunk, AstChunkerError> {
        let code = if chunk.raw_source.is_empty() {
            original_source
                .get(chunk.start_byte..chunk.end_byte)
                .ok_or_else(|| AstChunkerError::ParseFailed {
                    file_path: file_path.to_string(),
                    reason: format!(
                        "canonical chunk byte range {}..{} is outside source length {}",
                        chunk.start_byte,
                        chunk.end_byte,
                        original_source.len()
                    ),
                    diagnostics: Vec::new(),
                })?
                .to_string()
        } else {
            chunk.raw_source.clone()
        };

        let legacy_type = map_entity_type(chunk.entity_type);
        let mut scope_chain = chunk.parent_chain.clone();
        if let Some(symbol) = &chunk.symbol_name {
            if !symbol.is_empty() {
                scope_chain.push(symbol.clone());
            }
        }
        let entity_signature = extract_signature(&code, legacy_type);
        let parent_definition = if self.config.include_parent_context {
            chunk.parent_chain.last().cloned()
        } else {
            None
        };
        let metadata = CodeChunkMetadata {
            file_path: file_path.to_string(),
            language: chunk.language.slug().to_string(),
            scope_chain,
            entity_type: legacy_type,
            entity_signature,
            start_line: chunk.line_start,
            end_line: chunk.line_end,
            start_byte: chunk.start_byte,
            end_byte: chunk.end_byte,
            non_whitespace_chars: code.chars().filter(|c| !c.is_whitespace()).count(),
            imports: imports.to_vec(),
            parent_definition,
        };
        let contextualized_text = contextualize(&metadata, &code);
        Ok(CodeChunk {
            code,
            contextualized_text,
            description: None,
            metadata,
        })
    }
}

#[allow(deprecated)]
fn map_entity_type(entity_type: CanonicalEntityType) -> EntityType {
    match entity_type {
        CanonicalEntityType::Function => EntityType::Function,
        CanonicalEntityType::Method => EntityType::Method,
        CanonicalEntityType::Class => EntityType::Class,
        CanonicalEntityType::Struct => EntityType::Struct,
        CanonicalEntityType::Enum => EntityType::Enum,
        CanonicalEntityType::TraitOrInterface => EntityType::TraitOrInterface,
        CanonicalEntityType::Impl => EntityType::Impl,
        CanonicalEntityType::Module => EntityType::Module,
        CanonicalEntityType::Namespace => EntityType::Namespace,
        CanonicalEntityType::TestFunction => EntityType::TestFunction,
        CanonicalEntityType::Import => EntityType::Import,
        CanonicalEntityType::CommentBlock => EntityType::CommentBlock,
        CanonicalEntityType::Docstring => EntityType::Docstring,
    }
}

#[allow(deprecated)]
fn contextualize(metadata: &CodeChunkMetadata, code: &str) -> String {
    let mut parts = Vec::new();
    let path = Path::new(&metadata.file_path);
    let components = path
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    let short_path = if components.len() > 3 {
        components[components.len() - 3..].join("/")
    } else {
        metadata.file_path.clone()
    };
    parts.push(format!("File: {short_path}"));
    parts.push(format!("Language: {}", metadata.language));
    parts.push(format!("Entity: {:?}", metadata.entity_type));
    if !metadata.scope_chain.is_empty() {
        parts.push(format!("Scope: {}", metadata.scope_chain.join(" > ")));
    }
    if let Some(signature) = &metadata.entity_signature {
        parts.push(format!("Signature: {signature}"));
    }
    if !metadata.imports.is_empty() {
        parts.push(format!("Imports: {}", metadata.imports.join(" | ")));
    }
    parts.push("---".to_string());
    parts.push(code.to_string());
    parts.join("\n")
}

#[allow(deprecated)]
fn extract_signature(code: &str, entity_type: EntityType) -> Option<String> {
    if !matches!(
        entity_type,
        EntityType::Function | EntityType::Method | EntityType::TestFunction
    ) {
        return None;
    }
    let first = code.lines().find(|line| !line.trim().is_empty())?.trim();
    let end = first
        .find('{')
        .or_else(|| first.find(':'))
        .unwrap_or(first.len());
    Some(first[..end].trim().to_string())
}

#[allow(deprecated)]
fn map_canonical_error(err: CanonicalAstError) -> AstChunkerError {
    let code = err.code();
    let mapped = match err {
        CanonicalAstError::EmptySource => AstChunkerError::EmptySource,
        CanonicalAstError::UnsupportedLanguage { requested } => {
            AstChunkerError::UnsupportedExtension {
                extension: requested,
            }
        }
        CanonicalAstError::LanguageSetFailed { language, reason } => {
            AstChunkerError::LanguageSetFailed {
                language: format!("{language}: {reason}"),
            }
        }
        CanonicalAstError::ParseFailed {
            file_path,
            language,
            reason,
            diagnostics,
        } => AstChunkerError::ParseFailed {
            file_path,
            reason: format!("{code} for {language:?}: {reason}"),
            diagnostics: diagnostics.into_iter().map(map_diagnostic).collect(),
        },
        CanonicalAstError::MixedLanguage { primary, embedded } => AstChunkerError::ParseFailed {
            file_path: "<unknown>".to_string(),
            reason: format!(
                "{code}: mixed-language source primary={primary:?} embedded={embedded}"
            ),
            diagnostics: Vec::new(),
        },
        CanonicalAstError::RoutingMissing {
            language,
            entity_type,
            reason,
        } => AstChunkerError::ParseFailed {
            file_path: "<unknown>".to_string(),
            reason: format!("{code}: routing missing for {language:?}/{entity_type:?}: {reason}"),
            diagnostics: Vec::new(),
        },
    };
    error!(error_code = mapped.code(), error = %mapped, "deprecated AstCodeChunker failed closed");
    mapped
}

#[allow(deprecated)]
fn map_diagnostic(diagnostic: CanonicalDiagnostic) -> AstParseDiagnostic {
    AstParseDiagnostic {
        kind: diagnostic.kind,
        is_error: diagnostic.is_error,
        is_missing: diagnostic.is_missing,
        start_byte: diagnostic.start_byte,
        end_byte: diagnostic.end_byte,
        start_line: diagnostic.start_line,
        start_column: diagnostic.start_column,
        end_line: diagnostic.end_line,
        end_column: diagnostic.end_column,
        excerpt: diagnostic.excerpt,
    }
}

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use super::*;

    const SIMPLE_RUST_CODE: &str = r#"
use std::collections::HashMap;

pub struct TestStruct {
    field1: String,
    field2: i32,
}

impl TestStruct {
    pub fn new(field1: String, field2: i32) -> Self {
        Self { field1, field2 }
    }
}

pub fn helper_function(x: i32) -> i32 {
    x * 2
}
"#;

    #[test]
    fn test_chunker_creation() {
        let chunker = AstCodeChunker::default_rust().unwrap();
        assert_eq!(chunker.config().target_size, 500);
        assert_eq!(chunker.config().min_size, 100);
        assert_eq!(chunker.config().max_size, 1000);
    }

    #[test]
    fn test_empty_source_error() {
        let mut chunker = AstCodeChunker::default_rust().unwrap();
        assert!(matches!(
            chunker.chunk("", "test.rs"),
            Err(AstChunkerError::EmptySource)
        ));
        assert!(matches!(
            chunker.chunk("   \n\t  ", "test.rs"),
            Err(AstChunkerError::EmptySource)
        ));
    }

    #[test]
    fn test_invalid_rust_fails_closed_with_diagnostics() {
        let mut chunker = AstCodeChunker::default_rust().unwrap();
        let broken = "pub fn broken() {\n    let value = ;\n}\n";
        let result = chunker.chunk(broken, "broken.rs");
        match result {
            Err(AstChunkerError::ParseFailed {
                file_path,
                reason,
                diagnostics,
            }) => {
                println!("=== INVALID RUST DIAGNOSTICS ===");
                println!("file_path: {file_path}");
                println!("reason: {reason}");
                println!("diagnostics: {diagnostics:?}");
                assert_eq!(file_path, "broken.rs");
                assert!(reason.contains("MEJEPA_CHUNKER_PARSE_FAILED"));
                assert!(diagnostics.iter().any(|d| d.is_error || d.is_missing));
            }
            other => panic!("invalid Rust must fail closed, got {other:?}"),
        }
    }

    #[test]
    fn test_multi_language_python_chunking() {
        let mut chunker = AstCodeChunker::default_multi_language().unwrap();
        let chunks = chunker
            .chunk(
                "import os\n\nclass Solver:\n    def compute(self):\n        return 1\n",
                "solver.py",
            )
            .unwrap();
        assert!(chunks
            .iter()
            .any(|chunk| chunk.metadata.language == "python"));
        assert!(chunks
            .iter()
            .any(|chunk| chunk.metadata.entity_type == EntityType::Class));
        assert!(chunks
            .iter()
            .any(|chunk| chunk.metadata.entity_type == EntityType::Method));
    }

    #[test]
    fn test_legacy_uses_canonical_rust_entity_types() {
        let mut chunker = AstCodeChunker::default_rust().unwrap();
        let chunks = chunker.chunk(SIMPLE_RUST_CODE, "test.rs").unwrap();
        assert!(chunks
            .iter()
            .any(|chunk| chunk.metadata.entity_type == EntityType::Import));
        assert!(chunks
            .iter()
            .any(|chunk| chunk.metadata.entity_type == EntityType::Struct));
        assert!(chunks
            .iter()
            .any(|chunk| chunk.metadata.entity_type == EntityType::Method));
        assert!(chunks
            .iter()
            .any(|chunk| chunk.metadata.entity_type == EntityType::Function));
        assert!(chunks
            .iter()
            .all(|chunk| chunk.contextualized_text.contains("File:")));
    }

    #[test]
    fn test_hash_determinism() {
        let hash1 = AstCodeChunker::compute_hash(SIMPLE_RUST_CODE);
        let hash2 = AstCodeChunker::compute_hash(SIMPLE_RUST_CODE);
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64);
        assert_ne!(hash1, AstCodeChunker::compute_hash("different content"));
    }
}
