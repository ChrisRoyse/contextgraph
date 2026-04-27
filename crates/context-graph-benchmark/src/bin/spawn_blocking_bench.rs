//! spawn_blocking Benchmark Suite
//!
//! Benchmarks for spawn_blocking operations in the RocksDB store to measure
//! latency, concurrency scaling, and verify async runtime is not blocked.
//!
//! Usage:
//!     cargo run -p context-graph-benchmark --bin spawn-blocking-bench --release --features real-embeddings -- \
//!         --data-path ./data/hf_benchmark_diverse \
//!         --max-chunks 1000 \
//!         --output spawn_blocking_results.json

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use context_graph_benchmark::realdata::embedder::{EmbedderConfig, RealDataEmbedder};
use context_graph_benchmark::realdata::loader::DatasetLoader;
use context_graph_core::traits::{
    SearchStrategy, TeleologicalMemoryStore, TeleologicalSearchOptions,
};
use context_graph_core::types::fingerprint::TeleologicalFingerprint;
use context_graph_storage::teleological::RocksDbTeleologicalStore;

// ============================================================================
// CLI Arguments
// ============================================================================

#[derive(Debug)]
struct Args {
    data_path: PathBuf,
    max_chunks: usize,
    concurrency_levels: Vec<usize>,
    output: PathBuf,
    iterations: usize,
    warmup: usize,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            data_path: PathBuf::from("./data/hf_benchmark_diverse"),
            max_chunks: 1000,
            concurrency_levels: vec![1, 2, 4, 8, 16, 32],
            output: PathBuf::from("spawn_blocking_results.json"),
            iterations: 100,
            warmup: 10,
        }
    }
}

