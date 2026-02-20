//! Code Query Type Detection (ARCH-16)
//!
//! Detects whether a query is actual code syntax (Code2Code) or
//! natural language about code (Text2Code) for optimized E7 Code
//! embedder similarity computation.
//!
//! # Research Background
//!
//! Per LoRACode research, code search needs separate handling for:
//! - **Code2Code**: Query is actual code syntax ("fn process_batch<T>")
//! - **Text2Code**: Query is natural language about code ("batch processing function")
//!
//! E7 Code embeddings show different behavior depending on query type.
//! Benchmark results showed 25% success rate when treating all queries
//! uniformly, indicating the need for query-type-aware similarity computation.
//!
//! # Architecture
//!
//! - Query type detection is fast (O(n) string scan)
//! - Similarity adjustment is applied post-embedding comparison
//! - Integration point: `compute_embedder_scores` in storage layer

use serde::{Deserialize, Serialize};

/// Query type for code search operations.
///
/// Used to adjust E7 Code embedder similarity computation
/// based on whether the query is actual code or natural language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum CodeQueryType {
    /// Query is actual code syntax (e.g., "fn process_batch<T>", "impl Iterator")
    ///
    /// Code2Code queries benefit from higher precision matching,
    /// as structural similarity is more important than semantic similarity.
    Code2Code,

    /// Query is natural language about code (e.g., "batch processing function")
    ///
    /// Text2Code queries use standard semantic similarity,
    /// as the user is describing functionality, not syntax.
    Text2Code,

    /// Query doesn't appear to be code-related
    ///
    /// For non-code queries, E7 similarity should have reduced weight
    /// since the Code embedder is not optimized for general text.
    #[default]
    NonCode,
}

impl CodeQueryType {
    /// Check if this is a code-related query type.
    #[inline]
    pub fn is_code_related(self) -> bool {
        matches!(self, Self::Code2Code | Self::Text2Code)
    }

    /// Get a short name for display/logging.
    pub fn short_name(self) -> &'static str {
        match self {
            Self::Code2Code => "code2code",
            Self::Text2Code => "text2code",
            Self::NonCode => "non-code",
        }
    }
}

impl std::fmt::Display for CodeQueryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.short_name())
    }
}

// =============================================================================
// CODE SYNTAX INDICATORS
// Language-specific patterns that indicate actual code syntax
// =============================================================================

/// Rust code syntax indicators.
const RUST_INDICATORS: &[&str] = &[
    "fn ",
    "pub fn",
    "async fn",
    "unsafe fn",
    "const fn",
    "impl ",
    "struct ",
    "enum ",
    "trait ",
    "type ",
    "mod ",
    "use ",
    "pub(",
    "pub mod",
    "pub use",
    "pub struct",
    "pub enum",
    "pub trait",
    "->",
    "::",
    "Vec<",
    "Option<",
    "Result<",
    "Box<",
    "Arc<",
    "Rc<",
    "RefCell<",
    "Mutex<",
    "RwLock<",
    "&mut ",
    "&self",
    "Self::",
    "#[",
    "macro_rules!",
    "where ",
    "dyn ",
    "impl<",
    "for<",
    "'static",
    "'_",
    "move |",
    "async move",
    "tokio::",
    "std::",
];

/// Python code syntax indicators.
/// Note: "from " and "import " without context match common English, so we require more specific patterns.
const PYTHON_INDICATORS: &[&str] = &[
    "def ",
    "class ",
    "import os",
    "import sys",
    "import json",
    "import re",
    "import typing",
    "import numpy",
    "import pandas",
    "import requests",
    "import flask",
    "import django",
    " as np",      // Common numpy alias pattern
    " as pd",      // Common pandas alias pattern
    "from typing",
    "from os",
    "from collections",
    "from dataclasses",
    "async def",
    "await ",
    "self.",
    "__init__",
    "__main__",
    "__name__",
    "__str__",
    "__repr__",
    "lambda ",
    "yield ",
    "raise ",
    "except ",
    "finally:",
    "@property",
    "@staticmethod",
    "@classmethod",
    "@dataclass",
    "-> None",
    "-> bool",
    "-> str",
    "-> int",
    "-> float",
    "-> List[",
    "-> Dict[",
    "-> Optional[",
    "typing.",
];

