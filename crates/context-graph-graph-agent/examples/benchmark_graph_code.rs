//! Benchmark for Graph Relationship Discovery using REAL CODE pairs.
//!
//! Run with: cargo run -p context-graph-graph-agent --example benchmark_graph_code --features cuda --release
//!
//! This benchmark tests the graph discovery service against actual Rust code
//! with known structural relationships (imports, dependencies, implementations).

use std::collections::HashMap;
use std::fs::File;
use std::sync::Arc;
use std::time::Instant;

use context_graph_causal_agent::llm::{CausalDiscoveryLLM, LlmConfig};
use context_graph_graph_agent::{GraphDiscoveryConfig, GraphDiscoveryService, RelationshipType};
use serde::Serialize;

/// A code pair with known relationship for ground truth comparison.
struct CodePair {
    name: &'static str,
    code_a: &'static str,
    code_b: &'static str,
    expected_relationship: RelationshipType,
    expected_has_connection: bool,
}

/// Benchmark result for a single pair.
#[derive(Debug, Serialize)]
struct PairResult {
    name: String,
    code_a_snippet: String,
    code_b_snippet: String,
    expected_relationship: String,
    expected_has_connection: bool,
    actual_relationship: String,
    actual_has_connection: bool,
    confidence: f32,
    direction: String,
    description: String,
    inference_time_ms: u64,
    is_correct: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// Overall benchmark results.
#[derive(Debug, Serialize)]
struct BenchmarkResults {
    total_pairs: usize,
    correct_predictions: usize,
    accuracy: f64,
    avg_confidence: f32,
    avg_inference_time_ms: f64,
    relationship_accuracy: HashMap<String, (usize, usize)>, // (correct, total)
    model_load_time_sec: f64,
    total_time_sec: f64,
}

/// Get test code pairs with known relationships.
fn get_code_pairs() -> Vec<CodePair> {
    vec![
        // =========================================================================
        // IMPORTS RELATIONSHIPS
        // =========================================================================
        CodePair {
            name: "rust_use_statement",
            code_a: r#"
// File: src/handlers/mod.rs
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::protocol::JsonRpcRequest;
use crate::tools::ToolRegistry;

pub struct Handlers {
    store: Arc<dyn MemoryStore>,
    registry: ToolRegistry,
}
"#,
            code_b: r#"
// File: src/protocol/mod.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: Option<serde_json::Value>,
    pub id: Option<JsonRpcId>,
}
"#,
            expected_relationship: RelationshipType::Imports,
            expected_has_connection: true,
        },
        CodePair {
            name: "python_import",
            code_a: r#"
# File: app/services/user_service.py
from typing import Optional
from app.models.user import User
from app.repositories.user_repository import UserRepository

class UserService:
    def __init__(self, repo: UserRepository):
        self.repo = repo

    def get_user(self, user_id: int) -> Optional[User]:
        return self.repo.find_by_id(user_id)
"#,
            code_b: r#"
# File: app/models/user.py
from dataclasses import dataclass
from datetime import datetime

@dataclass
class User:
    id: int
    email: str
    name: str
    created_at: datetime
"#,
            expected_relationship: RelationshipType::Imports,
            expected_has_connection: true,
        },
        // =========================================================================
        // IMPLEMENTS RELATIONSHIPS
        // =========================================================================
        CodePair {
            name: "rust_impl_trait",
            code_a: r#"
// File: src/storage/memory.rs
use async_trait::async_trait;
use crate::traits::MemoryStore;

pub struct InMemoryStore {
    data: DashMap<Uuid, Memory>,
}

#[async_trait]
impl MemoryStore for InMemoryStore {
    async fn store(&self, memory: Memory) -> Result<Uuid> {
        let id = memory.id;
        self.data.insert(id, memory);
        Ok(id)
    }

    async fn retrieve(&self, id: Uuid) -> Result<Option<Memory>> {
        Ok(self.data.get(&id).map(|r| r.clone()))
    }
}
"#,
            code_b: r#"
// File: src/traits/mod.rs
use async_trait::async_trait;

#[async_trait]
pub trait MemoryStore: Send + Sync {
    async fn store(&self, memory: Memory) -> Result<Uuid>;
    async fn retrieve(&self, id: Uuid) -> Result<Option<Memory>>;
    async fn delete(&self, id: Uuid) -> Result<bool>;
}
"#,
            expected_relationship: RelationshipType::Implements,
            expected_has_connection: true,
        },
        // =========================================================================
        // CALLS RELATIONSHIPS
        // =========================================================================
        CodePair {
            name: "function_call",
            code_a: r#"
// File: src/api/handlers.rs
async fn create_user(req: CreateUserRequest) -> Result<Response> {
    let user = User::new(&req.email, &req.name);
    let validated = validate_user(&user)?;
    let stored = user_service.save_user(validated).await?;
    Ok(Response::json(stored))
}
"#,
            code_b: r#"
// File: src/validation/user.rs
pub fn validate_user(user: &User) -> Result<ValidatedUser> {
    if user.email.is_empty() {
        return Err(ValidationError::EmptyEmail);
    }
    if !user.email.contains('@') {
        return Err(ValidationError::InvalidEmail);
    }
    Ok(ValidatedUser::from(user))
}
"#,
            expected_relationship: RelationshipType::Calls,
            expected_has_connection: true,
        },
        // =========================================================================
        // DEPENDS_ON RELATIONSHIPS
        // =========================================================================
        CodePair {
            name: "cargo_dependency",
            code_a: r#"
[package]
name = "context-graph-mcp"
version = "0.1.0"

[dependencies]
context-graph-core = { path = "../context-graph-core" }
context-graph-embeddings = { path = "../context-graph-embeddings" }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
"#,
            code_b: r#"
[package]
name = "context-graph-core"
version = "0.1.0"

[dependencies]
uuid = { version = "1", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
serde = { version = "1", features = ["derive"] }
"#,
            expected_relationship: RelationshipType::DependsOn,
            expected_has_connection: true,
        },
        // =========================================================================
        // CONTAINS RELATIONSHIPS
        // =========================================================================
        CodePair {
            name: "module_contains_struct",
            code_a: r#"
// File: src/embeddings/mod.rs
pub mod e1_semantic;
pub mod e5_causal;
pub mod e7_code;

pub use e1_semantic::SemanticEmbedder;
pub use e5_causal::CausalEmbedder;
pub use e7_code::CodeEmbedder;

pub struct EmbeddingPipeline {
    semantic: SemanticEmbedder,
    causal: CausalEmbedder,
    code: CodeEmbedder,
}
"#,
            code_b: r#"
// File: src/embeddings/e1_semantic.rs
use crate::traits::Embedder;

pub struct SemanticEmbedder {
    model: E5Model,
    dimension: usize,
}

impl SemanticEmbedder {
    pub fn new() -> Self {
        Self {
            model: E5Model::load("e5-large-v2"),
            dimension: 1024,
        }
    }

    pub fn embed(&self, text: &str) -> Vec<f32> {
        self.model.encode(text)
    }
}
"#,
            expected_relationship: RelationshipType::Contains,
            expected_has_connection: true,
        },
        // =========================================================================
        // EXTENDS RELATIONSHIPS
        // =========================================================================
        CodePair {
            name: "class_inheritance",
            code_a: r#"
// File: errors/api_error.py
class ApiError(BaseError):
    def __init__(self, message: str, status_code: int):
        super().__init__(message)
        self.status_code = status_code

    def to_response(self) -> dict:
        return {
            "error": self.message,
            "status": self.status_code
        }
"#,
            code_b: r#"
// File: errors/base.py
class BaseError(Exception):
    def __init__(self, message: str):
        self.message = message
        super().__init__(message)

    def __str__(self) -> str:
        return self.message
"#,
            expected_relationship: RelationshipType::Extends,
            expected_has_connection: true,
        },
        // =========================================================================
        // REFERENCES RELATIONSHIPS
        // =========================================================================
        CodePair {
            name: "doc_reference",
            code_a: r#"
/// Memory storage implementation.
///
/// See also:
/// - [`TeleologicalMemoryStore`] for the trait definition
/// - [`InMemoryStore`] for the in-memory implementation
/// - https://docs.rs/context-graph-core/memory
///
/// # Examples
/// ```rust
/// let store = RocksDBStore::new("./data");
/// store.store(memory).await?;
/// ```
pub struct RocksDBStore {
    db: DB,
    column_family: String,
}
"#,
            code_b: r#"
/// Trait for teleological memory storage.
///
/// All memory stores must implement this trait.
/// See the module documentation for implementation guidelines.
#[async_trait]
pub trait TeleologicalMemoryStore: Send + Sync {
    async fn store(&self, fp: TeleologicalFingerprint) -> Result<Uuid>;
    async fn retrieve(&self, id: Uuid) -> Result<Option<TeleologicalFingerprint>>;
    async fn count(&self) -> Result<usize>;
}
"#,
            expected_relationship: RelationshipType::References,
            expected_has_connection: true,
        },
        // =========================================================================
        // NO RELATIONSHIP (NEGATIVE CASES)
        // =========================================================================
        CodePair {
            name: "unrelated_modules",
            code_a: r#"
// File: src/logging/logger.rs
use tracing::{info, warn, error};

pub struct Logger {
    level: LogLevel,
}

impl Logger {
    pub fn info(&self, msg: &str) {
        info!(msg);
    }
}
"#,
            code_b: r#"
// File: src/math/statistics.rs
pub fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

pub fn std_dev(values: &[f64]) -> f64 {
    let m = mean(values);
    let variance = values.iter().map(|v| (v - m).powi(2)).sum::<f64>() / values.len() as f64;
    variance.sqrt()
}
"#,
            expected_relationship: RelationshipType::None,
            expected_has_connection: false,
        },
        CodePair {
            name: "different_domains",
            code_a: r#"
// Database connection pool
pub struct ConnectionPool {
    connections: Vec<Connection>,
    max_size: usize,
}

impl ConnectionPool {
    pub async fn acquire(&self) -> Result<Connection> {
        // Get connection from pool
    }
}
"#,
            code_b: r#"
// Image processing utilities
pub fn resize_image(img: &Image, width: u32, height: u32) -> Image {
    img.resize(width, height, FilterType::Lanczos3)
}

pub fn apply_grayscale(img: &mut Image) {
    img.grayscale();
}
"#,
            expected_relationship: RelationshipType::None,
            expected_has_connection: false,
        },
    ]
}

fn truncate(s: &str, max_len: usize) -> String {
    let s = s.replace('\n', " ").trim().to_string();
    if s.len() <= max_len {
        format!("{:width$}", s, width = max_len)
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("╔═══════════════════════════════════════════════════════════════════════════════╗");
    println!("║      Graph Relationship Discovery Benchmark (Real Code Pairs)                 ║");
    println!("║                 Testing Structural Relationship Detection                      ║");
    println!("╚═══════════════════════════════════════════════════════════════════════════════╝\n");

    // Find workspace root
    let mut workspace_root = std::env::current_dir()?;
    while !workspace_root.join("models").exists() {
        if !workspace_root.pop() {
            return Err("Could not find workspace root".into());
        }
    }

    let model_dir = workspace_root.join("models/hermes-2-pro");
    println!("Model directory: {:?}\n", model_dir);

    // Get code pairs
    let pairs = get_code_pairs();
    println!(
        "Loaded {} code pairs with known relationships\n",
        pairs.len()
    );

    // Initialize LLM with Hermes 2 Pro and GBNF grammar constraints
    let config = LlmConfig {
        model_path: model_dir.join("Hermes-2-Pro-Mistral-7B.Q5_K_M.gguf"),
        causal_grammar_path: model_dir.join("causal_analysis.gbnf"),
        graph_grammar_path: model_dir.join("graph_relationship.gbnf"),
        validation_grammar_path: model_dir.join("validation.gbnf"),
        n_gpu_layers: u32::MAX, // Full GPU offload
        temperature: 0.0,       // Deterministic for reliable JSON output
        max_tokens: 256,
        ..Default::default()
    };

    println!("Initializing CausalDiscoveryLLM...");
    let llm = CausalDiscoveryLLM::with_config(config)?;

    println!("Loading model (~6GB VRAM required)...");
    let model_load_start = Instant::now();
    llm.load().await?;
    let model_load_time = model_load_start.elapsed();
    println!("✓ Model loaded in {:.2?}\n", model_load_time);

    // Create GraphDiscoveryService
    let graph_config = GraphDiscoveryConfig {
        min_confidence: 0.5,
        ..Default::default()
    };

    let service = GraphDiscoveryService::with_config(Arc::new(llm), graph_config);
    let llm = service.llm();

    // Run benchmark
    println!("Running code relationship benchmark...\n");
    println!(
        "┌────┬──────────────────────────────────────────────────────────────────────────────┐"
    );
    println!("│ #  │ Pair Name                    │ Expected    │ Actual      │ Conf │ Time │ ✓ │");
    println!(
        "├────┼──────────────────────────────────────────────────────────────────────────────┤"
    );

    let benchmark_start = Instant::now();
    let mut results: Vec<PairResult> = Vec::new();
    let mut correct_count = 0;
    let mut relationship_accuracy: HashMap<String, (usize, usize)> = HashMap::new();

    for (i, pair) in pairs.iter().enumerate() {
        let start = Instant::now();
        let analysis = llm.analyze_relationship(pair.code_a, pair.code_b).await;
        let elapsed = start.elapsed();

        match analysis {
            Ok(result) => {
                let is_correct = (result.has_connection == pair.expected_has_connection)
                    && (result.relationship_type == pair.expected_relationship
                        || (!pair.expected_has_connection && !result.has_connection));

                if is_correct {
                    correct_count += 1;
                }

                // Track per-relationship accuracy
                let key = pair.expected_relationship.as_str().to_string();
                let entry = relationship_accuracy.entry(key).or_insert((0, 0));
                entry.1 += 1;
                if is_correct {
                    entry.0 += 1;
                }

                let symbol = if is_correct { "✓" } else { "✗" };
                let expected_str = if pair.expected_has_connection {
                    pair.expected_relationship.as_str()
                } else {
                    "none"
                };
                let actual_str = if result.has_connection {
                    result.relationship_type.as_str()
                } else {
                    "none"
                };

                println!(
                    "│ {:>2} │ {:28} │ {:11} │ {:11} │ {:.2} │ {:4}ms │ {} │",
                    i + 1,
                    truncate(pair.name, 28),
                    expected_str,
                    actual_str,
                    result.confidence,
                    elapsed.as_millis(),
                    symbol
                );

                results.push(PairResult {
                    name: pair.name.to_string(),
                    code_a_snippet: truncate(pair.code_a, 100),
                    code_b_snippet: truncate(pair.code_b, 100),
                    expected_relationship: pair.expected_relationship.as_str().to_string(),
                    expected_has_connection: pair.expected_has_connection,
                    actual_relationship: result.relationship_type.as_str().to_string(),
                    actual_has_connection: result.has_connection,
                    confidence: result.confidence,
                    direction: result.direction.as_str().to_string(),
                    description: result.description,
                    inference_time_ms: elapsed.as_millis() as u64,
                    is_correct,
                    error: None,
                });
            }
            Err(e) => {
                let error_msg = format!("{:?}", e);
                println!(
                    "│ {:>2} │ {:28} │ ERROR: {}",
                    i + 1,
                    pair.name,
                    truncate(&error_msg, 50)
                );

                // Still record the result so we can analyze failures
                results.push(PairResult {
                    name: pair.name.to_string(),
                    code_a_snippet: truncate(pair.code_a, 100),
                    code_b_snippet: truncate(pair.code_b, 100),
                    expected_relationship: pair.expected_relationship.as_str().to_string(),
                    expected_has_connection: pair.expected_has_connection,
                    actual_relationship: "error".to_string(),
                    actual_has_connection: false,
                    confidence: 0.0,
                    direction: "none".to_string(),
                    description: String::new(),
                    inference_time_ms: elapsed.as_millis() as u64,
                    is_correct: false,
                    error: Some(error_msg),
                });
            }
        }
    }

    println!(
        "└────┴──────────────────────────────────────────────────────────────────────────────┘\n"
    );

    let total_time = benchmark_start.elapsed();

    // Calculate statistics
    let accuracy = correct_count as f64 / results.len() as f64;
    let avg_confidence: f32 =
        results.iter().map(|r| r.confidence).sum::<f32>() / results.len() as f32;
    let avg_inference_time: f64 = results
        .iter()
        .map(|r| r.inference_time_ms as f64)
        .sum::<f64>()
        / results.len() as f64;

    let benchmark_results = BenchmarkResults {
        total_pairs: results.len(),
        correct_predictions: correct_count,
        accuracy,
        avg_confidence,
        avg_inference_time_ms: avg_inference_time,
        relationship_accuracy: relationship_accuracy.clone(),
        model_load_time_sec: model_load_time.as_secs_f64(),
        total_time_sec: total_time.as_secs_f64(),
    };

    // Print summary
    println!("╔═══════════════════════════════════════════════════════════════════════════════╗");
    println!("║                           BENCHMARK RESULTS                                    ║");
    println!("╠═══════════════════════════════════════════════════════════════════════════════╣");
    println!("║ Total Pairs:                {:>50} ║", results.len());
    println!("║ Correct Predictions:        {:>50} ║", correct_count);
    println!(
        "║ Accuracy:                   {:>49.1}% ║",
        accuracy * 100.0
    );
    println!("║ Average Confidence:         {:>50.3} ║", avg_confidence);
    println!(
        "║ Average Inference Time:     {:>47.0}ms ║",
        avg_inference_time
    );
    println!(
        "║ Model Load Time:            {:>48.2}s ║",
        model_load_time.as_secs_f64()
    );
    println!(
        "║ Total Benchmark Time:       {:>48.2}s ║",
        total_time.as_secs_f64()
    );
    println!("╠═══════════════════════════════════════════════════════════════════════════════╣");
    println!("║ PER-RELATIONSHIP ACCURACY                                                     ║");
    for (rel_type, (correct, total)) in &relationship_accuracy {
        let rel_acc = *correct as f64 / *total as f64 * 100.0;
        println!(
            "║   {:15}: {:>2}/{:<2} ({:>5.1}%)                                             ║",
            rel_type, correct, total, rel_acc
        );
    }
    println!("╚═══════════════════════════════════════════════════════════════════════════════╝");

    // Save results
    let output_path = workspace_root.join("benchmark_results/graph_code_benchmark.json");
    std::fs::create_dir_all(output_path.parent().unwrap())?;
    let output_file = File::create(&output_path)?;
    serde_json::to_writer_pretty(output_file, &benchmark_results)?;
    println!("\nResults saved to: {:?}", output_path);

    let detailed_path = workspace_root.join("benchmark_results/graph_code_benchmark_detailed.json");
    let detailed_file = File::create(&detailed_path)?;
    serde_json::to_writer_pretty(detailed_file, &results)?;
    println!("Detailed results saved to: {:?}", detailed_path);

    Ok(())
}