fn parse_args() -> Args {
    let mut args = Args::default();
    let mut argv = std::env::args().skip(1);

    while let Some(arg) = argv.next() {
        match arg.as_str() {
            "--data-path" => {
                args.data_path = PathBuf::from(argv.next().expect("--data-path requires a value"));
            }
            "--max-chunks" | "-n" => {
                args.max_chunks = argv
                    .next()
                    .expect("--max-chunks requires a value")
                    .parse()
                    .expect("--max-chunks must be a number");
            }
            "--concurrency" => {
                let levels_str = argv.next().expect("--concurrency requires a value");
                args.concurrency_levels = levels_str
                    .split(',')
                    .map(|s| {
                        s.trim()
                            .parse()
                            .expect("concurrency level must be a number")
                    })
                    .collect();
            }
            "--output" | "-o" => {
                args.output = PathBuf::from(argv.next().expect("--output requires a value"));
            }
            "--iterations" => {
                args.iterations = argv
                    .next()
                    .expect("--iterations requires a value")
                    .parse()
                    .expect("--iterations must be a number");
            }
            "--warmup" => {
                args.warmup = argv
                    .next()
                    .expect("--warmup requires a value")
                    .parse()
                    .expect("--warmup must be a number");
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
spawn_blocking Benchmark Suite

USAGE:
    spawn-blocking-bench [OPTIONS]

OPTIONS:
    --data-path <PATH>       Path to dataset directory with chunks.jsonl
    --max-chunks, -n <NUM>   Maximum chunks to load (default: 1000)
    --concurrency <LIST>     Comma-separated concurrency levels (default: 1,2,4,8,16,32)
    --output, -o <PATH>      Output path for results JSON
    --iterations <NUM>       Number of iterations per benchmark (default: 100)
    --warmup <NUM>           Number of warmup iterations (default: 10)
    --help, -h               Show this help message

NOTE:
    This benchmark requires --features real-embeddings and a CUDA GPU.
"#
    );
}

// ============================================================================
// Result Structures
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResults {
    pub metadata: Metadata,
    pub operations: HashMap<String, OperationResult>,
    pub blocking_detection: BlockingResult,
    pub summary: Summary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub timestamp: String,
    pub data_size: usize,
    pub iterations: usize,
    pub warmup: usize,
    pub concurrency_levels: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationResult {
    pub baseline: LatencyStats,
    pub concurrent: HashMap<usize, ConcurrentStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LatencyStats {
    pub p50_us: u64,
    pub p90_us: u64,
    pub p99_us: u64,
    pub mean_us: f64,
    pub min_us: u64,
    pub max_us: u64,
}

impl LatencyStats {
    fn from_measurements(latencies: &[u64]) -> Self {
        if latencies.is_empty() {
            return Self::default();
        }

        let mut sorted = latencies.to_vec();
        sorted.sort();

        let len = sorted.len();
        let mean = sorted.iter().sum::<u64>() as f64 / len as f64;

        Self {
            p50_us: sorted[(len * 50 / 100).min(len - 1)],
            p90_us: sorted[(len * 90 / 100).min(len - 1)],
            p99_us: sorted[(len * 99 / 100).min(len - 1)],
            mean_us: mean,
            min_us: sorted[0],
            max_us: sorted[len - 1],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConcurrentStats {
    pub concurrency: usize,
    pub latency: LatencyStats,
    pub throughput_ops_sec: f64,
    pub wall_clock_ms: u64,
    pub scaling_factor: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockingResult {
    pub elapsed_ms: u64,
    pub expected_ticks: usize,
    pub actual_ticks: usize,
    pub tick_ratio: f64,
    pub is_blocking: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    pub optimal_concurrency: usize,
    pub peak_throughput: f64,
    pub blocking_detected: bool,
    pub recommendations: Vec<String>,
}

// ============================================================================
// Main Entry Point
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let args = parse_args();

    println!("=======================================================================");
    println!("  spawn_blocking Benchmark Suite");
    println!("=======================================================================");
    println!();
    println!("Configuration:");
    println!("  Data path: {}", args.data_path.display());
    println!("  Max chunks: {}", args.max_chunks);
    println!("  Concurrency levels: {:?}", args.concurrency_levels);
    println!("  Iterations: {}", args.iterations);
    println!("  Warmup: {}", args.warmup);
    println!();

    // Phase 1: Load and embed data
    let (store, query_fp) = setup_store(&args).await?;
    let store = Arc::new(store);
    println!();

    // Phase 2: Run baseline benchmarks
    println!("PHASE 2: Single-Threaded Baseline");
    println!("{}", "-".repeat(70));
    let mut operations: HashMap<String, OperationResult> = HashMap::new();

    // Benchmark search_semantic
    print!("  Benchmarking search_semantic... ");
    std::io::stdout().flush()?;
    let baseline = benchmark_search_semantic(&store, &query_fp, args.iterations, args.warmup).await;
    println!(
        "P50={:.2}ms, P99={:.2}ms",
        baseline.p50_us as f64 / 1000.0,
        baseline.p99_us as f64 / 1000.0
    );
    operations.insert(
        "search_semantic".to_string(),
        OperationResult {
            baseline,
            concurrent: HashMap::new(),
        },
    );

    // Benchmark retrieve_batch
    print!("  Benchmarking retrieve_batch... ");
    std::io::stdout().flush()?;
    let baseline = benchmark_retrieve_batch(&store, args.iterations, args.warmup).await;
    println!(
        "P50={:.2}ms, P99={:.2}ms",
        baseline.p50_us as f64 / 1000.0,
        baseline.p99_us as f64 / 1000.0
    );
    operations.insert(
        "retrieve_batch".to_string(),
        OperationResult {
            baseline,
            concurrent: HashMap::new(),
        },
    );

    // Benchmark count
    print!("  Benchmarking count... ");
    std::io::stdout().flush()?;
    let baseline = benchmark_count(&store, args.iterations, args.warmup).await;
    println!(
        "P50={:.2}ms, P99={:.2}ms",
        baseline.p50_us as f64 / 1000.0,
        baseline.p99_us as f64 / 1000.0
    );
    operations.insert(
        "count".to_string(),
        OperationResult {
            baseline,
            concurrent: HashMap::new(),
        },
    );

    // Benchmark get_content_batch
    print!("  Benchmarking get_content_batch... ");
    std::io::stdout().flush()?;
    let baseline = benchmark_get_content_batch(&store, args.iterations, args.warmup).await;
    println!(
        "P50={:.2}ms, P99={:.2}ms",
        baseline.p50_us as f64 / 1000.0,
        baseline.p99_us as f64 / 1000.0
    );
    operations.insert(
        "get_content_batch".to_string(),
        OperationResult {
            baseline,
            concurrent: HashMap::new(),
        },
    );

    println!();

    // Phase 3: Run concurrent benchmarks
    println!("PHASE 3: Concurrent Scaling");
    println!("{}", "-".repeat(70));

    for &concurrency in &args.concurrency_levels {
        println!("  Testing concurrency level: {}", concurrency);

        // search_semantic concurrent
        let stats =
            benchmark_search_semantic_concurrent(&store, &query_fp, concurrency, args.iterations)
                .await;
        println!(
            "    search_semantic: throughput={:.0} ops/s, scaling={:.2}x",
            stats.throughput_ops_sec, stats.scaling_factor
        );
        operations
            .get_mut("search_semantic")
            .unwrap()
            .concurrent
            .insert(concurrency, stats);

        // retrieve_batch concurrent
        let stats = benchmark_retrieve_batch_concurrent(&store, concurrency, args.iterations).await;
        println!(
            "    retrieve_batch: throughput={:.0} ops/s, scaling={:.2}x",
            stats.throughput_ops_sec, stats.scaling_factor
        );
        operations
            .get_mut("retrieve_batch")
            .unwrap()
            .concurrent
            .insert(concurrency, stats);

        // count concurrent
        let stats = benchmark_count_concurrent(&store, concurrency, args.iterations).await;
        println!(
            "    count: throughput={:.0} ops/s, scaling={:.2}x",
            stats.throughput_ops_sec, stats.scaling_factor
        );
        operations
            .get_mut("count")
            .unwrap()
            .concurrent
            .insert(concurrency, stats);
    }

    println!();

    // Phase 4: Blocking detection
    println!("PHASE 4: Blocking Detection");
    println!("{}", "-".repeat(70));
    let blocking_result = detect_blocking(&store, &query_fp).await;
    println!("  Elapsed: {}ms", blocking_result.elapsed_ms);
    println!("  Expected ticks: {}", blocking_result.expected_ticks);
    println!("  Actual ticks: {}", blocking_result.actual_ticks);
    println!("  Tick ratio: {:.2}", blocking_result.tick_ratio);
    println!("  Blocking detected: {}", blocking_result.is_blocking);
    println!();

    // Phase 5: Generate results
    let summary = generate_summary(&operations, &blocking_result, &args.concurrency_levels);

    let results = BenchmarkResults {
        metadata: Metadata {
            timestamp: Utc::now().to_rfc3339(),
            data_size: args.max_chunks,
            iterations: args.iterations,
            warmup: args.warmup,
            concurrency_levels: args.concurrency_levels.clone(),
        },
        operations,
        blocking_detection: blocking_result,
        summary,
    };

    // Save results
    save_results(&args, &results)?;

    // Print summary
    print_summary(&results);

    Ok(())
}

// ============================================================================
// Setup: Load Data and Create Store
// ============================================================================

async fn setup_store(
    args: &Args,
) -> Result<(RocksDbTeleologicalStore, TeleologicalFingerprint), Box<dyn std::error::Error>> {
    println!("PHASE 1: Setup");
    println!("{}", "-".repeat(70));

    // Load dataset
    print!("  Loading dataset... ");
    std::io::stdout().flush()?;
    let loader = DatasetLoader::new().with_max_chunks(args.max_chunks);
    let dataset = loader.load_from_dir(&args.data_path)?;
    println!("{} chunks loaded", dataset.chunks.len());

    // Embed dataset
    print!("  Embedding chunks (this may take a while)... ");
    std::io::stdout().flush()?;

    let config = EmbedderConfig {
        batch_size: 32,
        show_progress: false,
        device: "cuda:0".to_string(),
    };

    let embedder = RealDataEmbedder::with_config(config);

    #[cfg(feature = "real-embeddings")]
    let embedded = embedder.embed_dataset(&dataset).await?;

    #[cfg(not(feature = "real-embeddings"))]
    let _embedded: context_graph_benchmark::realdata::embedder::EmbeddedDataset = {
        let _ = embedder;
        eprintln!("\nERROR: This benchmark requires the 'real-embeddings' feature.");
        std::process::exit(1);
    };

    println!("{} fingerprints", embedded.fingerprints.len());

    // Create temp store
    print!("  Creating temp RocksDB store... ");
    std::io::stdout().flush()?;
    let temp_dir = tempfile::TempDir::new()?;
    let store = RocksDbTeleologicalStore::open(temp_dir.path())?;
    println!("OK");

    // Convert and store fingerprints
    print!("  Storing fingerprints... ");
    std::io::stdout().flush()?;

    let mut query_fp: Option<TeleologicalFingerprint> = None;

    for (i, (id, semantic_fp)) in embedded.fingerprints.iter().enumerate() {
        // Use a deterministic hash based on ID for benchmarking (actual hash not important for perf testing)
        let mut content_hash = [0u8; 32];
        content_hash[..16].copy_from_slice(id.as_bytes());

        // Create TeleologicalFingerprint from SemanticFingerprint
        let tp = TeleologicalFingerprint::with_id(*id, semantic_fp.clone(), content_hash);

        // Store content too (for content batch benchmark)
        if let Some(chunk) = dataset.chunks.get(i) {
            store.store_content(*id, &chunk.text).await?;
        }

        store.store(tp.clone()).await?;

        // Keep first fingerprint as query
        if query_fp.is_none() {
            query_fp = Some(tp);
        }
    }

    println!("{} stored", embedded.fingerprints.len());

    let query_fp = query_fp.expect("Need at least one fingerprint");

    Ok((store, query_fp))
}

// ============================================================================
// Baseline Benchmarks
// ============================================================================

async fn benchmark_search_semantic(
    store: &RocksDbTeleologicalStore,
    query: &TeleologicalFingerprint,
    iterations: usize,
    warmup: usize,
) -> LatencyStats {
    let options = TeleologicalSearchOptions {
        top_k: 10,
        strategy: SearchStrategy::E1Only,
        ..Default::default()
    };

    // Warmup
    for _ in 0..warmup {
        let _ = store
            .search_semantic(&query.semantic, options.clone())
            .await;
    }

    // Measure
    let mut latencies = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let start = Instant::now();
        let _ = store
            .search_semantic(&query.semantic, options.clone())
            .await;
        latencies.push(start.elapsed().as_micros() as u64);
    }

    LatencyStats::from_measurements(&latencies)
}

async fn benchmark_retrieve_batch(
    store: &RocksDbTeleologicalStore,
    iterations: usize,
    warmup: usize,
) -> LatencyStats {
    // Get some IDs to retrieve
    let count = store.count().await.unwrap_or(0);
    if count == 0 {
        return LatencyStats::default();
    }

    // We'll create a batch of 10 UUIDs (some may not exist, but that's OK for benchmarking)
    let batch_ids: Vec<Uuid> = (0..10).map(|_| Uuid::new_v4()).collect();

    // Warmup
    for _ in 0..warmup {
        let _ = store.retrieve_batch(&batch_ids).await;
    }

    // Measure
    let mut latencies = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let start = Instant::now();
        let _ = store.retrieve_batch(&batch_ids).await;
        latencies.push(start.elapsed().as_micros() as u64);
    }

    LatencyStats::from_measurements(&latencies)
}

async fn benchmark_count(
    store: &RocksDbTeleologicalStore,
    iterations: usize,
    warmup: usize,
) -> LatencyStats {
    // Warmup (first call populates cache)
    for _ in 0..warmup {
        // Invalidate cache for accurate measurement
        store.invalidate_count_cache();
        let _ = store.count().await;
    }

    // Measure (with cache invalidation each time to measure actual work)
    let mut latencies = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        store.invalidate_count_cache();
        let start = Instant::now();
        let _ = store.count().await;
        latencies.push(start.elapsed().as_micros() as u64);
    }

    LatencyStats::from_measurements(&latencies)
}

async fn benchmark_get_content_batch(
    store: &RocksDbTeleologicalStore,
    iterations: usize,
    warmup: usize,
) -> LatencyStats {
    // Get some IDs to retrieve content for
    let batch_ids: Vec<Uuid> = (0..10).map(|_| Uuid::new_v4()).collect();

    // Warmup
    for _ in 0..warmup {
        let _ = store.get_content_batch(&batch_ids).await;
    }

    // Measure
    let mut latencies = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let start = Instant::now();
        let _ = store.get_content_batch(&batch_ids).await;
        latencies.push(start.elapsed().as_micros() as u64);
    }

    LatencyStats::from_measurements(&latencies)
}

// ============================================================================
// Concurrent Benchmarks
// ============================================================================

async fn benchmark_search_semantic_concurrent(
    store: &Arc<RocksDbTeleologicalStore>,
    query: &TeleologicalFingerprint,
    concurrency: usize,
    total_iterations: usize,
) -> ConcurrentStats {
    let iterations_per_task = total_iterations / concurrency;
    let options = TeleologicalSearchOptions {
        top_k: 10,
        strategy: SearchStrategy::E1Only,
        ..Default::default()
    };

    let wall_start = Instant::now();

    let handles: Vec<_> = (0..concurrency)
        .map(|_| {
            let store = Arc::clone(store);
            let query_semantic = query.semantic.clone();
            let opts = options.clone();

            tokio::spawn(async move {
                let mut latencies = Vec::with_capacity(iterations_per_task);
                for _ in 0..iterations_per_task {
                    let start = Instant::now();
                    let _ = store.search_semantic(&query_semantic, opts.clone()).await;
                    latencies.push(start.elapsed().as_micros() as u64);
                }
                latencies
            })
        })
        .collect();

    let all_latencies: Vec<u64> = futures::future::join_all(handles)
        .await
        .into_iter()
        .filter_map(|r| r.ok())
        .flatten()
        .collect();

    let wall_clock_ms = wall_start.elapsed().as_millis() as u64;
    let throughput = all_latencies.len() as f64 / (wall_clock_ms as f64 / 1000.0);

    // Calculate scaling factor (vs single-threaded baseline)
    let baseline_throughput =
        1000.0 * 1000.0 / LatencyStats::from_measurements(&all_latencies).mean_us;
    let scaling_factor = throughput / baseline_throughput.max(1.0);

    ConcurrentStats {
        concurrency,
        latency: LatencyStats::from_measurements(&all_latencies),
        throughput_ops_sec: throughput,
        wall_clock_ms,
        scaling_factor,
    }
}

async fn benchmark_retrieve_batch_concurrent(
    store: &Arc<RocksDbTeleologicalStore>,
    concurrency: usize,
    total_iterations: usize,
) -> ConcurrentStats {
    let iterations_per_task = total_iterations / concurrency;
    let batch_ids: Vec<Uuid> = (0..10).map(|_| Uuid::new_v4()).collect();

    let wall_start = Instant::now();

    let handles: Vec<_> = (0..concurrency)
        .map(|_| {
            let store = Arc::clone(store);
            let ids = batch_ids.clone();

            tokio::spawn(async move {
                let mut latencies = Vec::with_capacity(iterations_per_task);
                for _ in 0..iterations_per_task {
                    let start = Instant::now();
                    let _ = store.retrieve_batch(&ids).await;
                    latencies.push(start.elapsed().as_micros() as u64);
                }
                latencies
            })
        })
        .collect();

    let all_latencies: Vec<u64> = futures::future::join_all(handles)
        .await
        .into_iter()
        .filter_map(|r| r.ok())
        .flatten()
        .collect();

    let wall_clock_ms = wall_start.elapsed().as_millis() as u64;
    let throughput = all_latencies.len() as f64 / (wall_clock_ms as f64 / 1000.0);

    let baseline_throughput =
        1000.0 * 1000.0 / LatencyStats::from_measurements(&all_latencies).mean_us;
    let scaling_factor = throughput / baseline_throughput.max(1.0);

    ConcurrentStats {
        concurrency,
        latency: LatencyStats::from_measurements(&all_latencies),
        throughput_ops_sec: throughput,
        wall_clock_ms,
        scaling_factor,
    }
}

async fn benchmark_count_concurrent(
    store: &Arc<RocksDbTeleologicalStore>,
    concurrency: usize,
    total_iterations: usize,
) -> ConcurrentStats {
    let iterations_per_task = total_iterations / concurrency;

    let wall_start = Instant::now();

    let handles: Vec<_> = (0..concurrency)
        .map(|_| {
            let store = Arc::clone(store);

            tokio::spawn(async move {
                let mut latencies = Vec::with_capacity(iterations_per_task);
                for _ in 0..iterations_per_task {
                    store.invalidate_count_cache();
                    let start = Instant::now();
                    let _ = store.count().await;
                    latencies.push(start.elapsed().as_micros() as u64);
                }
                latencies
            })
        })
        .collect();

    let all_latencies: Vec<u64> = futures::future::join_all(handles)
        .await
        .into_iter()
        .filter_map(|r| r.ok())
        .flatten()
        .collect();

    let wall_clock_ms = wall_start.elapsed().as_millis() as u64;
    let throughput = all_latencies.len() as f64 / (wall_clock_ms as f64 / 1000.0);

    let baseline_throughput =
        1000.0 * 1000.0 / LatencyStats::from_measurements(&all_latencies).mean_us;
    let scaling_factor = throughput / baseline_throughput.max(1.0);

    ConcurrentStats {
        concurrency,
        latency: LatencyStats::from_measurements(&all_latencies),
        throughput_ops_sec: throughput,
        wall_clock_ms,
        scaling_factor,
    }
}

// ============================================================================
// Blocking Detection
// ============================================================================

async fn detect_blocking(
    store: &RocksDbTeleologicalStore,
    query: &TeleologicalFingerprint,
) -> BlockingResult {
    let (tx, mut rx) = tokio::sync::mpsc::channel(1000);

    let options = TeleologicalSearchOptions {
        top_k: 10,
        strategy: SearchStrategy::E1Only,
        ..Default::default()
    };

    // Ticker task - should tick every 1ms if runtime not blocked
    let ticker = tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(1)).await;
            if tx.send(()).await.is_err() {
                break;
            }
        }
    });

    // Run 100 blocking operations
    let start = Instant::now();
    for _ in 0..100 {
        let _ = store
            .search_semantic(&query.semantic, options.clone())
            .await;
    }
    let elapsed = start.elapsed();

    ticker.abort();

    // Count ticks received
    let mut ticks = 0;
    while rx.try_recv().is_ok() {
        ticks += 1;
    }

    let expected = elapsed.as_millis() as usize;
    let ratio = ticks as f64 / expected.max(1) as f64;

    BlockingResult {
        elapsed_ms: elapsed.as_millis() as u64,
        expected_ticks: expected,
        actual_ticks: ticks,
        tick_ratio: ratio,
        is_blocking: ratio < 0.5,
    }
}

