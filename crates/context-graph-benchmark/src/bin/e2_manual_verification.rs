//! E2 V_freshness Manual Verification Test
//!
//! Validates that E2 temporal boost infrastructure is working correctly.
//!
//! Run with: cargo run --bin e2-manual-verification --features real-embeddings --release
//!
//! Key findings from this test:
//! - E2 model generates correct 512D embeddings
//! - Temporal boost decay functions work correctly
//! - The boost is only applied when temporal_weight > 0 (defaults to 0.0)

use std::time::Instant;

use chrono::{Duration, Utc};
use context_graph_core::traits::{DecayFunction, TemporalScale, TemporalSearchOptions};
use context_graph_embeddings::models::TemporalRecentModel;
use context_graph_embeddings::traits::EmbeddingModel;
use context_graph_embeddings::types::ModelInput;
use context_graph_storage::teleological::search::temporal_boost;
use tracing::{info, warn, Level};
use tracing_subscriber::FmtSubscriber;

struct TestResult {
    name: String,
    passed: bool,
    details: String,
}

impl TestResult {
    fn pass(name: &str, details: impl Into<String>) -> Self {
        Self {
            name: name.to_string(),
            passed: true,
            details: details.into(),
        }
    }

    fn fail(name: &str, details: impl Into<String>) -> Self {
        Self {
            name: name.to_string(),
            passed: false,
            details: details.into(),
        }
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    context_graph_benchmark::util::cosine_similarity_raw(a, b)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(false)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("========================================");
    info!("  E2 V_freshness Verification Test");
    info!("========================================");
    info!("");

    let mut results = Vec::new();

    // Test 1: E2 Model Dimension
    info!("=== Test 1: E2 Model Dimension ===");

    let model = TemporalRecentModel::new();
    let dimension = model.dimension();

    if dimension == 512 {
        info!("E2 dimension: {} (expected: 512)", dimension);
        results.push(TestResult::pass("E2 Dimension", format!("{}", dimension)));
    } else {
        warn!("E2 dimension: {} (expected: 512)", dimension);
        results.push(TestResult::fail(
            "E2 Dimension",
            format!("Expected 512, got {}", dimension),
        ));
    }

    // Test 2: E2 Embedding Generation
    info!("");
    info!("=== Test 2: E2 Embedding Generation ===");

    // Use fixed reference time for reproducible tests
    let now = Utc::now();
    let model = TemporalRecentModel::with_reference_time(now);

    // Create input with current timestamp
    let input_now = ModelInput::text_with_instruction(
        "test content",
        &format!("timestamp:{}", now.to_rfc3339()),
    )?;

    let embedding_now = model.embed(&input_now).await?;

    info!(
        "E2 embedding for now: {} dimensions, latency={}us",
        embedding_now.vector.len(),
        embedding_now.latency_us
    );

    // Verify L2 normalized
    let norm: f32 = embedding_now
        .vector
        .iter()
        .map(|x| x * x)
        .sum::<f32>()
        .sqrt();
    if (norm - 1.0).abs() < 0.001 {
        info!("L2 norm: {:.4} (expected: 1.0)", norm);
        results.push(TestResult::pass(
            "E2 L2 Normalization",
            format!("norm={:.4}", norm),
        ));
    } else {
        warn!("L2 norm: {:.4} (expected: 1.0)", norm);
        results.push(TestResult::fail(
            "E2 L2 Normalization",
            format!("Expected 1.0, got {:.4}", norm),
        ));
    }

    // Test 3: E2 Timestamp Sensitivity
    info!("");
    info!("=== Test 3: E2 Timestamp Sensitivity ===");

    // Create inputs at different times
    let ts_1h_ago = now - Duration::hours(1);
    let ts_24h_ago = now - Duration::hours(24);
    let ts_7d_ago = now - Duration::days(7);

    let input_1h = ModelInput::text_with_instruction(
        "test content",
        &format!("timestamp:{}", ts_1h_ago.to_rfc3339()),
    )?;
    let input_24h = ModelInput::text_with_instruction(
        "test content",
        &format!("timestamp:{}", ts_24h_ago.to_rfc3339()),
    )?;
    let input_7d = ModelInput::text_with_instruction(
        "test content",
        &format!("timestamp:{}", ts_7d_ago.to_rfc3339()),
    )?;

    let emb_1h = model.embed(&input_1h).await?;
    let emb_24h = model.embed(&input_24h).await?;
    let emb_7d = model.embed(&input_7d).await?;

    // Compare embeddings via cosine similarity
    let sim_now_1h = cosine_similarity(&embedding_now.vector, &emb_1h.vector);
    let sim_now_24h = cosine_similarity(&embedding_now.vector, &emb_24h.vector);
    let sim_now_7d = cosine_similarity(&embedding_now.vector, &emb_7d.vector);

    info!("E2 similarity (now vs 1h ago):  {:.4}", sim_now_1h);
    info!("E2 similarity (now vs 24h ago): {:.4}", sim_now_24h);
    info!("E2 similarity (now vs 7d ago):  {:.4}", sim_now_7d);

    // More recent memories should have higher similarity
    let timestamp_order_correct = sim_now_1h > sim_now_24h && sim_now_24h > sim_now_7d;

    if timestamp_order_correct {
        results.push(TestResult::pass(
            "E2 Timestamp Order",
            format!(
                "1h({:.3}) > 24h({:.3}) > 7d({:.3})",
                sim_now_1h, sim_now_24h, sim_now_7d
            ),
        ));
    } else {
        results.push(TestResult::fail(
            "E2 Timestamp Order",
            format!(
                "Expected decreasing similarity: 1h({:.3}), 24h({:.3}), 7d({:.3})",
                sim_now_1h, sim_now_24h, sim_now_7d
            ),
        ));
    }

    // Test 4: Temporal Boost Decay Functions
    info!("");
    info!("=== Test 4: Decay Functions ===");

    let now_ms = now.timestamp_millis();

    // Test linear decay with Macro scale (1 week horizon)
    // Note: Linear decay uses temporal_scale.horizon_seconds() as max_age
    let options_linear = TemporalSearchOptions::default()
        .with_temporal_weight(0.3)
        .with_decay_function(DecayFunction::Linear)
        .with_temporal_scale(TemporalScale::Macro); // 1 week horizon

    let score_1h_linear = temporal_boost::compute_e2_recency_score(
        (now - Duration::hours(1)).timestamp_millis(),
        now_ms,
        &options_linear,
    );
    let score_24h_linear = temporal_boost::compute_e2_recency_score(
        (now - Duration::hours(24)).timestamp_millis(),
        now_ms,
        &options_linear,
    );
    let score_7d_linear = temporal_boost::compute_e2_recency_score(
        (now - Duration::days(7)).timestamp_millis(),
        now_ms,
        &options_linear,
    );

    info!(
        "Linear decay scores (Macro scale, 7d horizon): 1h={:.3}, 24h={:.3}, 7d={:.3}",
        score_1h_linear, score_24h_linear, score_7d_linear
    );

    let linear_order_correct =
        score_1h_linear > score_24h_linear && score_24h_linear > score_7d_linear;
    if linear_order_correct {
        results.push(TestResult::pass(
            "Linear Decay Order",
            format!(
                "1h({:.3}) > 24h({:.3}) > 7d({:.3})",
                score_1h_linear, score_24h_linear, score_7d_linear
            ),
        ));
    } else {
        results.push(TestResult::fail(
            "Linear Decay Order",
            format!(
                "Expected decreasing: 1h({:.3}), 24h({:.3}), 7d({:.3})",
                score_1h_linear, score_24h_linear, score_7d_linear
            ),
        ));
    }

    // Test exponential decay with Long scale (1 month horizon)
    // Note: effective_half_life() falls back to scale.decay_half_life() if user sets the default (86400)
    // So we use Long scale which has decay_half_life of 3 days (259200s), and set half_life to 2 days
    let options_exp = TemporalSearchOptions::default()
        .with_temporal_weight(0.3)
        .with_decay_function(DecayFunction::Exponential)
        .with_temporal_scale(TemporalScale::Macro) // Use Macro to test with 12h half-life
        .with_decay_half_life(43201); // 12h + 1s to avoid default fallback

    let score_1h_exp = temporal_boost::compute_e2_recency_score(
        (now - Duration::hours(1)).timestamp_millis(),
        now_ms,
        &options_exp,
    );
    let score_12h_exp = temporal_boost::compute_e2_recency_score(
        (now - Duration::hours(12)).timestamp_millis(),
        now_ms,
        &options_exp,
    );
    let score_24h_exp = temporal_boost::compute_e2_recency_score(
        (now - Duration::hours(24)).timestamp_millis(),
        now_ms,
        &options_exp,
    );

    info!(
        "Exponential decay scores (half_life=12h): 1h={:.3}, 12h={:.3}, 24h={:.3}",
        score_1h_exp, score_12h_exp, score_24h_exp
    );

    // At 12h (half-life), score should be ~0.5
    if (score_12h_exp - 0.5).abs() < 0.1 {
        results.push(TestResult::pass(
            "Exponential Half-Life",
            format!(
                "At 12h (half-life), score={:.3} (expected ~0.5)",
                score_12h_exp
            ),
        ));
    } else {
        results.push(TestResult::fail(
            "Exponential Half-Life",
            format!("At 12h, score={:.3} (expected ~0.5)", score_12h_exp),
        ));
    }

    // Test step decay
    let options_step = TemporalSearchOptions::default()
        .with_temporal_weight(0.3)
        .with_decay_function(DecayFunction::Step);

    let score_1m_step = temporal_boost::compute_e2_recency_score(
        (now - Duration::minutes(1)).timestamp_millis(),
        now_ms,
        &options_step,
    );
    let score_30m_step = temporal_boost::compute_e2_recency_score(
        (now - Duration::minutes(30)).timestamp_millis(),
        now_ms,
        &options_step,
    );
    let score_12h_step = temporal_boost::compute_e2_recency_score(
        (now - Duration::hours(12)).timestamp_millis(),
        now_ms,
        &options_step,
    );

    info!(
        "Step decay scores: 1m={:.3}, 30m={:.3}, 12h={:.3}",
        score_1m_step, score_30m_step, score_12h_step
    );

    // Step function should have discrete jumps
    let step_has_discrete_levels = score_1m_step == 1.0 && score_30m_step == 0.8;
    if step_has_discrete_levels {
        results.push(TestResult::pass(
            "Step Decay Buckets",
            format!(
                "<5m={:.1} (1.0), <1h={:.1} (0.8)",
                score_1m_step, score_30m_step
            ),
        ));
    } else {
        results.push(TestResult::fail(
            "Step Decay Buckets",
            format!(
                "Expected <5m=1.0, <1h=0.8, got {:.1}, {:.1}",
                score_1m_step, score_30m_step
            ),
        ));
    }

    // Test 5: has_any_boost() Logic
    info!("");
    info!("=== Test 5: has_any_boost() Logic ===");

    let temporal_default = TemporalSearchOptions::default();
    let temporal_with_weight = TemporalSearchOptions::default().with_temporal_weight(0.3);
    let temporal_weight_zero = TemporalSearchOptions::default().with_temporal_weight(0.0);

    info!(
        "default (weight=0.0):        has_any_boost() = {}",
        temporal_default.has_any_boost()
    );
    info!(
        "with_weight(0.3):            has_any_boost() = {}",
        temporal_with_weight.has_any_boost()
    );
    info!(
        "with_weight(0.0):            has_any_boost() = {}",
        temporal_weight_zero.has_any_boost()
    );

    if !temporal_default.has_any_boost()
        && temporal_with_weight.has_any_boost()
        && !temporal_weight_zero.has_any_boost()
    {
        results.push(TestResult::pass(
            "has_any_boost Logic",
            "Returns false for weight=0, true for weight=0.3",
        ));
    } else {
        results.push(TestResult::fail(
            "has_any_boost Logic",
            format!(
                "Unexpected: default={}, 0.3={}, 0.0={}",
                temporal_default.has_any_boost(),
                temporal_with_weight.has_any_boost(),
                temporal_weight_zero.has_any_boost()
            ),
        ));
    }

    // Test 6: E2 Latency Budget
    info!("");
    info!("=== Test 6: E2 Latency ===");

    // Run 100 embeddings and check average latency
    let mut total_latency_us = 0u64;
    let runs = 100;
    let model = TemporalRecentModel::new();

    for _ in 0..runs {
        let input = ModelInput::text("test content for latency")?;
        let start = Instant::now();
        let _ = model.embed(&input).await?;
        total_latency_us += start.elapsed().as_micros() as u64;
    }

    let avg_latency_us = total_latency_us / runs as u64;
    let avg_latency_ms = avg_latency_us as f64 / 1000.0;

    info!(
        "Average E2 latency over {} runs: {:.2}ms",
        runs, avg_latency_ms
    );

    // Constitution budget: <2ms
    if avg_latency_ms < 2.0 {
        results.push(TestResult::pass(
            "E2 Latency",
            format!("{:.2}ms (budget: <2ms)", avg_latency_ms),
        ));
    } else {
        results.push(TestResult::fail(
            "E2 Latency",
            format!("{:.2}ms exceeds 2ms budget", avg_latency_ms),
        ));
    }

    // Summary
    info!("");
    info!("========================================");
    info!("  RESULTS");
    info!("========================================");
    info!("");

    let passed = results.iter().filter(|r| r.passed).count();
    let failed = results.iter().filter(|r| !r.passed).count();

    for r in &results {
        let status = if r.passed { "[PASS]" } else { "[FAIL]" };
        info!("{} {}: {}", status, r.name, r.details);
    }

    info!("");
    info!("Total: {} passed, {} failed", passed, failed);
    info!("");

    if failed > 0 {
        info!("Some tests failed.");
        std::process::exit(1);
    }

    info!("All tests passed. E2 infrastructure is working correctly.");
    info!("");
    info!("To use E2 temporal boost in searches:");
    info!("  - Set temporalWeight > 0 (e.g., 0.2)");
    info!("  - Choose a decayFunction: linear, exponential, or step");
    info!("  - Boost is applied POST-retrieval per ARCH-25");

    Ok(())
}
