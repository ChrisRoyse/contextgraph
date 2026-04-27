//! Integration test for Causal Discovery LLM with RTX 5090.
//!
//! Run with: cargo run -p context-graph-causal-agent --example test_llm --features cuda

use std::time::Instant;

use context_graph_causal_agent::llm::{CausalDiscoveryLLM, LlmConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║    Causal Discovery LLM Integration Test (Hermes 2 Pro)       ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");

    // Find workspace root
    let mut workspace_root = std::env::current_dir()?;
    while !workspace_root.join("models").exists() {
        if !workspace_root.pop() {
            return Err("Could not find workspace root".into());
        }
    }

    let model_dir = workspace_root.join("models/hermes-2-pro");

    let config = LlmConfig {
        model_path: model_dir.join("Hermes-2-Pro-Mistral-7B.Q5_K_M.gguf"),
        causal_grammar_path: model_dir.join("causal_analysis.gbnf"),
        graph_grammar_path: model_dir.join("graph_relationship.gbnf"),
        validation_grammar_path: model_dir.join("validation.gbnf"),
        n_gpu_layers: u32::MAX, // Full GPU offload
        temperature: 0.0,       // Deterministic
        max_tokens: 256,
        ..Default::default()
    };

    println!("Configuration:");
    println!("  Model: {:?}", config.model_path);
    println!("  GPU Layers: {} (MAX = all)", config.n_gpu_layers);
    println!("  Temperature: {}", config.temperature);
    println!("  Max Tokens: {}", config.max_tokens);
    println!();

    // Create LLM
    let llm = CausalDiscoveryLLM::with_config(config)?;

    // Load model
    println!("Loading model...");
    let start = Instant::now();
    llm.load().await?;
    println!("✓ Model loaded in {:?}\n", start.elapsed());

    // Test cases
    let test_cases = [
        (
            "The server ran out of memory",
            "The application crashed with an OOM error",
            "Expected: A causes B (memory exhaustion leads to crash)",
        ),
        (
            "We deployed a new version of the API",
            "Users reported faster response times",
            "Expected: A causes B (deployment improved performance)",
        ),
        (
            "It was raining outside",
            "The database query was slow",
            "Expected: No causal relationship (unrelated events)",
        ),
    ];

    println!("Running {} test cases:\n", test_cases.len());

    for (i, (statement_a, statement_b, expected)) in test_cases.iter().enumerate() {
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("Test Case #{}", i + 1);
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("Statement A: {}", statement_a);
        println!("Statement B: {}", statement_b);
        println!("{}\n", expected);

        let start = Instant::now();
        match llm
            .analyze_causal_relationship(statement_a, statement_b)
            .await
        {
            Ok(result) => {
                let elapsed = start.elapsed();
                println!("Result:");
                println!("  Has Causal Link: {}", result.has_causal_link);
                println!("  Confidence:      {:.2}", result.confidence);
                println!("  Direction:       {:?}", result.direction);
                println!("  Mechanism:       {}", result.mechanism);
                println!("  Time:            {:?}", elapsed);
            }
            Err(e) => {
                println!("Error: {:?}", e);
            }
        }
        println!();
    }

    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║                     Test Complete                              ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");

    Ok(())
}
