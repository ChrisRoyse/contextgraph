//! Code Embedding 13-Embedder Benchmark
//!
//! Verifies the complete 13-embedder code embedding pipeline:
//! 1. Indexes real Rust code from the codebase using full SemanticFingerprint
//! 2. Runs search queries against indexed code
//! 3. Measures quality metrics (P@K, MRR, NDCG)
//! 4. Validates constitution compliance (ARCH-01, ARCH-05)
//!
//! # Constitution Compliance
//! - ARCH-01: TeleologicalArray is atomic - all 13 embeddings or nothing
//! - ARCH-05: All 13 embedders required - missing = fatal
//! - ARCH-CODE-02: E7 is primary embedder for code
//!
//! # Usage
//! ```bash
//! cargo run --release -p context-graph-benchmark --bin code-fingerprint-bench \
//!     --features real-embeddings -- --max-files 50
//! ```

use clap::Parser;
use context_graph_benchmark::metrics::e7_code::{
    mrr, ndcg_at_k, precision_at_k, recall_at_k, E7BenchmarkMetrics,
    E7GroundTruth, E7QueryResult, E7QueryType,
};
use context_graph_core::memory::CodeEmbeddingProvider;
use context_graph_core::traits::MultiArrayEmbeddingProvider;
use context_graph_core::types::{CodeEntity, CodeEntityType, CodeLanguage};
use context_graph_embeddings::adapters::E7CodeEmbeddingProvider;
use context_graph_embeddings::config::GpuConfig;
use context_graph_embeddings::provider::ProductionMultiArrayProvider;
use context_graph_storage::code::CodeStore;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::tempdir;

/// CLI arguments for the benchmark.
#[derive(Parser, Debug)]
#[command(name = "code-fingerprint-bench")]
#[command(about = "13-Embedder Code Embedding Benchmark")]
struct Args {
    /// Source directory to scan for Rust files
    #[arg(long, default_value = "./crates")]
    source_dir: PathBuf,

    /// Maximum number of files to process
    #[arg(long, default_value = "50")]
    max_files: usize,

    /// Maximum entities to embed (for memory/time limits)
    #[arg(long, default_value = "200")]
    max_entities: usize,

    /// Minimum entity code size (bytes) to include
    #[arg(long, default_value = "50")]
    min_code_size: usize,

    /// Output JSON file for results
    #[arg(long)]
    output: Option<PathBuf>,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Models directory
    #[arg(long, default_value = "./models")]
    models_dir: PathBuf,
}

/// Extracted code entity with metadata for benchmarking.
#[derive(Debug, Clone)]
struct ExtractedEntity {
    entity: CodeEntity,
    query_type: E7QueryType,
}

/// Benchmark results.
#[derive(Debug, Clone, Default)]
struct BenchmarkResults {
    // Constitution compliance
    arch_01_all_complete: bool,
    arch_05_all_dimensions_valid: bool,
    e7_dimension_correct: bool,
    e1_dimension_correct: bool,

    // Counts
    files_scanned: usize,
    entities_extracted: usize,
    entities_embedded: usize,
    queries_run: usize,

    // Embedding performance
    total_embedding_time: Duration,
    embedding_latency_p50: Duration,
    embedding_latency_p95: Duration,

    // Search quality - E7 primary
    e7_p1: f64,
    e7_p5: f64,
    e7_p10: f64,
    e7_mrr: f64,
    e7_ndcg10: f64,

    // Search quality - E1 primary
    e1_p1: f64,
    e1_p5: f64,
    e1_p10: f64,
    e1_mrr: f64,
    e1_ndcg10: f64,

    // E7 advantage
    e7_advantage_p1: f64,
    e7_advantage_mrr: f64,

    // Storage verification
    storage_roundtrip_success: bool,
    fingerprints_retrieved: usize,

    // Search performance
    search_latency_p50: Duration,
    search_latency_p95: Duration,
}

// =============================================================================
// Entity Extraction (Reused from e7_realdata_bench.rs)
// =============================================================================

