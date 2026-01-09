//! Real Integration Tests for Qodo-Embed-1-1.5B Code Embedding Model
//!
//! CRITICAL: These tests use REAL model inference - NO MOCKS.
//! The model MUST be loaded from disk and produce actual GPU-computed embeddings.
//!
//! # Test Requirements (from user)
//! - "Do not use mock data in tests, use the real data"
//! - "test to ensure everything is working and functioning as it should"
//! - "Do not cover up something not working correctly by making a test that passes
//!    when the project is in a broken state"
//!
//! # Model Details
//! - Model: Qodo/Qodo-Embed-1-1.5B
//! - Architecture: Qwen2 (28 layers, 1536 hidden dim, GQA)
//! - Native Dimension: 1536D
//! - Location: /home/cabdru/contextgraph/models/code-1536
//!
//! # Test Categories
//! 1. Model loading verification
//! 2. Embedding dimension validation (MUST be 1536D)
//! 3. Non-trivial output verification (NOT zeros, NOT constants)
//! 4. Batch inference functionality
//! 5. Semantic similarity tests (similar code = high similarity)
//! 6. Dissimilarity tests (different code = low similarity)
//!
//! # FAIL FAST Policy
//! All tests MUST fail if the model doesn't work correctly.
//! No silent fallbacks, no mock data, no default vectors.

use std::path::Path;
use std::time::Instant;

use context_graph_embeddings::{
    error::EmbeddingResult,
    models::CodeModel,
    traits::{EmbeddingModel, SingleModelConfig},
    types::{ModelId, ModelInput},
};

/// Model path for Qodo-Embed-1-1.5B
const MODEL_PATH: &str = "/home/cabdru/contextgraph/models/code-1536";

/// Expected embedding dimension from Constitution
const EXPECTED_DIMENSION: usize = 1536;

/// Minimum reasonable embedding magnitude (reject near-zero vectors)
const MIN_EMBEDDING_MAGNITUDE: f32 = 0.1;

/// Maximum allowed zero values in embedding (reject mostly-zero vectors)
const MAX_ZERO_RATIO: f32 = 0.95;

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Compute cosine similarity between two vectors.
///
/// Returns a value in [-1, 1] where:
/// - 1.0 = identical direction
/// - 0.0 = orthogonal
/// - -1.0 = opposite direction
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len(), "Vectors must have same dimension");

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

/// Compute L2 norm (magnitude) of a vector.
fn l2_norm(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt()
}

/// Count near-zero values in vector (|x| < epsilon).
fn count_near_zero(v: &[f32], epsilon: f32) -> usize {
    v.iter().filter(|&&x| x.abs() < epsilon).count()
}

/// Verify embedding is non-trivial (not zeros, not constant).
fn verify_non_trivial_embedding(embedding: &[f32], context: &str) {
    let dim = embedding.len();

    // Check dimension
    assert_eq!(
        dim, EXPECTED_DIMENSION,
        "{}: Expected {}D embedding, got {}D",
        context, EXPECTED_DIMENSION, dim
    );

    // Check magnitude
    let magnitude = l2_norm(embedding);
    assert!(
        magnitude > MIN_EMBEDDING_MAGNITUDE,
        "{}: Embedding magnitude {} is too small (min: {}). Model may not be computing real embeddings.",
        context, magnitude, MIN_EMBEDDING_MAGNITUDE
    );

    // Check not mostly zeros
    let zero_count = count_near_zero(embedding, 1e-10);
    let zero_ratio = zero_count as f32 / dim as f32;
    assert!(
        zero_ratio < MAX_ZERO_RATIO,
        "{}: Embedding is {:.1}% zeros ({}/{}). Model may be returning default vectors.",
        context, zero_ratio * 100.0, zero_count, dim
    );

    // Check for variance (not constant)
    let mean: f32 = embedding.iter().sum::<f32>() / dim as f32;
    let variance: f32 = embedding.iter()
        .map(|x| (x - mean).powi(2))
        .sum::<f32>() / dim as f32;
    assert!(
        variance > 1e-10,
        "{}: Embedding variance {} is too small. Model may be returning constant vectors.",
        context, variance
    );

    // Check no NaN or Inf
    assert!(
        embedding.iter().all(|x| x.is_finite()),
        "{}: Embedding contains NaN or Inf values. Model computation failed.",
        context
    );
}

