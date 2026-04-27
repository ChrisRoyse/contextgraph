//! Token-level IoU (Intersection over Union) metrics for E7 code retrieval.
//!
//! IoU measures how well retrieved code chunks overlap with ground truth code.
//! This is critical for code search where partial matches are valuable.
//!
//! # Formula
//! IoU = |A ∩ B| / |A ∪ B|
//!
//! Where:
//! - A = tokens in retrieved chunk(s)
//! - B = tokens in ground truth code
//!
//! # Usage
//! - IoU > 0.7: Excellent match (most of ground truth covered)
//! - IoU > 0.5: Good match (majority overlap)
//! - IoU > 0.3: Partial match (some overlap)
//! - IoU < 0.3: Poor match (minimal overlap)

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Token type for IoU computation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenType {
    /// Code identifier (variable, function, type name).
    Identifier,
    /// Keyword (fn, let, pub, impl, etc.).
    Keyword,
    /// Literal (string, number, etc.).
    Literal,
    /// Operator (+, -, *, /, etc.).
    Operator,
    /// Punctuation ({, }, (, ), etc.).
    Punctuation,
    /// Whitespace or comment.
    Whitespace,
}

/// A token extracted from code.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CodeToken {
    /// The token text.
    pub text: String,
    /// Token type.
    pub token_type: TokenType,
}

impl CodeToken {
    /// Create a new token.
    pub fn new(text: impl Into<String>, token_type: TokenType) -> Self {
        Self {
            text: text.into(),
            token_type,
        }
    }

    /// Create an identifier token.
    pub fn identifier(text: impl Into<String>) -> Self {
        Self::new(text, TokenType::Identifier)
    }

    /// Create a keyword token.
    pub fn keyword(text: impl Into<String>) -> Self {
        Self::new(text, TokenType::Keyword)
    }
}

/// IoU result for a single query.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IoUResult {
    /// Overall IoU score (token-level).
    pub token_iou: f64,
    /// IoU computed on identifiers only.
    pub identifier_iou: f64,
    /// IoU computed on keywords only.
    pub keyword_iou: f64,
    /// Number of tokens in retrieved content.
    pub retrieved_token_count: usize,
    /// Number of tokens in ground truth.
    pub ground_truth_token_count: usize,
    /// Number of overlapping tokens.
    pub intersection_count: usize,
    /// Number of tokens in union.
    pub union_count: usize,
}

/// Aggregated IoU metrics across multiple queries.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IoUMetrics {
    /// Number of queries.
    pub query_count: usize,
    /// Mean token-level IoU.
    pub mean_token_iou: f64,
    /// Mean identifier IoU.
    pub mean_identifier_iou: f64,
    /// Mean keyword IoU.
    pub mean_keyword_iou: f64,
    /// Median IoU.
    pub median_iou: f64,
    /// Standard deviation of IoU.
    pub std_iou: f64,
    /// IoU@K metrics.
    pub iou_at: std::collections::HashMap<usize, f64>,
}

impl IoUMetrics {
    /// Create metrics from individual results.
    pub fn from_results(results: &[IoUResult]) -> Self {
        if results.is_empty() {
            return Self::default();
        }

        let query_count = results.len();
        let n = query_count as f64;

        let mean_token_iou = results.iter().map(|r| r.token_iou).sum::<f64>() / n;
        let mean_identifier_iou = results.iter().map(|r| r.identifier_iou).sum::<f64>() / n;
        let mean_keyword_iou = results.iter().map(|r| r.keyword_iou).sum::<f64>() / n;

        // Compute median
        let mut sorted_ious: Vec<f64> = results.iter().map(|r| r.token_iou).collect();
        sorted_ious.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median_iou = sorted_ious[query_count / 2];

        // Compute standard deviation
        let variance = results
            .iter()
            .map(|r| (r.token_iou - mean_token_iou).powi(2))
            .sum::<f64>()
            / n;
        let std_iou = variance.sqrt();

        Self {
            query_count,
            mean_token_iou,
            mean_identifier_iou,
            mean_keyword_iou,
            median_iou,
            std_iou,
            iou_at: std::collections::HashMap::new(),
        }
    }