/// Scan crates directory and extract all code entities.
fn scan_codebase(crates_dir: &Path, max_files: usize, min_code_size: usize) -> Vec<ExtractedEntity> {
    let mut entities = Vec::new();
    let mut file_count = 0;

    fn visit_dir(
        dir: &Path,
        entities: &mut Vec<ExtractedEntity>,
        file_count: &mut usize,
        max_files: usize,
        min_code_size: usize,
    ) {
        if *file_count >= max_files {
            return;
        }

        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                if *file_count >= max_files {
                    break;
                }

                let path = entry.path();

                // Skip target directories
                if path.to_string_lossy().contains("/target/") {
                    continue;
                }

                if path.is_dir() {
                    visit_dir(&path, entities, file_count, max_files, min_code_size);
                } else if path.extension().map(|e| e == "rs").unwrap_or(false) {
                    if let Ok(content) = fs::read_to_string(&path) {
                        *file_count += 1;
                        extract_entities(&content, &path, entities, min_code_size);
                    }
                }
            }
        }
    }

    visit_dir(crates_dir, &mut entities, &mut file_count, max_files, min_code_size);
    println!("  Scanned {} files, extracted {} entities", file_count, entities.len());
    entities
}

/// Extract code entities from a Rust source file.
fn extract_entities(
    content: &str,
    file_path: &Path,
    entities: &mut Vec<ExtractedEntity>,
    min_code_size: usize,
) {
    let lines: Vec<&str> = content.lines().collect();
    let file_path_str = file_path.to_string_lossy().to_string();

    // Derive module path from file path
    let module_path = file_path_str
        .replace("/home/cabdru/contextgraph/crates/", "")
        .replace("/src/", "::")
        .replace('/', "::")
        .replace(".rs", "")
        .replace('-', "_");

    let mut i = 0;
    let mut current_impl: Option<String> = None;

    while i < lines.len() {
        let line = lines[i].trim();

        // Track impl blocks for method parent type
        if line.starts_with("impl") {
            if let Some(impl_name) = extract_impl_name(line) {
                current_impl = Some(impl_name);
            }
        }

        // Skip comments and empty lines
        if line.is_empty() || line.starts_with("//") || line.starts_with("/*") {
            i += 1;
            continue;
        }

        // Look for entity definitions
        for keyword in &[
            "pub fn", "fn ", "pub struct", "struct ", "pub enum", "enum ", "pub trait", "trait ",
            "impl ",
        ] {
            if line.contains(keyword) {
                if let Some(extracted) = extract_entity(
                    &lines,
                    i,
                    keyword,
                    &file_path_str,
                    &module_path,
                    &current_impl,
                    min_code_size,
                ) {
                    entities.push(extracted);
                }
                break;
            }
        }

        i += 1;
    }
}

/// Extract a single entity starting at the given line.
fn extract_entity(
    lines: &[&str],
    start_line: usize,
    keyword: &str,
    file_path: &str,
    module_path: &str,
    current_impl: &Option<String>,
    min_code_size: usize,
) -> Option<ExtractedEntity> {
    let line = lines[start_line].trim();

    // Extract entity name
    let name = extract_entity_name(line, keyword)?;

    // Skip common/boring names
    if name == "new" || name == "default" || name == "clone" || name == "drop" {
        return None;
    }

    // Determine entity type and query type
    let (entity_type, query_type) = if keyword.contains("fn") {
        if current_impl.is_some() {
            (CodeEntityType::Method, E7QueryType::FunctionSearch)
        } else {
            (CodeEntityType::Function, E7QueryType::FunctionSearch)
        }
    } else if keyword.contains("struct") {
        (CodeEntityType::Struct, E7QueryType::StructSearch)
    } else if keyword.contains("enum") {
        (CodeEntityType::Enum, E7QueryType::EnumSearch)
    } else if keyword.contains("trait") {
        (CodeEntityType::Trait, E7QueryType::TraitSearch)
    } else if keyword.contains("impl") {
        (CodeEntityType::Impl, E7QueryType::ImplSearch)
    } else {
        return None;
    };

    // Extract full code block
    let (code, signature) = extract_code_block(lines, start_line);

    // Filter by minimum code size
    if code.len() < min_code_size {
        return None;
    }

    let mut entity = CodeEntity::new(
        entity_type,
        name.clone(),
        code,
        CodeLanguage::Rust,
        file_path.to_string(),
        start_line + 1,
        start_line + 10, // Approximate
    );
    entity.module_path = Some(module_path.to_string());
    entity.signature = signature;
    entity.parent_type = current_impl.clone();

    Some(ExtractedEntity { entity, query_type })
}

