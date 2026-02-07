//! Code entity types for code embedding system.
//!
//! These types represent code entities extracted from source files via AST parsing.
//! Code entities are stored separately from regular text content and use E7
//! (Qodo-Embed-1-1.5B) as the primary embedder.
//!
//! # Architecture
//! - Code is chunked via AST (functions, structs, traits as atomic units)
//! - Each entity has rich metadata (language, signature, parent type, line numbers)
//! - E7 embeddings are stored separately from the 13-embedder teleological system
//! - Enables code-specific retrieval patterns (signature search, pattern matching)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use uuid::Uuid;

/// A code entity extracted from source files via AST parsing.
///
/// Represents a logical unit of code (function, struct, trait, etc.) that
/// can be independently embedded and retrieved.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeEntity {
    /// Unique identifier for this entity.
    pub id: Uuid,

    /// Type of code entity.
    pub entity_type: CodeEntityType,

    /// Entity name (function name, struct name, etc.).
    pub name: String,

    /// Full code content of this entity.
    pub code: String,

    /// Programming language.
    pub language: CodeLanguage,

    /// Absolute file path where this entity is defined.
    pub file_path: String,

    /// Starting line number (1-indexed).
    pub line_start: usize,

    /// Ending line number (1-indexed, inclusive).
    pub line_end: usize,

    /// Module path (e.g., "context_graph_core::memory::watcher").
    pub module_path: Option<String>,

    /// Function/method signature (for callable entities).
    pub signature: Option<String>,

    /// Parent type name (for methods inside impl blocks).
    pub parent_type: Option<String>,

    /// Visibility modifier.
    pub visibility: Visibility,

    /// When this entity was last updated.
    pub last_updated: DateTime<Utc>,

    /// SHA256 hash of the code content for change detection.
    pub content_hash: String,

    /// Documentation comment if present.
    pub doc_comment: Option<String>,

    /// List of attributes/decorators (e.g., #[test], #[derive]).
    pub attributes: Vec<String>,

    /// Git metadata for this entity (Phase 3b code git provenance).
    #[serde(default)]
    pub git_metadata: Option<CodeGitMetadata>,
}

/// Git metadata for a code entity.
///
/// Phase 3b: Captures git provenance for code entities, enabling
/// "who wrote this" and "when was this last changed" queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeGitMetadata {
    /// Git commit hash of the last change to this entity's file.
    pub commit_hash: Option<String>,
    /// Git author name of the last commit.
    pub author: Option<String>,
    /// Git branch name at time of scan.
    pub branch: Option<String>,
    /// Timestamp of the last commit.
    pub commit_timestamp: Option<DateTime<Utc>>,
}

impl CodeEntity {
    /// Create a new code entity with auto-generated ID and content hash.
    pub fn new(
        entity_type: CodeEntityType,
        name: String,
        code: String,
        language: CodeLanguage,
        file_path: String,
        line_start: usize,
        line_end: usize,
    ) -> Self {
        let content_hash = Self::compute_hash(&code);

        Self {
            id: Uuid::new_v4(),
            entity_type,
            name,
            code,
            language,
            file_path,
            line_start,
            line_end,
            module_path: None,
            signature: None,
            parent_type: None,
            visibility: Visibility::Private,
            last_updated: Utc::now(),
            content_hash,
            doc_comment: None,
            attributes: Vec::new(),
            git_metadata: None,
        }
    }

