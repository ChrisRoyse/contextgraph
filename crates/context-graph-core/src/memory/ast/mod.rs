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

use ruff_python_ast::{
    token::Tokens, ExceptHandler, ModModule, Stmt, StmtClassDef, StmtFunctionDef,
};
use ruff_text_size::{Ranged, TextRange, TextSize};

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
    #[serde(default)]
    pub symbol_name: Option<String>,
    pub parent_chain: Vec<String>,
    pub line_start: u32,
    pub line_end: u32,
    pub start_byte: usize,
    pub end_byte: usize,
    pub sha256: String,
    pub parse_diagnostics: Vec<ParseDiagnostic>,
    #[serde(default)]
    pub raw_source: String,
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
    if language == Language::Python {
        return chunk_python_with_options(source_text, source, options);
    }

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PythonScopeKind {
    Class,
    Function,
}

fn chunk_python_with_options(
    source_text: &str,
    source: &[u8],
    options: &AstChunkOptions,
) -> Result<Vec<AstChunk>, AstChunkerError> {
    let parsed = ruff_python_parser::parse_module(source_text).map_err(|err| {
        AstChunkerError::ParseFailed {
            file_path: options.file_path.clone(),
            language: Language::Python,
            reason: format!("ruff-python-parser rejected source: {err}"),
            diagnostics: vec![ruff_parse_diagnostic(&err, source)],
        }
    })?;
    let tokens = parsed.tokens().clone();
    let module = parsed.into_syntax();
    let mut chunks = Vec::new();
    if let Some(module_chunk) =
        build_ruff_python_module_globals_chunk(&module, &tokens, source, options)?
    {
        chunks.push(module_chunk);
    }
    let mut scope_names = Vec::new();
    let mut scope_kinds = Vec::new();
    walk_ruff_python_suite(
        &module.body,
        &tokens,
        source,
        options,
        &mut scope_names,
        &mut scope_kinds,
        &mut chunks,
    )?;
    if chunks.is_empty() {
        chunks.push(build_text_chunk(TextChunkInput {
            language: Language::Python,
            entity_type: EntityType::Module,
            parent_chain: &[],
            symbol_name: None,
            start_byte: 0,
            end_byte: source.len(),
            raw: source_text.to_string(),
            source,
            options,
            sha256: None,
        })?);
    }
    Ok(chunks)
}

fn walk_ruff_python_suite(
    suite: &[Stmt],
    tokens: &Tokens,
    source: &[u8],
    options: &AstChunkOptions,
    scope_names: &mut Vec<String>,
    scope_kinds: &mut Vec<PythonScopeKind>,
    chunks: &mut Vec<AstChunk>,
) -> Result<(), AstChunkerError> {
    for stmt in suite {
        walk_ruff_python_stmt(
            stmt,
            tokens,
            source,
            options,
            scope_names,
            scope_kinds,
            chunks,
        )?;
    }
    Ok(())
}