/// JavaScript/TypeScript code syntax indicators.
/// Note: Some keywords like "interface" and "type" match common English, so we require more context.
const JS_TS_INDICATORS: &[&str] = &[
    "function ",
    "const ",
    "let ",
    "var ",
    "=> ",
    "async function",
    "export default",
    "export const",
    "export function",
    "export class",
    "import {",
    "import *",
    "interface {",     // TypeScript interface declaration
    "type {",          // TypeScript type alias
    "extends ",
    "implements ",
    "constructor(",
    "private ",
    "public ",
    "protected ",
    "readonly ",
    "Promise<",
    "Array<",
    "Map<",
    "Set<",
    "React.",
    "useState",
    "useEffect",
    "useCallback",
    "useMemo",
];

/// Go code syntax indicators.
const GO_INDICATORS: &[&str] = &[
    "func ",
    "func (",
    "package ",
    "type ",
    "struct {",
    "interface {",
    "chan ",
    "go func",
    "defer ",
    "select {",
    "case <-",
    "range ",
    "make(",
    "new(",
    "[]byte",
    "[]string",
    "map[",
    "error)",
    "context.",
    "http.",
    "fmt.",
    "sync.",
];

/// C/C++ code syntax indicators.
const C_CPP_INDICATORS: &[&str] = &[
    "int main",
    "void ",
    "char *",
    "int *",
    "float *",
    "double *",
    "const char*",
    "#include",
    "#define",
    "#ifdef",
    "#ifndef",
    "#pragma",
    "sizeof(",
    "malloc(",
    "free(",
    "nullptr",
    "std::vector",
    "std::string",
    "std::map",
    "std::unique_ptr",
    "std::shared_ptr",
    "template<",
    "typename ",
    "namespace ",
    "class ",
    "public:",
    "private:",
    "protected:",
    "virtual ",
    "override",
];

/// General code syntax indicators (language-agnostic).
const GENERAL_CODE_INDICATORS: &[&str] = &[
    "= {",
    "};",
    "();",
    "[]",
    "()",
    "{ }",
    ": string",  // TypeScript type annotation
    ": number",  // TypeScript type annotation
    ": boolean", // TypeScript type annotation
    ": any",     // TypeScript type annotation
    ": void",    // TypeScript type annotation
    "!= ",
    "== ",
    "=== ",
    "!== ",
    "|| ",
    "&& ",
    "<<",
    ">>",
    "+=",
    "-=",
    "*=",
    "/=",
    "++",
    "--",
    "/*",
    "*/",
    "//",
    "/**",
    "///",
    "```",
];

// =============================================================================
// NATURAL LANGUAGE CODE INDICATORS
// Terms that indicate natural language *about* code
// =============================================================================

/// Natural language terms about programming concepts.
const NL_CODE_INDICATORS: &[&str] = &[
    // Function/method concepts
    "function",
    "method",
    "procedure",
    "subroutine",
    "callback",
    "handler",
    "listener",
    // Class/type concepts
    "class",
    "struct",
    "interface",
    "type",
    "enum",
    "module",
    "package",
    // Variable concepts
    "variable",
    "constant",
    "parameter",
    "argument",
    "property",
    "field",
    "attribute",
    // Control flow concepts
    "loop",
    "iteration",
    "condition",
    "branch",
    "switch",
    "recursion",
    // OOP concepts
    "inheritance",
    "polymorphism",
    "encapsulation",
    "abstraction",
    "composition",
    "aggregation",
    // Error handling
    "error handling",
    "exception",
    "try catch",
    "error recovery",
    // Async concepts
    "async",
    "asynchronous",
    "concurrent",
    "parallel",
    "promise",
    "future",
    "await",
    "callback",
    // Data structures
    "array",
    "list",
    "vector",
    "map",
    "dictionary",
    "set",
    "hash",
    "tree",
    "graph",
    "queue",
    "stack",
    "heap",
    // I/O concepts
    "stream",
    "buffer",
    "socket",
    "file",
    "reader",
    "writer",
    // Serialization
    "parse",
    "serialize",
    "deserialize",
    "encode",
    "decode",
    "marshal",
    "unmarshal",
    // Design patterns
    "factory",
    "singleton",
    "observer",
    "strategy",
    "decorator",
    "adapter",
    "facade",
    "proxy",
    // Testing
    "unit test",
    "test case",
    "mock",
    "stub",
    "fixture",
    "assertion",
    // Generic programming
    "generic",
    "template",
    "trait",
    "protocol",
    "mixin",
    // Memory
    "memory",
    "allocation",
    "deallocation",
    "garbage collection",
    "reference",
    "pointer",
    "ownership",
    "borrowing",
    // API concepts
    "endpoint",
    "request",
    "response",
    "middleware",
    "router",
    "controller",
    "service",
    "repository",
    // Database
    "query",
    "transaction",
    "migration",
    "schema",
    "index",
    "constraint",
    // Action verbs commonly used when describing code
    "implement",
    "refactor",
    "optimize",
    "debug",
    "fix",
    "add",
    "remove",
    "update",
    "modify",
    "create",
    "delete",
    "fetch",
    "store",
    "retrieve",
    "validate",
    "convert",
    "transform",
    "process",
    "handle",
    "execute",
    "invoke",
    "call",
    "return",
];