    /// Check if IoU meets quality threshold.
    pub fn meets_threshold(&self, min_iou: f64) -> bool {
        self.mean_token_iou >= min_iou
    }
}

// ===========================================================================
// Tokenization
// ===========================================================================

/// Rust keywords for token classification.
const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type",
    "unsafe", "use", "where", "while", "abstract", "become", "box", "do", "final", "macro",
    "override", "priv", "typeof", "unsized", "virtual", "yield", "try",
];

/// Tokenize code into code tokens.
///
/// Uses a simple tokenizer suitable for Rust code.
/// For production use, tree-sitter tokenization is preferred.
pub fn tokenize_code(code: &str) -> Vec<CodeToken> {
    let keywords: HashSet<&str> = RUST_KEYWORDS.iter().copied().collect();
    let mut tokens = Vec::new();
    let mut current = String::new();

    let mut chars = code.chars().peekable();

    while let Some(c) = chars.next() {
        if c.is_alphanumeric() || c == '_' {
            // Accumulate identifier/keyword
            current.push(c);
        } else {
            // Flush current token
            if !current.is_empty() {
                let token_type = if keywords.contains(current.as_str()) {
                    TokenType::Keyword
                } else if current
                    .chars()
                    .next()
                    .map(|c| c.is_ascii_digit())
                    .unwrap_or(false)
                {
                    TokenType::Literal
                } else {
                    TokenType::Identifier
                };
                tokens.push(CodeToken::new(std::mem::take(&mut current), token_type));
            }

            // Handle the non-alphanumeric character
            if c.is_whitespace() {
                // Skip whitespace
                continue;
            } else if c == '"' || c == '\'' {
                // String/char literal
                let quote = c;
                let mut literal = String::new();
                literal.push(c);
                while let Some(&next) = chars.peek() {
                    literal.push(chars.next().unwrap());
                    if next == quote && !literal.ends_with("\\\"") && !literal.ends_with("\\'") {
                        break;
                    }
                }
                tokens.push(CodeToken::new(literal, TokenType::Literal));
            } else if "+-*/%=<>!&|^~".contains(c) {
                tokens.push(CodeToken::new(c.to_string(), TokenType::Operator));
            } else if "(){}[]<>;:,.?@#$".contains(c) {
                tokens.push(CodeToken::new(c.to_string(), TokenType::Punctuation));
            }
        }
    }

    // Flush final token
    if !current.is_empty() {
        let token_type = if keywords.contains(current.as_str()) {
            TokenType::Keyword
        } else if current
            .chars()
            .next()
            .map(|c| c.is_ascii_digit())
            .unwrap_or(false)
        {
            TokenType::Literal
        } else {
            TokenType::Identifier
        };
        tokens.push(CodeToken::new(current, token_type));
    }

    tokens
}

/// Extract just the token strings (for simpler IoU computation).
pub fn extract_token_strings(code: &str) -> HashSet<String> {
    tokenize_code(code).into_iter().map(|t| t.text).collect()
}

// ===========================================================================
// IoU Computation
// ===========================================================================

/// Compute token-level IoU between retrieved and ground truth code.
///
/// # Arguments
/// * `retrieved` - Retrieved code chunk(s)
/// * `ground_truth` - Ground truth code
///
/// # Returns
/// IoU score in [0, 1]
pub fn compute_token_iou(retrieved: &str, ground_truth: &str) -> f64 {
    let retrieved_tokens = extract_token_strings(retrieved);
    let ground_truth_tokens = extract_token_strings(ground_truth);

    if retrieved_tokens.is_empty() && ground_truth_tokens.is_empty() {
        return 1.0; // Both empty = perfect match
    }

    if retrieved_tokens.is_empty() || ground_truth_tokens.is_empty() {
        return 0.0; // One empty = no match
    }

    let intersection: HashSet<_> = retrieved_tokens
        .intersection(&ground_truth_tokens)
        .collect();
    let union: HashSet<_> = retrieved_tokens.union(&ground_truth_tokens).collect();

    intersection.len() as f64 / union.len() as f64
}