fn walk_ruff_python_stmt(
    stmt: &Stmt,
    tokens: &Tokens,
    source: &[u8],
    options: &AstChunkOptions,
    scope_names: &mut Vec<String>,
    scope_kinds: &mut Vec<PythonScopeKind>,
    chunks: &mut Vec<AstChunk>,
) -> Result<(), AstChunkerError> {
    match stmt {
        Stmt::FunctionDef(function) => {
            let entity_type = ruff_python_function_entity_type(function, scope_kinds);
            chunks.push(build_ruff_python_span_chunk(
                entity_type,
                scope_names,
                Some(function.name.as_str().to_string()),
                ruff_python_function_start_byte(function, source),
                text_range_end(function.range()),
                source,
                options,
                tokens,
            )?);
            scope_names.push(function.name.as_str().to_string());
            scope_kinds.push(PythonScopeKind::Function);
            walk_ruff_python_suite(
                &function.body,
                tokens,
                source,
                options,
                scope_names,
                scope_kinds,
                chunks,
            )?;
            scope_kinds.pop();
            scope_names.pop();
        }
        Stmt::ClassDef(class_def) => {
            chunks.push(build_ruff_python_class_shell_chunk(
                class_def,
                scope_names,
                source,
                options,
            )?);
            scope_names.push(class_def.name.as_str().to_string());
            scope_kinds.push(PythonScopeKind::Class);
            walk_ruff_python_suite(
                &class_def.body,
                tokens,
                source,
                options,
                scope_names,
                scope_kinds,
                chunks,
            )?;
            scope_kinds.pop();
            scope_names.pop();
        }
        Stmt::Import(_) | Stmt::ImportFrom(_) => {
            chunks.push(build_ruff_python_span_chunk(
                EntityType::Import,
                scope_names,
                None,
                line_start_byte(source, text_range_start(stmt.range())),
                text_range_end(stmt.range()),
                source,
                options,
                tokens,
            )?);
        }
        _ => walk_ruff_python_nested_suites(
            stmt,
            tokens,
            source,
            options,
            scope_names,
            scope_kinds,
            chunks,
        )?,
    }
    Ok(())
}

fn walk_ruff_python_nested_suites(
    stmt: &Stmt,
    tokens: &Tokens,
    source: &[u8],
    options: &AstChunkOptions,
    scope_names: &mut Vec<String>,
    scope_kinds: &mut Vec<PythonScopeKind>,
    chunks: &mut Vec<AstChunk>,
) -> Result<(), AstChunkerError> {
    match stmt {
        Stmt::For(stmt_for) => {
            walk_ruff_python_suite(
                &stmt_for.body,
                tokens,
                source,
                options,
                scope_names,
                scope_kinds,
                chunks,
            )?;
            walk_ruff_python_suite(
                &stmt_for.orelse,
                tokens,
                source,
                options,
                scope_names,
                scope_kinds,
                chunks,
            )?;
        }
        Stmt::While(stmt_while) => {
            walk_ruff_python_suite(
                &stmt_while.body,
                tokens,
                source,
                options,
                scope_names,
                scope_kinds,
                chunks,
            )?;
            walk_ruff_python_suite(
                &stmt_while.orelse,
                tokens,
                source,
                options,
                scope_names,
                scope_kinds,
                chunks,
            )?;
        }
        Stmt::If(stmt_if) => {
            walk_ruff_python_suite(
                &stmt_if.body,
                tokens,
                source,
                options,
                scope_names,
                scope_kinds,
                chunks,
            )?;
            for clause in &stmt_if.elif_else_clauses {
                walk_ruff_python_suite(
                    &clause.body,
                    tokens,
                    source,
                    options,
                    scope_names,
                    scope_kinds,
                    chunks,
                )?;
            }
        }
        Stmt::With(stmt_with) => {
            walk_ruff_python_suite(
                &stmt_with.body,
                tokens,
                source,
                options,
                scope_names,
                scope_kinds,
                chunks,
            )?;
        }
        Stmt::Match(stmt_match) => {
            for case in &stmt_match.cases {
                walk_ruff_python_suite(
                    &case.body,
                    tokens,
                    source,
                    options,
                    scope_names,
                    scope_kinds,
                    chunks,
                )?;
            }
        }
        Stmt::Try(stmt_try) => {
            walk_ruff_python_suite(
                &stmt_try.body,
                tokens,
                source,
                options,
                scope_names,
                scope_kinds,
                chunks,
            )?;
            for handler in &stmt_try.handlers {
                let ExceptHandler::ExceptHandler(handler) = handler;
                walk_ruff_python_suite(
                    &handler.body,
                    tokens,
                    source,
                    options,
                    scope_names,
                    scope_kinds,
                    chunks,
                )?;
            }
            walk_ruff_python_suite(
                &stmt_try.orelse,
                tokens,
                source,
                options,
                scope_names,
                scope_kinds,
                chunks,
            )?;
            walk_ruff_python_suite(
                &stmt_try.finalbody,
                tokens,
                source,
                options,
                scope_names,
                scope_kinds,
                chunks,
            )?;
        }
        _ => {}
    }
    Ok(())
}

