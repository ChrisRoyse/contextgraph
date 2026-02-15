//! E7 code search tool implementation (search_code).
//!
//! # E7 Code Search (V_correctness)
//!
//! This tool leverages E7 dense embeddings (1536D) for code-specific understanding:
//! - E7 finds code patterns, function signatures, implementations
//! - Understands code constructs that E1 misses by treating code as natural language
//! - Blends E7 code scores with E1 semantic for comprehensive results
//!
//! ## Constitution Compliance
//!
//! - ARCH-12: E1 is the semantic foundation, E7 enhances with code understanding
//! - ARCH-13: Uses multi-space search with E1+E7
//! - E7 finds: "Code patterns, function signatures" that E1 misses by "Treating code as NL"
//! - AP-02: All comparisons within respective spaces (no cross-embedder)
//! - FAIL FAST: All errors propagate immediately with logging
//!
//! ## E7-WIRING Enhancement
//!
//! When the code embedding pipeline is available (CodeStore + E7 provider),
//! search_code can also search directly against code entities stored via
//! the AST chunker. This provides more accurate results for code-specific queries.

use tracing::{debug, error, info};
use uuid::Uuid;

use context_graph_core::traits::{SearchStrategy, TeleologicalSearchOptions};

use crate::protocol::JsonRpcId;
use crate::protocol::JsonRpcResponse;

use super::code_dtos::{
    CodeSearchMetadata, CodeSearchMode, CodeSearchResult, CodeSourceInfo, DetectedLanguageInfo,
    SearchCodeRequest, SearchCodeResponse, CodeEntityResult,
};

use super::helpers::ToolErrorKind;
use super::super::Handlers;