/// Compute IoU result with detailed breakdown.
pub fn compute_iou_result(retrieved: &str, ground_truth: &str) -> IoUResult {
    let retrieved_tokens = tokenize_code(retrieved);
    let ground_truth_tokens = tokenize_code(ground_truth);

    let retrieved_set: HashSet<_> = retrieved_tokens.iter().map(|t| &t.text).collect();
    let ground_truth_set: HashSet<_> = ground_truth_tokens.iter().map(|t| &t.text).collect();

    let intersection_count = retrieved_set.intersection(&ground_truth_set).count();
    let union_count = retrieved_set.union(&ground_truth_set).count();

    let token_iou = if union_count == 0 {
        0.0
    } else {
        intersection_count as f64 / union_count as f64
    };

    // Identifier-only IoU
    let retrieved_identifiers: HashSet<_> = retrieved_tokens
        .iter()
        .filter(|t| t.token_type == TokenType::Identifier)
        .map(|t| &t.text)
        .collect();
    let ground_truth_identifiers: HashSet<_> = ground_truth_tokens
        .iter()
        .filter(|t| t.token_type == TokenType::Identifier)
        .map(|t| &t.text)
        .collect();
    let id_intersection = retrieved_identifiers
        .intersection(&ground_truth_identifiers)
        .count();
    let id_union = retrieved_identifiers
        .union(&ground_truth_identifiers)
        .count();
    let identifier_iou = if id_union == 0 {
        0.0
    } else {
        id_intersection as f64 / id_union as f64
    };

    // Keyword-only IoU
    let retrieved_keywords: HashSet<_> = retrieved_tokens
        .iter()
        .filter(|t| t.token_type == TokenType::Keyword)
        .map(|t| &t.text)
        .collect();
    let ground_truth_keywords: HashSet<_> = ground_truth_tokens
        .iter()
        .filter(|t| t.token_type == TokenType::Keyword)
        .map(|t| &t.text)
        .collect();
    let kw_intersection = retrieved_keywords
        .intersection(&ground_truth_keywords)
        .count();
    let kw_union = retrieved_keywords.union(&ground_truth_keywords).count();
    let keyword_iou = if kw_union == 0 {
        0.0
    } else {
        kw_intersection as f64 / kw_union as f64
    };

    IoUResult {
        token_iou,
        identifier_iou,
        keyword_iou,
        retrieved_token_count: retrieved_tokens.len(),
        ground_truth_token_count: ground_truth_tokens.len(),
        intersection_count,
        union_count,
    }
}