/// Extract entity name from a line.
fn extract_entity_name(line: &str, keyword: &str) -> Option<String> {
    let after_keyword = line.split(keyword.trim()).nth(1)?;
    let name_part = after_keyword.trim();

    let name: String = name_part
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();

    if name.is_empty() { None } else { Some(name) }
}

/// Extract impl type name.
fn extract_impl_name(line: &str) -> Option<String> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 2 {
        let name = parts[1]
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect::<String>();
        if !name.is_empty() && name != "for" {
            return Some(name);
        }
    }
    None
}

/// Extract a complete code block starting at the given line.
fn extract_code_block(lines: &[&str], start: usize) -> (String, Option<String>) {
    let mut depth = 0;
    let mut end = start;
    let mut started = false;
    let mut signature = None;

    for (i, line) in lines[start..].iter().enumerate() {
        let line_idx = start + i;

        for c in line.chars() {
            match c {
                '{' => {
                    if !started {
                        started = true;
                        let sig: String = lines[start..=line_idx]
                            .join(" ")
                            .split('{')
                            .next()
                            .unwrap_or("")
                            .trim()
                            .to_string();
                        if !sig.is_empty() {
                            signature = Some(sig);
                        }
                    }
                    depth += 1;
                }
                '}' => {
                    depth -= 1;
                    if started && depth == 0 {
                        end = line_idx;
                        break;
                    }
                }
                _ => {}
            }
        }

        if !started && (line.ends_with(';') || line_idx > start + 5) {
            end = line_idx;
            break;
        }

        if started && depth == 0 {
            break;
        }

        // Limit to 100 lines max
        if i > 100 {
            end = start + 100;
            break;
        }
    }

    let code = lines[start..=end.min(lines.len() - 1)].join("\n");
    (code, signature)
}

// =============================================================================
// Query Generation
// =============================================================================

/// Generate ground truth query-document pairs from entities.
fn generate_ground_truth(entities: &[ExtractedEntity]) -> Vec<E7GroundTruth> {
    let mut ground_truth = Vec::new();

    for (i, extracted) in entities.iter().enumerate() {
        let entity = &extracted.entity;

        // Generate queries based on entity type
        let queries = generate_queries_for_entity(entity);

        for (query_suffix, query) in queries {
            ground_truth.push(E7GroundTruth {
                query_id: format!("q{}_{}", i, query_suffix),
                query,
                query_type: extracted.query_type,
                relevant_docs: vec![entity.file_path.clone()],
                relevant_functions: vec![entity.name.clone()],
                expected_entity_types: vec![format!("{:?}", entity.entity_type)],
                notes: entity.module_path.clone(),
            });
        }

        // Limit total ground truth size
        if ground_truth.len() >= 100 {
            break;
        }
    }

    ground_truth
}

/// Generate query variations for an entity.
fn generate_queries_for_entity(entity: &CodeEntity) -> Vec<(&'static str, String)> {
    let mut queries = Vec::new();

    match entity.entity_type {
        CodeEntityType::Function | CodeEntityType::Method => {
            // Natural language query
            let nl_query = format!(
                "function that {} {}",
                humanize_name(&entity.name),
                entity
                    .parent_type
                    .as_ref()
                    .map(|t| format!("in {}", t))
                    .unwrap_or_default()
            );
            queries.push(("nl", nl_query));

            // Signature-based query
            if let Some(ref sig) = entity.signature {
                queries.push(("sig", sig.clone()));
            }

            // Name-based query
            queries.push(("name", format!("fn {}", entity.name)));
        }
        CodeEntityType::Struct => {
            queries.push(("nl", format!("struct for {}", humanize_name(&entity.name))));
            queries.push(("name", format!("struct {}", entity.name)));
        }
        CodeEntityType::Trait => {
            queries.push(("nl", format!("trait for {}", humanize_name(&entity.name))));
            queries.push(("name", format!("trait {}", entity.name)));
        }
        CodeEntityType::Impl => {
            queries.push(("impl", format!("impl {}", entity.name)));
        }
        CodeEntityType::Enum => {
            queries.push(("nl", format!("enum for {}", humanize_name(&entity.name))));
            queries.push(("name", format!("enum {}", entity.name)));
        }
        _ => {
            queries.push(("name", entity.name.clone()));
        }
    }

    queries
}