// ============================================================================
// Results Generation
// ============================================================================

fn generate_summary(
    operations: &HashMap<String, OperationResult>,
    blocking: &BlockingResult,
    concurrency_levels: &[usize],
) -> Summary {
    let mut recommendations = Vec::new();

    // Find optimal concurrency (highest throughput for search_semantic)
    let mut optimal_concurrency = 1;
    let mut peak_throughput = 0.0;

    if let Some(search_result) = operations.get("search_semantic") {
        for &level in concurrency_levels {
            if let Some(stats) = search_result.concurrent.get(&level) {
                if stats.throughput_ops_sec > peak_throughput {
                    peak_throughput = stats.throughput_ops_sec;
                    optimal_concurrency = level;
                }
            }
        }
    }

    // Check scaling
    if let Some(search_result) = operations.get("search_semantic") {
        if let Some(stats) = search_result.concurrent.get(&4) {
            if stats.scaling_factor < 1.5 {
                recommendations.push(
                    "Warning: Scaling factor at concurrency=4 is below 1.5x. \
                     Consider checking for contention issues."
                        .to_string(),
                );
            } else {
                recommendations.push(format!(
                    "Good scaling: {:.2}x at concurrency=4",
                    stats.scaling_factor
                ));
            }
        }
    }

    // Check blocking
    if blocking.is_blocking {
        recommendations.push(
            "WARNING: Blocking detected! spawn_blocking may not be working correctly.".to_string(),
        );
    } else {
        recommendations.push(format!(
            "spawn_blocking working correctly (tick ratio: {:.2})",
            blocking.tick_ratio
        ));
    }

    // Latency recommendations
    if let Some(search_result) = operations.get("search_semantic") {
        let p99 = search_result.baseline.p99_us;
        if p99 > 10_000 {
            recommendations.push(format!(
                "High P99 latency for search_semantic: {:.2}ms. Consider index optimization.",
                p99 as f64 / 1000.0
            ));
        }
    }

    Summary {
        optimal_concurrency,
        peak_throughput,
        blocking_detected: blocking.is_blocking,
        recommendations,
    }
}

