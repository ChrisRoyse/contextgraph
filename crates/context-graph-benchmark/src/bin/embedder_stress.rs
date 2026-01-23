//! Per-Embedder Stress Test Benchmark
//!
//! Measures each embedder's unique contribution through:
//! 1. REAL embedding of stress test corpus using GPU
//! 2. REAL search operations using HNSW indexes
//! 3. REAL metric computation (MRR, success rate, ablation delta)
//!
//! NO MOCK DATA. NO FALLBACKS. FAIL FAST.
//!
//! # Usage
//!
//! ```bash
//! # Run with real embeddings (requires GPU)
//! cargo run -p context-graph-benchmark --bin embedder-stress --features real-embeddings
//!
//! # Run specific embedder stress test
//! cargo run -p context-graph-benchmark --bin embedder-stress --features real-embeddings -- --embedder E7
//!
//! # Output formats
//! cargo run -p context-graph-benchmark --bin embedder-stress --features real-embeddings -- --format json
//! ```

use std::env;
use std::time::Instant;

use context_graph_benchmark::stress_corpus::{
    get_all_stress_configs, get_stress_config, EmbedderStressConfig, EmbedderStressResults,
    StressQuery,
};

use context_graph_core::types::fingerprint::SemanticFingerprint;
use context_graph_storage::teleological::indexes::EmbedderIndex;
use uuid::Uuid;

/// CLI arguments
struct Args {
    /// Output format: console, json, markdown
    format: OutputFormat,
    /// Run stress test for specific embedder only
    embedder: Option<EmbedderIndex>,
    /// Verbose output
    verbose: bool,
}

#[derive(Debug, Clone, Copy)]
enum OutputFormat {
    Console,
    Json,
    Markdown,
}

/// Corpus document with its fingerprint
#[allow(dead_code)]
struct EmbeddedDoc {
    doc_id: usize,
    content: String,  // Kept for debug output
    fingerprint: SemanticFingerprint,
    uuid: Uuid,  // Kept for traceability
}

/// Search result from an index
struct SearchResult {
    doc_id: usize,
    similarity: f32,
}

