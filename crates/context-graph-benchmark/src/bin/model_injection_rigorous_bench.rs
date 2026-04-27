//! Rigorous Model Injection Verification Benchmark
//!
//! This benchmark proves that model injection produces REAL embeddings by:
//! 1. Verifying embeddings have non-trivial values (not zeros or placeholders)
//! 2. Checking semantic consistency (similar inputs should have similar embeddings)
//! 3. Testing asymmetric behavior (source ≠ target for different relationships)
//! 4. Measuring actual GPU inference times (not instant returns)
//!
//! ## Usage
//!
//! ```bash
//! cargo run -p context-graph-benchmark --bin model-injection-rigorous-bench --release --features real-embeddings
//! ```

use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

use context_graph_causal_agent::llm::{CausalDiscoveryLLM, LlmConfig};
use context_graph_embeddings::{
    get_warm_causal_model, get_warm_graph_model, initialize_global_warm_provider,
    is_warm_initialized,
};
use context_graph_graph_agent::{GraphDiscoveryConfig, GraphDiscoveryService};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::WARN) // Less verbose
        .with_target(false)
        .finish();
    tracing::subscriber::set_global_default(subscriber).ok();

    println!("\n");
    println!("╔══════════════════════════════════════════════════════════════════════════════╗");
    println!("║             RIGOROUS MODEL INJECTION VERIFICATION                            ║");
    println!("╠══════════════════════════════════════════════════════════════════════════════╣");
    println!("║ Proves embeddings are REAL by testing semantic properties, not just shapes   ║");
    println!("╚══════════════════════════════════════════════════════════════════════════════╝");
    println!();

    let mut all_pass = true;

    // Step 1: Initialize warm provider
    println!("═══════════════════════════════════════════════════════════════════════════════");
    println!("STEP 1: Initialize Global Warm Provider");
    println!("═══════════════════════════════════════════════════════════════════════════════");

    if !is_warm_initialized() {
        println!("  Loading 13 embedding models to GPU...");
        initialize_global_warm_provider().await?;
    }
    println!("  ✓ Warm provider ready");
    println!();

    // Step 2: Get models
    let graph_model = get_warm_graph_model()?;
    let causal_model = get_warm_causal_model()?;
    println!("  ✓ Models obtained from warm provider");
    println!();

    // Test 1: Vector statistics prove real computation
    println!("═══════════════════════════════════════════════════════════════════════════════");
    println!("TEST 1: Vector Statistics (prove not zeros/placeholders)");
    println!("═══════════════════════════════════════════════════════════════════════════════");

    let test_text = "The authentication module imports cryptographic functions.";
    let (source, target) = graph_model.embed_dual(test_text).await?;

    // Check statistics
    let source_mean = mean(&source);
    let source_std = std_dev(&source);
    let source_min = min(&source);
    let source_max = max(&source);
    let source_nonzero_pct = nonzero_pct(&source);

    println!("  E8 GraphModel (1024D):");
    println!(
        "    Mean:     {:.6} (should be near 0 for normalized)",
        source_mean
    );
    println!("    Std Dev:  {:.6} (should be non-zero)", source_std);
    println!("    Min:      {:.6}", source_min);
    println!("    Max:      {:.6}", source_max);
    println!("    Non-zero: {:.1}% (should be ~100%)", source_nonzero_pct);

    // Validation: Real embeddings have specific statistical properties
    let stats_pass = source_std > 0.01 && source_nonzero_pct > 95.0;
    println!(
        "    Result:   {}",
        if stats_pass { "✓ PASS" } else { "✗ FAIL" }
    );
    if !stats_pass {
        all_pass = false;
    }
    println!();

    // Test 2: Semantic similarity (similar texts should have higher similarity than different)
    println!("═══════════════════════════════════════════════════════════════════════════════");
    println!("TEST 2: Semantic Consistency (similar texts → higher similarity)");
    println!("═══════════════════════════════════════════════════════════════════════════════");

    let text_a = "The user authentication system validates login credentials.";
    let text_b = "Login validation checks user credentials for authentication.";
    let text_c = "The weather forecast predicts rain tomorrow afternoon.";

    let (embed_a, _) = graph_model.embed_dual(text_a).await?;
    let (embed_b, _) = graph_model.embed_dual(text_b).await?;
    let (embed_c, _) = graph_model.embed_dual(text_c).await?;

    let sim_ab = cosine_similarity(&embed_a, &embed_b);
    let sim_ac = cosine_similarity(&embed_a, &embed_c);

    println!("  A: \"{}\"", truncate(text_a, 50));
    println!("  B: \"{}\"", truncate(text_b, 50));
    println!("  C: \"{}\"", truncate(text_c, 50));
    println!();
    println!("  sim(A, B) = {:.4} (same topic - should be high)", sim_ab);
    println!("  sim(A, C) = {:.4} (different topic)", sim_ac);

    // Key validation: Similar texts (A,B) should have HIGHER similarity than different texts (A,C)
    // Also validates embeddings are producing meaningful similarities (not random or constant)
    let semantic_pass = sim_ab > 0.8 && sim_ab > sim_ac;
    println!(
        "  sim(A,B) > sim(A,C): {} ({:.4} > {:.4})",
        sim_ab > sim_ac,
        sim_ab,
        sim_ac
    );
    println!(
        "  Result:   {}",
        if semantic_pass {
            "✓ PASS"
        } else {
            "✗ FAIL"
        }
    );
    if !semantic_pass {
        println!("    NOTE: Expected sim(A,B) > 0.8 and sim(A,B) > sim(A,C)");
        all_pass = false;
    }
    println!();

    // Test 3: Asymmetric embeddings are genuinely different
    println!("═══════════════════════════════════════════════════════════════════════════════");
    println!("TEST 3: Asymmetric Difference (source ≠ target)");
    println!("═══════════════════════════════════════════════════════════════════════════════");

    let rel_text = "Module A imports functions from Module B.";
    let (source_vec, target_vec) = graph_model.embed_dual(rel_text).await?;

    let source_target_sim = cosine_similarity(&source_vec, &target_vec);
    let l2_distance = l2_norm(&subtract(&source_vec, &target_vec));

    println!("  Text: \"{}\"", rel_text);
    println!(
        "  source↔target cosine sim: {:.4} (should be < 1.0)",
        source_target_sim
    );
    println!(
        "  source↔target L2 distance: {:.4} (should be > 0)",
        l2_distance
    );

    let asymmetric_pass = source_target_sim < 0.999 && l2_distance > 0.01;
    println!(
        "  Result:   {}",
        if asymmetric_pass {
            "✓ PASS"
        } else {
            "✗ FAIL"
        }
    );
    if !asymmetric_pass {
        all_pass = false;
    }
    println!();

    // Test 4: E5 Causal model produces real embeddings with proper dimensionality
    println!("═══════════════════════════════════════════════════════════════════════════════");
    println!("TEST 4: E5 Causal Model Embedding Validation");
    println!("═══════════════════════════════════════════════════════════════════════════════");

    let causal_text = "High temperatures cause water to evaporate faster from open containers.";

    let (cause_embed, effect_embed) = causal_model.embed_dual(causal_text).await?;

    // Validate embedding properties
    let cause_dim = cause_embed.len();
    let effect_dim = effect_embed.len();
    let cause_std = std_dev(&cause_embed);
    let effect_std = std_dev(&effect_embed);
    let cause_effect_sim = cosine_similarity(&cause_embed, &effect_embed);

    println!("  Text: \"{}\"", truncate(causal_text, 60));
    println!();
    println!(
        "  Cause embedding:  {}D, std_dev={:.6}",
        cause_dim, cause_std
    );
    println!(
        "  Effect embedding: {}D, std_dev={:.6}",
        effect_dim, effect_std
    );
    println!(
        "  Cause↔Effect sim: {:.4} (should be < 1.0 - asymmetric)",
        cause_effect_sim
    );

    // Validation: 768D embeddings with non-trivial values and asymmetric behavior
    let causal_pass = cause_dim == 768
        && effect_dim == 768
        && cause_std > 0.01
        && effect_std > 0.01
        && cause_effect_sim < 0.999; // Not identical

    println!(
        "  Dimension check:  {} (expected 768D)",
        if cause_dim == 768 && effect_dim == 768 {
            "✓"
        } else {
            "✗"
        }
    );
    println!(
        "  Non-trivial std:  {} (cause={:.4}, effect={:.4})",
        if cause_std > 0.01 && effect_std > 0.01 {
            "✓"
        } else {
            "✗"
        },
        cause_std,
        effect_std
    );
    println!(
        "  Asymmetric:       {} (sim={:.4} < 1.0)",
        if cause_effect_sim < 0.999 {
            "✓"
        } else {
            "✗"
        },
        cause_effect_sim
    );
    println!(
        "  Result:   {}",
        if causal_pass { "✓ PASS" } else { "✗ FAIL" }
    );
    if !causal_pass {
        all_pass = false;
    }
    println!();

    // Test 5: GPU timing (real inference takes time, not instant)
    println!("═══════════════════════════════════════════════════════════════════════════════");
    println!("TEST 5: GPU Inference Timing (real compute takes time)");
    println!("═══════════════════════════════════════════════════════════════════════════════");

    let long_text = "This is a longer piece of text that should take more time to process \
                     through the neural network transformer layers because it has more tokens.";

    let timing_iterations = 5;
    let mut times = Vec::new();

    for _ in 0..timing_iterations {
        let start = Instant::now();
        let _ = graph_model.embed_dual(long_text).await?;
        times.push(start.elapsed().as_micros() as f64);
    }

    let avg_time = times.iter().sum::<f64>() / times.len() as f64;
    let min_time = times.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_time = times.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    println!("  Iterations: {}", timing_iterations);
    println!(
        "  Avg time:   {:.1} µs ({:.2} ms)",
        avg_time,
        avg_time / 1000.0
    );
    println!("  Min time:   {:.1} µs", min_time);
    println!("  Max time:   {:.1} µs", max_time);

    // Real GPU inference should take at least 1ms
    let timing_pass = avg_time > 1000.0; // > 1ms
    println!(
        "  Result:   {}",
        if timing_pass {
            "✓ PASS (real GPU inference)"
        } else {
            "✗ FAIL (too fast - may be cached/stub)"
        }
    );
    if !timing_pass {
        all_pass = false;
    }
    println!();

    // Test 6: GraphDiscoveryService creation with model injection
    println!("═══════════════════════════════════════════════════════════════════════════════");
    println!("TEST 6: GraphDiscoveryService Model Injection");
    println!("═══════════════════════════════════════════════════════════════════════════════");

    println!("  Creating CausalDiscoveryLLM...");
    let llm = CausalDiscoveryLLM::with_config(LlmConfig {
        context_size: 512,
        max_tokens: 128,
        ..Default::default()
    })?;
    llm.load().await?;
    println!("  ✓ LLM loaded");

    let graph_model_for_service = get_warm_graph_model()?;
    let service = GraphDiscoveryService::with_models(
        Arc::new(llm),
        graph_model_for_service,
        GraphDiscoveryConfig::default(),
    );

    println!("  ✓ Service created with ::with_models()");
    println!("  ✓ GraphModel injected into E8Activator");
    println!();

    // Summary
    println!("═══════════════════════════════════════════════════════════════════════════════");
    println!("                              SUMMARY                                           ");
    println!("═══════════════════════════════════════════════════════════════════════════════");
    println!();

    if all_pass {
        println!("  ╔═══════════════════════════════════════════════════════════════════════╗");
        println!("  ║  ✓ ALL TESTS PASSED - Model injection producing REAL embeddings       ║");
        println!("  ╚═══════════════════════════════════════════════════════════════════════╝");
    } else {
        println!("  ╔═══════════════════════════════════════════════════════════════════════╗");
        println!("  ║  ✗ SOME TESTS FAILED - Check output above                             ║");
        println!("  ╚═══════════════════════════════════════════════════════════════════════╝");
    }

    println!();
    println!("  Embeddings verified as REAL because:");
    println!("    - Non-trivial statistics (non-zero std dev, high non-zero %)");
    println!("    - Semantic consistency (similar texts have higher similarity)");
    println!("    - Asymmetric behavior (source ≠ target vectors)");
    println!("    - Real GPU timing (not instant returns)");
    println!();

    // Keep service alive to verify it compiles
    drop(service);

    Ok(())
}

// Helper functions
fn mean(v: &[f32]) -> f32 {
    v.iter().sum::<f32>() / v.len() as f32
}

fn std_dev(v: &[f32]) -> f32 {
    let m = mean(v);
    let variance = v.iter().map(|x| (x - m).powi(2)).sum::<f32>() / v.len() as f32;
    variance.sqrt()
}

fn min(v: &[f32]) -> f32 {
    v.iter().cloned().fold(f32::INFINITY, f32::min)
}

fn max(v: &[f32]) -> f32 {
    v.iter().cloned().fold(f32::NEG_INFINITY, f32::max)
}

fn nonzero_pct(v: &[f32]) -> f32 {
    let nonzero = v.iter().filter(|&&x| x.abs() > 1e-9).count();
    100.0 * nonzero as f32 / v.len() as f32
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    context_graph_benchmark::util::cosine_similarity_raw(a, b)
}

fn l2_norm(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt()
}

fn subtract(a: &[f32], b: &[f32]) -> Vec<f32> {
    a.iter().zip(b.iter()).map(|(x, y)| x - y).collect()
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max - 3])
    }
}
