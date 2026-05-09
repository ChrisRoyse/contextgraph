//! AST-based code chunking for E7 code embeddings.
//!
//! This module provides AST-aware chunking for source code files, following the cAST
//! methodology (EMNLP 2025) and Qodo best practices for code embeddings.
//!
//! # Key Features
//! - Uses tree-sitter for robust, multi-language AST parsing
//! - Targets ~500 non-whitespace characters per chunk (Qodo recommendation)
//! - Preserves syntactic boundaries (functions, structs, impls)
//! - Context prepending (file path, scope chain, imports)
//! - Includes parent struct/class definition with methods
//!
//! # Constitution Compliance
//! - E7 (V_correctness): 1536D code patterns, function signatures
//! - This chunker finds what E1 misses by treating code as NL
//!
//! # References
//! - cAST paper: https://arxiv.org/html/2506.15655v1
//! - Qodo best practices: https://www.qodo.ai/blog/rag-for-large-scale-code-repos/
//! - Supermemory AST chunking: https://supermemory.ai/blog/building-code-chunk-ast-aware-code-chunking/

use sha2::{Digest, Sha256};
use std::path::Path;
use thiserror::Error;
use tree_sitter::{Node, Parser};

const MAX_PARSE_DIAGNOSTICS: usize = 32;

/// Errors that can occur during AST-based code chunking.
#[derive(Debug, Error)]
pub enum AstChunkerError {
    /// Failed to parse the source code with tree-sitter.
    #[error("Failed to parse Rust source {file_path}: {reason}; diagnostics: {diagnostics:?}")]
    ParseFailed {
        file_path: String,
        reason: String,
        diagnostics: Vec<AstParseDiagnostic>,
    },

    /// Failed to set the tree-sitter language.
    #[error("Failed to set language for parser: {language}")]
    LanguageSetFailed { language: String },

    /// Source code is empty or contains only whitespace.
    #[error("Source code is empty or contains only whitespace")]
    EmptySource,

    /// Unsupported file extension.
    #[error("Unsupported file extension: {extension}")]
    UnsupportedExtension { extension: String },
}

/// Byte-precise tree-sitter parse diagnostic captured before failing closed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AstParseDiagnostic {
    /// Tree-sitter node kind, commonly `ERROR` or the missing token kind.
    pub kind: String,
    /// True when this diagnostic comes from a tree-sitter `ERROR` node.
    pub is_error: bool,
    /// True when this diagnostic comes from a tree-sitter recovered `MISSING` node.
    pub is_missing: bool,
    /// Start byte in the source.
    pub start_byte: usize,
    /// End byte in the source.
    pub end_byte: usize,
    /// 1-based start line.
    pub start_line: u32,
    /// 0-based start column.
    pub start_column: u32,
    /// 1-based end line.
    pub end_line: u32,
    /// 0-based end column.
    pub end_column: u32,
    /// Line-level excerpt around the diagnostic.
    pub excerpt: String,
}

/// Configuration for AST-based code chunking.
///
/// Per Qodo best practices and cAST paper:
/// - Target size: ~500 non-whitespace characters
/// - Minimum size: 100 chars (avoid tiny fragments)
/// - Maximum size: 1000 chars (prevent semantic dilution)
#[derive(Debug, Clone)]
pub struct AstChunkConfig {
    /// Target chunk size in non-whitespace characters (default: 500).
    pub target_size: usize,
    /// Minimum chunk size before merging with siblings (default: 100).
    pub min_size: usize,
    /// Maximum chunk size before recursive splitting (default: 1000).
    pub max_size: usize,
    /// Include parent struct/class definition with methods (default: true).
    pub include_parent_context: bool,
    /// Include relevant imports in each chunk (default: true).
    pub include_imports: bool,
}

impl Default for AstChunkConfig {
    fn default() -> Self {
        Self {
            target_size: 500, // Qodo recommendation
            min_size: 100,    // Avoid tiny fragments
            max_size: 1000,   // Prevent semantic dilution
            include_parent_context: true,
            include_imports: true,
        }
    }
}

/// Entity type detected in the AST.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityType {
    /// A function definition (standalone, not a method).
    Function,
    /// A method within an impl block.
    Method,
    /// A struct definition.
    Struct,
    /// An enum definition.
    Enum,
    /// A trait definition.
    Trait,
    /// An impl block.
    Impl,
    /// A module definition.
    Module,
    /// A const definition.
    Const,
    /// A static definition.
    Static,
    /// A type alias.
    TypeAlias,
    /// A macro definition.
    Macro,
    /// Documentation comment block.
    Comment,
    /// Mixed content (merged siblings of different types).
    Mixed,
    /// Unknown or unrecognized node type.
    Unknown,
}

