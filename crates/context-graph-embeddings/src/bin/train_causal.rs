//! Training binary for E5 causal embedder fine-tuning.
//!
//! Runs the 3-stage progressive training pipeline:
//! 1. Projection-only warm-up with easy pairs
//! 2. LoRA activation with all pairs
//! 3. Directional emphasis
//!
//! # Usage
//!
//! ```bash
//! train-causal --model-path models/causal --output models/causal/trained
//! train-causal --model-path models/causal --data pairs.jsonl --epochs 50
//! ```

use std::path::PathBuf;

use candle_core::Device;

use context_graph_embeddings::models::pretrained::{
    load_nomic_weights, CAUSAL_DIMENSION,
};
use context_graph_embeddings::training::data::{
    expand_seed_pairs, seed_training_pairs, CausalTrainingPair,
};
use context_graph_embeddings::training::lora::LoraConfig;
use context_graph_embeddings::training::pipeline::{CausalTrainingPipeline, PipelineConfig};

/// CLI arguments.
struct Args {
    /// Path to the nomic-embed-text-v1.5 model directory.
    model_path: PathBuf,
    /// Optional path to additional training data (JSONL format).
    data_path: Option<PathBuf>,
    /// Output directory for checkpoints.
    output: PathBuf,
    /// Total epochs across all stages (overrides per-stage defaults).
    epochs: Option<u32>,
    /// Batch size.
    batch_size: usize,
    /// LoRA rank.
    lora_rank: usize,
    /// Evaluation frequency (every N epochs).
    eval_every: u32,
    /// Random seed.
    seed: u64,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            model_path: PathBuf::from("models/causal"),
            data_path: None,
            output: PathBuf::from("models/causal/trained"),
            epochs: None,
            batch_size: 16,
            lora_rank: 16,
            eval_every: 5,
            seed: 42,
        }
    }
}

fn parse_args() -> Args {
    let args: Vec<String> = std::env::args().collect();
    let mut result = Args::default();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--model-path" | "-m" => {
                i += 1;
                if i < args.len() {
                    result.model_path = PathBuf::from(&args[i]);
                }
            }
            "--data" | "-d" => {
                i += 1;
                if i < args.len() {
                    result.data_path = Some(PathBuf::from(&args[i]));
                }
            }
            "--output" | "-o" => {
                i += 1;
                if i < args.len() {
                    result.output = PathBuf::from(&args[i]);
                }
            }
            "--epochs" | "-e" => {
                i += 1;
                if i < args.len() {
                    result.epochs = args[i].parse().ok();
                }
            }
            "--batch-size" | "-b" => {
                i += 1;
                if i < args.len() {
                    result.batch_size = args[i].parse().unwrap_or(16);
                }
            }
            "--lora-rank" => {
                i += 1;
                if i < args.len() {
                    result.lora_rank = args[i].parse().unwrap_or(16);
                }
            }
            "--eval-every" => {
                i += 1;
                if i < args.len() {
                    result.eval_every = args[i].parse().unwrap_or(5);
                }
            }
            "--seed" => {
                i += 1;
                if i < args.len() {
                    result.seed = args[i].parse().unwrap_or(42);
                }
            }
            "--help" | "-h" => {
                println!("train-causal: E5 causal embedder fine-tuning");
                println!();
                println!("Usage: train-causal [OPTIONS]");
                println!();
                println!("Options:");
                println!("  -m, --model-path <PATH>  Model directory (default: models/causal)");
                println!("  -d, --data <PATH>        Additional JSONL training data");
                println!("  -o, --output <PATH>      Output directory (default: models/causal/trained)");
                println!("  -e, --epochs <N>         Total epochs (overrides per-stage defaults)");
                println!("  -b, --batch-size <N>     Batch size (default: 16)");
                println!("      --lora-rank <N>      LoRA rank (default: 16)");
                println!("      --eval-every <N>     Evaluate every N epochs (default: 5)");
                println!("      --seed <N>           Random seed (default: 42)");
                println!("  -h, --help               Show this help");
                std::process::exit(0);
            }
            other => {
                eprintln!("Unknown argument: {}", other);
                std::process::exit(1);
            }
        }
        i += 1;
    }

    result
}

/// Load additional training pairs from JSONL file.
fn load_jsonl_pairs(path: &std::path::Path) -> Vec<CausalTrainingPair> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Warning: Failed to read {}: {}", path.display(), e);
            return Vec::new();
        }
    };

    let mut pairs = Vec::new();
    for (line_num, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        match serde_json::from_str::<CausalTrainingPair>(line) {
            Ok(pair) => pairs.push(pair),
            Err(e) => {
                eprintln!("Warning: Line {} parse error: {}", line_num + 1, e);
            }
        }
    }

    println!("Loaded {} pairs from {}", pairs.len(), path.display());
    pairs
}

fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_target(false)
        .with_timer(tracing_subscriber::fmt::time::uptime())
        .init();

    let args = parse_args();

    println!("=== E5 Causal Embedder Training ===");
    println!("Model path: {}", args.model_path.display());
    println!("Output:     {}", args.output.display());
    println!("Batch size: {}", args.batch_size);
    println!("LoRA rank:  {}", args.lora_rank);
    println!("Seed:       {}", args.seed);
    println!();

    // Select device
    let device = if candle_core::utils::cuda_is_available() {
        println!("Using CUDA GPU");
        Device::new_cuda(0).expect("Failed to initialize CUDA device")
    } else {
        println!("Warning: CUDA not available, using CPU (training will be slow)");
        Device::Cpu
    };

    // Load model weights
    println!("Loading nomic-embed-text-v1.5 weights...");
    let device_ref: &'static Device = Box::leak(Box::new(device.clone()));
    let weights = load_nomic_weights(&args.model_path, device_ref)
        .expect("Failed to load model weights");
    println!(
        "Model loaded: {} layers, hidden_size={}",
        weights.encoder_layers.len(),
        weights.config.hidden_size
    );

    // Load tokenizer
    let tokenizer_path = args.model_path.join("tokenizer.json");
    let tokenizer = tokenizers::Tokenizer::from_file(&tokenizer_path)
        .expect("Failed to load tokenizer");
    println!("Tokenizer loaded from {}", tokenizer_path.display());

    // Build training data
    println!("\nPreparing training data...");
    let mut all_pairs = seed_training_pairs();
    println!("Seed pairs: {}", all_pairs.len());

    // Load additional data if provided
    if let Some(ref data_path) = args.data_path {
        let extra = load_jsonl_pairs(data_path);
        all_pairs.extend(extra);
    }

    // Expand seed pairs programmatically
    let expanded = expand_seed_pairs(&all_pairs);
    println!("Expanded pairs: {}", expanded.len());

    // Split train/eval (80/20)
    let mut rng_pairs = expanded;
    use rand::seq::SliceRandom;
    use rand::SeedableRng;
    let mut rng = rand::rngs::StdRng::seed_from_u64(args.seed);
    rng_pairs.shuffle(&mut rng);

    let eval_count = (rng_pairs.len() as f32 * 0.2).ceil() as usize;
    let eval_pairs: Vec<CausalTrainingPair> =
        rng_pairs.drain(rng_pairs.len() - eval_count..).collect();
    let train_pairs = rng_pairs;

    println!(
        "Train: {} pairs, Eval: {} pairs",
        train_pairs.len(),
        eval_pairs.len()
    );

    // Configure pipeline
    let mut pipeline_config = PipelineConfig {
        lora_config: LoraConfig {
            rank: args.lora_rank,
            hidden_size: CAUSAL_DIMENSION,
            ..Default::default()
        },
        batch_size: args.batch_size,
        eval_every: args.eval_every,
        output_dir: args.output.clone(),
        seed: args.seed,
        ..Default::default()
    };

    // Override per-stage epochs if total specified
    if let Some(total) = args.epochs {
        let stage1 = total / 5; // 20% warm-up
        let stage2 = total * 2 / 5; // 40% LoRA
        let stage3 = total - stage1 - stage2; // 40% directional
        pipeline_config.stage1_epochs = stage1.max(1);
        pipeline_config.stage2_epochs = stage2.max(1);
        pipeline_config.stage3_epochs = stage3.max(1);
    }

    println!("\nPipeline configuration:");
    println!(
        "  Stage 1 (warm-up):    {} epochs",
        pipeline_config.stage1_epochs
    );
    println!(
        "  Stage 2 (LoRA):       {} epochs",
        pipeline_config.stage2_epochs
    );
    println!(
        "  Stage 3 (direction):  {} epochs",
        pipeline_config.stage3_epochs
    );
    println!(
        "  LoRA params:          ~{}",
        pipeline_config.lora_config.total_params()
    );
    println!(
        "  Projection params:    {}",
        CAUSAL_DIMENSION * CAUSAL_DIMENSION * 2 + CAUSAL_DIMENSION * 2
    );
    println!();

    // Create and run pipeline
    let pipeline = CausalTrainingPipeline::new(pipeline_config, device)
        .expect("Failed to create pipeline");

    println!("Starting 3-stage training...\n");

    let result = pipeline
        .run_full_pipeline(train_pairs, eval_pairs, &weights, &tokenizer)
        .expect("Training failed");

    // Print results
    println!("\n=== Training Complete ===");
    println!("Total epochs: {}", result.total_epochs);
    println!("Stages completed: {}", result.stages.len());

    for stage in &result.stages {
        println!(
            "  Stage {}: {} epochs, loss={:.4}, early_stopped={}",
            stage.stage,
            stage.epochs_completed,
            stage.final_loss.total,
            stage.early_stopped
        );
        if let Some(ref m) = stage.best_metrics {
            println!("    Best: {}", m.summary());
        }
    }

    if let Some(ref m) = result.best_metrics {
        println!("\nBest overall metrics:");
        println!("  {}", m.summary());

        if m.meets_finetuning_targets() {
            println!("\n  PASS: Fine-tuning targets met!");
        } else {
            println!("\n  FAIL: Fine-tuning targets not yet met.");
            println!("    Targets: spread > 0.10, anisotropy < 0.30, standalone >= 0.67");
        }
    }

    if let Some(ref path) = result.checkpoint_path {
        println!("\nBest checkpoint: {}", path.display());
    }

    // Save LoRA weights
    let lora_path = args.output.join("lora_weights.safetensors");
    if let Err(e) = pipeline.save_lora(&lora_path) {
        eprintln!("Warning: Failed to save LoRA weights: {}", e);
    }

    println!("\nDone.");
}