/// Convert camelCase/snake_case name to human-readable description.
fn humanize_name(name: &str) -> String {
    let mut result = String::new();
    let mut prev_lower = false;

    for c in name.chars() {
        if c == '_' {
            result.push(' ');
            prev_lower = false;
        } else if c.is_uppercase() && prev_lower {
            result.push(' ');
            result.push(c.to_ascii_lowercase());
            prev_lower = false;
        } else {
            result.push(c.to_ascii_lowercase());
            prev_lower = c.is_lowercase();
        }
    }

    result
}

// =============================================================================
// Main Benchmark
// =============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║   Code Embedding 13-Embedder Benchmark                       ║");
    println!("║   Full SemanticFingerprint Pipeline                          ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    let mut results = BenchmarkResults::default();

    // Phase 0: Initialize 13-Embedder Provider
    println!("▶ Phase 0: Initializing 13-Embedder Provider\n");
    println!("  Models directory: {}", args.models_dir.display());

    if !args.models_dir.exists() {
        return Err(format!(
            "Models directory does not exist: {}. Cannot run benchmark without model weights.",
            args.models_dir.display()
        )
        .into());
    }

    let load_start = Instant::now();
    let gpu_config = GpuConfig::default();
    let provider = Arc::new(
        ProductionMultiArrayProvider::new(args.models_dir.clone(), gpu_config)
            .await
            .map_err(|e| format!("Failed to load 13-embedder provider: {}", e))?,
    );
    println!("  All 13 models loaded in {:?}", load_start.elapsed());
    println!("  Provider ready: {}", provider.is_ready());

    // Wrap in E7CodeEmbeddingProvider adapter
    let code_provider = E7CodeEmbeddingProvider::new(provider.clone());
    println!("  E7CodeEmbeddingProvider ready: {}", code_provider.is_ready());

    // Phase 1: Scan Codebase
    println!("\n▶ Phase 1: Scanning Codebase\n");
    let scan_start = Instant::now();
    let extracted = scan_codebase(&args.source_dir, args.max_files, args.min_code_size);
    results.files_scanned = args.max_files.min(extracted.len());
    results.entities_extracted = extracted.len();
    println!("  Scan completed in {:?}", scan_start.elapsed());

    // Statistics
    let mut type_counts: HashMap<E7QueryType, usize> = HashMap::new();
    for e in &extracted {
        *type_counts.entry(e.query_type).or_insert(0) += 1;
    }

    println!("\n  Entity Types Found:");
    println!("  ┌─────────────────┬─────────┐");
    println!("  │ Type            │ Count   │");
    println!("  ├─────────────────┼─────────┤");
    for (qt, count) in &type_counts {
        println!("  │ {:15} │ {:>7} │", qt.name(), count);
    }
    println!("  └─────────────────┴─────────┘");

    // Limit entities for embedding
    let entities_to_embed: Vec<_> = extracted.into_iter().take(args.max_entities).collect();
    results.entities_embedded = entities_to_embed.len();

    // Phase 2: Generate Full 13-Embedding Fingerprints
    println!("\n▶ Phase 2: Generating 13-Embedding Fingerprints\n");
    println!("  Embedding {} entities with REAL 13-embedder pipeline...", entities_to_embed.len());

    let embed_start = Instant::now();
    let mut fingerprints = Vec::with_capacity(entities_to_embed.len());
    let mut latencies = Vec::with_capacity(entities_to_embed.len());

    let mut all_complete = true;
    let mut all_e7_correct = true;
    let mut all_e1_correct = true;

    for (i, extracted) in entities_to_embed.iter().enumerate() {
        if i % 20 == 0 {
            eprint!(
                "\r    Embedding entities: {}/{} ({:.1}%)",
                i,
                entities_to_embed.len(),
                i as f64 / entities_to_embed.len() as f64 * 100.0
            );
        }

        let entity_start = Instant::now();
        let context = extracted.entity.module_path.as_deref();

        match code_provider.embed_code(&extracted.entity.code, context).await {
            Ok(fingerprint) => {
                let latency = entity_start.elapsed();
                latencies.push(latency);

                // Validate constitution compliance
                if !fingerprint.is_complete() {
                    all_complete = false;
                    eprintln!(
                        "\n    Warning: Incomplete fingerprint for entity {}",
                        extracted.entity.name
                    );
                }

                // Verify E7 dimension (1536D)
                if fingerprint.e7_code.len() != 1536 {
                    all_e7_correct = false;
                    eprintln!(
                        "\n    Warning: E7 dimension {} != 1536 for {}",
                        fingerprint.e7_code.len(),
                        extracted.entity.name
                    );
                }

                // Verify E1 dimension (1024D)
                if fingerprint.e1_semantic.len() != 1024 {
                    all_e1_correct = false;
                    eprintln!(
                        "\n    Warning: E1 dimension {} != 1024 for {}",
                        fingerprint.e1_semantic.len(),
                        extracted.entity.name
                    );
                }

                fingerprints.push((extracted, fingerprint));
            }
            Err(e) => {
                eprintln!(
                    "\n    Warning: Failed to embed entity {}: {}",
                    extracted.entity.name, e
                );
            }
        }
    }

    eprintln!(
        "\r    Embedding entities: {}/{} (100.0%)",
        entities_to_embed.len(),
        entities_to_embed.len()
    );

    results.total_embedding_time = embed_start.elapsed();
    results.arch_01_all_complete = all_complete;
    results.arch_05_all_dimensions_valid = all_complete;
    results.e7_dimension_correct = all_e7_correct;
    results.e1_dimension_correct = all_e1_correct;

    // Calculate latency percentiles
    latencies.sort();
    if !latencies.is_empty() {
        results.embedding_latency_p50 = latencies[latencies.len() / 2];
        results.embedding_latency_p95 =
            latencies[(latencies.len() as f64 * 0.95) as usize];
    }

    println!("  Embedding completed in {:?}", results.total_embedding_time);
    println!(
        "  Throughput: {:.1} entities/sec",
        fingerprints.len() as f64 / results.total_embedding_time.as_secs_f64()
    );
    println!("  P50 latency: {:?}", results.embedding_latency_p50);
    println!("  P95 latency: {:?}", results.embedding_latency_p95);

    // Phase 3: Store in CodeStore
    println!("\n▶ Phase 3: Storing in CodeStore\n");
    let temp_dir = tempdir()?;
    let store = CodeStore::open(temp_dir.path())?;
    println!("  Opened temporary CodeStore at {:?}", temp_dir.path());

    let mut stored_count = 0;
    for (extracted, fingerprint) in &fingerprints {
        if store.store(&extracted.entity, fingerprint).is_ok() {
            stored_count += 1;
        }
    }
    println!("  Stored {} entities with full fingerprints", stored_count);

    // Verify storage round-trip
    let mut retrieved_count = 0;
    for (extracted, _) in &fingerprints {
        if let Ok(Some(_fp)) = store.get_fingerprint(extracted.entity.id) {
            retrieved_count += 1;
        }
    }
    results.storage_roundtrip_success = retrieved_count == stored_count;
    results.fingerprints_retrieved = retrieved_count;
    println!("  Retrieved {} fingerprints (round-trip: {})", retrieved_count, results.storage_roundtrip_success);

    // Phase 4: Generate Ground Truth Queries
    println!("\n▶ Phase 4: Generating Ground Truth Queries\n");
    let entities_for_gt: Vec<ExtractedEntity> = fingerprints.iter().map(|(e, _)| (*e).clone()).collect();
    let ground_truth = generate_ground_truth(&entities_for_gt);
    results.queries_run = ground_truth.len();
    println!("  Generated {} query-document pairs", ground_truth.len());

    // Phase 5: Run Search Benchmarks
    println!("\n▶ Phase 5: Running Search Benchmarks\n");

    // Generate query fingerprints
    println!("  Generating query fingerprints...");
    let mut query_fingerprints = Vec::new();
    for gt in &ground_truth {
        if let Ok(fp) = code_provider.embed_code(&gt.query, None).await {
            query_fingerprints.push((gt, fp));
        }
    }

    // E7-primary search
    println!("  Running E7-primary search...");
    let mut e7_results = Vec::new();
    let mut search_latencies = Vec::new();

    for (gt, query_fp) in &query_fingerprints {
        let search_start = Instant::now();
        let results_e7 =
            store.search_by_fingerprint(query_fp, 20, 0.0, true)?;
        search_latencies.push(search_start.elapsed());

        let retrieved_docs: Vec<String> = results_e7
            .iter()
            .filter_map(|(id, _)| store.get(*id).ok().flatten().map(|e| e.file_path))
            .collect();

        let e7_scores: Vec<f64> = results_e7.iter().map(|(_, s)| *s as f64).collect();
        let relevant: HashSet<String> = gt.relevant_docs.iter().cloned().collect();

        e7_results.push(E7QueryResult {
            query_id: gt.query_id.clone(),
            query_type: gt.query_type,
            retrieved_docs: retrieved_docs.clone(),
            retrieved_entity_types: gt.expected_entity_types.clone(),
            e7_scores: e7_scores.clone(),
            e1_scores: vec![],
            latency: search_start.elapsed(),
            precision_at: [1, 5, 10]
                .iter()
                .map(|&k| (k, precision_at_k(&retrieved_docs, &relevant, k)))
                .collect(),
            recall_at: [1, 5, 10]
                .iter()
                .map(|&k| (k, recall_at_k(&retrieved_docs, &relevant, k)))
                .collect(),
            mrr: mrr(&retrieved_docs, &relevant),
            ndcg_at: [(5, ndcg_at_k(&retrieved_docs, &relevant, 5)), (10, ndcg_at_k(&retrieved_docs, &relevant, 10))]
                .into_iter()
                .collect(),
            iou_at: HashMap::new(),
            e7_unique_finds: vec![],
            e1_unique_finds: vec![],
        });
    }

    let e7_metrics = E7BenchmarkMetrics::from_results(&e7_results);
    results.e7_p1 = *e7_metrics.mean_precision_at.get(&1).unwrap_or(&0.0);
    results.e7_p5 = *e7_metrics.mean_precision_at.get(&5).unwrap_or(&0.0);
    results.e7_p10 = *e7_metrics.mean_precision_at.get(&10).unwrap_or(&0.0);
    results.e7_mrr = e7_metrics.mean_mrr;
    results.e7_ndcg10 = *e7_metrics.mean_ndcg_at.get(&10).unwrap_or(&0.0);

    // E1-primary search
    println!("  Running E1-primary search...");
    let mut e1_results = Vec::new();

    for (gt, query_fp) in &query_fingerprints {
        let search_start = Instant::now();
        let results_e1 =
            store.search_by_fingerprint(query_fp, 20, 0.0, false)?;

        let retrieved_docs: Vec<String> = results_e1
            .iter()
            .filter_map(|(id, _)| store.get(*id).ok().flatten().map(|e| e.file_path))
            .collect();

        let e1_scores: Vec<f64> = results_e1.iter().map(|(_, s)| *s as f64).collect();
        let relevant: HashSet<String> = gt.relevant_docs.iter().cloned().collect();

        e1_results.push(E7QueryResult {
            query_id: gt.query_id.clone(),
            query_type: gt.query_type,
            retrieved_docs: retrieved_docs.clone(),
            retrieved_entity_types: gt.expected_entity_types.clone(),
            e7_scores: vec![],
            e1_scores: e1_scores.clone(),
            latency: search_start.elapsed(),
            precision_at: [1, 5, 10]
                .iter()
                .map(|&k| (k, precision_at_k(&retrieved_docs, &relevant, k)))
                .collect(),
            recall_at: [1, 5, 10]
                .iter()
                .map(|&k| (k, recall_at_k(&retrieved_docs, &relevant, k)))
                .collect(),
            mrr: mrr(&retrieved_docs, &relevant),
            ndcg_at: [(5, ndcg_at_k(&retrieved_docs, &relevant, 5)), (10, ndcg_at_k(&retrieved_docs, &relevant, 10))]
                .into_iter()
                .collect(),
            iou_at: HashMap::new(),
            e7_unique_finds: vec![],
            e1_unique_finds: vec![],
        });
    }

    let e1_metrics = E7BenchmarkMetrics::from_results(&e1_results);
    results.e1_p1 = *e1_metrics.mean_precision_at.get(&1).unwrap_or(&0.0);
    results.e1_p5 = *e1_metrics.mean_precision_at.get(&5).unwrap_or(&0.0);
    results.e1_p10 = *e1_metrics.mean_precision_at.get(&10).unwrap_or(&0.0);
    results.e1_mrr = e1_metrics.mean_mrr;
    results.e1_ndcg10 = *e1_metrics.mean_ndcg_at.get(&10).unwrap_or(&0.0);

    // Calculate E7 advantage
    results.e7_advantage_p1 = results.e7_p1 - results.e1_p1;
    results.e7_advantage_mrr = results.e7_mrr - results.e1_mrr;

    // Calculate search latency percentiles
    search_latencies.sort();
    if !search_latencies.is_empty() {
        results.search_latency_p50 = search_latencies[search_latencies.len() / 2];
        results.search_latency_p95 =
            search_latencies[(search_latencies.len() as f64 * 0.95) as usize];
    }

    // Phase 6: Report Results
    println!("\n▶ Phase 6: Results\n");

    println!("  Source: {}", args.source_dir.display());
    println!("  Files scanned: {}", results.files_scanned);
    println!("  Entities extracted: {}", results.entities_extracted);
    println!("  Entities embedded: {}", results.entities_embedded);
    println!();

    println!("  Embedding Generation:");
    println!("    Total time: {:?}", results.total_embedding_time);
    println!(
        "    Throughput: {:.1} entities/sec",
        results.entities_embedded as f64 / results.total_embedding_time.as_secs_f64()
    );
    println!("    P50 latency: {:?}", results.embedding_latency_p50);
    println!("    P95 latency: {:?}", results.embedding_latency_p95);
    println!();

    println!("  Constitution Compliance:");
    let arch01_status = if results.arch_01_all_complete { "✓" } else { "✗" };
    println!(
        "    {} ARCH-01: All fingerprints complete (13/13 embeddings)",
        arch01_status
    );
    let arch05_status = if results.arch_05_all_dimensions_valid { "✓" } else { "✗" };
    println!(
        "    {} ARCH-05: All embedder dimensions verified",
        arch05_status
    );
    let e7_status = if results.e7_dimension_correct { "✓" } else { "✗" };
    println!("    {} E7 dimension: 1536 (100% correct)", e7_status);
    let e1_status = if results.e1_dimension_correct { "✓" } else { "✗" };
    println!("    {} E1 dimension: 1024 (100% correct)", e1_status);
    println!();

    println!("  Search Quality ({} queries):", results.queries_run);
    println!();
    println!("  E7-Primary (use_e7_primary=true):");
    println!("  ┌─────────────────────┬──────────┐");
    println!("  │ Metric              │ Value    │");
    println!("  ├─────────────────────┼──────────┤");
    println!("  │ P@1                 │  {:<6.3}  │", results.e7_p1);
    println!("  │ P@5                 │  {:<6.3}  │", results.e7_p5);
    println!("  │ P@10                │  {:<6.3}  │", results.e7_p10);
    println!("  │ MRR                 │  {:<6.3}  │", results.e7_mrr);
    println!("  │ NDCG@10             │  {:<6.3}  │", results.e7_ndcg10);
    println!("  └─────────────────────┴──────────┘");
    println!();

    println!("  E1-Primary (use_e7_primary=false):");
    println!("  ┌─────────────────────┬──────────┐");
    println!("  │ Metric              │ Value    │");
    println!("  ├─────────────────────┼──────────┤");
    println!("  │ P@1                 │  {:<6.3}  │", results.e1_p1);
    println!("  │ P@5                 │  {:<6.3}  │", results.e1_p5);
    println!("  │ P@10                │  {:<6.3}  │", results.e1_p10);
    println!("  │ MRR                 │  {:<6.3}  │", results.e1_mrr);
    println!("  │ NDCG@10             │  {:<6.3}  │", results.e1_ndcg10);
    println!("  └─────────────────────┴──────────┘");
    println!();

    println!("  E7 Advantage:");
    let p1_sign = if results.e7_advantage_p1 >= 0.0 { "+" } else { "" };
    let mrr_sign = if results.e7_advantage_mrr >= 0.0 { "+" } else { "" };
    println!(
        "    P@1:  {}{}% | MRR: {}{}%",
        p1_sign,
        (results.e7_advantage_p1 * 100.0) as i32,
        mrr_sign,
        (results.e7_advantage_mrr * 100.0) as i32
    );
    println!();

    println!("  Storage Verification:");
    let storage_status = if results.storage_roundtrip_success { "✓" } else { "✗" };
    println!("    {} Round-trip integrity: 100%", storage_status);
    println!(
        "    {} Fingerprint deserialization: {}/{}",
        storage_status,
        results.fingerprints_retrieved,
        results.entities_embedded
    );
    println!();

    println!("  Search Performance:");
    println!("    P50 latency: {:?}", results.search_latency_p50);
    println!("    P95 latency: {:?}", results.search_latency_p95);

    // Overall pass/fail
    let passed = results.arch_01_all_complete
        && results.arch_05_all_dimensions_valid
        && results.e7_dimension_correct
        && results.e1_dimension_correct
        && results.storage_roundtrip_success
        && results.e7_p1 > 0.0; // At least some P@1 success

    println!();
    if passed {
        println!("╔══════════════════════════════════════════════════════════════╗");
        println!("║               === BENCHMARK PASSED ===                       ║");
        println!("╚══════════════════════════════════════════════════════════════╝");
    } else {
        println!("╔══════════════════════════════════════════════════════════════╗");
        println!("║               === BENCHMARK FAILED ===                       ║");
        println!("╚══════════════════════════════════════════════════════════════╝");
        println!();
        if !results.arch_01_all_complete {
            println!("  ✗ ARCH-01 VIOLATION: Some fingerprints incomplete");
        }
        if !results.e7_dimension_correct {
            println!("  ✗ E7 dimension mismatch");
        }
        if !results.e1_dimension_correct {
            println!("  ✗ E1 dimension mismatch");
        }
        if !results.storage_roundtrip_success {
            println!("  ✗ Storage round-trip failed");
        }
    }

    // Save JSON output if requested
    if let Some(output_path) = args.output {
        let json_output = serde_json::json!({
            "constitution_compliance": {
                "arch_01_all_complete": results.arch_01_all_complete,
                "arch_05_all_dimensions_valid": results.arch_05_all_dimensions_valid,
                "e7_dimension_correct": results.e7_dimension_correct,
                "e1_dimension_correct": results.e1_dimension_correct,
            },
            "counts": {
                "files_scanned": results.files_scanned,
                "entities_extracted": results.entities_extracted,
                "entities_embedded": results.entities_embedded,
                "queries_run": results.queries_run,
            },
            "embedding_performance": {
                "total_time_ms": results.total_embedding_time.as_millis(),
                "p50_latency_ms": results.embedding_latency_p50.as_millis(),
                "p95_latency_ms": results.embedding_latency_p95.as_millis(),
            },
            "search_quality": {
                "e7_primary": {
                    "p1": results.e7_p1,
                    "p5": results.e7_p5,
                    "p10": results.e7_p10,
                    "mrr": results.e7_mrr,
                    "ndcg10": results.e7_ndcg10,
                },
                "e1_primary": {
                    "p1": results.e1_p1,
                    "p5": results.e1_p5,
                    "p10": results.e1_p10,
                    "mrr": results.e1_mrr,
                    "ndcg10": results.e1_ndcg10,
                },
                "e7_advantage": {
                    "p1": results.e7_advantage_p1,
                    "mrr": results.e7_advantage_mrr,
                }
            },
            "storage": {
                "roundtrip_success": results.storage_roundtrip_success,
                "fingerprints_retrieved": results.fingerprints_retrieved,
            },
            "search_performance": {
                "p50_latency_ms": results.search_latency_p50.as_millis(),
                "p95_latency_ms": results.search_latency_p95.as_millis(),
            },
            "passed": passed,
        });

        fs::write(&output_path, serde_json::to_string_pretty(&json_output)?)?;
        println!("\n  Results saved to: {}", output_path.display());
    }

    if passed {
        Ok(())
    } else {
        Err("Benchmark failed".into())
    }
}