fn save_results(args: &Args, results: &BenchmarkResults) -> Result<(), Box<dyn std::error::Error>> {
    println!("PHASE 5: Saving Results");
    println!("{}", "-".repeat(70));

    // Create output directory if needed
    if let Some(parent) = args.output.parent() {
        fs::create_dir_all(parent)?;
    }

    // Save JSON results
    let file = File::create(&args.output)?;
    let writer = BufWriter::new(file);
    serde_json::to_writer_pretty(writer, results)?;
    println!("  JSON results saved to: {}", args.output.display());

    // Generate markdown report
    let report_path = args.output.with_extension("md");
    generate_markdown_report(&report_path, results)?;
    println!("  Markdown report saved to: {}", report_path.display());

    Ok(())
}

fn generate_markdown_report(
    path: &std::path::Path,
    results: &BenchmarkResults,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut f = File::create(path)?;

    writeln!(f, "# spawn_blocking Benchmark Results")?;
    writeln!(f)?;
    writeln!(f, "**Generated:** {}", results.metadata.timestamp)?;
    writeln!(f, "**Data size:** {} chunks", results.metadata.data_size)?;
    writeln!(f)?;

    // Summary
    writeln!(f, "## Summary")?;
    writeln!(f)?;
    writeln!(
        f,
        "- **Optimal Concurrency:** {}",
        results.summary.optimal_concurrency
    )?;
    writeln!(
        f,
        "- **Peak Throughput:** {:.0} ops/sec",
        results.summary.peak_throughput
    )?;
    writeln!(
        f,
        "- **Blocking Detected:** {}",
        if results.summary.blocking_detected {
            "Yes"
        } else {
            "No"
        }
    )?;
    writeln!(f)?;

    // Single-Threaded Baseline
    writeln!(f, "## Single-Threaded Baseline")?;
    writeln!(f)?;
    writeln!(f, "| Operation | P50 | P90 | P99 | Mean |")?;
    writeln!(f, "|-----------|-----|-----|-----|------|")?;

    for (name, result) in &results.operations {
        writeln!(
            f,
            "| {} | {:.2}ms | {:.2}ms | {:.2}ms | {:.2}ms |",
            name,
            result.baseline.p50_us as f64 / 1000.0,
            result.baseline.p90_us as f64 / 1000.0,
            result.baseline.p99_us as f64 / 1000.0,
            result.baseline.mean_us / 1000.0
        )?;
    }
    writeln!(f)?;

    // Scaling by Concurrency
    writeln!(f, "## Scaling by Concurrency")?;
    writeln!(f)?;

    if let Some(search_result) = results.operations.get("search_semantic") {
        writeln!(f, "### search_semantic")?;
        writeln!(f)?;
        writeln!(f, "| N | Throughput | Scaling | Wall Clock |")?;
        writeln!(f, "|---|------------|---------|------------|")?;

        let mut levels: Vec<_> = search_result.concurrent.keys().collect();
        levels.sort();

        for &level in &levels {
            if let Some(stats) = search_result.concurrent.get(level) {
                writeln!(
                    f,
                    "| {} | {:.0} ops/s | {:.2}x | {}ms |",
                    level, stats.throughput_ops_sec, stats.scaling_factor, stats.wall_clock_ms
                )?;
            }
        }
        writeln!(f)?;
    }

    // Blocking Detection
    writeln!(f, "## Blocking Detection")?;
    writeln!(f)?;
    writeln!(f, "| Metric | Value |")?;
    writeln!(f, "|--------|-------|")?;
    writeln!(
        f,
        "| Elapsed | {}ms |",
        results.blocking_detection.elapsed_ms
    )?;
    writeln!(
        f,
        "| Expected Ticks | {} |",
        results.blocking_detection.expected_ticks
    )?;
    writeln!(
        f,
        "| Actual Ticks | {} |",
        results.blocking_detection.actual_ticks
    )?;
    writeln!(
        f,
        "| Tick Ratio | {:.2} |",
        results.blocking_detection.tick_ratio
    )?;
    writeln!(
        f,
        "| Blocking Detected | {} |",
        if results.blocking_detection.is_blocking {
            "Yes"
        } else {
            "No"
        }
    )?;
    writeln!(f)?;

    // Recommendations
    writeln!(f, "## Recommendations")?;
    writeln!(f)?;
    for rec in &results.summary.recommendations {
        writeln!(f, "- {}", rec)?;
    }
    writeln!(f)?;

    writeln!(f, "---")?;
    writeln!(
        f,
        "*Generated with context-graph-benchmark spawn-blocking-bench*"
    )?;

    Ok(())
}

fn print_summary(results: &BenchmarkResults) {
    println!();
    println!("=======================================================================");
    println!("  Benchmark Summary");
    println!("=======================================================================");
    println!();
    println!(
        "Optimal Concurrency: {}",
        results.summary.optimal_concurrency
    );
    println!(
        "Peak Throughput: {:.0} ops/sec",
        results.summary.peak_throughput
    );
    println!(
        "Blocking Detected: {}",
        if results.summary.blocking_detected {
            "Yes"
        } else {
            "No"
        }
    );
    println!();
    println!("Recommendations:");
    for rec in &results.summary.recommendations {
        println!("  - {}", rec);
    }
    println!();
}