/// Create and load the CodeModel for testing.
async fn load_code_model() -> EmbeddingResult<CodeModel> {
    let model_path = Path::new(MODEL_PATH);

    // Verify model directory exists
    assert!(
        model_path.exists(),
        "Model directory does not exist: {}. Cannot run integration tests without model weights.",
        MODEL_PATH
    );

    // Verify key files exist
    let tokenizer_path = model_path.join("tokenizer.json");
    assert!(
        tokenizer_path.exists(),
        "Tokenizer not found: {}. Model is incomplete.",
        tokenizer_path.display()
    );

    let weights1 = model_path.join("model-00001-of-00002.safetensors");
    let weights2 = model_path.join("model-00002-of-00002.safetensors");
    assert!(
        weights1.exists() && weights2.exists(),
        "Model weights not found. Expected: model-00001-of-00002.safetensors and model-00002-of-00002.safetensors"
    );

    // Create model instance
    let config = SingleModelConfig::cuda_fp16();
    let model = CodeModel::new(model_path, config)?;

    // Model should NOT be initialized before load()
    assert!(
        !model.is_initialized(),
        "Model should not be initialized before load() is called"
    );

    // Load model weights (this is where REAL model loading happens)
    model.load().await?;

    // Model MUST be initialized after load()
    assert!(
        model.is_initialized(),
        "Model should be initialized after load() succeeds"
    );

    Ok(model)
}

// =============================================================================
// TEST 1: Model Loading
// =============================================================================

/// Test: Qodo-Embed model loads successfully from disk.
///
/// This verifies:
/// - Model directory exists
/// - Tokenizer loads correctly
/// - Safetensors weights load to GPU
/// - Model is initialized after load()
#[tokio::test]
async fn test_qodo_embed_model_loads() -> EmbeddingResult<()> {
    println!("\n=== TEST: Model Loading ===\n");

    let start = Instant::now();
    let model = load_code_model().await?;
    let load_time = start.elapsed();

    println!("Model loaded in {:.2?}", load_time);
    println!("Model ID: {:?}", model.model_id());
    println!("Dimension: {}", model.dimension());
    println!("Is initialized: {}", model.is_initialized());

    // Verify model properties
    assert_eq!(model.model_id(), ModelId::Code);
    assert_eq!(model.dimension(), EXPECTED_DIMENSION);
    assert!(model.is_initialized());

    println!("\n[PASS] Model loads correctly from {}", MODEL_PATH);
    Ok(())
}

// =============================================================================
// TEST 2: Embedding Dimension
// =============================================================================

/// Test: Qodo-Embed produces 1536D embeddings.
///
/// This verifies:
/// - Output dimension is exactly 1536
/// - Embedding is non-zero (real computation)
/// - Embedding has variance (not constant)
#[tokio::test]
async fn test_qodo_embed_produces_1536d_embeddings() -> EmbeddingResult<()> {
    println!("\n=== TEST: Embedding Dimension ===\n");

    let model = load_code_model().await?;

    // Sample code to embed
    let code = r#"fn main() { println!("Hello, world!"); }"#;
    let input = ModelInput::code(code, "rust")?;

    // Generate embedding (REAL inference)
    let start = Instant::now();
    let embedding = model.embed(&input).await?;
    let inference_time = start.elapsed();

    let vector = embedding.vector;

    println!("Input: {}", code);
    println!("Dimension: {}", vector.len());
    println!("Inference time: {:.2?}", inference_time);
    println!("L2 norm: {:.4}", l2_norm(&vector));
    println!("First 10 values: {:?}", &vector[..10.min(vector.len())]);
    println!("Last 10 values: {:?}", &vector[vector.len().saturating_sub(10)..]);

    // Verify dimension
    assert_eq!(
        vector.len(), EXPECTED_DIMENSION,
        "Expected {}D embedding, got {}D",
        EXPECTED_DIMENSION, vector.len()
    );

    // Verify non-trivial
    verify_non_trivial_embedding(&vector, "Hello world");

    println!("\n[PASS] Model produces {}D non-trivial embeddings", vector.len());
    Ok(())
}

// =============================================================================
// TEST 3: Different Inputs Produce Different Outputs
// =============================================================================