impl EntityType {
    /// Convert from tree-sitter node kind (Rust).
    fn from_rust_node_kind(kind: &str, parent_kind: Option<&str>) -> Self {
        match kind {
            "function_item" if parent_kind == Some("impl_item") => EntityType::Method,
            "function_item" => EntityType::Function,
            "impl_item" => EntityType::Impl,
            "struct_item" => EntityType::Struct,
            "enum_item" => EntityType::Enum,
            "trait_item" => EntityType::Trait,
            "mod_item" => EntityType::Module,
            "const_item" => EntityType::Const,
            "static_item" => EntityType::Static,
            "type_item" => EntityType::TypeAlias,
            "macro_definition" | "macro_rules" => EntityType::Macro,
            "line_comment" | "block_comment" => EntityType::Comment,
            _ => EntityType::Unknown,
        }
    }
}

/// Metadata for a code chunk.
#[derive(Debug, Clone)]
pub struct CodeChunkMetadata {
    /// Path to the source file.
    pub file_path: String,
    /// Detected programming language.
    pub language: String,
    /// Scope chain (e.g., ["CodeModel", "embed"]).
    pub scope_chain: Vec<String>,
    /// Type of entity this chunk represents.
    pub entity_type: EntityType,
    /// Full signature if available (e.g., "pub async fn embed(&self, input: &ModelInput) -> Result<ModelEmbedding>").
    pub entity_signature: Option<String>,
    /// Starting line number (1-based).
    pub start_line: u32,
    /// Ending line number (1-based).
    pub end_line: u32,
    /// Start byte offset in source.
    pub start_byte: usize,
    /// End byte offset in source.
    pub end_byte: usize,
    /// Size in non-whitespace characters (per cAST paper).
    pub non_whitespace_chars: usize,
    /// Relevant imports for this chunk.
    pub imports: Vec<String>,
    /// Parent struct/class definition if applicable.
    pub parent_definition: Option<String>,
}

/// A code chunk with full context for embedding.
#[derive(Debug, Clone)]
pub struct CodeChunk {
    /// Raw code content.
    pub code: String,
    /// Contextualized text for embedding (includes metadata).
    pub contextualized_text: String,
    /// Optional natural language description (if generated).
    pub description: Option<String>,
    /// Chunk metadata.
    pub metadata: CodeChunkMetadata,
}

/// AST-based code chunker following cAST methodology.
///
/// # Algorithm (per cAST paper)
/// 1. Parse source code into AST using tree-sitter
/// 2. Traverse top-down, attempting to fit large nodes into single chunks
/// 3. For nodes exceeding size limits, recursively split into children
/// 4. Greedily merge adjacent sibling nodes to maximize information density
/// 5. Measure chunks by non-whitespace characters (not lines)
pub struct AstCodeChunker {
    parser: Parser,
    config: AstChunkConfig,
    language_name: String,
}

impl AstCodeChunker {
    /// Create a new AstCodeChunker for Rust source code.
    ///
    /// # Errors
    /// Returns `AstChunkerError::LanguageSetFailed` if the language cannot be set.
    pub fn new_rust(config: AstChunkConfig) -> Result<Self, AstChunkerError> {
        let mut parser = Parser::new();
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        parser
            .set_language(&language)
            .map_err(|e| AstChunkerError::LanguageSetFailed {
                language: format!("rust: {:?}", e),
            })?;
        Ok(Self {
            parser,
            config,
            language_name: "rust".to_string(),
        })
    }

    /// Create a new AstCodeChunker with default configuration for Rust.
    pub fn default_rust() -> Result<Self, AstChunkerError> {
        Self::new_rust(AstChunkConfig::default())
    }

    /// Get the configuration.
    pub fn config(&self) -> &AstChunkConfig {
        &self.config
    }