    /// Compute SHA256 hash of content.
    pub fn compute_hash(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Check if the entity content has changed.
    pub fn content_changed(&self, new_code: &str) -> bool {
        let new_hash = Self::compute_hash(new_code);
        self.content_hash != new_hash
    }

    /// Get the number of lines in this entity.
    pub fn line_count(&self) -> usize {
        if self.line_end >= self.line_start {
            self.line_end - self.line_start + 1
        } else {
            0
        }
    }

    /// Check if this is a test entity (has #[test] or similar).
    pub fn is_test(&self) -> bool {
        self.attributes.iter().any(|a| a.contains("test"))
    }

    /// Check if this is a public entity.
    pub fn is_public(&self) -> bool {
        matches!(self.visibility, Visibility::Public | Visibility::PublicCrate)
    }

    /// Get a display-friendly location string.
    pub fn location(&self) -> String {
        format!("{}:{}-{}", self.file_path, self.line_start, self.line_end)
    }

    /// Get a short identifier for display.
    pub fn short_id(&self) -> String {
        if let Some(ref parent) = self.parent_type {
            format!("{}::{}", parent, self.name)
        } else {
            self.name.clone()
        }
    }

    /// Builder method to set module path.
    pub fn with_module_path(mut self, module_path: impl Into<String>) -> Self {
        self.module_path = Some(module_path.into());
        self
    }

    /// Builder method to set signature.
    pub fn with_signature(mut self, signature: impl Into<String>) -> Self {
        self.signature = Some(signature.into());
        self
    }

    /// Builder method to set parent type.
    pub fn with_parent_type(mut self, parent_type: impl Into<String>) -> Self {
        self.parent_type = Some(parent_type.into());
        self
    }

    /// Builder method to set visibility.
    pub fn with_visibility(mut self, visibility: Visibility) -> Self {
        self.visibility = visibility;
        self
    }

    /// Builder method to set doc comment.
    pub fn with_doc_comment(mut self, doc: impl Into<String>) -> Self {
        self.doc_comment = Some(doc.into());
        self
    }

    /// Builder method to add attribute.
    pub fn with_attribute(mut self, attr: impl Into<String>) -> Self {
        self.attributes.push(attr.into());
        self
    }

    /// Builder method to add multiple attributes.
    pub fn with_attributes(mut self, attrs: Vec<String>) -> Self {
        self.attributes.extend(attrs);
        self
    }

    /// Builder method to set git metadata.
    pub fn with_git_metadata(mut self, metadata: CodeGitMetadata) -> Self {
        self.git_metadata = Some(metadata);
        self
    }
}

/// Type of code entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CodeEntityType {
    /// Standalone function (not inside impl block).
    Function,
    /// Method inside an impl block.
    Method,
    /// Struct definition.
    Struct,
    /// Enum definition.
    Enum,
    /// Trait definition.
    Trait,
    /// Impl block (entire block, not individual methods).
    Impl,
    /// Constant definition.
    Const,
    /// Static variable.
    Static,
    /// Type alias.
    TypeAlias,
    /// Macro definition (macro_rules! or proc macro).
    Macro,
    /// Module definition.
    Module,
    /// Import/use statement.
    Import,
    /// Test function.
    Test,
    /// Associated type in trait.
    AssociatedType,
    /// Variant of an enum.
    EnumVariant,
    /// Field of a struct.
    StructField,
}

impl CodeEntityType {
    /// Get a human-readable name for this entity type.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Method => "method",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Trait => "trait",
            Self::Impl => "impl",
            Self::Const => "const",
            Self::Static => "static",
            Self::TypeAlias => "type",
            Self::Macro => "macro",
            Self::Module => "module",
            Self::Import => "import",
            Self::Test => "test",
            Self::AssociatedType => "associated_type",
            Self::EnumVariant => "variant",
            Self::StructField => "field",
        }
    }

    /// Check if this type represents a callable entity.
    pub fn is_callable(&self) -> bool {
        matches!(self, Self::Function | Self::Method | Self::Macro)
    }

    /// Check if this type represents a type definition.
    pub fn is_type_definition(&self) -> bool {
        matches!(
            self,
            Self::Struct | Self::Enum | Self::Trait | Self::TypeAlias
        )
    }
}

impl fmt::Display for CodeEntityType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Programming language of the code entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum CodeLanguage {
    /// Rust
    Rust,
    /// Python
    Python,
    /// TypeScript
    TypeScript,
    /// JavaScript
    JavaScript,
    /// Go
    Go,
    /// Java
    Java,
    /// C++
    Cpp,
    /// C
    C,
    /// SQL
    Sql,
    /// TOML configuration
    Toml,
    /// YAML configuration
    Yaml,
    /// JSON
    Json,
    /// Markdown (for embedded code)
    Markdown,
    /// Shell/Bash
    Shell,
    /// Unknown language
    #[default]
    Unknown,
}