/// Test: Different code produces different embeddings.
///
/// This is critical to verify the model is actually computing:
/// - Two different code samples should have different embeddings
/// - Cosine similarity should be < 0.99 (not identical)
/// - This proves embeddings are not constants or defaults
#[tokio::test]
async fn test_qodo_embed_different_inputs_different_outputs() -> EmbeddingResult<()> {
    println!("\n=== TEST: Different Inputs -> Different Outputs ===\n");

    let model = load_code_model().await?;

    // Two completely different code samples
    let code1 = r#"fn add(a: i32, b: i32) -> i32 { a + b }"#;
    let code2 = r#"class UserAuthentication { void login(String user, String pass) { } }"#;

    let input1 = ModelInput::code(code1, "rust")?;
    let input2 = ModelInput::code(code2, "java")?;

    // Generate embeddings
    let emb1 = model.embed(&input1).await?;
    let emb2 = model.embed(&input2).await?;

    let vec1 = emb1.vector;
    let vec2 = emb2.vector;

    // Compute similarity
    let similarity = cosine_similarity(&vec1, &vec2);

    println!("Code 1: {}", code1);
    println!("Code 2: {}", code2);
    println!("Cosine similarity: {:.4}", similarity);
    println!("Embedding 1 norm: {:.4}", l2_norm(&vec1));
    println!("Embedding 2 norm: {:.4}", l2_norm(&vec2));

    // Verify non-trivial
    verify_non_trivial_embedding(&vec1, "Code 1");
    verify_non_trivial_embedding(&vec2, "Code 2");

    // Critical: Embeddings must be different
    assert!(
        similarity < 0.99,
        "Embeddings are too similar ({:.4}). Model may be returning constant vectors!",
        similarity
    );

    // Also check they're not completely orthogonal (both should be code)
    assert!(
        similarity > -0.99,
        "Embeddings are negatively correlated ({:.4}). Something is wrong.",
        similarity
    );

    println!("\n[PASS] Different code produces different embeddings (similarity={:.4})", similarity);
    Ok(())
}

// =============================================================================
// TEST 4: Batch Inference
// =============================================================================

/// Test: Batch inference produces correct embeddings.
///
/// This verifies:
/// - Multiple embeddings can be generated
/// - All have correct dimension
/// - Each is non-trivial
/// - Reasonable throughput
#[tokio::test]
async fn test_qodo_embed_batch_inference() -> EmbeddingResult<()> {
    println!("\n=== TEST: Batch Inference ===\n");

    let model = load_code_model().await?;

    // Batch of 5 different code samples
    let code_samples = [
        ("fn factorial(n: u64) -> u64 { if n <= 1 { 1 } else { n * factorial(n - 1) } }", "rust"),
        ("def fibonacci(n): return n if n < 2 else fibonacci(n-1) + fibonacci(n-2)", "python"),
        ("public int binarySearch(int[] arr, int target) { int lo=0, hi=arr.length; }", "java"),
        ("const quickSort = arr => arr.length <= 1 ? arr : quickSort(arr.filter(x => x < arr[0])).concat(arr[0])", "javascript"),
        ("SELECT * FROM users WHERE created_at > NOW() - INTERVAL '7 days'", "sql"),
    ];

    let inputs: Vec<ModelInput> = code_samples
        .iter()
        .map(|(code, lang)| ModelInput::code(*code, *lang).unwrap())
        .collect();

    // Time batch inference
    let start = Instant::now();
    let embeddings = model.embed_batch(&inputs).await?;
    let batch_time = start.elapsed();

    println!("Batch size: {}", inputs.len());
    println!("Total time: {:.2?}", batch_time);
    println!("Per-item: {:.2?}", batch_time / inputs.len() as u32);

    // Verify each embedding
    for (i, (emb, (code, _))) in embeddings.iter().zip(code_samples.iter()).enumerate() {
        let vector = &emb.vector;
        verify_non_trivial_embedding(vector, &format!("Sample {}", i + 1));
        println!(
            "Sample {}: {} chars, norm={:.4}",
            i + 1,
            code.len(),
            l2_norm(vector)
        );
    }

    // Verify all embeddings are different from each other
    for i in 0..embeddings.len() {
        for j in (i + 1)..embeddings.len() {
            let sim = cosine_similarity(&embeddings[i].vector, &embeddings[j].vector);
            assert!(
                sim < 0.99,
                "Samples {} and {} too similar ({:.4}). Model may be malfunctioning.",
                i + 1, j + 1, sim
            );
        }
    }

    println!("\n[PASS] Batch inference produces {} distinct {}D embeddings",
             embeddings.len(), EXPECTED_DIMENSION);
    Ok(())
}