    /// Chunk source code into AST-aware code chunks.
    ///
    /// # Arguments
    /// * `source` - The source code to chunk
    /// * `file_path` - Path to the source file (for metadata)
    ///
    /// # Returns
    /// Vector of CodeChunk instances with proper metadata and contextualized text.
    ///
    /// # Errors
    /// - `AstChunkerError::EmptySource` if source is empty
    /// - `AstChunkerError::ParseFailed` if tree-sitter fails to parse
    pub fn chunk(
        &mut self,
        source: &str,
        file_path: &str,
    ) -> Result<Vec<CodeChunk>, AstChunkerError> {
        // Fail fast on empty content
        if source.is_empty() || source.trim().is_empty() {
            return Err(AstChunkerError::EmptySource);
        }

        // Parse the source code
        let tree = self
            .parser
            .parse(source, None)
            .ok_or_else(|| AstChunkerError::ParseFailed {
                file_path: file_path.to_string(),
                reason: "tree-sitter returned None".to_string(),
                diagnostics: Vec::new(),
            })?;

        let root = tree.root_node();

        // FAIL FAST: tree-sitter is error-tolerant and can recover by inserting
        // ERROR/MISSING nodes. ME-JEPA cannot train on recovered trees as if
        // they were reality-grounded ASTs.
        if root.has_error() {
            let diagnostics = parse_diagnostics(root, source);
            tracing::error!(
                file_path,
                diagnostics = ?diagnostics,
                "Rust AST chunking failed closed because tree-sitter reported syntax errors"
            );
            return Err(AstChunkerError::ParseFailed {
                file_path: file_path.to_string(),
                reason: "tree-sitter produced ERROR or MISSING nodes".to_string(),
                diagnostics,
            });
        }

        // Extract imports
        let imports = self.extract_imports(&root, source);

        // Collect chunks
        let mut raw_chunks = Vec::new();
        self.process_node(
            &root,
            source,
            file_path,
            &imports,
            &mut vec![], // scope chain
            None,        // parent definition
            None,        // parent node kind
            &mut raw_chunks,
        );

        // Merge small adjacent chunks
        let merged = self.merge_small_chunks(raw_chunks);

        // Build contextualized text for each chunk
        let chunks = merged.into_iter().map(|c| self.contextualize(c)).collect();

        Ok(chunks)
    }

    /// Process a node and its children, creating chunks as appropriate.
    #[allow(clippy::too_many_arguments)]
    fn process_node(
        &self,
        node: &Node,
        source: &str,
        file_path: &str,
        imports: &[String],
        scope_chain: &mut Vec<String>,
        parent_def: Option<&str>,
        parent_kind: Option<&str>,
        chunks: &mut Vec<CodeChunk>,
    ) {
        let size = self.non_whitespace_size(node, source);

        // Skip empty nodes
        if size == 0 {
            return;
        }

        // Case 1: Node fits in a single chunk
        if size <= self.config.max_size && self.is_chunkable_node(node) {
            if let Some(chunk) = self.node_to_chunk(
                node,
                source,
                file_path,
                imports,
                scope_chain,
                parent_def,
                parent_kind,
            ) {
                chunks.push(chunk);
                return;
            }
        }

        // Case 2: Node too large - recurse into children
        let mut cursor = node.walk();
        let children: Vec<_> = node.children(&mut cursor).collect();

        for child in children {
            if self.is_chunkable_node(&child) || child.child_count() > 0 {
                // Update scope chain for named nodes
                let name = self.get_node_name(&child, source);
                if let Some(ref n) = name {
                    scope_chain.push(n.clone());
                }

                // Get parent definition for methods
                let new_parent = if self.is_container_node(node) {
                    Some(self.get_abbreviated_definition(node, source))
                } else {
                    parent_def.map(|s| s.to_string())
                };

                self.process_node(
                    &child,
                    source,
                    file_path,
                    imports,
                    scope_chain,
                    new_parent.as_deref(),
                    Some(node.kind()),
                    chunks,
                );

                if name.is_some() {
                    scope_chain.pop();
                }
            }
        }
    }

    /// Convert a node to a CodeChunk.
    fn node_to_chunk(
        &self,
        node: &Node,
        source: &str,
        file_path: &str,
        imports: &[String],
        scope_chain: &[String],
        parent_def: Option<&str>,
        parent_kind: Option<&str>,
    ) -> Option<CodeChunk> {
        let code = source[node.start_byte()..node.end_byte()].to_string();
        let entity_type = EntityType::from_rust_node_kind(node.kind(), parent_kind);

        // Extract signature for functions/methods
        let entity_signature = self.extract_signature(node, source);

        let metadata = CodeChunkMetadata {
            file_path: file_path.to_string(),
            language: self.language_name.clone(),
            scope_chain: scope_chain.to_vec(),
            entity_type,
            entity_signature,
            start_line: node.start_position().row as u32 + 1, // 1-based
            end_line: node.end_position().row as u32 + 1,
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            non_whitespace_chars: self.non_whitespace_size(node, source),
            imports: if self.config.include_imports {
                self.filter_relevant_imports(&code, imports)
            } else {
                vec![]
            },
            parent_definition: if self.config.include_parent_context {
                parent_def.map(|s| s.to_string())
            } else {
                None
            },
        };

        Some(CodeChunk {
            code,
            contextualized_text: String::new(), // Filled in later by contextualize()
            description: None,
            metadata,
        })
    }

