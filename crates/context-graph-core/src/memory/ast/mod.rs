mod classify;
mod routing;

pub use routing::{
    route_for_entity_type, route_for_key, route_to_embedders, routing_table_entries,
    try_route_to_embedders, DirectInstrument, EmbedderId, RoutingError, RoutingKey, RoutingResult,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use thiserror::Error;
use tree_sitter::{Node, Parser};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Go,
    Java,
    C,
    Cpp,
    CSharp,
    Ruby,
    Php,
}

impl Language {
    #[rustfmt::skip]
    pub fn all() -> [Self; 11] {
        [Self::Rust, Self::Python, Self::JavaScript, Self::TypeScript, Self::Go, Self::Java, Self::C, Self::Cpp, Self::CSharp, Self::Ruby, Self::Php]
    }

    pub fn slug(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Python => "python",
            Self::JavaScript => "javascript",
            Self::TypeScript => "typescript",
            Self::Go => "go",
            Self::Java => "java",
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::CSharp => "csharp",
            Self::Ruby => "ruby",
            Self::Php => "php",
        }
    }

    pub fn from_slug(slug: &str) -> Result<Self, AstChunkerError> {
        match slug.to_ascii_lowercase().as_str() {
            "rust" | "rs" => Ok(Self::Rust),
            "python" | "py" => Ok(Self::Python),
            "javascript" | "js" | "mjs" | "cjs" => Ok(Self::JavaScript),
            "typescript" | "ts" | "tsx" | "mts" | "cts" => Ok(Self::TypeScript),
            "go" => Ok(Self::Go),
            "java" => Ok(Self::Java),
            "c" => Ok(Self::C),
            "cpp" | "c++" | "cc" | "cxx" | "hpp" | "hh" | "hxx" => Ok(Self::Cpp),
            "csharp" | "c#" | "cs" => Ok(Self::CSharp),
            "ruby" | "rb" => Ok(Self::Ruby),
            "php" => Ok(Self::Php),
            requested => Err(AstChunkerError::UnsupportedLanguage {
                requested: requested.to_string(),
            }),
        }
    }

    pub fn from_path(path: &str) -> Result<Self, AstChunkerError> {
        let ext = path.rsplit('.').next().unwrap_or(path);
        Self::from_slug(ext)
    }

    fn tree_sitter_language(self, path: &str) -> tree_sitter::Language {
        match self {
            Self::Rust => tree_sitter_rust::LANGUAGE.into(),
            Self::Python => tree_sitter_python::LANGUAGE.into(),
            Self::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Self::TypeScript if path.ends_with(".tsx") => {
                tree_sitter_typescript::LANGUAGE_TSX.into()
            }
            Self::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Self::Go => tree_sitter_go::LANGUAGE.into(),
            Self::Java => tree_sitter_java::LANGUAGE.into(),
            Self::C => tree_sitter_c::LANGUAGE.into(),
            Self::Cpp => tree_sitter_cpp::LANGUAGE.into(),
            Self::CSharp => tree_sitter_c_sharp::LANGUAGE.into(),
            Self::Ruby => tree_sitter_ruby::LANGUAGE.into(),
            Self::Php => tree_sitter_php::LANGUAGE_PHP.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    Function,
    Method,
    Class,
    Struct,
    Enum,
    TraitOrInterface,
    Impl,
    Module,
    Namespace,
    TestFunction,
    Import,
    CommentBlock,
    Docstring,
}

impl EntityType {
    #[rustfmt::skip]
    pub fn all() -> [Self; 13] {
        [Self::Function, Self::Method, Self::Class, Self::Struct, Self::Enum, Self::TraitOrInterface, Self::Impl, Self::Module, Self::Namespace, Self::TestFunction, Self::Import, Self::CommentBlock, Self::Docstring]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParseDiagnostic {
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AstChunk {
    pub language: Language,
    pub entity_type: EntityType,
    pub parent_chain: Vec<String>,
    pub line_start: u32,
    pub line_end: u32,
    pub start_byte: usize,
    pub end_byte: usize,
    pub sha256: String,
    pub parse_diagnostics: Vec<ParseDiagnostic>,
    pub content: String,
    pub embedder_routing_set: BTreeSet<EmbedderId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AstChunkOptions {
    pub file_path: String,
    pub max_non_ws_chars: usize,
}

impl AstChunkOptions {
    pub fn for_path(path: impl Into<String>) -> Self {
        Self {
            file_path: path.into(),
            max_non_ws_chars: 500,
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AstChunkerError {
    #[error("parse failed for {file_path} as {language:?}: {reason}; diagnostics={diagnostics:?}")]
    ParseFailed {
        file_path: String,
        language: Language,
        reason: String,
        diagnostics: Vec<ParseDiagnostic>,
    },
    #[error("unsupported language requested: {requested}")]
    UnsupportedLanguage { requested: String },
    #[error("empty source passed to AST chunker")]
    EmptySource,
    #[error("mixed-language source rejected: primary={primary:?}, embedded={embedded}")]
    MixedLanguage { primary: Language, embedded: String },
    #[error("failed to set tree-sitter language {language}: {reason}")]
    LanguageSetFailed { language: String, reason: String },
    #[error("embedder route missing for {entity_type:?} in {language:?}: {reason}")]
    RoutingMissing {
        language: Language,
        entity_type: EntityType,
        reason: String,
    },
}

impl AstChunkerError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::ParseFailed { .. } => "MEJEPA_CHUNKER_PARSE_FAILED",
            Self::UnsupportedLanguage { .. } => "MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE",
            Self::EmptySource => "MEJEPA_CHUNKER_EMPTY_SOURCE",
            Self::MixedLanguage { .. } => "MEJEPA_CHUNKER_MIXED_LANGUAGE",
            Self::LanguageSetFailed { .. } => "MEJEPA_CHUNKER_LANGUAGE_SET_FAILED",
            Self::RoutingMissing { .. } => "MEJEPA_EMBED_ROUTING_MISSING",
        }
    }
}

pub fn chunk(source: &[u8], language: Language) -> Result<Vec<AstChunk>, AstChunkerError> {
    chunk_with_options(source, language, &AstChunkOptions::for_path("<memory>"))
}

pub fn chunk_path(source: &[u8], path: &str) -> Result<Vec<AstChunk>, AstChunkerError> {
    let language = Language::from_path(path)?;
    chunk_with_options(source, language, &AstChunkOptions::for_path(path))
}

pub fn chunk_with_options(
    source: &[u8],
    language: Language,
    options: &AstChunkOptions,
) -> Result<Vec<AstChunk>, AstChunkerError> {
    if source.iter().all(|byte| byte.is_ascii_whitespace()) {
        return Err(AstChunkerError::EmptySource);
    }
    let source_text = std::str::from_utf8(source).map_err(|err| AstChunkerError::ParseFailed {
        file_path: options.file_path.clone(),
        language,
        reason: format!("source is not valid UTF-8: {err}"),
        diagnostics: vec![],
    })?;
    detect_mixed_language(source_text, language)?;

    let grammar = language.tree_sitter_language(&options.file_path);
    let mut parser = Parser::new();
    parser
        .set_language(&grammar)
        .map_err(|err| AstChunkerError::LanguageSetFailed {
            language: language.slug().to_string(),
            reason: err.to_string(),
        })?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| AstChunkerError::ParseFailed {
            file_path: options.file_path.clone(),
            language,
            reason: "tree-sitter returned no parse tree".to_string(),
            diagnostics: vec![],
        })?;
    let root = tree.root_node();
    if root.has_error() {
        let diagnostics = parse_diagnostics(root, source);
        return Err(AstChunkerError::ParseFailed {
            file_path: options.file_path.clone(),
            language,
            reason: "tree-sitter reported ERROR or MISSING nodes".to_string(),
            diagnostics,
        });
    }

    let mut chunks = Vec::new();
    let mut scope = Vec::new();
    walk_and_emit(root, source, language, options, &mut scope, &mut chunks)?;
    if chunks.is_empty() {
        chunks.push(build_chunk(
            root,
            source,
            language,
            EntityType::Module,
            options,
            &[],
        )?);
    }
    Ok(chunks)
}

fn walk_and_emit(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    options: &AstChunkOptions,
    scope: &mut Vec<String>,
    chunks: &mut Vec<AstChunk>,
) -> Result<(), AstChunkerError> {
    let entity_type = classify::classify_node(language, node, source, &options.file_path);
    if let Some(entity_type) = entity_type {
        chunks.push(build_chunk(
            node,
            source,
            language,
            entity_type,
            options,
            scope,
        )?);
    }

    let pushed = if entity_type.is_some() {
        classify::node_name(language, node, source).map(|name| {
            scope.push(name);
        })
    } else {
        None
    };

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_and_emit(child, source, language, options, scope, chunks)?;
    }
    if pushed.is_some() {
        scope.pop();
    }
    Ok(())
}

fn build_chunk(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    entity_type: EntityType,
    options: &AstChunkOptions,
    parent_chain: &[String],
) -> Result<AstChunk, AstChunkerError> {
    let line_start = node.start_position().row as u32 + 1;
    let line_end = node.end_position().row as u32 + 1;
    let raw = node.utf8_text(source).unwrap_or("");
    let routing = try_route_to_embedders(entity_type, Some(language)).map_err(|err| {
        AstChunkerError::RoutingMissing {
            language,
            entity_type,
            reason: format!("{}: {}", err.code(), err),
        }
    })?;
    let non_ws = raw.chars().filter(|ch| !ch.is_whitespace()).count();
    let content = format!(
        "path: {}\nlanguage: {}\nentity_type: {:?}\nparent_chain: {}\nlines: {}-{}\nnon_ws_chars: {}\n--- source ---\n{}",
        options.file_path,
        language.slug(),
        entity_type,
        parent_chain.join("::"),
        line_start,
        line_end,
        non_ws,
        raw
    );
    Ok(AstChunk {
        language,
        entity_type,
        parent_chain: parent_chain.to_vec(),
        line_start,
        line_end,
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
        sha256: structural_sha256(node, source, language, entity_type, parent_chain),
        parse_diagnostics: vec![],
        content,
        embedder_routing_set: routing,
    })
}

fn structural_sha256(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    entity_type: EntityType,
    parent_chain: &[String],
) -> String {
    let mut signature = format!(
        "{:?}|{:?}|{}\n",
        language,
        entity_type,
        parent_chain.join("::")
    );
    append_structural_tokens(node, source, &mut signature);
    hex_sha256(signature.as_bytes())
}

fn append_structural_tokens(node: Node<'_>, source: &[u8], out: &mut String) {
    if node.is_named() || node.is_extra() {
        out.push_str(node.kind());
        if stable_text_kind(node.kind()) {
            if let Ok(text) = node.utf8_text(source) {
                out.push(':');
                out.push_str(&normalize_token_text(text));
            }
        }
        out.push('\n');
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        append_structural_tokens(child, source, out);
    }
}

fn stable_text_kind(kind: &str) -> bool {
    matches!(
        kind,
        "identifier"
            | "type_identifier"
            | "field_identifier"
            | "property_identifier"
            | "scoped_identifier"
            | "namespace_identifier"
            | "name"
            | "string"
            | "string_literal"
            | "comment"
            | "line_comment"
            | "block_comment"
            | "integer"
            | "number"
            | "float"
    )
}

fn normalize_token_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn parse_diagnostics(root: Node<'_>, source: &[u8]) -> Vec<ParseDiagnostic> {
    let mut diagnostics = Vec::new();
    collect_parse_diagnostics(root, source, &mut diagnostics);
    diagnostics.truncate(32);
    diagnostics
}

fn collect_parse_diagnostics(
    node: Node<'_>,
    source: &[u8],
    diagnostics: &mut Vec<ParseDiagnostic>,
) {
    if diagnostics.len() >= 32 {
        return;
    }
    if node.is_error() || node.is_missing() {
        diagnostics.push(ParseDiagnostic {
            kind: node.kind().to_string(),
            is_error: node.is_error(),
            is_missing: node.is_missing(),
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            start_line: node.start_position().row as u32 + 1,
            start_column: node.start_position().column as u32,
            end_line: node.end_position().row as u32 + 1,
            end_column: node.end_position().column as u32,
            excerpt: diagnostic_excerpt(node, source),
        });
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.has_error() || child.is_missing() {
            collect_parse_diagnostics(child, source, diagnostics);
        }
    }
}

fn diagnostic_excerpt(node: Node<'_>, source: &[u8]) -> String {
    let start = node.start_byte().saturating_sub(64);
    let end = (node.end_byte() + 64).min(source.len());
    std::str::from_utf8(&source[start..end])
        .unwrap_or("")
        .chars()
        .take(128)
        .collect()
}

fn detect_mixed_language(source: &str, language: Language) -> Result<(), AstChunkerError> {
    match language {
        Language::Php if source.contains("?>") => Err(AstChunkerError::MixedLanguage {
            primary: language,
            embedded: "html".to_string(),
        }),
        Language::JavaScript | Language::TypeScript
            if source.lines().any(|line| {
                let lower = line.to_ascii_lowercase();
                lower.contains("<script") || lower.contains("<style")
            }) =>
        {
            Err(AstChunkerError::MixedLanguage {
                primary: language,
                embedded: "html".to_string(),
            })
        }
        _ => Ok(()),
    }
}

fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}

#[cfg(test)]
mod tests;