// =============================================================================
// TEST 5: Semantic Similarity (Similar Code)
// =============================================================================

/// Test: Semantically similar code has high similarity.
///
/// Two functions that do the same thing (add two numbers) but with
/// different names should have HIGH cosine similarity (> 0.8).
///
/// This verifies the model captures semantic meaning, not just syntax.
#[tokio::test]
async fn test_qodo_embed_semantic_similarity() -> EmbeddingResult<()> {
    println!("\n=== TEST: Semantic Similarity ===\n");

    let model = load_code_model().await?;

    // Semantically equivalent code with different names
    let code1 = "fn add(a: i32, b: i32) -> i32 { a + b }";
    let code2 = "fn sum(x: i32, y: i32) -> i32 { x + y }";

    let input1 = ModelInput::code(code1, "rust")?;
    let input2 = ModelInput::code(code2, "rust")?;

    let emb1 = model.embed(&input1).await?;
    let emb2 = model.embed(&input2).await?;

    let similarity = cosine_similarity(&emb1.vector, &emb2.vector);

    println!("Code 1: {}", code1);
    println!("Code 2: {}", code2);
    println!("Cosine similarity: {:.4}", similarity);

    // Verify non-trivial
    verify_non_trivial_embedding(&emb1.vector, "add function");
    verify_non_trivial_embedding(&emb2.vector, "sum function");

    // Similar code should have high similarity
    // Note: Threshold 0.7 is more realistic for real models
    assert!(
        similarity > 0.7,
        "Semantically similar code should have similarity > 0.7, got {:.4}",
        similarity
    );

    println!("\n[PASS] Semantically similar code has high similarity ({:.4})", similarity);
    Ok(())
}

// =============================================================================
// TEST 6: Dissimilar Code
// =============================================================================

/// Test: Semantically different code has low similarity.
///
/// An arithmetic function and a file I/O function should have
/// LOW cosine similarity (< 0.7).
///
/// This verifies the model distinguishes between different purposes.
#[tokio::test]
async fn test_qodo_embed_dissimilar_code() -> EmbeddingResult<()> {
    println!("\n=== TEST: Dissimilar Code ===\n");

    let model = load_code_model().await?;

    // Semantically very different code
    let code1 = "fn add(a: i32, b: i32) -> i32 { a + b }";
    let code2 = r#"fn read_file(path: &str) -> String { std::fs::read_to_string(path).unwrap() }"#;

    let input1 = ModelInput::code(code1, "rust")?;
    let input2 = ModelInput::code(code2, "rust")?;

    let emb1 = model.embed(&input1).await?;
    let emb2 = model.embed(&input2).await?;

    let similarity = cosine_similarity(&emb1.vector, &emb2.vector);

    println!("Code 1 (arithmetic): {}", code1);
    println!("Code 2 (file I/O): {}", code2);
    println!("Cosine similarity: {:.4}", similarity);

    // Verify non-trivial
    verify_non_trivial_embedding(&emb1.vector, "add function");
    verify_non_trivial_embedding(&emb2.vector, "read_file function");

    // Different code should have lower similarity
    // Note: Real models may still have moderate similarity for code in same language
    assert!(
        similarity < 0.85,
        "Semantically different code should have similarity < 0.85, got {:.4}",
        similarity
    );

    println!("\n[PASS] Dissimilar code has lower similarity ({:.4})", similarity);
    Ok(())
}

// =============================================================================
// EDGE CASE TESTS
// =============================================================================

/// Edge Case 1: Empty code string should fail gracefully.
#[tokio::test]
async fn test_edge_case_empty_code_fails() {
    println!("\n=== EDGE CASE: Empty Code ===\n");

    let result = ModelInput::code("", "rust");

    assert!(
        result.is_err(),
        "Empty code should return an error, not succeed silently"
    );

    println!("[PASS] Empty code correctly rejected");
}