/// Detect the type of code query based on syntax patterns.
///
/// This function analyzes the query string to determine if it contains
/// actual code syntax (Code2Code), natural language about code (Text2Code),
/// or neither (NonCode).
///
/// # Algorithm
///
/// 1. First check for strong code syntax indicators (language-specific patterns)
/// 2. Then check for general code syntax patterns
/// 3. Finally check for natural language programming terms
/// 4. Default to NonCode if no patterns match
///
/// # Performance
///
/// O(n * m) where n is query length and m is total indicator patterns.
/// In practice, very fast for typical query lengths (<1ms).
///
/// # Examples
///
/// ```
/// use context_graph_core::code::detect_code_query_type;
/// use context_graph_core::code::CodeQueryType;
///
/// // Code syntax
/// assert_eq!(detect_code_query_type("fn process_batch<T>()"), CodeQueryType::Code2Code);
/// assert_eq!(detect_code_query_type("impl Iterator for Counter"), CodeQueryType::Code2Code);
///
/// // Natural language about code
/// assert_eq!(detect_code_query_type("batch processing function"), CodeQueryType::Text2Code);
/// assert_eq!(detect_code_query_type("implement the iterator pattern"), CodeQueryType::Text2Code);
///
/// // Non-code
/// assert_eq!(detect_code_query_type("hello world"), CodeQueryType::NonCode);
/// ```
pub fn detect_code_query_type(query: &str) -> CodeQueryType {
    // Early return for empty queries
    if query.trim().is_empty() {
        return CodeQueryType::NonCode;
    }

    // Check for language-specific code indicators (highest confidence)
    let all_code_indicators = RUST_INDICATORS
        .iter()
        .chain(PYTHON_INDICATORS)
        .chain(JS_TS_INDICATORS)
        .chain(GO_INDICATORS)
        .chain(C_CPP_INDICATORS)
        .chain(GENERAL_CODE_INDICATORS);

    for indicator in all_code_indicators {
        if query.contains(indicator) {
            return CodeQueryType::Code2Code;
        }
    }

    // Check for natural language about code (case-insensitive)
    let query_lower = query.to_lowercase();
    for indicator in NL_CODE_INDICATORS {
        if query_lower.contains(indicator) {
            return CodeQueryType::Text2Code;
        }
    }

    CodeQueryType::NonCode
}