fn ruff_python_function_entity_type(
    function: &StmtFunctionDef,
    scope_kinds: &[PythonScopeKind],
) -> EntityType {
    if python_is_test_function_name(function.name.as_str()) {
        EntityType::TestFunction
    } else if scope_kinds.last() == Some(&PythonScopeKind::Class) {
        EntityType::Method
    } else {
        EntityType::Function
    }
}

fn build_ruff_python_module_globals_chunk(
    module: &ModModule,
    tokens: &Tokens,
    source: &[u8],
    options: &AstChunkOptions,
) -> Result<Option<AstChunk>, AstChunkerError> {
    let mut ranges = Vec::new();
    for stmt in &module.body {
        if is_ruff_python_module_global_stmt(stmt) {
            ranges.push((
                line_start_byte(source, text_range_start(stmt.range())),
                text_range_end(stmt.range()),
            ));
        }
    }
    if ranges.is_empty() {
        return Ok(None);
    }
    let raw = join_source_ranges(source, &ranges);
    Ok(Some(build_text_chunk(TextChunkInput {
        language: Language::Python,
        entity_type: EntityType::Module,
        parent_chain: &[],
        symbol_name: None,
        start_byte: ranges[0].0,
        end_byte: ranges.last().map(|(_, end)| *end).unwrap_or(ranges[0].1),
        raw,
        source,
        options,
        sha256: Some(python_token_sha256(
            tokens,
            source,
            ranges[0].0,
            ranges.last().map(|(_, end)| *end).unwrap_or(ranges[0].1),
            EntityType::Module,
            &[],
        )),
    })?))
}