/// Compute IoU@K: IoU of top-K retrieved chunks combined.
///
/// # Arguments
/// * `retrieved_chunks` - Retrieved code chunks in ranked order
/// * `ground_truth` - Ground truth code
/// * `k` - Number of top chunks to consider
pub fn compute_iou_at_k(retrieved_chunks: &[&str], ground_truth: &str, k: usize) -> f64 {
    if retrieved_chunks.is_empty() || k == 0 {
        return 0.0;
    }

    // Combine top-K chunks
    let combined: String = retrieved_chunks
        .iter()
        .take(k)
        .copied()
        .collect::<Vec<_>>()
        .join("\n");

    compute_token_iou(&combined, ground_truth)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_simple_code() {
        let code = "fn main() { let x = 42; }";
        let tokens = tokenize_code(code);

        println!("Tokens: {:?}", tokens);

        // Should have: fn, main, (, ), {, let, x, =, 42, ;, }
        assert!(tokens
            .iter()
            .any(|t| t.text == "fn" && t.token_type == TokenType::Keyword));
        assert!(tokens
            .iter()
            .any(|t| t.text == "main" && t.token_type == TokenType::Identifier));
        assert!(tokens
            .iter()
            .any(|t| t.text == "let" && t.token_type == TokenType::Keyword));
        assert!(tokens
            .iter()
            .any(|t| t.text == "x" && t.token_type == TokenType::Identifier));
        assert!(tokens
            .iter()
            .any(|t| t.text == "42" && t.token_type == TokenType::Literal));
    }

    #[test]
    fn test_token_iou_identical() {
        let code = "fn foo() { let x = 1; }";
        let iou = compute_token_iou(code, code);
        assert!(
            (iou - 1.0).abs() < 0.001,
            "Identical code should have IoU = 1.0"
        );
    }

    #[test]
    fn test_token_iou_no_overlap() {
        let code1 = "fn foo() {}";
        let code2 = "struct Bar { x: i32 }";
        let iou = compute_token_iou(code1, code2);

        // There should be minimal overlap (maybe just punctuation)
        println!("IoU for non-overlapping code: {}", iou);
        assert!(iou < 0.5, "Different code should have low IoU");
    }

    #[test]
    fn test_token_iou_partial_overlap() {
        let code1 = "fn foo(x: i32) -> i32 { x + 1 }";
        let code2 = "fn foo(y: i32) -> i32 { y * 2 }";
        let iou = compute_token_iou(code1, code2);

        // Should have some overlap (fn, foo, i32, etc.)
        println!("IoU for similar code: {}", iou);
        assert!(iou > 0.3, "Similar code should have moderate IoU");
        assert!(iou < 1.0, "Different code should not have perfect IoU");
    }

    #[test]
    fn test_iou_result_breakdown() {
        let retrieved = "fn calculate(a: i32, b: i32) -> i32 { a + b }";
        let ground_truth = "fn calculate(x: i32, y: i32) -> i32 { x + y }";

        let result = compute_iou_result(retrieved, ground_truth);

        println!("IoU Result: {:?}", result);

        assert!(result.token_iou > 0.5, "Should have good token overlap");
        assert!(
            result.keyword_iou > 0.8,
            "Should have high keyword overlap (fn, i32)"
        );
        assert!(result.intersection_count > 0);
        assert!(result.union_count > result.intersection_count);
    }

    #[test]
    fn test_iou_at_k() {
        let chunks = ["fn a() {}", "fn b() {}", "fn c() {}"];
        let ground_truth = "fn a() {} fn b() {}";

        let iou_1 = compute_iou_at_k(chunks.as_ref(), ground_truth, 1);
        let iou_2 = compute_iou_at_k(chunks.as_ref(), ground_truth, 2);
        let iou_3 = compute_iou_at_k(chunks.as_ref(), ground_truth, 3);

        println!("IoU@1: {}, IoU@2: {}, IoU@3: {}", iou_1, iou_2, iou_3);

        // IoU should increase as we include more chunks that match
        assert!(iou_2 >= iou_1, "IoU@2 should be >= IoU@1");
    }

    #[test]
    fn test_iou_metrics_aggregation() {
        let results = vec![
            IoUResult {
                token_iou: 0.8,
                identifier_iou: 0.9,
                keyword_iou: 0.95,
                retrieved_token_count: 20,
                ground_truth_token_count: 25,
                intersection_count: 18,
                union_count: 27,
            },
            IoUResult {
                token_iou: 0.6,
                identifier_iou: 0.7,
                keyword_iou: 0.8,
                retrieved_token_count: 15,
                ground_truth_token_count: 20,
                intersection_count: 12,
                union_count: 23,
            },
        ];

        let metrics = IoUMetrics::from_results(&results);

        assert_eq!(metrics.query_count, 2);
        assert!((metrics.mean_token_iou - 0.7).abs() < 0.001);
        assert!((metrics.mean_identifier_iou - 0.8).abs() < 0.001);
        assert!(metrics.meets_threshold(0.5));
        assert!(!metrics.meets_threshold(0.9));
    }

    #[test]
    fn test_empty_code_handling() {
        assert_eq!(compute_token_iou("", ""), 1.0);
        assert_eq!(compute_token_iou("fn foo() {}", ""), 0.0);
        assert_eq!(compute_token_iou("", "fn foo() {}"), 0.0);
    }

    #[test]
    fn test_rust_keyword_detection() {
        let code = "pub fn async_handler() -> impl Future { async { } }";
        let tokens = tokenize_code(code);

        let keywords: Vec<_> = tokens
            .iter()
            .filter(|t| t.token_type == TokenType::Keyword)
            .map(|t| t.text.as_str())
            .collect();

        println!("Keywords found: {:?}", keywords);

        assert!(keywords.contains(&"pub"));
        assert!(keywords.contains(&"fn"));
        assert!(keywords.contains(&"async"));
        assert!(keywords.contains(&"impl"));
    }
}