/// Compute E7 Code similarity with query-type awareness.
///
/// Adjusts the base cosine similarity based on the detected query type
/// to improve retrieval accuracy for code-related queries.
///
/// # Query Type Adjustments
///
/// - **Code2Code**: Sharpens similarity curve to boost high matches and penalize low ones.
///   This increases precision for exact structural matches.
///
/// - **Text2Code**: Uses standard similarity without modification.
///   Semantic similarity is appropriate for natural language queries.
///
/// - **NonCode**: Reduces similarity weight since E7 is not optimized for general text.
///   This prevents E7 from dominating for non-code queries.
///
/// # Arguments
///
/// * `query_embedding` - The E7 embedding of the query
/// * `doc_embedding` - The E7 embedding of the stored document
/// * `query_type` - The detected query type
///
/// # Returns
///
/// Adjusted similarity score in range [0.0, 1.0]
///
/// # Examples
///
/// ```
/// use context_graph_core::code::{compute_e7_similarity_with_query_type, CodeQueryType};
///
/// let query = vec![0.5; 1536];
/// let doc = vec![0.5; 1536];
///
/// // Code2Code boosts high similarities
/// let code2code_sim = compute_e7_similarity_with_query_type(&query, &doc, CodeQueryType::Code2Code);
///
/// // Text2Code uses standard similarity
/// let text2code_sim = compute_e7_similarity_with_query_type(&query, &doc, CodeQueryType::Text2Code);
///
/// // NonCode reduces weight
/// let noncode_sim = compute_e7_similarity_with_query_type(&query, &doc, CodeQueryType::NonCode);
/// ```
pub fn compute_e7_similarity_with_query_type(
    query_embedding: &[f32],
    doc_embedding: &[f32],
    query_type: CodeQueryType,
) -> f32 {
    let raw_cos = cosine_similarity(query_embedding, doc_embedding);
    let base_similarity = (raw_cos + 1.0) / 2.0; // Normalize [-1,1] to [0,1] to match all other embedders

    match query_type {
        CodeQueryType::Code2Code => {
            // For code queries, we want higher precision.
            // Apply a sharpening transformation that:
            // - Boosts similarities above 0.75 (high matches get higher)
            // - Reduces similarities below 0.75
            // This creates a steeper S-curve for better discrimination.
            // Note: 0.75 in [0,1] space corresponds to 0.5 in raw [-1,1] space.
            if base_similarity > 0.75 {
                // Boost high similarities: map [0.75, 1.0] to [0.75, 1.0] with amplification
                let excess = base_similarity - 0.75;
                let boosted = 0.75 + excess * 1.4; // 1.4x amplification
                boosted.min(1.0)
            } else {
                // Below 0.75 in normalized space, apply mild reduction
                base_similarity * 0.9
            }
        }
        CodeQueryType::Text2Code => {
            // For natural language queries about code, use standard similarity.
            // The semantic representation captures the intent well.
            base_similarity
        }
        CodeQueryType::NonCode => {
            // For non-code queries, E7 Code embedder should have reduced influence.
            // Scale down similarity to reduce E7's contribution to final ranking.
            base_similarity * 0.5
        }
    }
}