fn build_ruff_python_class_shell_chunk(
    class_def: &StmtClassDef,
    parent_chain: &[String],
    source: &[u8],
    options: &AstChunkOptions,
) -> Result<AstChunk, AstChunkerError> {
    let start_byte = ruff_python_class_start_byte(class_def, source);
    let class_end = text_range_end(class_def.range());
    let header_end = class_def
        .body
        .first()
        .map(|stmt| text_range_start(stmt.range()))
        .unwrap_or(class_end);
    let class_line = line_for_byte(source, start_byte);
    let class_indent = python_line_indent(source, start_byte);
    let mut raw_parts = vec![source_slice(source, start_byte, header_end)
        .trim_end()
        .to_string()];
    let mut included_parts = 1_usize;
    let mut end_byte = header_end;
    for stmt in &class_def.body {
        if !stmt_contains_python_definition(stmt) {
            let child_start = text_range_start(stmt.range());
            let child_end = text_range_end(stmt.range());
            if line_for_byte(source, child_start) == class_line {
                raw_parts.push(format!(
                    "{}    {}",
                    class_indent,
                    source_slice(source, child_start, child_end).trim()
                ));
            } else {
                raw_parts.push(
                    source_slice(source, line_start_byte(source, child_start), child_end)
                        .trim_end()
                        .to_string(),
                );
            }
            included_parts += 1;
            end_byte = child_end;
        }
    }
    let mut raw = raw_parts
        .into_iter()
        .filter(|part| !part.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    if included_parts == 1 {
        raw.push_str(&python_shell_pass_line(source, start_byte));
    }
    let sha256 = atomic_text_sha256(Language::Python, EntityType::Class, parent_chain, &raw);
    build_text_chunk(TextChunkInput {
        language: Language::Python,
        entity_type: EntityType::Class,
        parent_chain,
        symbol_name: Some(class_def.name.as_str().to_string()),
        start_byte,
        end_byte,
        raw,
        source,
        options,
        sha256: Some(sha256),
    })
}

fn build_ruff_python_span_chunk(
    entity_type: EntityType,
    parent_chain: &[String],
    symbol_name: Option<String>,
    start_byte: usize,
    end_byte: usize,
    source: &[u8],
    options: &AstChunkOptions,
    tokens: &Tokens,
) -> Result<AstChunk, AstChunkerError> {
    let raw = source_slice(source, start_byte, end_byte);
    let sha256 = python_token_sha256(
        tokens,
        source,
        start_byte,
        end_byte,
        entity_type,
        parent_chain,
    );
    build_text_chunk(TextChunkInput {
        language: Language::Python,
        entity_type,
        parent_chain,
        symbol_name,
        start_byte,
        end_byte,
        raw,
        source,
        options,
        sha256: Some(sha256),
    })
}

fn is_ruff_python_module_global_stmt(stmt: &Stmt) -> bool {
    !matches!(
        stmt,
        Stmt::Import(_) | Stmt::ImportFrom(_) | Stmt::FunctionDef(_) | Stmt::ClassDef(_)
    ) && !stmt_contains_python_definition(stmt)
}

fn stmt_contains_python_definition(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::FunctionDef(_) | Stmt::ClassDef(_) => true,
        Stmt::For(stmt_for) => {
            suite_contains_python_definition(&stmt_for.body)
                || suite_contains_python_definition(&stmt_for.orelse)
        }
        Stmt::While(stmt_while) => {
            suite_contains_python_definition(&stmt_while.body)
                || suite_contains_python_definition(&stmt_while.orelse)
        }
        Stmt::If(stmt_if) => {
            suite_contains_python_definition(&stmt_if.body)
                || stmt_if
                    .elif_else_clauses
                    .iter()
                    .any(|clause| suite_contains_python_definition(&clause.body))
        }
        Stmt::With(stmt_with) => suite_contains_python_definition(&stmt_with.body),
        Stmt::Match(stmt_match) => stmt_match
            .cases
            .iter()
            .any(|case| suite_contains_python_definition(&case.body)),
        Stmt::Try(stmt_try) => {
            suite_contains_python_definition(&stmt_try.body)
                || stmt_try.handlers.iter().any(|handler| {
                    let ExceptHandler::ExceptHandler(handler) = handler;
                    suite_contains_python_definition(&handler.body)
                })
                || suite_contains_python_definition(&stmt_try.orelse)
                || suite_contains_python_definition(&stmt_try.finalbody)
        }
        _ => false,
    }
}

fn suite_contains_python_definition(suite: &[Stmt]) -> bool {
    suite.iter().any(stmt_contains_python_definition)
}

fn ruff_python_function_start_byte(function: &StmtFunctionDef, source: &[u8]) -> usize {
    let start = function
        .decorator_list
        .first()
        .map(|decorator| text_range_start(decorator.range()))
        .unwrap_or_else(|| text_range_start(function.range()));
    line_start_byte(source, start)
}

fn ruff_python_class_start_byte(class_def: &StmtClassDef, source: &[u8]) -> usize {
    let start = class_def
        .decorator_list
        .first()
        .map(|decorator| text_range_start(decorator.range()))
        .unwrap_or_else(|| text_range_start(class_def.range()));
    line_start_byte(source, start)
}