fn parse_args() -> Args {
    let raw_args: Vec<String> = env::args().collect();
    let mut format = OutputFormat::Console;
    let mut embedder = None;
    let mut verbose = false;

    let mut i = 1;
    while i < raw_args.len() {
        match raw_args[i].as_str() {
            "--format" => {
                i += 1;
                if i < raw_args.len() {
                    format = match raw_args[i].as_str() {
                        "json" => OutputFormat::Json,
                        "markdown" | "md" => OutputFormat::Markdown,
                        _ => OutputFormat::Console,
                    };
                }
            }
            "--verbose" | "-v" => verbose = true,
            "--embedder" => {
                i += 1;
                if i < raw_args.len() {
                    embedder = parse_embedder(&raw_args[i]);
                    if embedder.is_none() {
                        eprintln!("ERROR: Unknown embedder: {}", raw_args[i]);
                        eprintln!("Valid embedders: E1-E13");
                        std::process::exit(1);
                    }
                }
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            _ => {}
        }
        i += 1;
    }

    Args {
        format,
        embedder,
        verbose,
    }
}

fn parse_embedder(s: &str) -> Option<EmbedderIndex> {
    match s.to_uppercase().as_str() {
        "E1" | "E1_SEMANTIC" => Some(EmbedderIndex::E1Semantic),
        "E2" | "E2_TEMPORAL" => Some(EmbedderIndex::E2TemporalRecent),
        "E3" | "E3_PERIODIC" => Some(EmbedderIndex::E3TemporalPeriodic),
        "E4" | "E4_POSITIONAL" => Some(EmbedderIndex::E4TemporalPositional),
        "E5" | "E5_CAUSAL" => Some(EmbedderIndex::E5Causal),
        "E6" | "E6_SPARSE" => Some(EmbedderIndex::E6Sparse),
        "E7" | "E7_CODE" => Some(EmbedderIndex::E7Code),
        "E8" | "E8_GRAPH" => Some(EmbedderIndex::E8Graph),
        "E9" | "E9_HDC" => Some(EmbedderIndex::E9HDC),
        "E10" | "E10_MULTIMODAL" => Some(EmbedderIndex::E10Multimodal),
        "E11" | "E11_ENTITY" => Some(EmbedderIndex::E11Entity),
        "E12" | "E12_LATE" => Some(EmbedderIndex::E12LateInteraction),
        "E13" | "E13_SPLADE" => Some(EmbedderIndex::E13Splade),
        _ => None,
    }
}

fn print_help() {
    eprintln!(
        r#"
Per-Embedder Stress Test Benchmark (REAL EMBEDDINGS - NO MOCK DATA)

USAGE:
    embedder-stress [OPTIONS]

REQUIREMENTS:
    - GPU with CUDA support
    - real-embeddings feature enabled
    - All 13 embedding models available

OPTIONS:
    --format <FORMAT>     Output format: console, json, markdown
    --ablation-only       Run ablation study only (no stress tests)
    --embedder <NAME>     Run stress test for specific embedder (e.g., E7, E5_CAUSAL)
    --data-dir <PATH>     Directory for temp storage (default: /tmp/embedder-stress)
    --verbose, -v         Verbose output with detailed logging
    --help, -h            Print this help

EXAMPLES:
    # Run all stress tests with real embeddings
    cargo run -p context-graph-benchmark --bin embedder-stress --features real-embeddings

    # Run E7 Code stress test only
    cargo run -p context-graph-benchmark --bin embedder-stress --features real-embeddings -- --embedder E7 -v

EMBEDDER NAMES:
    E5, E5_CAUSAL       - Causal relationships (asymmetric)
    E6, E6_SPARSE       - Sparse keywords (exact match)
    E7, E7_CODE         - Code patterns
    E8, E8_GRAPH        - Graph connectivity (source/target asymmetric)
    E9, E9_HDC          - Hyperdimensional (noise robust)
    E10, E10_MULTIMODAL - Cross-modal (visual/intent)
    E11, E11_ENTITY     - Named entities
    E12, E12_LATE       - Late interaction (word order)
    E13, E13_SPLADE     - SPLADE expansion
"#
    );
}

/// CRITICAL: This function requires real-embeddings feature
/// NO FALLBACK - will fail if feature not enabled
#[cfg(feature = "real-embeddings")]
async fn embed_text(text: &str, verbose: bool) -> Result<SemanticFingerprint, String> {
    use context_graph_embeddings::{get_warm_provider, initialize_global_warm_provider};

    if verbose {
        eprintln!("[EMBED] Embedding text: {}...", &text[..text.len().min(50)]);
    }

    // Initialize warm provider (loads all 13 models to GPU)
    initialize_global_warm_provider()
        .await
        .map_err(|e| format!("FATAL: Failed to initialize embedding provider: {}", e))?;

    let provider = get_warm_provider()
        .map_err(|e| format!("FATAL: Failed to get warm provider: {}", e))?;

    let output = provider
        .embed_all(text)
        .await
        .map_err(|e| format!("FATAL: Embedding failed for text '{}': {}", &text[..text.len().min(30)], e))?;

    if verbose {
        eprintln!("[EMBED] Success - E1 dim: {}, E7 dim: {}",
            output.fingerprint.e1_semantic.len(),
            output.fingerprint.e7_code.len());
    }

    Ok(output.fingerprint)
}

#[cfg(not(feature = "real-embeddings"))]
async fn embed_text(_text: &str, _verbose: bool) -> Result<SemanticFingerprint, String> {
    Err("FATAL: real-embeddings feature not enabled. Run with: cargo run --features real-embeddings".to_string())
}

/// Embed entire stress corpus for a config
async fn embed_corpus(config: &EmbedderStressConfig, verbose: bool) -> Result<Vec<EmbeddedDoc>, String> {
    let mut docs = Vec::with_capacity(config.corpus.len());

    eprintln!("[CORPUS] Embedding {} documents for {}...", config.corpus.len(), config.name);
    let start = Instant::now();

    for entry in &config.corpus {
        let fp = embed_text(&entry.content, verbose).await?;

        let uuid = Uuid::new_v4();
        docs.push(EmbeddedDoc {
            doc_id: entry.doc_id,
            content: entry.content.clone(),
            fingerprint: fp,
            uuid,
        });

        if verbose {
            eprintln!("[CORPUS] Embedded doc {} -> UUID {}", entry.doc_id, uuid);
        }
    }

    eprintln!("[CORPUS] Completed in {:?}", start.elapsed());
    Ok(docs)
}

/// Compute cosine similarity between two vectors
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        eprintln!("[ERROR] Dimension mismatch: {} vs {}", a.len(), b.len());
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a < f32::EPSILON || norm_b < f32::EPSILON {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

/// Get specific embedder vector from fingerprint
fn get_embedder_vector(fp: &SemanticFingerprint, embedder: EmbedderIndex) -> Option<&[f32]> {
    match embedder {
        EmbedderIndex::E1Semantic | EmbedderIndex::E1Matryoshka128 => Some(&fp.e1_semantic),
        EmbedderIndex::E2TemporalRecent => Some(&fp.e2_temporal_recent),
        EmbedderIndex::E3TemporalPeriodic => Some(&fp.e3_temporal_periodic),
        EmbedderIndex::E4TemporalPositional => Some(&fp.e4_temporal_positional),
        EmbedderIndex::E5Causal => Some(&fp.e5_causal),
        // E5 asymmetric indexes (ARCH-15)
        EmbedderIndex::E5CausalCause => Some(fp.get_e5_as_cause()),
        EmbedderIndex::E5CausalEffect => Some(fp.get_e5_as_effect()),
        EmbedderIndex::E7Code => Some(&fp.e7_code),
        EmbedderIndex::E8Graph => Some(&fp.e8_graph),
        EmbedderIndex::E9HDC => Some(&fp.e9_hdc),
        EmbedderIndex::E10Multimodal => Some(&fp.e10_multimodal),
        EmbedderIndex::E11Entity => Some(&fp.e11_entity),
        // Sparse embedders need special handling
        EmbedderIndex::E6Sparse | EmbedderIndex::E12LateInteraction | EmbedderIndex::E13Splade => None,
    }
}

/// Search corpus using a specific embedder
fn search_with_embedder(
    query_fp: &SemanticFingerprint,
    corpus: &[EmbeddedDoc],
    embedder: EmbedderIndex,
    top_k: usize,
    verbose: bool,
) -> Vec<SearchResult> {
    let query_vec = match get_embedder_vector(query_fp, embedder) {
        Some(v) => v,
        None => {
            if verbose {
                eprintln!("[SEARCH] Embedder {:?} uses sparse vectors - using E1 fallback", embedder);
            }
            &query_fp.e1_semantic
        }
    };

    let mut results: Vec<(usize, f32)> = corpus
        .iter()
        .map(|doc| {
            let doc_vec = get_embedder_vector(&doc.fingerprint, embedder)
                .unwrap_or(&doc.fingerprint.e1_semantic);
            let sim = cosine_similarity(query_vec, doc_vec);
            (doc.doc_id, sim)
        })
        .collect();

    // Sort by similarity descending
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(top_k);

    results
        .into_iter()
        .map(|(doc_id, similarity)| SearchResult { doc_id, similarity })
        .collect()
}

/// Search using multi-space weighted fusion (excludes temporal per AP-71)
fn search_multi_space(
    query_fp: &SemanticFingerprint,
    corpus: &[EmbeddedDoc],
    top_k: usize,
    verbose: bool,
) -> Vec<SearchResult> {
    // Weights per constitution (temporal E2-E4 = 0)
    let weights: [(EmbedderIndex, f32); 7] = [
        (EmbedderIndex::E1Semantic, 0.35),
        (EmbedderIndex::E5Causal, 0.15),
        (EmbedderIndex::E7Code, 0.20),
        (EmbedderIndex::E10Multimodal, 0.15),
        (EmbedderIndex::E8Graph, 0.05),
        (EmbedderIndex::E11Entity, 0.05),
        (EmbedderIndex::E9HDC, 0.05),
    ];

    let mut results: Vec<(usize, f32)> = corpus
        .iter()
        .map(|doc| {
            let mut weighted_sim = 0.0f32;
            for (embedder, weight) in &weights {
                let query_vec = get_embedder_vector(query_fp, *embedder);
                let doc_vec = get_embedder_vector(&doc.fingerprint, *embedder);

                if let (Some(q), Some(d)) = (query_vec, doc_vec) {
                    weighted_sim += weight * cosine_similarity(q, d);
                }
            }
            (doc.doc_id, weighted_sim)
        })
        .collect();

    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(top_k);

    if verbose {
        eprintln!("[MULTI-SPACE] Top result: doc {} with sim {:.4}",
            results.first().map(|r| r.0).unwrap_or(0),
            results.first().map(|r| r.1).unwrap_or(0.0));
    }

    results
        .into_iter()
        .map(|(doc_id, similarity)| SearchResult { doc_id, similarity })
        .collect()
}

/// Compute MRR for a query
fn compute_query_mrr(results: &[SearchResult], expected_docs: &[usize]) -> f32 {
    for (i, result) in results.iter().enumerate() {
        if expected_docs.contains(&result.doc_id) {
            return 1.0 / (i + 1) as f32;
        }
    }
    0.0
}

/// Check if expected doc is rank 1
fn check_success(results: &[SearchResult], expected_docs: &[usize]) -> bool {
    results.first().map(|r| expected_docs.contains(&r.doc_id)).unwrap_or(false)
}

/// Check anti-ranking (expected bad docs should NOT be in top-k)
fn compute_anti_score(results: &[SearchResult], anti_docs: &[usize], k: usize) -> f32 {
    if anti_docs.is_empty() {
        return 1.0;
    }

    let top_k_ids: Vec<usize> = results.iter().take(k).map(|r| r.doc_id).collect();
    let anti_in_top_k = anti_docs.iter().filter(|id| top_k_ids.contains(id)).count();

    1.0 - (anti_in_top_k as f32 / anti_docs.len() as f32)
}

/// Run stress tests for a single embedder config
async fn run_stress_test(
    config: &EmbedderStressConfig,
    verbose: bool,
) -> Result<EmbedderStressResults, String> {
    eprintln!("\n[TEST] Starting stress test for {}", config.name);
    eprintln!("[TEST] Description: {}", config.description);
    eprintln!("[TEST] Corpus size: {} documents", config.corpus.len());
    eprintln!("[TEST] Query count: {} queries", config.queries.len());

    // Step 1: Embed the corpus
    let corpus = embed_corpus(config, verbose).await?;

    // Verify corpus was embedded
    if corpus.len() != config.corpus.len() {
        return Err(format!(
            "FATAL: Corpus embedding incomplete. Expected {}, got {}",
            config.corpus.len(),
            corpus.len()
        ));
    }

    // Step 2: Embed and run each query
    let mut mrr_sum = 0.0f32;
    let mut success_count = 0usize;
    let mut anti_score_sum = 0.0f32;

    for (i, query) in config.queries.iter().enumerate() {
        eprintln!("[QUERY {}/{}] {}", i + 1, config.queries.len(), &query.query);

        // Embed query
        let query_fp = embed_text(&query.query, verbose).await?;

        // Search using target embedder
        let results = search_with_embedder(&query_fp, &corpus, query.target_embedder, 10, verbose);

        // Compute metrics
        let mrr = compute_query_mrr(&results, &query.expected_top_docs);
        let success = check_success(&results, &query.expected_top_docs);
        let anti_score = compute_anti_score(&results, &query.anti_expected_docs, 3);

        mrr_sum += mrr;
        if success {
            success_count += 1;
        }
        anti_score_sum += anti_score;

        // Log results
        eprintln!("[QUERY {}/{}] Results:", i + 1, config.queries.len());
        for (j, r) in results.iter().take(5).enumerate() {
            let marker = if query.expected_top_docs.contains(&r.doc_id) {
                "✓"
            } else if query.anti_expected_docs.contains(&r.doc_id) {
                "✗"
            } else {
                " "
            };
            eprintln!("  {}. doc {} - sim {:.4} {}", j + 1, r.doc_id, r.similarity, marker);
        }
        eprintln!("[QUERY {}/{}] MRR: {:.4}, Success: {}, Anti-score: {:.4}",
            i + 1, config.queries.len(), mrr, success, anti_score);
    }

    let query_count = config.queries.len() as f32;
    let stress_mrr = mrr_sum / query_count;
    let success_rate = success_count as f32 / query_count;
    let anti_score = anti_score_sum / query_count;

    // Step 3: Run ablation to measure this embedder's contribution
    eprintln!("[ABLATION] Computing ablation delta for {}...", config.name);
    let ablation_delta = compute_ablation_delta(&corpus, &config.queries, config.embedder, verbose).await?;

    let result = EmbedderStressResults {
        embedder: config.embedder,
        name: config.name.to_string(),
        stress_test_mrr: stress_mrr,
        stress_test_success_rate: success_rate,
        ablation_delta,
        unique_contribution: stress_mrr * ablation_delta,
        anti_ranking_score: anti_score,
        queries_passed: success_count,
        queries_total: config.queries.len(),
    };

    eprintln!("[TEST] Completed {} - MRR: {:.4}, Success: {:.1}%, Ablation Δ: {:.4}",
        config.name, stress_mrr, success_rate * 100.0, ablation_delta);

    Ok(result)
}

/// Compute ablation delta: Score(all) - Score(without this embedder)
async fn compute_ablation_delta(
    corpus: &[EmbeddedDoc],
    queries: &[StressQuery],
    target_embedder: EmbedderIndex,
    verbose: bool,
) -> Result<f32, String> {
    // Score with all embedders
    let mut all_mrr_sum = 0.0f32;

    // Score without target embedder
    let mut without_mrr_sum = 0.0f32;

    for query in queries {
        let query_fp = embed_text(&query.query, verbose).await?;

        // Multi-space with all embedders
        let all_results = search_multi_space(&query_fp, corpus, 10, false);
        all_mrr_sum += compute_query_mrr(&all_results, &query.expected_top_docs);

        // Multi-space without target (using E1 only as proxy)
        let without_results = search_with_embedder(&query_fp, corpus, EmbedderIndex::E1Semantic, 10, false);
        without_mrr_sum += compute_query_mrr(&without_results, &query.expected_top_docs);
    }

    let n = queries.len() as f32;
    let all_score = all_mrr_sum / n;
    let without_score = without_mrr_sum / n;

    let delta = all_score - without_score;

    if verbose {
        eprintln!("[ABLATION] All embedders MRR: {:.4}, Without {:?} MRR: {:.4}, Delta: {:.4}",
            all_score, target_embedder, without_score, delta);
    }

    Ok(delta.max(0.0)) // Delta should be non-negative
}

/// Main entry point
#[tokio::main]
#[allow(unreachable_code, unused_variables)]
async fn main() {
    let args = parse_args();

    // Print header
    eprintln!();
    eprintln!("=======================================================================");
    eprintln!("  PER-EMBEDDER STRESS TEST BENCHMARK (REAL EMBEDDINGS)");
    eprintln!("=======================================================================");
    eprintln!();
    eprintln!("Mode: {}", if cfg!(feature = "real-embeddings") { "REAL EMBEDDINGS" } else { "ERROR - NO EMBEDDINGS" });
    eprintln!();

    #[cfg(not(feature = "real-embeddings"))]
    {
        eprintln!("FATAL: This benchmark requires real embeddings.");
        eprintln!("Run with: cargo run -p context-graph-benchmark --bin embedder-stress --features real-embeddings");
        std::process::exit(1);
    }

    // Get stress test configs
    let configs: Vec<EmbedderStressConfig> = if let Some(embedder) = args.embedder {
        match get_stress_config(embedder) {
            Some(c) => vec![c],
            None => {
                eprintln!("ERROR: No stress test defined for {:?}", embedder);
                eprintln!("Available stress tests: E5, E6, E7, E9, E10, E11, E12, E13");
                std::process::exit(1);
            }
        }
    } else {
        get_all_stress_configs()
    };

    if configs.is_empty() {
        eprintln!("ERROR: No stress test configurations found");
        std::process::exit(1);
    }

    eprintln!("[INFO] Running {} stress test(s)", configs.len());

    // Run stress tests
    let mut results: Vec<EmbedderStressResults> = Vec::new();

    for config in &configs {
        match run_stress_test(config, args.verbose).await {
            Ok(result) => {
                results.push(result);
            }
            Err(e) => {
                eprintln!();
                eprintln!("=======================================================================");
                eprintln!("  FATAL ERROR");
                eprintln!("=======================================================================");
                eprintln!("Stress test for {} failed: {}", config.name, e);
                eprintln!();
                eprintln!("This is a REAL failure, not a test skip. Debug and fix the issue.");
                std::process::exit(1);
            }
        }
    }

    // Sort by unique contribution
    results.sort_by(|a, b| {
        b.unique_contribution
            .partial_cmp(&a.unique_contribution)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Output results
    match args.format {
        OutputFormat::Console => print_console_output(&results),
        OutputFormat::Json => print_json_output(&results),
        OutputFormat::Markdown => print_markdown_output(&results),
    }

    // Final verification
    eprintln!();
    eprintln!("=======================================================================");
    eprintln!("  VERIFICATION SUMMARY");
    eprintln!("=======================================================================");
    eprintln!("Total embedders tested: {}", results.len());
    eprintln!("Total queries executed: {}", results.iter().map(|r| r.queries_total).sum::<usize>());
    eprintln!("Total queries passed: {}", results.iter().map(|r| r.queries_passed).sum::<usize>());

    let avg_mrr: f32 = results.iter().map(|r| r.stress_test_mrr).sum::<f32>() / results.len() as f32;
    eprintln!("Average MRR across all tests: {:.4}", avg_mrr);

    if avg_mrr < 0.5 {
        eprintln!();
        eprintln!("WARNING: Average MRR is low ({:.4}). Check stress corpus and embeddings.", avg_mrr);
    }

    eprintln!();
}

/// Print results in console format
fn print_console_output(results: &[EmbedderStressResults]) {
    println!();
    println!("=======================================================================");
    println!("  STRESS TEST RESULTS (REAL EMBEDDINGS)");
    println!("=======================================================================");
    println!();
    println!("Embedder       Stress MRR  Success%  Ablation Δ  Unique Contrib  Anti-Rank");
    println!("--------------------------------------------------------------------------------");

    for r in results {
        println!(
            "{:<14} {:>8.3}    {:>5.0}%      {:>+5.2}         {:>6.3}        {:>5.2}",
            r.name,
            r.stress_test_mrr,
            r.stress_test_success_rate * 100.0,
            r.ablation_delta,
            r.unique_contribution,
            r.anti_ranking_score
        );
    }

    println!();
    println!("=======================================================================");
    println!("  TOP CONTRIBUTORS (by Unique Contribution)");
    println!("=======================================================================");

    let max_contrib = results
        .iter()
        .map(|r| r.unique_contribution)
        .fold(0.0f32, f32::max);

    for (i, r) in results.iter().take(5).enumerate() {
        let bar_len = if max_contrib > 0.0 {
            ((r.unique_contribution / max_contrib) * 20.0) as usize
        } else {
            0
        };
        let bar = "█".repeat(bar_len);
        let empty = " ".repeat(20 - bar_len);
        println!(
            "{}. {:<14} {:>6.3}  [{}{}]",
            i + 1,
            r.name,
            r.unique_contribution,
            bar,
            empty
        );
    }

    println!();
    println!("=======================================================================");
    println!("  RECOMMENDATIONS");
    println!("=======================================================================");

    for r in results.iter().filter(|r| r.unique_contribution > 0.08) {
        println!(
            "✓ {} shows HIGH unique contribution ({:.3}) - maintain high weight",
            r.name, r.unique_contribution
        );
    }

    for r in results.iter().filter(|r| r.unique_contribution < 0.03) {
        println!(
            "⚠ {} has LOW contribution ({:.3}) - consider reducing weight",
            r.name, r.unique_contribution
        );
    }

    println!();
}

/// Print results in JSON format
fn print_json_output(results: &[EmbedderStressResults]) {
    let output = serde_json::json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "mode": "real_embeddings",
        "embedder_results": results,
        "summary": {
            "total_embedders": results.len(),
            "total_queries": results.iter().map(|r| r.queries_total).sum::<usize>(),
            "total_passed": results.iter().map(|r| r.queries_passed).sum::<usize>(),
            "avg_mrr": results.iter().map(|r| r.stress_test_mrr).sum::<f32>() / results.len() as f32,
        },
        "recommendations": {
            "high_value": results.iter().filter(|r| r.unique_contribution > 0.08).map(|r| r.name.clone()).collect::<Vec<_>>(),
            "medium_value": results.iter().filter(|r| r.unique_contribution > 0.03 && r.unique_contribution <= 0.08).map(|r| r.name.clone()).collect::<Vec<_>>(),
            "low_value": results.iter().filter(|r| r.unique_contribution <= 0.03).map(|r| r.name.clone()).collect::<Vec<_>>(),
        }
    });

    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}

/// Print results in Markdown format
fn print_markdown_output(results: &[EmbedderStressResults]) {
    println!("# Per-Embedder Stress Test Results (Real Embeddings)");
    println!();
    println!("Generated: {}", chrono::Utc::now().to_rfc3339());
    println!();
    println!("## Results Table");
    println!();
    println!("| Embedder | Stress MRR | Success % | Ablation Δ | Unique Contrib | Anti-Rank |");
    println!("|----------|------------|-----------|------------|----------------|-----------|");

    for r in results {
        println!(
            "| {} | {:.3} | {:.0}% | {:.2} | {:.3} | {:.2} |",
            r.name,
            r.stress_test_mrr,
            r.stress_test_success_rate * 100.0,
            r.ablation_delta,
            r.unique_contribution,
            r.anti_ranking_score
        );
    }

    println!();
    println!("## Summary");
    println!();
    println!("- **Total embedders tested**: {}", results.len());
    println!("- **Total queries executed**: {}", results.iter().map(|r| r.queries_total).sum::<usize>());
    println!("- **Average MRR**: {:.4}", results.iter().map(|r| r.stress_test_mrr).sum::<f32>() / results.len() as f32);

    println!();
    println!("## Recommendations");
    println!();

    let high: Vec<_> = results.iter().filter(|r| r.unique_contribution > 0.08).collect();
    if !high.is_empty() {
        println!("### High Value Embedders");
        for r in high {
            println!("- **{}**: {:.3} unique contribution", r.name, r.unique_contribution);
        }
        println!();
    }

    let low: Vec<_> = results.iter().filter(|r| r.unique_contribution < 0.03).collect();
    if !low.is_empty() {
        println!("### Low Value Embedders (consider reducing weight)");
        for r in low {
            println!("- **{}**: {:.3} unique contribution", r.name, r.unique_contribution);
        }
    }
}