/// Compute cosine similarity between two dense vectors.
///
/// # Returns
///
/// Similarity score in range [-1.0, 1.0], clamped for numerical stability.
/// Returns 0.0 for empty or mismatched vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;

    for i in 0..a.len() {
        dot += a[i] * b[i];
        norm_a += a[i] * a[i];
        norm_b += b[i] * b[i];
    }

    let norm_a = norm_a.sqrt();
    let norm_b = norm_b.sqrt();

    if norm_a < f32::EPSILON || norm_b < f32::EPSILON {
        return 0.0;
    }

    (dot / (norm_a * norm_b)).clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Code2Code Detection Tests
    // =========================================================================

    #[test]
    fn test_detect_rust_code() {
        // Rust function definitions
        assert_eq!(
            detect_code_query_type("fn process_batch<T>()"),
            CodeQueryType::Code2Code
        );
        assert_eq!(
            detect_code_query_type("pub fn new() -> Self"),
            CodeQueryType::Code2Code
        );
        assert_eq!(
            detect_code_query_type("async fn handle_request()"),
            CodeQueryType::Code2Code
        );
        assert_eq!(
            detect_code_query_type("unsafe fn raw_ptr()"),
            CodeQueryType::Code2Code
        );
        assert_eq!(
            detect_code_query_type("const fn compile_time()"),
            CodeQueryType::Code2Code
        );

        // Rust type definitions
        assert_eq!(
            detect_code_query_type("impl Iterator for Counter"),
            CodeQueryType::Code2Code
        );
        assert_eq!(
            detect_code_query_type("struct Point { x: f32, y: f32 }"),
            CodeQueryType::Code2Code
        );
        assert_eq!(
            detect_code_query_type("enum Option<T> { Some(T), None }"),
            CodeQueryType::Code2Code
        );
        assert_eq!(
            detect_code_query_type("trait Display { fn fmt(&self); }"),
            CodeQueryType::Code2Code
        );

        // Rust-specific syntax
        assert_eq!(
            detect_code_query_type("Vec<String>"),
            CodeQueryType::Code2Code
        );
        assert_eq!(
            detect_code_query_type("Option<&str>"),
            CodeQueryType::Code2Code
        );
        assert_eq!(
            detect_code_query_type("Result<T, E>"),
            CodeQueryType::Code2Code
        );
        assert_eq!(
            detect_code_query_type("std::collections::HashMap"),
            CodeQueryType::Code2Code
        );
        assert_eq!(
            detect_code_query_type("#[derive(Debug)]"),
            CodeQueryType::Code2Code
        );

        println!("[PASS] Rust code patterns detected as Code2Code");
    }

    #[test]
    fn test_detect_python_code() {
        // Python function definitions
        assert_eq!(
            detect_code_query_type("def process_data(items):"),
            CodeQueryType::Code2Code
        );
        assert_eq!(
            detect_code_query_type("async def fetch_data():"),
            CodeQueryType::Code2Code
        );

        // Python class definitions
        assert_eq!(
            detect_code_query_type("class MyClass:"),
            CodeQueryType::Code2Code
        );

        // Python-specific syntax
        assert_eq!(
            detect_code_query_type("import numpy as np"),
            CodeQueryType::Code2Code
        );
        assert_eq!(
            detect_code_query_type("from typing import List"),
            CodeQueryType::Code2Code
        );
        assert_eq!(
            detect_code_query_type("self.value = 42"),
            CodeQueryType::Code2Code
        );
        assert_eq!(
            detect_code_query_type("if __name__ == '__main__':"),
            CodeQueryType::Code2Code
        );
        assert_eq!(
            detect_code_query_type("lambda x: x * 2"),
            CodeQueryType::Code2Code
        );

        println!("[PASS] Python code patterns detected as Code2Code");
    }

    #[test]
    fn test_detect_js_ts_code() {
        // JavaScript/TypeScript function definitions
        assert_eq!(
            detect_code_query_type("function processData(items) {"),
            CodeQueryType::Code2Code
        );
        assert_eq!(
            detect_code_query_type("const handler = async () => {"),
            CodeQueryType::Code2Code
        );
        assert_eq!(
            detect_code_query_type("let count = 0;"),
            CodeQueryType::Code2Code
        );

        // TypeScript-specific
        assert_eq!(
            detect_code_query_type("interface User { name: string; }"),  // Contains "{ " which is a general code indicator
            CodeQueryType::Code2Code
        );
        assert_eq!(
            detect_code_query_type("type Handler<T> = (t: T) => void"),
            CodeQueryType::Code2Code
        );
        assert_eq!(
            detect_code_query_type("export default class"),
            CodeQueryType::Code2Code
        );

        // Module syntax
        assert_eq!(
            detect_code_query_type("import { useState } from 'react'"),
            CodeQueryType::Code2Code
        );
        assert_eq!(
            detect_code_query_type("export const API_URL = 'https://'"),
            CodeQueryType::Code2Code
        );

        println!("[PASS] JavaScript/TypeScript patterns detected as Code2Code");
    }

    #[test]
    fn test_detect_go_code() {
        // Go function definitions
        assert_eq!(
            detect_code_query_type("func (s *Server) Start()"),
            CodeQueryType::Code2Code
        );
        assert_eq!(
            detect_code_query_type("func main() {"),
            CodeQueryType::Code2Code
        );

        // Go-specific syntax
        assert_eq!(
            detect_code_query_type("package main"),
            CodeQueryType::Code2Code
        );
        assert_eq!(
            detect_code_query_type("type User struct {"),
            CodeQueryType::Code2Code
        );
        assert_eq!(
            detect_code_query_type("chan int"),
            CodeQueryType::Code2Code
        );
        assert_eq!(
            detect_code_query_type("go func() { }()"),
            CodeQueryType::Code2Code
        );
        assert_eq!(
            detect_code_query_type("defer file.Close()"),
            CodeQueryType::Code2Code
        );
        assert_eq!(
            detect_code_query_type("map[string]interface{}"),
            CodeQueryType::Code2Code
        );

        println!("[PASS] Go code patterns detected as Code2Code");
    }

    #[test]
    fn test_detect_cpp_code() {
        // C/C++ code patterns
        assert_eq!(
            detect_code_query_type("int main(int argc, char** argv)"),
            CodeQueryType::Code2Code
        );
        assert_eq!(
            detect_code_query_type("#include <iostream>"),
            CodeQueryType::Code2Code
        );
        assert_eq!(
            detect_code_query_type("#define MAX_SIZE 1024"),
            CodeQueryType::Code2Code
        );
        assert_eq!(
            detect_code_query_type("std::vector<int> numbers;"),
            CodeQueryType::Code2Code
        );
        assert_eq!(
            detect_code_query_type("std::unique_ptr<Widget> widget;"),
            CodeQueryType::Code2Code
        );
        assert_eq!(
            detect_code_query_type("template<typename T>"),
            CodeQueryType::Code2Code
        );
        assert_eq!(
            detect_code_query_type("namespace utils {"),
            CodeQueryType::Code2Code
        );
        assert_eq!(
            detect_code_query_type("virtual void draw() override"),
            CodeQueryType::Code2Code
        );

        println!("[PASS] C/C++ code patterns detected as Code2Code");
    }

    #[test]
    fn test_detect_general_code_syntax() {
        // Common syntax patterns
        assert_eq!(
            detect_code_query_type("if (x == y) { }"),
            CodeQueryType::Code2Code
        );
        assert_eq!(
            detect_code_query_type("x != null && y != undefined"),
            CodeQueryType::Code2Code
        );
        assert_eq!(
            detect_code_query_type("a || b && c"),
            CodeQueryType::Code2Code
        );
        assert_eq!(
            detect_code_query_type("items.push();"),
            CodeQueryType::Code2Code
        );
        assert_eq!(
            detect_code_query_type("// comment"),
            CodeQueryType::Code2Code
        );
        assert_eq!(
            detect_code_query_type("/* block comment */"),
            CodeQueryType::Code2Code
        );

        println!("[PASS] General code syntax patterns detected as Code2Code");
    }

    // =========================================================================
    // Text2Code Detection Tests
    // =========================================================================

    #[test]
    fn test_detect_nl_about_code() {
        // Function/method descriptions
        assert_eq!(
            detect_code_query_type("batch processing function"),
            CodeQueryType::Text2Code
        );
        assert_eq!(
            detect_code_query_type("method to handle user authentication"),
            CodeQueryType::Text2Code
        );
        assert_eq!(
            detect_code_query_type("callback for button click"),
            CodeQueryType::Text2Code
        );

        // Type descriptions
        assert_eq!(
            detect_code_query_type("implement the iterator pattern"),
            CodeQueryType::Text2Code
        );
        assert_eq!(
            detect_code_query_type("user data structure"),
            CodeQueryType::Text2Code
        );
        assert_eq!(
            detect_code_query_type("interface for database access"),
            CodeQueryType::Text2Code
        );

        // Error handling descriptions
        assert_eq!(
            detect_code_query_type("error handling for network requests"),
            CodeQueryType::Text2Code
        );
        assert_eq!(
            detect_code_query_type("exception recovery strategy"),
            CodeQueryType::Text2Code
        );

        // Async descriptions
        assert_eq!(
            detect_code_query_type("asynchronous file reading"),
            CodeQueryType::Text2Code
        );
        assert_eq!(
            detect_code_query_type("concurrent task execution"),
            CodeQueryType::Text2Code
        );
        assert_eq!(
            detect_code_query_type("promise chaining"),
            CodeQueryType::Text2Code
        );

        // Data structure descriptions
        assert_eq!(
            detect_code_query_type("hash map implementation"),
            CodeQueryType::Text2Code
        );
        assert_eq!(
            detect_code_query_type("binary tree traversal"),
            CodeQueryType::Text2Code
        );
        assert_eq!(
            detect_code_query_type("stack-based algorithm"),
            CodeQueryType::Text2Code
        );

        // Design pattern descriptions
        assert_eq!(
            detect_code_query_type("factory pattern for object creation"),
            CodeQueryType::Text2Code
        );
        assert_eq!(
            detect_code_query_type("singleton instance"),
            CodeQueryType::Text2Code
        );
        assert_eq!(
            detect_code_query_type("observer pattern implementation"),
            CodeQueryType::Text2Code
        );

        // Action descriptions
        assert_eq!(
            detect_code_query_type("implement file upload"),
            CodeQueryType::Text2Code
        );
        assert_eq!(
            detect_code_query_type("refactor the authentication module"),
            CodeQueryType::Text2Code
        );
        assert_eq!(
            detect_code_query_type("optimize database queries"),
            CodeQueryType::Text2Code
        );

        println!("[PASS] Natural language about code detected as Text2Code");
    }

    // =========================================================================
    // NonCode Detection Tests
    // =========================================================================

    #[test]
    fn test_detect_non_code() {
        // General text
        assert_eq!(
            detect_code_query_type("hello world"),
            CodeQueryType::NonCode
        );
        assert_eq!(
            detect_code_query_type("what is the weather today"),
            CodeQueryType::NonCode
        );
        assert_eq!(
            detect_code_query_type("meeting notes from yesterday"),
            CodeQueryType::NonCode
        );
        assert_eq!(
            detect_code_query_type("project timeline for Q1"),
            CodeQueryType::NonCode
        );

        // Empty/whitespace
        assert_eq!(detect_code_query_type(""), CodeQueryType::NonCode);
        assert_eq!(detect_code_query_type("   "), CodeQueryType::NonCode);
        assert_eq!(detect_code_query_type("\n\t"), CodeQueryType::NonCode);

        // Short generic words
        assert_eq!(detect_code_query_type("test"), CodeQueryType::NonCode);
        assert_eq!(detect_code_query_type("data"), CodeQueryType::NonCode);

        println!("[PASS] Non-code text detected as NonCode");
    }

    // =========================================================================
    // Similarity Computation Tests
    // =========================================================================

    #[test]
    fn test_code2code_similarity_boost() {
        // High similarity should be boosted
        let query = vec![0.7; 1536];
        let doc = vec![0.7; 1536];

        let base = cosine_similarity(&query, &doc);
        let adjusted = compute_e7_similarity_with_query_type(&query, &doc, CodeQueryType::Code2Code);

        // Base should be 1.0 (identical vectors)
        assert!((base - 1.0).abs() < 0.001);
        // Adjusted should also be 1.0 (can't exceed)
        assert!((adjusted - 1.0).abs() < 0.001);

        println!("[PASS] Code2Code similarity at 1.0 stays at 1.0");
    }

    #[test]
    fn test_code2code_similarity_sharpening() {
        // Create vectors with moderate similarity (~0.7 raw -> ~0.85 normalized)
        let query = vec![0.5; 1536];
        let mut doc = vec![0.5; 1536];
        // Diverge some dimensions
        for i in 0..500 {
            doc[i] = 0.3;
        }

        let raw_cos = cosine_similarity(&query, &doc);
        let normalized = (raw_cos + 1.0) / 2.0;
        let adjusted = compute_e7_similarity_with_query_type(&query, &doc, CodeQueryType::Code2Code);

        // If normalized > 0.75, adjusted should be higher than normalized (boosted)
        if normalized > 0.75 {
            assert!(
                adjusted > normalized,
                "Expected {} > {} for Code2Code with normalized > 0.75",
                adjusted,
                normalized
            );
        }

        println!(
            "[PASS] Code2Code sharpening: raw={:.4}, normalized={:.4}, adjusted={:.4}",
            raw_cos, normalized, adjusted
        );
    }

    #[test]
    fn test_code2code_low_similarity_penalty() {
        // Create vectors with low raw similarity (~-0.35 -> ~0.325 normalized)
        let query = vec![1.0; 1536];
        let mut doc = vec![-1.0; 1536];
        // Make some dimensions positive
        for i in 0..500 {
            doc[i] = 1.0;
        }

        let raw_cos = cosine_similarity(&query, &doc);
        let normalized = (raw_cos + 1.0) / 2.0;
        let adjusted = compute_e7_similarity_with_query_type(&query, &doc, CodeQueryType::Code2Code);

        // If normalized < 0.75, adjusted should be lower than normalized (mild reduction * 0.9)
        if normalized < 0.75 && normalized > 0.0 {
            assert!(
                adjusted < normalized,
                "Expected {} < {} for Code2Code with normalized < 0.75",
                adjusted,
                normalized
            );
        }

        println!(
            "[PASS] Code2Code low similarity penalty: raw={:.4}, normalized={:.4}, adjusted={:.4}",
            raw_cos, normalized, adjusted
        );
    }

    #[test]
    fn test_text2code_no_modification() {
        let query = vec![0.5; 1536];
        let doc = vec![0.5; 1536];

        let raw_cos = cosine_similarity(&query, &doc);
        let normalized = (raw_cos + 1.0) / 2.0;
        let adjusted = compute_e7_similarity_with_query_type(&query, &doc, CodeQueryType::Text2Code);

        // Text2Code returns the normalized base_similarity unchanged
        assert!(
            (normalized - adjusted).abs() < f32::EPSILON,
            "Text2Code should return normalized similarity: expected {}, got {}",
            normalized, adjusted
        );

        println!("[PASS] Text2Code similarity unchanged (normalized)");
    }

    #[test]
    fn test_noncode_reduction() {
        let query = vec![0.5; 1536];
        let doc = vec![0.5; 1536];

        let raw_cos = cosine_similarity(&query, &doc);
        let normalized = (raw_cos + 1.0) / 2.0;
        let adjusted = compute_e7_similarity_with_query_type(&query, &doc, CodeQueryType::NonCode);

        // NonCode should reduce normalized similarity by 50%
        let expected = normalized * 0.5;
        assert!(
            (adjusted - expected).abs() < f32::EPSILON,
            "NonCode should reduce by 50%: expected {}, got {}",
            expected,
            adjusted
        );

        println!("[PASS] NonCode similarity reduced by 50% (of normalized)");
    }

    // =========================================================================
    // Edge Cases
    // =========================================================================

    #[test]
    fn test_empty_vectors() {
        let empty: Vec<f32> = vec![];
        let normal = vec![0.5; 1536];

        assert_eq!(cosine_similarity(&empty, &normal), 0.0);
        assert_eq!(cosine_similarity(&normal, &empty), 0.0);
        assert_eq!(cosine_similarity(&empty, &empty), 0.0);

        println!("[PASS] Empty vectors return 0.0 similarity");
    }

    #[test]
    fn test_mismatched_dimensions() {
        let a = vec![0.5; 100];
        let b = vec![0.5; 200];

        assert_eq!(cosine_similarity(&a, &b), 0.0);

        println!("[PASS] Mismatched dimensions return 0.0 similarity");
    }

    #[test]
    fn test_zero_vectors() {
        let zero = vec![0.0; 1536];
        let normal = vec![0.5; 1536];

        assert_eq!(cosine_similarity(&zero, &normal), 0.0);
        assert_eq!(cosine_similarity(&normal, &zero), 0.0);
        assert_eq!(cosine_similarity(&zero, &zero), 0.0);

        println!("[PASS] Zero vectors return 0.0 similarity");
    }

    // =========================================================================
    // Display/Serialization Tests
    // =========================================================================

    #[test]
    fn test_query_type_display() {
        assert_eq!(format!("{}", CodeQueryType::Code2Code), "code2code");
        assert_eq!(format!("{}", CodeQueryType::Text2Code), "text2code");
        assert_eq!(format!("{}", CodeQueryType::NonCode), "non-code");

        println!("[PASS] CodeQueryType display is correct");
    }

    #[test]
    fn test_query_type_serialization() {
        let code2code = CodeQueryType::Code2Code;
        let json = serde_json::to_string(&code2code).unwrap();
        let recovered: CodeQueryType = serde_json::from_str(&json).unwrap();
        assert_eq!(code2code, recovered);

        println!("[PASS] CodeQueryType serialization roundtrip works");
    }

    #[test]
    fn test_is_code_related() {
        assert!(CodeQueryType::Code2Code.is_code_related());
        assert!(CodeQueryType::Text2Code.is_code_related());
        assert!(!CodeQueryType::NonCode.is_code_related());

        println!("[PASS] is_code_related() works correctly");
    }

    #[test]
    fn test_default_query_type() {
        let default: CodeQueryType = Default::default();
        assert_eq!(default, CodeQueryType::NonCode);

        println!("[PASS] Default CodeQueryType is NonCode");
    }
}