fn python_token_sha256(
    tokens: &Tokens,
    source: &[u8],
    start_byte: usize,
    end_byte: usize,
    entity_type: EntityType,
    parent_chain: &[String],
) -> String {
    let start = text_size_from_usize(start_byte);
    let end = text_size_from_usize(end_byte);
    let mut signature = format!(
        "{:?}|{:?}|{}\n",
        Language::Python,
        entity_type,
        parent_chain.join("::")
    );
    for token in tokens.iter() {
        if token.end() <= start || token.start() >= end {
            continue;
        }
        let kind = token.kind();
        if kind.is_trivia()
            || kind.is_eof()
            || matches!(
                kind,
                ruff_python_ast::token::TokenKind::Newline
                    | ruff_python_ast::token::TokenKind::Indent
                    | ruff_python_ast::token::TokenKind::Dedent
            )
        {
            continue;
        }
        signature.push_str(&format!("{kind:?}"));
        if matches!(kind, ruff_python_ast::token::TokenKind::Name)
            || matches!(
                kind,
                ruff_python_ast::token::TokenKind::String
                    | ruff_python_ast::token::TokenKind::FStringStart
                    | ruff_python_ast::token::TokenKind::FStringMiddle
                    | ruff_python_ast::token::TokenKind::FStringEnd
                    | ruff_python_ast::token::TokenKind::TStringStart
                    | ruff_python_ast::token::TokenKind::TStringMiddle
                    | ruff_python_ast::token::TokenKind::TStringEnd
            )
            || matches!(
                kind,
                ruff_python_ast::token::TokenKind::Int
                    | ruff_python_ast::token::TokenKind::Float
                    | ruff_python_ast::token::TokenKind::Complex
            )
        {
            let token_start = token.start().to_usize();
            let token_end = token.end().to_usize();
            signature.push(':');
            signature.push_str(&normalize_token_text(&source_slice(
                source,
                token_start,
                token_end,
            )));
        }
        signature.push('\n');
    }
    hex_sha256(signature.as_bytes())
}

struct TextChunkInput<'a> {
    language: Language,
    entity_type: EntityType,
    parent_chain: &'a [String],
    symbol_name: Option<String>,
    start_byte: usize,
    end_byte: usize,
    raw: String,
    source: &'a [u8],
    options: &'a AstChunkOptions,
    sha256: Option<String>,
}

fn build_text_chunk(input: TextChunkInput<'_>) -> Result<AstChunk, AstChunkerError> {
    let TextChunkInput {
        language,
        entity_type,
        parent_chain,
        symbol_name,
        start_byte,
        end_byte,
        raw,
        source,
        options,
        sha256,
    } = input;
    let routing = try_route_to_embedders(entity_type, Some(language)).map_err(|err| {
        AstChunkerError::RoutingMissing {
            language,
            entity_type,
            reason: format!("{}: {}", err.code(), err),
        }
    })?;
    let line_start = line_for_byte(source, start_byte);
    let line_end = line_for_byte(source, end_byte);
    let non_ws = raw.chars().filter(|ch| !ch.is_whitespace()).count();
    let content = format!(
        "path: {}\nlanguage: {}\nentity_type: {:?}\nsymbol_name: {}\nparent_chain: {}\nlines: {}-{}\nnon_ws_chars: {}\n--- source ---\n{}",
        options.file_path,
        language.slug(),
        entity_type,
        symbol_name.as_deref().unwrap_or(""),
        parent_chain.join("::"),
        line_start,
        line_end,
        non_ws,
        raw
    );
    Ok(AstChunk {
        language,
        entity_type,
        symbol_name,
        parent_chain: parent_chain.to_vec(),
        line_start,
        line_end,
        start_byte,
        end_byte,
        sha256: sha256
            .unwrap_or_else(|| atomic_text_sha256(language, entity_type, parent_chain, &raw)),
        parse_diagnostics: vec![],
        raw_source: raw,
        content,
        embedder_routing_set: routing,
    })
}

fn ruff_parse_diagnostic(err: &ruff_python_parser::ParseError, source: &[u8]) -> ParseDiagnostic {
    let start_byte = text_range_start(err.range());
    let end_byte = text_range_end(err.range()).max(start_byte);
    ParseDiagnostic {
        kind: format!("{:?}", &err.error),
        is_error: true,
        is_missing: false,
        start_byte,
        end_byte,
        start_line: line_for_byte(source, start_byte),
        start_column: column_for_byte(source, start_byte),
        end_line: line_for_byte(source, end_byte),
        end_column: column_for_byte(source, end_byte),
        excerpt: byte_excerpt(start_byte, end_byte, source),
    }
}

fn text_range_start(range: TextRange) -> usize {
    range.start().to_usize()
}

fn text_range_end(range: TextRange) -> usize {
    range.end().to_usize()
}

