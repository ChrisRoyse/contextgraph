//! Phase 7: Causal System Performance & Latency Benchmarks
//!
//! Measures latency and throughput of individual causal system components:
//! - 7.1: Latency Budget (per-operation timing against hard limits)
//! - 7.2: Throughput Under Load (batch processing rates)
//! - 7.3: Accuracy vs Latency Tradeoff (parameter impact analysis)
//!
//! Run without GPU features (pure code components only):
//!   cargo run -p context-graph-benchmark --bin causal-perf-bench --release
//!
//! Run with GPU features:
//!   cargo run -p context-graph-benchmark --bin causal-perf-bench --release --features real-embeddings

use anyhow::{Context, Result};
use chrono::Utc;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::Instant;
use tracing::{info, warn};

use std::hint::black_box;

use context_graph_core::causal::asymmetric::{
    compute_asymmetric_similarity, detect_causal_query_intent,
};
use context_graph_core::causal::CausalDirection;

#[cfg(feature = "real-embeddings")]
use context_graph_causal_agent::CausalDiscoveryLLM;

#[cfg(feature = "real-embeddings")]
use context_graph_embeddings::models::pretrained::CausalModel;
#[cfg(feature = "real-embeddings")]
use context_graph_embeddings::SingleModelConfig;

// ============================================================================
// Data types
// ============================================================================

#[derive(Serialize)]
struct BenchmarkResult<M: Serialize> {
    benchmark_name: String,
    timestamp: String,
    phase: String,
    metrics: M,
    pass: bool,
    targets: HashMap<String, f64>,
    actual: HashMap<String, f64>,
}

fn write_json_result<M: Serialize>(
    dir: &str,
    filename: &str,
    result: &BenchmarkResult<M>,
) -> Result<()> {
    fs::create_dir_all(dir)?;
    let path = format!("{}/{}", dir, filename);
    let json = serde_json::to_string_pretty(result)?;
    fs::write(&path, &json)?;
    info!("Wrote result: {}", path);
    Ok(())
}

// ============================================================================
// Test data generators
// ============================================================================

fn causal_texts() -> Vec<&'static str> {
    vec![
        "Chronic stress causes elevated cortisol levels in the bloodstream",
        "Deforestation leads to increased soil erosion and flooding",
        "What triggers autoimmune responses in genetically predisposed individuals?",
        "The consequence of prolonged UV exposure is melanoma risk",
        "Rising sea temperatures result in coral bleaching events",
        "Why does insulin resistance cause type 2 diabetes?",
        "Sleep deprivation leads to impaired cognitive function",
        "Excessive screen time triggers digital eye strain",
        "Climate change causes shifts in agricultural growing seasons",
        "What are the effects of microplastic pollution on marine ecosystems?",
        "Poverty leads to limited access to healthcare and education",
        "Volcanic eruptions cause temporary global cooling from aerosols",
        "Antibiotic overuse triggers the emergence of resistant bacteria",
        "Why does dehydration cause headaches and fatigue?",
        "Income inequality results in social instability",
        "The oil spill caused extensive damage to coastal ecosystems",
        "Trade restrictions lead to higher consumer prices domestically",
        "What causes glacial retreat in polar regions?",
        "Sedentary lifestyle leads to cardiovascular disease risk",
        "Ocean acidification causes shell dissolution in marine organisms",
    ]
}

fn non_causal_texts() -> Vec<&'static str> {
    vec![
        "The human body has approximately 37 trillion cells",
        "Python was first released in 1991 by Guido van Rossum",
        "The Great Wall of China stretches over 13,000 miles",
        "Water molecules consist of two hydrogen atoms and one oxygen atom",
        "Jupiter is the largest planet in our solar system",
        "The periodic table has 118 confirmed elements",
        "DNA was first described by Watson and Crick in 1953",
        "The Andes mountain range is the longest continental range",
        "Rust programming language was first stable in 2015",
        "The International Space Station orbits at about 400 km altitude",
    ]
}