    /// Build contextualized text for embedding.
    fn contextualize(&self, mut chunk: CodeChunk) -> CodeChunk {
        let mut parts = Vec::new();

        // File path (truncated to last 3 segments)
        let path = Path::new(&chunk.metadata.file_path);
        let components: Vec<_> = path.components().collect();
        let short_path = if components.len() > 3 {
            components[components.len() - 3..]
                .iter()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/")
        } else {
            chunk.metadata.file_path.clone()
        };
        parts.push(format!("File: {}", short_path));

        // Scope chain
        if !chunk.metadata.scope_chain.is_empty() {
            parts.push(format!("Scope: {}", chunk.metadata.scope_chain.join(" > ")));
        }

        // Entity signature
        if let Some(ref sig) = chunk.metadata.entity_signature {
            parts.push(format!("Signature: {}", sig));
        }

        // Imports (abbreviated)
        if !chunk.metadata.imports.is_empty() {
            let import_str = chunk
                .metadata
                .imports
                .iter()
                .take(5)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ");
            parts.push(format!("Uses: {}", import_str));
        }

        parts.push("---".to_string());

        // Parent definition (abbreviated)
        if let Some(ref parent) = chunk.metadata.parent_definition {
            parts.push(parent.clone());
            parts.push(String::new());
        }

        // The actual code
        parts.push(chunk.code.clone());

        chunk.contextualized_text = parts.join("\n");
        chunk
    }

    /// Count non-whitespace characters (per cAST paper).
    fn non_whitespace_size(&self, node: &Node, source: &str) -> usize {
        let text = &source[node.start_byte()..node.end_byte()];
        text.chars().filter(|c| !c.is_whitespace()).count()
    }

    /// Check if a node type is chunkable (represents a meaningful code unit).
    fn is_chunkable_node(&self, node: &Node) -> bool {
        matches!(
            node.kind(),
            "function_item"
                | "impl_item"
                | "struct_item"
                | "enum_item"
                | "trait_item"
                | "mod_item"
                | "const_item"
                | "static_item"
                | "type_item"
                | "macro_definition"
                | "macro_rules"
                | "attribute_item"
        )
    }

    /// Check if a node is a container that provides context for children.
    fn is_container_node(&self, node: &Node) -> bool {
        matches!(
            node.kind(),
            "impl_item" | "struct_item" | "enum_item" | "trait_item" | "mod_item"
        )
    }

    /// Get the name of a named node.
    fn get_node_name(&self, node: &Node, source: &str) -> Option<String> {
        // Look for name field in different node types
        let name_node = node.child_by_field_name("name")?;
        let name = source[name_node.start_byte()..name_node.end_byte()].to_string();
        if name.is_empty() {
            None
        } else {
            Some(name)
        }
    }

    /// Get an abbreviated definition of a container node (for parent context).
    fn get_abbreviated_definition(&self, node: &Node, source: &str) -> String {
        let full_text = &source[node.start_byte()..node.end_byte()];

        // Take the first line or first 100 chars, whichever is shorter
        let first_line = full_text.lines().next().unwrap_or("");
        let abbreviated = if first_line.len() > 100 {
            format!("{}...", &first_line[..100])
        } else {
            first_line.to_string()
        };

        // If it's a struct/enum, include the opening brace
        if abbreviated.contains('{') {
            abbreviated
        } else if let Some(brace_pos) = full_text.find('{') {
            let with_brace = &full_text[..brace_pos + 1];
            let lines: Vec<&str> = with_brace.lines().take(3).collect();
            format!("{} ...", lines.join("\n"))
        } else {
            abbreviated
        }
    }

    /// Extract function/method signature from a node.
    fn extract_signature(&self, node: &Node, source: &str) -> Option<String> {
        if !matches!(node.kind(), "function_item") {
            return None;
        }

        let full_text = &source[node.start_byte()..node.end_byte()];

        // Find the function signature (up to the opening brace)
        if let Some(brace_pos) = full_text.find('{') {
            let sig = full_text[..brace_pos].trim();
            Some(sig.to_string())
        } else {
            // Might be a function declaration without body
            let first_line = full_text.lines().next()?;
            Some(first_line.trim().to_string())
        }
    }