fn text_size_from_usize(value: usize) -> TextSize {
    TextSize::try_from(value).expect("Python source exceeds ruff_text_size u32 range")
}

fn column_for_byte(source: &[u8], byte: usize) -> u32 {
    byte.saturating_sub(line_start_byte(source, byte)) as u32
}

fn byte_excerpt(start_byte: usize, end_byte: usize, source: &[u8]) -> String {
    let start = start_byte.saturating_sub(64);
    let end = (end_byte + 64).min(source.len());
    std::str::from_utf8(&source[start..end])
        .unwrap_or("")
        .chars()
        .take(128)
        .collect()
}

fn python_is_test_function_name(name: &str) -> bool {
    name.starts_with("test_") || name.ends_with("_test") || name.ends_with("Test")
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
    let start_byte = node.start_byte();
    let end_byte = node.end_byte();
    let raw = node.utf8_text(source).unwrap_or("").to_string();
    let symbol_name = classify::node_name(language, node, source);
    let sha256 = structural_sha256(node, source, language, entity_type, parent_chain);
    let routing = try_route_to_embedders(entity_type, Some(language)).map_err(|err| {
        AstChunkerError::RoutingMissing {
            language,
            entity_type,
            reason: format!("{}: {}", err.code(), err),
        }
    })?;
    let non_ws = raw.chars().filter(|ch| !ch.is_whitespace()).count();
    let content = format!(
        "path: {}\nlanguage: {}\nentity_type: {:?}\nsymbol_name: {}\nparent_chain: {}\nlines: {}-{}\nnon_ws_chars: {}\n--- source ---\n{}",
        options.file_path,
        language.slug(),
        entity_type,
        symbol_name.as_deref().unwrap_or(""),
        parent_chain.join("::"),
        line_start,
        line_end,
        non_ws,
        raw
    );
    Ok(AstChunk {
        language,
        entity_type,
        symbol_name,
        parent_chain: parent_chain.to_vec(),
        line_start,
        line_end,
        start_byte,
        end_byte,
        sha256,
        parse_diagnostics: vec![],
        raw_source: raw,
        content,
        embedder_routing_set: routing,
    })
}

fn source_slice(source: &[u8], start: usize, end: usize) -> String {
    std::str::from_utf8(&source[start..end])
        .unwrap_or("")
        .to_string()
}

fn line_start_byte(source: &[u8], byte: usize) -> usize {
    let mut index = byte.min(source.len());
    while index > 0 && source[index - 1] != b'\n' {
        index -= 1;
    }
    index
}

fn python_shell_pass_line(source: &[u8], class_start_byte: usize) -> String {
    let indent = python_line_indent(source, class_start_byte);
    format!("\n{indent}    pass")
}

fn python_line_indent(source: &[u8], byte: usize) -> String {
    let line_start = line_start_byte(source, byte);
    let mut indent_end = line_start;
    while indent_end < source.len() && matches!(source[indent_end], b' ' | b'\t') {
        indent_end += 1;
    }
    let class_indent = &source[line_start..indent_end];
    std::str::from_utf8(class_indent)
        .expect("source was validated as UTF-8 before chunking")
        .to_string()
}

fn join_source_ranges(source: &[u8], ranges: &[(usize, usize)]) -> String {
    ranges
        .iter()
        .filter_map(|(start, end)| std::str::from_utf8(&source[*start..*end]).ok())
        .map(str::trim_end)
        .filter(|text| !text.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn atomic_text_sha256(
    language: Language,
    entity_type: EntityType,
    parent_chain: &[String],
    raw: &str,
) -> String {
    let signature = format!(
        "{:?}|{:?}|{}\n{}",
        language,
        entity_type,
        parent_chain.join("::"),
        normalize_token_text(raw)
    );
    hex_sha256(signature.as_bytes())
}

fn line_for_byte(source: &[u8], byte: usize) -> u32 {
    let end = byte.min(source.len());
    source[..end].iter().filter(|byte| **byte == b'\n').count() as u32 + 1
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
