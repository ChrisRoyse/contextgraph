//! Causal Discovery MCP Tool Benchmark
//!
//! Benchmarks the newly implemented `trigger_causal_discovery` and
//! `get_causal_discovery_status` MCP tools.
//!
//! ## Usage
//!
//! ```bash
//! # Full benchmark with real GPU embeddings:
//! cargo run -p context-graph-benchmark --bin causal-discovery-mcp-bench --release \
//!     --features real-embeddings
//!
//! # Quick test with limited memories:
//! cargo run -p context-graph-benchmark --bin causal-discovery-mcp-bench --release \
//!     --features real-embeddings -- --max-memories 20 --max-pairs 10
//! ```
//!
//! ## Metrics Collected
//!
//! - `trigger_causal_discovery` latency (dry run vs actual)
//! - `get_causal_discovery_status` response time
//! - GraphDiscoveryService throughput
//! - Memory fetching overhead
//! - LLM inference latency per pair
//! - Edge creation rate

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

// ============================================================================
// CLI Arguments
// ============================================================================

#[derive(Debug)]
struct Args {
    output_path: PathBuf,
    max_memories: usize,
    max_pairs: usize,
    min_confidence: f32,
    num_iterations: usize,
    dry_run_only: bool,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            output_path: PathBuf::from("benchmark_results/causal_discovery_mcp_bench.json"),
            max_memories: 50,
            max_pairs: 25,
            min_confidence: 0.7,
            num_iterations: 3,
            dry_run_only: false,
        }
    }
}

fn parse_args() -> Args {
    let mut args = Args::default();
    let mut argv = std::env::args().skip(1);

    while let Some(arg) = argv.next() {
        match arg.as_str() {
            "--output" | "-o" => {
                args.output_path = PathBuf::from(argv.next().expect("--output requires a value"));
            }
            "--max-memories" | "-m" => {
                args.max_memories = argv
                    .next()
                    .expect("--max-memories requires a value")
                    .parse()
                    .expect("--max-memories must be a number");
            }
            "--max-pairs" | "-p" => {
                args.max_pairs = argv
                    .next()
                    .expect("--max-pairs requires a value")
                    .parse()
                    .expect("--max-pairs must be a number");
            }
            "--min-confidence" => {
                args.min_confidence = argv
                    .next()
                    .expect("--min-confidence requires a value")
                    .parse()
                    .expect("--min-confidence must be a number");
            }
            "--iterations" | "-n" => {
                args.num_iterations = argv
                    .next()
                    .expect("--iterations requires a value")
                    .parse()
                    .expect("--iterations must be a number");
            }
            "--dry-run-only" => {
                args.dry_run_only = true;
            }
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            _ => {
                eprintln!("Unknown argument: {}", arg);
                print_usage();
                std::process::exit(1);
            }
        }
    }

    args
}

fn print_usage() {
    eprintln!(
        r#"
Causal Discovery MCP Tool Benchmark

USAGE:
    causal-discovery-mcp-bench [OPTIONS]

OPTIONS:
    --output, -o <PATH>         Output path for results JSON
    --max-memories, -m <NUM>    Maximum memories to create (default: 50)
    --max-pairs, -p <NUM>       Maximum pairs to analyze (default: 25)
    --min-confidence <NUM>      Minimum confidence threshold (default: 0.7)
    --iterations, -n <NUM>      Number of benchmark iterations (default: 3)
    --dry-run-only              Only run dry-run benchmarks (no LLM)
    --help, -h                  Show this help message

NOTE:
    This benchmark requires --features real-embeddings and a CUDA GPU.

EXAMPLE:
    cargo run -p context-graph-benchmark --bin causal-discovery-mcp-bench --release \
        --features real-embeddings -- --max-memories 30 --max-pairs 15
"#
    );
}