// ============================================================================
// Phase 7.1: Latency Budget
// ============================================================================

#[derive(Serialize)]
struct LatencyMetrics {
    operations: Vec<OperationLatency>,
    pure_code_summary: PureCodeLatencySummary,
}

#[derive(Serialize)]
struct OperationLatency {
    operation: String,
    samples: usize,
    mean_us: f64,
    median_us: f64,
    p95_us: f64,
    p99_us: f64,
    min_us: f64,
    max_us: f64,
    target_us: f64,
    hard_limit_us: f64,
    pass: bool,
}

#[derive(Serialize)]
struct PureCodeLatencySummary {
    detect_intent_mean_us: f64,
    asymmetric_sim_mean_us: f64,
    cosine_sim_mean_us: f64,
    all_within_budget: bool,
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = (p / 100.0 * (sorted.len() - 1) as f64).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn measure_operation<F: FnMut()>(
    name: &str,
    iterations: usize,
    mut f: F,
    target_us: f64,
    hard_limit_us: f64,
) -> OperationLatency {
    let mut timings = Vec::with_capacity(iterations);

    // Warmup
    for _ in 0..10 {
        f();
    }

    for _ in 0..iterations {
        let start = Instant::now();
        f();
        timings.push(start.elapsed().as_nanos() as f64 / 1000.0); // sub-microsecond precision
    }

    timings.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let mean = timings.iter().sum::<f64>() / timings.len() as f64;
    let median = percentile(&timings, 50.0);
    let p95 = percentile(&timings, 95.0);
    let p99 = percentile(&timings, 99.0);

    let pass = p95 <= hard_limit_us;

    info!(
        "  {}: mean={:.1}us, median={:.1}us, p95={:.1}us, p99={:.1}us [{}]",
        name,
        mean,
        median,
        p95,
        p99,
        if pass { "PASS" } else { "FAIL" }
    );

    OperationLatency {
        operation: name.to_string(),
        samples: iterations,
        mean_us: mean,
        median_us: median,
        p95_us: p95,
        p99_us: p99,
        min_us: timings.first().copied().unwrap_or(0.0),
        max_us: timings.last().copied().unwrap_or(0.0),
        target_us,
        hard_limit_us,
        pass,
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    context_graph_benchmark::util::cosine_similarity_raw(a, b)
}

fn run_phase_7_1() -> Result<BenchmarkResult<LatencyMetrics>> {
    info!("=== Phase 7.1: Latency Budget ===");
    let phase_start = Instant::now();

    let causal = causal_texts();
    let non_causal = non_causal_texts();
    let all_texts: Vec<&str> = causal.iter().chain(non_causal.iter()).copied().collect();

    let mut operations = Vec::new();

    // 1. detect_causal_query_intent: target < 10us, hard limit < 50us
    let texts_clone = all_texts.clone();
    let mut text_idx = 0usize;
    let intent_op = measure_operation(
        "detect_causal_query_intent",
        10000,
        || {
            let t = texts_clone[text_idx % texts_clone.len()];
            black_box(detect_causal_query_intent(t));
            text_idx += 1;
        },
        10.0,
        50.0,
    );
    let intent_mean = intent_op.mean_us;
    operations.push(intent_op);

    // 2. compute_asymmetric_similarity: target < 1us, hard limit < 5us
    let base_cosine: f32 = 0.85;
    let directions = [
        CausalDirection::Cause,
        CausalDirection::Effect,
        CausalDirection::Unknown,
    ];
    let mut dir_idx = 0usize;
    let asym_op = measure_operation(
        "compute_asymmetric_similarity",
        100000,
        || {
            let q = directions[dir_idx % 3];
            let r = directions[(dir_idx + 1) % 3];
            black_box(compute_asymmetric_similarity(base_cosine, q, r, None, None));
            dir_idx += 1;
        },
        1.0,
        5.0,
    );
    let asym_mean = asym_op.mean_us;
    operations.push(asym_op);

    // 3. cosine_similarity (768D vectors): target < 5us, hard limit < 20us
    let vec_a: Vec<f32> = (0..768).map(|i| (i as f32 * 0.001).sin()).collect();
    let vec_b: Vec<f32> = (0..768).map(|i| (i as f32 * 0.002).cos()).collect();
    let cosine_op = measure_operation(
        "cosine_similarity_768d",
        100000,
        || {
            black_box(cosine_similarity(&vec_a, &vec_b));
        },
        5.0,
        20.0,
    );
    let cosine_mean = cosine_op.mean_us;
    operations.push(cosine_op);

    // 4. cosine_similarity (1024D vectors): target < 7us, hard limit < 30us
    let vec_c: Vec<f32> = (0..1024).map(|i| (i as f32 * 0.001).sin()).collect();
    let vec_d: Vec<f32> = (0..1024).map(|i| (i as f32 * 0.002).cos()).collect();
    let cosine1024_op = measure_operation(
        "cosine_similarity_1024d",
        100000,
        || {
            black_box(cosine_similarity(&vec_c, &vec_d));
        },
        7.0,
        30.0,
    );
    operations.push(cosine1024_op);

    // 5. Full intent detection + asymmetric pipeline: target < 15us, hard limit < 60us
    let pipeline_texts = causal_texts();
    let mut p_idx = 0usize;
    let pipeline_op = measure_operation(
        "intent_detect_plus_asymmetric",
        10000,
        || {
            let t = pipeline_texts[p_idx % pipeline_texts.len()];
            let dir = detect_causal_query_intent(t);
            black_box(compute_asymmetric_similarity(
                0.85,
                dir,
                CausalDirection::Effect,
                None,
                None,
            ));
            p_idx += 1;
        },
        15.0,
        60.0,
    );
    operations.push(pipeline_op);

    let all_pass = operations.iter().all(|op| op.pass);

    let elapsed = phase_start.elapsed();
    info!(
        "Phase 7.1 complete in {:.2?}: {} operations measured, all_pass={}",
        elapsed,
        operations.len(),
        all_pass
    );

    let mut targets = HashMap::new();
    targets.insert("all_within_hard_limit".to_string(), 1.0);

    let mut actual = HashMap::new();
    actual.insert("intent_detect_mean_us".to_string(), intent_mean);
    actual.insert("asymmetric_sim_mean_us".to_string(), asym_mean);
    actual.insert("cosine_768d_mean_us".to_string(), cosine_mean);

    Ok(BenchmarkResult {
        benchmark_name: "causal_perf_bench".to_string(),
        timestamp: Utc::now().to_rfc3339(),
        phase: "7.1_latency_budget".to_string(),
        metrics: LatencyMetrics {
            operations,
            pure_code_summary: PureCodeLatencySummary {
                detect_intent_mean_us: intent_mean,
                asymmetric_sim_mean_us: asym_mean,
                cosine_sim_mean_us: cosine_mean,
                all_within_budget: all_pass,
            },
        },
        pass: all_pass,
        targets,
        actual,
    })
}

// ============================================================================
// Phase 7.2: Throughput Under Load
// ============================================================================

#[derive(Serialize)]
struct ThroughputMetrics {
    operations: Vec<ThroughputMeasurement>,
    overall_pass: bool,
}

#[derive(Serialize)]
struct ThroughputMeasurement {
    operation: String,
    batch_size: usize,
    total_time_ms: f64,
    throughput_per_sec: f64,
    target_per_sec: f64,
    pass: bool,
}

fn run_phase_7_2() -> Result<BenchmarkResult<ThroughputMetrics>> {
    info!("=== Phase 7.2: Throughput Under Load ===");
    let phase_start = Instant::now();

    let causal = causal_texts();
    let non_causal = non_causal_texts();
    let all_texts: Vec<&str> = causal.iter().chain(non_causal.iter()).copied().collect();

    let mut measurements = Vec::new();

    // 1. Intent detection throughput: target > 100K/sec
    {
        let batch_size = 100_000usize;
        let start = Instant::now();
        for i in 0..batch_size {
            let t = all_texts[i % all_texts.len()];
            black_box(detect_causal_query_intent(t));
        }
        let elapsed = start.elapsed();
        let throughput = batch_size as f64 / elapsed.as_secs_f64();
        let pass = throughput > 100_000.0;
        info!(
            "  intent_detection: {:.0}/sec (target: 100K/sec) [{}]",
            throughput,
            if pass { "PASS" } else { "FAIL" }
        );
        measurements.push(ThroughputMeasurement {
            operation: "detect_causal_query_intent".to_string(),
            batch_size,
            total_time_ms: elapsed.as_millis() as f64,
            throughput_per_sec: throughput,
            target_per_sec: 100_000.0,
            pass,
        });
    }

    // 2. Asymmetric similarity throughput: target > 1M/sec
    {
        let batch_size = 1_000_000usize;
        let directions = [
            CausalDirection::Cause,
            CausalDirection::Effect,
            CausalDirection::Unknown,
        ];
        let start = Instant::now();
        for i in 0..batch_size {
            let q = directions[i % 3];
            let r = directions[(i + 1) % 3];
            black_box(compute_asymmetric_similarity(0.85, q, r, None, None));
        }
        let elapsed = start.elapsed();
        let throughput = batch_size as f64 / elapsed.as_secs_f64();
        let pass = throughput > 1_000_000.0;
        info!(
            "  asymmetric_similarity: {:.0}/sec (target: 1M/sec) [{}]",
            throughput,
            if pass { "PASS" } else { "FAIL" }
        );
        measurements.push(ThroughputMeasurement {
            operation: "compute_asymmetric_similarity".to_string(),
            batch_size,
            total_time_ms: elapsed.as_millis() as f64,
            throughput_per_sec: throughput,
            target_per_sec: 1_000_000.0,
            pass,
        });
    }

    // 3. Cosine similarity 768D throughput: target > 500K/sec
    {
        let batch_size = 500_000usize;
        let vec_a: Vec<f32> = (0..768).map(|i| (i as f32 * 0.001).sin()).collect();
        let vec_b: Vec<f32> = (0..768).map(|i| (i as f32 * 0.002).cos()).collect();
        let start = Instant::now();
        for _ in 0..batch_size {
            black_box(cosine_similarity(&vec_a, &vec_b));
        }
        let elapsed = start.elapsed();
        let throughput = batch_size as f64 / elapsed.as_secs_f64();
        let pass = throughput > 500_000.0;
        info!(
            "  cosine_768d: {:.0}/sec (target: 500K/sec) [{}]",
            throughput,
            if pass { "PASS" } else { "FAIL" }
        );
        measurements.push(ThroughputMeasurement {
            operation: "cosine_similarity_768d".to_string(),
            batch_size,
            total_time_ms: elapsed.as_millis() as f64,
            throughput_per_sec: throughput,
            target_per_sec: 500_000.0,
            pass,
        });
    }

    // 4. Full pipeline (intent + asymmetric) throughput: target > 50K/sec
    {
        let batch_size = 100_000usize;
        let start = Instant::now();
        for i in 0..batch_size {
            let t = all_texts[i % all_texts.len()];
            let dir = detect_causal_query_intent(t);
            black_box(compute_asymmetric_similarity(
                0.85,
                dir,
                CausalDirection::Effect,
                None,
                None,
            ));
        }
        let elapsed = start.elapsed();
        let throughput = batch_size as f64 / elapsed.as_secs_f64();
        let pass = throughput > 50_000.0;
        info!(
            "  full_pipeline_pure: {:.0}/sec (target: 50K/sec) [{}]",
            throughput,
            if pass { "PASS" } else { "FAIL" }
        );
        measurements.push(ThroughputMeasurement {
            operation: "intent_plus_asymmetric_pipeline".to_string(),
            batch_size,
            total_time_ms: elapsed.as_millis() as f64,
            throughput_per_sec: throughput,
            target_per_sec: 50_000.0,
            pass,
        });
    }

    let overall_pass = measurements.iter().all(|m| m.pass);

    let elapsed = phase_start.elapsed();
    info!(
        "Phase 7.2 complete in {:.2?}: all_pass={}",
        elapsed, overall_pass
    );

    let mut targets = HashMap::new();
    targets.insert("all_targets_met".to_string(), 1.0);

    let mut actual = HashMap::new();
    for m in &measurements {
        actual.insert(format!("{}_per_sec", m.operation), m.throughput_per_sec);
    }

    Ok(BenchmarkResult {
        benchmark_name: "causal_perf_bench".to_string(),
        timestamp: Utc::now().to_rfc3339(),
        phase: "7.2_throughput".to_string(),
        metrics: ThroughputMetrics {
            operations: measurements,
            overall_pass,
        },
        pass: overall_pass,
        targets,
        actual,
    })
}

// ============================================================================
// Phase 7.3: Accuracy vs Latency Tradeoff
// ============================================================================

#[derive(Serialize)]
struct ParetoMetrics {
    tradeoff_points: Vec<TradeoffPoint>,
    modifier_impact: Vec<ModifierImpactPoint>,
    optimal_operating_point: String,
}

#[derive(Serialize)]
struct TradeoffPoint {
    description: String,
    accuracy: f64,
    latency_us: f64,
    throughput_per_sec: f64,
}

#[derive(Serialize)]
struct ModifierImpactPoint {
    cause_to_effect_mod: f64,
    effect_to_cause_mod: f64,
    forward_backward_ratio: f64,
    computation_ns: f64,
}

fn run_phase_7_3() -> Result<BenchmarkResult<ParetoMetrics>> {
    info!("=== Phase 7.3: Accuracy vs Latency Tradeoff ===");
    let phase_start = Instant::now();

    let test_texts = causal_texts();

    // Tradeoff 1: Simple intent check vs full pipeline
    let mut tradeoff_points = Vec::new();

    // Point 1: Intent-only (fast, less accurate)
    {
        let iterations = 10_000usize;
        let mut correct = 0usize;
        let start = Instant::now();
        for i in 0..iterations {
            let t = test_texts[i % test_texts.len()];
            let dir = detect_causal_query_intent(t);
            if dir != CausalDirection::Unknown {
                correct += 1;
            }
        }
        let elapsed = start.elapsed();
        let accuracy = correct as f64 / iterations as f64;
        let latency_us = elapsed.as_micros() as f64 / iterations as f64;
        let throughput = iterations as f64 / elapsed.as_secs_f64();

        tradeoff_points.push(TradeoffPoint {
            description: "intent_detection_only".to_string(),
            accuracy,
            latency_us,
            throughput_per_sec: throughput,
        });
        info!(
            "  Intent only: accuracy={:.1}%, latency={:.1}us, throughput={:.0}/sec",
            accuracy * 100.0,
            latency_us,
            throughput
        );
    }

    // Point 2: Intent + asymmetric scoring (moderate cost, better accuracy)
    {
        let iterations = 10_000usize;
        let base_cosine: f32 = 0.85;
        let mut significant = 0usize;
        let start = Instant::now();
        for i in 0..iterations {
            let t = test_texts[i % test_texts.len()];
            let dir = detect_causal_query_intent(t);
            let score = compute_asymmetric_similarity(
                base_cosine,
                dir,
                CausalDirection::Effect,
                None,
                None,
            );
            if score > 0.5 {
                significant += 1;
            }
        }
        let elapsed = start.elapsed();
        let accuracy = significant as f64 / iterations as f64;
        let latency_us = elapsed.as_micros() as f64 / iterations as f64;
        let throughput = iterations as f64 / elapsed.as_secs_f64();

        tradeoff_points.push(TradeoffPoint {
            description: "intent_plus_asymmetric".to_string(),
            accuracy,
            latency_us,
            throughput_per_sec: throughput,
        });
        info!(
            "  Intent+asymmetric: accuracy={:.1}%, latency={:.1}us, throughput={:.0}/sec",
            accuracy * 100.0,
            latency_us,
            throughput
        );
    }

    // Point 3: Full cosine + intent + asymmetric (highest cost, best accuracy)
    {
        let iterations = 10_000usize;
        let vec_a: Vec<f32> = (0..768).map(|i| (i as f32 * 0.001).sin()).collect();
        let vec_b: Vec<f32> = (0..768).map(|i| (i as f32 * 0.002).cos()).collect();
        let mut significant = 0usize;
        let start = Instant::now();
        for i in 0..iterations {
            let t = test_texts[i % test_texts.len()];
            let dir = detect_causal_query_intent(t);
            let base = cosine_similarity(&vec_a, &vec_b);
            let score =
                compute_asymmetric_similarity(base, dir, CausalDirection::Effect, None, None);
            if score > 0.5 {
                significant += 1;
            }
        }
        let elapsed = start.elapsed();
        let accuracy = significant as f64 / iterations as f64;
        let latency_us = elapsed.as_micros() as f64 / iterations as f64;
        let throughput = iterations as f64 / elapsed.as_secs_f64();

        tradeoff_points.push(TradeoffPoint {
            description: "cosine_plus_intent_plus_asymmetric".to_string(),
            accuracy,
            latency_us,
            throughput_per_sec: throughput,
        });
        info!(
            "  Full pipeline: accuracy={:.1}%, latency={:.1}us, throughput={:.0}/sec",
            accuracy * 100.0,
            latency_us,
            throughput
        );
    }

    // Modifier impact analysis: sweep different modifier values and measure
    // the forward/backward ratio each would produce. The API uses hardcoded
    // 1.2/0.8 modifiers, so we compute manual arithmetic to simulate what
    // different modifier values would yield, while timing the actual function.
    let base_cosine: f32 = 0.85;
    let mut modifier_impact = Vec::new();
    for c2e_10x in 10..=20 {
        let c2e = c2e_10x as f64 / 10.0;
        let e2c = 2.0 - c2e;

        // Measure computation time of the actual API call (with black_box to prevent elision)
        let iterations = 100_000usize;
        let start = Instant::now();
        for _ in 0..iterations {
            black_box(compute_asymmetric_similarity(
                black_box(base_cosine),
                CausalDirection::Cause,
                CausalDirection::Effect,
                None,
                None,
            ));
        }
        let elapsed_ns = start.elapsed().as_nanos() as f64 / iterations as f64;

        // Compute what the forward/backward ratio WOULD be with these modifiers
        // forward = base_cosine * c2e (cause query → effect result, amplified)
        // backward = base_cosine * e2c (effect query → cause result, dampened)
        let forward_sim = base_cosine as f64 * c2e;
        let backward_sim = base_cosine as f64 * e2c;
        let ratio = if backward_sim > 0.0 {
            forward_sim / backward_sim
        } else {
            0.0
        };

        modifier_impact.push(ModifierImpactPoint {
            cause_to_effect_mod: c2e,
            effect_to_cause_mod: e2c,
            forward_backward_ratio: ratio,
            computation_ns: elapsed_ns,
        });
    }

    // Determine optimal operating point
    let optimal = if tradeoff_points.len() >= 2 {
        // Pick the point with best accuracy/latency ratio
        tradeoff_points
            .iter()
            .max_by(|a, b| {
                let ra = a.accuracy / a.latency_us.max(0.001);
                let rb = b.accuracy / b.latency_us.max(0.001);
                ra.partial_cmp(&rb).unwrap()
            })
            .map(|p| p.description.clone())
            .unwrap_or_default()
    } else {
        "unknown".to_string()
    };

    let elapsed = phase_start.elapsed();
    info!("Phase 7.3 complete in {:.2?}: optimal={}", elapsed, optimal);

    let mut targets = HashMap::new();
    targets.insert("tradeoff_points_measured".to_string(), 3.0);

    let mut actual = HashMap::new();
    actual.insert("tradeoff_points".to_string(), tradeoff_points.len() as f64);
    actual.insert("modifier_points".to_string(), modifier_impact.len() as f64);

    Ok(BenchmarkResult {
        benchmark_name: "causal_perf_bench".to_string(),
        timestamp: Utc::now().to_rfc3339(),
        phase: "7.3_pareto_frontier".to_string(),
        metrics: ParetoMetrics {
            tradeoff_points,
            modifier_impact,
            optimal_operating_point: optimal,
        },
        pass: true,
        targets,
        actual,
    })
}

// ============================================================================
// Phase 7.1 GPU Operations (requires real-embeddings)
// ============================================================================

#[cfg(feature = "real-embeddings")]
#[derive(Serialize)]
struct GpuLatencyMetrics {
    e5_embed_dual_ms: Vec<f64>,
    e5_embed_dual_mean_ms: f64,
    e5_embed_dual_p95_ms: f64,
    llm_single_pair_ms: Vec<f64>,
    llm_single_pair_mean_ms: f64,
    llm_single_pair_p95_ms: f64,
    all_within_budget: bool,
}

#[cfg(feature = "real-embeddings")]
async fn run_phase_7_1_gpu() -> Result<BenchmarkResult<GpuLatencyMetrics>> {
    info!("=== Phase 7.1 GPU: E5 + LLM Latency ===");
    let phase_start = Instant::now();

    // Load E5 CausalModel
    let model = CausalModel::new(Path::new("models/causal"), SingleModelConfig::default())?;
    model.load().await?;

    let texts = causal_texts();
    let mut e5_times = Vec::new();

    // Warmup
    for _ in 0..3 {
        let _ = model.embed_dual(texts[0]).await?;
    }

    // Measure E5 embed_dual latency
    for text in texts.iter().take(10) {
        let start = Instant::now();
        let _ = model.embed_dual(text).await?;
        e5_times.push(start.elapsed().as_millis() as f64);
    }

    e5_times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let e5_mean = e5_times.iter().sum::<f64>() / e5_times.len() as f64;
    let e5_p95 = percentile(&e5_times, 95.0);

    info!(
        "  E5 embed_dual: mean={:.1}ms, p95={:.1}ms (target: <20ms, hard: <50ms)",
        e5_mean, e5_p95
    );

    // Measure LLM single pair latency
    let llm = CausalDiscoveryLLM::new()?;
    llm.load().await?;

    let mut llm_times = Vec::new();
    let pairs: Vec<(&str, &str)> = vec![
        (
            "Smoking damages lung tissue",
            "Lung cancer develops in smokers",
        ),
        ("Deforestation removes carbon sinks", "CO2 levels increase"),
        (
            "Heavy rainfall saturated the soil",
            "A landslide destroyed the village",
        ),
    ];

    for (a, b) in &pairs {
        let start = Instant::now();
        let _ = llm.analyze_causal_relationship(a, b).await?;
        llm_times.push(start.elapsed().as_millis() as f64);
    }

    llm_times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let llm_mean = llm_times.iter().sum::<f64>() / llm_times.len() as f64;
    let llm_p95 = percentile(&llm_times, 95.0);

    info!(
        "  LLM single pair: mean={:.1}ms, p95={:.1}ms (target: <500ms, hard: <2000ms)",
        llm_mean, llm_p95
    );

    let e5_pass = e5_p95 <= 50.0;
    let llm_pass = llm_p95 <= 2000.0;
    let all_pass = e5_pass && llm_pass;

    model.unload().await?;
    llm.unload().await?;

    let elapsed = phase_start.elapsed();
    info!(
        "Phase 7.1 GPU complete in {:.2?}: e5_pass={}, llm_pass={}",
        elapsed, e5_pass, llm_pass
    );

    let mut targets = HashMap::new();
    targets.insert("e5_hard_limit_ms".to_string(), 50.0);
    targets.insert("llm_hard_limit_ms".to_string(), 2000.0);

    let mut actual = HashMap::new();
    actual.insert("e5_mean_ms".to_string(), e5_mean);
    actual.insert("e5_p95_ms".to_string(), e5_p95);
    actual.insert("llm_mean_ms".to_string(), llm_mean);
    actual.insert("llm_p95_ms".to_string(), llm_p95);

    Ok(BenchmarkResult {
        benchmark_name: "causal_perf_bench".to_string(),
        timestamp: Utc::now().to_rfc3339(),
        phase: "7.1_gpu_latency".to_string(),
        metrics: GpuLatencyMetrics {
            e5_embed_dual_ms: e5_times,
            e5_embed_dual_mean_ms: e5_mean,
            e5_embed_dual_p95_ms: e5_p95,
            llm_single_pair_ms: llm_times,
            llm_single_pair_mean_ms: llm_mean,
            llm_single_pair_p95_ms: llm_p95,
            all_within_budget: all_pass,
        },
        pass: all_pass,
        targets,
        actual,
    })
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    info!("Starting Causal Performance Benchmark (Phases 7.1, 7.2, 7.3)");
    let overall_start = Instant::now();

    let base_dir = "benchmark_results/causal_accuracy";
    let mut all_passed = true;

    // Phase 7.1: Latency Budget (pure code)
    let result_7_1 = run_phase_7_1().context("Phase 7.1 (Latency Budget) failed")?;
    if !result_7_1.pass {
        warn!("Phase 7.1 FAILED targets");
        all_passed = false;
    }
    write_json_result(
        &format!("{}/phase7_performance", base_dir),
        "latency.json",
        &result_7_1,
    )?;

    // Phase 7.1 GPU (requires real-embeddings)
    #[cfg(feature = "real-embeddings")]
    {
        info!("Phase 7.1 GPU enabled (real-embeddings feature active)");
        let result = run_phase_7_1_gpu()
            .await
            .context("Phase 7.1 GPU (E5+LLM Latency) failed")?;
        if !result.pass {
            warn!("Phase 7.1 GPU FAILED targets");
            all_passed = false;
        }
        write_json_result(
            &format!("{}/phase7_performance", base_dir),
            "gpu_latency.json",
            &result,
        )?;
    }

    #[cfg(not(feature = "real-embeddings"))]
    {
        warn!("Phase 7.1 GPU SKIPPED: real-embeddings feature not enabled.");
    }

    // Phase 7.2: Throughput Under Load (pure code)
    let result_7_2 = run_phase_7_2().context("Phase 7.2 (Throughput) failed")?;
    if !result_7_2.pass {
        warn!("Phase 7.2 FAILED targets");
        all_passed = false;
    }
    write_json_result(
        &format!("{}/phase7_performance", base_dir),
        "throughput.json",
        &result_7_2,
    )?;

    // Phase 7.3: Accuracy vs Latency Tradeoff (pure code)
    let result_7_3 = run_phase_7_3().context("Phase 7.3 (Pareto Frontier) failed")?;
    if !result_7_3.pass {
        warn!("Phase 7.3 FAILED targets");
        all_passed = false;
    }
    write_json_result(
        &format!("{}/phase7_performance", base_dir),
        "pareto_frontier.json",
        &result_7_3,
    )?;

    let total_elapsed = overall_start.elapsed();
    info!(
        "All Phase 7 benchmarks complete in {:.2?}. Overall pass: {}",
        total_elapsed, all_passed
    );

    if !all_passed {
        warn!("One or more Phase 7 benchmarks did not meet target thresholds.");
    }

    Ok(())
}