impl CodeLanguage {
    /// Detect language from file extension.
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "rs" => Self::Rust,
            "py" | "pyi" => Self::Python,
            "ts" | "tsx" => Self::TypeScript,
            "js" | "jsx" | "mjs" | "cjs" => Self::JavaScript,
            "go" => Self::Go,
            "java" => Self::Java,
            "cpp" | "cc" | "cxx" | "hpp" | "hxx" => Self::Cpp,
            "c" | "h" => Self::C,
            "sql" => Self::Sql,
            "toml" => Self::Toml,
            "yaml" | "yml" => Self::Yaml,
            "json" => Self::Json,
            "md" | "markdown" => Self::Markdown,
            "sh" | "bash" | "zsh" => Self::Shell,
            _ => Self::Unknown,
        }
    }

    /// Detect language from file path.
    pub fn from_path(path: &str) -> Self {
        if let Some(ext) = std::path::Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
        {
            Self::from_extension(ext)
        } else {
            Self::Unknown
        }
    }

    /// Get the file extensions for this language.
    pub fn extensions(&self) -> &'static [&'static str] {
        match self {
            Self::Rust => &["rs"],
            Self::Python => &["py", "pyi"],
            Self::TypeScript => &["ts", "tsx"],
            Self::JavaScript => &["js", "jsx", "mjs", "cjs"],
            Self::Go => &["go"],
            Self::Java => &["java"],
            Self::Cpp => &["cpp", "cc", "cxx", "hpp", "hxx"],
            Self::C => &["c", "h"],
            Self::Sql => &["sql"],
            Self::Toml => &["toml"],
            Self::Yaml => &["yaml", "yml"],
            Self::Json => &["json"],
            Self::Markdown => &["md", "markdown"],
            Self::Shell => &["sh", "bash", "zsh"],
            Self::Unknown => &[],
        }
    }

    /// Get a human-readable name for this language.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Rust => "Rust",
            Self::Python => "Python",
            Self::TypeScript => "TypeScript",
            Self::JavaScript => "JavaScript",
            Self::Go => "Go",
            Self::Java => "Java",
            Self::Cpp => "C++",
            Self::C => "C",
            Self::Sql => "SQL",
            Self::Toml => "TOML",
            Self::Yaml => "YAML",
            Self::Json => "JSON",
            Self::Markdown => "Markdown",
            Self::Shell => "Shell",
            Self::Unknown => "Unknown",
        }
    }

    /// Check if this language should be parsed for code entities.
    pub fn is_parseable(&self) -> bool {
        matches!(
            self,
            Self::Rust
                | Self::Python
                | Self::TypeScript
                | Self::JavaScript
                | Self::Go
                | Self::Java
                | Self::Cpp
                | Self::C
        )
    }
}

impl fmt::Display for CodeLanguage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Visibility modifier for code entities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Visibility {
    /// Public (pub)
    Public,
    /// Crate-public (pub(crate))
    PublicCrate,
    /// Super-public (pub(super))
    PublicSuper,
    /// Module-public (pub(in path))
    PublicIn,
    /// Private (no modifier)
    #[default]
    Private,
}

impl Visibility {
    /// Parse visibility from Rust syntax.
    pub fn from_rust_str(s: &str) -> Self {
        match s.trim() {
            "pub" => Self::Public,
            "pub(crate)" => Self::PublicCrate,
            "pub(super)" => Self::PublicSuper,
            s if s.starts_with("pub(in ") => Self::PublicIn,
            _ => Self::Private,
        }
    }

    /// Get the Rust syntax for this visibility.
    pub fn to_rust_str(&self) -> &'static str {
        match self {
            Self::Public => "pub",
            Self::PublicCrate => "pub(crate)",
            Self::PublicSuper => "pub(super)",
            Self::PublicIn => "pub(in ...)",
            Self::Private => "",
        }
    }
}

impl fmt::Display for Visibility {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_rust_str())
    }
}

/// Entry in the code file index mapping file paths to entity IDs.
///
/// Similar to FileIndexEntry but with code-specific metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeFileIndexEntry {
    /// The file path this entry represents.
    pub file_path: String,

    /// Programming language of the file.
    pub language: CodeLanguage,

    /// UUIDs of code entities in this file.
    pub entity_ids: Vec<Uuid>,

    /// When this file was last scanned.
    pub last_scanned: DateTime<Utc>,

    /// SHA256 hash of the entire file content.
    pub file_hash: String,

    /// Total lines of code in the file.
    pub total_lines: usize,

    /// Number of functions/methods in the file.
    pub function_count: usize,

    /// Number of type definitions (struct/enum/trait) in the file.
    pub type_count: usize,
}

impl CodeFileIndexEntry {
    /// Create a new code file index entry.
    pub fn new(file_path: String, language: CodeLanguage, file_hash: String) -> Self {
        Self {
            file_path,
            language,
            entity_ids: Vec::new(),
            last_scanned: Utc::now(),
            file_hash,
            total_lines: 0,
            function_count: 0,
            type_count: 0,
        }
    }

    /// Add an entity ID to this entry.
    pub fn add_entity(&mut self, id: Uuid) {
        if !self.entity_ids.contains(&id) {
            self.entity_ids.push(id);
        }
    }

    /// Remove an entity ID from this entry.
    pub fn remove_entity(&mut self, id: Uuid) -> bool {
        if let Some(pos) = self.entity_ids.iter().position(|&x| x == id) {
            self.entity_ids.remove(pos);
            true
        } else {
            false
        }
    }

    /// Get the number of entities in this file.
    pub fn entity_count(&self) -> usize {
        self.entity_ids.len()
    }

    /// Check if this entry is empty (no entities).
    pub fn is_empty(&self) -> bool {
        self.entity_ids.is_empty()
    }