// ============================================================================
// Result Structures
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalDiscoveryBenchResults {
    pub timestamp: String,
    pub config: BenchmarkConfig,
    pub trigger_discovery: TriggerDiscoveryMetrics,
    pub get_status: GetStatusMetrics,
    pub graph_service: GraphServiceMetrics,
    pub performance_targets: PerformanceTargets,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkConfig {
    pub max_memories: usize,
    pub max_pairs: usize,
    pub min_confidence: f32,
    pub num_iterations: usize,
    pub dry_run_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerDiscoveryMetrics {
    /// Dry run latency (no LLM, just parameter validation and status)
    pub dry_run_latency_ms: LatencyStats,
    /// Full execution latency (with LLM analysis)
    pub full_execution_latency_ms: LatencyStats,
    /// Memory fetching overhead
    pub memory_fetch_overhead_ms: f64,
    /// Pairs analyzed per iteration
    pub pairs_analyzed: Vec<usize>,
    /// Relationships found per iteration
    pub relationships_found: Vec<usize>,
    /// Average confidence of found relationships
    pub avg_confidence: f64,
    /// Error count
    pub errors: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetStatusMetrics {
    /// Response time for get_causal_discovery_status
    pub latency_ms: LatencyStats,
    /// Response size in bytes
    pub response_size_bytes: usize,
    /// Fields included in response
    pub fields_returned: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphServiceMetrics {
    /// Discovery cycle duration
    pub cycle_duration_ms: LatencyStats,
    /// LLM inference time per pair
    pub llm_inference_per_pair_ms: LatencyStats,
    /// Edge creation rate (edges per second)
    pub edge_creation_rate: f64,
    /// Embedding generation rate (embeddings per second)
    pub embedding_generation_rate: f64,
    /// Scanner heuristic hit rate
    pub scanner_hit_rate: f64,
    /// Activator confirmation rate
    pub activator_confirmation_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LatencyStats {
    pub min_ms: f64,
    pub max_ms: f64,
    pub avg_ms: f64,
    pub median_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub samples: usize,
}

impl LatencyStats {
    fn from_samples(samples: &[f64]) -> Self {
        if samples.is_empty() {
            return Self::default();
        }

        let mut sorted = samples.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let len = sorted.len();
        let sum: f64 = sorted.iter().sum();

        Self {
            min_ms: sorted[0],
            max_ms: sorted[len - 1],
            avg_ms: sum / len as f64,
            median_ms: sorted[len / 2],
            p95_ms: sorted[(len as f64 * 0.95) as usize],
            p99_ms: sorted[((len as f64 * 0.99) as usize).min(len - 1)],
            samples: len,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceTargets {
    /// Target: dry_run < 50ms
    pub dry_run_target_met: bool,
    /// Target: get_status < 10ms
    pub get_status_target_met: bool,
    /// Target: full_execution < 5000ms per 10 pairs
    pub full_execution_target_met: bool,
    /// Target: LLM inference < 500ms per pair
    pub llm_inference_target_met: bool,
}

// ============================================================================
// Test Corpus
// ============================================================================

const CAUSAL_CORPUS: &[(&str, &str)] = &[
    // Cause-effect pairs (should be detected)
    ("cause", "High memory usage causes the garbage collector to run more frequently, which leads to increased CPU overhead and application latency spikes."),
    ("effect", "The application experienced latency spikes because the garbage collector was running frequently due to high memory pressure."),
    ("cause", "Database connection pool exhaustion results in query timeouts and failed transactions, ultimately causing service degradation."),
    ("effect", "Service degradation was observed after multiple query timeouts, traced back to connection pool exhaustion."),
    ("cause", "Improper input validation allows SQL injection attacks, which can lead to data breaches and unauthorized access."),
    ("effect", "The data breach occurred through SQL injection, enabled by improper input validation in the login form."),

    // Technical documentation (mixed relationships)
    ("doc", "The async runtime schedules tasks on worker threads. When a task yields, another task can execute."),
    ("doc", "Rust's ownership system prevents data races at compile time. The borrow checker ensures exclusive mutable access."),
    ("doc", "HTTP/2 multiplexes multiple requests over a single TCP connection, reducing latency from connection establishment."),

    // Code-related content
    ("code", "The function parse_config() reads configuration from TOML files and returns a validated Config struct."),
    ("code", "Error handling uses the Result type with custom error variants for different failure modes."),
    ("code", "The cache layer implements LRU eviction with configurable TTL for each entry type."),

    // More causal pairs
    ("cause", "Network partition between data centers triggers failover to the secondary region."),
    ("effect", "Failover to secondary region was triggered by network partition detection."),
    ("cause", "Insufficient logging makes debugging production issues extremely difficult."),
    ("effect", "Debugging the production issue took days because of insufficient logging."),
];

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() {
    let args = parse_args();

    println!("╔═══════════════════════════════════════════════════════════════════════════════╗");
    println!("║          Causal Discovery MCP Tool Benchmark                                  ║");
    println!("║          trigger_causal_discovery + get_causal_discovery_status               ║");
    println!("╚═══════════════════════════════════════════════════════════════════════════════╝\n");

    println!("Configuration:");
    println!("  Max Memories: {}", args.max_memories);
    println!("  Max Pairs: {}", args.max_pairs);
    println!("  Min Confidence: {}", args.min_confidence);
    println!("  Iterations: {}", args.num_iterations);
    println!("  Dry Run Only: {}", args.dry_run_only);
    println!();

    #[cfg(not(feature = "real-embeddings"))]
    {
        eprintln!("This benchmark requires --features real-embeddings");
        eprintln!("Run with: cargo run -p context-graph-benchmark --bin causal-discovery-mcp-bench --release --features real-embeddings");
        std::process::exit(1);
    }

    #[cfg(feature = "real-embeddings")]
    {
        match run_benchmark(&args).await {
            Ok(results) => {
                // Save results
                if let Err(e) = save_results(&results, &args.output_path) {
                    eprintln!("Failed to save results: {}", e);
                }

                // Print summary
                print_summary(&results);

                // Check targets
                let success = check_performance_targets(&results);
                std::process::exit(if success { 0 } else { 1 });
            }
            Err(e) => {
                eprintln!("Benchmark failed: {}", e);
                std::process::exit(1);
            }
        }
    }
}

#[cfg(feature = "real-embeddings")]
async fn run_benchmark(
    args: &Args,
) -> Result<CausalDiscoveryBenchResults, Box<dyn std::error::Error>> {
    use context_graph_causal_agent::llm::CausalDiscoveryLLM;
    use context_graph_core::monitoring::{LayerStatusProvider, StubLayerStatusProvider};
    use context_graph_core::traits::TeleologicalMemoryStore;
    use context_graph_embeddings::{get_warm_provider, initialize_global_warm_provider};
    use context_graph_graph_agent::GraphDiscoveryService;
    use context_graph_mcp::handlers::Handlers;
    use context_graph_mcp::protocol::{JsonRpcId, JsonRpcRequest};
    use context_graph_storage::teleological::RocksDbTeleologicalStore;
    use tempfile::TempDir;

    // ========================================================================
    // Phase 1: Initialize MCP Handlers
    // ========================================================================
    println!("Phase 1: Initializing MCP handlers with GPU embeddings...");
    let init_start = Instant::now();

    // Initialize global warm provider (loads all 13 models)
    initialize_global_warm_provider().await?;
    let multi_array_provider = get_warm_provider()?;

    // Create temporary RocksDB store
    let tempdir = TempDir::new()?;
    let db_path = tempdir.path().join("causal_discovery_bench_db");
    let rocksdb_store = RocksDbTeleologicalStore::open(&db_path)?;
    let teleological_store: Arc<dyn TeleologicalMemoryStore> = Arc::new(rocksdb_store);

    let layer_status_provider: Arc<dyn LayerStatusProvider> = Arc::new(StubLayerStatusProvider);

    // Create CausalDiscoveryLLM and GraphDiscoveryService
    println!("  Creating CausalDiscoveryLLM (requires ~6GB VRAM)...");
    let llm = Arc::new(CausalDiscoveryLLM::new()?);
    println!("  Loading LLM model...");
    llm.load().await?;
    println!("  LLM loaded successfully");

    // Create GraphDiscoveryService with the LLM
    let graph_discovery_service = Arc::new(GraphDiscoveryService::new(llm));

    // Create MCP handlers
    let handlers = Handlers::with_defaults(
        teleological_store.clone(),
        multi_array_provider.clone(),
        layer_status_provider,
        graph_discovery_service,
    )?;

    println!(
        "  Handlers initialized in {:.2}s",
        init_start.elapsed().as_secs_f64()
    );

    // ========================================================================
    // Phase 2: Inject Test Corpus
    // ========================================================================
    println!("\nPhase 2: Injecting test corpus via MCP inject_context...");
    let inject_start = Instant::now();

    let corpus_size = args.max_memories.min(CAUSAL_CORPUS.len());
    let mut doc_ids: Vec<Uuid> = Vec::new();

    for (i, (doc_type, text)) in CAUSAL_CORPUS.iter().take(corpus_size).enumerate() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "tools/call".to_string(),
            id: Some(JsonRpcId::Number(i as i64)),
            params: Some(json!({
                "name": "inject_context",
                "arguments": {
                    "content": text,
                    "rationale": format!("Causal discovery test doc {} (type: {})", i, doc_type),
                    "importance": 0.6
                }
            })),
        };

        let response = handlers.dispatch(request).await;
        if let Some(result) = response.result {
            if let Some(content) = result.get("content").and_then(|v| v.as_array()) {
                if let Some(text_obj) = content.first() {
                    if let Some(text_str) = text_obj.get("text").and_then(|v| v.as_str()) {
                        let data: serde_json::Value = serde_json::from_str(text_str)?;
                        if let Some(fp_id) = data.get("fingerprintId").and_then(|v| v.as_str()) {
                            doc_ids.push(Uuid::parse_str(fp_id)?);
                        }
                    }
                }
            }
        }

        if (i + 1) % 5 == 0 || i == corpus_size - 1 {
            print!("\r  Injected {}/{} documents", i + 1, corpus_size);
            std::io::stdout().flush()?;
        }
    }
    println!();
    println!(
        "  Injection complete in {:.2}s ({} docs)",
        inject_start.elapsed().as_secs_f64(),
        doc_ids.len()
    );

    // ========================================================================
    // Phase 3: Benchmark get_causal_discovery_status
    // ========================================================================
    println!("\nPhase 3: Benchmarking get_causal_discovery_status...");

    let mut status_latencies: Vec<f64> = Vec::new();
    let mut status_response_size = 0usize;
    let mut status_fields: Vec<String> = Vec::new();

    for i in 0..args.num_iterations {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "tools/call".to_string(),
            id: Some(JsonRpcId::Number(1000 + i as i64)),
            params: Some(json!({
                "name": "get_causal_discovery_status",
                "arguments": {
                    "includeLastResult": true,
                    "includeGraphStats": true
                }
            })),
        };

        let start = Instant::now();
        let response = handlers.dispatch(request).await;
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        status_latencies.push(elapsed);

        if let Some(result) = &response.result {
            let json_str = serde_json::to_string(result)?;
            status_response_size = json_str.len();

            // Extract fields on first iteration
            if i == 0 {
                if let Some(content) = result.get("content").and_then(|v| v.as_array()) {
                    if let Some(text_obj) = content.first() {
                        if let Some(text_str) = text_obj.get("text").and_then(|v| v.as_str()) {
                            let data: serde_json::Value = serde_json::from_str(text_str)?;
                            if let Some(obj) = data.as_object() {
                                status_fields = obj.keys().cloned().collect();
                            }
                        }
                    }
                }
            }
        }
    }

    let get_status_metrics = GetStatusMetrics {
        latency_ms: LatencyStats::from_samples(&status_latencies),
        response_size_bytes: status_response_size,
        fields_returned: status_fields,
    };

    println!(
        "  Status latency: avg={:.2}ms, p95={:.2}ms",
        get_status_metrics.latency_ms.avg_ms, get_status_metrics.latency_ms.p95_ms
    );

    // ========================================================================
    // Phase 4: Benchmark trigger_causal_discovery (Dry Run)
    // ========================================================================
    println!("\nPhase 4: Benchmarking trigger_causal_discovery (dry run)...");

    let mut dry_run_latencies: Vec<f64> = Vec::new();

    for i in 0..args.num_iterations {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "tools/call".to_string(),
            id: Some(JsonRpcId::Number(2000 + i as i64)),
            params: Some(json!({
                "name": "trigger_causal_discovery",
                "arguments": {
                    "maxPairs": args.max_pairs,
                    "minConfidence": args.min_confidence,
                    "sessionScope": "all",
                    "similarityThreshold": 0.5,
                    "skipAnalyzed": true,
                    "dryRun": true
                }
            })),
        };

        let start = Instant::now();
        // Response intentionally consumed for latency measurement only
        let _ = handlers.dispatch(request).await;
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        dry_run_latencies.push(elapsed);
    }

    println!(
        "  Dry run latency: avg={:.2}ms, p95={:.2}ms",
        LatencyStats::from_samples(&dry_run_latencies).avg_ms,
        LatencyStats::from_samples(&dry_run_latencies).p95_ms
    );

    // ========================================================================
    // Phase 5: Benchmark trigger_causal_discovery (Full Execution)
    // ========================================================================
    let mut full_latencies: Vec<f64> = Vec::new();
    let mut pairs_analyzed: Vec<usize> = Vec::new();
    let mut relationships_found: Vec<usize> = Vec::new();
    let mut total_errors = 0usize;
    let memory_fetch_times: Vec<f64> = Vec::new();

    if !args.dry_run_only {
        println!("\nPhase 5: Benchmarking trigger_causal_discovery (full execution)...");

        for i in 0..args.num_iterations {
            let request = JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                method: "tools/call".to_string(),
                id: Some(JsonRpcId::Number(3000 + i as i64)),
                params: Some(json!({
                    "name": "trigger_causal_discovery",
                    "arguments": {
                        "maxPairs": args.max_pairs,
                        "minConfidence": args.min_confidence,
                        "sessionScope": "all",
                        "similarityThreshold": 0.5,
                        "skipAnalyzed": false,
                        "dryRun": false
                    }
                })),
            };

            let start = Instant::now();
            let response = handlers.dispatch(request).await;
            let elapsed = start.elapsed().as_secs_f64() * 1000.0;
            full_latencies.push(elapsed);

            // Parse response
            if let Some(result) = &response.result {
                if let Some(content) = result.get("content").and_then(|v| v.as_array()) {
                    if let Some(text_obj) = content.first() {
                        if let Some(text_str) = text_obj.get("text").and_then(|v| v.as_str()) {
                            let data: serde_json::Value = serde_json::from_str(text_str)?;

                            if let Some(pa) = data.get("pairsAnalyzed").and_then(|v| v.as_u64()) {
                                pairs_analyzed.push(pa as usize);
                            }
                            if let Some(rf) =
                                data.get("relationshipsFound").and_then(|v| v.as_u64())
                            {
                                relationships_found.push(rf as usize);
                            }
                            if let Some(errs) = data.get("errors").and_then(|v| v.as_u64()) {
                                total_errors += errs as usize;
                            }
                        }
                    }
                }
            }

            println!(
                "  Iteration {}: {:.0}ms, {} pairs, {} relationships",
                i + 1,
                elapsed,
                pairs_analyzed.last().unwrap_or(&0),
                relationships_found.last().unwrap_or(&0)
            );
        }
    } else {
        println!("\nPhase 5: Skipped (--dry-run-only)");
    }

    // ========================================================================
    // Phase 6: Compute Metrics
    // ========================================================================
    println!("\nPhase 6: Computing metrics...");

    let avg_confidence = if !relationships_found.is_empty() {
        relationships_found.iter().sum::<usize>() as f64 / relationships_found.len() as f64
    } else {
        0.0
    };

    let trigger_metrics = TriggerDiscoveryMetrics {
        dry_run_latency_ms: LatencyStats::from_samples(&dry_run_latencies),
        full_execution_latency_ms: LatencyStats::from_samples(&full_latencies),
        memory_fetch_overhead_ms: if memory_fetch_times.is_empty() {
            0.0
        } else {
            memory_fetch_times.iter().sum::<f64>() / memory_fetch_times.len() as f64
        },
        pairs_analyzed,
        relationships_found,
        avg_confidence,
        errors: total_errors,
    };

    // Graph service metrics (derived)
    let total_pairs: usize = trigger_metrics.pairs_analyzed.iter().sum();
    let total_relationships: usize = trigger_metrics.relationships_found.iter().sum();
    let total_time_ms: f64 = full_latencies.iter().sum();

    let graph_metrics = GraphServiceMetrics {
        cycle_duration_ms: LatencyStats::from_samples(&full_latencies),
        llm_inference_per_pair_ms: if total_pairs > 0 && !full_latencies.is_empty() {
            let per_pair: Vec<f64> = full_latencies
                .iter()
                .zip(trigger_metrics.pairs_analyzed.iter())
                .filter(|(_, &pairs)| pairs > 0)
                .map(|(&time, &pairs)| time / pairs as f64)
                .collect();
            LatencyStats::from_samples(&per_pair)
        } else {
            LatencyStats::default()
        },
        edge_creation_rate: if total_time_ms > 0.0 {
            total_relationships as f64 / (total_time_ms / 1000.0)
        } else {
            0.0
        },
        embedding_generation_rate: if total_time_ms > 0.0 {
            (total_relationships * 2) as f64 / (total_time_ms / 1000.0) // 2 embeddings per relationship
        } else {
            0.0
        },
        scanner_hit_rate: 0.0, // Would need scanner stats
        activator_confirmation_rate: if total_pairs > 0 {
            total_relationships as f64 / total_pairs as f64
        } else {
            0.0
        },
    };

    // Performance targets
    let targets = PerformanceTargets {
        dry_run_target_met: trigger_metrics.dry_run_latency_ms.p95_ms < 50.0,
        get_status_target_met: get_status_metrics.latency_ms.p95_ms < 10.0,
        full_execution_target_met: trigger_metrics.full_execution_latency_ms.avg_ms < 5000.0,
        llm_inference_target_met: graph_metrics.llm_inference_per_pair_ms.avg_ms < 500.0,
    };

    // Generate recommendations
    let mut recommendations = Vec::new();
    if !targets.dry_run_target_met {
        recommendations.push(format!(
            "Dry run p95 latency ({:.0}ms) exceeds 50ms target. Check parameter validation.",
            trigger_metrics.dry_run_latency_ms.p95_ms
        ));
    }
    if !targets.get_status_target_met {
        recommendations.push(format!(
            "get_status p95 latency ({:.0}ms) exceeds 10ms target. Consider caching status.",
            get_status_metrics.latency_ms.p95_ms
        ));
    }
    if !targets.full_execution_target_met && !args.dry_run_only {
        recommendations.push(format!(
            "Full execution avg latency ({:.0}ms) exceeds 5000ms target. Reduce batch size or optimize LLM.",
            trigger_metrics.full_execution_latency_ms.avg_ms
        ));
    }
    if !targets.llm_inference_target_met && !args.dry_run_only {
        recommendations.push(format!(
            "LLM inference per pair ({:.0}ms) exceeds 500ms target. Consider smaller model or GPU upgrade.",
            graph_metrics.llm_inference_per_pair_ms.avg_ms
        ));
    }
    if recommendations.is_empty() {
        recommendations.push("All performance targets met!".to_string());
    }

    let results = CausalDiscoveryBenchResults {
        timestamp: Utc::now().to_rfc3339(),
        config: BenchmarkConfig {
            max_memories: args.max_memories,
            max_pairs: args.max_pairs,
            min_confidence: args.min_confidence,
            num_iterations: args.num_iterations,
            dry_run_only: args.dry_run_only,
        },
        trigger_discovery: trigger_metrics,
        get_status: get_status_metrics,
        graph_service: graph_metrics,
        performance_targets: targets,
        recommendations,
    };

    // Cleanup
    drop(tempdir);

    Ok(results)
}

fn save_results(results: &CausalDiscoveryBenchResults, path: &PathBuf) -> std::io::Result<()> {
    // Ensure directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let file = File::create(path)?;
    serde_json::to_writer_pretty(file, results)?;
    println!("\nResults saved to: {:?}", path);

    // Also save markdown report
    let md_path = path.with_extension("md");
    let mut md_file = File::create(&md_path)?;

    writeln!(md_file, "# Causal Discovery MCP Tool Benchmark Report")?;
    writeln!(md_file)?;
    writeln!(md_file, "**Generated:** {}", results.timestamp)?;
    writeln!(md_file)?;
    writeln!(md_file, "## Configuration")?;
    writeln!(md_file)?;
    writeln!(md_file, "| Parameter | Value |")?;
    writeln!(md_file, "|-----------|-------|")?;
    writeln!(
        md_file,
        "| Max Memories | {} |",
        results.config.max_memories
    )?;
    writeln!(md_file, "| Max Pairs | {} |", results.config.max_pairs)?;
    writeln!(
        md_file,
        "| Min Confidence | {} |",
        results.config.min_confidence
    )?;
    writeln!(
        md_file,
        "| Iterations | {} |",
        results.config.num_iterations
    )?;
    writeln!(md_file)?;
    writeln!(md_file, "## trigger_causal_discovery Performance")?;
    writeln!(md_file)?;
    writeln!(md_file, "| Metric | Dry Run | Full Execution | Target |")?;
    writeln!(md_file, "|--------|---------|----------------|--------|")?;
    writeln!(
        md_file,
        "| Avg Latency | {:.1}ms | {:.1}ms | <5000ms |",
        results.trigger_discovery.dry_run_latency_ms.avg_ms,
        results.trigger_discovery.full_execution_latency_ms.avg_ms
    )?;
    writeln!(
        md_file,
        "| P95 Latency | {:.1}ms | {:.1}ms | - |",
        results.trigger_discovery.dry_run_latency_ms.p95_ms,
        results.trigger_discovery.full_execution_latency_ms.p95_ms
    )?;
    writeln!(md_file)?;
    writeln!(md_file, "## get_causal_discovery_status Performance")?;
    writeln!(md_file)?;
    writeln!(md_file, "| Metric | Value | Target |")?;
    writeln!(md_file, "|--------|-------|--------|")?;
    writeln!(
        md_file,
        "| Avg Latency | {:.1}ms | <10ms |",
        results.get_status.latency_ms.avg_ms
    )?;
    writeln!(
        md_file,
        "| P95 Latency | {:.1}ms | - |",
        results.get_status.latency_ms.p95_ms
    )?;
    writeln!(
        md_file,
        "| Response Size | {} bytes | - |",
        results.get_status.response_size_bytes
    )?;
    writeln!(md_file)?;
    writeln!(md_file, "## Performance Targets")?;
    writeln!(md_file)?;
    writeln!(md_file, "| Target | Status |")?;
    writeln!(md_file, "|--------|--------|")?;
    writeln!(
        md_file,
        "| Dry Run < 50ms | {} |",
        if results.performance_targets.dry_run_target_met {
            "PASS"
        } else {
            "FAIL"
        }
    )?;
    writeln!(
        md_file,
        "| Get Status < 10ms | {} |",
        if results.performance_targets.get_status_target_met {
            "PASS"
        } else {
            "FAIL"
        }
    )?;
    writeln!(
        md_file,
        "| Full Execution < 5000ms | {} |",
        if results.performance_targets.full_execution_target_met {
            "PASS"
        } else {
            "FAIL"
        }
    )?;
    writeln!(
        md_file,
        "| LLM Inference < 500ms/pair | {} |",
        if results.performance_targets.llm_inference_target_met {
            "PASS"
        } else {
            "FAIL"
        }
    )?;
    writeln!(md_file)?;
    writeln!(md_file, "## Recommendations")?;
    writeln!(md_file)?;
    for rec in &results.recommendations {
        writeln!(md_file, "- {}", rec)?;
    }

    println!("Markdown report saved to: {:?}", md_path);
    Ok(())
}

fn print_summary(results: &CausalDiscoveryBenchResults) {
    println!("\n╔═══════════════════════════════════════════════════════════════════════════════╗");
    println!("║                           BENCHMARK RESULTS                                    ║");
    println!("╠═══════════════════════════════════════════════════════════════════════════════╣");
    println!("║ TRIGGER_CAUSAL_DISCOVERY                                                      ║");
    println!(
        "║   Dry Run:        avg={:>6.1}ms  p95={:>6.1}ms  (target: <50ms)               ║",
        results.trigger_discovery.dry_run_latency_ms.avg_ms,
        results.trigger_discovery.dry_run_latency_ms.p95_ms
    );
    println!(
        "║   Full Execution: avg={:>6.1}ms  p95={:>6.1}ms  (target: <5000ms)             ║",
        results.trigger_discovery.full_execution_latency_ms.avg_ms,
        results.trigger_discovery.full_execution_latency_ms.p95_ms
    );
    println!(
        "║   Pairs Analyzed: {:?}                                                        ║",
        results.trigger_discovery.pairs_analyzed
    );
    println!(
        "║   Relationships:  {:?}                                                        ║",
        results.trigger_discovery.relationships_found
    );
    println!("╠═══════════════════════════════════════════════════════════════════════════════╣");
    println!("║ GET_CAUSAL_DISCOVERY_STATUS                                                   ║");
    println!(
        "║   Latency:        avg={:>6.1}ms  p95={:>6.1}ms  (target: <10ms)               ║",
        results.get_status.latency_ms.avg_ms, results.get_status.latency_ms.p95_ms
    );
    println!(
        "║   Response Size:  {} bytes                                                    ║",
        results.get_status.response_size_bytes
    );
    println!("╠═══════════════════════════════════════════════════════════════════════════════╣");
    println!("║ GRAPH SERVICE                                                                 ║");
    println!(
        "║   LLM per pair:   avg={:>6.1}ms  (target: <500ms)                             ║",
        results.graph_service.llm_inference_per_pair_ms.avg_ms
    );
    println!(
        "║   Edge rate:      {:.2} edges/sec                                             ║",
        results.graph_service.edge_creation_rate
    );
    println!(
        "║   Confirmation:   {:.1}%                                                      ║",
        results.graph_service.activator_confirmation_rate * 100.0
    );
    println!("╚═══════════════════════════════════════════════════════════════════════════════╝");
}

fn check_performance_targets(results: &CausalDiscoveryBenchResults) -> bool {
    println!("\n╔═══════════════════════════════════════════════════════════════════════════════╗");
    println!("║                        PERFORMANCE TARGET EVALUATION                           ║");
    println!("╠═══════════════════════════════════════════════════════════════════════════════╣");

    let targets = &results.performance_targets;

    println!(
        "║   Dry Run < 50ms:           {}                                                ║",
        if targets.dry_run_target_met {
            "PASS"
        } else {
            "FAIL"
        }
    );
    println!(
        "║   Get Status < 10ms:        {}                                                ║",
        if targets.get_status_target_met {
            "PASS"
        } else {
            "FAIL"
        }
    );
    println!(
        "║   Full Execution < 5000ms:  {}                                                ║",
        if targets.full_execution_target_met {
            "PASS"
        } else {
            "FAIL"
        }
    );
    println!(
        "║   LLM Inference < 500ms:    {}                                                ║",
        if targets.llm_inference_target_met {
            "PASS"
        } else {
            "FAIL"
        }
    );

    println!("╚═══════════════════════════════════════════════════════════════════════════════╝");

    let all_pass = targets.dry_run_target_met
        && targets.get_status_target_met
        && (targets.full_execution_target_met || results.config.dry_run_only)
        && (targets.llm_inference_target_met || results.config.dry_run_only);

    if all_pass {
        println!("\n[SUCCESS] All performance targets met!");
    } else {
        println!("\n[WARNING] Some performance targets not met. Review recommendations.");
        for rec in &results.recommendations {
            println!("  - {}", rec);
        }
    }

    all_pass
}