impl Handlers {
    /// search_code tool implementation.
    ///
    /// Finds memories containing code patterns using E7 dense embeddings.
    /// ENHANCES E1 semantic search with code-specific understanding.
    ///
    /// # Algorithm
    ///
    /// 1. Embed the code query using all 13 embedders
    /// 2. Detect programming language indicators in query
    /// 3. Search using E1 semantic as primary (over-fetch for blending)
    /// 4. Compute E7 code similarity for each candidate
    /// 5. Blend E1 and E7 scores: (1-blend)*E1 + blend*E7
    /// 6. Filter by minScore and return top-K results
    ///
    /// # Parameters
    ///
    /// - `query`: The code query to search for (required)
    /// - `topK`: Maximum results to return (1-50, default: 10)
    /// - `minScore`: Minimum blended score threshold (0-1, default: 0.2)
    /// - `blendWithSemantic`: E7 weight in blend (0-1, default: 0.4)
    /// - `includeContent`: Include full content text (default: false)
    pub(crate) async fn call_search_code(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        // Parse and validate request
        let request: SearchCodeRequest = match self.parse_request(id.clone(), args, "search_code") {
            Ok(req) => req,
            Err(resp) => return resp,
        };

        let query = &request.query;
        let top_k = request.top_k;
        let min_score = request.min_score;
        let e7_blend = request.blend_with_semantic;
        let e1_weight = 1.0 - e7_blend;
        let search_mode = request.search_mode;
        let language_hint = request.language_hint.clone();

        // Detect programming language from query (merge with hint if provided)
        let mut detected_language = detect_language(query);
        if let Some(ref hint) = language_hint {
            // Language hint overrides detection if provided
            detected_language.primary_language = Some(hint.clone());
            detected_language.confidence = 1.0;
        }

        // Parse strategy from request - Pipeline enables E13 recall + E12 reranking
        let strategy = request.parse_strategy();
        let enable_rerank = matches!(strategy, SearchStrategy::Pipeline);

        info!(
            query_preview = %query.chars().take(50).collect::<String>(),
            top_k = top_k,
            min_score = min_score,
            search_mode = %search_mode,
            strategy = ?strategy,
            enable_rerank = enable_rerank,
            e7_blend = e7_blend,
            detected_language = ?detected_language.primary_language,
            "search_code: Starting code search"
        );

        // Step 1: Embed the code query
        let query_embedding = match self.embed_query(id.clone(), query, "search_code").await {
            Ok(fp) => fp,
            Err(resp) => return resp,
        };

        // Step 2: Search for candidates (3x over-fetch for blended reranking)
        let fetch_multiplier = 3;
        let fetch_top_k = top_k * fetch_multiplier;

        // Use code_search weight profile with E7 emphasis
        let options = TeleologicalSearchOptions::quick(fetch_top_k)
            .with_strategy(strategy)
            .with_weight_profile("code_search") // E7 emphasis
            .with_min_similarity(0.0) // Get all candidates, filter later
            .with_rerank(enable_rerank); // Auto-enable E12 for pipeline

        let candidates = match self
            .teleological_store
            .search_semantic(&query_embedding, options)
            .await
        {
            Ok(results) => results,
            Err(e) => {
                error!(error = %e, "search_code: Candidate search FAILED");
                return self.tool_error(id, &format!("Search failed: {}", e));
            }
        };

        let candidates_evaluated = candidates.len();
        debug!(
            candidates_evaluated = candidates_evaluated,
            "search_code: Evaluating candidates for code scoring"
        );

        // Step 3: Compute code-enhanced scores based on search mode
        let query_e7 = &query_embedding.e7_code;
        let query_e1 = &query_embedding.e1_semantic;

        let mut scored_results: Vec<(Uuid, f32, f32, f32)> = Vec::with_capacity(candidates.len());

        for candidate in &candidates {
            let cand_id = candidate.fingerprint.id;
            let cand_e1 = &candidate.fingerprint.semantic.e1_semantic;
            let cand_e7 = &candidate.fingerprint.semantic.e7_code;

            // E1 cosine similarity (THE semantic foundation per ARCH-12)
            let e1_sim = cosine_similarity(query_e1, cand_e1);

            // E7 cosine similarity (code-specific understanding)
            let e7_sim = cosine_similarity(query_e7, cand_e7);

            // Compute final score based on search mode
            // All modes produce scores in [0, 1] range
            let final_score = match search_mode {
                CodeSearchMode::Hybrid => {
                    // Blend: (1-blend)*E1 + blend*E7
                    e1_weight * e1_sim + e7_blend * e7_sim
                }
                CodeSearchMode::E7Only => {
                    // Pure E7 code search (ignores E1)
                    e7_sim
                }
                CodeSearchMode::E1WithE7Rerank => {
                    // E1 primary (90%) with E7 tiebreaker (10%)
                    // Per ARCH-12: E1 is foundation, E7 enhances
                    0.9 * e1_sim + 0.1 * e7_sim
                }
                CodeSearchMode::Pipeline => {
                    // Same as Hybrid - uses user's blend weight
                    e1_weight * e1_sim + e7_blend * e7_sim
                }
            };

            if final_score >= min_score {
                scored_results.push((cand_id, final_score, e1_sim, e7_sim));
            }
        }

        // Step 4: Sort by blended score and take top-K
        scored_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored_results.truncate(top_k);

        let filtered_count = candidates_evaluated - scored_results.len();

        // Step 5: Build results with optional content
        let result_ids: Vec<Uuid> = scored_results.iter().map(|r| r.0).collect();

        // Get content if requested - FAIL FAST on error
        let contents: Vec<Option<String>> = if request.include_content && !result_ids.is_empty() {
            match self.teleological_store.get_content_batch(&result_ids).await {
                Ok(c) => c,
                Err(e) => {
                    error!(
                        error = %e,
                        result_count = result_ids.len(),
                        "search_code: Content retrieval FAILED"
                    );
                    return self.tool_error(
                        id,
                        &format!(
                            "Failed to retrieve content for {} results: {}",
                            result_ids.len(),
                            e
                        ),
                    );
                }
            }
        } else {
            vec![None; result_ids.len()]
        };

        // Get source metadata - FAIL FAST on error
        let source_metadata = match self
            .teleological_store
            .get_source_metadata_batch(&result_ids)
            .await
        {
            Ok(m) => m,
            Err(e) => {
                error!(
                    error = %e,
                    result_count = result_ids.len(),
                    "search_code: Source metadata retrieval FAILED"
                );
                return self.tool_error(
                    id,
                    &format!(
                        "Failed to retrieve source metadata for {} results: {}",
                        result_ids.len(),
                        e
                    ),
                );
            }
        };

        // Build response
        let results: Vec<CodeSearchResult> = scored_results
            .into_iter()
            .enumerate()
            .map(|(i, (memory_id, score, e1_sim, e7_sim))| {
                let source = source_metadata.get(i).and_then(|m| {
                    m.as_ref().map(|meta| CodeSourceInfo {
                        source_type: format!("{}", meta.source_type),
                        file_path: meta.file_path.clone(),
                        hook_type: meta.hook_type.clone(),
                        tool_name: meta.tool_name.clone(),
                    })
                });

                CodeSearchResult {
                    memory_id,
                    score,
                    e1_similarity: e1_sim,
                    e7_code_score: e7_sim,
                    content: contents.get(i).and_then(|c| c.clone()),
                    source,
                }
            })
            .collect();

        // E7-WIRING: Optionally search CodeStore for code entities
        // ERR-11 FIX: Propagate CodeStore errors to caller instead of swallowing
        let code_entities = if self.has_code_pipeline() {
            match self
                .search_code_entities(query, top_k, min_score, request.include_content)
                .await
            {
                Ok(entities) => Some(entities),
                Err(e) => {
                    error!(error = %e, "search_code: CodeStore entity search failed");
                    return self.tool_error(id, &format!("CodeStore entity search failed: {}", e));
                }
            }
        } else {
            None
        };

        let result_count = results.len();
        let response = SearchCodeResponse {
            query: query.clone(),
            results,
            count: result_count,
            metadata: CodeSearchMetadata {
                candidates_evaluated,
                filtered_by_score: filtered_count,
                search_mode,
                e7_blend_weight: e7_blend,
                e1_weight,
                language_hint,
                detected_language,
            },
            code_entities,
        };

        info!(
            results_found = response.count,
            candidates_evaluated = candidates_evaluated,
            filtered = filtered_count,
            has_code_entities = response.code_entities.is_some(),
            "search_code: Completed code-enhanced search"
        );

        match serde_json::to_value(&response) {
            Ok(v) => self.tool_result(id, v),
            Err(e) => {
                error!(error = %e, "search_code: Response serialization failed");
                self.tool_error_typed(id, ToolErrorKind::Execution, &format!("Response serialization failed: {}", e))
            }
        }
    }