    /// Check if the file content has changed.
    pub fn file_changed(&self, new_hash: &str) -> bool {
        self.file_hash != new_hash
    }

    /// Update the file hash and scan time.
    pub fn update_hash(&mut self, new_hash: String) {
        self.file_hash = new_hash;
        self.last_scanned = Utc::now();
    }
}

/// Statistics about code entities in the knowledge graph.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CodeStats {
    /// Total number of code files.
    pub total_files: usize,
    /// Total number of code entities.
    pub total_entities: usize,
    /// Entities by type.
    pub entities_by_type: std::collections::HashMap<CodeEntityType, usize>,
    /// Entities by language.
    pub entities_by_language: std::collections::HashMap<CodeLanguage, usize>,
    /// Average entities per file.
    pub avg_entities_per_file: f64,
    /// Total lines of code.
    pub total_lines: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_entity_new() {
        let entity = CodeEntity::new(
            CodeEntityType::Function,
            "my_function".to_string(),
            "fn my_function() {}".to_string(),
            CodeLanguage::Rust,
            "/path/to/file.rs".to_string(),
            10,
            15,
        );

        assert_eq!(entity.name, "my_function");
        assert_eq!(entity.entity_type, CodeEntityType::Function);
        assert_eq!(entity.language, CodeLanguage::Rust);
        assert_eq!(entity.line_count(), 6);
        assert!(!entity.content_hash.is_empty());
    }

    #[test]
    fn test_code_entity_builder() {
        let entity = CodeEntity::new(
            CodeEntityType::Method,
            "process".to_string(),
            "pub fn process(&self) -> Result<()> {}".to_string(),
            CodeLanguage::Rust,
            "/path/to/file.rs".to_string(),
            20,
            30,
        )
        .with_parent_type("MyStruct")
        .with_visibility(Visibility::Public)
        .with_signature("pub fn process(&self) -> Result<()>")
        .with_doc_comment("Process the data")
        .with_attribute("#[inline]");

        assert_eq!(entity.parent_type, Some("MyStruct".to_string()));
        assert_eq!(entity.visibility, Visibility::Public);
        assert!(entity.signature.is_some());
        assert!(entity.doc_comment.is_some());
        assert_eq!(entity.attributes.len(), 1);
    }

    #[test]
    fn test_code_language_from_extension() {
        assert_eq!(CodeLanguage::from_extension("rs"), CodeLanguage::Rust);
        assert_eq!(CodeLanguage::from_extension("py"), CodeLanguage::Python);
        assert_eq!(CodeLanguage::from_extension("ts"), CodeLanguage::TypeScript);
        assert_eq!(CodeLanguage::from_extension("go"), CodeLanguage::Go);
        assert_eq!(CodeLanguage::from_extension("xyz"), CodeLanguage::Unknown);
    }

    #[test]
    fn test_code_language_from_path() {
        assert_eq!(
            CodeLanguage::from_path("/home/user/project/main.rs"),
            CodeLanguage::Rust
        );
        assert_eq!(
            CodeLanguage::from_path("src/components/Button.tsx"),
            CodeLanguage::TypeScript
        );
    }

    #[test]
    fn test_code_file_index_entry() {
        let mut entry = CodeFileIndexEntry::new(
            "/path/to/file.rs".to_string(),
            CodeLanguage::Rust,
            "abc123".to_string(),
        );

        assert!(entry.is_empty());

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        entry.add_entity(id1);
        entry.add_entity(id2);
        assert_eq!(entry.entity_count(), 2);

        entry.add_entity(id1); // Duplicate
        assert_eq!(entry.entity_count(), 2);

        assert!(entry.remove_entity(id1));
        assert_eq!(entry.entity_count(), 1);
        assert!(!entry.remove_entity(id1)); // Already removed
    }

    #[test]
    fn test_visibility_parsing() {
        assert_eq!(Visibility::from_rust_str("pub"), Visibility::Public);
        assert_eq!(
            Visibility::from_rust_str("pub(crate)"),
            Visibility::PublicCrate
        );
        assert_eq!(
            Visibility::from_rust_str("pub(super)"),
            Visibility::PublicSuper
        );
        assert_eq!(Visibility::from_rust_str(""), Visibility::Private);
    }

    #[test]
    fn test_content_hash_change_detection() {
        let entity = CodeEntity::new(
            CodeEntityType::Function,
            "test".to_string(),
            "fn test() {}".to_string(),
            CodeLanguage::Rust,
            "/test.rs".to_string(),
            1,
            1,
        );

        assert!(!entity.content_changed("fn test() {}"));
        assert!(entity.content_changed("fn test() { todo!() }"));
    }
}