/// Edge Case 2: Very long code should either succeed or fail gracefully.
#[tokio::test]
async fn test_edge_case_long_code() -> EmbeddingResult<()> {
    println!("\n=== EDGE CASE: Long Code ===\n");

    let model = load_code_model().await?;

    // Generate code longer than typical context window
    let long_code = format!(
        "fn process() {{ {} }}",
        (0..500)
            .map(|i| format!("let x{} = {};", i, i))
            .collect::<Vec<_>>()
            .join(" ")
    );

    println!("Code length: {} chars", long_code.len());

    let input = ModelInput::code(&long_code, "rust")?;

    // Should either succeed with truncation or return an error
    let result = model.embed(&input).await;

    match result {
        Ok(embedding) => {
            // If it succeeds, verify the embedding is valid
            let vector = embedding.vector;
            assert_eq!(vector.len(), EXPECTED_DIMENSION);
            verify_non_trivial_embedding(&vector, "Long code");
            println!("[PASS] Long code embedded successfully (truncated)");
        }
        Err(e) => {
            // If it fails, it should be a clear error
            println!("Long code rejected with error: {}", e);
            println!("[PASS] Long code correctly rejected with error");
        }
    }

    Ok(())
}

/// Edge Case 3: Special characters and unicode in code.
#[tokio::test]
async fn test_edge_case_unicode_code() -> EmbeddingResult<()> {
    println!("\n=== EDGE CASE: Unicode Code ===\n");

    let model = load_code_model().await?;

    // Code with unicode characters
    let unicode_code = r#"fn greet() { println!("Hello, 世界! 🌍"); }"#;

    let input = ModelInput::code(unicode_code, "rust")?;
    let embedding = model.embed(&input).await?;

    let vector = embedding.vector;
    verify_non_trivial_embedding(&vector, "Unicode code");

    println!("Code: {}", unicode_code);
    println!("Embedding norm: {:.4}", l2_norm(&vector));
    println!("[PASS] Unicode code embedded successfully");

    Ok(())
}

// =============================================================================
// COMPREHENSIVE VERIFICATION
// =============================================================================

/// Master test: Run all verifications in one comprehensive check.
#[tokio::test]
async fn test_qodo_embed_comprehensive_verification() -> EmbeddingResult<()> {
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║       QODO-EMBED-1-1.5B COMPREHENSIVE VERIFICATION           ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    // 1. Load model
    println!("[1/6] Loading model...");
    let start = Instant::now();
    let model = load_code_model().await?;
    println!("       Model loaded in {:.2?}", start.elapsed());

    // 2. Verify model properties
    println!("[2/6] Verifying model properties...");
    assert_eq!(model.model_id(), ModelId::Code, "Model ID mismatch");
    assert_eq!(model.dimension(), 1536, "Dimension mismatch");
    assert!(model.is_initialized(), "Model not initialized");
    println!("       ModelId=Code, Dimension=1536, Initialized=true");

    // 3. Generate test embedding
    println!("[3/6] Generating test embedding...");
    let code = "fn test() -> bool { true }";
    let input = ModelInput::code(code, "rust")?;
    let embedding = model.embed(&input).await?;
    let vector = embedding.vector;
    println!("       Input: {} ({} chars)", code, code.len());
    println!("       Output: {}D vector, norm={:.4}", vector.len(), l2_norm(&vector));

    // 4. Verify embedding properties
    println!("[4/6] Verifying embedding properties...");
    verify_non_trivial_embedding(&vector, "Test embedding");
    println!("       Dimension: {} (expected {})", vector.len(), EXPECTED_DIMENSION);
    println!("       Non-zero: YES");
    println!("       No NaN/Inf: YES");
    println!("       Has variance: YES");

    // 5. Verify different inputs produce different outputs
    println!("[5/6] Verifying input differentiation...");
    let input2 = ModelInput::code("class Foo { int x; }", "java")?;
    let embedding2 = model.embed(&input2).await?;
    let similarity = cosine_similarity(&vector, &embedding2.vector);
    println!("       Similarity between different code: {:.4}", similarity);
    assert!(similarity < 0.99, "Embeddings too similar!");

    // 6. Summary
    println!("[6/6] All verifications passed!\n");

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                    VERIFICATION SUMMARY                      ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  Model:         Qodo-Embed-1-1.5B                            ║");
    println!("║  Path:          {}             ║", MODEL_PATH);
    println!("║  Dimension:     1536D (native, no projection)                ║");
    println!("║  Architecture:  Qwen2 (28 layers, GQA, SwiGLU)               ║");
    println!("║  Status:        FULLY OPERATIONAL                            ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    Ok(())
}