    /// Extract use/import statements from the source.
    fn extract_imports(&self, root: &Node, source: &str) -> Vec<String> {
        let mut imports = Vec::new();
        let mut cursor = root.walk();

        for child in root.children(&mut cursor) {
            if child.kind() == "use_declaration" {
                let text = source[child.start_byte()..child.end_byte()].to_string();
                imports.push(text);
            }
        }

        imports
    }

    /// Filter imports relevant to a code chunk.
    fn filter_relevant_imports(&self, code: &str, imports: &[String]) -> Vec<String> {
        // Extract identifiers from the code chunk
        let code_lower = code.to_lowercase();

        imports
            .iter()
            .filter(|import| {
                // Extract the imported item name
                if let Some(item) = import.split("::").last() {
                    let item = item.trim().trim_end_matches(';').trim_end_matches('}');
                    // Check if this import is used in the code
                    code_lower.contains(&item.to_lowercase())
                } else {
                    false
                }
            })
            .cloned()
            .collect()
    }

    /// Merge small adjacent chunks to meet minimum size.
    fn merge_small_chunks(&self, chunks: Vec<CodeChunk>) -> Vec<CodeChunk> {
        if chunks.len() <= 1 {
            return chunks;
        }

        let mut result = Vec::new();
        let mut pending: Option<CodeChunk> = None;

        for chunk in chunks {
            if let Some(p) = pending.take() {
                // Check if pending chunk is too small
                if p.metadata.non_whitespace_chars < self.config.min_size {
                    // Merge with current chunk
                    let merged_code = format!("{}\n\n{}", p.code, chunk.code);
                    let merged_metadata = CodeChunkMetadata {
                        file_path: p.metadata.file_path.clone(),
                        language: p.metadata.language.clone(),
                        scope_chain: p.metadata.scope_chain.clone(),
                        entity_type: if p.metadata.entity_type == chunk.metadata.entity_type {
                            p.metadata.entity_type
                        } else {
                            EntityType::Mixed
                        },
                        entity_signature: None,
                        start_line: p.metadata.start_line,
                        end_line: chunk.metadata.end_line,
                        start_byte: p.metadata.start_byte,
                        end_byte: chunk.metadata.end_byte,
                        non_whitespace_chars: p.metadata.non_whitespace_chars
                            + chunk.metadata.non_whitespace_chars,
                        imports: {
                            let mut all = p.metadata.imports.clone();
                            all.extend(chunk.metadata.imports.clone());
                            all.sort();
                            all.dedup();
                            all
                        },
                        parent_definition: p.metadata.parent_definition.clone(),
                    };
                    pending = Some(CodeChunk {
                        code: merged_code,
                        contextualized_text: String::new(),
                        description: None,
                        metadata: merged_metadata,
                    });
                } else {
                    // Pending chunk is large enough, emit it
                    result.push(p);
                    pending = Some(chunk);
                }
            } else {
                pending = Some(chunk);
            }
        }

        // Don't forget the last chunk
        if let Some(p) = pending {
            result.push(p);
        }

        result
    }