    /// Search CodeStore for code entities using full 13-embedder fingerprint.
    ///
    /// E7-WIRING: Direct search against code entities stored via AST chunker.
    /// Uses E7 (code) as primary embedding for ranking per constitution.
    ///
    /// # Arguments
    /// * `query` - Code query string
    /// * `top_k` - Maximum results
    /// * `min_score` - Minimum similarity threshold
    /// * `include_content` - Whether to include code content
    ///
    /// # Returns
    /// Vector of CodeEntityResult, or error if code pipeline unavailable.
    ///
    /// # Constitution Compliance
    /// - ARCH-01: Uses full SemanticFingerprint (all 13 embeddings)
    /// - E7 is primary for code search (use_e7_primary=true)
    async fn search_code_entities(
        &self,
        query: &str,
        top_k: usize,
        min_score: f32,
        include_content: bool,
    ) -> Result<Vec<CodeEntityResult>, String> {
        // Get code embedding provider
        let provider = self
            .code_embedding_provider()
            .ok_or_else(|| "Code embedding provider not available".to_string())?;

        // Get code store
        let store = self
            .code_store()
            .ok_or_else(|| "Code store not available".to_string())?;

        // Generate full fingerprint for query (all 13 embeddings)
        let query_fingerprint = provider
            .embed_code(query, None)
            .await
            .map_err(|e| format!("Failed to embed query: {}", e))?;

        // Search code store by fingerprint similarity
        // use_e7_primary=true: E7 (code) embedding used for ranking
        let results = store
            .search_by_fingerprint(&query_fingerprint, top_k, min_score, true)
            .await
            .map_err(|e| format!("Failed to search code store: {}", e))?;

        debug!(
            query_len = query.len(),
            e7_dim = query_fingerprint.e7_code.len(),
            results_found = results.len(),
            include_content = include_content,
            "search_code_entities: E7-primary code search completed"
        );

        // Convert to CodeEntityResult
        let entity_results: Vec<CodeEntityResult> = results
            .into_iter()
            .map(|(entity, score)| CodeEntityResult {
                id: entity.id.to_string(),
                name: entity.name,
                entity_type: format!("{}", entity.entity_type),
                score,
                file_path: entity.file_path,
                start_line: Some(entity.line_start),
                end_line: Some(entity.line_end),
                content: if include_content {
                    Some(entity.code)
                } else {
                    None
                },
                // Module path as scope chain
                scope_chain: entity.module_path.map(|p| vec![p]),
            })
            .collect();

        Ok(entity_results)
    }
}

