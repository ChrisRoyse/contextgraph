# TASK-TRAIN-013 — CLI binary `mejepa-train` + `verify-chain` + `eval-holdout` subcommands

```xml
<task_spec id="TASK-TRAIN-013" version="1.0">

<metadata>
  <title>CLI entrypoint `cargo run --release -p context-graph-mejepa-train -- train --corpus ./data/mejepa-corpus --output ./data/mejepa-checkpoints` + verify-chain + eval-holdout subcommands using clap; surfaces structured exit codes 0/1/2/3 per TECH-TRAINING api_contracts</title>
  <status>blocked</status>
  <layer>surface</layer>
  <sequence>13</sequence>
  <implements>
    <item ref="REQ-TRAIN-01..12">end-to-end CLI surface</item>
    <item ref="api_contract">cargo run -- train; verify-chain; eval-holdout per TECH-TRAINING api_contracts</item>
    <item ref="exit_code_0">success; best.safetensors written; phase3_dod_passed=true</item>
    <item ref="exit_code_1">DoD failed; MEJEPA_TRAIN_DOD_FAILED on stderr</item>
    <item ref="exit_code_2">halted mid-run (NaN escalation, grad explode, OOM, chain broken, regression escalation, instrument gradient leak)</item>
    <item ref="exit_code_3">config invalid OR embedder digest mismatch; refuses to start</item>
  </implements>
  <depends_on>
    <task_ref>TASK-TRAIN-012</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>2 created</estimated_files>
</metadata>

<context>
The CLI entrypoint is the user-facing surface. Three subcommands: `train` (full training run), `verify-chain` (replay-only audit), `eval-holdout` (standalone holdout evaluation against an existing checkpoint). All flags map to TrainingConfig fields and override TOML defaults if `--config` is also supplied.
</context>

<input_context_files>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-TRAINING.md (UT-01 binary surface)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-TRAINING.md (api_contracts: train, verify-chain, eval-holdout endpoints with full flag list + response codes)</file>
  <file purpose="prior_tasks">TASK-TRAIN-002 (TrainingConfig + load_from_toml), TASK-TRAIN-009 (verify_chain), TASK-TRAIN-011 (HoldoutEvaluator), TASK-TRAIN-012 (Trainer::run_full_training)</file>
</input_context_files>

<prerequisites>
  <check>TASK-TRAIN-012 complete</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `src/bin/mejepa-train.rs` with clap-based CLI:
      ```
      mejepa-train train --corpus PATH --output PATH [--config PATH] [--epochs N] [--batch-size N] [--lr F] [--weight-decay F] [--warmup-steps N] [--max-grad-norm F] [--no-mixed-precision] [--full-finetune] [--lora-rank N] [--lora-alpha N] [--lora-dropout F] [--checkpoint-interval-steps N] [--holdout-eval-interval-steps N] [--counterfactual-interval-steps N] [--counterfactual-warmup-steps N] [--distillation-interval-steps N] [--cross-task-transfer-probability F] [--cross-task-cosine-threshold F] [--random-seed N] [--resume PATH]

      mejepa-train verify-chain --rocksdb PATH [--from-step N] [--to-step N]

      mejepa-train eval-holdout --weights PATH --holdout PATH [--constellation PATH] [--alpha F]
      ```
    - Create `src/cli/mod.rs` with three handlers:
      - `pub fn run_train(cli_args: TrainArgs) -&gt; std::process::ExitCode`
      - `pub fn run_verify_chain(cli_args: VerifyChainArgs) -&gt; std::process::ExitCode`
      - `pub fn run_eval_holdout(cli_args: EvalHoldoutArgs) -&gt; std::process::ExitCode`
    - run_train flow:
      1. Parse args; if `--config` supplied, `TrainingConfig::load_from_toml` then override with CLI flags
      2. `config.validate()` → on error: print MejepaTrainConfigInvalid + structured context to stderr; exit 3
      3. Open RocksDB at output_dir; create CFs CF_MEJEPA_TRAIN_CERTS, CF_MEJEPA_HOLDOUT_REPORTS, CF_MEJEPA_WEIGHT_BLOBS, CF_MEJEPA_EPOCH_WITNESS
      4. Verify embedder digests; on mismatch: emit MejepaTrainEmbedderDigestMismatch; exit 3
      5. Load corpus from --corpus path; build TrainingDataset via Stoer-Wagner split
      6. Construct Trainer; run_full_training; collect TrainingResult or TrainerError
      7. On Ok: print TrainingResult JSON to stdout; exit 0
      8. On Err with code MejepaTrainDodFailed: print structured error to stderr; exit 1
      9. On Err with halt codes (LossNan/GradExplode/VramOom/CheckpointCorrupt/CertChainBroken/HoldoutRegression/InstrumentGradientLeak): print structured error to stderr; exit 2
    - run_verify_chain: open rocksdb readonly; call verify_chain(); print ChainVerificationReport JSON; exit 0 on verified; exit 1 on broken_at Some
    - run_eval_holdout: load weights from `--weights`; load holdout from `--holdout`; optionally load constellation; run HoldoutEvaluator::evaluate; print HoldoutReport JSON; exit 0 on success; exit 1 on weights load fail
    - SIGINT handler: set abort flag; Trainer cleanly checkpoints Last + exits with note
    - Logging: emit per-step + per-1000-step + per-epoch summaries to stdout; redirect detailed cert JSON to RocksDB only
    - Unit tests: argument parsing; TrainingConfig override precedence (CLI > TOML > Default); exit-code mapping for each error variant
  </in_scope>
  <out_of_scope>
    - Real corpus loading (Phase 0 dep — placeholder loader for CLI flow)
    - Real predictor instantiation (Phase 2 dep)
    - Phase 4 inference integration
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-train/src/bin/mejepa-train.rs">
use clap::{Parser, Subcommand};
use context_graph_mejepa_train::cli;

#[derive(Parser)]
#[command(name = "mejepa-train", version, about = "ME-JEPA Phase 3 Training Loop CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Train(cli::TrainArgs),
    VerifyChain(cli::VerifyChainArgs),
    EvalHoldout(cli::EvalHoldoutArgs),
}

fn main() -&gt; std::process::ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Train(a) =&gt; cli::run_train(a),
        Command::VerifyChain(a) =&gt; cli::run_verify_chain(a),
        Command::EvalHoldout(a) =&gt; cli::run_eval_holdout(a),
    }
}
    </signature>
    <signature file="crates/context-graph-mejepa-train/src/cli/mod.rs">
use std::path::PathBuf;
use std::process::ExitCode;
use clap::Args;

#[derive(Args, Debug, Clone)]
pub struct TrainArgs {
    #[arg(long)] pub corpus: PathBuf,
    #[arg(long)] pub output: PathBuf,
    #[arg(long)] pub config: Option&lt;PathBuf&gt;,
    #[arg(long)] pub epochs: Option&lt;u32&gt;,
    #[arg(long, name = "batch-size")] pub batch_size: Option&lt;usize&gt;,
    #[arg(long)] pub lr: Option&lt;f64&gt;,
    #[arg(long, name = "weight-decay")] pub weight_decay: Option&lt;f64&gt;,
    #[arg(long, name = "warmup-steps")] pub warmup_steps: Option&lt;u32&gt;,
    #[arg(long, name = "max-grad-norm")] pub max_grad_norm: Option&lt;f32&gt;,
    #[arg(long, name = "no-mixed-precision")] pub no_mixed_precision: bool,
    #[arg(long, name = "full-finetune")] pub full_finetune: bool,
    #[arg(long, name = "lora-rank")] pub lora_rank: Option&lt;usize&gt;,
    #[arg(long, name = "lora-alpha")] pub lora_alpha: Option&lt;usize&gt;,
    #[arg(long, name = "lora-dropout")] pub lora_dropout: Option&lt;f32&gt;,
    #[arg(long, name = "checkpoint-interval-steps")] pub checkpoint_interval_steps: Option&lt;u64&gt;,
    #[arg(long, name = "holdout-eval-interval-steps")] pub holdout_eval_interval_steps: Option&lt;u64&gt;,
    #[arg(long, name = "counterfactual-interval-steps")] pub counterfactual_interval_steps: Option&lt;u64&gt;,
    #[arg(long, name = "counterfactual-warmup-steps")] pub counterfactual_warmup_steps: Option&lt;u64&gt;,
    #[arg(long, name = "distillation-interval-steps")] pub distillation_interval_steps: Option&lt;u64&gt;,
    #[arg(long, name = "cross-task-transfer-probability")] pub cross_task_transfer_probability: Option&lt;f32&gt;,
    #[arg(long, name = "cross-task-cosine-threshold")] pub cross_task_cosine_threshold: Option&lt;f32&gt;,
    #[arg(long, name = "random-seed")] pub random_seed: u64, // required
    #[arg(long)] pub resume: Option&lt;PathBuf&gt;,
}

#[derive(Args, Debug, Clone)]
pub struct VerifyChainArgs {
    #[arg(long)] pub rocksdb: PathBuf,
    #[arg(long, name = "from-step")] pub from_step: Option&lt;u64&gt;,
    #[arg(long, name = "to-step")] pub to_step: Option&lt;u64&gt;,
}

#[derive(Args, Debug, Clone)]
pub struct EvalHoldoutArgs {
    #[arg(long)] pub weights: PathBuf,
    #[arg(long)] pub holdout: PathBuf,
    #[arg(long)] pub constellation: Option&lt;PathBuf&gt;,
    #[arg(long, default_value_t = 0.10)] pub alpha: f32,
}

pub fn run_train(args: TrainArgs) -&gt; ExitCode;
pub fn run_verify_chain(args: VerifyChainArgs) -&gt; ExitCode;
pub fn run_eval_holdout(args: EvalHoldoutArgs) -&gt; ExitCode;
    </signature>
  </signatures>
  <constraints>
    - Cargo.toml `[[bin]]` entry `name = "mejepa-train" path = "src/bin/mejepa-train.rs"`
    - Exit codes 0/1/2/3 per TECH-TRAINING api_contracts
    - Errors print as JSON to stderr (machine-parseable)
    - Successful TrainingResult prints as JSON to stdout
    - SIGINT installed via `signal-hook` or std::os::unix conditional; abort_flag.store(true, SeqCst)
    - File &lt; 500 lines per file
  </constraints>
  <verification>
    - `cargo run --release -p context-graph-mejepa-train -- --help` prints usage
    - `cargo run --release -p context-graph-mejepa-train -- train --help` prints all flags
    - Argument parsing tests pass (synthetic args)
    - Override precedence test: CLI flag overrides TOML; both override default
  </verification>
</definition_of_done>

<pseudo_code>
pub fn run_train(args: TrainArgs) -&gt; ExitCode {
    let mut config = match &amp;args.config {
        Some(p) => match TrainingConfig::load_from_toml(p) {
            Ok(c) => c,
            Err(e) => { eprintln!("{}", serde_json::to_string(&amp;e).unwrap()); return ExitCode::from(3); }
        }
        None => TrainingConfig::default(),
    };
    apply_cli_overrides(&amp;mut config, &amp;args);
    config.random_seed = args.random_seed;
    if let Err(e) = config.validate() {
        eprintln!("{}", serde_json::to_string(&amp;e).unwrap());
        return ExitCode::from(3);
    }
    let rocksdb = open_or_create(&amp;args.output)?;
    let device = candle_core::Device::cuda_if_available(0).unwrap_or(candle_core::Device::Cpu);
    let mut trainer = match Trainer::new(config, rocksdb, device) {
        Ok(t) => t,
        Err(e) => { eprintln!("{}", serde_json::to_string(&amp;e).unwrap()); return ExitCode::from(3); }
    };
    let dataset = load_corpus(&amp;args.corpus)?;
    match trainer.run_full_training(&amp;dataset) {
        Ok(result) => { println!("{}", serde_json::to_string(&amp;result).unwrap()); ExitCode::from(0) }
        Err(e) =&gt; {
            eprintln!("{}", serde_json::to_string(&amp;e).unwrap());
            match e.code {
                "MEJEPA_TRAIN_DOD_FAILED" =&gt; ExitCode::from(1),
                "MEJEPA_TRAIN_CONFIG_INVALID" | "MEJEPA_TRAIN_EMBEDDER_DIGEST_MISMATCH" =&gt; ExitCode::from(3),
                _ =&gt; ExitCode::from(2),
            }
        }
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-train/src/bin/mejepa-train.rs">CLI main</file>
  <file path="crates/context-graph-mejepa-train/src/cli/mod.rs">TrainArgs + VerifyChainArgs + EvalHoldoutArgs + handlers + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-train/Cargo.toml">add [[bin]] entry; add signal-hook 0.3 dep (optional)</file>
  <file path="crates/context-graph-mejepa-train/src/lib.rs">pub mod cli;</file>
</files_to_modify>

<validation_criteria>
  <criterion>--help prints usage</criterion>
  <criterion>train --help shows all 22 flags</criterion>
  <criterion>verify-chain on synthetic 100-cert chain returns exit 0</criterion>
  <criterion>verify-chain with tampered cert returns exit 1 + JSON error</criterion>
  <criterion>config-invalid path triggers exit 3</criterion>
  <criterion>DoD failure path triggers exit 1</criterion>
  <criterion>halt path (e.g., synthetic InstrumentGradientLeak) triggers exit 2</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-train --bin mejepa-train</command>
  <command>cargo run --release -p context-graph-mejepa-train --bin mejepa-train -- --help</command>
  <command>cargo run --release -p context-graph-mejepa-train --bin mejepa-train -- train --help</command>
  <command>cargo test -p context-graph-mejepa-train --lib cli</command>
  <command>cargo clippy -p context-graph-mejepa-train -- -D warnings</command>
</test_commands>

</task_spec>
```