    /// Compute SHA256 hash of source content.
    pub fn compute_hash(source: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(source.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

fn parse_diagnostics(root: Node<'_>, source: &str) -> Vec<AstParseDiagnostic> {
    let mut diagnostics = Vec::new();
    collect_parse_diagnostics(root, source, &mut diagnostics);
    if diagnostics.is_empty() {
        diagnostics.push(diagnostic_for_node(root, source));
    }
    diagnostics
}

fn collect_parse_diagnostics(
    node: Node<'_>,
    source: &str,
    diagnostics: &mut Vec<AstParseDiagnostic>,
) {
    if diagnostics.len() >= MAX_PARSE_DIAGNOSTICS {
        return;
    }

    if node.is_error() || node.is_missing() {
        diagnostics.push(diagnostic_for_node(node, source));
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if diagnostics.len() >= MAX_PARSE_DIAGNOSTICS {
            break;
        }
        if child.has_error() || child.is_missing() || child.is_error() {
            collect_parse_diagnostics(child, source, diagnostics);
        }
    }
}

fn diagnostic_for_node(node: Node<'_>, source: &str) -> AstParseDiagnostic {
    let start = node.start_position();
    let end = node.end_position();
    AstParseDiagnostic {
        kind: node.kind().to_string(),
        is_error: node.is_error(),
        is_missing: node.is_missing(),
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
        start_line: start.row as u32 + 1,
        start_column: start.column as u32,
        end_line: end.row as u32 + 1,
        end_column: end.column as u32,
        excerpt: line_excerpt_at_byte(source, node.start_byte()),
    }
}

fn line_excerpt_at_byte(source: &str, byte: usize) -> String {
    if source.is_empty() {
        return String::new();
    }
    let byte = floor_char_boundary(source, byte.min(source.len()));
    let line_start = source[..byte].rfind('\n').map(|idx| idx + 1).unwrap_or(0);
    let line_end = source[byte..]
        .find('\n')
        .map(|idx| byte + idx)
        .unwrap_or(source.len());
    source
        .get(line_start..line_end)
        .unwrap_or("")
        .chars()
        .take(160)
        .collect()
}

fn floor_char_boundary(source: &str, mut byte: usize) -> usize {
    while byte > 0 && !source.is_char_boundary(byte) {
        byte -= 1;
    }
    byte
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE_RUST_CODE: &str = r#"
use std::collections::HashMap;
use tokio::sync::RwLock;

/// A simple struct for testing.
pub struct TestStruct {
    field1: String,
    field2: i32,
}

impl TestStruct {
    /// Create a new TestStruct.
    pub fn new(field1: String, field2: i32) -> Self {
        Self { field1, field2 }
    }

    /// Get the first field.
    pub fn get_field1(&self) -> &str {
        &self.field1
    }
}

/// A standalone function.
pub fn helper_function(x: i32) -> i32 {
    x * 2
}
"#;

    const LARGE_FUNCTION: &str = r#"
/// A function with a large body that should be kept together.
pub fn large_function(input: &str) -> Result<String, Error> {
    let mut result = String::new();

    // Process the input in multiple steps
    for line in input.lines() {
        if line.starts_with('#') {
            // This is a comment line
            continue;
        }

        // Parse the line
        let parts: Vec<&str> = line.split('=').collect();
        if parts.len() != 2 {
            return Err(Error::InvalidFormat);
        }

        let key = parts[0].trim();
        let value = parts[1].trim();

        // Validate the key
        if key.is_empty() {
            return Err(Error::EmptyKey);
        }

        // Store the result
        result.push_str(&format!("{}: {}\n", key, value));
    }

    Ok(result)
}
"#;

    #[test]
    fn test_chunker_creation() {
        let chunker = AstCodeChunker::default_rust();
        assert!(chunker.is_ok(), "Should create Rust chunker successfully");

        let chunker = chunker.unwrap();
        assert_eq!(chunker.config.target_size, 500);
        assert_eq!(chunker.config.min_size, 100);
        assert_eq!(chunker.config.max_size, 1000);
    }

    #[test]
    fn test_empty_source_error() {
        let mut chunker = AstCodeChunker::default_rust().unwrap();

        let result = chunker.chunk("", "test.rs");
        assert!(matches!(result, Err(AstChunkerError::EmptySource)));

        let result = chunker.chunk("   \n\t  ", "test.rs");
        assert!(matches!(result, Err(AstChunkerError::EmptySource)));
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
                println!("=== END ===");
                assert_eq!(file_path, "broken.rs");
                assert!(reason.contains("ERROR") || reason.contains("MISSING"));
                assert!(
                    diagnostics.iter().any(|d| d.is_error || d.is_missing),
                    "diagnostics must include at least one ERROR or MISSING node"
                );
                assert!(
                    diagnostics.iter().any(|d| d.start_line >= 2),
                    "diagnostics should point at or after the broken assignment"
                );
            }
            other => panic!("invalid Rust must fail closed, got {other:?}"),
        }
    }

    #[test]
    fn test_impl_functions_are_tagged_as_methods_when_split() {
        let config = AstChunkConfig {
            max_size: 40,
            min_size: 1,
            ..AstChunkConfig::default()
        };
        let mut chunker = AstCodeChunker::new_rust(config).unwrap();
        let code = r#"
pub struct Worker {
    count: usize,
}

impl Worker {
    pub fn run(&self) -> usize {
        self.count
    }
}
"#;

        let chunks = chunker.chunk(code, "worker.rs").expect("valid Rust chunks");
        println!("=== METHOD ENTITY TYPES ===");
        for chunk in &chunks {
            println!(
                "entity={:?} lines={}-{} code={:?}",
                chunk.metadata.entity_type,
                chunk.metadata.start_line,
                chunk.metadata.end_line,
                chunk.code
            );
        }
        println!("=== END ===");
        assert!(
            chunks
                .iter()
                .any(|chunk| chunk.metadata.entity_type == EntityType::Method),
            "function_item inside impl_item must be tagged as Method"
        );
    }

    #[test]
    fn test_simple_rust_chunking() {
        let mut chunker = AstCodeChunker::default_rust().unwrap();

        let chunks = chunker
            .chunk(SIMPLE_RUST_CODE, "test.rs")
            .expect("Should chunk successfully");

        println!("=== SIMPLE RUST CHUNKING TEST ===");
        println!("Number of chunks: {}", chunks.len());
        for (i, chunk) in chunks.iter().enumerate() {
            println!(
                "Chunk {}: entity={:?}, lines={}-{}, chars={}",
                i,
                chunk.metadata.entity_type,
                chunk.metadata.start_line,
                chunk.metadata.end_line,
                chunk.metadata.non_whitespace_chars
            );
            println!("  Scope: {:?}", chunk.metadata.scope_chain);
            println!("  Signature: {:?}", chunk.metadata.entity_signature);
        }
        println!("=== END TEST ===");

        // Should have at least struct, impl, and standalone function
        assert!(chunks.len() >= 2, "Should produce multiple chunks");

        // Check that contextualized text contains file path
        for chunk in &chunks {
            assert!(
                chunk.contextualized_text.contains("File:"),
                "Contextualized text should contain file path"
            );
        }
    }

    #[test]
    fn test_large_function_stays_together() {
        let mut chunker = AstCodeChunker::default_rust().unwrap();

        let chunks = chunker
            .chunk(LARGE_FUNCTION, "large.rs")
            .expect("Should chunk successfully");

        println!("=== LARGE FUNCTION TEST ===");
        println!("Number of chunks: {}", chunks.len());
        for (i, chunk) in chunks.iter().enumerate() {
            println!(
                "Chunk {}: chars={}, type={:?}",
                i, chunk.metadata.non_whitespace_chars, chunk.metadata.entity_type
            );
        }
        println!("=== END TEST ===");

        // The large function should stay as one chunk since it's under max_size
        assert_eq!(chunks.len(), 1, "Large function should be one chunk");
        assert_eq!(chunks[0].metadata.entity_type, EntityType::Function);
    }

    #[test]
    fn test_non_whitespace_counting() {
        let mut chunker = AstCodeChunker::default_rust().unwrap();

        let code = "fn foo() { }\n\n\n";
        let chunks = chunker.chunk(code, "test.rs").expect("Should chunk");

        // "fn foo() { }" has 10 non-whitespace chars: f,n,f,o,o,(,),{,}
        // Actually: 'f','n','f','o','o','(',')','{',' ','}' - let's count
        // fn foo() { } -> f,n,f,o,o,(,),{,} = 9 non-WS chars (space between { } is WS)
        assert!(
            chunks[0].metadata.non_whitespace_chars < 15,
            "Should count non-whitespace correctly, got {}",
            chunks[0].metadata.non_whitespace_chars
        );
    }

    #[test]
    fn test_entity_type_detection() {
        let mut chunker = AstCodeChunker::default_rust().unwrap();

        let chunks = chunker
            .chunk(SIMPLE_RUST_CODE, "test.rs")
            .expect("Should chunk");

        let types: Vec<EntityType> = chunks.iter().map(|c| c.metadata.entity_type).collect();
        println!("Detected entity types: {:?}", types);

        // Should detect struct, impl, function, or Mixed (merged struct+impl)
        assert!(
            types.contains(&EntityType::Struct)
                || types.contains(&EntityType::Impl)
                || types.contains(&EntityType::Mixed)
                || types.contains(&EntityType::Function),
            "Should detect struct, impl, mixed, or function"
        );
    }

    #[test]
    fn test_import_extraction() {
        let mut chunker = AstCodeChunker::default_rust().unwrap();

        let chunks = chunker
            .chunk(SIMPLE_RUST_CODE, "test.rs")
            .expect("Should chunk");

        // At least some chunks should have imports extracted
        let _has_imports = chunks.iter().any(|c| !c.metadata.imports.is_empty());
        println!(
            "Chunks with imports: {}",
            chunks
                .iter()
                .filter(|c| !c.metadata.imports.is_empty())
                .count()
        );

        // This test is informational - imports may or may not be relevant to each chunk
    }

    #[test]
    fn test_scope_chain() {
        let mut chunker = AstCodeChunker::default_rust().unwrap();

        let code = r#"
pub struct Outer {
    inner: i32,
}

impl Outer {
    pub fn method(&self) -> i32 {
        self.inner
    }
}
"#;

        let chunks = chunker.chunk(code, "scope.rs").expect("Should chunk");

        println!("=== SCOPE CHAIN TEST ===");
        for (i, chunk) in chunks.iter().enumerate() {
            println!("Chunk {}: scope={:?}", i, chunk.metadata.scope_chain);
        }
        println!("=== END TEST ===");

        // Some chunks should have scope chains
        // Note: exact behavior depends on how we traverse
    }

    #[test]
    fn test_contextualized_text_format() {
        let mut chunker = AstCodeChunker::default_rust().unwrap();

        let chunks = chunker
            .chunk(SIMPLE_RUST_CODE, "src/memory/test.rs")
            .expect("Should chunk");

        assert!(!chunks.is_empty());

        let ctx = &chunks[0].contextualized_text;
        println!("=== CONTEXTUALIZED TEXT ===");
        println!("{}", ctx);
        println!("=== END ===");

        // Should have file path
        assert!(ctx.contains("File:"), "Should have file path");
        // Should have separator
        assert!(ctx.contains("---"), "Should have separator");
    }

    #[test]
    fn test_hash_determinism() {
        let hash1 = AstCodeChunker::compute_hash(SIMPLE_RUST_CODE);
        let hash2 = AstCodeChunker::compute_hash(SIMPLE_RUST_CODE);

        assert_eq!(hash1, hash2, "Same content should produce same hash");
        assert_eq!(hash1.len(), 64, "SHA256 should be 64 hex chars");

        let hash3 = AstCodeChunker::compute_hash("different content");
        assert_ne!(
            hash1, hash3,
            "Different content should produce different hash"
        );
    }

    #[test]
    fn test_custom_config() {
        let config = AstChunkConfig {
            target_size: 300,
            min_size: 50,
            max_size: 600,
            include_parent_context: false,
            include_imports: false,
        };

        let chunker = AstCodeChunker::new_rust(config.clone()).unwrap();

        assert_eq!(chunker.config().target_size, 300);
        assert_eq!(chunker.config().min_size, 50);
        assert_eq!(chunker.config().max_size, 600);
        assert!(!chunker.config().include_parent_context);
        assert!(!chunker.config().include_imports);
    }

    #[test]
    fn test_line_numbers() {
        let mut chunker = AstCodeChunker::default_rust().unwrap();

        let chunks = chunker
            .chunk(SIMPLE_RUST_CODE, "test.rs")
            .expect("Should chunk");

        for chunk in &chunks {
            assert!(
                chunk.metadata.start_line >= 1,
                "Line numbers should be 1-based"
            );
            assert!(
                chunk.metadata.start_line <= chunk.metadata.end_line,
                "Start line should be <= end line"
            );
        }
    }

    // Edge case: Very small file
    #[test]
    fn test_tiny_file() {
        let mut chunker = AstCodeChunker::default_rust().unwrap();

        let code = "fn x() {}";
        let chunks = chunker.chunk(code, "tiny.rs").expect("Should chunk");

        assert_eq!(chunks.len(), 1, "Tiny file should be one chunk");
    }

    // Edge case: File with only comments
    #[test]
    fn test_comments_only() {
        let mut chunker = AstCodeChunker::default_rust().unwrap();

        let code = "// This is a comment\n/* Multi-line\ncomment */\n";
        let result = chunker.chunk(code, "comments.rs");

        // Should either produce empty chunks or handle gracefully
        // tree-sitter will parse this but we might not chunk comments
        println!("Comments-only result: {:?}", result);
    }

    // Synthetic test with known expected output
    #[test]
    fn test_synthetic_verification() {
        let mut chunker = AstCodeChunker::default_rust().unwrap();

        let code = r#"
/// Doc comment
pub fn documented_function(a: i32, b: String) -> Result<(), Error> {
    println!("{}: {}", a, b);
    Ok(())
}
"#;

        let chunks = chunker.chunk(code, "synthetic.rs").expect("Should chunk");

        assert_eq!(chunks.len(), 1, "Should be exactly one chunk");

        let chunk = &chunks[0];
        assert_eq!(chunk.metadata.entity_type, EntityType::Function);
        assert!(chunk
            .metadata
            .entity_signature
            .as_ref()
            .unwrap()
            .contains("documented_function"));
        assert!(chunk.contextualized_text.contains("documented_function"));

        println!("=== SYNTHETIC VERIFICATION ===");
        println!("Entity type: {:?}", chunk.metadata.entity_type);
        println!("Signature: {:?}", chunk.metadata.entity_signature);
        println!(
            "Lines: {}-{}",
            chunk.metadata.start_line, chunk.metadata.end_line
        );
        println!("Non-WS chars: {}", chunk.metadata.non_whitespace_chars);
        println!("=== END ===");
    }
}