/// Compute cosine similarity between two dense vectors.
///
/// Returns 0.0 if either vector is empty or has zero norm.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a < f32::EPSILON || norm_b < f32::EPSILON {
        return 0.0;
    }

    (dot / (norm_a * norm_b)).clamp(-1.0, 1.0)
}

/// Language detection patterns: (language_name, indicator_patterns)
const LANGUAGE_PATTERNS: &[(&str, &[&str])] = &[
    ("rust", &["impl ", "fn ", "let mut", "struct ", "enum ", "pub fn", "trait ", "cargo", ".rs", "unwrap(", "match "]),
    ("python", &["def ", "import ", "self.", "__init__", "python", ".py", "pip ", "async def", "await ", "pytest"]),
    ("javascript", &["function ", "const ", "let ", "var ", "=>", "async ", "await ", ".js", "npm ", "node ", "react", "vue", "angular"]),
    ("typescript", &["interface ", ": string", ": number", ": boolean", ".ts", "type ", "<T>"]),
    ("go", &["func ", "package ", "import ", "go ", "goroutine", "chan ", "defer ", ".go", "golang"]),
    ("java", &["public class", "private ", "protected ", "static void", "extends ", "implements ", ".java", "maven", "gradle", "@Override"]),
    ("cpp", &["#include", "int main", "void ", "std::", "cout <<", "cin >>", ".cpp", ".hpp", ".h", "template<"]),
    ("sql", &["select ", "from ", "where ", "join ", "insert ", "update ", "delete ", "create table", "alter ", "sql"]),
];

/// Detect programming language indicators in query.
///
/// Returns language info with confidence and indicators.
fn detect_language(query: &str) -> DetectedLanguageInfo {
    let query_lower = query.to_lowercase();

    LANGUAGE_PATTERNS
        .iter()
        .filter_map(|(lang, patterns)| {
            let matched: Vec<String> = patterns
                .iter()
                .filter(|p| query_lower.contains(*p))
                .map(|p| p.trim().to_string())
                .collect();

            if matched.is_empty() {
                None
            } else {
                let confidence = (matched.len() as f32 / 3.0).min(1.0);
                Some((lang, matched, confidence))
            }
        })
        .max_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(lang, indicators, confidence)| DetectedLanguageInfo {
            primary_language: Some(lang.to_string()),
            confidence,
            indicators,
        })
        .unwrap_or(DetectedLanguageInfo {
            primary_language: None,
            confidence: 0.0,
            indicators: Vec::new(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 0.001);
    }

    #[test]
    fn test_detect_language_rust() {
        let query = "impl async fn handler with Result<T, Error>";
        let info = detect_language(query);
        assert_eq!(info.primary_language, Some("rust".to_string()));
        assert!(info.confidence > 0.0);
        assert!(!info.indicators.is_empty());
    }

    #[test]
    fn test_detect_language_python() {
        let query = "def process_data(self) with async def and await";
        let info = detect_language(query);
        assert_eq!(info.primary_language, Some("python".to_string()));
        assert!(info.confidence > 0.0);
    }

    #[test]
    fn test_detect_language_javascript() {
        let query = "const handler = async () => { await fetch() }";
        let info = detect_language(query);
        assert_eq!(info.primary_language, Some("javascript".to_string()));
    }

    #[test]
    fn test_detect_language_no_match() {
        let query = "hello world general text";
        let info = detect_language(query);
        assert!(info.primary_language.is_none());
        assert!((info.confidence - 0.0).abs() < 0.001);
        assert!(info.indicators.is_empty());
    }

    #[test]
    fn test_detect_language_sql() {
        let query = "SELECT * FROM users WHERE id = 1 JOIN orders";
        let info = detect_language(query);
        assert_eq!(info.primary_language, Some("sql".to_string()));
        assert!(info.confidence >= 0.66); // At least 2 indicators
    }

    #[test]
    fn test_detect_language_go() {
        let query = "func main() package main with goroutine";
        let info = detect_language(query);
        assert_eq!(info.primary_language, Some("go".to_string()));
    }
}
